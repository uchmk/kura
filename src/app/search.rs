//! Search and filter: starting a search and draining its results.

use super::*;

impl App {
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
