//! Saving and restoring state.

use super::*;

/// The tabs open when kura closed, restored when it starts without a path.
#[derive(Default, Debug, serde::Serialize, serde::Deserialize)]
struct SessionFile {
    #[serde(default)]
    active: usize,
    #[serde(default)]
    tabs: Vec<String>,
}

/// Whether `dir` is where a shortcut or the Start menu starts a program, rather
/// than somewhere a shell was deliberately `cd`'d to: home, or inside the
/// system folder (where an elevated launch lands).
pub fn is_default_start(dir: &Path) -> bool {
    if ito_common::home_dir().is_some_and(|h| h == dir) {
        return true;
    }
    #[cfg(windows)]
    if let Some(root) = std::env::var_os("SystemRoot") {
        return dir.starts_with(root);
    }
    false
}

impl App {
    pub(super) fn state_file(name: &str) -> PathBuf {
        Config::state_dir().join(name)
    }

    pub(super) fn load_state(&mut self) {
        if let Ok(text) = std::fs::read_to_string(Self::state_file("bookmarks.toml")) {
            if let Ok(f) = toml::from_str::<BookmarkFile>(&text) {
                self.bookmarks = f.bookmark;
            }
        }
        // The state files are TOML. A `.txt` of an older build is read when
        // there is no `.toml` yet, and the `.toml` written from it at once; the
        // `.txt` is left where it is.
        if let Some(f) = ito_common::kept::read::<HistoryFile>(self.kept.history.path()) {
            self.history = f.entries.into_iter().map(visit_of).collect();
        } else if let Ok(text) = std::fs::read_to_string(Self::state_file("history.txt")) {
            self.history = text.lines().map(parse_visit).collect();
            self.save_history();
        }
        for (group, txt) in Self::INPUT_HISTORY_TXT.into_iter().enumerate() {
            if let Some(f) = ito_common::kept::read::<InputFile>(self.kept.input[group].path()) {
                self.input_history[group] = f.entries.into_iter().filter(|l| !l.is_empty()).collect();
            } else if let Ok(text) = std::fs::read_to_string(Self::state_file(txt)) {
                self.input_history[group] = text.lines().filter(|l| !l.is_empty()).map(str::to_owned).collect();
                self.kept.input[group].save_toml(&InputFile { entries: self.input_history[group].clone() });
            }
        }
    }

    pub fn save_state(&self) {
        let dir = Config::state_dir();
        if std::fs::create_dir_all(&dir).is_err() {
            return;
        }
        let f = BookmarkFile { bookmark: self.bookmarks.clone() };
        if let Ok(text) = toml::to_string_pretty(&f) {
            let _ = std::fs::write(dir.join("bookmarks.toml"), text);
        }
        self.save_history();
    }

    pub(super) fn history_file(&self) -> HistoryFile {
        HistoryFile {
            entries: self.history.iter().map(|v| VisitRow { path: v.path.display().to_string(), hits: v.hits, at: v.at }).collect(),
        }
    }

    pub(super) fn save_history(&self) {
        self.kept.history.save_toml(&self.history_file());
    }

    /// The tabs as `session.toml` keeps them: every folder, and which tab was in front.
    pub(super) fn session_text(&self) -> Option<String> {
        let f = SessionFile { active: self.active, tabs: self.tabs.iter().map(|t| t.cwd.display().to_string()).collect() };
        toml::to_string_pretty(&f).ok()
    }

    /// Open the tabs a session file names, in place of the one the app started with.
    ///
    /// Folders are taken on faith like the start folder is (`start_unproven`): a
    /// folder that is gone, or a share that does not answer, falls back to `home`
    /// when its listing fails instead of holding the window back. Returns whether
    /// anything was restored.
    pub fn restore_session(&mut self, text: &str, home: &Path, mode: Restore) -> bool {
        let Ok(mut f) = toml::from_str::<SessionFile>(text) else {
            return false;
        };
        if mode == Restore::Off {
            return false;
        }
        let mut dirs: Vec<PathBuf> = f.tabs.iter().filter(|t| !t.is_empty()).map(PathBuf::from).take(MAX_TABS).collect();
        if dirs.is_empty() {
            return false;
        }
        if mode == Restore::Folder {
            // Only the folder that was in front, in the one tab.
            let front = dirs.swap_remove(f.active.min(dirs.len() - 1));
            dirs = vec![front];
            f.active = 0;
        }
        let (sort, hidden, linemode) = (self.tabs[0].sort, self.tabs[0].show_hidden, self.tabs[0].linemode);
        self.tabs = dirs
            .into_iter()
            .map(|dir| {
                let unproven = dir != home;
                let mut tab = Tab::new(dir, sort, hidden, linemode);
                if unproven {
                    tab.pending_cd = Some(PendingCd { from: home.to_path_buf(), pushed: false, fallback: true, reveal: None });
                }
                tab
            })
            .collect();
        self.active = f.active.min(self.tabs.len() - 1);
        self.split = None;
        self.switch_tab(self.active);
        true
    }

    /// Restore the last session from the state folder, if the file is there.
    ///
    /// `asked` is `--last`: it restores every tab even when `[ui] restore` says `off`.
    pub fn restore_last_session(&mut self, home: &Path, asked: bool) -> bool {
        let mut mode = self.cfg.ui.restore_mode();
        if asked && mode == Restore::Off {
            mode = Restore::Tabs;
        }
        if mode == Restore::Off {
            return false;
        }
        std::fs::read_to_string(Self::state_file("session.toml")).map(|text| self.restore_session(&text, home, mode)).unwrap_or(false)
    }

    /// What is kept when kura ends by any road, `q` or the window's x: bookmarks,
    /// history and the session.
    pub fn save_on_close(&self) {
        self.save_state();
        if let Some(text) = self.session_text() {
            let _ = std::fs::write(Self::state_file("session.toml"), text);
        }
        // The thread of the save above may not finish before the process ends.
        if let Ok(text) = toml::to_string(&self.history_file()) {
            let _ = self.kept.history.save_now(text);
        }
    }

    /// `q`: what is kept on any close, and the files `--cwd-file` and `--chooser-file` name.
    pub fn on_quit(&self) {
        self.save_on_close();
        if let Some(f) = &self.cwd_file {
            let _ = std::fs::write(f, self.tabs[self.active].cwd.display().to_string());
        }
        if let Some(f) = &self.chooser_file {
            let paths = self.tabs[self.active].targets();
            let text: Vec<String> = paths.iter().map(|p| p.display().to_string()).collect();
            let _ = std::fs::write(f, text.join("\n"));
        }
    }
}
