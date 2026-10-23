//! The spot panel, the overlay keys and the task list actions.

use super::*;

impl App {
    pub(super) fn open_spot(&mut self) {
        if self.tabs[self.active].current.hovered().is_none() {
            return;
        }
        self.overlay = Overlay::Spot(SpotOverlay { cursor: 0, scroll: 0 });
        self.sync_spot();
    }

    /// Keep the worker pointed at the file under the cursor.
    ///
    /// The panel is a page about one file, so it has to be about the hovered
    /// one -- and the cursor moves in ways the panel does not hear directly.
    /// `h` and `l` change directory, and what ends up hovered there is decided
    /// by a listing that arrives frames later on a worker thread, so asking at
    /// the moment the key is pressed would ask about nothing. Called once a
    /// frame as well, which is what makes that case work.
    pub(super) fn sync_spot(&mut self) {
        if !matches!(self.overlay, Overlay::Spot(_)) {
            self.spot_asked = None;
            return;
        }
        let Some(path) = self.tabs[self.active].current.hovered().map(|e| e.path.clone()) else {
            return;
        };
        if self.spot_asked.as_ref() == Some(&path) {
            return;
        }
        self.spot_asked = Some(path.clone());
        self.spotter.request(path);
    }

    /// What the spot panel shows for the hovered file: the listing's facts,
    /// what its preview found, then the worker's findings once they are in.
    pub fn spot_sections(&self) -> Vec<Section> {
        let Some(entry) = self.tabs[self.active].current.hovered() else { return Vec::new() };
        let mut out = vec![spot::base(entry)];
        if self.preview_ready() {
            if let PreviewState::Ready(p) = &self.preview.state {
                out.extend(self.preview_section(p));
            }
        }
        if let Some((path, found)) = &self.spotted {
            if *path == entry.path {
                out.extend(found.iter().cloned());
            }
        }
        out
    }

    pub(super) fn preview_section(&self, payload: &Payload) -> Option<Section> {
        let mut rows: Vec<(String, String)> = Vec::new();
        let mut row = |k: &str, v: String| rows.push((k.into(), v));
        let read = |truncated: bool| truncated.then(|| format!("first {} only", util::human_size(self.cfg.ui.max_text_bytes as u64)));
        match payload {
            Payload::Text { extent, outline, .. } => {
                row("Lines", extent.lines());
                if !outline.is_empty() {
                    row("Outline", format!("{} entries", outline.len()));
                }
                if let Some(r) = read(extent.truncated) {
                    row("Read", r);
                }
            }
            Payload::Markdown { doc, extent, .. } => {
                row("Lines", extent.lines());
                // A table stopped at its row cap says so here too (#205).
                match extent.rows {
                    Some(n) => row("Table", format!("first {n} rows only")),
                    None => row("Headings", doc.toc.len().to_string()),
                }
                if let Some(r) = read(extent.truncated) {
                    row("Read", r);
                }
            }
            Payload::Image { caption, .. } if !caption.is_empty() => row("Shows", caption.clone()),
            Payload::Meta { rows: meta } => rows.extend(meta.iter().cloned()),
            Payload::Error(e) => row("Error", e.clone()),
            _ => {}
        }
        (!rows.is_empty()).then(|| Section { title: "Preview".into(), rows })
    }

    /// The task panel's own commands: move, pause, cancel, reorder.
    /// Feed a key to the panel in front, through its own keymap layer.
    ///
    /// The four panels resolve keys identically -- push onto `pending`, resolve
    /// against the layer, run what a hit names, clear on a miss -- and differed
    /// only in which layer and which dispatcher. They were four copies of this,
    /// and adding a panel meant writing a fifth. The terminal and the file list
    /// are genuinely different (bytes for the shell, the `which` panel and a
    /// pending bookmark) and keep their own.
    pub fn feed_overlay_key(&mut self, k: Key) {
        let Some(layer) = PanelLayer::of(&self.overlay) else { return };
        self.note_key(&k);
        self.pending.push(k);
        let acts = match keymap::resolve(layer.bindings(&self.cfg.keymap), &self.pending) {
            keymap::Match::Exact(b) => b.run.clone(),
            // More keys may still follow, so what has been pressed stays.
            keymap::Match::Pending(_) => return,
            keymap::Match::None => {
                self.pending.clear();
                return;
            }
        };
        self.pending.clear();
        for a in acts {
            // The size of the window belongs to the window, not to whichever
            // panel happens to be over it. Handled here rather than in each
            // layer because all four `act`s end in a `_ => {}`: v0.46.0 bound
            // these chords in every panel and shipped them doing nothing, the
            // binding resolving and the action then being dropped in silence.
            // A fifth panel inherits this instead of repeating the mistake.
            if let Act::Scale(to) = a {
                self.scale(to);
                continue;
            }
            // F1 is help in every uchmk app and from anywhere in it (since
            // v0.99.0): over a panel it opens the key list in its place. The
            // help panel's own `help` closes it, so that one goes through.
            if a == Act::Help && layer != PanelLayer::Help {
                self.act(Act::Help);
                continue;
            }
            layer.act(self, a);
        }
    }

    pub(super) fn tasks_act(&mut self, a: Act) {
        let len = self.tasks.len();
        let page = self.tabs[self.active].page_rows.max(1);
        let Overlay::Tasks(ov) = &mut self.overlay else { return };
        match a {
            Act::Close | Act::Escape(_) | Act::TasksShow | Act::Quit => {
                self.overlay = Overlay::None;
            }
            Act::Arrow(step) if len > 0 => ov.cursor = step.apply(ov.cursor, len, page),
            Act::TaskToggle => self.toggle_task(),
            Act::TaskCancel => self.cancel_task(),
            Act::TaskTop => self.promote_task(),
            _ => {}
        }
    }

    /// The job the panel's cursor is on, when there is one.
    pub(super) fn selected_task(&self) -> Option<(u64, TaskState)> {
        let Overlay::Tasks(ov) = &self.overlay else { return None };
        self.tasks.get(ov.cursor).map(|t| (t.id, t.state.clone()))
    }

    pub(super) fn toggle_task(&mut self) {
        let Some((id, state)) = self.selected_task() else { return };
        match state {
            // A queued job has not started, so there is nothing to park.
            TaskState::Queued => self.toast("That job has not started yet"),
            TaskState::Running => self.ops.control(id, ops::Control::Pause),
            TaskState::Paused => self.ops.control(id, ops::Control::Resume),
            _ => {}
        }
    }

    pub(super) fn cancel_task(&mut self) {
        let Some((id, state)) = self.selected_task() else { return };
        if !state.is_live() {
            return;
        }
        // A job still in the queue never reaches the worker, so the panel
        // closes it out itself; a running one is told to stop and answers
        // with its own `Finished`.
        if self.ops.drop_queued(id) {
            if let Some(t) = self.tasks.iter_mut().find(|t| t.id == id) {
                t.state = TaskState::Cancelled;
                t.finished = Some(Instant::now());
            }
            return;
        }
        self.ops.control(id, ops::Control::Cancel);
    }

    pub(super) fn promote_task(&mut self) {
        let Some((id, state)) = self.selected_task() else { return };
        if state != TaskState::Queued {
            self.toast("Only a queued job can be moved up");
            return;
        }
        if !self.ops.promote(id) {
            return;
        }
        // The panel lists jobs in the order they were made, so say what
        // happened rather than leaving the row where it was.
        self.toast("Moved to the front of the queue");
    }

    /// The help panel's own keys, through the `[help]` layer like every other
    /// overlay.
    ///
    /// `j`, `k` and the arrows used to be read straight off the event in
    /// `main.rs`, which is why the panel that exists to show what the keys are
    /// was the one place they could not be changed -- and why it had no way to
    /// move by more than a line at a time through a list as long as the keymap.
    pub(super) fn help_act(&mut self, a: Act) {
        match a {
            Act::Close | Act::Escape(_) | Act::Quit | Act::Help => self.overlay = Overlay::None,
            // A page is the panel's own height, not the file list's: "half a
            // page" has to mean half of what is on screen here. The stop is a
            // scroll position, so it is one past `lines - rows`, not `lines - 1`.
            Act::Arrow(step) => {
                let page = self.help_rows.max(1);
                let stop = self.help_lines.saturating_sub(page) + 1;
                self.help_scroll = step.apply(self.help_scroll, stop, page);
            }
            // What the panel's own config rows tell you to press (#163).
            Act::ConfigReload => self.act(Act::ConfigReload),
            // The whole list as text, as spot's `C` does (Q48).
            Act::Copy(CopyWhat::All) => {
                let (text, keys) = crate::ui::overlay::help_text(self);
                match exec::set_clipboard(&text) {
                    Ok(()) => self.toast(format!("Copied the help panel: {keys} keys")),
                    Err(err) => self.error(format!("Clipboard: {err}")),
                }
            }
            _ => {}
        }
    }

    /// yazi's spot commands: close, arrow (rows), swipe (files), copy (cell).
    pub(super) fn spot_act(&mut self, a: Act) {
        let rows: usize = self.spot_sections().iter().map(|s| s.rows.len()).sum();
        let page = self.tabs[self.active].page_rows.max(1);
        let pr_url = if matches!(a, Act::Enter) { self.spot_pr_url() } else { None };
        let Overlay::Spot(ov) = &mut self.overlay else { return };
        match a {
            Act::Close | Act::Escape(_) | Act::Spot | Act::Quit => self.overlay = Overlay::None,
            Act::Arrow(step) => ov.cursor = step.apply(ov.cursor, rows, page),
            Act::Swipe(n) => {
                self.act(Act::Arrow(Step::Rel(n)));
                self.sync_spot();
            }
            // `h` and `l` change directory, as they do in the list and so as
            // they do under `<F3>`, which keeps the list's own keys live.
            // `<Enter>` on a pull request's rows opens its page (Q20), before
            // anything moves: the rows are what the cursor is on.
            Act::Enter if pr_url.is_some() => {
                let url = pr_url.unwrap_or_default();
                match exec::open_url(&url) {
                    Ok(()) => self.toast(format!("Opened {url}")),
                    Err(e) => self.error(format!("could not open the browser: {e}")),
                }
            }
            Act::Leave | Act::Enter => {
                // In the list, `enter` on a plain file focuses the preview's
                // outline -- another panel wanting these same keys. With the
                // spotter open only a directory is worth moving into.
                let into_dir = self.tabs[self.active].current.hovered().is_some_and(|e| e.is_dir_like());
                if matches!(a, Act::Enter) && !into_dir {
                    return;
                }
                self.act(a);
                // The listing is still on its way, so the file this lands on
                // is not known yet; the once-a-frame call catches it.
                self.sync_spot();
            }
            // Everything at once, labelled, so it can be pasted into a bug
            // report or a chat as text rather than a screenshot (Q18).
            Act::Copy(CopyWhat::All) if self.tabs[self.active].selected.len() > 1 => self.copy_spot_of_selection(),
            Act::Copy(CopyWhat::All) => {
                let sections = self.spot_sections();
                let text = spot_text(&sections);
                let rows: usize = sections.iter().map(|s| s.rows.len()).sum();
                match exec::set_clipboard(&text) {
                    Ok(()) => self.toast(format!("Copied the spot panel: {rows} rows")),
                    Err(err) => self.error(format!("Clipboard: {err}")),
                }
            }
            Act::Copy(_) => {
                let cursor = ov.cursor;
                let sections = self.spot_sections();
                let Some((key, value)) = sections.iter().flat_map(|s| &s.rows).nth(cursor) else { return };
                match exec::set_clipboard(value) {
                    Ok(()) => self.toast(format!("Copied {key}: {value}")),
                    Err(err) => self.error(format!("Clipboard: {err}")),
                }
            }
            _ => {}
        }
    }

    /// Copy the folder this tab shows (the path line's right-click).
    pub fn copy_cwd(&mut self) {
        let text = self.tabs[self.active].cwd.display().to_string();
        match exec::set_clipboard(&text) {
            Ok(()) => self.toast(format!("Copied {text}")),
            Err(err) => self.error(format!("Clipboard: {err}")),
        }
    }

    /// `C` with files selected: one panel per file under a `#### <path>`
    /// heading. Reading each file is the spot worker's kind of work, so it
    /// happens on a thread of its own and the toast follows when it is done.
    pub(super) fn copy_spot_of_selection(&mut self) {
        if self.spot_copy_rx.is_some() {
            self.toast("Still copying the spot panels");
            return;
        }
        // `selected` outlives the folder it was made in, so the entries are
        // built from the paths, not picked out of the listing on screen.
        let paths: Vec<PathBuf> = self.tabs[self.active].selected.iter().cloned().collect();
        let (tx, rx) = crossbeam_channel::bounded(1);
        self.spot_copy_rx = Some(rx);
        let ctx = self.ctx.clone();
        std::thread::spawn(move || {
            crate::preview::init_com_thread();
            let entries = entries_of_paths(paths);
            let text = spot_text_of(&entries);
            let _ = tx.send(exec::set_clipboard(&text).map(|()| entries.len()).map_err(|e| e.to_string()));
            ctx.request_repaint();
        });
    }

    /// The page the spot cursor is on, when it is on one: the pull request
    /// from its `Came in via` row or its own row, the branch from `From branch`.
    pub(super) fn spot_pr_url(&self) -> Option<String> {
        let Overlay::Spot(ov) = &self.overlay else { return None };
        let sections = self.spot_sections();
        let mut at = ov.cursor;
        for s in &sections {
            if at < s.rows.len() {
                return page_for(&s.rows, &s.rows[at].0);
            }
            at -= s.rows.len();
        }
        None
    }
}

/// The web page behind one row of spot's Git section. `From branch` sat
/// between two rows that opened and was the one that did nothing (#119); its
/// page is built from the pull request's, which already carries the
/// repository, so `<Enter>` asks git nothing on the UI thread.
pub(super) fn page_for(rows: &[(String, String)], key: &str) -> Option<String> {
    let pr = rows.iter().find(|(k, _)| k == crate::spot::PR_ROW).map(|(_, v)| v.as_str())?;
    match key {
        "Came in via" => Some(pr.to_owned()),
        k if k == crate::spot::PR_ROW => Some(pr.to_owned()),
        "From branch" => {
            let branch = rows.iter().find(|(k, _)| k == "From branch").map(|(_, v)| v.as_str())?;
            let repo = &pr[..pr.rfind("/pull/")?];
            Some(format!("{repo}/tree/{branch}"))
        }
        _ => None,
    }
}
