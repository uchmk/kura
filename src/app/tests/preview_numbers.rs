//! `#` numbers the lines of the preview (the column's own flag, or the one `T`
//! and `<F3>` share), `<F2>` renames, `<C-g>` goes to a line from the list.

use super::*;
use crate::config::cmd::parse;
use crate::config::keymap::{self, Keymap, Match};
use crate::config::keys::Key;
use crate::preview::Span;

fn app() -> App {
    let mut a = App::new(Config::load(), crate::util::test_tmp(), egui::Context::default());
    // The test must not depend on this machine's config.toml.
    a.cfg.ui.line_numbers = false;
    a.max_numbers = true;
    a.max_preview = false;
    a.quick = false;
    a
}

fn text(n: usize) -> PreviewState {
    let lines = (0..n).map(|i| vec![Span { text: format!("line {i}"), ..Default::default() }]).collect();
    PreviewState::Ready(Payload::Text { lines, map: Vec::new(), extent: Default::default(), outline: Vec::new(), tabs: Default::default() })
}

fn bound(section: &[keymap::Binding], key: &str) -> Vec<Act> {
    let k = Key::parse(key).unwrap_or_else(|| panic!("`{key}` is not key notation"));
    match keymap::resolve(section, &[k]) {
        Match::Exact(b) => b.run.clone(),
        _ => panic!("`{key}` is not bound to anything on its own"),
    }
}

#[test]
fn the_command_reads_its_word() {
    assert_eq!(parse("preview_numbers"), Act::PreviewNumbers(None));
    assert_eq!(parse("preview_numbers on"), Act::PreviewNumbers(Some(true)));
    assert_eq!(parse("preview_numbers show"), Act::PreviewNumbers(Some(true)));
    assert_eq!(parse("preview_numbers off"), Act::PreviewNumbers(Some(false)));
    assert_eq!(parse("preview_numbers hide"), Act::PreviewNumbers(Some(false)));
}

#[test]
fn the_column_flips_its_own_flag_and_leaves_the_maximized_one() {
    let mut a = app();
    assert!(!a.preview_numbers(), "the column starts without numbers");
    a.act(Act::PreviewNumbers(None));
    assert!(a.cfg.ui.line_numbers && a.max_numbers);
    assert!(a.preview_numbers());
    a.act(Act::PreviewNumbers(Some(true)));
    assert!(a.cfg.ui.line_numbers, "`on` sets rather than flips");
    a.act(Act::PreviewNumbers(Some(false)));
    assert!(!a.cfg.ui.line_numbers);
    assert!(a.max_numbers, "the session flag was not touched");
}

#[test]
fn t_and_quick_look_flip_the_session_flag_and_leave_the_columns() {
    for quick in [false, true] {
        let mut a = app();
        a.max_preview = !quick;
        a.quick = quick;
        assert!(a.preview_numbers(), "T and <F3> start with numbers");
        a.act(Act::PreviewNumbers(None));
        assert!(!a.max_numbers && !a.preview_numbers());
        assert!(!a.cfg.ui.line_numbers, "the column's flag was not touched");
        a.act(Act::PreviewNumbers(None));
        assert!(a.max_numbers);
        a.act(Act::PreviewNumbers(Some(false)));
        a.act(Act::PreviewNumbers(Some(false)));
        assert!(!a.max_numbers, "`off` sets rather than flips");
        // Back in the column, the column's flag decides again.
        a.max_preview = false;
        a.quick = false;
        assert!(!a.preview_numbers());
        a.cfg.ui.line_numbers = true;
        assert!(a.preview_numbers());
    }
}

#[test]
fn the_default_keys_reach_the_commands() {
    let (km, warnings) = Keymap::load(&[]);
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(bound(&km.mgr, "#"), vec![Act::PreviewNumbers(None)]);
    assert_eq!(bound(&km.mgr, "<F2>"), bound(&km.mgr, "r"));
    assert_eq!(bound(&km.mgr, "<F2>"), vec![parse("rename")]);
    assert_eq!(bound(&km.mgr, "<C-g>"), vec![Act::PreviewGoto]);
    // `T` falls back to `[mgr]` for `#`, so the `[preview]` layer must not bind it.
    assert!(km.preview.iter().all(|b| crate::config::keys::render_seq(&b.on) != "#"), "`#` is not shadowed in [preview]");
    assert_eq!(bound(&km.preview, "<C-g>"), vec![Act::PreviewGoto]);
}

#[test]
fn rename_is_listed_once_and_a_view_change_is_not_in_the_menu() {
    let (km, _) = Keymap::load(&[]);
    let (_, details, runs) = palette_items(&km.mgr, &[]);
    let rename = parse("rename");
    assert_eq!(runs.iter().filter(|r| ***r == [rename.clone()]).count(), 1, "the palette lists rename once");
    let at = runs.iter().position(|r| **r == [rename.clone()]).unwrap();
    assert_eq!(details[at], "r", "under its first key");
    assert!(runs.iter().any(|r| **r == [Act::PreviewNumbers(None)]), "the palette lists the numbers command");

    let (_, _, runs) = menu_items(&km.mgr, &[]);
    assert_eq!(runs.iter().filter(|r| ***r == [rename.clone()]).count(), 1, "the menu lists rename once");
    assert!(!runs.iter().any(|r| r.contains(&Act::PreviewNumbers(None))), "a view change is not a file action");
    assert!(!runs.iter().any(|r| r.contains(&Act::PreviewGoto)), "nor is going to a line");
}

#[test]
fn goto_works_from_the_column_and_refuses_an_empty_preview() {
    let mut a = app();
    a.preview.state = text(300);
    a.preview_goto();
    assert!(matches!(a.overlay, Overlay::Input(_)), "the prompt opens in the column");
    assert!(a.toasts.is_empty());

    a.overlay = Overlay::None;
    a.preview.state = PreviewState::Empty;
    a.preview_goto();
    assert!(!matches!(a.overlay, Overlay::Input(_)));
    assert_eq!(a.toasts.last().map(|t| t.text.as_str()), Some("Go to line works on a text file in the preview"));
}

#[test]
fn submitting_a_line_scrolls_the_column() {
    let mut a = app();
    a.preview.state = text(300);
    a.preview.max_offset = 280;
    a.preview_goto_submit("120");
    assert_eq!(a.tabs[a.active].preview_offset, 119);
    a.preview_goto_submit("999");
    assert_eq!(a.tabs[a.active].preview_offset, 280, "clamped to the column's last offset");
    // And in <F3>, which has its own max offset but the same field.
    a.quick = true;
    a.preview.max_offset = 100;
    a.preview_goto_submit("50");
    assert_eq!(a.tabs[a.active].preview_offset, 49);
}
