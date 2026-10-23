//! `q` means the same thing over every panel: close this, do not quit.

use super::*;

fn app() -> App {
    let ctx = egui::Context::default();
    App::new(Config::load(), std::env::temp_dir(), ctx)
}

/// Reported from use: `q` quit the app with quick look still up. The panels
/// that carry their own keymap layer all bind `q` to `close`; these two have
/// no layer, by design, so the key reached `mgr`'s `quit`.
#[test]
fn quick_look_closes_before_the_process_does() {
    let mut a = app();
    a.quick = true;
    a.act(Act::Quit);
    assert!(!a.quick, "`q` closed the app instead of the panel");
    assert!(!a.quit, "and it must not have quit on the way");

    a.act(Act::Quit);
    assert!(a.quit, "a second `q` still quits");
}

#[test]
fn a_maximized_preview_closes_first_too() {
    let mut a = app();
    a.max_preview = true;
    a.act(Act::Quit);
    assert!(!a.max_preview && !a.quit);
    a.act(Act::Quit);
    assert!(a.quit);
}

/// With nothing in front, `q` is still `quit` on the first press.
#[test]
fn q_quits_when_no_panel_is_up() {
    let mut a = app();
    a.act(Act::Quit);
    assert!(a.quit);
}

/// `q` and `<Esc>` peel in the same order, because `q` routes through the
/// same code. Two panels up means two presses, either key.
#[test]
fn q_and_escape_agree_on_the_order() {
    for quit_key in [true, false] {
        let mut a = app();
        a.quick = true;
        a.max_preview = true;
        let press = |a: &mut App| {
            if quit_key {
                a.act(Act::Quit);
            } else {
                a.act(Act::Escape(EscapeWhat::default()));
            }
        };
        press(&mut a);
        assert!(!a.quick && a.max_preview, "the panel in front goes first");
        press(&mut a);
        assert!(!a.max_preview);
        assert!(!a.quit, "neither key quits while something is still up");
    }
}
