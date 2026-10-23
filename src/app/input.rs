//! Prompts and typed input: the input box, completion, search, the terminal pane.

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

    pub(super) fn do_create(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        let base = self.tabs[self.active].cwd.clone();
        let as_dir = text.ends_with('/') || text.ends_with('\\');
        let target = util::resolve_against(&base, text.trim_end_matches(['/', '\\']));
        let made = paths_to_make(&target);
        let res = if as_dir {
            std::fs::create_dir_all(&target)
        } else {
            if let Some(parent) = target.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            std::fs::OpenOptions::new().write(true).create_new(true).open(&target).map(|_| ())
        };
        match res {
            Ok(()) => {
                // The cursor goes to what was made, as yazi's does (#250): to
                // `new` for `a new/deep/note.txt`, the part of it in this folder.
                // `land_on`, not the memo: the row the cursor was on when the
                // listing comes back would win over a memo.
                let here = target.strip_prefix(&base).ok().and_then(|r| r.components().next()).map(|c| base.join(c));
                self.land_on = here.or(Some(target.clone()));
                self.cache.remove(&base);
                self.rescan(&base);
                if !made.is_empty() {
                    self.undos.land(UndoStep::Create { paths: made, file: !as_dir }, Land::Fresh);
                }
            }
            Err(e) => self.error(format!("Create failed: {e}")),
        }
    }

    pub(super) fn do_rename(&mut self, from: &Path, text: &str) {
        if text.is_empty() {
            return;
        }
        let base = self.tabs[self.active].cwd.clone();
        let to = util::resolve_against(&base, text);
        if to == from {
            return;
        }
        match self.apply_rename(from, &to) {
            // Said like `d` and `-` say theirs: a rename was the one change
            // that went by in silence, though its undo is the one people use
            // most (#225).
            Ok(()) => {
                // Onto the new name, as after `a` (#250).
                self.land_on = Some(to.clone());
                let step = UndoStep::Rename { from: from.to_path_buf(), to };
                self.toast(format!("{} — u to undo", step.redone_label()));
                self.undos.land(step, Land::Fresh);
            }
            Err(e) => self.error(format!("Rename failed: {e}")),
        }
    }

    /// The two files to compare. With the view split that is what each pane is
    /// standing on, which is the whole reason the split exists; otherwise it is
    /// the two that are selected.
    pub(super) fn compare_pair(&self) -> Result<(PathBuf, PathBuf), String> {
        let hovered = |t: usize| self.tabs[t].current.hovered().map(|e| (e.path.clone(), e.kind));
        let (a, b) = match self.split {
            Some(sp) => {
                let (l, r) = match sp.right {
                    true => (sp.other, self.active),
                    false => (self.active, sp.other),
                };
                match (hovered(l), hovered(r)) {
                    (Some(a), Some(b)) => (a, b),
                    _ => return Err("both panes need a file".into()),
                }
            }
            None => {
                let sel = self.tabs[self.active].targets();
                if sel.len() != 2 {
                    return Err("split the view, or select exactly two files".into());
                }
                let of = |p: &PathBuf| {
                    let kind = self.tabs[self.active].current.entries.iter().find(|e| &e.path == p).map(|e| e.kind);
                    (p.clone(), kind.unwrap_or(Kind::File))
                };
                (of(&sel[0]), of(&sel[1]))
            }
        };
        // Two folders are compared as trees; two files line by line. One of each
        // is neither, and there is nothing sensible to show for it.
        if a.1.is_dir_like() != b.1.is_dir_like() {
            return Err("compare two files, or two folders — not one of each".into());
        }
        if a.0 == b.0 {
            return Err("that is the same file on both sides".into());
        }
        Ok((a.0, b.0))
    }

    pub(super) fn start_compare(&mut self) {
        let (left, right) = match self.compare_pair() {
            Ok(pair) => pair,
            Err(why) => return self.error(format!("Compare: {why}")),
        };
        self.differ.request(diff::Request { left: left.clone(), right: right.clone(), max_bytes: self.cfg.ui.max_text_bytes });
        self.overlay =
            Overlay::Diff(DiffOverlay { left, right, outcome: None, offset: 0, rows: 1, cursor: 0, hide_same: false, back: None });
    }

    /// The compare view's own commands: scroll, jump between differences, close.
    pub(super) fn diff_act(&mut self, a: Act) {
        let page = self.tabs[self.active].page_rows.max(1);
        let Overlay::Diff(ov) = &mut self.overlay else { return };
        // A tree comparison is a list with a cursor, so the same keys mean
        // something different there: `j` moves the selection rather than the
        // scroll, and `n`/`N` walk to the next path that is not a match.
        if matches!(&ov.outcome, Some(diff::Outcome::Tree { .. })) {
            let shown = ov.shown();
            let len = shown.len();
            let Some(diff::Outcome::Tree { rows, .. }) = &ov.outcome else { return };
            match a {
                Act::Close | Act::Escape(_) | Act::Quit | Act::Compare => self.overlay = Overlay::None,
                // The pair under the cursor, compared as files. Only a file on
                // both sides has a pair; anything else says why there is not.
                Act::Enter | Act::Open { .. } => {
                    let Some(row) = shown.get(ov.cursor).map(|&i| &rows[i]) else { return };
                    let why = match row.state {
                        diff::TreeState::LeftOnly | diff::TreeState::RightOnly => Some("it is on one side only"),
                        _ if row.dir => Some("that is a folder"),
                        _ => None,
                    };
                    if let Some(why) = why {
                        return self.error(format!("Compare: {why}"));
                    }
                    let (left, right) = (ov.left.join(&row.rel), ov.right.join(&row.rel));
                    let Overlay::Diff(tree) = std::mem::replace(&mut self.overlay, Overlay::None) else { return };
                    self.differ.request(diff::Request { left: left.clone(), right: right.clone(), max_bytes: self.cfg.ui.max_text_bytes });
                    self.overlay = Overlay::Diff(DiffOverlay {
                        left,
                        right,
                        outcome: None,
                        offset: 0,
                        rows: 1,
                        cursor: 0,
                        hide_same: false,
                        back: Some(Box::new(tree)),
                    });
                }
                // Hidden or shown, the cursor stays on the row it was on; when
                // that row is a match being hidden, on the next one that is not.
                Act::HideSame => {
                    let at = shown.get(ov.cursor).copied().unwrap_or(0);
                    ov.hide_same = !ov.hide_same;
                    let now = ov.shown();
                    ov.cursor = now.iter().position(|&i| i >= at).unwrap_or(now.len().saturating_sub(1));
                    let what = if ov.hide_same { "Hiding matching rows" } else { "Showing matching rows" };
                    self.toast_instead(&["Hiding matching rows", "Showing matching rows"], what.into());
                }
                Act::Copy(_) => {
                    let text = ov.shown_text();
                    let n = len;
                    match exec::set_clipboard(&text) {
                        Ok(()) if n == 1 => self.toast("Copied 1 row".to_string()),
                        Ok(()) => self.toast(format!("Copied {n} rows")),
                        Err(e) => self.error(format!("Clipboard: {e}")),
                    }
                }
                Act::Arrow(step) if len > 0 => ov.cursor = step.apply(ov.cursor, len, page),
                Act::FindArrow { prev } if len > 0 => {
                    let differs = |&i: &usize| !matches!(rows[i].state, diff::TreeState::Same);
                    let found = if prev {
                        shown[..ov.cursor].iter().rposition(differs)
                    } else {
                        shown[ov.cursor + 1..].iter().position(differs).map(|i| i + ov.cursor + 1)
                    };
                    match found {
                        Some(at) => ov.cursor = at,
                        None => {
                            let word = if prev { "first" } else { "last" };
                            self.toast_instead(&["At the "], format!("At the {word} difference"));
                        }
                    }
                }
                _ => {}
            }
            return;
        }
        let rows: &[diff::Row] = match &ov.outcome {
            Some(diff::Outcome::Rows { rows, .. }) => rows,
            _ => &[],
        };
        match a {
            // Back to the folder comparison it was opened from, if any.
            Act::Close | Act::Escape(_) => {
                self.overlay = match ov.back.take() {
                    Some(tree) => Overlay::Diff(*tree),
                    None => Overlay::None,
                }
            }
            Act::Quit | Act::Compare => self.overlay = Overlay::None,
            Act::Copy(_) => self.error("Copy: only a folder comparison has rows to copy".to_string()),
            // Against the last *scroll position*, not the last row. Clamping
            // to `len - 1` left `G` a screenful past where the pane can
            // actually sit, so the next `j` or `k` moved a number nothing was
            // drawing from and the keys looked dead -- for exactly as many
            // presses as the pane is tall, which is why a half-page `<C-u>`
            // appeared to wake them up.
            Act::Arrow(step) if !rows.is_empty() => {
                let stop = rows.len().saturating_sub(ov.rows.max(1)) + 1;
                ov.offset = step.apply(ov.offset, stop, page);
            }
            Act::FindArrow { prev } if !rows.is_empty() => match diff::next_change(rows, ov.offset, prev) {
                Some(at) => ov.offset = at,
                None => {
                    let word = if prev { "first" } else { "last" };
                    self.toast_instead(&["At the "], format!("At the {word} difference"));
                }
            },
            _ => {}
        }
    }

    /// Ask for a rule to rename everything selected by. The prompt starts on
    /// the rule that changes nothing, so the preview below it opens showing the
    /// names as they are and the person edits from there.
    pub(super) fn start_bulk_rename(&mut self) {
        let paths = self.tabs[self.active].targets();
        if paths.is_empty() {
            return;
        }
        self.overlay = Overlay::Input(InputOverlay {
            kind: InputKind::Bulk { paths },
            title: "Bulk rename".into(),
            text: "{name}{ext}".into(),
            initial_selection: Some((0, "{name}{ext}".chars().count())),
            focused: false,
            completion: Vec::new(),
            completion_at: 0,
        });
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

    pub(super) fn do_bulk_rename(&mut self, paths: &[PathBuf], text: &str) {
        let rows = match self.bulk_preview(paths, text) {
            Ok(rows) => rows,
            Err(e) => return self.error(format!("Bulk rename: {e}")),
        };
        // The preview already said which rows are wrong; renaming the rest and
        // leaving the batch half applied would be worse than doing nothing.
        if let Some(bad) = rows.iter().find(|r| r.problem.is_some()) {
            let why = bad.problem.as_deref().unwrap_or_default();
            return self.error(format!("{}: {why}", util::file_name(&bad.from)));
        }
        let pairs: Vec<(PathBuf, PathBuf)> =
            rows.iter().filter(|r| r.to != util::file_name(&r.from)).map(|r| (r.from.clone(), r.from.with_file_name(&r.to))).collect();
        if pairs.is_empty() {
            return self.toast("Bulk rename: nothing to change");
        }
        let done = pairs.len();
        match self.run_renames(&pairs) {
            Ok(()) => {
                self.tabs[self.active].clear_selection();
                self.undos.land(UndoStep::Bulk { pairs }, Land::Fresh);
                self.toast(format!("Renamed {done} file(s)"));
            }
            Err(e) => self.error(format!("Bulk rename: {e}")),
        }
    }

    /// Carry out a batch of renames, in an order that works even when two files
    /// swap names. Stops at the first failure and says so: what has already
    /// moved keeps its new name, which is at least a state the listing shows
    /// honestly.
    pub(super) fn run_renames(&mut self, pairs: &[(PathBuf, PathBuf)]) -> Result<(), String> {
        let names: Vec<(String, String)> = pairs.iter().map(|(a, b)| (util::file_name(a), util::file_name(b))).collect();
        // Where each file is right now; parking moves one aside for a moment.
        let mut at: Vec<PathBuf> = pairs.iter().map(|(a, _)| a.clone()).collect();

        for step in rename::order(&names) {
            let (i, to) = match step {
                rename::Step::Park(i) => {
                    let dir = pairs[i].0.parent().unwrap_or(Path::new(""));
                    (i, rename::park_name(dir, i))
                }
                rename::Step::Rename(i) => (i, pairs[i].1.clone()),
            };
            self.apply_rename(&at[i].clone(), &to)?;
            at[i] = to;
        }
        Ok(())
    }

    /// Move `from` onto `to` and leave the cursor there. Undo and redo walk the
    /// same rename backwards and forwards, so this takes two paths rather than
    /// the name typed at the prompt.
    pub(super) fn apply_rename(&mut self, from: &Path, to: &Path) -> Result<(), String> {
        if ops::exists(to) {
            return Err(format!("Already exists: {}", util::file_name(to)));
        }
        if let Some(parent) = to.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        std::fs::rename(from, to).map_err(|e| e.to_string())?;
        // `r` takes a path, so a rename can cross directories and leave a
        // listing stale at either end.
        for dir in [from.parent(), to.parent()].into_iter().flatten() {
            let dir = dir.to_path_buf();
            self.cache.remove(&dir);
            self.rescan(&dir);
        }
        if let Some(dir) = to.parent() {
            let name = util::file_name(to);
            self.tabs[self.active].memo.insert(dir.to_path_buf(), name);
        }
        Ok(())
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

    /// `-` on a folder without Developer Mode: Windows refused the symlink, and
    /// a junction would do without the privilege. Asked rather than done,
    /// because a junction is not the same thing -- it holds the full path where
    /// `-` would have written a relative one, and cannot reach a network
    /// location -- and swapping one kind for the other in silence would leave
    /// that to be found out later (Q46).
    pub(super) fn offer_junctions(&mut self, links: Vec<ops::Link>) {
        const SHOWN: usize = 5;
        let mut body = vec![
            "Windows would not make the symlink: that needs Developer Mode or administrator.".to_owned(),
            "A junction needs neither. Unlike the symlink it holds the full path, not a relative one,".to_owned(),
            "and it cannot point at a network location.".to_owned(),
            String::new(),
        ];
        body.extend(links.iter().take(SHOWN).map(|l| format!("{}  →  {}", l.at.display(), l.target.display())));
        if links.len() > SHOWN {
            body.push(format!("… and {} more", links.len() - SHOWN));
        }
        // What `c` copies, on the screen it is asked about: the toast that
        // named it was gone by then (#197).
        body.push(String::new());
        body.push(if links.len() == 1 { "`c` copies:".to_owned() } else { "`c` copies, one per link:".to_owned() });
        body.extend(links.iter().take(SHOWN).map(|l| format!("  {}", ops::mklink_line(&l.at, &l.target))));
        let what = if links.len() == 1 { "Make the junction".to_owned() } else { format!("Make {} junctions", links.len()) };
        // `c` for someone who would rather make it themselves, or somewhere
        // else: the line is two absolute paths long, and a toast cannot be
        // copied from (#185, #191; Q56).
        let copy = if links.len() == 1 { "Copy the mklink command" } else { "Copy the mklink commands" };
        self.overlay = Overlay::Confirm(ConfirmOverlay {
            title: "Make a junction instead?".into(),
            body,
            options: vec![('y', what), ('c', copy.into()), ('n', "No".into())],
            action: ConfirmAction::Junctions { links },
            dest: None,
        });
    }

    /// The `y` to [`App::offer_junctions`]: made like any link, so `u` takes
    /// them back and `U` makes them again, as junctions.
    pub(super) fn make_junctions(&mut self, links: Vec<ops::Link>) {
        let (done, left, err) = each_link(links, ops::Link::make);
        if let Some(l) = done.first().or(left.first()) {
            self.refresh_parent(&l.at.clone());
        }
        if !done.is_empty() {
            let said = match done.len() {
                1 => format!("Made a junction {} — u to undo", util::file_name(&done[0].at)),
                n => format!("Made {n} junctions — u to undo"),
            };
            self.undos.land(UndoStep::Link { links: done }, Land::Fresh);
            self.toast(said);
        }
        if let (Some(e), Some(l)) = (err, left.first()) {
            self.error(format!("Junction: {}: {e}", util::file_name(&l.at)));
        }
    }

    /// The junction question's `c`: the `mklink /J` lines, one per link, and
    /// nothing made. Each runs in any shell (see `ops::mklink_line`).
    pub(super) fn copy_mklink(&mut self, links: &[ops::Link]) {
        let lines: Vec<String> = links.iter().map(|l| ops::mklink_line(&l.at, &l.target)).collect();
        match exec::set_clipboard(&lines.join("\n")) {
            Ok(()) => self.toast(match lines.len() {
                1 => "Copied the mklink command — it runs in cmd or PowerShell".to_owned(),
                n => format!("Copied {n} mklink commands — they run in cmd or PowerShell"),
            }),
            Err(e) => self.error(format!("Clipboard: {e}")),
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

    /// Take back the newest step. A step that will not go back stays on the
    /// stack — the usual reason is something standing where it came from, which
    /// the person can clear before pressing `u` again.
    pub(super) fn undo_step(&mut self) {
        let Some(step) = self.undos.undo.pop() else {
            self.toast("Nothing to undo");
            return;
        };
        match step {
            UndoStep::Rename { from, to } => match self.apply_rename(&to, &from) {
                Ok(()) => {
                    let step = UndoStep::Rename { from, to };
                    self.toast(step.undone_label());
                    self.undos.land(step, Land::Undone);
                }
                Err(e) => {
                    self.error(format!("Undo: {e}"));
                    self.undos.keep(UndoStep::Rename { from, to }, Land::Undone);
                }
            },
            UndoStep::Move { pairs } => {
                let back: Vec<(PathBuf, PathBuf)> = pairs.iter().rev().map(|(a, b)| (b.clone(), a.clone())).collect();
                match self.run_renames(&back) {
                    Ok(()) => {
                        let step = UndoStep::Move { pairs };
                        self.toast(step.undone_label());
                        self.undos.land(step, Land::Undone);
                    }
                    Err(e) => {
                        self.error(format!("Undo: {e}"));
                        self.undos.keep(UndoStep::Move { pairs }, Land::Undone);
                    }
                }
            }
            UndoStep::Bulk { pairs } => {
                let back: Vec<(PathBuf, PathBuf)> = pairs.iter().rev().map(|(a, b)| (b.clone(), a.clone())).collect();
                match self.run_renames(&back) {
                    Ok(()) => {
                        let step = UndoStep::Bulk { pairs };
                        self.toast(step.undone_label());
                        self.undos.land(step, Land::Undone);
                    }
                    Err(e) => {
                        self.error(format!("Undo: {e}"));
                        self.undos.keep(UndoStep::Bulk { pairs }, Land::Undone);
                    }
                }
            }
            UndoStep::Trash { paths, dir } => {
                if !restore::SUPPORTED {
                    self.error(format!("Undo: {}", restore::UNSUPPORTED));
                    self.undos.keep(UndoStep::Trash { paths, dir }, Land::Undone);
                    return;
                }
                let id = self.submit_op(OpKind::Restore, paths.clone(), dir.clone(), true);
                self.record_job(id, UndoStep::Trash { paths, dir }, Land::Undone);
            }
            UndoStep::Create { paths, file } => match unmake(&paths, file) {
                Ok(folders) => {
                    self.refresh_parent(&paths[0]);
                    let step = UndoStep::Create { paths, file };
                    self.toast(removed_label(&step, folders));
                    self.undos.land(step, Land::Undone);
                }
                Err(e) => {
                    self.error(format!("Undo: {e}"));
                    self.undos.keep(UndoStep::Create { paths, file }, Land::Undone);
                }
            },
            UndoStep::Link { links } => self.relink(links, ops::Link::remove, Land::Undone),
        }
    }

    /// Remove or make again a set of links. Each stands alone, so the ones
    /// that went through move to the other stack and the rest stay, named in
    /// the error -- `u` again once the way is clear finishes the job.
    pub(super) fn relink(&mut self, links: Vec<ops::Link>, f: fn(&ops::Link) -> std::io::Result<()>, how: Land) {
        let (done, left, err) = each_link(links, f);
        if let Some(l) = done.first().or(left.first()) {
            self.refresh_parent(&l.at.clone());
        }
        let verb = if how == Land::Undone { "Undo" } else { "Redo" };
        if !done.is_empty() {
            let step = UndoStep::Link { links: done };
            self.toast(match how {
                Land::Undone => step.undone_label(),
                _ => step.redone_label(),
            });
            self.undos.land(step, how);
        }
        if let Some(e) = err {
            self.error(format!("{verb}: {e}"));
            self.undos.keep(UndoStep::Link { links: left }, how);
        }
    }

    /// List again the folder `p` is in, which `u` has just changed.
    pub(super) fn refresh_parent(&mut self, p: &Path) {
        if let Some(dir) = p.parent() {
            let dir = dir.to_path_buf();
            self.cache.remove(&dir);
            self.rescan(&dir);
        }
    }

    /// Do again what `u` took back.
    pub(super) fn redo_step(&mut self) {
        let Some(step) = self.undos.redo.pop() else {
            self.toast("Nothing to redo");
            return;
        };
        match step {
            UndoStep::Rename { from, to } => match self.apply_rename(&from, &to) {
                Ok(()) => {
                    let step = UndoStep::Rename { from, to };
                    self.toast(step.redone_label());
                    self.undos.land(step, Land::Redone);
                }
                Err(e) => {
                    self.error(format!("Redo: {e}"));
                    self.undos.keep(UndoStep::Rename { from, to }, Land::Redone);
                }
            },
            UndoStep::Move { pairs } => match self.run_renames(&pairs) {
                Ok(()) => {
                    let step = UndoStep::Move { pairs };
                    self.toast(step.redone_label());
                    self.undos.land(step, Land::Redone);
                }
                Err(e) => {
                    self.error(format!("Redo: {e}"));
                    self.undos.keep(UndoStep::Move { pairs }, Land::Redone);
                }
            },
            UndoStep::Bulk { pairs } => match self.run_renames(&pairs) {
                Ok(()) => {
                    let step = UndoStep::Bulk { pairs };
                    self.toast(step.redone_label());
                    self.undos.land(step, Land::Redone);
                }
                Err(e) => {
                    self.error(format!("Redo: {e}"));
                    self.undos.keep(UndoStep::Bulk { pairs }, Land::Redone);
                }
            },
            UndoStep::Trash { paths, dir } => {
                let id = self.submit_op(OpKind::Trash, paths.clone(), dir.clone(), true);
                self.record_job(id, UndoStep::Trash { paths, dir }, Land::Redone);
            }
            UndoStep::Create { paths, file } => match remake(&paths, file) {
                Ok(()) => {
                    self.refresh_parent(&paths[0]);
                    let step = UndoStep::Create { paths, file };
                    self.toast(step.redone_label());
                    self.undos.land(step, Land::Redone);
                }
                Err(e) => {
                    self.error(format!("Redo: {e}"));
                    self.undos.keep(UndoStep::Create { paths, file }, Land::Redone);
                }
            },
            UndoStep::Link { links } => self.relink(links, ops::Link::make, Land::Redone),
        }
    }

    pub(super) fn complete_input(&mut self) {
        let Overlay::Input(ov) = &mut self.overlay else { return };
        // `E`'s field is a new name, so there is nothing to complete: `<Tab>`
        // turns the extension into the next format, the stem untouched (Q79).
        if matches!(ov.kind, InputKind::Compress) {
            ov.text = next_archive_name(&ov.text);
            let stem = util::stem_and_ext(&ov.text).0.chars().count();
            let stem = if ov.text.ends_with(".tar.gz") { stem - 4 } else { stem };
            ov.initial_selection = Some((0, stem));
            ov.focused = false;
            return;
        }
        if !matches!(ov.kind, InputKind::Cd) {
            return;
        }
        if let Some(dir) = cycling_in(&ov.text, &ov.completion, ov.completion_at) {
            let hits = ov.completion.clone();
            self.apply_completion(&dir, hits);
            return;
        }
        let (dir, prefix) = completion_target(&ov.text, &self.tabs[self.active].cwd);
        // A directory that has been listed once answers on the spot, which
        // covers the cwd, its parent and everywhere the tab has been.
        if let Some(entries) = self.cache.get(&dir).cloned() {
            self.pending_completion = None;
            self.apply_completion(&dir, completion_hits(&entries, &prefix));
            return;
        }
        // Everything else goes to the scan pool. `read_dir` on a dead share
        // sits for half a minute, and Tab must not take the window with it.
        let sort = self.tabs[self.active].sort;
        let id = self.scanner.scan_low(dir.clone(), sort);
        self.inflight.insert(id, dir.clone());
        self.pending_completion = Some(PendingCompletion { id, dir, prefix, since: Instant::now() });
    }

    /// Answer a completion whose listing has just arrived. What the user typed
    /// in the meantime wins: an answer to an older question is dropped.
    pub(super) fn complete_from(&mut self, id: u64, path: &Path, entries: &[Entry]) {
        let Some(p) = self.pending_completion.take() else { return };
        if p.id != id || p.dir != path {
            self.pending_completion = Some(p);
            return;
        }
        let Overlay::Input(ov) = &self.overlay else { return };
        if !matches!(ov.kind, InputKind::Cd) {
            return;
        }
        if completion_target(&ov.text, &self.tabs[self.active].cwd) != (p.dir.clone(), p.prefix.clone()) {
            return;
        }
        self.apply_completion(&p.dir, completion_hits(entries, &p.prefix));
    }

    /// Put the next match on the input line. Repeated presses walk the list,
    /// and `completion_at` remembers how far they got.
    pub(super) fn apply_completion(&mut self, dir: &Path, hits: Vec<String>) {
        let Overlay::Input(ov) = &mut self.overlay else { return };
        if hits.is_empty() {
            return;
        }
        let idx = ov.completion_at % hits.len();
        ov.text = completed_text(dir, &hits[idx]);
        ov.completion_at = idx + 1;
        ov.completion = hits;
        ov.initial_selection = {
            let n = ov.text.chars().count();
            Some((n, n))
        };
        ov.focused = false;
    }

    /// Open the terminal pane, close it, or move the keys in and out of it.
    ///
    /// `None` is the toggle a key presses: open it and take the keys, or give
    /// them back when it already has them.
    pub(super) fn terminal(&mut self, what: Tri) {
        if what == Some(false) {
            // Ending the shell ends whatever runs under it, and on screen that
            // looks just like `<C-t>` hiding the pane -- so a running program
            // is asked about first, and a prompt is not (Q21).
            if let Some(t) = self.term.as_ref().filter(|t| t.busy()) {
                // An empty title (ConPTY before the program sets one) falls back to the shell's own name.
                let what = match (t.title.is_empty(), self.term_shell.is_empty()) {
                    (false, _) => format!("`{}`", program_label(&t.title)),
                    (true, false) => format!("`{}`", self.term_shell),
                    (true, true) => "A program".to_owned(),
                };
                self.overlay = Overlay::Confirm(ConfirmOverlay {
                    title: "End the shell?".into(),
                    body: vec![format!("{what} is still running in the terminal, and ends with it.")],
                    options: vec![('y', "End it".into()), ('n', "Keep it".into())],
                    action: ConfirmAction::EndShell,
                    dest: None,
                });
                return;
            }
            let none = self.term.is_none();
            self.end_shell();
            if none {
                self.toast("No terminal to close");
            }
            return;
        }
        if self.term.is_some() {
            self.term_focus = what.unwrap_or(!self.term_focus);
            return;
        }
        let cwd = self.tabs[self.active].cwd.clone();
        let ctx = self.ctx.clone();
        // The real shape arrives with the first frame that draws it; this is
        // only what the shell starts life believing.
        let size = crate::terminal::Size::new(80, 24);
        // Empty means the default: `pwsh` on Windows when it is installed,
        // otherwise whatever the platform starts (Q29). A name here is the way
        // to ask for a particular one.
        let shell = match self.cfg.term.shell.is_empty() {
            false => Some((self.cfg.term.shell.clone(), self.cfg.term.args.clone())),
            // The arguments apply to the automatic pick too (#442); with no
            // program named there is nothing for them to go to.
            true => crate::terminal::default_shell().map(|s| (s, self.cfg.term.args.clone())),
        };
        let label = crate::terminal::shell_label(shell.as_ref().map(|(p, _)| p.as_str()));
        let program = shell.as_ref().map(|(p, _)| p.clone());
        let args = shell.as_ref().map(|(_, a)| a.clone()).unwrap_or_default();
        let log = std::env::var_os("KURA_PTY_LOG").map(PathBuf::from);
        let env: Vec<(String, String)> = self.cfg.term.env.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        match crate::terminal::Terminal::spawn_with_env(&cwd, size, (8, 16), shell, log.as_deref(), env, move || ctx.request_repaint()) {
            Ok(t) => {
                self.term = Some(t);
                self.term_focus = true;
                // The arguments the shell started with, from `KURA_TERM_ARGS`
                // or `[term] args`: a run could not see they had arrived (#267, #281).
                let shown = match args.is_empty() {
                    false => format!("{label} {}", args.join(" ")),
                    true => label.clone(),
                };
                self.toast(format!("Started {shown} — <C-t> back to the list"));
                crate::runinfo::remember_shell(&label);
                self.term_shell = label;
            }
            // Name the shell: `Terminal failed: … (os error 2)` did not say
            // which program was missing (#173). Looked up only now, after the
            // start failed, so a shell found some way other than `PATH` is
            // never refused on a guess.
            Err(e) => {
                // `reason` is what `kura env` repeats after its own
                // "did not start:"; the OS's own sentence is left out when the
                // missing program is named (it is in the OS language).
                let (reason, said) = match program.filter(|p| util::locate(p).is_none()) {
                    Some(p) => {
                        // A path was given, so PATH was never searched.
                        let r = match p.contains(['/', '\\']) {
                            true => format!("`{p}` does not exist"),
                            false => format!("`{p}` was not found on PATH"),
                        };
                        let s = format!("Terminal failed: {r} — set [term] shell to one that is");
                        (r, s)
                    }
                    None => (format!("{label}: {e}"), format!("Terminal failed ({label}): {e}")),
                };
                crate::runinfo::remember_shell_failed(&reason);
                self.error(said);
            }
        }
    }

    /// Drop the terminal, which sends the shell its shutdown, and say so: the
    /// pane going away looks the same as `<C-t>` hiding it otherwise.
    pub(super) fn end_shell(&mut self) {
        self.term_pending = None;
        let had = self.term.take().is_some();
        self.term_focus = false;
        self.max_term = false;
        if had {
            // Replace the `Started …` still up: the newer news hid behind it (#280).
            let started = format!("Started {}", self.term_shell);
            self.toast_instead(&[started.as_str()], "Ended the shell".into());
        }
    }

    /// Type the selection into the shell, quoted so a path with a space in it
    /// arrives as one word. Nothing is run: the line is left for the user to
    /// put a command in front of.
    ///
    /// With the pane closed, it is opened first (Q35). A shell that is still
    /// reading its profile can drop what is typed at it, so the line waits
    /// until the shell has marked its prompt (OSC 133), or written something
    /// and then gone quiet for 800 ms (Q39, #250), or five seconds, whichever
    /// comes first.
    pub(super) fn term_send_paths(&mut self) {
        let paths = self.tabs[self.active].targets();
        if paths.is_empty() {
            return;
        }
        let opened = self.term.is_none();
        if opened {
            self.terminal(Some(true));
        }
        let Some(term) = &self.term else { return };
        let how = term.quoting();
        let line: Vec<String> = paths.iter().map(|p| crate::terminal::quote(&p.to_string_lossy(), how)).collect();
        let bytes = format!(" {}", line.join(" ")).into_bytes();
        match !opened && term.has_drawn() {
            true => term.send(bytes),
            false => self.term_pending = Some((bytes, Instant::now())),
        }
        self.term_focus = true;
    }

    /// Whether a line is waiting for a pane that has just been opened, which
    /// keeps the frames coming until it has gone.
    pub fn term_waiting(&self) -> bool {
        self.term_pending.is_some()
    }

    /// Look for `needle` in the terminal's scrollback and put the match on
    /// screen. Repeating the command walks the matches; running out wraps.
    pub(super) fn term_find(&mut self, needle: &str, back: bool) {
        let needle = needle.trim();
        if needle.is_empty() {
            return;
        }
        self.term_needle = needle.to_owned();
        let Some(term) = &mut self.term else { return };
        match term.search(needle, back) {
            Some(false) => return,
            Some(true) => return self.toast("Wrapped"),
            None => {}
        }
        // Nothing from here on; start again from the view.
        term.end_search();
        match term.search(needle, back) {
            Some(_) => self.toast("Wrapped"),
            None => self.error(format!("No match for {needle}")),
        }
    }

    /// Whether a configured command draws this file.
    pub fn is_external_preview(&self, path: &Path) -> bool {
        crate::config::PreviewRule::for_path(&self.cfg.preview, path).is_some()
    }

    /// The file under the cursor, or nothing-shaped when there is none.
    pub(super) fn hovered_path(&self) -> PathBuf {
        self.tabs[self.active].current.hovered().map_or_else(PathBuf::new, |e| e.path.clone())
    }

    /// Show picture `n` of the hovered file.
    ///
    /// The old picture is left up while the new one is drawn. A command takes
    /// long enough to see, and blinking to "loading" and back on every press
    /// of `<A-j>` is worse than a moment of the previous page.
    pub(super) fn go_to_picture(&mut self, n: i64) {
        if self.preview.n == n {
            return;
        }
        self.preview.n = n;
        self.preview.pending_since = None;
        self.request_preview(true);
    }

    /// Follow the shell: put the pane where it says it is.
    ///
    /// The other direction, and the useful one when a command has moved the
    /// shell somewhere the pane knows nothing about. It needs the shell to
    /// report its directory (OSC 7), which most do out of the box and some
    /// have to be told to.
    pub(super) fn term_pull_cwd(&mut self) {
        let Some(term) = &self.term else {
            self.error("The terminal is not open");
            return;
        };
        let Some(cwd) = term.shell_cwd.clone() else {
            // Naming the obstacle alone leaves nowhere to go: "OSC 7" is
            // hard to search for, and most of what comes back overrides
            // `prompt`, which breaks Starship and the other generators people
            // actually run. The hook that does not is named here, and the
            // command that prints it (Q50): a toast does not wrap, and the
            // hook itself is wider than any window.
            let said = no_osc7(&self.term_shell, &kura_command());
            self.error(said);
            return;
        };
        if cwd == self.tabs[self.active].cwd {
            return;
        }
        self.cd(cwd, true);
    }

    /// Read what the shell has said, and keep it in the directory the pane is
    /// showing. Called once a frame, and cheap when there is nothing to do.
    pub(super) fn pump_terminal(&mut self) {
        let cwd = self.tabs[self.active].cwd.clone();
        let Some(term) = &mut self.term else { return };
        let (fg, bg) = (self.cfg.theme.fg, self.cfg.theme.bg_alt);
        term.set_colors([fg.r(), fg.g(), fg.b()], [bg.r(), bg.g(), bg.b()]);
        for text in term.drain() {
            // A program asked for the clipboard; only this thread can oblige.
            let _ = exec::set_clipboard(&text);
        }
        if term.exited {
            self.term = None;
            self.term_pending = None;
            self.term_focus = false;
            self.max_term = false;
            self.toast("The shell exited");
            return;
        }
        // A shell that marks its prompt (OSC 133) is ready when it says so.
        // Otherwise, quiet for 800 ms after its first output: 300 was enough
        // on ARM64 (#249), but the x64 machine's pwsh printed its banner and
        // then said nothing for over 300 ms while its profile loaded, so the
        // path went in before the prompt, 3 runs of 3 (#250).
        let ready =
            |at: &Instant| term.prompt_seen() || term.quiet_for(Duration::from_millis(800)) || at.elapsed() > Duration::from_secs(5);
        if self.term_pending.as_ref().is_some_and(|(_, at)| ready(at)) {
            if let Some((bytes, _)) = self.term_pending.take() {
                term.send(bytes);
            }
        }
        term.follow(&cwd);
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

    /// How the pane's keymap spells handing the keys back to the list.
    pub(super) fn term_leave_key(&self) -> Option<String> {
        let b = self.cfg.keymap.term.iter().find(|b| matches!(b.run.as_slice(), [Act::Close | Act::Escape(_)]))?;
        Some(crate::config::keys::display_seq(&b.on))
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

    pub(super) fn start_search(&mut self, typed: &str, via: SearchVia) {
        // `ext:log` words narrow what is searched; the rest is what to look for.
        let (query, exts) = crate::search::split_ext(typed);
        let query = query.as_str();
        // Only an extension: every file of that kind, which a content search
        // has nothing to look for in and a ranking has nothing to rank.
        let via = match via {
            SearchVia::Fuzzy if query.is_empty() => SearchVia::Name,
            v => v,
        };
        if typed.trim().is_empty() {
            return;
        }
        if query.is_empty() && via == SearchVia::Content {
            return self.error("A content search needs something to look for besides ext:");
        }
        // A query that is not a regular expression says why and starts nothing;
        // `F` is the one search that takes letters in order instead.
        let matcher = match via {
            SearchVia::Fuzzy => ito_match::Matcher::fuzzy(query),
            _ => match ito_match::Matcher::new(query) {
                Ok(m) => m,
                Err(e) => return self.error(format!("Not a regular expression: {e}")),
            },
        };
        let root = self.tabs[self.active].cwd.clone();
        let reach = crate::search::Reach { hidden: self.tabs[self.active].show_hidden, exts };
        let ctx = self.ctx.clone();
        let finder = Finder {
            query: query.to_owned(),
            matcher: Some(matcher.clone()),
            prev: false,
            names: via != SearchVia::Content,
            body: via != SearchVia::Name,
            ..Default::default()
        };
        let handle =
            crate::search::spawn(&root, matcher, typed, via, reach, 5000, self.cfg.ui.content_max_bytes, move || ctx.request_repaint());

        let tab = &mut self.tabs[self.active];
        tab.remember_cursor();
        let mut folder = Folder::loading(search_path(typed, &root), None);
        folder.state = LoadState::Ready;
        tab.current = folder;
        tab.finder = Some(finder);
        self.search = Some(handle);
        self.preview.state = PreviewState::Empty;
        self.preview.key = None;
    }

    /// Measure what is under each child of the current directory and show them
    /// largest first.
    ///
    /// The same trick the search view uses: a `Folder` whose path is not a real
    /// directory, filled in as the worker answers. That is the whole reason this
    /// needs no overlay, no keymap layer and no `is_modal` entry -- `j`/`k`, the
    /// wheel, selection, `y`, `d` and `<Esc>` all work because this is the file
    /// list, not a panel pretending to be one.
    /// `l` on an archive: list it on a worker, and show its top level once
    /// the list arrives. A tar has no index and is read end to end, so even
    /// listing it can take a while.
    pub(super) fn open_archive(&mut self, archive: PathBuf) {
        let (tx, rx) = crossbeam_channel::bounded(1);
        let ctx = self.ctx.clone();
        let path = archive.clone();
        std::thread::Builder::new()
            .name("archive-list".into())
            .spawn(move || {
                let _ = tx.send(ArchiveMsg::Listed(path.clone(), archive::list(&path, ARCHIVE_LIMIT)));
                ctx.request_repaint();
            })
            .expect("spawn archive thread");
        self.archive_rx = Some(rx);
        let tab = &mut self.tabs[self.active];
        tab.remember_cursor();
        tab.current = Folder::loading(archive_path(&archive, ""), None);
        let scratch = preview_scratch(&archive);
        self.archive_view = Some(ArchiveView {
            archive,
            inner: String::new(),
            members: Arc::new(Vec::new()),
            more: false,
            copies: HashMap::new(),
            copying: None,
            refused: BTreeSet::new(),
            scratch,
        });
        self.preview.state = PreviewState::Empty;
        self.preview.key = None;
    }

    /// Put the level `inner` of the archive on screen, the cursor on `want`.
    pub(super) fn archive_show(&mut self, inner: String, want: Option<String>) {
        let Some(view) = &mut self.archive_view else { return };
        view.inner = inner;
        let base = view.inner.split('/').filter(|p| !p.is_empty()).fold(view.archive.clone(), |p, part| p.join(part));
        let entries: Vec<Entry> = archive::level(&view.members, &view.inner)
            .into_iter()
            .map(|row| Entry {
                path: base.join(&row.name),
                ext: (!row.dir)
                    .then(|| util::stem_and_ext(&row.name).1.trim_start_matches('.').to_ascii_lowercase())
                    .filter(|e| !e.is_empty()),
                hidden: row.name.starts_with('.'),
                kind: if row.dir { Kind::Dir } else { Kind::File },
                len: row.size,
                name: row.name,
                ..Default::default()
            })
            .collect();
        let path = archive_path(&view.archive, &view.inner);
        let tab = &mut self.tabs[self.active];
        let mut folder = Folder::from_entries(path, Arc::new(entries), tab.show_hidden);
        if let Some(name) = want {
            folder.select_name(&name);
        }
        tab.current = folder;
        tab.finder = None;
        self.preview.key = None;
        self.request_preview(true);
    }

    /// Drop the archive view, and with it any members still selected: they
    /// are not files, and outside the view `d` sent the paths to the trash,
    /// which took them without a word and was then said to have (#251).
    pub(super) fn leave_archive(&mut self) {
        if let Some(view) = self.archive_view.take() {
            self.tabs[self.active].selected.retain(|p| p == &view.archive || !p.starts_with(&view.archive));
        }
        self.archive_rx = None;
    }

    pub(super) fn archive_down(&mut self, name: &str) {
        let Some(view) = &self.archive_view else { return };
        let inner = match view.inner.is_empty() {
            true => name.to_owned(),
            false => format!("{}/{name}", view.inner),
        };
        self.archive_show(inner, None);
    }

    /// `l` or `<Enter>` on a file inside an archive: unpack that one member
    /// into a folder of kura's own under the temporary folder, on a worker,
    /// and open the copy with the system's default app. A copy, and said to
    /// be one: changes to it do not go back into the archive.
    pub(super) fn open_member(&mut self, entry: &Entry) {
        let Some(view) = &self.archive_view else { return };
        let member = match view.inner.is_empty() {
            true => entry.name.clone(),
            false => format!("{}/{}", view.inner, entry.name),
        };
        let archive = view.archive.clone();
        // The member's folders inside the archive are kept, so `a/readme.txt`
        // and `b/readme.txt` do not take turns at one copy (#251).
        let base = util::archive_scratch().join(util::file_name(&archive));
        let dest = view.inner.split('/').filter(|p| !p.is_empty()).fold(base, |p, part| p.join(part));
        let out = dest.join(&entry.name);
        let (tx, rx) = crossbeam_channel::bounded(1);
        let ctx = self.ctx.clone();
        std::thread::Builder::new()
            .name("archive-member".into())
            .spawn(move || {
                let got = archive::extract_one(&archive, &member, &dest, &mut |_, _| true).map(|()| out);
                let _ = tx.send(ArchiveMsg::Copied(got));
                ctx.request_repaint();
            })
            .expect("spawn archive thread");
        self.archive_rx = Some(rx);
        self.toast(format!("Unpacking {}…", entry.name));
    }

    pub(super) fn drain_archive(&mut self) {
        if let Some(Ok(ArchiveMsg::Previewed(fake, got))) = self.archive_preview_rx.as_ref().map(|rx| rx.try_recv()) {
            self.archive_preview_rx = None;
            if let Some(view) = &mut self.archive_view {
                view.copying = None;
                match got {
                    Ok(real) => {
                        view.copies.insert(fake, real);
                    }
                    Err(_) => {
                        view.refused.insert(fake);
                    }
                }
                // Whatever the cursor is on now: the one just unpacked, or
                // one it moved to while that was going.
                self.preview.key = None;
                self.request_preview(true);
            }
        }
        let Some(rx) = &self.archive_rx else { return };
        let Ok(msg) = rx.try_recv() else { return };
        self.archive_rx = None;
        match msg {
            ArchiveMsg::Listed(path, listed) => {
                // Left, or went into another, before the list came back.
                if self.archive_view.as_ref().is_none_or(|v| v.archive != path) {
                    return;
                }
                match listed {
                    Ok((members, more)) => {
                        if let Some(view) = &mut self.archive_view {
                            view.members = Arc::new(members);
                            view.more = more;
                        }
                        self.archive_show(String::new(), None);
                        if more {
                            self.toast(format!("Showing the first {ARCHIVE_LIMIT} entries of {}", util::file_name(&path)));
                        }
                    }
                    Err(e) => {
                        self.exit_search_view();
                        self.error(format!("{}: {e}", util::file_name(&path)));
                    }
                }
            }
            ArchiveMsg::Copied(Ok(path)) => {
                self.toasts.retain(|t| !t.text.starts_with("Unpacking "));
                match exec::open_default(&path) {
                    Ok(()) => self.toast(format!("Opened a copy of {} — changes stay out of the archive", util::file_name(&path))),
                    Err(e) => self.error(format!("Open failed: {e}")),
                }
            }
            ArchiveMsg::Previewed(..) => {}
            ArchiveMsg::Copied(Err(e)) => {
                self.toasts.retain(|t| !t.text.starts_with("Unpacking "));
                self.error(format!("Unpack failed: {e}"));
            }
        }
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

    pub(super) fn start_usage(&mut self) {
        if self.in_search_view() {
            self.error("Usage: leave this view first");
            return;
        }
        self.usage_root = Some(self.tabs[self.active].cwd.clone());
        self.usage_want = None;
        self.measure_here();
    }

    /// Move the usage view to `dir` and measure it, keeping where `gu` started.
    /// `want` is the name to put the cursor on when it turns up.
    pub(super) fn remeasure(&mut self, dir: PathBuf, want: Option<String>) {
        let root = self.usage_root.clone();
        self.exit_search_view();
        self.cd(dir, true);
        self.usage_root = root;
        self.usage_want = want;
        // The level just left had its own total; it is not this one's.
        self.toasts.retain(|t| !t.text.contains(" in total"));
        self.measure_here();
    }

    pub(super) fn measure_here(&mut self) {
        let root = self.tabs[self.active].cwd.clone();
        let ctx = self.ctx.clone();
        let handle = crate::fs::usage::spawn(&root, move || ctx.request_repaint());

        let tab = &mut self.tabs[self.active];
        tab.remember_cursor();
        let mut folder = Folder::loading(usage_path(&root), None);
        folder.state = LoadState::Ready;
        tab.current = folder;
        self.usage = Some(handle);
        self.usage_max = 0;
        self.usage_linemode = Some(tab.linemode);
        tab.linemode = crate::fs::entry::Linemode::Usage;
        self.preview.state = PreviewState::Empty;
        self.preview.key = None;
        self.toast("Measuring… <Esc> to leave");
    }

    pub(super) fn drain_usage(&mut self) {
        let Some(handle) = &self.usage else { return };
        let mut batch: Vec<crate::fs::usage::Child> = Vec::new();
        let mut done: Option<(u64, bool)> = None;
        while let Ok(msg) = handle.rx.try_recv() {
            match msg {
                crate::fs::usage::Msg::Sized(mut v) => batch.append(&mut v),
                crate::fs::usage::Msg::Done { total, capped } => {
                    done = Some((total, capped));
                    break;
                }
            }
        }
        if !batch.is_empty() {
            let f = &mut self.tabs[self.active].current;
            let entries = Arc::make_mut(&mut f.entries);
            for (path, bytes, _files, whole) in batch {
                if let Ok(mut e) = Entry::from_path(path) {
                    e.usage = Some(bytes);
                    e.usage_cut = !whole;
                    entries.push(e);
                }
            }
            // Biggest first, which is the question being asked. Sorted here
            // rather than through `SortSpec` because the tab's own sort puts
            // every directory above every file, and a usage list that does that
            // cannot be read.
            entries.sort_by(|a, b| b.usage_bytes().cmp(&a.usage_bytes()).then(a.name.cmp(&b.name)));
            self.usage_max = entries.first().map_or(0, Entry::usage_bytes);
            f.rebuild(true);
            if let Some(name) = self.usage_want.take() {
                if !f.select_name(&name) {
                    self.usage_want = Some(name);
                }
            }
        }
        if let Some((total, capped)) = done {
            self.usage = None;
            self.toasts.retain(|t| !t.text.starts_with("Measuring"));
            self.toast(format!(
                "{} in total{} — <Esc> to leave",
                crate::util::human_size(total),
                if capped { " (walk cut short; totals are floors)" } else { "" }
            ));
        }
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

    pub(super) fn drain_search(&mut self) {
        let Some(handle) = &self.search else { return };
        let mut batch: Vec<PathBuf> = Vec::new();
        let mut done: Option<(usize, bool, Vec<PathBuf>, usize, usize)> = None;
        while let Ok(msg) = handle.rx.try_recv() {
            match msg {
                crate::search::Msg::Found(mut v) => batch.append(&mut v),
                crate::search::Msg::Done { total, truncated, order, binary, capped } => {
                    done = Some((total, truncated, order, binary, capped));
                    break;
                }
            }
        }
        if !batch.is_empty() {
            let show_hidden = true;
            let f = &mut self.tabs[self.active].current;
            let entries = Arc::make_mut(&mut f.entries);
            for p in batch {
                if let Ok(e) = Entry::from_path(p) {
                    entries.push(e);
                }
            }
            f.rebuild(show_hidden);
        }
        if let Some((total, truncated, order, binary, capped)) = done {
            let scope = self.search.take().map(|h| crate::search::scope(&h.root, &h.reach)).unwrap_or_default();
            if !order.is_empty() {
                // Best match first, and the cursor on it.
                let f = &mut self.tabs[self.active].current;
                let rank: HashMap<&Path, usize> = order.iter().enumerate().map(|(i, p)| (p.as_path(), i)).collect();
                Arc::make_mut(&mut f.entries).sort_by_key(|e| rank.get(e.path.as_path()).copied().unwrap_or(usize::MAX));
                f.rebuild(true);
                f.cursor = 0;
                f.offset = 0;
            }
            // A content search says what it did not read, so a miss is not a
            // mystery: binary files are never searched.
            let mut skipped = if binary > 0 { format!("; {binary} binary file(s) not searched") } else { String::new() };
            if capped > 0 {
                skipped.push_str(&format!(
                    "; {capped} large file(s) searched only in their first {}",
                    util::human_size(self.cfg.ui.content_max_bytes)
                ));
            }
            if total == 0 {
                self.error(format!("No matches {scope}{skipped}"));
            } else {
                self.toast(format!(
                    "{total} match(es) {scope}{}{skipped} — <Esc> to leave the search view",
                    if truncated { " (truncated)" } else { "" }
                ));
            }
        }
    }
}
