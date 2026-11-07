//! Deliver lazy details without filesystem work in the UI thread.
use super::*;
use crate::fs::entry::Linemode;
use crate::fs::SortBy;
use std::sync::Weak;

#[derive(Default)]
pub(super) struct DetailState {
    worker: Option<crate::fs::details::Worker>,
    generation: u64,
    /// The generation in force when the scope last changed. An answer newer than
    /// this belongs to the current scope even if the window has moved since.
    scope_generation: u64,
    scope: Vec<(usize, PathBuf)>,
    /// The rows of the tabs read a window at a time, as last asked for.
    wanted: Vec<(PathBuf, Option<std::time::SystemTime>)>,
    cache: HashMap<PathBuf, (Option<std::time::SystemTime>, crate::fs::details::Details)>,
    pub pending_sort: Option<(usize, PathBuf, crate::fs::SortSpec)>,
    /// The language every Type sort key in the tabs is in.
    type_lang: String,
    /// Per tab: the entry list (address, length) the Type keys were last
    /// filled in for. An address can come back for a later list once the old
    /// one is freed; such a list is new and its missing keys give it away.
    /// A weak handle would rule the reuse out, but it also moves the list on
    /// every `Arc::make_mut`, and each move would sort the list again.
    type_key: HashMap<usize, (usize, usize)>,
    /// The entry list of the tab being sorted, as last filled and asked for.
    /// The weak handle keeps the allocation, so no later list takes its place.
    all_list: Option<Weak<Vec<Entry>>>,
}

impl DetailState {
    /// Keep an answer that belongs to the current scope, even if the window has
    /// moved on since it was asked for. One older than the last scope change is
    /// another folder's and is dropped.
    fn accept(&mut self, id: u64, entry: Entry, details: crate::fs::details::Details) -> bool {
        if id <= self.scope_generation {
            return false;
        }
        self.cache.insert(entry.path, (entry.modified, details));
        true
    }

    /// Whether `list` is the sorting tab's list as last filled and asked for.
    /// A change through `Arc::make_mut` moves a list with a weak handle on it,
    /// so a list changed in place does not pass either.
    fn same_list(&self, list: &Arc<Vec<Entry>>) -> bool {
        self.all_list.as_ref().is_some_and(|held| std::ptr::eq(held.as_ptr(), Arc::as_ptr(list)))
    }
}

/// The rows to read for a tab: the visible ones (the partly visible last row
/// included), then a page below, then a page above.
fn window_rows(offset: usize, page_rows: usize, len: usize) -> impl Iterator<Item = usize> {
    let vis_end = (offset + page_rows + 1).min(len);
    let below_end = (vis_end + page_rows).min(len);
    (offset.min(len)..vis_end).chain(vis_end..below_end).chain(offset.saturating_sub(page_rows).min(len)..offset.min(len))
}

impl App {
    pub(super) fn pump_details(&mut self) {
        // Type keys exist in one language at a time: a change drops every
        // tab's, shown or not, and the pass below fills in the ones in use.
        if self.detail_state.type_lang != self.lang {
            for tab in &mut self.tabs {
                if tab.current.entries.iter().any(|e| e.sort_type.is_some()) {
                    for e in Arc::make_mut(&mut tab.current.entries) {
                        e.sort_type = None;
                    }
                }
            }
            self.detail_state.type_key.clear();
            self.detail_state.type_lang.clone_from(&self.lang);
        }
        // The keys are filled in when a tab's entry list changed, not on every
        // frame of a huge folder.
        for idx in self.pane_tabs() {
            let tab = &mut self.tabs[idx];
            if tab.sort.by == SortBy::Type {
                let key = (Arc::as_ptr(&tab.current.entries) as usize, tab.current.entries.len());
                let fresh = |e: Option<&crate::fs::Entry>| e.is_some_and(|e| e.sort_type.is_none());
                let unkeyed = fresh(tab.current.entries.first()) || fresh(tab.current.entries.last());
                if unkeyed || self.detail_state.type_key.get(&idx) != Some(&key) {
                    for e in Arc::make_mut(&mut tab.current.entries) {
                        if e.sort_type.is_none() {
                            e.sort_type = Some(crate::fs::details::detail_type(e, &self.lang));
                        }
                    }
                    tab.current.resort(&tab.sort, tab.show_hidden || self.usage_linemode.is_some());
                    let key = (Arc::as_ptr(&tab.current.entries) as usize, tab.current.entries.len());
                    self.detail_state.type_key.insert(idx, key);
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
            self.detail_state.scope_generation = self.detail_state.generation;
            self.detail_state.cache.clear();
            self.detail_state.scope = scope.clone();
            self.detail_state.wanted.clear();
            self.detail_state.all_list = None;
        }
        // The tab being sorted is read in full, the others a window at a time.
        // Its request is rebuilt when its listing changes, not when answers
        // land in it.
        let sorting = self.detail_state.pending_sort.as_ref().map(|(i, _, _)| *i).filter(|i| scope.iter().any(|(s, _)| s == i));
        let relist = match sorting {
            Some(i) => !self.detail_state.same_list(&self.tabs[i].current.entries),
            None => self.detail_state.all_list.is_some(),
        };
        let mut arrived = false;
        let answers: Vec<_> = self.detail_state.worker.as_ref().map(|w| w.rx.try_iter().collect()).unwrap_or_default();
        for (id, entry, details) in answers {
            arrived |= self.detail_state.accept(id, entry, details);
        }
        for (idx, _) in &scope {
            if self.detail_state.cache.is_empty() {
                break;
            }
            let tab = &mut self.tabs[*idx];
            let cache = &self.detail_state.cache;
            let hit = |e: &Entry| cache.get(e.path.as_path()).filter(|(m, _)| *m == e.modified).map(|(_, d)| d);
            if sorting == Some(*idx) {
                if (arrived || relist) && tab.current.entries.iter().any(|e| e.details.is_none() && hit(e).is_some()) {
                    for e in Arc::make_mut(&mut tab.current.entries) {
                        if e.details.is_none() {
                            e.details = hit(e).cloned();
                        }
                    }
                }
            } else {
                let folder = &mut tab.current;
                let rows: Vec<usize> =
                    window_rows(folder.offset, tab.page_rows, folder.view.len()).map(|r| folder.view[r] as usize).collect();
                if rows.iter().any(|&i| folder.entries[i].details.is_none() && hit(&folder.entries[i]).is_some()) {
                    let entries = Arc::make_mut(&mut folder.entries);
                    for i in rows {
                        if entries[i].details.is_none() {
                            entries[i].details = hit(&entries[i]).cloned();
                        }
                    }
                }
            }
        }
        let mut wanted = Vec::new();
        for (idx, _) in &scope {
            if sorting == Some(*idx) {
                continue;
            }
            let tab = &self.tabs[*idx];
            for row in window_rows(tab.current.offset, tab.page_rows, tab.current.view.len()) {
                if let Some(e) = tab.current.at(row) {
                    wanted.push((e.path.clone(), e.modified));
                }
            }
        }
        if relist || wanted != self.detail_state.wanted {
            self.detail_state.wanted = wanted;
            let mut unread = Vec::new();
            for (idx, _) in &scope {
                let tab = &self.tabs[*idx];
                if sorting == Some(*idx) {
                    unread.extend(tab.current.entries.iter().filter(|e| e.details.is_none()).cloned());
                } else {
                    let rows = window_rows(tab.current.offset, tab.page_rows, tab.current.view.len());
                    unread.extend(rows.filter_map(|row| tab.current.at(row)).filter(|e| e.details.is_none()).cloned());
                }
            }
            if !unread.is_empty() || self.detail_state.worker.is_some() {
                let worker = self.detail_state.worker.get_or_insert_with(|| crate::fs::details::Worker::new(self.ctx.clone()));
                self.detail_state.generation = worker.request(unread);
            }
        }
        self.detail_state.all_list = sorting.map(|i| Arc::downgrade(&self.tabs[i].current.entries));
        self.finish_pending_sort();
    }

    fn finish_pending_sort(&mut self) {
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
            self.detail_state.all_list = None;
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

    fn wait_for(app: &mut App, row: usize) {
        for _ in 0..1000 {
            app.pump_details();
            if app.tabs[0].current.at(row).unwrap().details.is_some() {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        panic!("row {row} never got its details");
    }

    #[test]
    fn rows_near_the_window_are_read_and_leaving_discards_requests() {
        let dir = crate::util::test_dir("details-visible");
        for i in 0..20 {
            std::fs::write(dir.join(format!("f{i:02}.txt")), "text").unwrap();
        }
        let mut screen = Screen::without_state(dir);
        screen.settle();
        let app = &mut screen.app;
        app.pump_details();
        assert!(app.detail_state.worker.is_none());
        app.tabs[0].linemode = Linemode::All;
        app.tabs[0].page_rows = 2;
        app.tabs[0].current.offset = 8;
        app.pump_details();
        // Rows 8..11 are visible (the last one only partly), 11..13 a page below, 6..8 a page above.
        assert_eq!(app.detail_state.wanted.len(), 7);
        for row in [8, 9, 10, 11, 12, 6, 7] {
            wait_for(app, row);
        }
        for row in [8, 9, 10, 11, 12, 6, 7] {
            assert!(app.tabs[0].current.at(row).unwrap().details.is_some(), "row {row}");
        }
        for row in [0, 5, 13, 19] {
            assert!(app.tabs[0].current.at(row).unwrap().details.is_none(), "row {row} is outside the window");
        }
        let generation = app.detail_state.generation;
        app.tabs[0].linemode = Linemode::None;
        app.pump_details();
        assert!(app.detail_state.generation > generation);
        assert!(app.detail_state.cache.is_empty());
        assert!(app.detail_state.wanted.is_empty());
    }

    #[test]
    fn a_sort_waiting_on_details_asks_once_while_answers_land_and_the_other_pane_keeps_its_window() {
        let dir = crate::util::test_dir("details-pending-sort");
        for i in 0..20 {
            std::fs::write(dir.join(format!("f{i:02}.txt")), "text").unwrap();
        }
        let mut screen = Screen::without_state(dir);
        screen.settle();
        let app = &mut screen.app;
        app.tabs[0].linemode = Linemode::All;
        assert!(app.open_split());
        let other = app.other_pane().expect("a second pane");
        assert_eq!(app.tabs[other].current.view.len(), 20, "the second pane starts from the listing in hand");
        app.tabs[other].page_rows = 2;
        app.tabs[other].current.offset = 0;
        let before = app.detail_state.generation;
        app.sort(Some(SortBy::Owner), None, None);
        assert!(app.detail_state.pending_sort.is_some());
        let generation = app.detail_state.generation;
        assert!(generation > before, "the sort asks for the whole folder");
        for _ in 0..1000 {
            app.pump_details();
            if app.detail_state.pending_sort.is_none() {
                break;
            }
            // Answers landing in the list being sorted do not send it again.
            assert_eq!(app.detail_state.generation, generation);
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert!(app.detail_state.pending_sort.is_none(), "the sort never finished");
        assert!(app.tabs[app.active].current.entries.iter().all(|e| e.details.is_some()));
        // The other pane reads its window (rows 0..5 at two rows a page) and
        // nothing past it.
        let window_read = |app: &App| (0..5).all(|row| app.tabs[other].current.at(row).unwrap().details.is_some());
        for _ in 0..1000 {
            app.pump_details();
            if window_read(app) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert!(window_read(app), "the other pane's window was never read");
        for row in 5..20 {
            assert!(app.tabs[other].current.at(row).unwrap().details.is_none(), "row {row} is outside the other pane's window");
        }
    }

    #[test]
    fn an_older_answer_in_the_same_scope_is_kept_and_one_from_before_a_scope_change_is_not() {
        let dir = crate::util::test_dir("details-generation");
        for name in ["a.txt", "b.txt", "c.txt"] {
            std::fs::write(dir.join(name), "text").unwrap();
        }
        let mut screen = Screen::without_state(dir);
        screen.settle();
        let app = &mut screen.app;
        app.tabs[0].linemode = Linemode::All;
        app.tabs[0].page_rows = 1;
        app.pump_details();
        let scope_generation = app.detail_state.scope_generation;
        let worker = app.detail_state.worker.as_ref().expect("a worker");
        let older = worker.request(Vec::new());
        app.detail_state.generation = worker.request(Vec::new());
        assert!(older > scope_generation && app.detail_state.generation > older);
        let answer = |owner: &str| crate::fs::details::Details { owner: owner.into(), dimensions: None };
        let cached = |app: &App, row: usize| {
            let path = &app.tabs[0].current.at(row).unwrap().path;
            app.detail_state.cache.get(path).map(|(_, d)| d.owner.clone())
        };

        // Asked for before the window moved, answered after: still the right data.
        let kept = app.tabs[0].current.at(1).unwrap().clone();
        assert!(app.detail_state.accept(older, kept, answer("kept")));
        assert_eq!(cached(app, 1).as_deref(), Some("kept"));
        // Asked for before the scope changed: dropped.
        let dropped = app.tabs[0].current.at(2).unwrap().clone();
        assert!(!app.detail_state.accept(scope_generation, dropped, answer("dropped")));
        assert_eq!(cached(app, 2), None);
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
        app.detail_state.cache.insert(entry.path.clone(), (None, crate::fs::details::Details { owner: "stale".into(), dimensions: None }));
        // A stale cached read cannot fill a replacement with a different mtime.
        assert!(entry.modified.is_some());
        app.pump_details();
        assert_ne!(app.tabs[0].current.at(0).unwrap().details.as_ref().map(|d| d.owner.as_str()), Some("stale"));
        app.tabs[0].current.path = app.tabs[0].current.path.join("other");
        app.pump_details();
        assert!(!app.detail_state.cache.contains_key(&entry.path));
    }
}
