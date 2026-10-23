//! Changing directory, history, entering and leaving.

use super::*;

impl App {
    pub fn cd(&mut self, target: PathBuf, push_history: bool) {
        self.cd_inner(target, push_history, false);
    }

    /// `cd` for a path someone typed. If it turns out to name a file, the tab
    /// lands on the parent with that file under the cursor.
    pub(super) fn cd_or_reveal(&mut self, target: PathBuf) {
        self.cd_inner(target, true, true);
    }

    /// Move the focused tab to `target`.
    ///
    /// No directory check happens here: on a share that stopped answering,
    /// `is_dir` can block for half a minute, and the UI thread cannot wait.
    /// The tab moves at once and the background scan has the last word —
    /// `Tab::cd_failed` decides what to do when the listing never arrives.
    pub(super) fn cd_inner(&mut self, target: PathBuf, push_history: bool, fallback: bool) {
        let target = util::normalize(&target);
        if self.tabs[self.active].cwd == target {
            return;
        }
        let from = self.tabs[self.active].cwd.clone();
        self.cd_refused = None;
        // A jump from inside an archive leaves it.
        self.leave_archive();
        {
            let tab = &mut self.tabs[self.active];
            tab.remember_cursor();
            if push_history {
                tab.back.push(from.clone());
                tab.forward.clear();
            }
            tab.visual = None;
            tab.mouse_range = None;
            tab.finder = None;
        }

        let active = self.active;
        let pending = PendingCd { from, pushed: push_history, fallback, reveal: None };
        self.arrive(active, target.clone(), Some(pending));

        self.remember_history(&target);
        self.kick_scans();
        // Even a cache hit gets a background refresh, so the view is never stale.
        if self.cache.peek(&target).is_some() {
            let sort = self.tabs[self.active].sort;
            let id = self.scanner.scan(target.clone(), sort);
            self.inflight.insert(id, target);
        }
    }

    /// Put a tab on `target` and show whatever the cache already holds.
    /// `pending` survives only while the listing is still missing: a cached
    /// directory was listed before and needs no second opinion.
    pub(super) fn arrive(&mut self, idx: usize, target: PathBuf, pending: Option<PendingCd>) {
        let show_hidden = self.tabs[idx].show_hidden;
        let cur = match self.cache.get(&target) {
            Some(entries) => Folder::from_entries(target.clone(), entries.clone(), show_hidden),
            None => Folder::loading(target.clone(), None),
        };
        self.tabs[idx].cwd = target.clone();
        self.tabs[idx].current = cur;
        self.tabs[idx].preview_offset = 0;
        self.tabs[idx].recall_cursor();

        let parent = util::parent_dir(&target);
        self.tabs[idx].parent = parent.map(|p| match self.cache.get(&p) {
            Some(entries) => {
                let mut f = Folder::from_entries(p.clone(), entries.clone(), show_hidden);
                f.select_name(&util::file_name(&target));
                f
            }
            None => Folder::loading(p, None),
        });

        let listed = self.tabs[idx].current.state != LoadState::Loading;
        // A listing already here answers a jump's question now: the scan for
        // the parent column often lands before the jump's own fails, and the
        // pending jump -- with the name it was to reveal -- used to be dropped
        // unasked, so a typed path that named nothing landed in silence about
        // one run in two (#242).
        if listed {
            if let Some(name) = pending.as_ref().and_then(|p| p.reveal.as_deref()) {
                if !self.tabs[idx].current.entries.iter().any(|e| e.name == name) {
                    // The cursor goes to the nearest name rather than the top.
                    let near = crate::core::fuzzy::closest(name, self.tabs[idx].current.entries.iter().map(|e| e.name.as_str()))
                        .map(str::to_owned);
                    self.error(nothing_there(name, &target, near.as_deref()));
                    if let Some(near) = near {
                        self.tabs[idx].current.select_name(&near);
                    }
                }
            }
        }
        self.tabs[idx].pending_cd = pending.filter(|_| !listed);

        if idx == self.active {
            self.preview.state = PreviewState::Empty;
            self.preview.key = None;
            self.preview.texture = None;
            self.preview.pending_since = None;
            self.preview.in_flight = false;
        }
    }

    /// A jump whose listing never arrived: undo it rather than leave the tab
    /// parked on a path that did not answer.
    pub(super) fn undo_cd(&mut self, idx: usize, path: &Path, error: &str) {
        match self.tabs[idx].cd_failed() {
            CdFallout::Keep => {}
            CdFallout::Reveal { to, pending } => {
                self.arrive(idx, to, Some(pending));
                self.kick_scans();
            }
            CdFallout::Revert { to, asked } => {
                self.arrive(idx, to, None);
                self.kick_scans();
                let path = asked.as_deref().unwrap_or(path);
                self.error(format!("{}: {error}", path.display()));
                self.cd_refused = Some(path.to_path_buf());
            }
        }
    }

    /// `<Esc>` while a jump is still waiting for its first listing: go back to
    /// where it came from. A mistyped address held the tab for twenty seconds
    /// with nothing to do but wait (#105). The scan itself cannot be called
    /// off, so its answer is let through quietly when it comes.
    pub(super) fn abandon_cd(&mut self) -> bool {
        let tab = &mut self.tabs[self.active];
        if tab.current.state != LoadState::Loading {
            return false;
        }
        let Some(p) = tab.pending_cd.take() else { return false };
        if p.pushed {
            tab.back.pop();
        }
        let gone = tab.cwd.clone();
        self.arrive(self.active, p.from, None);
        self.kick_scans();
        self.cd_refused = Some(gone.clone());
        self.toast(format!("Stopped waiting for {}", gone.display()));
        true
    }

    /// Count a visit. The hit count survives the move to the end of the list,
    /// so a directory worked in every day climbs even though each visit looks
    /// like the last one.
    pub(super) fn remember_history(&mut self, path: &Path) {
        let hits = match self.history.iter().position(|v| v.path == path) {
            Some(i) => self.history.remove(i).hits.saturating_add(1),
            None => 1,
        };
        self.history.push(Visit { path: path.to_path_buf(), hits, at: epoch_secs() });
        let max = self.cfg.ui.max_history;
        if self.history.len() > max {
            let cut = self.history.len() - max;
            self.history.drain(..cut);
        }
    }

    pub fn exit_search_view(&mut self) {
        self.search = None;
        self.leave_archive();
        self.archive_preview_rx = None;
        // Dropping the handle cancels the walk, so leaving is all it takes.
        self.usage = None;
        self.usage_max = 0;
        self.usage_root = None;
        self.usage_want = None;
        if let Some(mode) = self.usage_linemode.take() {
            self.tabs[self.active].linemode = mode;
        }
        // It said the walk was still going, which stopped being true just now
        // (#109).
        self.toasts.retain(|t| !t.text.starts_with("Measuring"));
        // And the total told how to leave a view that is no longer up.
        self.toasts.retain(|t| !t.text.ends_with("<Esc> to leave"));
        let cwd = self.tabs[self.active].cwd.clone();
        let show_hidden = self.tabs[self.active].show_hidden;
        let folder = match self.cache.get(&cwd) {
            Some(entries) => Folder::from_entries(cwd.clone(), entries.clone(), show_hidden),
            None => Folder::loading(cwd.clone(), None),
        };
        self.tabs[self.active].current = folder;
        self.tabs[self.active].finder = None;
        self.tabs[self.active].recall_cursor();
        self.kick_scans();
    }

    pub(super) fn enter(&mut self) {
        let Some(entry) = self.tabs[self.active].current.hovered().cloned() else { return };
        if self.archive_view.is_some() {
            match entry.is_dir_like() {
                true => self.archive_down(&entry.name),
                false => self.open_member(&entry),
            }
            return;
        }
        // An archive is gone into like a folder (Q75). Only from a real
        // listing: from a search or usage result `h` would not know where
        // to come back to.
        if !entry.is_dir_like() && !self.in_search_view() && archive::Format::from_path(&entry.path).is_some() {
            self.open_archive(entry.path);
            return;
        }
        // In the usage view a folder is measured in turn, the way `ncdu` goes
        // down: the view stays, and `h` comes back up.
        if entry.is_dir_like() && self.in_usage_view() {
            self.remeasure(entry.path, None);
        } else if entry.is_dir_like() {
            self.cd(entry.path, true);
        } else if !self.focus_outline() && !self.preview_ready() {
            // Still loading: move in once it arrives. A file without an
            // outline just stays where it is; opening is `open`'s job.
            self.preview.outline_wanted = Some(entry.path);
        }
    }

    pub(super) fn leave(&mut self) {
        if let Some(view) = &self.archive_view {
            match view.inner.rsplit_once('/') {
                Some((up, name)) => {
                    let (up, name) = (up.to_owned(), name.to_owned());
                    self.archive_show(up, Some(name));
                }
                None if !view.inner.is_empty() => {
                    let name = view.inner.clone();
                    self.archive_show(String::new(), Some(name));
                }
                None => self.exit_search_view(),
            }
            return;
        }
        let cwd = self.tabs[self.active].cwd.clone();
        if self.in_usage_view() && self.usage_root.as_ref().is_some_and(|r| cwd.starts_with(r) && *r != cwd) {
            if let Some(p) = util::parent_dir(&cwd) {
                self.remeasure(p, Some(util::file_name(&cwd)));
                return;
            }
        }
        if self.in_search_view() {
            self.exit_search_view();
            return;
        }
        let cwd = self.tabs[self.active].cwd.clone();
        if let Some(p) = util::parent_dir(&cwd) {
            let name = util::file_name(&cwd);
            self.tabs[self.active].memo.insert(p.clone(), name);
            self.cd(p, true);
        }
    }
}
