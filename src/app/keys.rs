//! Key input: what a key press reaches.

use super::*;

impl App {
    /// Whether `k`, pressed now, reaches a binding (all of one, or its start).
    pub fn binds(&self, k: &Key) -> bool {
        let mut seq = self.pending.clone();
        seq.push(*k);
        let in_preview = self.preview_layer_active() && !matches!(keymap::resolve(&self.cfg.keymap.preview, &seq), keymap::Match::None);
        in_preview || !matches!(keymap::resolve(&self.cfg.keymap.mgr, &seq), keymap::Match::None)
    }

    /// Whether the `[preview]` layer is tried before `[mgr]`: the preview is
    /// maximized by `T`. Quick look (`<F3>`) keeps the list's keys, the list
    /// being in view there.
    pub fn preview_layer_active(&self) -> bool {
        self.max_preview && !self.quick
    }

    pub fn feed_key(&mut self, k: Key) {
        self.note_key(&k);
        if let Some(_op) = self.pending_bookmark {
            match k.code {
                Code::Char(c) if k.is_bare_char() => {
                    self.bookmark_key(c);
                    return;
                }
                _ => {
                    self.pending_bookmark = None;
                    return;
                }
            }
        }

        self.pending.push(k);
        // `[preview]` first while it is active; what it does not bind is `[mgr]`'s.
        let bindings =
            if self.preview_layer_active() && !matches!(keymap::resolve(&self.cfg.keymap.preview, &self.pending), keymap::Match::None) {
                &self.cfg.keymap.preview
            } else {
                &self.cfg.keymap.mgr
            };
        match keymap::resolve(bindings, &self.pending) {
            keymap::Match::Exact(b) => {
                let acts = b.run.clone();
                self.pending.clear();
                self.which.clear();
                self.run(&acts);
            }
            keymap::Match::Pending(cands) => {
                let depth = self.pending.len();
                self.which = cands
                    .iter()
                    .filter(|b| b.on.len() > depth)
                    .map(|b| (crate::config::keys::display_seq(&b.on[depth..]), b.desc.clone(), b.raw.clone()))
                    .collect();
            }
            keymap::Match::None => {
                self.pending.clear();
                self.which.clear();
            }
        }
    }

    /// Rows currently visible in one pane's file list; set by the renderer.
    pub fn set_page_rows(&mut self, idx: usize, rows: usize) {
        if let Some(tab) = self.tabs.get_mut(idx) {
            tab.page_rows = rows;
        }
    }
}
