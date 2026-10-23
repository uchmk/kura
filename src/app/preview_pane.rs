//! Asking for the preview, taking its answer, searching and outlining it.

use super::*;

impl App {
    pub fn request_preview(&mut self, force: bool) {
        // The debounce's timer, taken rather than read: every way out of this
        // function below leaves it cleared except the one that is still
        // waiting, which puts it back. `main` repaints every 16 ms while it is
        // set, and each early return used to leave it set for good -- an idle
        // window drawing 60 frames a second, minimised or not (#86).
        let pending = self.preview.pending_since.take();
        let Some(mut entry) = self.tabs[self.active].current.hovered().cloned() else {
            self.preview.in_flight = false;
            self.preview.state = PreviewState::Empty;
            self.preview.key = None;
            self.preview.outline = None;
            self.preview.outline_wanted = None;
            return;
        };
        // Inside an archive a small member is unpacked on a worker and its
        // copy previewed like any file; until then, and for a folder or a
        // big one, the card says what it is.
        let ctx = self.ctx.clone();
        if let Some(view) = &mut self.archive_view {
            match view.copies.get(&entry.path) {
                Some(real) => entry = Entry { path: real.clone(), ..entry },
                None => {
                    let wanted = !entry.is_dir_like()
                        && entry.len <= ARCHIVE_PREVIEW_LIMIT
                        && view.copying.is_none()
                        && !view.refused.contains(&entry.path);
                    if wanted {
                        let member = match view.inner.is_empty() {
                            true => entry.name.clone(),
                            false => format!("{}/{}", view.inner, entry.name),
                        };
                        let dest = view.inner.split('/').filter(|p| !p.is_empty()).fold(view.scratch.clone(), |p, part| p.join(part));
                        let (archive, fake) = (view.archive.clone(), entry.path.clone());
                        let real = dest.join(&entry.name);
                        let (tx, rx) = crossbeam_channel::bounded(1);
                        std::thread::Builder::new()
                            .name("archive-preview".into())
                            .spawn(move || {
                                let got = archive::extract_one(&archive, &member, &dest, &mut |_, _| true).map(|()| real);
                                let _ = tx.send(ArchiveMsg::Previewed(fake, got));
                                ctx.request_repaint();
                            })
                            .expect("spawn archive thread");
                        view.copying = Some(entry.path.clone());
                        self.archive_preview_rx = Some(rx);
                    }
                    self.preview.state = PreviewState::Ready(view.card(&entry));
                    self.preview.in_flight = false;
                    self.preview.key = None;
                    self.preview.texture = None;
                    return;
                }
            }
        }
        // The outline and the zoom belong to the file they were set on. Without
        // this, walking onto the next image shows a corner of it at 8x.
        let rule = crate::config::PreviewRule::for_path(&self.cfg.preview, &entry.path).cloned();
        if self.preview.key.as_ref().is_none_or(|k| k.path != entry.path) {
            self.preview.outline = None;
            self.preview.zoom = None;
            self.preview.pan = egui::Vec2::ZERO;
            self.preview.n = rule.as_ref().map_or(0, |r| r.first);
            self.preview.n_ok = None;
        }
        if self.preview.outline_wanted.as_ref().is_some_and(|p| *p != entry.path) {
            self.preview.outline_wanted = None;
        }

        if entry.is_dir_like() {
            self.preview.in_flight = false;
            let need = match &self.preview.state {
                PreviewState::Dir(f) => f.path != entry.path,
                _ => true,
            };
            if need {
                let show_hidden = self.tabs[self.active].show_hidden;
                let sort = self.tabs[self.active].sort;
                let folder = match self.cache.get(&entry.path) {
                    Some(entries) => Folder::from_entries(entry.path.clone(), entries.clone(), show_hidden),
                    None => {
                        let id = self.scanner.scan_low(entry.path.clone(), sort);
                        self.inflight.insert(id, entry.path.clone());
                        Folder::loading(entry.path.clone(), Some(id))
                    }
                };
                self.preview.state = PreviewState::Dir(folder);
                self.preview.key = None;
                self.preview.texture = None;
                self.tabs[self.active].preview_offset = 0;
                self.sync_watcher();
            }
            return;
        }

        let deep = self.body_search_open();
        let mime = crate::mime::guess(&entry);
        let key = preview::Key {
            path: entry.path.clone(),
            len: entry.len,
            mtime: entry.modified,
            box_size: self.preview.box_size,
            // Markdown, HTML and CSV are laid out to the pane's width, so both have to
            // be re-read when the window is resized; nothing else does. Leaving
            // CSV out here is not a small bug: `cols == 0` reads as 80 further
            // down, so the table would sit at a fixed 80 columns for ever and
            // never reflow.
            cols: if matches!(mime, "text/markdown" | "text/html" | "text/csv") { self.preview.cols } else { 0 },
            n: self.preview.n,
            deep,
        };
        if self.preview.key.as_ref() == Some(&key) && !force {
            return;
        }
        // A huge log that grew keeps its index and picks up the new lines in
        // `tick_log`; reading it again from the front would start over. The
        // pane's size does not matter to it (the window cuts the lines), and a
        // new view would lose the find that is running or done on this one.
        if let (PreviewState::Ready(Payload::Log { view }), Some(old)) = (&self.preview.state, &self.preview.key) {
            let same = old.path == key.path && old.deep == key.deep && old.n == key.n;
            if same && !force && key.len >= view.index.len() {
                self.preview.key = Some(key);
                return;
            }
        }
        if let Some(hit) = self.preview.cache.get(&key).cloned() {
            self.preview.in_flight = false;
            self.preview.key = Some(key);
            self.preview.texture = hit.texture;
            self.preview.state = PreviewState::Ready(hit.payload);
            self.preview.max_offset = 0;
            self.show_first_match();
            self.grant_outline_wish();
            return;
        }

        // Debounce: while the cursor is still moving, don't touch the disk.
        let debounce = Duration::from_millis(self.cfg.ui.preview_debounce_ms);
        match pending {
            Some(t) if t.elapsed() >= debounce => {}
            Some(t) => {
                self.preview.pending_since = Some(t);
                return;
            }
            None if !debounce.is_zero() => {
                self.preview.pending_since = Some(Instant::now());
                return;
            }
            None => {}
        }

        // A re-layout of the file already shown keeps it up until the new
        // one arrives, rather than blinking through "loading" on every resize.
        let relayout = self.preview.key.as_ref().is_some_and(|k| k.path == key.path && k.len == key.len && k.mtime == key.mtime)
            && matches!(
                self.preview.state,
                PreviewState::Ready(
                    // An image re-decoded for a zoom is the same picture at a
                    // different size: blinking through "loading" for it would
                    // be worse than a moment of the softer copy.
                    Payload::Text { .. } | Payload::Markdown { .. } | Payload::Image { .. }
                )
            );
        self.preview.key = Some(key.clone());
        if !relayout {
            self.preview.texture = None;
            self.preview.state = PreviewState::Loading;
        }
        self.preview.in_flight = true;
        self.preview.early = false;
        self.preview.placed = false;
        self.preview.request_id = self.previewer.request(preview::Request {
            id: 0,
            key,
            mime,
            ext: entry.ext.clone(),
            max_bytes: match deep {
                true => self.cfg.ui.max_text_bytes.max(preview::DEEP_BYTES),
                false => self.cfg.ui.max_text_bytes,
            },
            big_bytes: self.cfg.ui.big_text_bytes,
            tab_size: self.cfg.yazi.preview.tab_size,
            syntect_theme: self.cfg.theme.syntect_theme.clone(),
            markdown_rendered: self.render_markdown,
            preview: rule,
        });
    }

    pub(super) fn on_preview(&mut self, res: preview::Response, ctx: &egui::Context) {
        if self.preview.key.as_ref() != Some(&res.key) {
            return; // stale
        }
        if res.partial {
            // Shown while the rest is colored. Still in flight, and the scroll
            // position is kept for the whole one that replaces it.
            self.preview.state = PreviewState::Ready(res.payload);
            self.preview.early = true;
            // Every line is there, only the colours are to come, so the match
            // can be walked to now instead of when the last line is coloured.
            if self.body_search_open() {
                self.preview.max_offset = 0;
                self.show_first_match();
                self.preview.placed = true;
            }
            return;
        }
        self.preview.early = false;
        self.preview.in_flight = false;
        self.preview.texture = match &res.payload {
            Payload::Image { width, height, rgba, .. } => {
                let img = egui::ColorImage::from_rgba_unmultiplied([*width as usize, *height as usize], rgba);
                Some(ctx.load_texture("preview", img, egui::TextureOptions::LINEAR))
            }
            _ => None,
        };
        // Running off the end of a document stops, rather than replacing the
        // page with an error. `<A-j>` past the last page is the same gesture
        // as `j` at the bottom of the list, and that one simply does not move.
        // Nothing knows how many pages there are -- the command refusing *is*
        // the end -- so it is found by arriving at it, and then stepped back.
        if let (Payload::Error(why), Some(n_ok)) = (&res.payload, self.preview.n_ok) {
            if res.key.n != n_ok && self.is_external_preview(&res.key.path) {
                let said = why.clone();
                self.preview.n = n_ok;
                self.toast(format!("No more: {said}"));
                return;
            }
        }
        if !matches!(res.payload, Payload::Error(_)) && self.is_external_preview(&res.key.path) {
            self.preview.n_ok = Some(res.key.n);
        }
        self.preview.cache.put(res.key, CachedPreview { payload: res.payload.clone(), texture: self.preview.texture.clone() });
        // Restored: v0.5.0 replaced this line with the block below rather than
        // putting the block after it, and the answer has been going nowhere but
        // the cache ever since. The block reads `state` expecting it to be the
        // payload that just arrived, so it was dead too.
        self.preview.state = PreviewState::Ready(res.payload);
        self.preview.max_offset = 0;
        if let PreviewState::Ready(Payload::Image { source, .. }) = &self.preview.state {
            // A seed until the pane draws and says exactly: the box is twice the
            // pane in pixels, so half of it is roughly the points available.
            // Without this a zoom key pressed on this very frame would step from
            // whatever the last image's scale was.
            let (bw, bh) = self.preview.box_size;
            let avail = egui::vec2(bw as f32 / 2.0, bh as f32 / 2.0);
            self.preview.fit = image_fit(avail, source.0 as f32, source.1 as f32);
        }
        if !std::mem::take(&mut self.preview.placed) {
            self.show_first_match();
        }
        self.grant_outline_wish();
    }

    /// The lines of the text on show that the search view's body search finds,
    /// top to bottom. Empty when the view is not a body search or the preview
    /// is not text.
    /// Whether the list is the result of a content search (`S` / `F`), whose files
    /// are previewed as deep as the search read them.
    pub(super) fn body_search_open(&self) -> bool {
        self.tabs[self.active].finder.as_ref().is_some_and(|f| f.body) && self.in_search_view()
    }

    pub fn body_matches(&self) -> Vec<usize> {
        let Some(m) = self.tabs[self.active].finder.as_ref().filter(|f| f.body).and_then(|f| f.matcher.as_ref()) else {
            return Vec::new();
        };
        // Markdown is searched where it is drawn: the rendered text has no
        // markup characters, the source view does.
        let lines: Vec<&Vec<crate::preview::Span>> = match &self.preview.state {
            PreviewState::Ready(Payload::Text { lines, .. }) => lines.iter().collect(),
            PreviewState::Ready(Payload::Markdown { doc, .. }) if self.render_markdown => {
                return doc.marks(m, 0, doc.lines.len()).iter().enumerate().filter(|(_, r)| !r.is_empty()).map(|(i, _)| i).collect();
            }
            PreviewState::Ready(Payload::Markdown { source, .. }) => source.iter().collect(),
            _ => return Vec::new(),
        };
        lines
            .iter()
            .enumerate()
            .filter(|(_, l)| m.is_match(&l.iter().map(|s| s.text.as_str()).collect::<String>()))
            .map(|(i, _)| i)
            .collect()
    }

    /// A file that has just landed in the preview of a body search opens at
    /// its first match, a few lines down from the top so the lines before it
    /// show.
    pub(super) fn show_first_match(&mut self) {
        let ms = self.body_matches();
        let last = self.tabs[self.active].finder.as_mut().is_some_and(|f| std::mem::take(&mut f.last));
        if let Some(&line) = if last { ms.last() } else { ms.first() } {
            self.show_match(line);
        }
    }

    /// Scroll to a line of the preview that a body search found, and remember
    /// it so `n` / `N` go on from there.
    pub(super) fn show_match(&mut self, line: usize) {
        let tab = &mut self.tabs[self.active];
        tab.preview_offset = line.saturating_sub(3);
        if let Some(f) = tab.finder.as_mut() {
            f.at = Some(line);
        }
    }

    /// `n` / `N` in the result of `S` or `F`: the next / previous matching line
    /// of the file on show, and past its last / first one the next / previous
    /// file in the list. False when this is not such a view.
    pub(super) fn find_in_body(&mut self, prev: bool) -> bool {
        let Some(finder) = self.tabs[self.active].finder.clone() else { return false };
        if !finder.body || !self.in_search_view() {
            return false;
        }
        let Some(e) = self.tabs[self.active].current.hovered().cloned() else { return true };
        // A file crossed into is still on its way; going on now would step over
        // it on the old file's lines.
        let landed = self.preview.key.as_ref().is_some_and(|k| k.path == e.path) && !self.preview.in_flight;
        if !e.is_dir_like() && !landed {
            return true;
        }
        let ms = self.body_matches();
        let next = if prev {
            ms.iter().rev().find(|&&m| finder.at.is_none_or(|a| m < a))
        } else {
            ms.iter().find(|&&m| finder.at.is_none_or(|a| m > a))
        };
        if let Some(&line) = next {
            self.show_match(line);
            return true;
        }
        let tab = &mut self.tabs[self.active];
        let len = tab.current.view.len();
        if len <= 1 {
            // The one file there is: round again inside it.
            if let Some(&line) = if prev { ms.last() } else { ms.first() } {
                self.show_match(line);
            }
            return true;
        }
        let cursor = tab.current.cursor;
        tab.current.cursor = if prev { (cursor + len - 1) % len } else { (cursor + 1) % len };
        tab.sync_visual();
        if let Some(f) = tab.finder.as_mut() {
            f.at = None;
            f.last = prev;
        }
        true
    }

    /// The outline of the preview as it is shown: declarations in source
    /// code, or the headings of rendered Markdown.
    pub fn outline_entries(&self) -> &[TocEntry] {
        match &self.preview.state {
            PreviewState::Ready(Payload::Text { outline, .. }) => outline,
            PreviewState::Ready(Payload::Markdown { doc, .. }) if self.render_markdown => &doc.toc,
            _ => &[],
        }
    }

    /// The preview on screen is the hovered file's, fully loaded.
    /// Whether what the last key started has landed: the listing is read, the
    /// preview is up, no file job is still running, and an open spot panel or
    /// comparison has its answer. `--keys` waits on this between presses, as a
    /// person would wait to see.
    pub fn settled(&self) -> bool {
        let tab = &self.tabs[self.active];
        // A usage walk adds rows as it goes, so a key pressed mid-walk lands on
        // whatever row happened to be there; #122 typed 600 `k`s to wait one out.
        // A job says how it went in a toast when it finishes, so `u<Shot:x>`
        // pictured `Restore 0%` and missed the toast it was taken for (#168).
        // A row that wants the job mid-run says `<Now>`, as 12.14 does. A job
        // asking about a name already taken waits on the next key, not the
        // other way round.
        let job_going = self.tasks.iter().any(|t| matches!(t.state, TaskState::Queued | TaskState::Running));
        if self.usage.is_some() || matches!(tab.current.state, LoadState::Loading)
            || (job_going && !matches!(self.overlay, Overlay::Confirm(_)))
            || self.preview.pending_since.is_some()
            || self.preview.in_flight
            || matches!(self.preview.state, PreviewState::Loading)
            // A job that made something puts the cursor on it when the
            // listing is read again; until then `hovered:` names the row
            // before (#254: `e<State:>` read the archive, not what it made).
            || self.land_on.as_ref().is_some_and(|p| p.parent() == Some(tab.cwd.as_path()))
        {
            return false;
        }
        match &self.overlay {
            Overlay::Spot(_) => {
                let hovered = tab.current.hovered().map(|e| &e.path);
                hovered.is_none() || self.spotted.as_ref().map(|(p, _)| p) == hovered
            }
            Overlay::Diff(ov) => ov.outcome.is_some(),
            _ => true,
        }
    }

    pub(super) fn preview_ready(&self) -> bool {
        let hovered = self.tabs[self.active].current.hovered().map(|e| &e.path);
        matches!(self.preview.state, PreviewState::Ready(_)) && self.preview.key.as_ref().map(|k| &k.path) == hovered
    }

    /// Hand the keys to the outline, starting on the entry being read.
    /// Returns whether there was an outline to hand them to.
    pub(super) fn focus_outline(&mut self) -> bool {
        if !self.preview_ready() {
            return false;
        }
        let top = self.tabs[self.active].preview_offset;
        let entries = self.outline_entries();
        if entries.is_empty() {
            return false;
        }
        self.preview.outline = Some(entries.iter().rposition(|e| e.line <= top).unwrap_or(0));
        true
    }

    pub(super) fn toggle_outline(&mut self) {
        if self.preview.outline.take().is_some() {
            return;
        }
        if !self.focus_outline() {
            self.toast("No outline for this file");
        }
    }

    /// `enter` on a file whose preview had not loaded yet: now that it has,
    /// move into its outline if it has one.
    pub(super) fn grant_outline_wish(&mut self) {
        let wanted = self.preview.outline_wanted.take();
        if wanted.is_some() && wanted.as_ref() == self.preview.key.as_ref().map(|k| &k.path) {
            self.focus_outline();
        }
    }

    /// The 1-based source line outline entry `k` points at, for an editor.
    pub(crate) fn outline_source_line(&self, k: usize) -> Option<usize> {
        let e = self.outline_entries().get(k)?;
        let line = match &self.preview.state {
            PreviewState::Ready(Payload::Markdown { doc, .. }) => doc.src_for_line(e.line),
            _ => e.line,
        };
        Some(line + 1)
    }

    /// While the outline has the keys, moves go through it and scroll the
    /// preview along, and open starts the editor at the entry. Escape and
    /// leave hand the keys back; anything else not about the preview hands
    /// them back and then runs as usual. Returns whether the action was used
    /// up.
    pub(super) fn outline_act(&mut self, a: &Act) -> bool {
        let Some(cursor) = self.preview.outline else { return false };
        let entries = self.outline_entries();
        if entries.is_empty() {
            self.preview.outline = None;
            return false;
        }
        match a {
            Act::Arrow(step) => {
                let page = self.tabs[self.active].page_rows.max(1);
                let k = step.apply(cursor.min(entries.len() - 1), entries.len(), page);
                let line = entries[k].line;
                self.preview.outline = Some(k);
                // The same ceiling `seek` takes, and for the same reason: an
                // entry near the end of a file sits past the furthest the pane
                // can be scrolled, so jumping to it handed the draw an offset
                // beyond the content. The draw paints that frame and corrects
                // the number afterwards, which is one wrong frame per press --
                // invisible on a tap, and a flicker under a held key. This
                // path was missed when `seek` was clamped, which is why it
                // survived: it only shows on the last entry or two, and only
                // in a file whose last heading is near the end.
                self.tabs[self.active].preview_offset = line.min(self.preview.max_offset);
                true
            }
            Act::Seek(_) | Act::MaxPreview | Act::MaxTerm => false,
            // Already in the outline.
            Act::Enter => true,
            Act::Open { interactive, .. } => {
                let line = self.outline_source_line(cursor.min(entries.len() - 1));
                self.preview.outline = None;
                self.open(*interactive, line);
                true
            }
            Act::Escape(_) | Act::ToggleOutline | Act::Leave => {
                self.preview.outline = None;
                true
            }
            _ => {
                self.preview.outline = None;
                false
            }
        }
    }
}
