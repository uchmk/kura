//! The disk-usage view: measuring a folder and draining the sizes.

use super::*;

impl App {
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
}
