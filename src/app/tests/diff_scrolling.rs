use super::*;

fn app() -> App {
    let ctx = egui::Context::default();
    App::new(Config::load(), std::env::temp_dir(), ctx)
}

/// `G` then `k` has to move the view, and used not to.
///
/// `offset` is where the pane starts drawing, so the furthest it can
/// usefully go is the value that puts the last row at the *bottom*.
/// Clamping it to the last row instead let `G` leave it a screenful past
/// that, and the renderer drew from its own, lower, clamp — so the next
/// `j` or `k` changed a number nothing read. The keys came back only once
/// enough presses had walked `offset` down into the range being drawn,
/// which is why a half-page `<C-u>` looked like it repaired them.
#[test]
fn the_keys_answer_at_the_bottom() {
    let mut a = app();
    let rows: Vec<diff::Row> = (0..100)
        .map(|n| diff::Row {
            left: Some(diff::Line { no: n, text: format!("line {n}"), changed: Vec::new() }),
            right: Some(diff::Line { no: n, text: format!("line {n}"), changed: Vec::new() }),
            same: true,
        })
        .collect();
    // 20 rows of pane, as the renderer would have reported after a frame.
    a.overlay = Overlay::Diff(DiffOverlay {
        left: PathBuf::from("a"),
        right: PathBuf::from("b"),
        outcome: Some(diff::Outcome::Rows { rows, truncated: false, rough: false }),
        offset: 0,
        rows: 20,
        cursor: 0,
        hide_same: false,
        back: None,
    });
    let at = |a: &App| match &a.overlay {
        Overlay::Diff(ov) => ov.offset,
        _ => panic!("the overlay closed"),
    };

    a.diff_act(Act::Arrow(Step::Bot));
    assert_eq!(at(&a), 80, "the last row sits at the bottom, not at the top");

    a.diff_act(Act::Arrow(Step::Rel(-1)));
    assert_eq!(at(&a), 79, "and one back is one row, not a dead press");

    a.diff_act(Act::Arrow(Step::Rel(1)));
    assert_eq!(at(&a), 80, "forward returns to the bottom");
    a.diff_act(Act::Arrow(Step::Rel(1)));
    assert_eq!(at(&a), 80, "and stops there");

    a.diff_act(Act::Arrow(Step::Top));
    assert_eq!(at(&a), 0);
}

/// A file shorter than the pane has nowhere to scroll.
#[test]
fn a_short_diff_does_not_move() {
    let mut a = app();
    let rows: Vec<diff::Row> = (0..3).map(|n| diff::Row { left: None, right: None, same: n % 2 == 0 }).collect();
    a.overlay = Overlay::Diff(DiffOverlay {
        left: PathBuf::from("a"),
        right: PathBuf::from("b"),
        outcome: Some(diff::Outcome::Rows { rows, truncated: false, rough: false }),
        offset: 0,
        rows: 20,
        cursor: 0,
        hide_same: false,
        back: None,
    });
    a.diff_act(Act::Arrow(Step::Bot));
    match &a.overlay {
        Overlay::Diff(ov) => assert_eq!(ov.offset, 0, "all three are already on screen"),
        _ => panic!("the overlay closed"),
    }
}
