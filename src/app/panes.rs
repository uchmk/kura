//! Tabs and the split panes.

use super::*;

impl App {
    /// Open a tab on `path`, on the hovered directory, or on home.
    ///
    /// Nothing checks the target first, for the reason `cd_inner` gives: the
    /// tab opens where it was asked to and the background scan has the last
    /// word. A listing that never arrives sends the new tab back to the
    /// directory it was opened from, rather than closing it.
    pub(super) fn create_tab(&mut self, current: bool, path: Option<String>) {
        if self.tabs.len() >= MAX_TABS {
            self.error("Maximum number of tabs reached");
            return;
        }
        let base = self.tabs[self.active].cwd.clone();
        let (target, fallback) =
            new_tab_target(&base, current, path.as_deref(), self.tabs[self.active].current.hovered(), ito_common::home_dir());
        let tab =
            Tab::new(target.clone(), self.tabs[self.active].sort, self.tabs[self.active].show_hidden, self.tabs[self.active].linemode);
        let at = self.active + 1;
        self.tabs.insert(at, tab);
        if let Some(sp) = self.split {
            // The other pane keeps its tab, which the insert may have moved.
            if sp.other >= at {
                self.split = Some(Split { other: sp.other + 1, ..sp });
            }
        }
        self.switch_tab(at);
        // `switch_tab` fills the tab from the cache when the directory has been
        // listed before; one that is still loading has yet to prove it exists.
        if target != base && self.tabs[at].current.state == LoadState::Loading {
            self.tabs[at].pending_cd = Some(PendingCd { from: base, pushed: false, fallback, reveal: None });
        }
    }

    pub(super) fn close_tab(&mut self, idx: usize) {
        if self.tabs.len() <= 1 {
            self.quit = true;
            return;
        }
        if idx >= self.tabs.len() {
            return;
        }
        self.tabs.remove(idx);
        if let Some(sp) = self.split {
            self.split = split_after_remove(sp.other, idx).map(|other| Split { other, ..sp });
        }
        self.switch_tab(self.active.min(self.tabs.len() - 1));
        self.check_split();
    }

    /// Open the second pane. The current tab keeps the keys on the left; the
    /// right pane takes the next tab, or a fresh view of the same directory
    /// when this is the only tab.
    pub(super) fn open_split(&mut self) -> bool {
        if self.split.is_some() {
            return true;
        }
        let other = if self.tabs.len() > 1 {
            (self.active + 1) % self.tabs.len()
        } else if self.tabs.len() >= MAX_TABS {
            self.error("Maximum number of tabs reached");
            return false;
        } else {
            let src = &self.tabs[self.active];
            let name = src.current.hovered_name().map(str::to_owned);
            let mut tab = Tab::new(src.cwd.clone(), src.sort, src.show_hidden, src.linemode);
            // The listing is already in hand, so the new pane starts filled.
            if let Some(entries) = self.cache.get(&tab.cwd).cloned() {
                let show = tab.show_hidden;
                tab.current = Folder::from_entries(tab.cwd.clone(), entries, show);
                if let Some(name) = name {
                    tab.current.select_name(&name);
                }
            }
            let at = self.active + 1;
            self.tabs.insert(at, tab);
            at
        };
        self.split = Some(Split { other, right: false });
        self.kick_scans();
        true
    }

    /// Close the second pane. Its tab stays open, just no longer on screen.
    pub(super) fn close_split(&mut self) {
        self.split = None;
    }

    /// Give the keys to the pane showing `idx`, which must be the other pane.
    pub fn focus_pane(&mut self, idx: usize) {
        let Some(sp) = self.split else { return };
        if sp.other != idx || idx == self.active || idx >= self.tabs.len() {
            return;
        }
        self.split = Some(Split { other: self.active, right: !sp.right });
        self.switch_tab(idx);
    }

    /// Drop the split once its two tabs stop being a pair.
    pub(super) fn check_split(&mut self) {
        if let Some(sp) = self.split {
            if sp.other >= self.tabs.len() || sp.other == self.active {
                self.split = None;
            }
        }
    }

    pub(super) fn switch_tab(&mut self, idx: usize) {
        if idx >= self.tabs.len() || idx == self.active {
            self.active = idx.min(self.tabs.len() - 1);
        } else {
            self.active = idx;
        }
        let show_hidden = self.tabs[self.active].show_hidden;
        let cwd = self.tabs[self.active].cwd.clone();
        if let Some(entries) = self.cache.get(&cwd).cloned() {
            if self.tabs[self.active].current.state == LoadState::Loading {
                let mut f = Folder::from_entries(cwd.clone(), entries, show_hidden);
                if let Some(name) = self.tabs[self.active].memo.get(&cwd) {
                    f.select_name(name);
                }
                self.tabs[self.active].current = f;
            }
        }
        if let Some(p) = cwd.parent().map(Path::to_path_buf) {
            let needs = self.tabs[self.active].parent.as_ref().map(|f| f.state == LoadState::Loading).unwrap_or(true);
            if needs {
                if let Some(entries) = self.cache.get(&p).cloned() {
                    let mut f = Folder::from_entries(p, entries, show_hidden);
                    f.select_name(&util::file_name(&cwd));
                    self.tabs[self.active].parent = Some(f);
                }
            }
        }
        self.preview.state = PreviewState::Empty;
        self.preview.key = None;
        self.preview.texture = None;
        self.kick_scans();
    }
}
