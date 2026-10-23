//! Folder scans, directory watching and applying a listing.

use super::*;

impl App {
    /// Ask for any listing the current view needs and does not have.
    pub fn kick_scans(&mut self) {
        let sort = self.tabs[self.active].sort;
        let cwd = self.tabs[self.active].cwd.clone();
        let parent = util::parent_dir(&cwd);

        if self.tabs[self.active].current.scan_id.is_none() && self.tabs[self.active].current.state == LoadState::Loading {
            let id = self.scanner.scan(cwd.clone(), sort);
            self.inflight.insert(id, cwd.clone());
            self.tabs[self.active].current.scan_id = Some(id);
        }
        if let Some(p) = parent {
            let needs = match &self.tabs[self.active].parent {
                Some(f) => f.scan_id.is_none() && f.state == LoadState::Loading,
                None => false,
            };
            if needs {
                let id = self.scanner.scan_low(p.clone(), sort);
                self.inflight.insert(id, p.clone());
                if let Some(f) = self.tabs[self.active].parent.as_mut() {
                    f.scan_id = Some(id);
                }
            }
        }
        // The other pane is not what the keys drive, so it waits in the low
        // priority queue behind the directory the cursor is in.
        if let Some(idx) = self.other_pane().filter(|&i| i < self.tabs.len() && i != self.active) {
            let f = &self.tabs[idx].current;
            if f.scan_id.is_none() && f.state == LoadState::Loading {
                let (path, sort) = (self.tabs[idx].cwd.clone(), self.tabs[idx].sort);
                let id = self.scanner.scan_low(path.clone(), sort);
                self.inflight.insert(id, path);
                self.tabs[idx].current.scan_id = Some(id);
            }
        }
        self.ensure_dir_sizes();
        self.sync_watcher();
    }

    /// `linemode size` shows a child count for directories; compute it only for
    /// the rows actually on screen, and only once per directory.
    ///
    /// Not in a host's listing (`\\server`): its rows are shares, and counting
    /// one is a `read_dir` across the network for a number nobody needs while
    /// picking a share. Its size column stays empty (Q32, TESTING.md 31.9).
    pub(super) fn ensure_dir_sizes(&mut self) {
        if !self.tabs[self.active].linemode.wants_dir_size() {
            return;
        }
        if util::host_only_unc(&self.tabs[self.active].cwd) {
            return;
        }
        let tab = &self.tabs[self.active];
        let start = tab.current.offset;
        let end = (start + tab.page_rows + 1).min(tab.current.view.len());
        let mut want = Vec::new();
        for row in start..end {
            let Some(e) = tab.current.at(row) else { continue };
            if e.is_dir_like() && e.dir_size.is_none() && !self.counted.contains(&e.path) {
                want.push(e.path.clone());
            }
        }
        if want.is_empty() {
            return;
        }
        for p in &want {
            self.counted.insert(p.clone());
        }
        self.scanner.count(want);
    }

    pub(super) fn sync_watcher(&mut self) {
        let mut dirs: Vec<PathBuf> = vec![self.tabs[self.active].cwd.clone()];
        if let Some(p) = self.tabs[self.active].cwd.parent() {
            dirs.push(p.to_path_buf());
        }
        if let Some(idx) = self.other_pane().filter(|&i| i < self.tabs.len()) {
            dirs.push(self.tabs[idx].cwd.clone());
        }
        if let PreviewState::Dir(f) = &self.preview.state {
            dirs.push(f.path.clone());
        }
        let refs: Vec<&Path> = dirs.iter().map(PathBuf::as_path).collect();
        self.watcher.sync(refs);
    }

    pub fn drain_channels(&mut self, ctx: &egui::Context) {
        while let Ok(res) = self.scanner.rx.try_recv() {
            self.on_scan(res);
        }
        while let Ok(res) = self.previewer.rx.try_recv() {
            self.on_preview(res, ctx);
        }
        while let Ok(res) = self.spotter.rx.try_recv() {
            self.spotted = Some((res.path, res.sections));
        }
        if let Some(rx) = &self.spot_copy_rx {
            if let Ok(res) = rx.try_recv() {
                self.spot_copy_rx = None;
                match res {
                    Ok(n) => self.toast(format!("Copied the spot panels of {n} files")),
                    Err(err) => self.error(format!("Clipboard: {err}")),
                }
            }
        }
        while let Ok(res) = self.differ.rx.try_recv() {
            // An answer to a comparison that has since been closed, or replaced
            // by another pair, has nowhere to go.
            if let Overlay::Diff(ov) = &mut self.overlay {
                if ov.left == res.left && ov.right == res.right {
                    ov.arrive(res.outcome);
                }
            }
        }
        while let Ok(ev) = self.ops.rx.try_recv() {
            self.on_op_event(ev);
        }
        while let Ok(dir) = self.watcher.rx.try_recv() {
            self.dirty.insert(dir, Instant::now());
        }
        while let Ok(rep) = self.git.rx.try_recv() {
            self.git_status.put(rep.dir, Arc::new(rep.status));
        }
        while let Ok(themes) = self.themes_rx.try_recv() {
            self.set_themes(themes);
        }
        while let Some(read) = self.common_rx.as_ref().and_then(|rx| rx.try_recv().ok()) {
            self.set_common(read);
        }
        while let Ok((bad, text)) = self.settings_rx.try_recv() {
            if bad {
                self.error(text);
            } else {
                self.toast(text);
            }
        }
        while let Ok(w) = self.kura_rx.try_recv() {
            self.kura_written(w);
        }
        // A launch reports at most once, and its watcher lets go of the channel
        // when it stops caring, so a disconnected one is finished with.
        let mut failed = Vec::new();
        self.launches.retain(|(l, line, what, note)| match l.rx.try_recv() {
            Ok(msg) => {
                // The watcher words every failure as an opener's; a shell line is not one.
                let msg = match msg.strip_prefix("Open failed") {
                    Some(rest) => format!("{what}{rest}"),
                    None => msg,
                };
                // The `(no path: …)` note of the `$ …` toast this replaces, kept so the
                // failure still says kura left the path off (#290).
                failed.push((format!("{msg}{note}"), line.clone()));
                false
            }
            Err(crossbeam_channel::TryRecvError::Empty) => true,
            Err(crossbeam_channel::TryRecvError::Disconnected) => false,
        });
        for (msg, line) in failed {
            // `launched:` in `<State:>` says a PID, which a check reads as
            // "it started"; once the watcher heard it fall over it says so (#276).
            if let Some(l) = self
                .last_launch
                .as_mut()
                .filter(|l| l.strip_prefix("launched: ").and_then(|r| r.split_once(' ')).is_some_and(|(_, rest)| rest == line))
            {
                *l = format!("launch failed: {line}");
            }
            // The failure names the line itself, so the `$ …` toast of the same
            // launch would show the path twice (#278).
            let started = format!("$ {line}");
            self.toasts.retain(|t| !t.text.starts_with(&started));
            self.error(msg);
        }
        self.drain_search();
        self.drain_usage();
        self.drain_archive();
        self.drain_watched();
        self.sync_spot();
        self.pump_terminal();
        self.flush_dirty();
        self.toasts.retain(|t| t.at.elapsed() < Duration::from_secs(6));
        self.tasks.retain(|t| match t.finished {
            Some(at) => at.elapsed() < Duration::from_secs(20) || !t.errors.is_empty(),
            None => true,
        });
    }

    /// How long a flagged directory waits for the watcher to go quiet before it
    /// is read again — editors and installers touch a directory many times in a row.
    pub(super) const RESCAN_QUIET: Duration = Duration::from_millis(150);

    /// Rescan directories the watcher flagged, once they have been quiet for
    /// [`Self::RESCAN_QUIET`].
    pub(super) fn flush_dirty(&mut self) {
        if self.dirty.is_empty() {
            return;
        }
        let ready: Vec<PathBuf> = self.dirty.iter().filter(|(_, t)| t.elapsed() > Self::RESCAN_QUIET).map(|(p, _)| p.clone()).collect();
        for p in ready {
            self.dirty.remove(&p);
            self.cache.remove(&p);
            self.rescan(&p);
        }
    }

    /// When the next frame is owed to a flagged directory, if one is waiting.
    ///
    /// The watcher's own wake arrives the instant a change does, when the entry
    /// is not yet quiet enough to read, and an idle window asks for no frame
    /// after it -- so the change stayed off the screen until a key was pressed
    /// (#108). The frame loop asks for one at this moment instead; with nothing
    /// flagged it asks for none, and an idle window still costs nothing (47).
    pub fn rescan_due(&self) -> Option<Duration> {
        let oldest = self.dirty.values().map(|t| t.elapsed()).max()?;
        Some(Self::RESCAN_QUIET.saturating_sub(oldest) + Duration::from_millis(10))
    }

    pub(super) fn rescan(&mut self, path: &Path) {
        let sort = self.tabs[self.active].sort;
        let mut wanted = false;
        for idx in self.pane_tabs() {
            if self.tabs[idx].cwd == path {
                wanted = true;
            }
        }
        if let Some(p) = self.tabs[self.active].parent.as_ref() {
            if p.path == path {
                wanted = true;
            }
        }
        if let PreviewState::Dir(f) = &self.preview.state {
            if f.path == path {
                wanted = true;
            }
        }
        if !wanted {
            return;
        }
        let id = self.scanner.scan(path.to_path_buf(), sort);
        self.inflight.insert(id, path.to_path_buf());
    }

    pub(super) fn on_scan(&mut self, res: ScanResult) {
        match res {
            ScanResult::Listed { id, path, entries } => {
                self.inflight.remove(&id);
                let entries = Arc::new(entries);
                self.cache.put(path.clone(), entries.clone());
                self.apply_listing(&path, entries.clone());
                self.complete_from(id, &path, &entries);
            }
            ScanResult::Failed { id, path, error } => {
                self.inflight.remove(&id);
                // A completion that cannot be listed simply has no answer; the
                // toast below still says why.
                if self.pending_completion.as_ref().is_some_and(|p| p.id == id) {
                    self.pending_completion = None;
                }
                // A jump that was never listed is undone first: the tab goes
                // back where it was instead of showing an empty error column.
                let jumped = self.pane_tabs().into_iter().find(|&i| self.tabs[i].cwd == path && self.tabs[i].pending_cd.is_some());
                if let Some(idx) = jumped {
                    self.undo_cd(idx, &path, &error);
                    return;
                }
                let mut hit = false;
                for idx in self.pane_tabs() {
                    if self.tabs[idx].cwd != path {
                        continue;
                    }
                    let show_hidden = self.tabs[idx].show_hidden;
                    let f = &mut self.tabs[idx].current;
                    f.state = LoadState::Error(error.clone());
                    f.entries = Arc::new(Vec::new());
                    f.scan_id = None;
                    f.rebuild(show_hidden);
                    hit = true;
                }
                if !hit {
                    if let PreviewState::Dir(f) = &mut self.preview.state {
                        if f.path == path {
                            f.state = LoadState::Error(error.clone());
                            hit = true;
                        }
                    }
                }
                // Only when nothing on screen carries it. A pane or the preview
                // that just took `LoadState::Error` is already showing this, in
                // place and in the same words; a toast on top says it twice, and
                // opening `C:\` says it three times at once -- one per system
                // folder Windows refuses -- with the toasts stacked over the
                // pane that already explained itself.
                // A folder above a jump still waiting, or above the one just
                // taken back, fails for the same reason; that jump says it.
                let explained = self.tabs.iter().any(|t| t.pending_cd.is_some() && is_within(&t.cwd, &path))
                    || self.cd_refused.as_ref().is_some_and(|p| is_within(p, &path));
                if !hit && !explained {
                    self.error(format!("{}: {error}", util::file_name(&path)));
                }
            }
            ScanResult::Counted { counts, .. } => {
                let map: HashMap<&PathBuf, u64> = counts.iter().map(|(p, n)| (p, *n)).collect();
                for tab in &mut self.tabs {
                    let entries = Arc::make_mut(&mut tab.current.entries);
                    for e in entries.iter_mut() {
                        if let Some(n) = map.get(&e.path) {
                            e.dir_size = Some(*n);
                        }
                    }
                }
            }
        }
    }

    pub(super) fn apply_listing(&mut self, path: &Path, entries: Arc<Vec<Entry>>) {
        // A directory a pane is showing gets its git status asked for. Every
        // rescan comes through here, so a file operation or a change the
        // watcher caught refreshes the marks along with the listing.
        if self.pane_tabs().into_iter().any(|i| self.tabs[i].cwd == path) {
            self.git.request(path.to_path_buf());
        }
        let show_hidden = self.tabs[self.active].show_hidden;
        let memo = self.tabs[self.active].memo.get(path).cloned();

        // A view standing in for the listing (search, usage) keeps the pane;
        // the jump still stands.
        if self.tabs[self.active].cwd == path && self.tabs[self.active].current.path != path {
            self.tabs[self.active].pending_cd = None;
        } else if self.tabs[self.active].cwd == path {
            // The directory answered, so the jump that led here stands.
            let reveal = self.tabs[self.active].pending_cd.take().and_then(|p| p.reveal);
            let mut near = None;
            if let Some(name) = reveal.filter(|n| !entries.iter().any(|e| &e.name == n)) {
                near = crate::core::fuzzy::closest(&name, entries.iter().map(|e| e.name.as_str())).map(str::to_owned);
                // A typed path that named a file lands here with the file under
                // the cursor. One that named nothing used to land here too, in
                // silence, and looked like the place that was asked for.
                self.error(nothing_there(&name, path, near.as_deref()));
            }
            let keep = self.tabs[self.active].current.hovered_name().map(str::to_owned);
            let filter = self.tabs[self.active].current.filter.clone();
            let cursor = self.tabs[self.active].current.cursor;
            let offset = self.tabs[self.active].current.offset;
            let f = &mut self.tabs[self.active].current;
            f.entries = entries.clone();
            f.state = LoadState::Ready;
            f.scan_id = None;
            f.filter = filter;
            f.cursor = cursor;
            f.offset = offset;
            f.rebuild(show_hidden);
            if let Some(name) = near.or(keep).or(memo) {
                f.select_name(&name);
            }
            // A scan sent before the name was made can answer after it, and
            // is then the one without the name; the rescan sent for it is
            // still on its way, so the landing waits for that one (12.19 lost
            // the cursor to the rename's rescan about one run in a few
            // hundred).
            let waits = self.land_on.as_ref().is_some_and(|p| {
                p.parent() == Some(path) && !entries.iter().any(|e| e.path == *p) && self.inflight.values().any(|q| q == path)
            });
            let land = if waits { None } else { self.land_on.take().filter(|p| p.parent() == Some(path)) };
            if let Some(p) = land {
                let name = util::file_name(&p);
                self.tabs[self.active].memo.insert(path.to_path_buf(), name.clone());
                self.tabs[self.active].current.select_name(&name);
            }
        }

        let parent_hit = self.tabs[self.active].parent.as_ref().map(|p| p.path == path).unwrap_or(false);
        if parent_hit {
            let cwd = self.tabs[self.active].cwd.clone();
            if let Some(f) = self.tabs[self.active].parent.as_mut() {
                f.entries = entries.clone();
                f.state = LoadState::Ready;
                f.scan_id = None;
                f.rebuild(show_hidden);
                let name = util::file_name(&cwd);
                f.select_name(&name);
            }
        }

        if let PreviewState::Dir(f) = &mut self.preview.state {
            if f.path == path {
                f.entries = entries.clone();
                f.state = LoadState::Ready;
                f.scan_id = None;
                f.rebuild(show_hidden);
            }
        }

        // Other tabs share the listing but keep their own cursors. The other
        // pane is one of them, and it is on screen, so it needs the same care
        // over the cursor as the focused tab.
        for (i, tab) in self.tabs.iter_mut().enumerate() {
            if i == self.active || tab.cwd != path {
                continue;
            }
            tab.pending_cd = None;
            let keep = tab.current.hovered_name().map(str::to_owned);
            tab.current.entries = entries.clone();
            tab.current.state = LoadState::Ready;
            tab.current.scan_id = None;
            tab.current.rebuild(tab.show_hidden);
            if let Some(name) = keep.or_else(|| tab.memo.get(path).cloned()) {
                tab.current.select_name(&name);
            }
        }
    }
}
