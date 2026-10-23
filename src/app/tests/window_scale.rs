use super::*;
use crate::config::cmd::ScaleTo;

fn app() -> App {
    let ctx = egui::Context::default();
    App::new(Config::load(), std::env::temp_dir(), ctx)
}

/// Scaling steps and stops where egui's own did.
///
/// egui zooms on Ctrl +/-/0 at the end of every frame, and does not
/// consume the key on the way: each of those three is a key kura binds,
/// so both ran. `<C-->` made a hardlink *and* shrank the window, which is
/// the sort of thing that reads as the program being possessed.
#[test]
fn it_steps_and_stops_where_egui_did() {
    let mut a = app();
    assert_eq!(a.scale, 1.0);

    a.act(Act::Scale(ScaleTo::In));
    assert!((a.scale - 1.1).abs() < 1e-6, "{}", a.scale);
    a.act(Act::Scale(ScaleTo::Out));
    a.act(Act::Scale(ScaleTo::Out));
    assert!((a.scale - 0.9).abs() < 1e-6, "{}", a.scale);

    a.act(Act::Scale(ScaleTo::Reset));
    assert_eq!(a.scale, 1.0);

    // The same bounds, so a held key cannot shrink it to nothing.
    for _ in 0..40 {
        a.act(Act::Scale(ScaleTo::Out));
    }
    assert_eq!(a.scale, 0.2);
    for _ in 0..80 {
        a.act(Act::Scale(ScaleTo::In));
    }
    assert_eq!(a.scale, 5.0);
}

/// A panel does not get to swallow the window's own size.
///
/// v0.46.0 bound the scale chords in `[help]`, `[spot]`, `[tasks]` and
/// `[diff]` and shipped them doing nothing: the binding resolved, and then
/// every layer's `act` dropped the `Act` on its closing `_ => {}`. Nothing
/// in the build caught it, because a binding that resolves to an ignored
/// action is indistinguishable from a working one until a person presses
/// the key. This goes through `feed_overlay_key`, the same door the window
/// uses, so a fifth panel added later fails here rather than in someone's
/// hands.
#[test]
fn every_panel_lets_the_window_be_resized() {
    use crate::app::{DiffOverlay, SpotOverlay, TasksOverlay};
    let mut a = app();
    let panels: Vec<(&str, Overlay)> = vec![
        ("help", Overlay::Help),
        ("tasks", Overlay::Tasks(TasksOverlay { cursor: 0 })),
        ("spot", Overlay::Spot(SpotOverlay { cursor: 0, scroll: 0 })),
        (
            "diff",
            Overlay::Diff(DiffOverlay {
                left: std::path::PathBuf::from("a"),
                right: std::path::PathBuf::from("b"),
                outcome: None,
                offset: 0,
                rows: 10,
                cursor: 0,
                hide_same: false,
                back: None,
            }),
        ),
        ("settings", Overlay::Settings(Default::default())),
    ];
    for (name, overlay) in panels {
        a.scale = 1.0;
        a.overlay = overlay;
        a.feed_overlay_key(Key::parse("<C-+>").unwrap());
        assert!((a.scale - 1.1).abs() < 1e-6, "`<C-+>` with {name} open: {}", a.scale);
        a.feed_overlay_key(Key::parse("<C-;>").unwrap());
        assert!((a.scale - 1.2).abs() < 1e-6, "`<C-;>` with {name} open: {}", a.scale);
        for _ in 0..3 {
            a.feed_overlay_key(Key::parse("<C-->").unwrap());
        }
        assert!((a.scale - 0.9).abs() < 1e-6, "`<C-->` with {name} open: {}", a.scale);
        a.feed_overlay_key(Key::parse("<C-0>").unwrap());
        assert_eq!(a.scale, 1.0, "`<C-0>` with {name} open");
        // The panel is still up: resizing is not a way out of it.
        assert!(!matches!(a.overlay, Overlay::None), "{name} closed itself");
    }
}

/// F1 is help from anywhere, as in every uchmk app (since v0.99.0): over
/// any panel it opens the key list, and pressed again it closes it.
#[test]
fn f1_opens_help_over_every_panel() {
    use crate::app::{DiffOverlay, SpotOverlay, TasksOverlay};
    let mut a = app();
    let panels: Vec<(&str, Overlay)> = vec![
        ("tasks", Overlay::Tasks(TasksOverlay { cursor: 0 })),
        ("spot", Overlay::Spot(SpotOverlay { cursor: 0, scroll: 0 })),
        (
            "diff",
            Overlay::Diff(DiffOverlay {
                left: std::path::PathBuf::from("a"),
                right: std::path::PathBuf::from("b"),
                outcome: None,
                offset: 0,
                rows: 10,
                cursor: 0,
                hide_same: false,
                back: None,
            }),
        ),
    ];
    for (name, overlay) in panels {
        a.overlay = overlay;
        a.feed_overlay_key(Key::parse("<F1>").unwrap());
        assert!(matches!(a.overlay, Overlay::Help), "F1 over {name} opens help");
        a.feed_overlay_key(Key::parse("<F1>").unwrap());
        assert!(matches!(a.overlay, Overlay::None), "and F1 again closes it, from {name}");
    }
}

/// The two zooms are different commands, and stay that way.
///
/// `zoom` was already taken by the image preview. Naming this one the same
/// would have made the keymap ambiguous in a way only the parser could see.
#[test]
fn the_image_zoom_is_a_different_command() {
    use crate::config::cmd::parse;
    assert_eq!(parse("scale out"), Act::Scale(ScaleTo::Out));
    assert!(matches!(parse("zoom out"), Act::Zoom(_)));
}
