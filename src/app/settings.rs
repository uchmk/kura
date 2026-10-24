//! Reading and writing the shared and kura settings, themes, and the settings panel.

use super::*;

impl App {
    /// Take the clock, the language and the scale from the shared common.toml
    /// (`ito_common`, uchmk's common spec), then watch it and kura's own files.
    pub fn load_common(&mut self) {
        self.os_light = self.ctx.system_theme() == Some(egui::Theme::Light);
        let Some(base) = ito_common::base_dir() else { return };
        self.set_themes(read_themes(&base));
        self.set_common(ito_common::Common::read(&base));
        self.start_watching(&base);
        self.common_base = Some(base);
    }

    /// Look every 2 seconds, on a thread of its own, at common.toml, which
    /// another uchmk app may have changed, and at kura's config files, which
    /// are read again by themselves once saved (`<C-F5>` still does it at
    /// once). A changed file is read on that thread and applied by
    /// `drain_channels`, with no restart.
    pub(super) fn start_watching(&mut self, base: &Path) {
        let (common_tx, common_rx) = crossbeam_channel::unbounded();
        let (tx, rx) = crossbeam_channel::unbounded();
        let common = ito_common::common_path(base);
        let paths: Vec<PathBuf> = std::iter::once(common.clone()).chain(crate::config::watched_files()).collect();
        let (base, ctx, to_common, to_app) = (base.to_path_buf(), self.ctx.clone(), common_tx.clone(), tx.clone());
        let to_themes = self.themes_tx.clone();
        ito_common::watch(paths, Duration::from_secs(2), move |changed| {
            let mut alive = true;
            if changed.contains(&common) {
                // The themes first: the `theme` the file names may be one
                // just added.
                alive &= to_themes.send(read_themes(&base)).is_ok();
                alive &= to_common.send(ito_common::Common::read(&base)).is_ok();
            }
            if changed.iter().any(|p| *p != common) {
                alive &= to_app.send(Watched::Config(Box::new(Config::read_fresh(&crate::config::dirs_to_read())))).is_ok();
            }
            ctx.request_repaint();
            alive
        });
        self.common_rx = Some(common_rx);
        self.common_tx = Some(common_tx);
        self.watched_rx = Some(rx);
        self.watched_tx = Some(tx);
    }

    /// What the watcher read of kura's own files, taken in.
    pub(super) fn drain_watched(&mut self) {
        let Some(rx) = &self.watched_rx else { return };
        let got: Vec<Watched> = rx.try_iter().collect();
        for w in got {
            match w {
                Watched::Config(fresh) => {
                    let mut cfg = Config::settle(&mut self.cfg, *fresh);
                    cfg.take_term_env();
                    // The settings screen's own write, coming round again.
                    if self.kura_wrote.take().is_some_and(|t| t.elapsed() < Duration::from_secs(5)) {
                        self.take_config_saying(cfg, Some(String::new()));
                    } else {
                        self.take_config(cfg);
                    }
                }
                Watched::Failed(e) => self.warn(e),
            }
        }
    }

    /// Apply what common.toml said. A file that does not read keeps what was
    /// there before (the defaults at start-up), so a save half-way through an
    /// edit does not flip the clock; either way the problem is a toast. A
    /// file gone is the defaults again, as at a start without one.
    pub fn set_common(&mut self, read: CommonRead) {
        match read {
            Ok((common, warnings)) => {
                let pressed = self.scale_pressed.is_some_and(|t| t.elapsed() < Duration::from_secs(5));
                if !self.scale_pinned && !pressed {
                    self.set_scale(common.scale_or_one());
                }
                self.clock = common.clock_or(None);
                self.lang = crate::i18n::resolve(common.language.as_deref(), ito_common::os_language().as_deref());
                if common.mono_font_or_default() != self.common.mono_font_or_default() {
                    self.refont = true;
                }
                self.common = common;
                if let Some(w) = warnings.first() {
                    self.warn(w.clone());
                }
                self.resolve_theme();
            }
            Err(e) => self.warn(e),
        }
    }

    /// Take the themes there are, read again; a theme file that does not
    /// read is said once, not at every look at common.toml.
    pub fn set_themes(&mut self, (themes, bad): Themes) {
        if bad != self.themes_bad {
            if let Some(b) = bad.first() {
                self.warn(format!("theme: {b}"));
            }
            self.themes_bad = bad;
        }
        if themes != self.themes {
            self.themes = themes;
            self.resolve_theme();
        }
    }

    /// Put in force the theme common.toml's `theme` chooses (Q98): its
    /// colours under kura's, and `theme.toml`'s over them. A name no theme
    /// has keeps the colours there were, and is said once.
    pub fn resolve_theme(&mut self) {
        let (choice, dark, light) = self.common.theme_choice(None);
        match ito_theme::pick(&self.themes, &choice, &dark, &light, self.os_light) {
            Ok(t) => {
                self.theme_said = None;
                let c = t.colors;
                if self.cfg.theme.common != c {
                    self.cfg.theme = Arc::new(self.cfg.theme.with_common(&c));
                    self.look_changed();
                }
            }
            Err(e) => {
                let e = format!("common.toml: {e}");
                if self.theme_said.as_ref() != Some(&e) {
                    self.warn(e.clone());
                    self.theme_said = Some(e);
                }
            }
        }
    }

    /// The OS turned light or dark, which `theme = "system"` follows.
    pub fn set_os_light(&mut self, light: bool) {
        if light != self.os_light {
            self.os_light = light;
            self.resolve_theme();
        }
    }

    /// The colours changed: egui's own widgets follow, and the preview holds
    /// a highlighted copy in the old ones.
    pub(super) fn look_changed(&mut self) {
        crate::ui::set_look(&self.ctx, self.cfg.theme.light());
        self.preview = PreviewSlot::default();
    }

    /// Write what the settings screen changed into common.toml, off the UI
    /// thread (`edit_common` keeps the comments and takes one write at a
    /// time), then read the file back and apply it like the watcher does.
    /// Every other uchmk app sees the same change through its own watcher.
    pub fn write_common(&mut self, changes: Vec<ito_common::CommonChange>) {
        if changes.is_empty() {
            return;
        }
        let Some(base) = self.common_base.clone() else {
            self.error("common.toml: there is no config folder to keep it in");
            return;
        };
        let tx = self.common_tx.clone();
        let to_themes = self.themes_tx.clone();
        let ctx = self.ctx.clone();
        std::thread::spawn(move || {
            let written = ito_common::edit_common(&base, |text| changes.iter().try_fold(text.to_owned(), |t, c| c.apply(&t)));
            let read = match written {
                Ok(()) => ito_common::Common::read(&base),
                Err(e) if e.contains("common.toml") => Err(e),
                Err(e) => Err(format!("common.toml: {e}")),
            };
            let _ = to_themes.send(read_themes(&base));
            if let Some(tx) = tx {
                let _ = tx.send(read);
            }
            ctx.request_repaint();
        });
    }

    /// kura's own config folder, where its config.toml is (`KURA_CONFIG_HOME`).
    pub fn kura_config_dir() -> Option<PathBuf> {
        crate::config::config_home("KURA_CONFIG_HOME")
    }

    /// The config.toml whose settings are in force: the last one read, since
    /// a later folder's `[ui]` replaces an earlier one's whole. Without any,
    /// the one kura's own folder would hold. Writing anywhere else would
    /// start a file that hides the one in use.
    pub fn own_config_in_force(&self) -> Option<PathBuf> {
        let read = self.cfg.loaded.iter().rev().find(|p| p.file_name().is_some_and(|n| n == crate::config::OWN_FILE));
        read.cloned().or_else(|| Self::kura_config_dir().map(|d| d.join(crate::config::OWN_FILE)))
    }

    /// Write what the settings screen's config.toml page changed, off the UI
    /// thread and keeping the file's comments, then read the config back
    /// and apply it as `<C-F5>` does.
    pub fn write_kura(&mut self, changes: Vec<KuraChange>) {
        if changes.is_empty() {
            return;
        }
        let Some(path) = self.own_config_in_force() else {
            self.error("config.toml: there is no config folder to keep it in");
            return;
        };
        self.write_kura_to(path, crate::config::dirs_to_read(), changes);
    }

    /// [`App::write_kura`] to a file named here, reading the config back from
    /// `dirs`: what a test drives with a folder of its own.
    pub(crate) fn write_kura_to(&mut self, path: PathBuf, dirs: Vec<PathBuf>, changes: Vec<KuraChange>) {
        self.send_kura_job(KuraJob { path, dirs, changes, keys: Vec::new(), webhooks: None });
    }

    /// The `[[notify.webhooks]]` the Notifications page changed, written to
    /// the config.toml in force.
    pub fn write_webhooks(&mut self, places: Vec<ito_notify::webhook::Place>) {
        let Some(path) = self.own_config_in_force() else {
            self.error("config.toml: there is no config folder to keep it in");
            return;
        };
        self.send_kura_job(KuraJob {
            path,
            dirs: crate::config::dirs_to_read(),
            changes: Vec::new(),
            keys: Vec::new(),
            webhooks: Some(places),
        });
    }

    pub(super) fn send_kura_job(&mut self, job: KuraJob) {
        if self.kura_jobs.is_none() {
            self.kura_jobs = kura_writer(self.kura_tx.clone(), self.ctx.clone());
        }
        let sent = self.kura_jobs.as_ref().is_some_and(|tx| tx.send(job).is_ok());
        if !sent {
            self.kura_jobs = None;
            self.error("config.toml: could not start writing it");
        }
    }

    /// The keymap.toml the Keys page writes: the one in kura's own folder,
    /// never yazi's. Keymaps stack, and kura's folder is read last, so its
    /// lines win wherever a config.toml happens to be.
    #[cfg(not(test))]
    pub fn own_keymap() -> Option<PathBuf> {
        Some(Self::kura_config_dir()?.join("keymap.toml"))
    }

    /// Tests name their own keymap.toml, never the real config folder's.
    #[cfg(test)]
    pub fn own_keymap() -> Option<PathBuf> {
        TEST_KEYMAP.with(|t| t.borrow().clone())
    }

    /// [`App::own_keymap`] spelled as the bindings read from it name it:
    /// with `YAZI_CONFIG_HOME` naming the same folder, the config reads it
    /// by yazi's spelling. Compared by letters only, as this runs every frame.
    pub fn own_keymap_named(&self) -> Option<String> {
        let own = Self::own_keymap()?;
        let key = |p: &Path| {
            let p: PathBuf = p.components().collect();
            if cfg!(windows) {
                p.to_string_lossy().to_lowercase()
            } else {
                p.to_string_lossy().into_owned()
            }
        };
        let read = self.cfg.loaded.iter().find(|p| key(p) == key(&own));
        Some(read.unwrap_or(&own).display().to_string())
    }

    /// Write what the settings screen's Keys page changed into kura's own
    /// keymap.toml, off the UI thread and keeping the file's comments, then
    /// read the config back so the new keys work at once.
    pub fn write_keymap(&mut self, edits: Vec<ito_keymap::Edit>) {
        if edits.is_empty() {
            return;
        }
        let Some(path) = Self::own_keymap() else {
            self.error("keymap.toml: there is no config folder to keep it in");
            return;
        };
        self.write_keymap_to(path, crate::config::dirs_to_read(), edits);
    }

    /// [`App::write_keymap`] to a file named here, reading the config back
    /// from `dirs`.
    pub(crate) fn write_keymap_to(&mut self, path: PathBuf, dirs: Vec<PathBuf>, keys: Vec<ito_keymap::Edit>) {
        if self.kura_jobs.is_none() {
            self.kura_jobs = kura_writer(self.kura_tx.clone(), self.ctx.clone());
        }
        let sent =
            self.kura_jobs.as_ref().is_some_and(|tx| tx.send(KuraJob { path, dirs, changes: Vec::new(), keys, webhooks: None }).is_ok());
        if !sent {
            self.kura_jobs = None;
            self.error("keymap.toml: could not start writing it");
        }
    }

    /// The writer's answer: the config it read back, applied with what a
    /// broken file gave last time kept, or what stopped the write.
    pub(crate) fn kura_written(&mut self, w: KuraWritten) {
        match w.read {
            Ok(fresh) => {
                let mut cfg = Config::settle(&mut self.cfg, fresh);
                cfg.take_term_env();
                self.take_config_saying(cfg, Some(w.said));
                if w.wrote {
                    self.kura_wrote = Some(Instant::now());
                }
            }
            Err(e) => self.error(e),
        }
    }

    /// Open config.toml in the system's editor for it, off the UI thread. A
    /// first one is made, with a line saying what goes in it, so the button
    /// works before there is anything to change.
    pub fn open_own_config(&mut self) {
        let Some(path) = self.own_config_in_force() else {
            self.error("config.toml: there is no config folder to keep it in");
            return;
        };
        let tx = self.settings_tx.clone();
        let ctx = self.ctx.clone();
        std::thread::spawn(move || {
            let made = match path.parent() {
                _ if path.exists() => Ok(()),
                Some(dir) => std::fs::create_dir_all(dir).and_then(|()| std::fs::write(&path, OWN_FILE_START)),
                None => std::fs::write(&path, OWN_FILE_START),
            };
            if let Err(e) = made.and_then(|()| exec::open_default(&path)) {
                let _ = tx.send((true, format!("{}: {e}", path.display())));
                ctx.request_repaint();
            }
        });
    }

    /// Write the settings to a file, or read them back from one
    /// (`ito_common::bundle`), on a thread of its own. The result is said by
    /// `drain_channels`; common.toml and kura's files are read again by their watchers.
    pub fn settings_backup(&mut self, what: ito_prefs::Backup) {
        let (Some(base), Some(app_dir)) = (ito_common::base_dir(), ito_common::app_dir("kura")) else {
            self.error("settings: there is no config folder");
            return;
        };
        let (tx, ctx) = (self.settings_tx.clone(), self.ctx.clone());
        std::thread::spawn(move || {
            let said = match what {
                ito_prefs::Backup::Export(file) => {
                    ito_common::bundle::export(&file, &base, &app_dir, "kura", env!("CARGO_PKG_VERSION"), &ito_common::bundle::all)
                        .map(|n| format!("Wrote {n} settings files to {}", file.display()))
                }
                ito_prefs::Backup::Import(file) => ito_common::bundle::import(&file, &base, &app_dir, "kura", &ito_common::bundle::all)
                    .map(|r| {
                        let mut s = format!("Read {} settings files from {} ({} unchanged)", r.written.len(), file.display(), r.unchanged);
                        for (name, why) in &r.skipped {
                            s.push_str(&format!("; skipped {name}: {why}"));
                        }
                        s
                    }),
            };
            let _ = tx.send(match said {
                Ok(s) => (false, s),
                Err(e) => (true, e),
            });
            ctx.request_repaint();
        });
    }

    /// Start over (the Advanced page's Reset): config.toml and keymap.toml are
    /// moved aside as `*.bak-<seconds>`, on a thread, and the watchers read
    /// the defaults back. common.toml and the themes are left alone.
    pub fn settings_reset(&mut self) {
        let files: Vec<PathBuf> = self.own_config_in_force().into_iter().chain(Self::own_keymap()).collect();
        // The watchers read the defaults back a moment later; their "Reloaded"
        // toast would replace the one that says how many files were kept.
        if files.iter().any(|f| f.exists()) {
            self.kura_wrote = Some(Instant::now());
        }
        let (tx, ctx) = (self.settings_tx.clone(), self.ctx.clone());
        std::thread::spawn(move || {
            let _ = tx.send(match move_aside(&files) {
                Ok(0) => (false, "Nothing to reset: no config.toml or keymap.toml".to_owned()),
                Ok(n) => (false, format!("Settings reset; {n} file(s) kept as *.bak-…")),
                Err(e) => (true, format!("Reset failed: {e}")),
            });
            ctx.request_repaint();
        });
    }

    /// Open the settings screen, or close it.
    pub fn toggle_settings(&mut self) {
        self.overlay = match self.overlay {
            Overlay::Settings(_) => Overlay::None,
            _ => Overlay::Settings(SettingsOverlay { startup_rx: Some(startup_worker(None, self.ctx.clone())), ..Default::default() }),
        };
        self.pending.clear();
    }

    /// Take the worker's answer about starting at sign-in, and start one when
    /// the switch was clicked (`toggled` is the new state).
    pub fn settings_startup(&mut self, toggled: Option<bool>) {
        let ctx = self.ctx.clone();
        let mut said = None;
        let Overlay::Settings(ov) = &mut self.overlay else { return };
        if let Some(on) = toggled {
            ov.startup_rx = Some(startup_worker(Some(on), ctx));
            ov.startup = None;
        }
        if let Some((now, err)) = ov.startup_rx.as_ref().and_then(|rx| rx.try_recv().ok()) {
            ov.startup = Some(now);
            ov.startup_rx = None;
            said = err;
        }
        if let Some(e) = said {
            self.error(format!("Start at sign-in: {e}"));
        }
    }

    /// The settings screen's own keys, from the `[settings]` layer. The rest
    /// go to its controls, which egui hands them to.
    pub(super) fn settings_act(&mut self, a: Act) {
        let Overlay::Settings(ov) = &mut self.overlay else { return };
        match a {
            Act::Close | Act::Escape(_) | Act::Quit | Act::Settings => self.overlay = Overlay::None,
            Act::TabSwitch { n, relative: true } if n > 0 => ov.keys.next = true,
            Act::TabSwitch { n, relative: true } if n < 0 => ov.keys.back = true,
            Act::Find { .. } => ov.keys.find = true,
            _ => {}
        }
    }

    /// Treat the directory the app opened on as unproven, the way a typed `cd`
    /// is: nothing checked it before the window went up, since `is_dir` on a
    /// dead share can hold the first frame for half a minute. If the first
    /// listing fails, the tab falls back to `home` — by way of the parent, so
    /// `kura C:\dir\file.txt` reveals the file instead of giving up.
    pub fn start_unproven(&mut self, home: PathBuf) {
        if self.tabs[self.active].cwd == home {
            return;
        }
        self.tabs[self.active].pending_cd = Some(PendingCd { from: home, pushed: false, fallback: true, reveal: None });
    }
}
