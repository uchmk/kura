//! Prompts and typed input: the input box, its history, config reload and the remaining small prompts.

use super::*;

impl App {
    /// A prompt that opens with text in it opens with that text selected, as
    /// Explorer's address bar and `F2` do: typing or pasting replaces it, and
    /// `<End>` keeps it to go on from (Q31). Two keep their own shape: a shell
    /// command is a template you add to, so the caret waits at its end, and an
    /// archive's name selects the part before `.zip`, which is what you change.
    pub fn open_input(&mut self, kind: InputKind, title: &str, text: String) {
        let len = text.chars().count();
        let selection = match kind {
            InputKind::Shell { .. } => (len, len),
            InputKind::Compress => (0, util::stem_and_ext(&text).0.chars().count()),
            _ => (0, len),
        };
        self.input_recall = None;
        self.overlay = Overlay::Input(InputOverlay {
            kind,
            title: title.to_owned(),
            text,
            initial_selection: Some(selection),
            focused: false,
            completion: Vec::new(),
            completion_at: 0,
        });
    }

    /// Which list of past input a prompt draws on: search words (shared by
    /// `s S F f / ?` and the terminal's find) or shell commands.
    pub fn input_group(kind: &InputKind) -> Option<usize> {
        match kind {
            InputKind::Search { .. } | InputKind::Filter | InputKind::Find { .. } | InputKind::TermFind | InputKind::PreviewFind => Some(0),
            InputKind::Shell { .. } => Some(1),
            _ => None,
        }
    }

    pub(super) const INPUT_HISTORY_TXT: [&'static str; 2] = ["search-history.txt", "shell-history.txt"];

    pub(super) const INPUT_HISTORY_MAX: usize = 100;

    /// Remember what was submitted: newest last, a repeat moves up rather than
    /// appearing twice.
    pub(super) fn remember_input(&mut self, group: usize, text: &str) {
        if text.is_empty() {
            return;
        }
        let list = &mut self.input_history[group];
        list.retain(|t| t != text);
        list.push(text.to_owned());
        if list.len() > Self::INPUT_HISTORY_MAX {
            list.remove(0);
        }
        self.kept.input[group].save_toml(&InputFile { entries: list.clone() });
    }

    /// `<Up>` / `<Down>` in a prompt: step back through what was typed before,
    /// and forward again to the unfinished line.
    pub fn recall_input(&mut self, older: bool) {
        let Overlay::Input(ov) = &self.overlay else { return };
        let Some(group) = Self::input_group(&ov.kind) else { return };
        let list = &self.input_history[group];
        if list.is_empty() {
            return;
        }
        let (at, text) = match (&self.input_recall, older) {
            (None, true) => (Some(list.len() - 1), None),
            (None, false) => return,
            (Some((i, draft)), true) => (Some(i.saturating_sub(1)), Some(draft.clone())),
            (Some((i, draft)), false) if i + 1 < list.len() => (Some(i + 1), Some(draft.clone())),
            (Some((_, draft)), false) => (None, Some(draft.clone())),
        };
        let draft = text.unwrap_or_else(|| ov.text.clone());
        let shown = at.map_or_else(|| draft.clone(), |i| list[i].clone());
        self.input_recall = at.map(|i| (i, draft));
        if let Overlay::Input(ov) = &mut self.overlay {
            let n = shown.chars().count();
            ov.text = shown;
            ov.initial_selection = Some((n, n));
            ov.focused = false;
        }
        self.input_changed();
    }

    /// Called on every keystroke for the live-updating inputs.
    pub fn input_changed(&mut self) {
        let Overlay::Input(ov) = &self.overlay else { return };
        match ov.kind.clone() {
            InputKind::Filter => {
                // Half-typed, `(` is not an expression yet: the list stays as
                // it was and the prompt says why, until the next key fixes it.
                let (title, filter) = match Filter::new(ov.text.clone()) {
                    Ok(f) => ("Filter".to_owned(), Some(f)),
                    Err(e) => (format!("Filter -- {e}"), None),
                };
                if let Overlay::Input(ov) = &mut self.overlay {
                    ov.title = title;
                }
                if let Some(filter) = filter {
                    let tab = &mut self.tabs[self.active];
                    tab.current.filter = Some(filter);
                    let show = tab.show_hidden;
                    tab.current.rebuild(show);
                }
            }
            InputKind::Find { prev } => {
                let query = ov.text.clone();
                // As `f` does: half-typed, `(` is not an expression yet, so
                // nothing moves and the prompt says why until the next key.
                let (title, matcher) = match ito_match::Matcher::new(&query) {
                    _ if query.is_empty() => (None, None),
                    Ok(m) => (None, Some(m)),
                    Err(e) => (Some(e), None),
                };
                let base = if prev { "Find previous" } else { "Find next" };
                if let Overlay::Input(ov) = &mut self.overlay {
                    ov.title = match &title {
                        Some(e) => format!("{base} -- {e}"),
                        None => base.to_owned(),
                    };
                }
                let tab = &mut self.tabs[self.active];
                if title.is_some() {
                    return;
                }
                tab.finder = Some(Finder { query, matcher: matcher.clone(), prev, names: true, ..Default::default() });
                let Some(matcher) = matcher else { return };
                let len = tab.current.view.len();
                let start = tab.current.cursor;
                for step in 0..len {
                    let idx = if prev { (start + len - step) % len } else { (start + step) % len };
                    let Some(e) = tab.current.at(idx) else { continue };
                    if matcher.is_match(&e.name) {
                        tab.current.cursor = idx;
                        break;
                    }
                }
            }
            _ => {}
        }
    }

    pub fn submit_input(&mut self) {
        let Overlay::Input(ov) = std::mem::replace(&mut self.overlay, Overlay::None) else {
            return;
        };
        self.pending_completion = None;
        let text = ov.text.trim().to_owned();
        if let Some(group) = Self::input_group(&ov.kind) {
            self.remember_input(group, &text);
        }
        self.input_recall = None;
        match ov.kind {
            InputKind::Create => self.do_create(&text),
            InputKind::Compress => self.do_compress(&text),
            InputKind::TermFind => self.term_find(&text, true),
            InputKind::PreviewFind => self.preview_find_submit(&text),
            InputKind::Rename { from } => self.do_rename(&from, &text),
            InputKind::Bulk { paths } => self.do_bulk_rename(&paths, &text),
            InputKind::Filter => { /* already applied live */ }
            InputKind::Find { .. } => { /* already applied live */ }
            InputKind::Cd => {
                if !text.is_empty() {
                    let base = self.tabs[self.active].cwd.clone();
                    self.cd_or_reveal(util::resolve_against(&base, &text));
                }
            }
            InputKind::Shell { block } => {
                if !text.is_empty() {
                    self.run_shell(&text, block, false);
                }
            }
            InputKind::Search { via } => self.start_search(&text, via),
            InputKind::ConflictRename { .. } => {
                if let Some(reply) = self.pending_conflict.take() {
                    let r = if text.is_empty() { Resolution::Skip } else { Resolution::Rename(text) };
                    let _ = reply.send(r);
                }
            }
        }
    }

    /// Dismiss the input line without acting on it. A job waiting on a rename
    /// must still be told something, or its worker stays parked forever.
    pub fn cancel_input(&mut self) {
        self.pending_completion = None;
        self.input_recall = None;
        if let Some(reply) = self.pending_conflict.take() {
            let _ = reply.send(Resolution::Skip);
        }
        self.overlay = Overlay::None;
    }

    /// The line above the `r` and `E` prompts while the name typed is already
    /// taken, saying what `<Enter>` will do about it. Until v0.84.0 that was
    /// first learnt from the box after `<Enter>` (#264, #265). Read from the
    /// listing in memory, as the bulk preview is, so typing never waits on the
    /// disk; a name in another folder says nothing until `<Enter>`.
    pub fn name_hint(&self) -> Option<String> {
        let Overlay::Input(ov) = &self.overlay else { return None };
        let tab = &self.tabs[self.active];
        let taken =
            |to: &Path| to.parent().is_some_and(|d| same_place(d, &tab.cwd)) && tab.current.entries.iter().any(|e| same_place(&e.path, to));
        match &ov.kind {
            // As `do_rename` reads it: untrimmed, and the file's own name is
            // no change at all.
            InputKind::Rename { from } => {
                if ov.text.is_empty() {
                    return None;
                }
                let to = util::resolve_against(&tab.cwd, &ov.text);
                (to != *from && taken(&to)).then(|| format!("{} already exists — Enter is refused", util::file_name(&to)))
            }
            InputKind::Compress => {
                let name = ov.text.trim();
                if name.is_empty() {
                    return None;
                }
                let dest = util::resolve_against(&tab.cwd, name);
                let file = util::file_name(&dest);
                if tab.targets().iter().any(|p| same_place(p, &dest)) {
                    Some(format!("{file} is being packed — Enter is refused"))
                } else {
                    taken(&dest).then(|| format!("{file} already exists — Enter asks before replacing it"))
                }
            }
            _ => None,
        }
    }

    /// What the preview panel under the prompt shows, and what the apply step
    /// works from. Both read the directory out of the listing already in
    /// memory, so typing a rule never touches the disk.
    pub fn bulk_preview(&self, paths: &[PathBuf], text: &str) -> Result<Vec<rename::Row>, String> {
        let taken: BTreeSet<String> = self.tabs[self.active].current.entries.iter().map(|e| e.name.clone()).collect();
        rename::plan(paths, text.trim(), &taken)
    }

    /// Scale the image preview. Stepping in or out from *fit* starts from the
    /// scale on screen, so the first press does not jump.
    pub(super) fn zoom_preview(&mut self, to: ZoomTo) {
        let PreviewState::Ready(Payload::Image { own, vector, .. }) = self.preview.state else {
            return;
        };
        let from = self.preview.zoom.unwrap_or(self.preview.fit);
        self.preview.zoom = match to {
            ZoomTo::Fit => None,
            // An SVG at its own size in logical pixels, a raster one pixel to
            // one of the screen's (Q65).
            ZoomTo::Actual => Some(crate::preview::actual_zoom(own, vector, self.ctx.pixels_per_point())),
            ZoomTo::In => Some((from * 1.25).clamp(ZOOM_MIN, ZOOM_MAX)),
            ZoomTo::Out => Some((from / 1.25).clamp(ZOOM_MIN, ZOOM_MAX)),
        };
        if self.preview.zoom.is_none() {
            self.preview.pan = egui::Vec2::ZERO;
        }
    }

    /// Read the config files again, so a theme, an icon set or a key can be
    /// changed without closing the window.
    ///
    /// What the person has changed by hand since the window opened is left
    /// alone: the sort a `,` key chose, and whether Markdown is rendered. Those
    /// have keys of their own, and having a reload undo them would be a
    /// surprise. The fonts are the one thing this cannot do itself — installing
    /// a face belongs to the frame loop — so it asks for it with a flag.
    pub(super) fn reload_config(&mut self) {
        let cfg = Config::reload(&mut self.cfg);
        self.take_config(cfg);
    }

    /// The half of `<C-F5>` after the files are read: what a test drives with
    /// a config read from a directory of its own.
    pub(crate) fn take_config(&mut self, cfg: Config) {
        self.take_config_saying(cfg, None);
    }

    /// [`App::take_config`], with `said` in the toast in place of "Reloaded";
    /// an empty `said`, no toast but a warning.
    pub(super) fn take_config_saying(&mut self, mut cfg: Config, said: Option<String>) {
        // The files were read over the default theme; the one common.toml
        // chose stays under them.
        if cfg.theme.common != self.cfg.theme.common {
            cfg.theme = Arc::new(cfg.theme.with_common(&self.cfg.theme.common));
        }
        let old_term = self.cfg.term.clone();
        let files = cfg.loaded.len();
        let warning = config_toast(&cfg.warnings);
        // A pane already running keeps the shell it started with; the new
        // `[term]` is for the next one. Two real-machine runs edited the shell,
        // reloaded, and read the old one back because nothing said so (#173).
        let shell_waits = self.term.is_some() && cfg.term != old_term;
        self.cfg = cfg;
        self.refont = true;
        // A theme change can turn every row a different color, and the preview
        // holds a highlighted copy of the old one.
        self.look_changed();
        let done = said.unwrap_or_else(|| format!("Reloaded {files} config file(s)"));
        match warning {
            Some(w) => self.warn(w),
            // Said already, by what the same change was a moment ago.
            None if done.is_empty() => {}
            None if shell_waits => self.toast(format!(
                "{done} — the pane keeps its shell until {}",
                self.term_close_key().map_or("it is closed".into(), |k| format!("{k} closes it"))
            )),
            None => self.toast(done),
        }
    }

    /// How the list's keymap spells `compress`, for a message that tells
    /// someone to press it.
    pub(super) fn compress_key(&self) -> Option<String> {
        let b = self.cfg.keymap.mgr.iter().find(|b| b.run.as_slice() == [Act::Compress])?;
        Some(crate::config::keys::display_seq(&b.on))
    }

    /// How the list's keymap spells `open --interactive`, for a message that
    /// tells someone to pick an opener.
    pub(super) fn pick_opener_key(&self) -> Option<String> {
        let b = self.cfg.keymap.mgr.iter().find(|b| matches!(b.run.as_slice(), [Act::Open { interactive: true, .. }]))?;
        Some(crate::config::keys::display_seq(&b.on))
    }

    /// How the pane's keymap spells `terminal close`, for a message that tells
    /// someone to press it.
    pub(super) fn term_close_key(&self) -> Option<String> {
        let b = self.cfg.keymap.term.iter().find(|b| b.raw == "terminal close")?;
        Some(crate::config::keys::display_seq(&b.on))
    }

    /// Whether a line is waiting for a pane that has just been opened, which
    /// keeps the frames coming until it has gone.
    pub fn term_waiting(&self) -> bool {
        self.term_pending.is_some()
    }

    /// Whether a configured command draws this file.
    pub fn is_external_preview(&self, path: &Path) -> bool {
        crate::config::PreviewRule::for_path(&self.cfg.preview, path).is_some()
    }

    /// A key while the terminal has the keys. The `[term]` keymap gets first
    /// refusal — that is where the way out is bound — and everything else is
    /// the shell's.
    pub fn feed_term_key(&mut self, k: Key, bytes: Option<Vec<u8>>) {
        // Only exact single-key bindings are consulted: a chord would have to
        // hold a key back, and the shell wants it now.
        let hit = self.cfg.keymap.term.iter().find(|b| b.on.len() == 1 && b.on[0] == k).map(|b| b.run.clone());
        if let Some(acts) = hit {
            // A full-screen program -- `nvim`, `less`, `htop` -- runs on the
            // alternate screen, which has no scrollback at all. Keeping a
            // scrolling key there would spend it on a scroll that cannot move
            // anything, and the program that owns the screen would never see
            // it, so hand it over instead. Only the scrolling ones: `<C-t>` has
            // to get you out of a full-screen program as much as out of a shell.
            let scrolls = !acts.is_empty() && acts.iter().all(|a| matches!(a, Act::TermScroll(_)));
            if scrolls && self.term_alt_screen() {
                if let (Some(term), Some(bytes)) = (&self.term, bytes) {
                    term.send(bytes);
                }
                return;
            }
            for a in acts {
                match a {
                    // Leaving the pane un-maximises it. A maximised pane is
                    // only of use while you are looking at it -- handing the
                    // keys back to a list that is not on screen leaves nothing
                    // to aim them at -- so "get out of the pane" and "give the
                    // window back" are in practice the same intent, and this
                    // spends one key on both rather than two.
                    Act::Close | Act::Escape(_) => {
                        self.term_focus = false;
                        self.max_term = false;
                    }
                    other => self.act(other),
                }
            }
            return;
        }
        match (&self.term, bytes) {
            (Some(term), Some(bytes)) => term.send(bytes),
            // #336: a key the shell is given nothing for left no trace in
            // KURA_PTY_LOG, so "sent nothing" and "never arrived" looked alike.
            (Some(term), None) => term.log_note(&format!("in key {k} (nothing sent)")),
            (None, _) => {}
        }
    }

    /// True while a full-screen program is drawing in the terminal pane.
    pub fn term_alt_screen(&self) -> bool {
        self.term.as_ref().is_some_and(|t| t.with_grid(crate::terminal::alt_screen))
    }

    /// The tag in the pane's corner while a full-screen program has the keys
    /// (Q77): such a program draws over every row, so nothing on screen said
    /// how to get back to the list once the start toast had gone.
    pub fn term_leave_badge(&self) -> Option<String> {
        if !self.term_focus || !self.term_alt_screen() {
            return None;
        }
        Some(format!("{} list", self.term_leave_key()?))
    }

    /// What git says about the rows of `dir`, or nothing while the answer is
    /// still on its way — or for ever, when there is no repository here.
    pub fn git_status(&self, dir: &Path) -> Option<Arc<git::Status>> {
        self.git_status.peek(&dir.to_path_buf()).cloned()
    }

    /// True while Tab is waiting on a listing, so the prompt can say so.
    pub fn completing(&self) -> bool {
        self.pending_completion.is_some()
    }

    /// How long Tab waits before the prompt names the folder it is reading.
    pub const SLOW_COMPLETION: Duration = Duration::from_millis(1500);

    /// The folder Tab has been waiting on for longer than
    /// [`Self::SLOW_COMPLETION`], so a share that does not answer reads
    /// differently from a folder with no match (#222).
    pub fn slow_completion(&self) -> Option<String> {
        let p = self.pending_completion.as_ref()?;
        (p.since.elapsed() >= Self::SLOW_COMPLETION).then(|| p.dir.display().to_string())
    }

    /// True while `l` has an archive's members on screen.
    pub fn in_archive_view(&self) -> bool {
        self.archive_view.is_some()
    }

    /// Where inside an archive the view is: the archive's path with the
    /// level on screen after it (`…/pack.zip/docs/a`), for the state file.
    pub fn archive_at(&self) -> Option<PathBuf> {
        let view = self.archive_view.as_ref()?;
        Some(view.inner.split('/').filter(|p| !p.is_empty()).fold(view.archive.clone(), |p, part| p.join(part)))
    }

    /// True while `gu`'s view is up, walk finished or not.
    pub fn in_usage_view(&self) -> bool {
        self.usage_linemode.is_some()
    }

    /// True while the current view is a search result list rather than a real
    /// directory; `leave`/`Esc` returns to the directory it started from.
    pub fn in_search_view(&self) -> bool {
        self.tabs[self.active].current.path != self.tabs[self.active].cwd
    }
}
