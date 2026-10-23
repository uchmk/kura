//! Comparing two files or folders: the diff view and its keys.

use super::*;

impl App {
    /// The two files to compare. With the view split that is what each pane is
    /// standing on, which is the whole reason the split exists; otherwise it is
    /// the two that are selected.
    pub(super) fn compare_pair(&self) -> Result<(PathBuf, PathBuf), String> {
        let hovered = |t: usize| self.tabs[t].current.hovered().map(|e| (e.path.clone(), e.kind));
        let (a, b) = match self.split {
            Some(sp) => {
                let (l, r) = match sp.right {
                    true => (sp.other, self.active),
                    false => (self.active, sp.other),
                };
                match (hovered(l), hovered(r)) {
                    (Some(a), Some(b)) => (a, b),
                    _ => return Err("both panes need a file".into()),
                }
            }
            None => {
                let sel = self.tabs[self.active].targets();
                if sel.len() != 2 {
                    return Err("split the view, or select exactly two files".into());
                }
                let of = |p: &PathBuf| {
                    let kind = self.tabs[self.active].current.entries.iter().find(|e| &e.path == p).map(|e| e.kind);
                    (p.clone(), kind.unwrap_or(Kind::File))
                };
                (of(&sel[0]), of(&sel[1]))
            }
        };
        // Two folders are compared as trees; two files line by line. One of each
        // is neither, and there is nothing sensible to show for it.
        if a.1.is_dir_like() != b.1.is_dir_like() {
            return Err("compare two files, or two folders — not one of each".into());
        }
        if a.0 == b.0 {
            return Err("that is the same file on both sides".into());
        }
        Ok((a.0, b.0))
    }

    pub(super) fn start_compare(&mut self) {
        let (left, right) = match self.compare_pair() {
            Ok(pair) => pair,
            Err(why) => return self.error(format!("Compare: {why}")),
        };
        self.differ.request(diff::Request { left: left.clone(), right: right.clone(), max_bytes: self.cfg.ui.max_text_bytes });
        self.overlay =
            Overlay::Diff(DiffOverlay { left, right, outcome: None, offset: 0, rows: 1, cursor: 0, hide_same: false, back: None });
    }

    /// The compare view's own commands: scroll, jump between differences, close.
    pub(super) fn diff_act(&mut self, a: Act) {
        let page = self.tabs[self.active].page_rows.max(1);
        let Overlay::Diff(ov) = &mut self.overlay else { return };
        // A tree comparison is a list with a cursor, so the same keys mean
        // something different there: `j` moves the selection rather than the
        // scroll, and `n`/`N` walk to the next path that is not a match.
        if matches!(&ov.outcome, Some(diff::Outcome::Tree { .. })) {
            let shown = ov.shown();
            let len = shown.len();
            let Some(diff::Outcome::Tree { rows, .. }) = &ov.outcome else { return };
            match a {
                Act::Close | Act::Escape(_) | Act::Quit | Act::Compare => self.overlay = Overlay::None,
                // The pair under the cursor, compared as files. Only a file on
                // both sides has a pair; anything else says why there is not.
                Act::Enter | Act::Open { .. } => {
                    let Some(row) = shown.get(ov.cursor).map(|&i| &rows[i]) else { return };
                    let why = match row.state {
                        diff::TreeState::LeftOnly | diff::TreeState::RightOnly => Some("it is on one side only"),
                        _ if row.dir => Some("that is a folder"),
                        _ => None,
                    };
                    if let Some(why) = why {
                        return self.error(format!("Compare: {why}"));
                    }
                    let (left, right) = (ov.left.join(&row.rel), ov.right.join(&row.rel));
                    let Overlay::Diff(tree) = std::mem::replace(&mut self.overlay, Overlay::None) else { return };
                    self.differ.request(diff::Request { left: left.clone(), right: right.clone(), max_bytes: self.cfg.ui.max_text_bytes });
                    self.overlay = Overlay::Diff(DiffOverlay {
                        left,
                        right,
                        outcome: None,
                        offset: 0,
                        rows: 1,
                        cursor: 0,
                        hide_same: false,
                        back: Some(Box::new(tree)),
                    });
                }
                // Hidden or shown, the cursor stays on the row it was on; when
                // that row is a match being hidden, on the next one that is not.
                Act::HideSame => {
                    let at = shown.get(ov.cursor).copied().unwrap_or(0);
                    ov.hide_same = !ov.hide_same;
                    let now = ov.shown();
                    ov.cursor = now.iter().position(|&i| i >= at).unwrap_or(now.len().saturating_sub(1));
                    let what = if ov.hide_same { "Hiding matching rows" } else { "Showing matching rows" };
                    self.toast_instead(&["Hiding matching rows", "Showing matching rows"], what.into());
                }
                Act::Copy(_) => {
                    let text = ov.shown_text();
                    let n = len;
                    match exec::set_clipboard(&text) {
                        Ok(()) if n == 1 => self.toast("Copied 1 row".to_string()),
                        Ok(()) => self.toast(format!("Copied {n} rows")),
                        Err(e) => self.error(format!("Clipboard: {e}")),
                    }
                }
                Act::Arrow(step) if len > 0 => ov.cursor = step.apply(ov.cursor, len, page),
                Act::FindArrow { prev } if len > 0 => {
                    let differs = |&i: &usize| !matches!(rows[i].state, diff::TreeState::Same);
                    let found = if prev {
                        shown[..ov.cursor].iter().rposition(differs)
                    } else {
                        shown[ov.cursor + 1..].iter().position(differs).map(|i| i + ov.cursor + 1)
                    };
                    match found {
                        Some(at) => ov.cursor = at,
                        None => {
                            let word = if prev { "first" } else { "last" };
                            self.toast_instead(&["At the "], format!("At the {word} difference"));
                        }
                    }
                }
                _ => {}
            }
            return;
        }
        let rows: &[diff::Row] = match &ov.outcome {
            Some(diff::Outcome::Rows { rows, .. }) => rows,
            _ => &[],
        };
        match a {
            // Back to the folder comparison it was opened from, if any.
            Act::Close | Act::Escape(_) => {
                self.overlay = match ov.back.take() {
                    Some(tree) => Overlay::Diff(*tree),
                    None => Overlay::None,
                }
            }
            Act::Quit | Act::Compare => self.overlay = Overlay::None,
            Act::Copy(_) => self.error("Copy: only a folder comparison has rows to copy".to_string()),
            // Against the last *scroll position*, not the last row. Clamping
            // to `len - 1` left `G` a screenful past where the pane can
            // actually sit, so the next `j` or `k` moved a number nothing was
            // drawing from and the keys looked dead -- for exactly as many
            // presses as the pane is tall, which is why a half-page `<C-u>`
            // appeared to wake them up.
            Act::Arrow(step) if !rows.is_empty() => {
                let stop = rows.len().saturating_sub(ov.rows.max(1)) + 1;
                ov.offset = step.apply(ov.offset, stop, page);
            }
            Act::FindArrow { prev } if !rows.is_empty() => match diff::next_change(rows, ov.offset, prev) {
                Some(at) => ov.offset = at,
                None => {
                    let word = if prev { "first" } else { "last" };
                    self.toast_instead(&["At the "], format!("At the {word} difference"));
                }
            },
            _ => {}
        }
    }
}
