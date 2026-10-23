//! Junction offers and the undo/redo steps.

use super::*;

impl App {
    /// `-` on a folder without Developer Mode: Windows refused the symlink, and
    /// a junction would do without the privilege. Asked rather than done,
    /// because a junction is not the same thing -- it holds the full path where
    /// `-` would have written a relative one, and cannot reach a network
    /// location -- and swapping one kind for the other in silence would leave
    /// that to be found out later (Q46).
    pub(super) fn offer_junctions(&mut self, links: Vec<ops::Link>) {
        const SHOWN: usize = 5;
        let mut body = vec![
            "Windows would not make the symlink: that needs Developer Mode or administrator.".to_owned(),
            "A junction needs neither. Unlike the symlink it holds the full path, not a relative one,".to_owned(),
            "and it cannot point at a network location.".to_owned(),
            String::new(),
        ];
        body.extend(links.iter().take(SHOWN).map(|l| format!("{}  →  {}", l.at.display(), l.target.display())));
        if links.len() > SHOWN {
            body.push(format!("… and {} more", links.len() - SHOWN));
        }
        // What `c` copies, on the screen it is asked about: the toast that
        // named it was gone by then (#197).
        body.push(String::new());
        body.push(if links.len() == 1 { "`c` copies:".to_owned() } else { "`c` copies, one per link:".to_owned() });
        body.extend(links.iter().take(SHOWN).map(|l| format!("  {}", ops::mklink_line(&l.at, &l.target))));
        let what = if links.len() == 1 { "Make the junction".to_owned() } else { format!("Make {} junctions", links.len()) };
        // `c` for someone who would rather make it themselves, or somewhere
        // else: the line is two absolute paths long, and a toast cannot be
        // copied from (#185, #191; Q56).
        let copy = if links.len() == 1 { "Copy the mklink command" } else { "Copy the mklink commands" };
        self.overlay = Overlay::Confirm(ConfirmOverlay {
            title: "Make a junction instead?".into(),
            body,
            options: vec![('y', what), ('c', copy.into()), ('n', "No".into())],
            action: ConfirmAction::Junctions { links },
            dest: None,
        });
    }

    /// The `y` to [`App::offer_junctions`]: made like any link, so `u` takes
    /// them back and `U` makes them again, as junctions.
    pub(super) fn make_junctions(&mut self, links: Vec<ops::Link>) {
        let (done, left, err) = each_link(links, ops::Link::make);
        if let Some(l) = done.first().or(left.first()) {
            self.refresh_parent(&l.at.clone());
        }
        if !done.is_empty() {
            let said = match done.len() {
                1 => format!("Made a junction {} — u to undo", util::file_name(&done[0].at)),
                n => format!("Made {n} junctions — u to undo"),
            };
            self.undos.land(UndoStep::Link { links: done }, Land::Fresh);
            self.toast(said);
        }
        if let (Some(e), Some(l)) = (err, left.first()) {
            self.error(format!("Junction: {}: {e}", util::file_name(&l.at)));
        }
    }

    /// The junction question's `c`: the `mklink /J` lines, one per link, and
    /// nothing made. Each runs in any shell (see `ops::mklink_line`).
    pub(super) fn copy_mklink(&mut self, links: &[ops::Link]) {
        let lines: Vec<String> = links.iter().map(|l| ops::mklink_line(&l.at, &l.target)).collect();
        match exec::set_clipboard(&lines.join("\n")) {
            Ok(()) => self.toast(match lines.len() {
                1 => "Copied the mklink command — it runs in cmd or PowerShell".to_owned(),
                n => format!("Copied {n} mklink commands — they run in cmd or PowerShell"),
            }),
            Err(e) => self.error(format!("Clipboard: {e}")),
        }
    }

    /// Take back the newest step. A step that will not go back stays on the
    /// stack — the usual reason is something standing where it came from, which
    /// the person can clear before pressing `u` again.
    pub(super) fn undo_step(&mut self) {
        let Some(step) = self.undos.undo.pop() else {
            self.toast("Nothing to undo");
            return;
        };
        match step {
            UndoStep::Rename { from, to } => match self.apply_rename(&to, &from) {
                Ok(()) => {
                    let step = UndoStep::Rename { from, to };
                    self.toast(step.undone_label());
                    self.undos.land(step, Land::Undone);
                }
                Err(e) => {
                    self.error(format!("Undo: {e}"));
                    self.undos.keep(UndoStep::Rename { from, to }, Land::Undone);
                }
            },
            UndoStep::Move { pairs } => {
                let back: Vec<(PathBuf, PathBuf)> = pairs.iter().rev().map(|(a, b)| (b.clone(), a.clone())).collect();
                match self.run_renames(&back) {
                    Ok(()) => {
                        let step = UndoStep::Move { pairs };
                        self.toast(step.undone_label());
                        self.undos.land(step, Land::Undone);
                    }
                    Err(e) => {
                        self.error(format!("Undo: {e}"));
                        self.undos.keep(UndoStep::Move { pairs }, Land::Undone);
                    }
                }
            }
            UndoStep::Bulk { pairs } => {
                let back: Vec<(PathBuf, PathBuf)> = pairs.iter().rev().map(|(a, b)| (b.clone(), a.clone())).collect();
                match self.run_renames(&back) {
                    Ok(()) => {
                        let step = UndoStep::Bulk { pairs };
                        self.toast(step.undone_label());
                        self.undos.land(step, Land::Undone);
                    }
                    Err(e) => {
                        self.error(format!("Undo: {e}"));
                        self.undos.keep(UndoStep::Bulk { pairs }, Land::Undone);
                    }
                }
            }
            UndoStep::Trash { paths, dir } => {
                if !restore::SUPPORTED {
                    self.error(format!("Undo: {}", restore::UNSUPPORTED));
                    self.undos.keep(UndoStep::Trash { paths, dir }, Land::Undone);
                    return;
                }
                let id = self.submit_op(OpKind::Restore, paths.clone(), dir.clone(), true);
                self.record_job(id, UndoStep::Trash { paths, dir }, Land::Undone);
            }
            UndoStep::Create { paths, file } => match unmake(&paths, file) {
                Ok(folders) => {
                    self.refresh_parent(&paths[0]);
                    let step = UndoStep::Create { paths, file };
                    self.toast(removed_label(&step, folders));
                    self.undos.land(step, Land::Undone);
                }
                Err(e) => {
                    self.error(format!("Undo: {e}"));
                    self.undos.keep(UndoStep::Create { paths, file }, Land::Undone);
                }
            },
            UndoStep::Link { links } => self.relink(links, ops::Link::remove, Land::Undone),
        }
    }

    /// Remove or make again a set of links. Each stands alone, so the ones
    /// that went through move to the other stack and the rest stay, named in
    /// the error -- `u` again once the way is clear finishes the job.
    pub(super) fn relink(&mut self, links: Vec<ops::Link>, f: fn(&ops::Link) -> std::io::Result<()>, how: Land) {
        let (done, left, err) = each_link(links, f);
        if let Some(l) = done.first().or(left.first()) {
            self.refresh_parent(&l.at.clone());
        }
        let verb = if how == Land::Undone { "Undo" } else { "Redo" };
        if !done.is_empty() {
            let step = UndoStep::Link { links: done };
            self.toast(match how {
                Land::Undone => step.undone_label(),
                _ => step.redone_label(),
            });
            self.undos.land(step, how);
        }
        if let Some(e) = err {
            self.error(format!("{verb}: {e}"));
            self.undos.keep(UndoStep::Link { links: left }, how);
        }
    }

    /// List again the folder `p` is in, which `u` has just changed.
    pub(super) fn refresh_parent(&mut self, p: &Path) {
        if let Some(dir) = p.parent() {
            let dir = dir.to_path_buf();
            self.cache.remove(&dir);
            self.rescan(&dir);
        }
    }

    /// Do again what `u` took back.
    pub(super) fn redo_step(&mut self) {
        let Some(step) = self.undos.redo.pop() else {
            self.toast("Nothing to redo");
            return;
        };
        match step {
            UndoStep::Rename { from, to } => match self.apply_rename(&from, &to) {
                Ok(()) => {
                    let step = UndoStep::Rename { from, to };
                    self.toast(step.redone_label());
                    self.undos.land(step, Land::Redone);
                }
                Err(e) => {
                    self.error(format!("Redo: {e}"));
                    self.undos.keep(UndoStep::Rename { from, to }, Land::Redone);
                }
            },
            UndoStep::Move { pairs } => match self.run_renames(&pairs) {
                Ok(()) => {
                    let step = UndoStep::Move { pairs };
                    self.toast(step.redone_label());
                    self.undos.land(step, Land::Redone);
                }
                Err(e) => {
                    self.error(format!("Redo: {e}"));
                    self.undos.keep(UndoStep::Move { pairs }, Land::Redone);
                }
            },
            UndoStep::Bulk { pairs } => match self.run_renames(&pairs) {
                Ok(()) => {
                    let step = UndoStep::Bulk { pairs };
                    self.toast(step.redone_label());
                    self.undos.land(step, Land::Redone);
                }
                Err(e) => {
                    self.error(format!("Redo: {e}"));
                    self.undos.keep(UndoStep::Bulk { pairs }, Land::Redone);
                }
            },
            UndoStep::Trash { paths, dir } => {
                let id = self.submit_op(OpKind::Trash, paths.clone(), dir.clone(), true);
                self.record_job(id, UndoStep::Trash { paths, dir }, Land::Redone);
            }
            UndoStep::Create { paths, file } => match remake(&paths, file) {
                Ok(()) => {
                    self.refresh_parent(&paths[0]);
                    let step = UndoStep::Create { paths, file };
                    self.toast(step.redone_label());
                    self.undos.land(step, Land::Redone);
                }
                Err(e) => {
                    self.error(format!("Redo: {e}"));
                    self.undos.keep(UndoStep::Create { paths, file }, Land::Redone);
                }
            },
            UndoStep::Link { links } => self.relink(links, ops::Link::make, Land::Redone),
        }
    }
}
