//! The keys for the pane's prompt marks, the same chords tsumugi uses.

use super::*;

fn run_of(layer: &[keymap::Binding], key: &str) -> Vec<Act> {
    layer.iter().find(|b| crate::config::keys::render_seq(&b.on) == key).unwrap_or_else(|| panic!("`{key}` is not bound")).run.clone()
}

#[test]
fn the_pane_binds_prompt_jumps_and_copying_the_output() {
    let (km, warnings) = keymap::Keymap::load(&[]);
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(run_of(&km.term, "<C-S-Up>"), vec![Act::TermPrompt { prev: true }]);
    assert_eq!(run_of(&km.term, "<C-S-Down>"), vec![Act::TermPrompt { prev: false }]);
    assert_eq!(run_of(&km.term, "<C-S-l>"), vec![Act::TermCopyOutput]);
}
