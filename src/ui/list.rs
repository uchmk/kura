//! The file list column.
//!
//! Rows are painted directly rather than built from widgets: only the visible
//! window is touched, so a directory with 200k entries costs the same as one
//! with twenty.

use egui::text::{LayoutJob, TextFormat};
use egui::{Align2, Color32, CornerRadius, FontId, Rect, Stroke, Ui, Vec2};

use crate::config::theme::{Style, Theme};
use crate::core::folder::{Folder, LoadState};
use crate::fs::Entry;
use crate::util;

pub struct ListStyle<'a> {
    pub theme: &'a Theme,
    pub lang: &'a str,
    pub font: FontId,
    pub row_h: f32,
    /// Dimmed columns (parent / preview) get a quieter cursor.
    pub active: bool,
    pub linemode: crate::fs::entry::Linemode,
    pub sort: Option<crate::fs::SortSpec>,
    /// The largest total in the disk-usage view, which every row's bar is drawn
    /// against. Zero everywhere else, and no bar is drawn.
    pub usage_max: u64,
    /// The folder a fuzzy search started from: its letters run over the path
    /// below it, so the highlight has to be worked out over the same text.
    pub search_root: Option<&'a std::path::Path>,
}

pub struct RowFlags {
    pub selected: bool,
    pub yanked: Option<bool>, // Some(true) = cut
    pub git: crate::fs::git::State,
}

pub struct ListResult {
    pub clicked: Option<usize>,
    pub double_clicked: Option<usize>,
    /// Right-click, which opens the context menu on that row.
    pub secondary_clicked: Option<usize>,
    /// The row a drag began on, the frame it began.
    pub drag_started: Option<usize>,
    /// The drag that began here has been let go, wherever the pointer is now.
    pub drag_stopped: bool,
    /// Wheel movement over this pane, in rows; the caller keeps the
    /// remainder, since it is the one that lives between frames.
    pub scroll_rows: f32,
    /// Modifiers held down for the click above.
    pub mods: egui::Modifiers,
    /// Each row's name as drawn, cut to the column, top to bottom.
    pub shown: Vec<String>,
}

pub fn draw(
    ui: &mut Ui,
    rect: Rect,
    folder: &Folder,
    st: &ListStyle<'_>,
    flags: &dyn Fn(&Entry) -> RowFlags,
    hits: bool,
    find: Option<&ito_match::Matcher>,
) -> ListResult {
    let painter = ui.painter_at(rect);
    let details = st.linemode == crate::fs::entry::Linemode::All;
    let cell_width = painter.layout_no_wrap("M".into(), st.font.clone(), st.theme.fg).size().x;
    let columns = if details { detail_columns(rect.width(), cell_width) } else { Vec::new() };
    if details {
        let name_width = columns.first().map_or(rect.width(), |c| c.1);
        let header = |col: usize| {
            use crate::fs::SortBy as S;
            let active = st.sort.is_some_and(|s| match col {
                0 => matches!(s.by, S::Alphabetical | S::Natural),
                1 => s.by == S::Mtime,
                2 => s.by == S::Btime,
                3 => s.by == S::Atime,
                4 => s.by == S::Type,
                5 => s.by == S::Size,
                6 => s.by == S::Dimensions,
                7 => s.by == S::Owner,
                8 => s.by == S::Permissions,
                9 => s.by == S::Extension,
                10 => s.by == S::Usage,
                _ => false,
            });
            let text = crate::i18n::tr(st.lang, DETAIL_KEYS[col]);
            if active {
                format!("{} {text}", if st.sort.is_some_and(|s| s.reverse) { "▼" } else { "▲" })
            } else {
                text
            }
        };
        for (col, x, width) in std::iter::once((0, 0.0, name_width)).chain(columns.iter().copied()) {
            let cell = Rect::from_min_size(egui::pos2(rect.left() + x + 4.0, rect.top()), Vec2::new((width - 8.0).max(0.0), st.row_h));
            painter.with_clip_rect(cell).text(cell.left_center(), Align2::LEFT_CENTER, header(col), st.font.clone(), st.theme.fg_dim);
        }
    }
    let rect = if details { Rect::from_min_max(rect.min + Vec2::new(0.0, st.row_h), rect.max) } else { rect };
    let rows = ((rect.height() / st.row_h).floor() as usize).max(1);
    let mut out = ListResult {
        clicked: None,
        double_clicked: None,
        secondary_clicked: None,
        drag_started: None,
        drag_stopped: false,
        scroll_rows: 0.0,
        mods: egui::Modifiers::NONE,
        shown: Vec::new(),
    };

    match &folder.state {
        LoadState::Error(e) => {
            painter.text(rect.left_top() + Vec2::new(8.0, 6.0), Align2::LEFT_TOP, e, st.font.clone(), st.theme.progress_error);
            return out;
        }
        LoadState::Loading if folder.entries.is_empty() => {
            painter.text(rect.left_top() + Vec2::new(8.0, 6.0), Align2::LEFT_TOP, "…", st.font.clone(), st.theme.fg_dim);
            return out;
        }
        _ => {}
    }

    if folder.view.is_empty() {
        painter.text(rect.left_top() + Vec2::new(8.0, 6.0), Align2::LEFT_TOP, empty_label(&folder.path), st.font.clone(), st.theme.fg_dim);
        return out;
    }

    let start = folder.offset.min(folder.view.len().saturating_sub(1));
    let end = (start + rows).min(folder.view.len());

    for (i, row) in (start..end).enumerate() {
        let Some(entry) = folder.at(row) else { continue };
        let y = rect.top() + i as f32 * st.row_h;
        let row_rect = Rect::from_min_size(egui::pos2(rect.left(), y), Vec2::new(rect.width(), st.row_h));
        let f = flags(entry);
        let hovered = row == folder.cursor;

        if hovered {
            let bg = if st.active { st.theme.hovered_bg } else { st.theme.inactive_hovered_bg };
            painter.rect_filled(row_rect, CornerRadius::same(3), bg);
        }

        // Marker bar: selection state first, then the yank register.
        let marker = if f.selected {
            Some(st.theme.marker_selected)
        } else {
            f.yanked.map(|cut| if cut { st.theme.marker_cut } else { st.theme.marker_copied })
        };
        if let Some(color) = marker {
            painter.rect_filled(
                Rect::from_min_size(row_rect.left_top() + Vec2::new(1.0, 2.0), Vec2::new(3.0, st.row_h - 4.0)),
                CornerRadius::same(2),
                color,
            );
        }

        let mime = crate::mime::guess(entry);
        let style = st.theme.style_for(entry, mime);
        let base_color = style.fg.unwrap_or(st.theme.fg);
        let icon = st.theme.icon_for(entry);
        let mut x = row_rect.left() + 8.0;

        // An all-blank icon still holds the column: the ASCII fallback draws
        // plain files as a *space* (`Theme::without_nerd_icons`), and trimming
        // first would have dropped that column, starting file names one place
        // to the left of the folders' on the same list.
        if !icon.text.is_empty() {
            let g = painter.layout_no_wrap(icon.text.clone(), st.font.clone(), icon.fg.unwrap_or(base_color));
            // Nerd Font glyphs differ in advance width, so they are padded to a
            // full em to line the names up. ASCII icons are monospace and line
            // up on their own; padding *those* to an em opens a gap wide enough
            // to read as indentation -- `/      config` instead of `/ config`.
            let w = if icon.text.is_ascii() { g.size().x } else { g.size().x.max(st.font.size) };
            painter.galley(egui::pos2(x, y + (st.row_h - g.size().y) / 2.0), g, base_color);
            x += w + 8.0;
        }

        // Right-hand line mode text is laid out first so the name knows its budget.
        let right = linemode_text(entry, st.linemode);
        let mut right_w = 0.0;
        if details {
            for &(col, offset, width) in &columns {
                let cell = Rect::from_min_size(egui::pos2(rect.left() + offset + 4.0, y), Vec2::new(width - 8.0, st.row_h));
                let text = detail_text(entry, col, st.lang);
                let cp = painter.with_clip_rect(cell);
                cp.text(
                    if col == 5 { cell.right_center() } else { cell.left_center() },
                    if col == 5 { Align2::RIGHT_CENTER } else { Align2::LEFT_CENTER },
                    text,
                    st.font.clone(),
                    st.theme.fg_dim,
                );
                if col == 10 && entry.usage.is_some() && st.usage_max > 0 {
                    let track = Rect::from_min_size(cell.left_bottom() - Vec2::new(0.0, 3.0), Vec2::new(cell.width(), 2.0));
                    cp.rect_filled(track, CornerRadius::ZERO, st.theme.border);
                    cp.rect_filled(
                        Rect::from_min_size(
                            track.min,
                            Vec2::new(track.width() * (entry.usage_bytes() as f32 / st.usage_max as f32).min(1.0), 2.0),
                        ),
                        CornerRadius::ZERO,
                        st.theme.progress_fg,
                    );
                }
            }
            right_w = columns.first().map_or(0.0, |c| rect.width() - c.1);
        }
        // The usage view's bar: the row's share of the biggest row, so the
        // largest folder fills it and the rest are read against that. Two
        // rectangles, the same pair the tasks panel draws its progress with.
        if st.usage_max > 0 && !details {
            let frac = entry.usage_bytes() as f32 / st.usage_max as f32;
            let w = 44.0;
            let track =
                egui::Rect::from_min_size(egui::pos2(row_rect.right() - w - 6.0, y + (st.row_h - 4.0) / 2.0), egui::Vec2::new(w, 4.0));
            painter.rect_filled(track, egui::CornerRadius::same(2), st.theme.border);
            if frac > 0.0 {
                painter.rect_filled(
                    egui::Rect::from_min_size(track.min, egui::Vec2::new(w * frac.min(1.0), 4.0)),
                    egui::CornerRadius::same(2),
                    st.theme.progress_fg,
                );
            }
            right_w = w + 12.0;
        }
        if !right.is_empty() && !details {
            let g = painter.layout_no_wrap(right.clone(), st.font.clone(), st.theme.fg_dim);
            let g_w = g.size().x + 10.0;
            painter.galley(
                egui::pos2(row_rect.right() - right_w - g.size().x - 6.0, y + (st.row_h - g.size().y) / 2.0),
                g,
                st.theme.fg_dim,
            );
            right_w += g_w;
        }

        // The git sign sits between the name and the line mode, so it stays
        // put as the name grows and the two never collide.
        if let Some(mark) = f.git.mark() {
            let color = st.theme.git_color(f.git);
            let g = painter.layout_no_wrap(mark.to_string(), st.font.clone(), color);
            let w = g.size().x;
            painter.galley(egui::pos2(row_rect.right() - right_w - w - 2.0, y + (st.row_h - g.size().y) / 2.0), g, color);
            right_w += w + 8.0;
        }

        let avail = (row_rect.right() - right_w - x - 6.0).max(if details { 0.0 } else { 16.0 });
        // The filter's hits are worked out when the list is built; `/`, `?`
        // and the search views mark what they look for as the row is drawn.
        let found;
        let positions: &[usize] = match (hits, find) {
            (true, _) if !folder.hit_at(row).is_empty() => folder.hit_at(row),
            (_, Some(m)) => {
                found = match st.search_root {
                    Some(root) if m.is_fuzzy() => path_hits(m, &entry.path, root, &entry.name),
                    _ => m.positions(&entry.name),
                };
                &found
            }
            (true, None) => folder.hit_at(row),
            (false, None) => &[],
        };
        // Too long for the row: cut inside the stem so the extension stays
        // readable (Q34, 24.2), and move the search hits along with the text.
        let width = |s: &str| painter.layout_no_wrap(s.to_owned(), st.font.clone(), base_color).size().x;
        let full = entry_name(&entry.name, &entry.kind);
        let (name, positions) = match elide_at(&full, avail, &width) {
            Some(cut) => {
                let near: Vec<&str> = [row.checked_sub(1), Some(row + 1)]
                    .into_iter()
                    .flatten()
                    .filter_map(|r| folder.at(r))
                    .map(|e| e.name.as_str())
                    .collect();
                elided(&full, positions, apart(&full, cut, &near))
            }
            None => (full, positions.to_vec()),
        };
        out.shown.push(name.clone());
        let job = name_job(&name, &positions, &st.font, base_color, &style, st.theme, avail);
        let galley = painter.layout_job(job);
        let name_rect = Rect::from_min_max(row_rect.min, egui::pos2((row_rect.right() - right_w).max(row_rect.left()), row_rect.bottom()));
        painter.with_clip_rect(name_rect).galley(egui::pos2(x, y + (st.row_h - galley.size().y) / 2.0), galley, base_color);
    }

    // Scrollbar hint when the list overflows.
    if folder.view.len() > rows {
        let track = Rect::from_min_size(egui::pos2(rect.right() - 3.0, rect.top()), Vec2::new(2.0, rect.height()));
        let frac = rows as f32 / folder.view.len() as f32;
        let h = (track.height() * frac).max(16.0);
        let t = folder.offset as f32 / (folder.view.len() - rows) as f32;
        let y = track.top() + (track.height() - h) * t;
        painter.rect_filled(Rect::from_min_size(egui::pos2(track.left(), y), Vec2::new(2.0, h)), CornerRadius::same(1), st.theme.border);
    }

    // Interaction
    let id = ui.id().with(("list", rect.left() as i32, rect.top() as i32));
    let resp = ui.interact(rect, id, egui::Sense::click_and_drag());
    // `hover_pos` is the fallback: a press that egui reports without an
    // interaction position still names the row the pointer is over.
    if let Some(pos) = resp.interact_pointer_pos().or_else(|| resp.hover_pos()) {
        let row = start + (((pos.y - rect.top()) / st.row_h).floor().max(0.0) as usize);
        if row < end {
            if resp.double_clicked() {
                out.double_clicked = Some(row);
            } else if resp.clicked() {
                out.clicked = Some(row);
            } else if resp.secondary_clicked() {
                out.secondary_clicked = Some(row);
            }

            if out.clicked.is_some() || out.double_clicked.is_some() {
                out.mods = ui.ctx().input(|i| i.modifiers);
            }
        }
    }
    // The row the button went down on, not the one under the pointer now:
    // egui calls it a drag only once the pointer has moved, and a quick pull
    // is rows away by then -- or past the last row, which started nothing.
    if resp.drag_started() {
        if let Some(at) = ui.ctx().input(|i| i.pointer.press_origin()) {
            let row = start + (((at.y - rect.top()) / st.row_h).floor().max(0.0) as usize);
            if row < end {
                out.drag_started = Some(row);
            }
        }
    }
    // Let go anywhere, not just over the row it started on.
    out.drag_stopped = resp.drag_stopped();
    if ui.rect_contains_pointer(rect) {
        let scroll = ui.ctx().input(|i| i.smooth_scroll_delta.y);
        out.scroll_rows = -scroll / st.row_h * 1.5;
    }
    out
}

/// The name as the row shows it: a link says so after its name.
fn entry_name(name: &str, kind: &crate::fs::Kind) -> String {
    match kind {
        crate::fs::Kind::Link { .. } => format!("{name}  ->"),
        _ => name.to_owned(),
    }
}

/// What an empty listing says. A host with no shares is a different answer
/// from a folder with no files, and `(empty)` read as the enumeration still
/// being out (#105).
fn empty_label(path: &std::path::Path) -> &'static str {
    if util::host_only_unc(path) {
        "(no shares)"
    } else {
        "(empty)"
    }
}

/// Where to cut a name that is wider than `max_width` (Q34): keep its first
/// `head` characters, then `…`, then everything from `tail` on -- the
/// extension and the last third of what is kept of the stem, so that
/// `report-2026-final.pdf` and `report-2026-final.docx` stay apart.
///
/// `None` when the name fits, and when it has no stem to speak of or the
/// extension is most of it; those are left to the plain cut at the end.
fn elide_at(name: &str, max_width: f32, width: &dyn Fn(&str) -> f32) -> Option<(usize, usize)> {
    if width(name) <= max_width {
        return None;
    }
    let chars: Vec<char> = name.chars().collect();
    let n = chars.len();
    let stem = crate::util::stem_and_ext(name).0.chars().count();
    let ext = n - stem;
    if stem < 2 || ext * 2 > n {
        return None;
    }
    // `keep` characters of the stem, two thirds at the front.
    let cut = |keep: usize| {
        let back = keep / 3;
        (keep - back, n - ext - back)
    };
    let shown =
        |(head, tail): (usize, usize)| -> String { chars[..head].iter().chain(['…'].iter()).chain(chars[tail..].iter()).collect() };
    let (mut lo, mut hi) = (1, stem - 1);
    if width(&shown(cut(lo))) > max_width {
        return None;
    }
    while lo < hi {
        let mid = (lo + hi).div_ceil(2);
        match width(&shown(cut(mid))) <= max_width {
            true => lo = mid,
            false => hi = mid - 1,
        }
    }
    Some(cut(lo))
}

/// `cut` moved, within the same number of kept characters, so that `name`
/// no longer reads the same as a neighbouring row's name (Q67, #227: a
/// column of `notes…84` with the cursor on one of them). More of the head
/// is kept up to the first character the two differ in; failing that, more
/// of the tail. A neighbour that already reads differently, or one the
/// budget cannot tell apart, leaves the cut as it was.
fn apart(name: &str, (head, tail): (usize, usize), neighbours: &[&str]) -> (usize, usize) {
    let chars: Vec<char> = name.chars().collect();
    let n = chars.len();
    let ext = n - crate::util::stem_and_ext(name).0.chars().count();
    let keep = head + (n - ext - tail);
    let (mut head, mut back) = (head, n - ext - tail);
    for other in neighbours {
        let o: Vec<char> = other.chars().collect();
        let same = |head: usize, back: usize| {
            let tail = n - ext - back;
            o.len() >= head + (n - tail) && o[..head] == chars[..head] && o[o.len() - (n - tail)..] == chars[tail..]
        };
        if *other == name || !same(head, back) {
            continue;
        }
        let front = chars.iter().zip(&o).take_while(|(a, b)| a == b).count();
        let end = chars.iter().rev().zip(o.iter().rev()).take_while(|(a, b)| a == b).count();
        if front < keep && front + 1 > head {
            (head, back) = (front + 1, keep - front - 1);
        } else if end >= ext && end - ext < keep && end - ext + 1 > back {
            (head, back) = (keep - (end - ext + 1), end - ext + 1);
        }
    }
    (head, n - ext - back)
}

/// `name` cut where `elide_at` said, with the search hits that survived the
/// cut moved to where their characters now are.
fn elided(name: &str, hits: &[usize], (head, tail): (usize, usize)) -> (String, Vec<usize>) {
    let chars: Vec<char> = name.chars().collect();
    let text = chars[..head].iter().chain(['…'].iter()).chain(chars[tail..].iter()).collect();
    let moved = hits
        .iter()
        .filter_map(|&i| match i {
            i if i < head => Some(i),
            i if i >= tail => Some(head + 1 + (i - tail)),
            _ => None,
        })
        .collect();
    (text, moved)
}

/// Where a fuzzy search's letters fall in the name, when it matched them over
/// the path below `root` (`src/main.rs` for `srcmain`): the hits in the
/// directory part have no place in the name and are dropped.
fn path_hits(m: &ito_match::Matcher, path: &std::path::Path, root: &std::path::Path, name: &str) -> Vec<usize> {
    let rel = path.strip_prefix(root).unwrap_or(path).to_string_lossy().replace('\\', "/");
    let shift = rel.chars().count().saturating_sub(name.chars().count());
    m.positions(&rel).into_iter().filter_map(|i| i.checked_sub(shift)).collect()
}

fn name_job(name: &str, positions: &[usize], font: &FontId, color: Color32, style: &Style, theme: &Theme, max_width: f32) -> LayoutJob {
    let mut job = LayoutJob::default();
    job.wrap.max_width = max_width;
    job.wrap.max_rows = 1;
    job.wrap.break_anywhere = true;
    job.wrap.overflow_character = Some('…');

    let fmt = |c: Color32, bg: Color32, underline: bool| TextFormat {
        font_id: font.clone(),
        color: c,
        background: bg,
        italics: style.italic,
        underline: if underline { Stroke::new(1.0, c) } else { Stroke::NONE },
        ..Default::default()
    };

    if positions.is_empty() {
        job.append(name, 0.0, fmt(color, Color32::TRANSPARENT, style.underline));
        return job;
    }

    let hl_fg = theme.find_keyword.fg.unwrap_or(color);
    let hl_bg = theme.find_keyword.bg.unwrap_or(Color32::TRANSPARENT);
    let mut buf = String::new();
    let mut buf_hl = false;
    for (i, ch) in name.chars().enumerate() {
        let is_hl = positions.contains(&i);
        if is_hl != buf_hl && !buf.is_empty() {
            let f = if buf_hl { fmt(hl_fg, hl_bg, false) } else { fmt(color, Color32::TRANSPARENT, style.underline) };
            job.append(&buf, 0.0, f);
            buf.clear();
        }
        buf_hl = is_hl;
        buf.push(ch);
    }
    if !buf.is_empty() {
        let f = if buf_hl { fmt(hl_fg, hl_bg, false) } else { fmt(color, Color32::TRANSPARENT, style.underline) };
        job.append(&buf, 0.0, f);
    }
    job
}

pub fn linemode_text(entry: &Entry, mode: crate::fs::entry::Linemode) -> String {
    use crate::fs::entry::Linemode as L;
    match mode {
        L::Size => entry.display_size().unwrap_or_default(),
        // The usage view's own mode: the measured total, files included, so a
        // folder and a file read on the same scale.
        // A folder nothing has measured -- `m u` outside `gu`'s view -- is
        // blank, as `size` leaves it: `0 B` read as an empty folder (#122).
        L::Usage if entry.usage.is_none() && entry.is_dir_like() => String::new(),
        L::Usage => match (entry.usage_cut, entry.usage_bytes()) {
            (true, 0) => "?".to_owned(),
            (true, n) => format!("≥ {}", crate::util::human_size(n)),
            (false, n) => crate::util::human_size(n),
        },
        L::Mtime => util::fmt_time(entry.modified, "%Y-%m-%d %H:%M"),
        L::Btime => util::fmt_time(entry.created, "%Y-%m-%d %H:%M"),
        L::Permissions => permissions(entry),
        L::Owner => entry.details.as_ref().map(|d| d.owner.clone()).unwrap_or_default(),
        L::All | L::None => String::new(),
    }
}

pub use crate::fs::details::permissions;

/// The icon column, which is the one part of a row that has no words in it.
///
/// Reported from a screenshot rather than from [TESTING.md]: with no Nerd Font
/// installed the list read `/      config`, a gap wide enough that the folders
/// looked indented under the files. Nothing in the drawn strings says so, which
/// is what `Painted::placed` is for.
///
/// [TESTING.md]: ../../TESTING.md
#[cfg(test)]
mod icon_column {
    use crate::ui::harness::Screen;

    /// kura's config as it comes out on a machine with no icon font: what
    /// `apply_fonts` does to the theme when `install_fonts` finds no Nerd Font.
    fn without_icon_font() -> crate::config::Config {
        let mut cfg = crate::config::Config::load();
        std::sync::Arc::make_mut(&mut cfg.theme).without_nerd_icons();
        cfg
    }

    /// A folder and a plain file, listed in that order.
    fn one_of_each(label: &str) -> (std::path::PathBuf, std::sync::Arc<Vec<crate::fs::Entry>>) {
        let dir = crate::util::test_dir(label);
        std::fs::create_dir_all(dir.join("config")).unwrap();
        std::fs::write(dir.join("notes.txt"), "1").unwrap();
        let entries = std::sync::Arc::new(vec![
            crate::fs::Entry::from_path(dir.join("config")).unwrap(),
            crate::fs::Entry::from_path(dir.join("notes.txt")).unwrap(),
        ]);
        (dir, entries)
    }

    fn listing(cfg: crate::config::Config, dir: &std::path::Path, entries: std::sync::Arc<Vec<crate::fs::Entry>>) -> Screen {
        let mut s = Screen::with_config(cfg, dir.to_path_buf());
        s.app.tabs[s.app.active].current = crate::core::folder::Folder::from_entries(dir.to_path_buf(), entries, true);
        s
    }

    /// The ASCII fallback's icons are one monospace character, so the column is
    /// one character wide -- not one em, which is nearly twice that.
    ///
    /// The width is measured rather than asserted against a number: a second
    /// frame widens the folder's icon to `//`, and the distance the name moves
    /// is exactly what one character costs in whatever font the test ran with.
    #[test]
    fn an_ascii_icon_takes_one_character_of_room() {
        let (dir, entries) = one_of_each("icon-column");

        let one = listing(without_icon_font(), &dir, entries.clone()).draw();
        let (icon, name) = (one.placed("/").expect("the folder's fallback icon is drawn"), one.placed("config").expect("and its name"));

        let mut wider = without_icon_font();
        std::sync::Arc::make_mut(&mut wider.theme).icon_dir_default.text = "//".into();
        let two = listing(wider, &dir, entries).draw();
        let per_char = two.placed("config").expect("the name moves right").x - name.x;
        assert!(per_char > 0.0, "widening the icon by a character moved the name: {per_char}");

        // One character, then the 8px pad the columns are separated by. What the
        // bug put in between was the difference between a character's advance
        // and a full em -- a monospace character is about 0.6 em, so the gap
        // came out around half again as wide as it should be.
        let gap = name.x - icon.x;
        assert!(
            (gap - (per_char + 8.0)).abs() < 0.5,
            "the name sits one character plus the pad past the icon: \
             gap {gap}, character {per_char}",
        );
    }

    /// Files and folders start their names in the same place.
    ///
    /// `without_nerd_icons` gives plain files a *space* rather than an empty
    /// string, on purpose: the column is still theirs. Trimming before asking
    /// whether there was an icon threw that away and left the file names one
    /// place to the left of the folders' on the same list.
    #[test]
    fn a_blank_icon_still_holds_its_column() {
        let (dir, entries) = one_of_each("icon-blank");
        let f = listing(without_icon_font(), &dir, entries).draw();

        let folder = f.placed("config").expect("the folder's name");
        let file = f.placed("notes.txt").expect("the file's name");
        assert!((folder.x - file.x).abs() < 0.01, "both names start at the same x: folder {}, file {}", folder.x, file.x,);
    }
}

/// Q34 / 24.2: a long name is cut inside its stem, never through its
/// extension. Widths here are one per character, which is enough to pin where
/// the cut goes; the renderer passes the font's own measure.
#[cfg(test)]
mod elision {
    use super::{apart, elide_at, elided};

    fn chars(s: &str) -> f32 {
        s.chars().count() as f32
    }

    fn shown(name: &str, max: f32) -> String {
        match elide_at(name, max, &chars) {
            Some(cut) => elided(name, &[], cut).0,
            None => name.to_owned(),
        }
    }

    #[test]
    fn the_extension_survives_the_cut() {
        let long = format!("very-{}name.txt", "long-".repeat(30));
        let s = shown(&long, 30.0);
        assert!(s.ends_with("name.txt"), "the end of the stem and the extension: {s}");
        assert!(s.starts_with("very-long-"), "and the start: {s}");
        assert!(s.contains('…'));
        assert!(s.chars().count() <= 30, "{} chars: {s}", s.chars().count());
        // Two files that differ only in what they are stay apart.
        assert_ne!(shown("report-2026-final-draft.pdf", 16.0), shown("report-2026-final-draft.docx", 16.0));
    }

    #[test]
    fn short_or_odd_names_are_left_alone() {
        assert_eq!(elide_at("a.txt", 30.0, &chars), None, "it fits");
        assert_eq!(elide_at("x.averyveryverylongextension", 10.0, &chars), None, "the extension is most of it");
        assert_eq!(elide_at("ab.txt", 2.0, &chars), None, "no room even for one character and the extension");
    }

    /// Q67: rows that would read the same keep more of where they differ,
    /// within the same width; rows that already differ are left alone.
    #[test]
    fn neighbours_that_would_read_the_same_are_told_apart() {
        let show = |name: &str, near: &[&str]| {
            let cut = elide_at(name, 12.0, &chars).unwrap();
            elided(name, &[], apart(name, cut, near)).0
        };
        let a = "notes-archive-x-15484.log";
        let b = "notes-test-yy-15484.log";
        assert_eq!(shown(a, 12.0), "notes…84.log");
        assert_eq!(shown(b, 12.0), "notes…84.log", "the case: the two read the same");
        assert_eq!(show(a, &[b]), "notes-a….log");
        assert_eq!(show(b, &[a]), "notes-t….log");
        assert_eq!(show(a, &[b]).chars().count(), shown(a, 12.0).chars().count(), "no wider");
        // They differ near the end: more of the tail instead.
        let c = "build-output-run-1-x.log";
        let d = "build-output-run-2-x.log";
        assert_eq!(shown(c, 12.0), "build…-x.log");
        assert_eq!(show(c, &[d]), "buil…1-x.log");
        // Already apart: unchanged.
        assert_eq!(show(a, &["zzzzzzzzzzzzzzzzzzzzzz.log"]), shown(a, 12.0));
    }

    /// `srcmain` matched over `src/main.rs`: the name keeps its letters, the
    /// directory part has none to show.
    #[test]
    fn a_fuzzy_hit_in_the_path_lands_in_the_name() {
        use std::path::Path;
        let m = ito_match::Matcher::fuzzy("srcmain");
        let hits = super::path_hits(&m, Path::new("/r/src/main.rs"), Path::new("/r"), "main.rs");
        assert_eq!(hits, [0, 1, 2, 3]);
    }

    /// The search highlight moves with the characters it marked, and a hit
    /// inside the cut is dropped rather than landing on the wrong letter.
    #[test]
    fn hits_follow_their_characters() {
        let (text, hits) = elided("abcdefghij.txt", &[0, 5, 11], (3, 8));
        assert_eq!(text, "abc…ij.txt");
        assert_eq!(hits, vec![0, 7], "`a` stays, `f` was cut, the `t` of `.txt` moved from 11 to 7");
    }
}

#[cfg(test)]
mod empty {
    use super::empty_label;

    #[test]
    fn a_folder_with_nothing_in_it_is_empty() {
        assert_eq!(empty_label(&std::env::temp_dir().join("nothing")), "(empty)");
    }

    #[cfg(windows)]
    #[test]
    fn a_host_with_nothing_shared_says_so() {
        use std::path::Path;
        assert_eq!(empty_label(Path::new(r"\\fileserver")), "(no shares)");
        assert_eq!(empty_label(Path::new(r"\\fileserver\share")), "(empty)");
    }
}

#[cfg(test)]
mod usage_column {
    use super::linemode_text;
    use crate::fs::entry::Linemode;
    use crate::fs::Entry;

    /// TESTING.md 44.18 — `m u` outside `gu`'s view: a folder nothing has
    /// measured is blank rather than `0 B`, and a file still shows its size.
    #[test]
    fn an_unmeasured_folder_is_blank_not_empty() {
        let dir = crate::util::test_dir("usage-column");
        std::fs::create_dir_all(dir.join("big")).unwrap();
        std::fs::write(dir.join("big").join("x"), vec![b'x'; 2048]).unwrap();
        std::fs::write(dir.join("note.txt"), vec![b'x'; 1024]).unwrap();
        let mut big = Entry::from_path(dir.join("big")).unwrap();
        let note = Entry::from_path(dir.join("note.txt")).unwrap();
        assert_eq!(linemode_text(&big, Linemode::Usage), "");
        assert_eq!(linemode_text(&note, Linemode::Usage), "1.0 K");
        big.usage = Some(2048);
        assert_eq!(linemode_text(&big, Linemode::Usage), "2.0 K", "measured, it shows");
    }
}

const DETAIL_KEYS: [&str; 11] = [
    "details.name",
    "details.modified",
    "details.created",
    "details.accessed",
    "details.type",
    "details.size",
    "details.dimensions",
    "details.owner",
    "details.permissions",
    "details.extension",
    "details.usage",
];

fn detail_columns(width: f32, cell: f32) -> Vec<(usize, f32, f32)> {
    let widths = [0.0, 17.0, 17.0, 17.0, 16.0, 9.0, 15.0, 16.0, 12.0, 10.0, 14.0];
    let mut visible: Vec<usize> = (1..11).collect();
    // Preserve the name, modified time, size and usage until the narrow fallback.
    for drop in [3, 7, 6, 2, 8, 9, 4, 1, 5, 10] {
        let used: f32 = visible.iter().map(|&i| widths[i] * cell).sum();
        if used + 18.0 * cell <= width {
            break;
        }
        visible.retain(|&i| i != drop);
    }
    let used: f32 = visible.iter().map(|&i| widths[i] * cell).sum();
    let mut x = (width - used).max(0.0);
    visible
        .into_iter()
        .map(|i| {
            let w = widths[i] * cell;
            let col = (i, x, w);
            x += w;
            col
        })
        .collect()
}

use crate::fs::details::detail_type;

fn detail_text(entry: &Entry, col: usize, lang: &str) -> String {
    use crate::fs::entry::Linemode as L;
    match col {
        1 => linemode_text(entry, L::Mtime),
        2 => linemode_text(entry, L::Btime),
        3 => util::fmt_time(entry.accessed, "%Y-%m-%d %H:%M"),
        4 => detail_type(entry, lang),
        5 => entry.display_size().unwrap_or_default(),
        6 => entry.details.as_ref().and_then(|d| d.dimensions).map(|(w, h)| format!("{w} × {h}")).unwrap_or_default(),
        7 => linemode_text(entry, L::Owner),
        8 => permissions(entry),
        9 => entry.ext.as_deref().unwrap_or("").to_lowercase(),
        10 if entry.usage.is_some() => linemode_text(entry, L::Usage),
        _ => String::new(),
    }
}

#[cfg(test)]
mod details_tests {
    use super::*;
    #[test]
    fn types_use_metadata_and_translated_extensions() {
        let mut e = Entry { kind: crate::fs::Kind::Dir, ..Default::default() };
        assert_eq!(detail_type(&e, "en"), "File folder");
        e.kind = crate::fs::Kind::File;
        assert_eq!(detail_type(&e, "en"), "File");
        e.ext = Some("json".into());
        assert_eq!(detail_type(&e, "en"), "JSON file");
        assert_eq!(detail_type(&e, "ja"), "JSON ファイル");
        e.ext = Some("unrecognized".into());
        assert_eq!(detail_type(&e, "en"), "UNRECOGNIZED file");
        e.kind = crate::fs::Kind::Link { to_dir: true, broken: false };
        assert_eq!(detail_type(&e, "en"), "Link");
    }
    #[test]
    fn narrow_tables_keep_size_and_usage_without_overlap() {
        let cols = detail_columns(400.0, 8.0);
        assert_eq!(cols.iter().map(|c| c.0).collect::<Vec<_>>(), vec![5, 10]);
        assert!(cols[0].1 > 0.0);
    }
    #[test]
    fn ordinary_tables_retain_all_eight_original_columns() {
        for cell in [8.0, 9.0] {
            let cols = detail_columns(1200.0, cell);
            for required in [1, 2, 4, 5, 8, 9, 10] {
                assert!(cols.iter().any(|c| c.0 == required), "missing {required} with cell {cell}");
            }
            assert!(!cols.iter().any(|c| c.0 == 3));
        }
    }
    #[test]
    fn wide_tables_show_every_column_and_tiny_tables_do_not_overlap() {
        let cols = detail_columns(1800.0, 8.0);
        assert_eq!(cols.iter().map(|c| c.0).collect::<Vec<_>>(), (1..11).collect::<Vec<_>>());
        for width in [0.0, 8.0, 80.0, 240.0, 400.0, 640.0, 1200.0, 1800.0] {
            let cols = detail_columns(width, 8.0);
            let mut previous_end = 0.0;
            for (_, x, w) in cols {
                assert!(x >= previous_end && x + w <= width);
                previous_end = x + w;
            }
        }
    }
    #[test]
    fn unmeasured_usage_and_missing_metadata_are_blank() {
        let e = Entry::default();
        assert_eq!(detail_text(&e, 10, "en"), "");
        assert_eq!(detail_text(&e, 6, "en"), "");
        assert_eq!(detail_text(&e, 7, "en"), "");
        let e = Entry {
            details: Some(crate::fs::details::Details { owner: "person".into(), dimensions: Some((1920, 1080)) }),
            ..Default::default()
        };
        assert_eq!(detail_text(&e, 6, "en"), "1920 × 1080");
        assert_eq!(linemode_text(&e, crate::fs::entry::Linemode::Owner), "person");
    }
}
