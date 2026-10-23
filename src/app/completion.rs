//! Path completion in the input box.

use super::*;

impl App {
    pub(super) fn complete_input(&mut self) {
        let Overlay::Input(ov) = &mut self.overlay else { return };
        // `E`'s field is a new name, so there is nothing to complete: `<Tab>`
        // turns the extension into the next format, the stem untouched (Q79).
        if matches!(ov.kind, InputKind::Compress) {
            ov.text = next_archive_name(&ov.text);
            let stem = util::stem_and_ext(&ov.text).0.chars().count();
            let stem = if ov.text.ends_with(".tar.gz") { stem - 4 } else { stem };
            ov.initial_selection = Some((0, stem));
            ov.focused = false;
            return;
        }
        if !matches!(ov.kind, InputKind::Cd) {
            return;
        }
        if let Some(dir) = cycling_in(&ov.text, &ov.completion, ov.completion_at) {
            let hits = ov.completion.clone();
            self.apply_completion(&dir, hits);
            return;
        }
        let (dir, prefix) = completion_target(&ov.text, &self.tabs[self.active].cwd);
        // A directory that has been listed once answers on the spot, which
        // covers the cwd, its parent and everywhere the tab has been.
        if let Some(entries) = self.cache.get(&dir).cloned() {
            self.pending_completion = None;
            self.apply_completion(&dir, completion_hits(&entries, &prefix));
            return;
        }
        // Everything else goes to the scan pool. `read_dir` on a dead share
        // sits for half a minute, and Tab must not take the window with it.
        let sort = self.tabs[self.active].sort;
        let id = self.scanner.scan_low(dir.clone(), sort);
        self.inflight.insert(id, dir.clone());
        self.pending_completion = Some(PendingCompletion { id, dir, prefix, since: Instant::now() });
    }

    /// Answer a completion whose listing has just arrived. What the user typed
    /// in the meantime wins: an answer to an older question is dropped.
    pub(super) fn complete_from(&mut self, id: u64, path: &Path, entries: &[Entry]) {
        let Some(p) = self.pending_completion.take() else { return };
        if p.id != id || p.dir != path {
            self.pending_completion = Some(p);
            return;
        }
        let Overlay::Input(ov) = &self.overlay else { return };
        if !matches!(ov.kind, InputKind::Cd) {
            return;
        }
        if completion_target(&ov.text, &self.tabs[self.active].cwd) != (p.dir.clone(), p.prefix.clone()) {
            return;
        }
        self.apply_completion(&p.dir, completion_hits(entries, &p.prefix));
    }

    /// Put the next match on the input line. Repeated presses walk the list,
    /// and `completion_at` remembers how far they got.
    pub(super) fn apply_completion(&mut self, dir: &Path, hits: Vec<String>) {
        let Overlay::Input(ov) = &mut self.overlay else { return };
        if hits.is_empty() {
            return;
        }
        let idx = ov.completion_at % hits.len();
        ov.text = completed_text(dir, &hits[idx]);
        ov.completion_at = idx + 1;
        ov.completion = hits;
        ov.initial_selection = {
            let n = ov.text.chars().count();
            Some((n, n))
        };
        ov.focused = false;
    }
}
