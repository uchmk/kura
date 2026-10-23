use super::*;

fn app() -> App {
    let ctx = egui::Context::default();
    let mut a = App::new(Config::load(), std::env::temp_dir(), ctx);
    a.overlay = Overlay::Help;
    // What the renderer leaves behind: a panel 20 rows tall showing a list
    // of 100.
    a.help_rows = 20;
    a.help_lines = 100;
    a
}

/// Each token is one key, spelled the way the keymap spells it.
fn press(a: &mut App, tokens: &[&str]) {
    for t in tokens {
        a.feed_overlay_key(Key::parse(t).expect("notation the keymap can spell"));
    }
}

/// `<A-j>` / `<A-k>` move half the panel, the way they do in every other
/// pane this app scrolls.
#[test]
fn alt_j_and_k_move_half_a_panel() {
    let mut a = app();
    press(&mut a, &["<A-j>"]);
    assert_eq!(a.help_scroll, 10, "half of the twenty rows on screen");
    press(&mut a, &["<A-j>"]);
    assert_eq!(a.help_scroll, 20);
    press(&mut a, &["<A-k>"]);
    assert_eq!(a.help_scroll, 10);

    // `<C-d>` / `<C-u>` are the same distance, for a hand coming from vim.
    press(&mut a, &["<C-d>"]);
    assert_eq!(a.help_scroll, 20);
    press(&mut a, &["<C-u>"]);
    assert_eq!(a.help_scroll, 10);
}

/// Scrolling stops with the last line at the bottom, not at the top.
///
/// It used to stop nowhere at all: `help_scroll` was incremented raw, so
/// holding `j` ran the number far past the end of the list while the panel
/// sat still. Every press back up then moved a number nothing was drawing
/// from, and the keys looked dead for exactly as many presses as had been
/// wasted going down.
#[test]
fn it_stops_with_the_last_line_on_screen() {
    let mut a = app();
    for _ in 0..500 {
        press(&mut a, &["j"]);
    }
    assert_eq!(a.help_scroll, 80, "100 lines less the 20 on screen");
    press(&mut a, &["k"]);
    assert_eq!(a.help_scroll, 79, "and one press comes straight back");

    press(&mut a, &["G"]);
    assert_eq!(a.help_scroll, 80);
    press(&mut a, &["g", "g"]);
    assert_eq!(a.help_scroll, 0);
    press(&mut a, &["k"]);
    assert_eq!(a.help_scroll, 0, "nor does it go above the first line");
}

/// A panel with room to spare does not scroll at all.
#[test]
fn a_short_list_does_not_move() {
    let mut a = app();
    a.help_lines = 5;
    press(&mut a, &["<A-j>"]);
    press(&mut a, &["G"]);
    assert_eq!(a.help_scroll, 0);
}

/// The keys that opened the panel close it, and so do `q` and `<Esc>`.
#[test]
fn it_closes_on_its_own_keys() {
    for seq in ["<Esc>", "q", "~", "<F1>"] {
        let mut a = app();
        press(&mut a, &[seq]);
        assert!(a.overlay.is_none(), "{seq} should have closed the panel");
    }
}

/// The panel's keys come from the keymap, so they can be rebound.
///
/// They were read straight off the egui event before, which left the one
/// panel whose subject is the keymap unable to honor it.
#[test]
fn the_layer_is_the_keymap_not_the_event_loop() {
    let text = "[[help.keymap]]\non = \"n\"\nrun = \"arrow 1\"\n";
    let (km, _) = crate::config::Keymap::load(&[text]);
    let mut a = app();
    a.cfg.keymap = km;
    press(&mut a, &["n"]);
    assert_eq!(a.help_scroll, 1, "the added key scrolls");
}

/// A panel over the list owns the wheel; a one-row prompt does not.
#[test]
fn a_modal_panel_takes_the_wheel() {
    assert!(Overlay::Help.is_modal());
    assert!(!Overlay::None.is_modal());
    let prompt = InputOverlay {
        kind: InputKind::Filter,
        title: "filter".into(),
        text: String::new(),
        initial_selection: None,
        focused: true,
        completion: Vec::new(),
        completion_at: 0,
    };
    assert!(!Overlay::Input(prompt).is_modal(), "the list above it is still being read");
}
