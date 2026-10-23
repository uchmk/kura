use super::*;

/// A config warning is not an error, and has to stop looking like one.
///
/// v0.20.0 put the startup line up through `error`, which paints it in the
/// colour of a failed operation. The first person to see it asked what had
/// gone wrong -- nothing had: a keymap of theirs bound `'` twice to the
/// same command, which changes no behaviour at all. Asserting on the level
/// rather than the text, because the wording is not what misled them.
#[test]
fn are_raised_as_warnings_not_errors() {
    let ctx = egui::Context::default();
    let cfg = Config { warnings: vec!["[mgr] `'` is bound twice".into()], ..Config::load() };
    let app = App::new(cfg, std::env::temp_dir(), ctx);

    let t = app.toasts.first().expect("the warning reaches the screen");
    assert_eq!(t.level, Level::Warn, "a config warning is advice, not a failure");
    assert!(t.text.starts_with("Config: "), "{}", t.text);
}

/// The colours have to differ, or the level above is a distinction the
/// reader cannot see.
#[test]
fn warning_and_error_are_different_colours() {
    let t = crate::config::theme::Theme::default();
    assert_ne!(t.warning, t.progress_error);
    assert_ne!(t.warning, t.fg);
}
