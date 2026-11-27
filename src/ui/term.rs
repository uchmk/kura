//! The terminal pane, drawn by `ito-pane`'s egui view in the theme's
//! colours. What only kura can do -- give the pane the keys, the clipboard,
//! and saying so when the clipboard will not open -- is done here with what
//! the view hands back.

use egui::{pos2, vec2, CornerRadius, FontId, Rect, Stroke, Ui};

use crate::app::App;
use crate::config::theme::Theme;

/// The pane's colours out of the theme: the pane sits on `bg_alt`, a selection
/// takes tsumugi's selection colour (ito-select's, as the preview's does), and
/// the cursor the `cwd` colour.
fn palette(theme: &Theme) -> ito_pane::Palette {
    ito_pane::Palette {
        bg: theme.bg_alt,
        fg: theme.fg,
        fg_dim: theme.fg_dim,
        selection: theme.common.selection(),
        found: theme.common.wait,
        cursor: theme.cwd.fg.unwrap_or(theme.fg),
        on_cursor: theme.bg,
        ansi: None,
        mark_ok: theme.common.success(),
        mark_failed: theme.common.err,
    }
}

/// How many frames of a drag `App::term_drag` keeps (enough to see the selection grow, not to grow without end).
pub const TERM_DRAG_KEPT: usize = 24;

/// What the pane drew of its selection in one frame, as `cell_rows` marks it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SelShown {
    /// Selected cells on the screen.
    pub cells: usize,
    /// The first screen row with any (0 = the top row on screen), and its first and last selected column.
    pub row: usize,
    pub first_col: usize,
    pub last_col: usize,
}

/// What of the selection is on the screen, from the rows the view paints (row 1.11: a check has to see that the
/// selection was drawn, not only that it was copied). `None` when no cell is selected (nothing selected, or scrolled
/// off). Only `selected` counts: a `found` cell is painted in the other colour.
pub fn selection_shown(rows: &[Vec<crate::terminal::CellView>]) -> Option<SelShown> {
    let mut cells = 0;
    let mut first: Option<(usize, usize, usize)> = None;
    for (r, row) in rows.iter().enumerate() {
        let mut span: Option<(usize, usize)> = None;
        for (c, cell) in row.iter().enumerate() {
            if cell.selected {
                cells += 1;
                span = Some(span.map_or((c, c), |(a, _)| (a, c)));
            }
        }
        if first.is_none() {
            first = span.map(|(a, b)| (r, a, b));
        }
    }
    first.map(|(row, first_col, last_col)| SelShown { cells, row, first_col, last_col })
}

pub fn draw(app: &mut App, ui: &mut Ui, rect: Rect, f: &FontId, row_h: f32) {
    let pal = palette(&app.cfg.theme);
    // What this frame will paint of the selection: the view takes its rows before it handles this frame's pointer,
    // so the selection as it is now is what is drawn. The grid is copied only while something is selected.
    let (pressed, held, origin) = ui.input(|i| (i.pointer.primary_pressed(), i.pointer.primary_down(), i.pointer.press_origin()));
    let from_pane = origin.is_some_and(|p| rect.contains(p));
    let sel =
        app.term.as_ref().filter(|t| t.has_selection()).and_then(|t| t.with_grid(|g| selection_shown(&crate::terminal::cell_rows(g))));
    if pressed && from_pane {
        app.term_drag.clear();
        app.term_drag_frames = 0;
    }
    if held && from_pane {
        app.term_drag_frames += 1;
        if app.term_drag.len() < TERM_DRAG_KEPT {
            app.term_drag.push(sel.map_or(0, |s| s.cells));
        }
    }
    // A panel over the pane owns the wheel; see `Overlay::is_modal`. Letting
    // go of a selection always copies it (below), and kura has no triggers.
    let opts = ito_pane::ViewOptions { focused: app.term_focus, wheel: !app.overlay.is_modal(), copy_on_select: true, highlights: &[] };
    let mut state = ito_pane::ViewState { scroll_rows: app.term_scroll_rows };
    let shown = ito_pane::show(ui, app.term.as_mut(), &mut state, rect, f, row_h, &pal, opts);
    app.term_scroll_rows = state.scroll_rows;
    app.term_grid = shown.grid;
    app.term_sel = sel;
    // A hover-only hook with a Test ID over the first selected row's run of cells, so a check spec can sample the
    // selection colour (row 1.11). It takes no click or drag, so the pane still gets them.
    if let (Some(s), Some((origin, cell))) = (sel, shown.grid) {
        let at = origin + vec2(s.first_col as f32 * cell.x, s.row as f32 * cell.y);
        let r = Rect::from_min_size(at, vec2((s.last_col - s.first_col + 1) as f32 * cell.x, cell.y)).intersect(rect);
        if r.width() > 0.0 && r.height() > 0.0 {
            let hook = ui.interact(r, ui.id().with("pane-selection"), egui::Sense::hover());
            ito_a11y::test_id_in(ui, &hook, "pane.selection", None);
        }
    }
    if shown.focus {
        app.term_focus = true;
    }
    // Accented while the pane has the keys, the same as the outline's rule and
    // in the same colour -- see `super::focus_rule` for why it is shared.
    let rule = super::focus_rule(&app.cfg.theme, app.term_focus);
    ui.painter_at(rect).line_segment([rect.left_top(), rect.right_top()], Stroke::new(1.0, rule));
    // A full-screen program covers every row, so the way back out is put in
    // the corner while it runs (Q77). On a plate, as the preview's zoom note
    // is: it sits over the program's first row and has to read over anything.
    if let Some(badge) = app.term_leave_badge() {
        let theme = &app.cfg.theme;
        let painter = ui.painter_at(rect);
        let g = painter.layout_no_wrap(badge, FontId::new(f.size * 0.85, f.family.clone()), theme.fg_dim);
        let at = pos2(rect.right() - g.size().x - 10.0, rect.top() + 4.0);
        let plate = Rect::from_min_size(at, g.size()).expand2(vec2(6.0, 1.0));
        painter.rect_filled(plate, CornerRadius::same(4), theme.bg.gamma_multiply(0.85));
        painter.galley(at, g, theme.fg_dim);
    }
    // Letting go of a selection copies it, which is what a terminal means by
    // selecting: there is no other step.
    if let Some(text) = shown.copy {
        let _ = ito_clipboard::clipboard!().set_text(&text);
    }
    // Ctrl+click (Cmd on a Mac) on a link: a web address goes to the browser,
    // a path is shown in the list -- this is a file manager, and the list is
    // where the next thing to do with a file is one key away.
    match shown.open {
        Some(ito_pane::Link::Url(url)) => {
            if let Err(e) = crate::exec::open_url(&url) {
                app.error(format!("Could not open {url}: {e}"));
            }
        }
        Some(ito_pane::Link::Path { path, .. }) => {
            // Relative to the shell, which is where it was printed.
            let base = app.term.as_ref().and_then(|t| t.shell_cwd.clone());
            let target = match base {
                Some(dir) => crate::util::resolve_against(&dir, &path).to_string_lossy().into_owned(),
                None => path,
            };
            app.reveal(target);
        }
        None => {}
    }
    // Right-click pastes. `<C-v>` reaches the pane as egui's paste event; a
    // right-click is not one, so the clipboard is read -- on a worker, as it
    // can block for seconds on X11; `drain_channels` pastes the answer.
    if shown.paste {
        app.start_term_paste();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terminal::alacritty_terminal::term::cell::Flags;
    use crate::terminal::alacritty_terminal::vte::ansi::{Color, NamedColor};

    fn cell(selected: bool, found: bool) -> crate::terminal::CellView {
        crate::terminal::CellView {
            c: 'x',
            fg: Color::Named(NamedColor::Foreground),
            bg: Color::Named(NamedColor::Background),
            flags: Flags::empty(),
            ul: None,
            selected,
            found,
        }
    }

    fn row(selected: std::ops::Range<usize>) -> Vec<crate::terminal::CellView> {
        (0..10).map(|c| cell(selected.contains(&c), false)).collect()
    }

    #[test]
    fn selection_shown_reads_the_marked_cells() {
        assert_eq!(selection_shown(&[]), None);
        assert_eq!(selection_shown(&[row(0..0), row(0..0)]), None);
        assert_eq!(selection_shown(&[row(2..6)]), Some(SelShown { cells: 4, row: 0, first_col: 2, last_col: 5 }));
        assert_eq!(selection_shown(&[row(0..0), row(3..10), row(0..4)]), Some(SelShown { cells: 11, row: 1, first_col: 3, last_col: 9 }));
        let found: Vec<_> = (0..4).map(|_| cell(false, true)).collect();
        assert_eq!(selection_shown(&[found]), None);
    }
}
