//! `<A-j>` / `<A-k>` scroll what is being read, in every pane that reads.

use super::*;

fn run_of(layer: &[keymap::Binding], key: &str) -> Vec<Act> {
    layer.iter().find(|b| crate::config::keys::render_seq(&b.on) == key).unwrap_or_else(|| panic!("`{key}` is not bound")).run.clone()
}

/// The terminal was the one pane where they were not bound, so they went
/// through to the shell and the pane looked like the odd one out. Pinned
/// against the list's own pair, because the two drifting apart is the
/// failure: same direction, same distance, different pane.
#[test]
fn the_terminal_scrolls_the_way_the_list_does() {
    let (km, warnings) = keymap::Keymap::load(&[]);
    assert!(warnings.is_empty(), "{warnings:?}");

    assert_eq!(run_of(&km.mgr, "<A-j>"), vec![Act::Seek(Step::Rel(5))]);
    assert_eq!(run_of(&km.mgr, "<A-k>"), vec![Act::Seek(Step::Rel(-5))]);
    assert_eq!(run_of(&km.term, "<A-j>"), vec![Act::TermScroll(Step::Rel(5))]);
    assert_eq!(run_of(&km.term, "<A-k>"), vec![Act::TermScroll(Step::Rel(-5))]);

    // `feed_term_key` only ever consults single-key bindings, so a chord
    // added here would be read by nothing and reach the shell instead.
    assert!(km.term.iter().all(|b| b.on.len() == 1));
}

/// The help panel moves by half its own height rather than five lines, so
/// this asserts the direction only -- `j` down, `k` up, as everywhere else.
#[test]
fn the_help_panel_agrees_on_which_way_is_down() {
    let (km, _) = keymap::Keymap::load(&[]);
    let down = run_of(&km.help, "<A-j>");
    let up = run_of(&km.help, "<A-k>");
    let step = |acts: &[Act]| match acts.first() {
        Some(Act::Arrow(s)) => *s,
        other => panic!("expected an arrow, got {other:?}"),
    };
    assert!(matches!(step(&down), Step::Pct(n) if n > 0), "{down:?}");
    assert!(matches!(step(&up), Step::Pct(n) if n < 0), "{up:?}");
}
