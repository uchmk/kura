//! Creating and renaming: the input results, bulk rename and the rename runner.

use super::*;

impl App {
    pub(super) fn do_create(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        let base = self.tabs[self.active].cwd.clone();
        let as_dir = text.ends_with('/') || text.ends_with('\\');
        let target = util::resolve_against(&base, text.trim_end_matches(['/', '\\']));
        let made = paths_to_make(&target);
        let res = if as_dir {
            std::fs::create_dir_all(&target)
        } else {
            if let Some(parent) = target.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            std::fs::OpenOptions::new().write(true).create_new(true).open(&target).map(|_| ())
        };
        match res {
            Ok(()) => {
                // The cursor goes to what was made, as yazi's does (#250): to
                // `new` for `a new/deep/note.txt`, the part of it in this folder.
                // `land_on`, not the memo: the row the cursor was on when the
                // listing comes back would win over a memo.
                let here = target.strip_prefix(&base).ok().and_then(|r| r.components().next()).map(|c| base.join(c));
                self.land_on = here.or(Some(target.clone()));
                self.cache.remove(&base);
                self.rescan(&base);
                if !made.is_empty() {
                    self.undos.land(UndoStep::Create { paths: made, file: !as_dir }, Land::Fresh);
                }
            }
            Err(e) => self.error(format!("Create failed: {e}")),
        }
    }

    pub(super) fn do_rename(&mut self, from: &Path, text: &str) {
        if text.is_empty() {
            return;
        }
        let base = self.tabs[self.active].cwd.clone();
        let to = util::resolve_against(&base, text);
        if to == from {
            return;
        }
        match self.apply_rename(from, &to) {
            // Said like `d` and `-` say theirs: a rename was the one change
            // that went by in silence, though its undo is the one people use
            // most (#225).
            Ok(()) => {
                // Onto the new name, as after `a` (#250).
                self.land_on = Some(to.clone());
                let step = UndoStep::Rename { from: from.to_path_buf(), to };
                self.toast(format!("{} — u to undo", step.redone_label()));
                self.undos.land(step, Land::Fresh);
            }
            Err(e) => self.error(format!("Rename failed: {e}")),
        }
    }

    /// Ask for a rule to rename everything selected by. The prompt starts on
    /// the rule that changes nothing, so the preview below it opens showing the
    /// names as they are and the person edits from there.
    pub(super) fn start_bulk_rename(&mut self) {
        let paths = self.tabs[self.active].targets();
        if paths.is_empty() {
            return;
        }
        self.overlay = Overlay::Input(InputOverlay {
            kind: InputKind::Bulk { paths },
            title: "Bulk rename".into(),
            text: "{name}{ext}".into(),
            initial_selection: Some((0, "{name}{ext}".chars().count())),
            focused: false,
            completion: Vec::new(),
            completion_at: 0,
        });
    }

    pub(super) fn do_bulk_rename(&mut self, paths: &[PathBuf], text: &str) {
        let rows = match self.bulk_preview(paths, text) {
            Ok(rows) => rows,
            Err(e) => return self.error(format!("Bulk rename: {e}")),
        };
        // The preview already said which rows are wrong; renaming the rest and
        // leaving the batch half applied would be worse than doing nothing.
        if let Some(bad) = rows.iter().find(|r| r.problem.is_some()) {
            let why = bad.problem.as_deref().unwrap_or_default();
            return self.error(format!("{}: {why}", util::file_name(&bad.from)));
        }
        let pairs: Vec<(PathBuf, PathBuf)> =
            rows.iter().filter(|r| r.to != util::file_name(&r.from)).map(|r| (r.from.clone(), r.from.with_file_name(&r.to))).collect();
        if pairs.is_empty() {
            return self.toast("Bulk rename: nothing to change");
        }
        let done = pairs.len();
        match self.run_renames(&pairs) {
            Ok(()) => {
                self.tabs[self.active].clear_selection();
                self.undos.land(UndoStep::Bulk { pairs }, Land::Fresh);
                self.toast(format!("Renamed {done} file(s)"));
            }
            Err(e) => self.error(format!("Bulk rename: {e}")),
        }
    }

    /// Carry out a batch of renames, in an order that works even when two files
    /// swap names. Stops at the first failure and says so: what has already
    /// moved keeps its new name, which is at least a state the listing shows
    /// honestly.
    pub(super) fn run_renames(&mut self, pairs: &[(PathBuf, PathBuf)]) -> Result<(), String> {
        let names: Vec<(String, String)> = pairs.iter().map(|(a, b)| (util::file_name(a), util::file_name(b))).collect();
        // Where each file is right now; parking moves one aside for a moment.
        let mut at: Vec<PathBuf> = pairs.iter().map(|(a, _)| a.clone()).collect();

        for step in rename::order(&names) {
            let (i, to) = match step {
                rename::Step::Park(i) => {
                    let dir = pairs[i].0.parent().unwrap_or(Path::new(""));
                    (i, rename::park_name(dir, i))
                }
                rename::Step::Rename(i) => (i, pairs[i].1.clone()),
            };
            self.apply_rename(&at[i].clone(), &to)?;
            at[i] = to;
        }
        Ok(())
    }

    /// Move `from` onto `to` and leave the cursor there. Undo and redo walk the
    /// same rename backwards and forwards, so this takes two paths rather than
    /// the name typed at the prompt.
    pub(super) fn apply_rename(&mut self, from: &Path, to: &Path) -> Result<(), String> {
        if ops::exists(to) {
            return Err(format!("Already exists: {}", util::file_name(to)));
        }
        if let Some(parent) = to.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        std::fs::rename(from, to).map_err(|e| e.to_string())?;
        // `r` takes a path, so a rename can cross directories and leave a
        // listing stale at either end.
        for dir in [from.parent(), to.parent()].into_iter().flatten() {
            let dir = dir.to_path_buf();
            self.cache.remove(&dir);
            self.rescan(&dir);
        }
        if let Some(dir) = to.parent() {
            let name = util::file_name(to);
            self.tabs[self.active].memo.insert(dir.to_path_buf(), name);
        }
        Ok(())
    }
}
