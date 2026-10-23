/// `term_scroll -50%` means half a screen *back*, the way `arrow -50%`
/// means half a screen up. Alacritty counts the other way round: a
/// positive delta is older. Passing the number through as it stood aimed
/// `<S-PageUp>` at the bottom, which is where the view already is, so the
/// first key anyone tries did nothing — while `<S-Home>` and `<S-End>`,
/// having no sign to get wrong, worked and made it look like a key
/// problem rather than an arithmetic one.
#[test]
fn a_negative_step_goes_back_into_the_history() {
    use crate::config::cmd::Step;
    let lines = 4usize;
    let mut t = crate::terminal::testing::term(20, lines);
    for i in 0..40 {
        crate::terminal::testing::feed(&mut t, &format!("line{i}\r\n"));
    }

    // The mapping under test, lifted out of `Act::TermScroll`.
    let scroll = |t: &mut ito_pane::alacritty_terminal::Term<crate::terminal::Proxy>, step: Step| {
        use ito_pane::alacritty_terminal::grid::Scroll;
        let page = lines as i64;
        let by = match step {
            Step::Top => Scroll::Top,
            Step::Bot => Scroll::Bottom,
            Step::Rel(n) => Scroll::Delta(-(n as i32)),
            Step::Pct(p) => Scroll::Delta(-((page * p / 100) as i32)),
        };
        t.scroll_display(by);
    };

    scroll(&mut t, Step::Pct(-50));
    assert_eq!(t.grid().display_offset(), 2, "back half of a four-line screen");
    scroll(&mut t, Step::Pct(50));
    assert_eq!(t.grid().display_offset(), 0, "and forward again");

    scroll(&mut t, Step::Rel(-3));
    assert_eq!(t.grid().display_offset(), 3, "three lines back");
    scroll(&mut t, Step::Top);
    assert!(t.grid().display_offset() > 3, "the top is as far as it goes");
    scroll(&mut t, Step::Bot);
    assert_eq!(t.grid().display_offset(), 0);
}
