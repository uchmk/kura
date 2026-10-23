use super::*;

fn app() -> (App, egui::Context) {
    let ctx = egui::Context::default();
    let app = App::new(Config::load(), std::env::temp_dir(), ctx.clone());
    (app, ctx)
}

/// `<A-j>` at the bottom of a file used to push `preview_offset` past the
/// end, and the pane was drawn from there before anything corrected it —
/// one wrong frame per press, which is why it took a held-down key to
/// catch.
#[test]
fn scrolling_down_stops_at_the_end() {
    let (mut a, _c) = app();
    a.preview.max_offset = 40;
    a.tabs[a.active].preview_offset = 38;

    for _ in 0..20 {
        a.act(Act::Seek(Step::Rel(5)));
    }

    assert_eq!(a.tabs[a.active].preview_offset, 40, "twenty presses at the bottom must leave the offset on the last line",);
}

/// And the other end, which was already right: one press back from the
/// bottom has to move, or the ceiling has turned into a trap.
#[test]
fn scrolling_back_up_is_immediate() {
    let (mut a, _c) = app();
    a.preview.max_offset = 40;
    a.tabs[a.active].preview_offset = 40;

    a.act(Act::Seek(Step::Rel(-5)));

    assert_eq!(a.tabs[a.active].preview_offset, 35, "leaving the bottom must take one press, not as many as were spent overshooting",);
}

#[test]
fn scrolling_up_stops_at_the_top() {
    let (mut a, _c) = app();
    a.preview.max_offset = 40;
    a.tabs[a.active].preview_offset = 3;
    for _ in 0..10 {
        a.act(Act::Seek(Step::Rel(-5)));
    }
    assert_eq!(a.tabs[a.active].preview_offset, 0);
}

/// A file with nothing to scroll does not scroll.
#[test]
fn a_short_file_does_not_move() {
    let (mut a, _c) = app();
    a.preview.max_offset = 0;
    a.act(Act::Seek(Step::Rel(5)));
    assert_eq!(a.tabs[a.active].preview_offset, 0);
}
