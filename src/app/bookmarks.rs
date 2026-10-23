//! Bookmarks and the commands that open, run or send things.

use super::*;

impl App {
    pub fn bookmark_key(&mut self, ch: char) {
        let Some(op) = self.pending_bookmark.take() else { return };
        let key = ch.to_string();
        match op {
            BookmarkOp::Save => {
                let path = self.tabs[self.active].cwd.clone();
                let name = util::file_name(&path);
                self.bookmarks.retain(|b| b.key != key);
                self.bookmarks.push(Bookmark { key: key.clone(), path, name });
                self.bookmarks.sort_by(|a, b| a.key.cmp(&b.key));
                self.save_state();
                self.toast(format!("Bookmark `{key}` saved"));
            }
            BookmarkOp::Jump => match self.bookmarks.iter().find(|b| b.key == key).cloned() {
                Some(b) => self.cd(b.path, true),
                None => self.error(format!("No bookmark `{key}`")),
            },
            BookmarkOp::Delete => {
                let before = self.bookmarks.len();
                self.bookmarks.retain(|b| b.key != key);
                if self.bookmarks.len() == before {
                    self.error(format!("No bookmark `{key}`"));
                } else {
                    self.save_state();
                    self.toast(format!("Bookmark `{key}` deleted"));
                }
            }
        }
    }

    pub(super) fn open_palette(&mut self) {
        let (items, details, runs) = palette_items(&self.cfg.keymap.mgr, &self.hovered_openers());
        if items.is_empty() {
            self.error("No commands are bound");
            return;
        }
        self.open_pick("Commands".into(), items, details, runs);
    }

    /// The context menu for the file under the cursor. Right-click opens it;
    /// so does the `menu` command.
    pub(super) fn open_menu(&mut self) {
        let Some(name) = self.tabs[self.active].current.hovered_name().map(str::to_owned) else {
            self.error("Nothing under the cursor");
            return;
        };
        let (items, details, runs) = menu_items(&self.cfg.keymap.mgr, &self.hovered_openers());
        if items.is_empty() {
            self.error("Nothing is bound for this file");
            return;
        }
        // The selection is what the commands will act on, so say so when it is
        // more than the one file the pointer landed on.
        let n = self.tabs[self.active].target_count();
        let title = match n > 1 {
            true => format!("Actions: {n} selected"),
            false => format!("Actions: {name}"),
        };
        self.open_pick(title, items, details, runs);
    }

    /// What `yazi.toml` offers to open the file under the cursor with.
    pub(super) fn hovered_openers(&self) -> Vec<OpenerRow> {
        let Some(entry) = self.tabs[self.active].current.hovered() else { return Vec::new() };
        let mime = crate::mime::guess(entry);
        exec::openers_for(&self.cfg.yazi, entry, mime).into_iter().map(|o| (o.run.clone(), o.block, o.orphan, o.label())).collect()
    }

    pub(super) fn open_pick(&mut self, title: String, items: Vec<String>, details: Vec<String>, runs: Vec<Vec<Act>>) {
        let mut pick = PickOverlay {
            title,
            items,
            details,
            query: String::new(),
            matches: Vec::new(),
            cursor: 0,
            action: PickAction::Command { runs },
            focused: false,
        };
        pick.refilter();
        self.overlay = Overlay::Pick(pick);
    }

    /// The bookmarks on their own, as a picker.
    ///
    /// Separate from [`Self::open_jump`], which mixes in the visit history: a
    /// bookmark was named on purpose, and a list of them is the one place to
    /// see what is bound to what without holding a key down to read the hint.
    pub(super) fn open_bookmark_list(&mut self) {
        if self.bookmarks.is_empty() {
            self.error("No bookmarks yet — `bs` saves this directory under a letter");
            return;
        }
        let mut items = Vec::new();
        let mut details = Vec::new();
        let mut paths = Vec::new();
        for b in &self.bookmarks {
            items.push(format!("[{}] {}", b.key, b.path.display()));
            details.push(b.name.clone());
            paths.push(b.path.clone());
        }
        let mut pick = PickOverlay {
            title: "Bookmarks".into(),
            items,
            details,
            query: String::new(),
            matches: Vec::new(),
            cursor: 0,
            action: PickAction::Jump { paths },
            focused: false,
        };
        pick.refilter();
        self.overlay = Overlay::Pick(pick);
    }

    pub(super) fn open_jump(&mut self) {
        let mut items = Vec::new();
        let mut details = Vec::new();
        let mut paths = Vec::new();
        for b in &self.bookmarks {
            items.push(format!("[{}] {}", b.key, b.path.display()));
            details.push(b.name.clone());
            paths.push(b.path.clone());
        }
        // Bookmarks were named on purpose, so they stay on top in their own
        // order. The rest is ranked: most used, least stale first.
        let now = epoch_secs();
        let mut ranked: Vec<&Visit> = self.history.iter().collect();
        ranked.sort_by(|a, b| {
            let score = |v: &Visit| frecency(v.hits, now.saturating_sub(v.at));
            score(b).total_cmp(&score(a)).then(b.at.cmp(&a.at))
        });
        for v in ranked {
            if paths.contains(&v.path) {
                continue;
            }
            items.push(v.path.display().to_string());
            details.push(ago(now.saturating_sub(v.at)));
            paths.push(v.path.clone());
        }
        if items.is_empty() {
            self.error("No bookmarks or history yet");
            return;
        }
        let mut pick = PickOverlay {
            title: "Jump to".into(),
            items,
            details,
            query: String::new(),
            matches: Vec::new(),
            cursor: 0,
            action: PickAction::Jump { paths },
            focused: false,
        };
        pick.refilter();
        self.overlay = Overlay::Pick(pick);
    }

    pub fn submit_pick(&mut self) {
        let Overlay::Pick(p) = std::mem::replace(&mut self.overlay, Overlay::None) else {
            return;
        };
        // Filtered down to nothing: stay open and say so, rather than close
        // as if something had been chosen (#198).
        let Some(idx) = p.selected() else {
            let said = format!("Nothing matches `{}` — <Esc> closes", p.query);
            self.overlay = Overlay::Pick(p);
            self.toast(said);
            return;
        };
        match p.action {
            PickAction::OpenWith { paths, runs, line } => {
                let Some((run, block, orphan)) = runs.get(idx).cloned() else { return };
                let cwd = self.tabs[self.active].cwd.clone();
                let line = exec::command_line(&run, &paths, line, &self.cfg.line_args);
                self.launch(&line, &cwd, block, orphan, "Open failed", "");
            }
            PickAction::Jump { paths } => {
                if let Some(p) = paths.get(idx).cloned() {
                    self.cd(p, true);
                }
            }
            PickAction::Command { runs } => {
                // The overlay is already closed, so a command that opens one of
                // its own (input, confirm, help) lands on a clean slate.
                let Some(acts) = runs.get(idx).cloned() else { return };
                for a in acts {
                    self.act(a);
                }
            }
        }
    }

    /// Paste `text` into the terminal. Several lines into a program that did
    /// not ask for bracketed paste would run each one as it lands, so that case
    /// is asked about first -- the same question tsumugi puts (Windows
    /// Terminal's and iTerm2's warning). A shell that asks for bracketed paste
    /// gets the lines without a word: there they are only text until Enter.
    pub fn paste_to_term(&mut self, text: &str) {
        let Some(t) = &self.term else { return };
        let Some(lines) = multiline_paste(text, t.bracketed_paste()) else {
            t.paste(text);
            return;
        };
        self.overlay = Overlay::Confirm(ConfirmOverlay {
            title: format!("Paste {lines} lines?"),
            body: ["The program here did not ask for bracketed paste, so each line runs as it lands.".to_owned(), String::new()]
                .into_iter()
                .chain(paste_preview(text))
                .collect(),
            options: vec![('y', "Paste".into()), ('n', "Cancel".into())],
            action: ConfirmAction::PasteToTerm { text: text.to_owned() },
            dest: None,
        });
    }

    /// Ask the user about a change `kura mcp` wants. Nothing is done until
    /// they press Allow; one box at a time, so a request that finds another
    /// overlay open is turned away rather than queued behind it.
    pub fn mcp_write(
        &mut self,
        write: crate::mcp::Write,
        reply: std::sync::mpsc::SyncSender<crate::mcp::Reply>,
        expired: std::sync::Arc<std::sync::atomic::AtomicBool>,
    ) {
        if !matches!(self.overlay, Overlay::None) {
            let _ = reply.send(Err("another box is open in the kura window; ask again in a moment".into()));
            return;
        }
        self.overlay = Overlay::Confirm(ConfirmOverlay {
            title: write.title(),
            body: write.body(),
            options: vec![('n', "Refuse".into()), ('y', "Allow".into())],
            action: ConfirmAction::Mcp { write, reply, expired },
            dest: None,
        });
    }

    /// Close a `kura mcp` box whose request has timed out: the client was
    /// told "not done", so the box asks nothing any more (#356).
    pub fn drop_expired_mcp_box(&mut self) {
        let Overlay::Confirm(c) = &self.overlay else { return };
        let ConfirmAction::Mcp { expired, .. } = &c.action else { return };
        if expired.load(std::sync::atomic::Ordering::Relaxed) {
            self.overlay = Overlay::None;
            self.toast("The request from Claude timed out -- nothing was done");
        }
    }

    /// Carry out what the user allowed; the text goes back to the client.
    pub(super) fn run_mcp_write(&mut self, w: crate::mcp::Write) -> Result<String, String> {
        use crate::mcp::WriteTool;
        let n = w.paths.len();
        match w.tool {
            WriteTool::Copy | WriteTool::Move => {
                let kind = if w.tool == WriteTool::Copy { OpKind::Copy } else { OpKind::Move };
                let dest = PathBuf::from(w.to.unwrap_or_default());
                self.submit_op(kind, w.paths, dest, false);
                Ok(format!("started: {n} item(s); progress is in kura's jobs list"))
            }
            WriteTool::Trash => {
                let dir = w.paths[0].parent().map(Path::to_path_buf).unwrap_or_default();
                let id = self.submit_op(OpKind::Trash, w.paths.clone(), dir.clone(), true);
                self.record_job(id, UndoStep::Trash { paths: w.paths, dir }, Land::Fresh);
                Ok(format!("started: {n} item(s) to the trash; the user can undo it in kura"))
            }
            WriteTool::Rename => {
                let from = w.paths[0].clone();
                let to = from.with_file_name(w.to.unwrap_or_default());
                if to.exists() {
                    return Err(format!("{} already exists; nothing renamed", to.display()));
                }
                self.apply_rename(&from, &to)?;
                self.undos.land(UndoStep::Rename { from, to: to.clone() }, Land::Fresh);
                Ok(format!("renamed to {}", to.display()))
            }
        }
    }

    pub fn answer_confirm(&mut self, ch: char) {
        // A key the box does not offer leaves it open (Q71): a stray `(`
        // used to close the `<F12>` box in silence, and the `<Enter>` meant
        // for it then went into a folder in the list. `n` always answers --
        // every box reads it as no -- and `<Esc>` closes before this.
        if let Overlay::Confirm(c) = &self.overlay {
            if ch != 'n' && !c.options.iter().any(|(k, _)| *k == ch) {
                return;
            }
        }
        let Overlay::Confirm(c) = std::mem::replace(&mut self.overlay, Overlay::None) else {
            return;
        };
        match c.action {
            ConfirmAction::Conflict { reply, job } => {
                let r = match ch {
                    'o' => Resolution::Overwrite,
                    'a' => Resolution::OverwriteAll,
                    's' => Resolution::Skip,
                    'S' => Resolution::SkipAll,
                    'r' => {
                        // The worker stays blocked until the new name is
                        // submitted (or the prompt is cancelled).
                        self.pending_conflict = Some(reply);
                        let suggestion = match &c.dest {
                            Some(d) => crate::util::file_name(&ops::unique_name(d)),
                            None => String::new(),
                        };
                        self.open_input(InputKind::ConflictRename { job }, "New name", suggestion);
                        return;
                    }
                    _ => Resolution::Cancel,
                };
                let _ = reply.send(r);
            }
            ConfirmAction::DeleteForever { paths } => {
                if ch == 'y' {
                    let dest = self.tabs[self.active].cwd.clone();
                    self.submit_op(OpKind::Delete, paths, dest, true);
                    self.tabs[self.active].clear_selection();
                }
            }
            ConfirmAction::EndShell => {
                if ch == 'y' {
                    self.end_shell();
                }
            }
            ConfirmAction::PasteToTerm { text } => {
                if ch == 'y' {
                    if let Some(t) = &self.term {
                        t.paste(&text);
                    }
                }
            }
            ConfirmAction::Junctions { links } => match ch {
                'y' => self.make_junctions(links),
                'c' => self.copy_mklink(&links),
                _ => {}
            },
            ConfirmAction::BugReport { url } => match ch {
                'o' => self.open_report(url),
                'c' => {
                    match exec::set_clipboard(&url) {
                        Ok(()) => self.toast("Copied the bug report's link -- paste it into a browser"),
                        Err(e) => self.error(format!("Clipboard: {e}")),
                    }
                    self.last_report = Some(url);
                }
                _ => {}
            },
            ConfirmAction::Mcp { write, reply, expired } => {
                let r = if ch != 'y' {
                    Err("the user refused: not done".to_owned())
                } else if expired.load(std::sync::atomic::Ordering::Relaxed) {
                    Err("too late: the request had timed out, nothing was done".to_owned())
                } else {
                    self.run_mcp_write(write)
                };
                let _ = reply.send(r);
            }
            ConfirmAction::BookmarkDeleteAll => {
                if ch == 'y' {
                    self.bookmarks.clear();
                    self.save_state();
                    self.toast("All bookmarks deleted");
                }
            }
        }
    }
}
