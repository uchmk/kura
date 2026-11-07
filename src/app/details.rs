//! Deliver lazy details without filesystem work in the UI thread.
use super::*;
use crate::fs::entry::Linemode;
use crate::fs::SortBy;

#[derive(Default)]
pub(super) struct DetailState {
    worker: Option<crate::fs::details::Worker>,
    generation: u64,
    scope: Vec<(usize, PathBuf)>,
    wanted: Vec<(PathBuf, Option<std::time::SystemTime>)>,
    cache: HashMap<(PathBuf, Option<std::time::SystemTime>), crate::fs::details::Details>,
    pub pending_sort: Option<(usize, PathBuf, crate::fs::SortSpec)>,
}

impl App {
    pub(super) fn pump_details(&mut self) {
        for idx in self.pane_tabs() {
            let tab = &mut self.tabs[idx];
            if tab.sort.by == SortBy::Type {
                let changed = tab
                    .current
                    .entries
                    .iter()
                    .any(|e| e.sort_type.as_deref() != Some(crate::fs::details::detail_type(e, &self.lang).as_str()));
                if changed {
                    for e in Arc::make_mut(&mut tab.current.entries) {
                        e.sort_type = Some(crate::fs::details::detail_type(e, &self.lang));
                    }
                    tab.current.resort(&tab.sort, tab.show_hidden || self.usage_linemode.is_some());
                }
            }
        }
        // Folder and tab changes discard outstanding sorts and worker answers.
        if self
            .detail_state
            .pending_sort
            .as_ref()
            .is_some_and(|(idx, path, sort)| *idx != self.active || self.tabs[*idx].current.path != *path || self.tabs[*idx].sort != *sort)
        {
            self.detail_state.pending_sort = None;
        }
        let tab = &self.tabs[self.active];
        if self.detail_state.pending_sort.is_none()
            && matches!(tab.sort.by, SortBy::Owner | SortBy::Dimensions)
            && tab.current.entries.iter().any(|e| e.details.is_none())
        {
            self.detail_state.pending_sort = Some((self.active, tab.current.path.clone(), tab.sort));
        }
        let scope: Vec<_> = self
            .pane_tabs()
            .into_iter()
            .filter(|&idx| {
                matches!(self.tabs[idx].linemode, Linemode::All | Linemode::Owner)
                    || self.detail_state.pending_sort.as_ref().is_some_and(|(i, _, _)| *i == idx)
            })
            .map(|idx| (idx, self.tabs[idx].current.path.clone()))
            .collect();
        if scope != self.detail_state.scope {
            if let Some(worker) = &self.detail_state.worker {
                self.detail_state.generation = worker.request(Vec::new());
            }
            self.detail_state.cache.clear();
            self.detail_state.scope = scope.clone();
            self.detail_state.wanted.clear();
        }
        if let Some(worker) = &self.detail_state.worker {
            while let Ok((id, entry, details)) = worker.rx.try_recv() {
                if id != self.detail_state.generation {
                    continue;
                }
                self.detail_state.cache.insert((entry.path, entry.modified), details);
            }
        }
        for (idx, _) in &scope {
            if self.detail_state.cache.is_empty() {
                break;
            }
            let folder = &mut self.tabs[*idx].current;
            let cached = |e: &Entry| self.detail_state.cache.get(&(e.path.clone(), e.modified));
            if folder.entries.iter().any(|e| e.details.is_none() && cached(e).is_some()) {
                for e in Arc::make_mut(&mut folder.entries) {
                    if e.details.is_none() {
                        e.details = cached(e).cloned();
                    }
                }
            }
        }
        let mut wanted = Vec::new();
        let mut unread = Vec::new();
        for (idx, _) in &scope {
            let tab = &self.tabs[*idx];
            let all = self.detail_state.pending_sort.as_ref().is_some_and(|(i, _, _)| i == idx);
            if all {
                for e in tab.current.entries.iter() {
                    wanted.push((e.path.clone(), e.modified));
                    if e.details.is_none() {
                        unread.push(e.clone());
                    }
                }
            } else {
                for row in tab.current.offset..(tab.current.offset + tab.page_rows).min(tab.current.view.len()) {
                    if let Some(e) = tab.current.at(row) {
                        wanted.push((e.path.clone(), e.modified));
                        if e.details.is_none() {
                            unread.push(e.clone());
                        }
                    }
                }
            }
        }
        if wanted != self.detail_state.wanted {
            self.detail_state.wanted = wanted;
            if !unread.is_empty() || self.detail_state.worker.is_some() {
                let worker = self.detail_state.worker.get_or_insert_with(|| crate::fs::details::Worker::new(self.ctx.clone()));
                self.detail_state.generation = worker.request(unread);
            }
        }
        if let Some((idx, _, sort)) = self.detail_state.pending_sort {
            let tab = &mut self.tabs[idx];
            if tab.current.state == LoadState::Ready && tab.current.entries.iter().all(|e| e.details.is_some()) {
                let show = tab.show_hidden || self.usage_linemode.is_some();
                tab.current.resort(&sort, show);
                self.detail_state.pending_sort = None;
            }
        }
    }

    pub(super) fn defer_detail_sort(&mut self) -> bool {
        let tab = &self.tabs[self.active];
        self.detail_state.pending_sort = None;
        if matches!(tab.sort.by, SortBy::Owner | SortBy::Dimensions)
            && (tab.current.state == LoadState::Loading || tab.current.entries.iter().any(|e| e.details.is_none()))
        {
            self.detail_state.pending_sort = Some((self.active, tab.current.path.clone(), tab.sort));
            self.detail_state.wanted.clear();
            self.pump_details();
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::harness::Screen;

    #[test]
    fn only_visible_rows_are_read_and_leaving_discards_requests() {
        let dir = crate::util::test_dir("details-visible");
        for name in ["a.txt", "b.txt", "c.txt"] {
            std::fs::write(dir.join(name), "text").unwrap();
        }
        let mut screen = Screen::without_state(dir);
        screen.settle();
        let app = &mut screen.app;
        app.pump_details();
        assert!(app.detail_state.worker.is_none());
        app.tabs[0].linemode = Linemode::All;
        app.tabs[0].page_rows = 1;
        app.pump_details();
        assert_eq!(app.detail_state.wanted.len(), 1);
        for _ in 0..200 {
            app.pump_details();
            if app.tabs[0].current.at(0).unwrap().details.is_some() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert!(app.tabs[0].current.at(0).unwrap().details.is_some());
        assert!(app.tabs[0].current.at(1).unwrap().details.is_none());
        assert!(app.tabs[0].current.at(2).unwrap().details.is_none());
        let generation = app.detail_state.generation;
        app.tabs[0].linemode = Linemode::None;
        app.pump_details();
        assert!(app.detail_state.generation > generation);
        assert!(app.detail_state.cache.is_empty());
        assert!(app.detail_state.wanted.is_empty());
    }

    #[test]
    fn cache_requires_mtime_and_directory_changes_discard_it() {
        let dir = crate::util::test_dir("details-cache");
        std::fs::write(dir.join("file"), "text").unwrap();
        let mut screen = Screen::without_state(dir);
        screen.settle();
        let app = &mut screen.app;
        app.tabs[0].linemode = Linemode::All;
        app.pump_details();
        let entry = app.tabs[0].current.at(0).unwrap().clone();
        app.detail_state.cache.insert((entry.path.clone(), None), crate::fs::details::Details { owner: "stale".into(), dimensions: None });
        // A stale cached read cannot fill a replacement with a different mtime.
        assert!(entry.modified.is_some());
        app.pump_details();
        assert_ne!(app.tabs[0].current.at(0).unwrap().details.as_ref().map(|d| d.owner.as_str()), Some("stale"));
        app.tabs[0].current.path = app.tabs[0].current.path.join("other");
        app.pump_details();
        assert!(!app.detail_state.cache.contains_key(&(entry.path, None)));
    }
}
