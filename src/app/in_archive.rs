//! Browsing inside an archive as if it were a folder.

use super::*;

impl App {
    /// Measure what is under each child of the current directory and show them
    /// largest first.
    ///
    /// The same trick the search view uses: a `Folder` whose path is not a real
    /// directory, filled in as the worker answers. That is the whole reason this
    /// needs no overlay, no keymap layer and no `is_modal` entry -- `j`/`k`, the
    /// wheel, selection, `y`, `d` and `<Esc>` all work because this is the file
    /// list, not a panel pretending to be one.
    /// `l` on an archive: list it on a worker, and show its top level once
    /// the list arrives. A tar has no index and is read end to end, so even
    /// listing it can take a while.
    pub(super) fn open_archive(&mut self, archive: PathBuf) {
        let (tx, rx) = crossbeam_channel::bounded(1);
        let ctx = self.ctx.clone();
        let path = archive.clone();
        std::thread::Builder::new()
            .name("archive-list".into())
            .spawn(move || {
                let _ = tx.send(ArchiveMsg::Listed(path.clone(), archive::list(&path, ARCHIVE_LIMIT)));
                ctx.request_repaint();
            })
            .expect("spawn archive thread");
        self.archive_rx = Some(rx);
        let tab = &mut self.tabs[self.active];
        tab.remember_cursor();
        tab.current = Folder::loading(archive_path(&archive, ""), None);
        let scratch = preview_scratch(&archive);
        self.archive_view = Some(ArchiveView {
            archive,
            inner: String::new(),
            members: Arc::new(Vec::new()),
            more: false,
            copies: HashMap::new(),
            copying: None,
            refused: BTreeSet::new(),
            scratch,
        });
        self.preview.state = PreviewState::Empty;
        self.preview.key = None;
    }

    /// Put the level `inner` of the archive on screen, the cursor on `want`.
    pub(super) fn archive_show(&mut self, inner: String, want: Option<String>) {
        let Some(view) = &mut self.archive_view else { return };
        view.inner = inner;
        let base = view.inner.split('/').filter(|p| !p.is_empty()).fold(view.archive.clone(), |p, part| p.join(part));
        let entries: Vec<Entry> = archive::level(&view.members, &view.inner)
            .into_iter()
            .map(|row| Entry {
                path: base.join(&row.name),
                ext: (!row.dir)
                    .then(|| util::stem_and_ext(&row.name).1.trim_start_matches('.').to_ascii_lowercase())
                    .filter(|e| !e.is_empty()),
                hidden: row.name.starts_with('.'),
                kind: if row.dir { Kind::Dir } else { Kind::File },
                len: row.size,
                name: row.name,
                ..Default::default()
            })
            .collect();
        let path = archive_path(&view.archive, &view.inner);
        let tab = &mut self.tabs[self.active];
        let mut folder = Folder::from_entries(path, Arc::new(entries), tab.show_hidden);
        if let Some(name) = want {
            folder.select_name(&name);
        }
        tab.current = folder;
        tab.finder = None;
        self.preview.key = None;
        self.request_preview(true);
    }

    /// Drop the archive view, and with it any members still selected: they
    /// are not files, and outside the view `d` sent the paths to the trash,
    /// which took them without a word and was then said to have (#251).
    pub(super) fn leave_archive(&mut self) {
        if let Some(view) = self.archive_view.take() {
            self.tabs[self.active].selected.retain(|p| p == &view.archive || !p.starts_with(&view.archive));
        }
        self.archive_rx = None;
    }

    pub(super) fn archive_down(&mut self, name: &str) {
        let Some(view) = &self.archive_view else { return };
        let inner = match view.inner.is_empty() {
            true => name.to_owned(),
            false => format!("{}/{name}", view.inner),
        };
        self.archive_show(inner, None);
    }

    /// `l` or `<Enter>` on a file inside an archive: unpack that one member
    /// into a folder of kura's own under the temporary folder, on a worker,
    /// and open the copy with the system's default app. A copy, and said to
    /// be one: changes to it do not go back into the archive.
    pub(super) fn open_member(&mut self, entry: &Entry) {
        let Some(view) = &self.archive_view else { return };
        let member = match view.inner.is_empty() {
            true => entry.name.clone(),
            false => format!("{}/{}", view.inner, entry.name),
        };
        let archive = view.archive.clone();
        // The member's folders inside the archive are kept, so `a/readme.txt`
        // and `b/readme.txt` do not take turns at one copy (#251).
        let base = util::archive_scratch().join(util::file_name(&archive));
        let dest = view.inner.split('/').filter(|p| !p.is_empty()).fold(base, |p, part| p.join(part));
        let out = dest.join(&entry.name);
        let (tx, rx) = crossbeam_channel::bounded(1);
        let ctx = self.ctx.clone();
        std::thread::Builder::new()
            .name("archive-member".into())
            .spawn(move || {
                let got = archive::extract_one(&archive, &member, &dest, &mut |_, _| true).map(|()| out);
                let _ = tx.send(ArchiveMsg::Copied(got));
                ctx.request_repaint();
            })
            .expect("spawn archive thread");
        self.archive_rx = Some(rx);
        self.toast(format!("Unpacking {}…", entry.name));
    }

    pub(super) fn drain_archive(&mut self) {
        if let Some(Ok(ArchiveMsg::Previewed(fake, got))) = self.archive_preview_rx.as_ref().map(|rx| rx.try_recv()) {
            self.archive_preview_rx = None;
            if let Some(view) = &mut self.archive_view {
                view.copying = None;
                match got {
                    Ok(real) => {
                        view.copies.insert(fake, real);
                    }
                    Err(_) => {
                        view.refused.insert(fake);
                    }
                }
                // Whatever the cursor is on now: the one just unpacked, or
                // one it moved to while that was going.
                self.preview.key = None;
                self.request_preview(true);
            }
        }
        let Some(rx) = &self.archive_rx else { return };
        let Ok(msg) = rx.try_recv() else { return };
        self.archive_rx = None;
        match msg {
            ArchiveMsg::Listed(path, listed) => {
                // Left, or went into another, before the list came back.
                if self.archive_view.as_ref().is_none_or(|v| v.archive != path) {
                    return;
                }
                match listed {
                    Ok((members, more)) => {
                        if let Some(view) = &mut self.archive_view {
                            view.members = Arc::new(members);
                            view.more = more;
                        }
                        self.archive_show(String::new(), None);
                        if more {
                            self.toast(format!("Showing the first {ARCHIVE_LIMIT} entries of {}", util::file_name(&path)));
                        }
                    }
                    Err(e) => {
                        self.exit_search_view();
                        self.error(format!("{}: {e}", util::file_name(&path)));
                    }
                }
            }
            ArchiveMsg::Copied(Ok(path)) => {
                self.toasts.retain(|t| !t.text.starts_with("Unpacking "));
                match exec::open_default(&path) {
                    Ok(()) => self.toast(format!("Opened a copy of {} — changes stay out of the archive", util::file_name(&path))),
                    Err(e) => self.error(format!("Open failed: {e}")),
                }
            }
            ArchiveMsg::Previewed(..) => {}
            ArchiveMsg::Copied(Err(e)) => {
                self.toasts.retain(|t| !t.text.starts_with("Unpacking "));
                self.error(format!("Unpack failed: {e}"));
            }
        }
    }
}
