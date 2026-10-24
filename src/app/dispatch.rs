//! The action dispatcher: `act` and the small actions it calls.

use super::*;

impl App {
    pub fn run(&mut self, acts: &[Act]) {
        for a in acts {
            self.act(a.clone());
        }
    }

    pub fn act(&mut self, a: Act) {
        self.last_action = Instant::now();
        if self.outline_act(&a) {
            return;
        }
        // Inside an archive the rows are not files yet, and the folder `p`
        // or `a` would write into is the one the archive sits in, not the one
        // on screen. These say so rather than act on paths that are not
        // there; `y` is let through, and `p` outside takes the members out.
        if self.archive_view.is_some() && changes_files(&a) {
            self.toast(archive_refusal(&a));
            return;
        }
        match a {
            Act::Noop | Act::Unsupported(_) => {
                if let Act::Unsupported(what) = a {
                    self.error(format!("Not supported: {what}"));
                }
            }
            Act::Escape(what) => self.escape(what),
            // Every panel that has a keymap layer of its own -- `help`, `tasks`,
            // `spot` -- binds `q` to `close`. Quick look and a maximized preview
            // deliberately have no layer, so that `j` and `k` keep walking the
            // list underneath, which also meant `q` fell through to this arm and
            // quit the process with a panel still on screen. Peel the front layer
            // instead, through `escape` so the two keys cannot drift apart; a
            // second `q` quits, as it does in every panel.
            Act::Quit => {
                if self.quick || self.max_preview {
                    self.escape(EscapeWhat::default());
                } else {
                    self.quit = true;
                }
            }
            Act::Close => {
                if self.tabs.len() > 1 {
                    self.close_tab(self.active);
                } else {
                    self.quit = true;
                }
            }

            Act::Swipe(n) => self.act(Act::Arrow(Step::Rel(n))),
            Act::Arrow(step) => {
                let page = self.tabs[self.active].page_rows.max(1);
                self.tabs[self.active].current.arrow(step, page);
                self.tabs[self.active].sync_visual();
                self.preview.pending_since = None;
            }
            Act::Leave => self.leave(),
            Act::Enter => self.enter(),
            Act::Back => {
                if let Some(p) = self.tabs[self.active].back.pop() {
                    let cur = self.tabs[self.active].cwd.clone();
                    self.tabs[self.active].forward.push(cur);
                    self.cd(p, false);
                }
            }
            Act::Forward => {
                if let Some(p) = self.tabs[self.active].forward.pop() {
                    let cur = self.tabs[self.active].cwd.clone();
                    self.tabs[self.active].back.push(cur);
                    self.cd(p, false);
                }
            }
            Act::Cd { target, interactive } => {
                if interactive || target.is_empty() {
                    let cwd = self.tabs[self.active].cwd.clone();
                    self.open_input(InputKind::Cd, "Change directory", dir_prefill(&cwd));
                } else {
                    let base = self.tabs[self.active].cwd.clone();
                    self.cd(util::resolve_against(&base, &target), true);
                }
            }
            Act::Reveal(target) => self.reveal(target),
            Act::Follow => self.follow_link(),
            Act::Refresh => {
                let cwd = self.tabs[self.active].cwd.clone();
                self.cache.remove(&cwd);
                self.preview.cache.clear();
                self.counted.clear();
                self.rescan(&cwd);
                self.request_preview(true);
            }

            Act::Seek(step) => self.seek(step),

            Act::TabCreate { current, path } => self.create_tab(current, path),
            Act::TabClose(n) => {
                let idx = n.unwrap_or(self.active);
                self.close_tab(idx);
            }
            Act::TabSwitch { n, relative } => self.tab_switch(n, relative),
            Act::TabSwap(n) => self.tab_swap(n),

            Act::Split(state) => {
                if state.unwrap_or(self.split.is_none()) {
                    self.open_split();
                } else {
                    self.close_split();
                }
            }
            Act::PaneFocus(side) => self.pane_focus(side),

            Act::Toggle { state } => {
                self.tabs[self.active].toggle(state);
            }
            Act::ToggleAll { state } => self.tabs[self.active].toggle_all(state),
            Act::VisualMode { unset } => {
                if self.tabs[self.active].visual.is_some() {
                    self.tabs[self.active].leave_visual();
                } else {
                    self.tabs[self.active].enter_visual(unset);
                }
            }

            Act::Open { interactive, hovered } => {
                let _ = hovered;
                self.open(interactive, None);
            }
            Act::Yank { cut } => self.yank(cut),
            Act::Unyank => {
                self.yank.paths.clear();
                self.yank.cut = false;
            }
            Act::Paste { force, follow } => self.paste(force, follow),
            Act::Link { relative } => self.link(OpKind::Symlink { relative }),
            Act::Hardlink => self.link(OpKind::Hardlink),
            Act::Remove { permanently, force, hovered } => self.remove(permanently, force, hovered),
            Act::Create { dir, force } => {
                let _ = force;
                let title = if dir { "Create directory" } else { "Create (end with / for a directory)" };
                self.open_input(InputKind::Create, title, String::new());
            }
            Act::Rename { force, cursor } => {
                let _ = force;
                self.start_rename(cursor);
            }
            Act::Copy(what) => self.copy_text(what),
            Act::Shell { run, block, confirm, orphan } => {
                if run.is_empty() || confirm {
                    self.open_input(InputKind::Shell { block }, "Shell", run);
                } else {
                    self.run_shell(&run, block, orphan);
                }
            }

            Act::Hidden(state) => self.hidden(state),
            Act::Linemode(m) => self.tabs[self.active].linemode = m,
            Act::Sort { by, reverse, dir_first } => self.sort(by, reverse, dir_first),

            Act::Find { prev, smart, insensitive } => {
                let _ = (smart, insensitive);
                self.open_input(InputKind::Find { prev }, if prev { "Find previous" } else { "Find next" }, String::new());
            }
            Act::FindArrow { prev } => self.find_arrow(prev),
            // Only the comparison view has matching rows to hide; `diff_act` takes it there.
            Act::HideSame => {}
            Act::Filter { smart, insensitive } => self.filter(smart, insensitive),
            Act::Usage => self.start_usage(),
            Act::Search { via, .. } => {
                self.open_input(
                    InputKind::Search { via },
                    match via {
                        SearchVia::Name => "Search by name",
                        SearchVia::Content => "Search by content",
                        SearchVia::Fuzzy => "Search fuzzily",
                    },
                    String::new(),
                );
            }
            Act::Submit => self.submit_input(),
            Act::Complete => self.complete_input(),
            Act::Jump => self.open_jump(),
            Act::Undo => self.undo_step(),
            Act::Redo => self.redo_step(),
            Act::ConfigReload => self.reload_config(),
            // Deliberately not an overlay: with the panel up, every key still
            // works, so `j` and `k` walk the list and the panel follows.
            Act::Quick(state) => self.quick = state.unwrap_or(!self.quick),
            Act::Zoom(to) => self.zoom_preview(to),
            Act::Minimap(state) => {
                self.cfg.ui.minimap = state.unwrap_or(!self.cfg.ui.minimap);
            }
            Act::BulkRename => self.start_bulk_rename(),
            Act::Compare => self.start_compare(),

            Act::Help => {
                self.help_scroll = 0;
                self.overlay = Overlay::Help;
            }
            // Opening a browser is a visible thing to do to someone's machine,
            // so it says what it did rather than leaving a window to appear
            // from nowhere.
            // The same steps and bounds egui's own zoom used, so turning
            // that off and doing it here is not a change in feel.
            Act::Scale(to) => self.scale(to),
            Act::BugReport => self.bug_report(),
            Act::Settings => self.toggle_settings(),
            Act::TasksShow => self.overlay = Overlay::Tasks(TasksOverlay { cursor: 0 }),
            // These act on the row the task panel has under its cursor, so
            // they open it first when it is not the overlay in front.
            Act::TaskToggle | Act::TaskCancel | Act::TaskTop => {
                if !matches!(self.overlay, Overlay::Tasks(_)) {
                    self.overlay = Overlay::Tasks(TasksOverlay { cursor: 0 });
                }
                self.tasks_act(a);
            }
            Act::Spot => self.open_spot(),
            Act::Palette => self.open_palette(),
            Act::Menu => self.open_menu(),
            Act::Terminal(what) => self.terminal(what),
            Act::TermSend => self.term_send_paths(),
            Act::TermCd => self.term_pull_cwd(),
            Act::TermFind { prev, repeat } => self.term_search(prev, repeat),
            Act::PreviewFind { prev, repeat } => self.preview_search(prev, repeat),
            Act::PreviewFollow => self.preview_follow(),
            Act::PreviewCopyLine => self.preview_copy_line(),
            Act::TermScroll(step) => self.term_scroll(step),
            Act::TermPrompt { prev } => self.term_prompt(prev),
            Act::TermCopyOutput => self.term_copy_output(),
            Act::Extract => self.do_extract(),
            Act::Compress => self.ask_compress(),
            Act::SendPane { cut } => self.send_to_pane(cut),
            Act::ToggleOutline => self.toggle_outline(),
            Act::ToggleRender => self.toggle_render(),

            // Nothing to maximise without a pane, and leaving the flag set
            // would surprise whoever opens one next.
            //
            // Maximising also hands the pane the keys. Pressed from the list,
            // it otherwise hides that list while leaving it holding them, so
            // the next keystroke goes somewhere off screen. Tying the two
            // together means maximised always implies the pane has the keys --
            // which is what makes leaving the pane the only way back.
            Act::MaxTerm => {
                self.max_term = self.term.is_some() && !self.max_term;
                if self.max_term {
                    self.term_focus = true;
                }
            }

            Act::MaxPreview => {
                self.max_preview = !self.max_preview;
                if self.max_preview {
                    self.hide_parent = false;
                }
            }
            Act::TogglePaneParent => self.hide_parent = !self.hide_parent,
            Act::BookmarkSave => self.pending_bookmark = Some(BookmarkOp::Save),
            Act::BookmarkJump => self.pending_bookmark = Some(BookmarkOp::Jump),
            Act::BookmarkList => self.open_bookmark_list(),
            Act::BookmarkDelete => self.pending_bookmark = Some(BookmarkOp::Delete),
            Act::BookmarkDeleteAll => {
                self.overlay = Overlay::Confirm(ConfirmOverlay {
                    title: "Delete all bookmarks?".into(),
                    body: vec![format!("{} bookmarks will be removed.", self.bookmarks.len())],
                    options: vec![('y', "Yes".into()), ('n', "No".into())],
                    action: ConfirmAction::BookmarkDeleteAll,
                    dest: None,
                });
            }
        }
    }

    /// Move the keys to the other pane of a split.
    pub(super) fn pane_focus(&mut self, side: Option<bool>) {
        // The first press splits the view; the next moves the keys.
        self.open_split();
        if let Some(sp) = self.split {
            if side.unwrap_or(!sp.right) != sp.right {
                self.focus_pane(sp.other);
            }
        }
    }

    /// Narrow the listing to what matches as it is typed.
    pub(super) fn filter(&mut self, smart: bool, insensitive: bool) {
        let _ = (smart, insensitive);
        let current = self.tabs[self.active].current.filter.as_ref().map(|f| f.query.clone()).unwrap_or_default();
        self.open_input(InputKind::Filter, "Filter", current);
    }

    /// Go to a path and put the cursor on it, rather than merely into its folder.
    pub(crate) fn reveal(&mut self, target: String) {
        let base = self.tabs[self.active].cwd.clone();
        let p = util::resolve_against(&base, &target);
        let name = util::file_name(&p);
        if let Some(dir) = p.parent() {
            self.cd(dir.to_path_buf(), true);
            let cwd = self.tabs[self.active].cwd.clone();
            self.tabs[self.active].memo.insert(cwd, name.clone());
            self.tabs[self.active].current.select_name(&name);
        }
    }

    /// The huge log on show in the preview, if that is what it is.
    pub(super) fn log_view(&self) -> Option<Arc<crate::preview::logindex::LogView>> {
        match &self.preview.state {
            PreviewState::Ready(Payload::Log { view }) => Some(view.clone()),
            _ => None,
        }
    }

    /// Turn following the end of the huge log on show on or off.
    pub(super) fn preview_follow(&mut self) {
        if self.log_view().is_none() {
            self.toast("Following works on a huge log in the preview");
            return;
        }
        self.log_follow = !self.log_follow;
        let msg = if self.log_follow { "Following the log" } else { "Stopped following" };
        self.toast(msg);
        if self.log_follow {
            self.log_polled = None;
        }
    }

    /// Look for lines added to the huge log on show (twice a second) and, when
    /// following, scroll to the last. Returns when to look again, if one is on show.
    pub fn tick_log(&mut self) -> Option<Duration> {
        const EVERY: Duration = Duration::from_millis(500);
        let Some(view) = self.log_view() else {
            self.log_follow = false;
            return None;
        };
        if let Some(t) = self.log_polled.filter(|t| t.elapsed() < EVERY) {
            return Some(EVERY - t.elapsed());
        }
        self.log_polled = Some(Instant::now());
        let before = view.index.lines();
        let grow = view.index.grow();
        if grow == crate::preview::logindex::Grow::Grew {
            // What was added is looked through for the find on show, too.
            view.find_more();
        }
        match grow {
            crate::preview::logindex::Grow::Rebuild => {
                // Cut or replaced: read it again from the front.
                self.preview.key = None;
                self.request_preview(true);
            }
            _ if self.log_follow => {
                let now = view.index.lines();
                let tab = &mut self.tabs[self.active];
                if now > before || tab.preview_offset + 1 < now {
                    tab.preview_offset = now.saturating_sub(1);
                }
            }
            _ => {}
        }
        Some(EVERY)
    }

    /// Copy the whole line the find stepped to (else the top line of the view):
    /// the screen only shows the first 8 KB of a long one.
    pub(super) fn preview_copy_line(&mut self) {
        let Some(view) = self.log_view() else {
            return self.toast("Copy line works on a huge text file");
        };
        let tab = &self.tabs[self.active];
        // The hit counts while it is still on screen; after a scroll away the top line does.
        let top = tab.preview_offset;
        let line = tab.find_at.filter(|l| (top..top + 30).contains(l)).unwrap_or(top);
        match view.index.full_line(line) {
            Ok(text) => match exec::set_clipboard(&text) {
                Ok(()) => self.toast(format!("Copied line {} ({})", line + 1, crate::util::human_size(text.len() as u64))),
                Err(err) => self.error(format!("Clipboard: {err}")),
            },
            Err(0) => self.error("Could not read the line"),
            Err(n) => self.error(format!("Line {} is too long to copy ({})", line + 1, crate::util::human_size(n))),
        }
    }

    /// Search the huge log in the preview, or step to the next match.
    pub(super) fn preview_search(&mut self, prev: bool, repeat: bool) {
        if self.log_view().is_none() {
            return self.toast("Preview find works on a huge text file");
        }
        if repeat && !self.log_needle.is_empty() {
            return self.log_step(prev);
        }
        let needle = self.log_needle.clone();
        self.open_input(InputKind::PreviewFind, "Find in log", needle);
    }

    pub(super) fn preview_find_submit(&mut self, text: &str) {
        let Some(view) = self.log_view() else { return };
        if text.is_empty() {
            return;
        }
        match ito_match::Matcher::new(text) {
            Ok(m) => {
                self.log_needle = text.to_owned();
                self.log_matcher = Some(m.clone());
                self.tabs[self.active].find_at = None;
                view.find(text, m);
                self.toast("Searching...");
            }
            Err(e) => self.toast(format!("Find: {e}")),
        }
    }

    /// Jump to the next (or previous) hit of the log search.
    pub(super) fn log_step(&mut self, prev: bool) {
        let Some(view) = self.log_view() else { return };
        let found = view.found();
        let tab = &self.tabs[self.active];
        let next = next_hit(&found.lines, tab.find_at, tab.preview_offset, prev);
        match next {
            Some(l) => {
                self.tabs[self.active].find_at = Some(l as usize);
                self.tabs[self.active].preview_offset = (l as usize).saturating_sub(3);
                view.focus(l as usize);
                let bytes = view.index.line_bytes(l as usize);
                if bytes > crate::preview::logindex::MAX_LINE_BYTES as u64 {
                    self.toast(format!("Long line ({} KB): showing around the match", bytes.div_ceil(1024)));
                }
            }
            None if !found.done => self.toast(format!("Searching... {} hits so far", found.lines.len())),
            None => self.toast(format!("No more matches for {}", found.query)),
        }
    }

    /// Search the terminal's scrollback, or step to the next match.
    pub(super) fn term_search(&mut self, prev: bool, repeat: bool) {
        let needle = self.term_needle.clone();
        match repeat && !needle.is_empty() {
            // `--prev` walks back towards the bottom, because the
            // search itself runs the other way: what you are looking
            // for in a terminal has scrolled off the top.
            true => self.term_find(&needle, !prev),
            // Nothing to repeat, so ask what to look for.
            false => self.open_input(InputKind::TermFind, "Find in terminal", needle),
        }
    }

    /// Show or hide the dotfiles, in every pane at once.
    pub(super) fn hidden(&mut self, state: crate::config::cmd::Tri) {
        let tab = &mut self.tabs[self.active];
        tab.show_hidden = state.unwrap_or(!tab.show_hidden);
        let show = tab.show_hidden;
        tab.current.rebuild(show);
        if let Some(p) = tab.parent.as_mut() {
            p.rebuild(show);
        }
        if let PreviewState::Dir(f) = &mut self.preview.state {
            f.rebuild(show);
        }
    }

    /// `KURA_SCALE` for one run: the scale `<C-=>` and `<C-->` would reach,
    /// without a press (#228), and common.toml's is then left alone. `1.2`
    /// or `120%`; a value that is not one from 0.2 to 5.0 is ignored, and the
    /// toast says so.
    pub fn start_scaled(&mut self, text: Option<&str>) {
        let Some(text) = text.map(str::trim).filter(|t| !t.is_empty()) else { return };
        match ito_common::parse_scale(text) {
            Some(next) => {
                self.set_scale(next);
                self.scale_pinned = true;
            }
            None => self.error(format!("KURA_SCALE={text} is not a scale from 0.2 to 5.0 (or 20% to 500%)")),
        }
    }

    pub(super) fn set_scale(&mut self, s: f32) {
        self.scale = s;
        self.ctx.set_zoom_factor(s);
    }

    /// A zoom key: the step is ito's, as in every uchmk app, and it is
    /// written to common.toml so the others follow.
    pub(super) fn scale(&mut self, to: crate::config::cmd::ScaleTo) {
        let next = ito_common::scale_step(self.scale, to);
        self.set_scale(next);
        self.scale_pressed = Some(Instant::now());
        self.write_scale(next);
        // Held down, the toast's `×N` keeps counting after the scale has
        // stopped moving; saying so is what tells the two apart.
        let said = ito_common::scale_text(next, &self.lang);
        self.toast_instead(&["Scale ", "倍率 "], said);
    }

    /// Hand the scale to the thread that writes common.toml, started at the
    /// first press. A held key sends many; it writes only the newest.
    pub(super) fn write_scale(&mut self, s: f32) {
        let Some(base) = self.common_base.clone().filter(|_| !self.scale_pinned) else { return };
        let failed = self.watched_tx.clone();
        let tx = self.scale_tx.get_or_insert_with(|| {
            let (tx, rx) = crossbeam_channel::unbounded::<f32>();
            let _ = std::thread::Builder::new().name("kura-scale".into()).spawn(move || {
                while let Ok(first) = rx.recv() {
                    let s = rx.try_iter().last().unwrap_or(first);
                    if let Err(e) = ito_common::edit_common(&base, |t| ito_common::scale_change(s).apply(t)) {
                        if let Some(tx) = &failed {
                            let _ = tx.send(Watched::Failed(format!("common.toml: {e}")));
                        }
                    }
                }
            });
            tx
        });
        let _ = tx.send(s);
    }

    /// Move the current tab along the bar.
    pub(super) fn tab_swap(&mut self, n: i64) {
        let len = self.tabs.len() as i64;
        let from = self.active;
        let to = (self.active as i64 + n).rem_euclid(len) as usize;
        self.tabs.swap(from, to);
        self.active = to;
        if let Some(sp) = self.split {
            self.split = Some(Split { other: split_after_swap(sp.other, from, to), ..sp });
        }
        self.check_split();
    }

    /// Switch Markdown between the rendered view and its source, keeping the
    /// place: the two have different line numbers for the same content.
    pub(super) fn toggle_render(&mut self) {
        self.render_markdown = !self.render_markdown;
        // Stay on the same part of the document across the switch.
        if let PreviewState::Ready(Payload::Markdown { doc, .. }) = &self.preview.state {
            let tab = &mut self.tabs[self.active];
            tab.preview_offset =
                if self.render_markdown { doc.line_for_src(tab.preview_offset) } else { doc.src_for_line(tab.preview_offset) };
        }
        // A body search's `n` / `N` go on from the match last shown, and its
        // line number is the old view's: put it on the nearest match of this one.
        let at = self.tabs[self.active].finder.as_ref().and_then(|f| f.at);
        if let Some(at) = at {
            let was = if self.render_markdown { self.markdown_src_to_line(at) } else { self.markdown_line_to_src(at) };
            let near = self.body_matches().into_iter().min_by_key(|&m| m.abs_diff(was));
            if let Some(f) = self.tabs[self.active].finder.as_mut() {
                f.at = near;
            }
        }
    }

    pub(super) fn markdown_line_to_src(&self, line: usize) -> usize {
        match &self.preview.state {
            PreviewState::Ready(Payload::Markdown { doc, .. }) => doc.src_for_line(line),
            _ => line,
        }
    }

    pub(super) fn markdown_src_to_line(&self, line: usize) -> usize {
        match &self.preview.state {
            PreviewState::Ready(Payload::Markdown { doc, .. }) => doc.line_for_src(line),
            _ => line,
        }
    }

    /// Go to a tab by number, or step between them.
    pub(super) fn tab_switch(&mut self, n: i64, relative: bool) {
        let len = self.tabs.len() as i64;
        let idx = if relative { (self.active as i64 + n).rem_euclid(len) } else { n.clamp(0, len - 1) } as usize;
        if self.other_pane() == Some(idx) {
            // It is already on screen: move the keys to its pane.
            self.focus_pane(idx);
        } else {
            self.switch_tab(idx);
        }
    }

    /// Walk the terminal's scrollback.
    pub(super) fn term_scroll(&mut self, step: Step) {
        if let Some(t) = &self.term {
            use ito_pane::alacritty_terminal::grid::Scroll;
            let page = t.size().lines as i64;
            // Negated, because the two conventions run opposite ways.
            // `term_scroll -50%` reads like `arrow -50%` and means
            // half a screen back, while alacritty counts a positive
            // delta as older. Taking the number as it stood sent
            // `<S-PageUp>` towards the bottom, where there is nothing
            // to go to, so the one key most likely to be tried first
            // did nothing at all.
            t.scroll(match step {
                Step::Top => Scroll::Top,
                Step::Bot => Scroll::Bottom,
                Step::Rel(n) => Scroll::Delta(-(n as i32)),
                Step::Pct(p) => Scroll::Delta(-((page * p / 100) as i32)),
            });
        }
    }

    /// Scroll the terminal to the next prompt up or down. Says why when it
    /// cannot: a shell that marks no prompts looks the same as one with no
    /// prompt left to go to, and only the first has something to fix.
    pub(super) fn term_prompt(&mut self, prev: bool) {
        let Some(t) = &self.term else { return };
        if t.jump_prompt(prev) {
            return;
        }
        let said = match t.blocks().is_empty() && !t.prompt_seen() {
            true => "The shell does not mark its prompts (OSC 133), so there is none to jump to",
            false if prev => "No prompt above this one",
            false => "No prompt below this one",
        };
        self.toast(said);
    }

    /// Copy what the last command printed, between its prompt and the next.
    pub(super) fn term_copy_output(&mut self) {
        let Some(t) = &self.term else { return };
        let Some(text) = t.command_output() else {
            self.toast("No finished command to copy (the shell has to mark its prompts, OSC 133)");
            return;
        };
        match crate::exec::set_clipboard(&text) {
            Ok(()) => {
                let n = text.lines().count();
                self.toast(format!("Copied the last command's output ({n} line{})", if n == 1 { "" } else { "s" }));
            }
            Err(e) => self.error(format!("Could not copy: {e}")),
        }
    }

    /// Scroll the preview, or step to the next picture where a configured
    /// previewer draws one at a time.
    pub(super) fn seek(&mut self, step: Step) {
        let page = self.tabs[self.active].page_rows.max(1) as i64;
        let delta = match step {
            Step::Rel(n) => n,
            Step::Pct(p) => page * p / 100,
            Step::Top => i64::MIN / 2,
            Step::Bot => i64::MAX / 2,
        };
        // A configured previewer draws one picture at a time, so
        // there is nothing to scroll: the keys step to the next
        // picture instead. Same keys, because it is the same
        // intention -- further into this file.
        if let Some(rule) = crate::config::PreviewRule::for_path(&self.cfg.preview, &self.hovered_path()) {
            let by = match step {
                Step::Rel(n) => n.signum() * rule.step,
                Step::Pct(p) => p.signum() * rule.step,
                Step::Top => return self.go_to_picture(rule.first),
                Step::Bot => return,
            };
            let want = self.preview.n + by;
            return self.go_to_picture(want.max(rule.first));
        }
        // Clamped here rather than only after the draw. The draw used
        // to be handed an offset past the end, paint a frame from
        // beyond the content, and fix the number afterwards — one bad
        // frame per keypress at the bottom of a file, which is why it
        // took a held-down key to see.
        let cur = self.tabs[self.active].preview_offset as i64;
        let want = cur.saturating_add(delta).max(0) as usize;
        self.tabs[self.active].preview_offset = want.min(self.preview.max_offset);
    }

    pub(super) fn escape(&mut self, what: EscapeWhat) {
        if !self.overlay.is_none() {
            self.cancel_input();
            return;
        }
        if self.pending_bookmark.take().is_some() {
            return;
        }
        // The panel is the thing in front of everything else, so it goes first.
        if self.quick {
            self.quick = false;
            return;
        }
        let all = what.everything();
        // A maximized preview squeezes the list to nothing, so the screen reads
        // as modal even though only a column width changed. `Esc` doing nothing
        // there left `q` as the next thing to try, and `q` quits the app -- an
        // expensive way to find out that this one was not a panel. Restoring the
        // columns costs nothing if that is not what was wanted: `T` again.
        // Only for a bare `escape`; `escape --filter` and friends stay targeted.
        if all && self.max_preview {
            self.max_preview = false;
            return;
        }
        if all && self.abandon_cd() {
            return;
        }
        if (all || what.search) && self.in_search_view() {
            self.exit_search_view();
            return;
        }
        let tab = &mut self.tabs[self.active];
        if (all || what.visual) && tab.leave_visual() {
            return;
        }
        if (all || what.filter) && tab.current.filter.is_some() {
            tab.current.filter = None;
            let show = tab.show_hidden;
            tab.current.rebuild(show);
            return;
        }
        if (all || what.find || what.search) && tab.finder.take().is_some() {
            return;
        }
        if all || what.select {
            tab.clear_selection();
        }
    }
}

/// The hit a log search steps to: from the last hit stepped to, or else from the
/// top of the view (a hit on the first rows counts, so the first step is not
/// skipped past a match that is already on screen).
fn next_hit(lines: &[u32], find_at: Option<usize>, top: usize, prev: bool) -> Option<u32> {
    let from = find_at.unwrap_or(top + 3);
    match prev {
        true => lines.iter().rev().copied().find(|&l| (l as usize) < from),
        false => lines.iter().copied().find(|&l| (l as usize) > from || (find_at.is_none() && l as usize >= top)),
    }
}

#[cfg(test)]
mod next_hit_tests {
    use super::next_hit;

    #[test]
    fn first_step_finds_a_hit_on_the_first_rows() {
        assert_eq!(next_hit(&[0, 50], None, 0, false), Some(0));
        assert_eq!(next_hit(&[1, 50], None, 0, false), Some(1));
    }

    #[test]
    fn steps_on_from_the_last_hit() {
        assert_eq!(next_hit(&[0, 50], Some(0), 0, false), Some(50));
        assert_eq!(next_hit(&[0, 50], Some(50), 47, true), Some(0));
        assert_eq!(next_hit(&[0, 50], Some(50), 47, false), None);
    }

    #[test]
    fn first_step_skips_hits_above_the_view() {
        assert_eq!(next_hit(&[2, 90], None, 40, false), Some(90));
    }
}
