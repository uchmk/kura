//! File operations: copy, move, delete, create, rename, links, extraction.

use super::*;

impl App {
    pub(super) fn yank(&mut self, cut: bool) {
        let paths = self.tabs[self.active].targets();
        if paths.is_empty() {
            return;
        }
        let n = paths.len();
        let from_archive = self.archive_view.is_some();
        self.yank = Yank { paths, cut, from_archive };
        let how = match (cut, from_archive) {
            (true, _) => " (cut)",
            (false, true) => " from the archive — p in a folder takes them out",
            (false, false) => "",
        };
        self.toast(format!("Yanked {}{how}", util::items(n)));
    }

    pub(super) fn paste(&mut self, force: bool, _follow: bool) {
        if self.yank.paths.is_empty() {
            self.error("Nothing to paste");
            return;
        }
        let dest = self.tabs[self.active].cwd.clone();
        // Members of an archive are unpacked rather than copied; the register
        // stays, as a copy's does.
        if self.yank.from_archive {
            let srcs = self.yank.paths.clone();
            self.submit_op(OpKind::TakeOut, srcs, dest, force);
            return;
        }
        let kind = if self.yank.cut { OpKind::Move } else { OpKind::Copy };
        let mut srcs = self.yank.paths.clone();
        // A cut pasted where it already is would be renamed `same_1.txt`
        // without a word (#238); `<A-c>` and a drop refuse the same folder,
        // and so does this. What is cut elsewhere still moves.
        if self.yank.cut {
            srcs.retain(|p| p.parent() != Some(dest.as_path()));
            if srcs.is_empty() {
                self.toast("Already here — the cut is still there");
                return;
            }
        }
        let id = self.submit_op(kind, srcs, dest, force);
        if self.yank.cut {
            self.cut_jobs.insert(id, std::mem::take(&mut self.yank.paths));
            self.yank.cut = false;
        }
    }

    /// Finish a drag. `onto` is the tab the pointer was over when it was let
    /// go, and `cut` is whether the move modifier was held.
    ///
    /// A drop needs somewhere to land, so it does nothing unless the pointer
    /// ended over a different pane: dropping a file back where it came from
    /// should be the no-op it looks like.
    pub fn drop_drag(&mut self, onto: Option<usize>, cut: bool) {
        let Some(drag) = self.drag.take() else { return };
        let Some(onto) = onto.filter(|&i| i != drag.from && i < self.tabs.len()) else { return };
        let dest = self.tabs[onto].cwd.clone();
        // Said, as `<A-c>` says it: let go over a list that showed the same
        // folder, the drop did nothing and nothing told you why (#208).
        if dest == self.tabs[drag.from].cwd {
            self.error("Both panes are in the same directory");
            return;
        }
        let kind = if cut { OpKind::Move } else { OpKind::Copy };
        self.submit_op(kind, drag.paths, dest, false);
        self.tabs[drag.from].clear_selection();
    }

    /// Start a drag on `row` in `tab`. The selection travels when the row is
    /// part of it; otherwise it is that one file, the way a drag usually works.
    pub fn start_drag(&mut self, tab: usize, row: usize) {
        let Some(entry) = self.tabs[tab].current.at(row).cloned() else { return };
        let selected = self.tabs[tab].selected.contains(&entry.path);
        let paths: Vec<PathBuf> = match selected {
            true => self.tabs[tab].selected.iter().cloned().collect(),
            false => vec![entry.path.clone()],
        };
        let label = match paths.len() {
            1 => entry.name.clone(),
            n => format!("{n} items"),
        };
        self.drag = Some(Drag { from: tab, paths, label });
    }

    /// Copy or move the selection into the other pane in one keypress.
    ///
    /// The same job a yank and a paste would raise, with the other pane's
    /// directory as the destination — the point being that the destination is
    /// already on screen, so naming it again is the step worth removing. The
    /// yank register is left alone: this is not a yank.
    pub(super) fn send_to_pane(&mut self, cut: bool) {
        let Some(other) = self.other_pane().filter(|&i| i < self.tabs.len()) else {
            self.error("Open the second pane first (<C-w>)");
            return;
        };
        let srcs = self.tabs[self.active].targets();
        if srcs.is_empty() {
            return;
        }
        let dest = self.tabs[other].cwd.clone();
        // Into the directory it is already in: the copy would land beside
        // itself under another name, which is never what this key meant.
        if dest == self.tabs[self.active].cwd {
            self.error("Both panes are in the same directory");
            return;
        }
        let kind = if cut { OpKind::Move } else { OpKind::Copy };
        self.submit_op(kind, srcs, dest, false);
        self.tabs[self.active].clear_selection();
    }

    pub(super) fn link(&mut self, kind: OpKind) {
        if self.yank.paths.is_empty() {
            self.error("Nothing to link; yank something first");
            return;
        }
        let dest = self.tabs[self.active].cwd.clone();
        let srcs = self.yank.paths.clone();
        self.submit_op(kind, srcs, dest, false);
    }

    pub(super) fn remove(&mut self, permanently: bool, force: bool, hovered: bool) {
        let paths = if hovered {
            self.tabs[self.active].current.hovered().map(|e| vec![e.path.clone()]).unwrap_or_default()
        } else {
            self.tabs[self.active].targets()
        };
        if paths.is_empty() {
            return;
        }
        if permanently && !force {
            self.overlay = Overlay::Confirm(ConfirmOverlay {
                title: "Delete permanently?".into(),
                body: preview_paths(&paths),
                options: vec![('y', "Delete".into()), ('n', "Cancel".into())],
                action: ConfirmAction::DeleteForever { paths },
                dest: None,
            });
            return;
        }
        let kind = if permanently { OpKind::Delete } else { OpKind::Trash };
        let dest = self.tabs[self.active].cwd.clone();
        let id = self.submit_op(kind, paths.clone(), dest.clone(), true);
        // Only the trash can be undone. `D` is asked for twice and then means it.
        if kind == OpKind::Trash {
            self.record_job(id, UndoStep::Trash { paths, dir: dest }, Land::Fresh);
        }
        self.tabs[self.active].clear_selection();
    }

    /// Unpack every selected archive. Anything that is not one is named in an
    /// error rather than silently dropped, so a mixed selection says what it
    /// skipped.
    pub(super) fn do_extract(&mut self) {
        let paths = self.tabs[self.active].targets();
        if paths.is_empty() {
            return;
        }
        let (archives, rest): (Vec<PathBuf>, Vec<PathBuf>) = paths.into_iter().partition(|p| archive::Format::from_path(p).is_some());
        if archives.is_empty() {
            // Name what was actually looked at. "Nothing here" reads as "this
            // directory has none", which is wrong often enough to matter: a
            // selection left over from an earlier command is what `e` acts on,
            // so the archive under the cursor can be ignored while the message
            // appears to deny it exists. Saying how many were examined is what
            // makes a stale selection visible — the count in the header is
            // behind the toast saying this.
            let n = self.tabs[self.active].selected.len();
            // A folder is the one thing `e` is pressed on by someone who meant
            // to pack it (#262): say which key does that.
            if n == 0 && rest.first().is_some_and(|p| p.is_dir()) {
                let name = rest[0].file_name().map_or_else(|| rest[0].display().to_string(), |n| n.to_string_lossy().into_owned());
                let pack = match self.compress_key() {
                    Some(k) => format!("; to pack it, press {k}"),
                    None => String::new(),
                };
                self.error(format!(
                    "{name}{} is a folder -- e unpacks an archive (zip, tar, tar.gz, tgz, 7z){pack}",
                    std::path::MAIN_SEPARATOR
                ));
                return;
            }
            let what = if n == 0 { "The file under the cursor is not".to_owned() } else { format!("None of the {n} selected item(s) is") };
            self.error(format!("{what} an archive kura can read (zip, tar, tar.gz, tgz, 7z)"));
            return;
        }
        if !rest.is_empty() {
            self.toast(format!("Skipping {} non-archive item(s)", rest.len()));
        }
        let cwd = self.tabs[self.active].cwd.clone();
        self.submit_op(OpKind::Extract, archives, cwd, false);
        self.tabs[self.active].clear_selection();
    }

    /// Ask what the archive should be called. The extension picks the format,
    /// so one prompt covers zip, tar and tar.gz without a second menu.
    pub(super) fn ask_compress(&mut self) {
        let paths = self.tabs[self.active].targets();
        if paths.is_empty() {
            return;
        }
        // One item names the archive after itself; several after the folder
        // they are in, which is what the user would have typed anyway.
        let stem = match paths.len() {
            1 => util::stem_and_ext(&util::file_name(&paths[0])).0.to_owned(),
            _ => util::file_name(&self.tabs[self.active].cwd),
        };
        let stem = match stem.is_empty() {
            true => "archive".to_owned(),
            false => stem,
        };
        // Packing `to-pack.zip` offered `to-pack.zip`, and two `<Enter>`s
        // replaced the archive with one holding itself (#265).
        let mut name = format!("{stem}.zip");
        if paths.iter().any(|p| util::file_name(p).eq_ignore_ascii_case(&name)) {
            name = format!("{stem}-packed.zip");
        }
        self.open_input(InputKind::Compress, "Compress to", name);
    }

    pub(super) fn do_compress(&mut self, name: &str) {
        let name = name.trim();
        if name.is_empty() {
            return;
        }
        let cwd = self.tabs[self.active].cwd.clone();
        let dest = util::resolve_against(&cwd, name);
        let Some(format) = archive::Format::from_path(&dest) else {
            self.error("Name it .zip, .7z, .tar or .tar.gz to say which format");
            return;
        };
        // Every format that can be read can be written, since v0.27.0. The
        // guard stays: `can_write` is what the writer asserts on, and a format
        // added for reading alone would otherwise reach it.
        if !format.can_write() {
            self.error(format!("{} can be read here but not written — use .zip, .7z, .tar or .tar.gz", format.label(),));
            return;
        }
        let paths = self.tabs[self.active].targets();
        if paths.is_empty() {
            return;
        }
        // The archive is written over `dest`, so a source with that name is
        // gone before it is read. Nothing to confirm: it can only be a mistake.
        if paths.iter().any(|p| same_place(p, &dest)) {
            self.error(format!("Can't pack {} into itself — give the archive another name", util::file_name(&dest)));
            return;
        }
        self.submit_op_to(OpKind::Compress(format), paths, cwd, Some(dest), false);
        self.tabs[self.active].clear_selection();
    }

    pub(super) fn submit_op(&mut self, kind: OpKind, srcs: Vec<PathBuf>, dest_dir: PathBuf, force: bool) -> u64 {
        self.submit_op_to(kind, srcs, dest_dir, None, force)
    }

    /// Queue a job and return its id, which [`App::record_job`] uses to hang an
    /// undo step on it.
    pub(super) fn submit_op_to(
        &mut self,
        kind: OpKind,
        srcs: Vec<PathBuf>,
        dest_dir: PathBuf,
        dest_file: Option<PathBuf>,
        force: bool,
    ) -> u64 {
        let id = self.scanner.next_id();
        let label = match &dest_file {
            // An archive says which format its name made: `tar.gz.zip` is a
            // zip, which is not what was meant by typing `tar.gz` over the
            // stem, and nothing said so (#174).
            Some(f) => match kind {
                OpKind::Compress(format) => {
                    format!("{} {} item(s) into {} (as {})", kind.verb(), srcs.len(), util::file_name(f), format.label())
                }
                _ => format!("{} {} item(s) into {}", kind.verb(), srcs.len(), util::file_name(f)),
            },
            None => format!("{} {} item(s)", kind.verb(), srcs.len()),
        };
        self.tasks.push(Task {
            id,
            kind,
            label,
            files: 0,
            bytes: 0,
            files_done: 0,
            bytes_done: 0,
            current: String::new(),
            // One job runs at a time, so a new one is queued until the worker
            // picks it up and says `Started`.
            state: TaskState::Queued,
            errors: Vec::new(),
            finished: None,
            speed: 0.0,
            sampled_at: Instant::now(),
            sampled_bytes: 0,
            started_at: Instant::now(),
        });
        if kind == OpKind::Delete {
            self.deleting.insert(id, srcs.clone());
        }
        self.ops.submit(OpRequest { id, kind, srcs, dest_dir, dest_file, force });
        id
    }

    /// `D` went through: a red `Trash: <name>: …` still up about one of the
    /// files it took -- the `d` that failed and said to press `D` -- is no
    /// longer true, so it gives way to `Deleted <name>` (#207). Only that:
    /// a `D` with nothing to answer stays as quiet as it was.
    pub(super) fn deleted(&mut self, paths: &[PathBuf]) {
        let names: Vec<String> = paths.iter().map(|p| util::file_name(p)).collect();
        let before = self.toasts.len();
        self.toasts.retain(|t| t.level != Level::Error || !names.iter().any(|n| t.text.starts_with(&format!("Trash: {n}: "))));
        if self.toasts.len() == before {
            return;
        }
        match names.as_slice() {
            [one] => self.toast(format!("Deleted {one}")),
            many => self.toast(format!("Deleted {} item(s)", many.len())),
        }
    }

    /// Remember that finishing job `id` leaves `step` on one of the undo
    /// stacks. Nothing is pushed until the worker says the work went through.
    pub(super) fn record_job(&mut self, id: u64, step: UndoStep, how: Land) {
        self.op_undo.insert(id, (step, how));
    }

    pub(super) fn on_op_event(&mut self, ev: ops::OpEvent) {
        match ev {
            ops::OpEvent::Started { id, files, bytes } => {
                if let Some(t) = self.tasks.iter_mut().find(|t| t.id == id) {
                    t.files = files;
                    t.bytes = bytes;
                    t.state = TaskState::Running;
                    // The queue may have held it a while; the speed is about
                    // the work, not the wait.
                    t.sampled_at = Instant::now();
                    t.sampled_bytes = 0;
                    t.started_at = Instant::now();
                }
            }
            ops::OpEvent::Progress { id, files_done, bytes_done, current } => {
                if let Some(t) = self.tasks.iter_mut().find(|t| t.id == id) {
                    t.files_done = files_done;
                    t.sample(bytes_done);
                    t.bytes_done = bytes_done;
                    if !current.is_empty() {
                        t.current = current;
                    }
                }
            }
            ops::OpEvent::Paused { id, paused } => {
                if let Some(t) = self.tasks.iter_mut().find(|t| t.id == id) {
                    t.state = if paused { TaskState::Paused } else { TaskState::Running };
                    // Nothing moved while it was parked, so the old speed is
                    // not a measurement of anything.
                    t.speed = 0.0;
                    t.sampled_at = Instant::now();
                    t.sampled_bytes = t.bytes_done;
                }
            }
            ops::OpEvent::Conflict { id, src, dest, reply } => {
                self.overlay = Overlay::Confirm(ConfirmOverlay {
                    title: "File already exists".into(),
                    body: vec![format!("src:  {}", src.display()), format!("dest: {}", dest.display())],
                    options: vec![
                        ('o', "Overwrite".into()),
                        ('a', "Overwrite all".into()),
                        ('s', "Skip".into()),
                        ('S', "Skip all".into()),
                        ('r', "Rename…".into()),
                        ('q', "Cancel".into()),
                    ],
                    action: ConfirmAction::Conflict { reply, job: id },
                    dest: Some(dest),
                });
            }
            ops::OpEvent::Finished { id, errors, cancelled, kind, moved, linked, junctions, made, trashed } => {
                // A move that actually moved something is a step `u` can take
                // back. A cancelled one is not: half a move is not a state
                // worth offering to reverse in one keystroke.
                // A cut whose paste moved nothing (every clash skipped, or the
                // whole thing cancelled) is still a cut: give it back, unless
                // something else has been yanked since.
                if let Some(cut) = self.cut_jobs.remove(&id) {
                    if moved.is_empty() && self.yank.paths.is_empty() {
                        self.yank = Yank { paths: cut, cut: true, from_archive: false };
                        if errors.is_empty() {
                            self.toast("Nothing moved — the cut is still there");
                        }
                    }
                }
                if kind == OpKind::Move && !cancelled && !moved.is_empty() {
                    self.undos.land(UndoStep::Move { pairs: moved }, Land::Fresh);
                }
                // Links are made one by one and each stands alone, so what was
                // made is a step even when a cancel or an error stopped the rest.
                if !linked.is_empty() {
                    let step = UndoStep::Link { links: linked };
                    // A new link barely changes the listing, so say it was
                    // made, as `d` does (#168). Not over an error: that toast
                    // comes below and is the one to read.
                    if errors.is_empty() && !cancelled {
                        self.toast(format!("{} — u to undo", step.redone_label()));
                    }
                    self.undos.land(step, Land::Fresh);
                }
                if let Some(t) = self.tasks.iter_mut().find(|t| t.id == id) {
                    t.state = if cancelled {
                        TaskState::Cancelled
                    } else if errors.is_empty() {
                        TaskState::Done
                    } else {
                        TaskState::Failed
                    };
                    t.errors = errors.clone();
                    t.finished = Some(Instant::now());
                }
                // A long job nobody is watching: the ways `[notify]` turns on
                // say it is over (#345). `focused` is `None` until the window
                // has said; then nothing is claimed.
                let behind = self.ctx.input(|i| i.viewport().focused) == Some(false);
                if let Some(t) = self.tasks.iter().find(|t| t.id == id) {
                    if let Some((title, body)) = long_op_notice(t, behind) {
                        let event = if t.state == TaskState::Failed { "failed" } else { "done" };
                        crate::notify::tell(&self.cfg.notify, event, &title, &body);
                    }
                }
                if !errors.is_empty() {
                    self.error(format!("{}: {}", kind.verb(), errors[0]));
                } else if cancelled {
                    // Stopped from the tasks list: what was already done stays
                    // (#241), and nothing else on screen says the job ended.
                    self.toast(format!("{} stopped — what was already done stays", kind.verb()));
                }
                if let Some(paths) = self.deleting.remove(&id) {
                    if errors.is_empty() && !cancelled {
                        self.deleted(&paths);
                    }
                }
                if !junctions.is_empty() {
                    self.offer_junctions(junctions);
                }
                if let Some((step, how)) = self.op_undo.remove(&id) {
                    if errors.is_empty() && !cancelled {
                        let said = match how {
                            Land::Undone => Some(step.undone_label()),
                            Land::Redone => Some(step.redone_label()),
                            // `d` looks the same as `D` on screen, so the trash
                            // says what went and how to get it back (#108).
                            Land::Fresh => matches!(step, UndoStep::Trash { .. }).then(|| format!("{} — u to undo", step.redone_label())),
                        };
                        self.undos.land(step, how);
                        if let Some(said) = said {
                            self.toast(said);
                        }
                    } else if let (UndoStep::Trash { dir, .. }, Land::Fresh, false) = (&step, how, trashed.is_empty()) {
                        // Some went and some did not: what went is a step `u`
                        // can take back, and the rest is in the error above.
                        self.undos.land(UndoStep::Trash { paths: trashed, dir: dir.clone() }, Land::Fresh);
                    } else {
                        self.undos.keep(step, how);
                    }
                }
                let cwd = self.tabs[self.active].cwd.clone();
                // The archive just packed is what you want to look at next
                // (Q25) -- but only where you still are: a compress that ends
                // after you moved on does not pull you back.
                if let (OpKind::Compress(_) | OpKind::TakeOut | OpKind::Extract, Some(archive)) = (kind, made.first()) {
                    if archive.parent() == Some(cwd.as_path()) {
                        self.land_on = Some(archive.clone());
                    }
                }
                // Packing and unpacking change the listing by one name each,
                // easily missed, so the end says what was made (#174, #205).
                // Not over an error or a cancel: the toast above is the one
                // to read then.
                if errors.is_empty() && !cancelled {
                    if let Some(said) = made_says(kind, &made) {
                        self.toast(said);
                    }
                }
                self.cache.remove(&cwd);
                self.rescan(&cwd);
            }
        }
    }

    /// `<F12>`: what a bug report would carry, shown before anything leaves
    /// the machine (Q62). `o` / `<Enter>` opens the issue form with it filled
    /// in, `c` copies the link, `n` / `<Esc>` drops it. It used to open the
    /// browser on the press, and nobody saw what went with it -- which is
    /// fine for a version and an OS, and not for the keys and names Q64 adds.
    pub(super) fn bug_report(&mut self) {
        // The `<F12>` itself is not part of what happened.
        let keys: Vec<String> = self.recent_keys.iter().rev().skip(1).rev().cloned().collect();
        let keys = keys.join(" ");
        let context = crate::bugreport::context(self.last_error.as_deref(), &self.cfg.loaded);
        let url = crate::bugreport::url(&[
            ("keys", if keys.is_empty() { String::new() } else { format!("Last keys, oldest first: {keys}") }),
            ("context", context.clone()),
        ]);
        let mut body = vec![crate::bugreport::version_line()];
        body.extend(crate::bugreport::os_line().lines().map(str::to_owned));
        body.push(format!("Last keys: {}", if keys.is_empty() { "(none)" } else { &keys }));
        body.extend(context.lines().map(str::to_owned));
        body.push(String::new());
        body.push("The form opens with these filled in; nothing is sent until you submit it there.".into());
        self.overlay = Overlay::Confirm(ConfirmOverlay {
            title: "Report a bug".into(),
            body,
            options: vec![('o', "Open the form in your browser".into()), ('c', "Copy the link".into()), ('n', "Cancel".into())],
            action: ConfirmAction::BugReport { url },
            dest: None,
        });
    }

    /// The `o` to [`App::bug_report`]. When the browser cannot be opened,
    /// the URL goes on the clipboard so it can be pasted into one by hand --
    /// only then, so a report that worked never overwrites what was on the
    /// clipboard (Q22). The form's fields travel in the URL, so the pasted
    /// link is the whole report.
    pub(super) fn open_report(&mut self, url: String) {
        match exec::open_url(&url) {
            Ok(()) => self.toast("Opened a bug report in your browser"),
            Err(e) => match exec::set_clipboard(&url) {
                Ok(()) => {
                    self.error(format!("could not open the browser: {e}. The report's link is on the clipboard -- paste it into one"))
                }
                Err(_) => self.error(format!("could not open the browser: {e}")),
            },
        }
        self.last_report = Some(url);
    }

    pub(super) fn start_rename(&mut self, cursor: RenameCursor) {
        let Some(e) = self.tabs[self.active].current.hovered().cloned() else { return };
        let name = e.name.clone();
        let (stem, _ext) = util::stem_and_ext(&name);
        // A folder's name has no extension to keep, whatever dot is in it.
        let stem = if e.is_dir_like() { name.as_str() } else { stem };
        let sel = match cursor {
            RenameCursor::Start => (0, 0),
            RenameCursor::End => (name.chars().count(), name.chars().count()),
            RenameCursor::BeforeExt => (0, stem.chars().count()),
        };
        let mut ov = InputOverlay {
            kind: InputKind::Rename { from: e.path.clone() },
            title: "Rename".into(),
            text: name,
            initial_selection: Some(sel),
            focused: false,
            completion: Vec::new(),
            completion_at: 0,
        };
        ov.completion.clear();
        self.overlay = Overlay::Input(ov);
    }

    /// What `cc` / `cd` / `cf` / `cn` put on the clipboard, one line per path.
    pub(super) fn copy_lines(paths: &[PathBuf], what: CopyWhat) -> String {
        let line = |p: &PathBuf| match what {
            // Outside the spot panel the path is the only cell.
            CopyWhat::Path | CopyWhat::Cell | CopyWhat::All => p.display().to_string(),
            CopyWhat::Dirname => p.parent().map(|d| d.display().to_string()).unwrap_or_default(),
            CopyWhat::Filename => util::file_name(p),
            CopyWhat::NameWithoutExt => util::stem_and_ext(&util::file_name(p)).0.to_owned(),
        };
        paths.iter().map(line).collect::<Vec<_>>().join("\n")
    }

    /// Copy the selection, or the hovered file when nothing is selected --
    /// the rule every other file command follows. Copying only the row under
    /// the cursor while several were marked silently dropped the rest.
    pub(super) fn copy_text(&mut self, what: CopyWhat) {
        let paths = self.tabs[self.active].targets();
        if paths.is_empty() {
            // Quietly doing nothing left the last thing copied on the
            // clipboard, which looks like a valid path when pasted (#108).
            self.error("Nothing to copy — the list is empty");
            return;
        }
        let text = Self::copy_lines(&paths, what);
        match exec::set_clipboard(&text) {
            Ok(()) => match paths.len() {
                1 => self.toast(format!("Copied: {text}")),
                n => self.toast(format!("Copied {n} lines")),
            },
            Err(err) => self.error(format!("Clipboard: {err}")),
        }
    }

    /// Jump to where the hovered link points. The target came off the scan
    /// worker with the rest of the entry, so nothing here touches disk —
    /// `canonicalize` on a link into a share that stopped answering used to
    /// freeze the window until it gave up.
    pub(super) fn follow_link(&mut self) {
        // An empty directory has nothing to say something about, so that one
        // stays quiet; a row that is simply not a link does not.
        let Some(e) = self.tabs[self.active].current.hovered().cloned() else { return };
        let Kind::Link { to_dir, broken } = e.kind else {
            // Naming the kind of thing the key is for, and how to spot one:
            // pressed on an ordinary file it did nothing at all, which is the
            // same as a key that is not bound to anything.
            self.error("Only a symlink can be followed — a link shows -> after its name");
            return;
        };
        let Some(target) = e.link_to.filter(|_| !broken) else {
            self.error(format!("Broken link: {}", e.name));
            return;
        };
        // A link to a file lands on its directory with the file under the
        // cursor; a link to a directory simply opens it.
        let (dir, reveal) = match target.parent() {
            Some(p) if !to_dir => (p.to_path_buf(), Some(util::file_name(&target))),
            _ => (target, None),
        };
        self.cd(dir.clone(), true);
        if let Some(name) = reveal {
            // The listing may still be on its way, so leave the name in `memo`
            // as well — that is what the cursor is restored from on arrival.
            let tab = &mut self.tabs[self.active];
            tab.memo.insert(dir, name.clone());
            tab.current.select_name(&name);
        }
    }

    pub(super) fn sort(&mut self, by: Option<crate::fs::SortBy>, reverse: Tri, dir_first: Tri) {
        let tab = &mut self.tabs[self.active];
        if let Some(b) = by {
            tab.sort.by = b;
        }
        if let Some(r) = reverse {
            tab.sort.reverse = r;
        }
        if let Some(d) = dir_first {
            tab.sort.dir_first = d;
        }
        let sort = tab.sort;
        let show = tab.show_hidden;
        // The usage view counted hidden files and lists them whatever the tab
        // shows; a re-sort that filtered them out left rows missing from the
        // total it still printed (#109).
        let in_usage = self.usage_linemode.is_some();
        let tab = &mut self.tabs[self.active];
        tab.current.resort(&sort, show || in_usage);
        if let Some(p) = tab.parent.as_mut() {
            p.resort(&sort, show);
        }
        if let PreviewState::Dir(f) = &mut self.preview.state {
            f.resort(&sort, show);
        }
        let label = sort.by.label().to_owned();
        self.toast(format!("Sort: {label}{}", if sort.reverse { " (reverse)" } else { "" }));
    }

    pub(super) fn find_arrow(&mut self, prev: bool) {
        if self.tabs[self.active].finder.is_none() && !self.log_needle.is_empty() && self.log_view().is_some() {
            return self.log_step(prev);
        }
        if self.find_in_body(prev) {
            return;
        }
        let Some(finder) = self.tabs[self.active].finder.clone() else { return };
        let tab = &mut self.tabs[self.active];
        let len = tab.current.view.len();
        if len == 0 {
            return;
        }
        let Some(matcher) = finder.matcher else { return };
        let dir: i64 = if prev != finder.prev { -1 } else { 1 };
        for step in 1..=len as i64 {
            let idx = (tab.current.cursor as i64 + dir * step).rem_euclid(len as i64) as usize;
            let Some(e) = tab.current.at(idx) else { continue };
            if matcher.is_match(&e.name) {
                tab.current.cursor = idx;
                tab.sync_visual();
                return;
            }
        }
    }

    /// Open the targets; `line` (1-based) asks an editor to start there, and is
    /// only honored when the hovered file is the one being opened.
    pub(super) fn open(&mut self, interactive: bool, line: Option<usize>) {
        let paths = self.tabs[self.active].targets();
        if paths.is_empty() {
            return;
        }
        let Some(entry) = self.tabs[self.active].current.hovered().cloned() else { return };
        // Inside an archive the members are not files yet: `<Enter>` opens a
        // copy, the way `l` does, and a folder is gone into.
        if self.archive_view.is_some() {
            self.enter();
            return;
        }
        // `<Enter>` on a folder is `l`, the usage view's going down included.
        if entry.is_dir_like() && !interactive {
            self.enter();
            return;
        }
        let line = line.filter(|_| paths.len() == 1 && paths[0] == entry.path);
        let mime = crate::mime::guess(&entry);
        let from = exec::openers_from(&self.cfg.yazi, &entry, mime);
        let openers: Vec<(String, bool, bool, String)> = from.iter().map(|(o, _)| (o.run.clone(), o.block, o.orphan, o.label())).collect();

        if interactive {
            if openers.is_empty() {
                self.error("No opener configured for this file type");
                return;
            }
            // Shown, not hidden: an opener that vanished would look like a
            // setting that was ignored.
            let items: Vec<String> = openers
                .iter()
                .map(|o| match exec::opener_missing(&o.0) {
                    true => format!("{} (not found)", o.3),
                    false => o.3.clone(),
                })
                .collect();
            // Where a row came from, for a config that layers `prepend_rules`
            // or `append_rules` over its `rules` (Q88).
            let details: Vec<String> = openers.iter().zip(&from).map(|(o, (_, set))| format!("{}{}", o.0, set.mark())).collect();
            // The cursor starts on the first opener that is installed (Q86);
            // all missing, or none to skip, leaves it on the first row.
            let first_found = openers.iter().position(|o| !exec::opener_missing(&o.0)).unwrap_or(0);
            let runs: Vec<(String, bool, bool)> = openers.iter().map(|o| (o.0.clone(), o.1, o.2)).collect();
            let mut pick = PickOverlay {
                title: "Open with".into(),
                items,
                details,
                query: String::new(),
                matches: Vec::new(),
                cursor: 0,
                action: PickAction::OpenWith { paths, runs, line },
                focused: false,
            };
            pick.refilter();
            pick.cursor = first_found;
            self.overlay = Overlay::Pick(pick);
            return;
        }

        let cwd = self.tabs[self.active].cwd.clone();
        // The first opener whose program is installed, as `<S-Enter>`'s list
        // starts its cursor (Q87); all missing leaves the first to say so.
        let first_found = openers.iter().position(|o| !exec::opener_missing(&o.0)).unwrap_or(0);
        // Asked before, not after (Q92): handed over, a type Windows has no
        // app for opened a chooser at best and nothing at all at worst, and
        // `open` says `Ok` either way.
        let others = openers.iter().any(|o| !exec::starts_default(&o.0));
        let hands_over = openers.get(first_found).is_none_or(|o| exec::starts_default(&o.0));
        let no_app = match openers.get(first_found) {
            None => (!exec::has_default_app(&entry.path)).then(|| entry.path.clone()),
            Some(_) => paths.iter().find(|p| !exec::has_default_app(p)).cloned(),
        };
        if let Some(path) = no_app.filter(|_| hands_over) {
            let pick = if others { self.pick_opener_key() } else { None };
            self.error(exec::no_default_app_note(&path, pick.as_deref()));
            return;
        }
        match openers.get(first_found) {
            Some((run, block, orphan, _)) => {
                // Said, so an opener that cannot take a line (Word) is not read as having lost it.
                let no_line = exec::no_line_note(run, &paths, line, &self.cfg.line_args);
                let line = exec::command_line(run, &paths, line, &self.cfg.line_args);
                let (block, orphan) = (*block, *orphan);
                // Said, so a `yazi.toml` line that did not run is not read as ignored.
                let mut note = match openers.first().and_then(|o| exec::missing_name(&o.0)) {
                    Some(name) if first_found > 0 => format!(" (skipped {name}: not found)"),
                    _ => String::new(),
                };
                note.push_str(&no_line);
                self.launch(&line, &cwd, block, orphan, "Open failed", &note);
            }
            // Said, as an opener's launch is: handed to the system, a file
            // whose app takes a while to appear looked as if `<Enter>` had
            // done nothing (#163).
            None => match exec::open_default(&entry.path) {
                Ok(()) => self.toast(exec::opened_default_note(&entry.name)),
                Err(e) => self.error(format!("Open failed: {e}")),
            },
        }
    }

    pub(super) fn run_shell(&mut self, run: &str, block: bool, orphan: bool) {
        let paths = self.tabs[self.active].targets();
        let cwd = self.tabs[self.active].cwd.clone();
        let line = exec::substitute_line(run, &paths);
        // Off Windows the terminal already holds the window when the line
        // fails; there a console simply closes, so `:` waits for a key (Q13).
        let line = if block && cfg!(windows) { exec::held(&line) } else { line };
        // Said, as the line alone does not tell a run that got the paths from one that did not.
        let note = match exec::line_skips_path(run) && !paths.is_empty() {
            true => " (no path: the line has a shell operator; use %* to place it)",
            false => "",
        };
        self.launch(&line, &cwd, block, orphan, "Command failed", note);
    }

    /// Run `line`, say so, and keep listening in case it falls over a moment
    /// later — which is the usual way an opener fails, the shell having
    /// started fine and then found nothing to run. See [`exec::Launch`].
    pub(super) fn launch(&mut self, line: &str, cwd: &Path, block: bool, orphan: bool, what: &str, note: &str) {
        match exec::shell(line, cwd, block, orphan) {
            Ok(l) => {
                self.toast(format!("$ {line}{note}"));
                self.last_launch = Some(format!("launched: {} {line}", l.pid.map_or("-".into(), |p| p.to_string())));
                self.launches.push((l, line.to_owned(), what.to_owned(), note.to_owned()));
                crate::runinfo::remember_launch(line);
            }
            Err(e) => self.error(format!("{what}: {e}")),
        }
    }
}
