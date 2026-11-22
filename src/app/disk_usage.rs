//! The disk-usage view: measuring a folder and draining the sizes.

use super::*;

/// The toast that says the walk stopped by itself. It ends with the same words
/// as the others, so leaving the view takes it away with them.
const PAUSED_TOAST: &str = "Big folder: paused — gu measures the rest, <Esc> to leave";

impl App {
    /// `gu`. Outside the usage view it measures the folder in full, never
    /// pausing. Inside it, a walk that is limited (`m a`, a jump in Details) is
    /// told to measure the rest; anything else is as it was.
    pub(super) fn start_usage(&mut self) {
        if self.usage_linemode.is_some() {
            self.measure_the_rest();
            return;
        }
        self.start_usage_as(false);
    }

    /// The details view's own start: measured by itself, paused when heavy.
    pub(super) fn start_usage_auto(&mut self) {
        if self.usage_linemode.is_none() {
            self.start_usage_as(true);
        }
    }

    fn start_usage_as(&mut self, auto: bool) {
        if self.in_search_view() {
            self.error("Usage: leave this view first");
            return;
        }
        self.usage_root = Some(self.tabs[self.active].cwd.clone());
        self.usage_want = None;
        self.measure_here(auto);
    }

    /// `gu` in the view: go on from where a limited walk stands. A finished
    /// walk, or one that was never limited, says nothing.
    fn measure_the_rest(&mut self) {
        let Some(handle) = self.usage.as_ref().filter(|h| h.limited()) else { return };
        handle.measure_all();
        self.usage_paused = false;
        self.toasts.retain(|t| !t.text.starts_with("Measuring") && t.text != PAUSED_TOAST);
        self.toast("Measuring the rest… <Esc> to leave");
    }

    /// Move the usage view to `dir` and measure it, keeping where `gu` started.
    /// `want` is the name to put the cursor on when it turns up. The new folder
    /// is measured the way the view was started.
    pub(super) fn remeasure(&mut self, dir: PathBuf, want: Option<String>) {
        let root = self.usage_root.clone();
        let auto = self.usage_auto;
        self.exit_search_view();
        self.cd_move(dir, true, false, false);
        self.usage_root = root;
        self.usage_want = want;
        // The level just left had its own total; it is not this one's.
        self.toasts.retain(|t| !t.text.contains(" in total"));
        self.measure_here(auto);
    }

    /// Measure the folder a Details tab has landed on after a jump, once the
    /// landing is settled: listed, not a file or a missing path, no search or
    /// archive view in the way. Until then the flag waits.
    pub(super) fn follow_usage(&mut self) {
        let Some(idx) = self.usage_follow else { return };
        let tab = &self.tabs[self.active];
        if idx != self.active
            || tab.linemode != crate::fs::entry::Linemode::All
            || self.in_usage_view()
            || self.archive_view.is_some()
            || self.in_search_view()
        {
            self.usage_follow = None;
            return;
        }
        if tab.current.state != LoadState::Ready || tab.pending_cd.is_some() {
            return;
        }
        let want = tab.current.hovered_name().map(str::to_owned);
        self.usage_follow = None;
        self.start_usage_auto();
        if self.in_usage_view() {
            self.usage_want = want;
        }
    }

    /// End the usage view before the active tab changes: the walk is not the
    /// tab's, it is the app's, and `drain_usage` fills whichever tab is active.
    pub(super) fn leave_tab_usage(&mut self) {
        self.usage_follow = None;
        if self.in_usage_view() {
            self.exit_search_view();
        }
    }

    /// Start a walk of the active tab's folder. `auto` measures by itself for
    /// `[ui] usage_auto_ms` and then pauses; otherwise it measures to the end.
    pub(super) fn measure_here(&mut self, auto: bool) {
        let root = self.tabs[self.active].cwd.clone();
        let ctx = self.ctx.clone();
        let pause_after = auto.then(|| Duration::from_millis(self.cfg.ui.usage_auto_ms));
        let handle = crate::fs::usage::spawn(&root, pause_after, move || ctx.request_repaint());

        let tab = &mut self.tabs[self.active];
        tab.remember_cursor();
        let mut folder = Folder::loading(usage_path(&root), None);
        folder.state = LoadState::Ready;
        tab.current = folder;
        self.usage = Some(handle);
        self.usage_max = 0;
        self.usage_placed = false;
        self.usage_auto = auto;
        self.usage_paused = false;
        self.usage_listed = false;
        self.usage_linemode = Some(tab.linemode);
        if tab.linemode != crate::fs::entry::Linemode::All {
            tab.linemode = crate::fs::entry::Linemode::Usage;
        }
        self.preview.state = PreviewState::Empty;
        self.preview.key = None;
        self.toast("Measuring… <Esc> to leave");
    }

    pub(super) fn drain_usage(&mut self) {
        use crate::fs::usage::Msg;
        let Some(handle) = &self.usage else { return };
        // Everything waiting is read first and applied in order in one pass, so
        // the list is sorted and rebuilt once however many batches came.
        let mut msgs: Vec<Msg> = Vec::new();
        while let Ok(msg) = handle.rx.try_recv() {
            let end = matches!(msg, Msg::Done { .. });
            msgs.push(msg);
            if end {
                break;
            }
        }
        if msgs.is_empty() {
            return;
        }
        // A `Paused` that comes after `gu` was pressed is stale.
        let limited = handle.limited();

        let details = self.tabs[self.active].linemode == crate::fs::entry::Linemode::All;
        let sort = self.tabs[self.active].sort;
        let f = &mut self.tabs[self.active].current;
        // Read before the list moves: `view` indexes into `entries`.
        let held = (self.usage_placed || f.cursor > 0).then(|| f.hovered_name().map(str::to_owned)).flatten();
        let entries = Arc::make_mut(&mut f.entries);
        // Built on this drain's first sized child, then kept up to date for the
        // rest of the drain: a folder can hold 68,000 rows.
        let mut index: Option<HashMap<PathBuf, usize>> = None;
        let mut changed = false;
        let mut done: Option<(u64, bool)> = None;
        let mut paused = false;
        for msg in msgs {
            match msg {
                Msg::Listed(v) => {
                    changed |= !v.is_empty();
                    for e in v {
                        if let Some(index) = index.as_mut() {
                            index.insert(e.path.clone(), entries.len());
                        }
                        entries.push(e);
                    }
                }
                Msg::Sized(v) => {
                    changed |= !v.is_empty();
                    let index = index.get_or_insert_with(|| entries.iter().enumerate().map(|(i, e)| (e.path.clone(), i)).collect());
                    for (e, bytes, _files, whole) in v {
                        // The row `Listed` earlier keeps whatever else it has
                        // gained (details metadata); one never listed is new.
                        let at = match index.get(&e.path) {
                            Some(&i) => i,
                            None => {
                                index.insert(e.path.clone(), entries.len());
                                entries.push(e);
                                entries.len() - 1
                            }
                        };
                        entries[at].usage = Some(bytes);
                        entries[at].usage_cut = !whole;
                    }
                }
                Msg::AllListed => self.usage_listed = true,
                Msg::Paused => paused = limited,
                Msg::Done { total, capped } => done = Some((total, capped)),
            }
        }
        if changed {
            // Biggest first, which is the question being asked. Sorted here
            // rather than through `SortSpec` because the tab's own sort puts
            // every directory above every file, and a usage list that does that
            // cannot be read. A folder not measured yet has no total, so it goes
            // after the ones that have, under any size sort.
            if details {
                if !matches!(sort.by, crate::fs::SortBy::Owner | crate::fs::SortBy::Dimensions)
                    || entries.iter().all(|e| e.details.is_some())
                {
                    sort.apply_usage(entries);
                }
            } else {
                entries.sort_by(|a, b| {
                    a.usage.is_none().cmp(&b.usage.is_none()).then(b.usage_bytes().cmp(&a.usage_bytes())).then(a.name.cmp(&b.name))
                });
            }
            self.usage_max = entries.iter().filter_map(|e| e.usage).max().unwrap_or(0);
            f.rebuild(true);
            match held {
                Some(name) => {
                    f.select_name(&name);
                }
                // Nobody placed it: the top row, even with a filter, where rebuild's stale read can land lower.
                None => f.cursor = 0,
            }
            if let Some(name) = self.usage_want.take() {
                if f.select_name(&name) {
                    self.usage_placed = true;
                } else {
                    self.usage_want = Some(name);
                }
            }
        }
        if paused && done.is_none() {
            self.usage_paused = true;
            self.toasts.retain(|t| !t.text.starts_with("Measuring"));
            self.toast(PAUSED_TOAST);
        }
        if let Some((total, capped)) = done {
            self.usage = None;
            self.usage_paused = false;
            self.toasts.retain(|t| !t.text.starts_with("Measuring") && t.text != PAUSED_TOAST);
            self.toast(format!(
                "{} in total{} — <Esc> to leave",
                crate::util::human_size(total),
                if capped { " (walk cut short; totals are floors)" } else { "" }
            ));
        }
    }
}
