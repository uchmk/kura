use crate::config::keymap;

/// `g`+`c` goes to kura's own directory, not yazi's.
///
/// Both are read, but they hold different things: `config.toml` only ever
/// belongs in kura's, and that is the one a reader cannot find, because
/// it is often the directory that does not exist yet. yazi's keeps its own
/// key rather than the shared one.
#[test]
fn the_two_config_directories_have_a_key_each() {
    let (km, warnings) = keymap::Keymap::load(&[]);
    assert!(warnings.is_empty(), "{warnings:?}");
    let run = |key: &str| {
        km.mgr.iter().find(|b| crate::config::keys::render_seq(&b.on) == key).unwrap_or_else(|| panic!("`{key}` is not bound")).raw.clone()
    };
    // Written as the variables kura actually searches, so they resolve on
    // every platform. `%APPDATA%` named nothing outside Windows, and an unset
    // `%VAR%` expands to nothing, so `gc` used to walk to `/kura` there.
    assert_eq!(run("gc"), "cd %KURA_CONFIG_HOME%");
    assert_eq!(run("gy"), "cd %YAZI_CONFIG_HOME%");
    for (key, var) in [("gc", "KURA_CONFIG_HOME"), ("gy", "YAZI_CONFIG_HOME")] {
        let dir = crate::config::config_home(var).expect("a config directory");
        assert_eq!(crate::util::expand(&format!("%{var}%")), dir, "{key}");
        assert!(dir.is_absolute(), "{key} must not land on a relative path: {dir:?}");
    }
    // The pair is the same two directories the help panel and `kura env` list.
    assert_eq!(
        crate::config::config_dirs(),
        vec![crate::config::config_home("YAZI_CONFIG_HOME").unwrap(), crate::config::config_home("KURA_CONFIG_HOME").unwrap(),]
    );

    // The pair the arrows reach, alongside the `H`/`L` that already did.
    assert_eq!(run("<A-Left>"), "back");
    assert_eq!(run("<A-Right>"), "forward");
    assert_eq!(run("H"), "back");
    assert_eq!(run("L"), "forward");
}

/// `T` maximizes the preview column, and is spelled as the shifted character.
///
/// The command had a parser and an implementation but no key, so the feature
/// was only reachable by writing a `prepend_keymap` line by hand. Pinned
/// through the keymap rather than the parser, because the two spellings of a
/// shifted letter parse equally well and only one of them ever matches.
#[test]
fn t_maximizes_the_preview_pane() {
    let (km, warnings) = keymap::Keymap::load(&[]);
    assert!(warnings.is_empty(), "{warnings:?}");
    let binding = km.mgr.iter().find(|b| crate::config::keys::render_seq(&b.on) == "T").expect("`T` is not bound");
    assert_eq!(binding.raw, "plugin toggle-pane max-preview");
    assert_eq!(crate::config::cmd::parse(&binding.raw), crate::config::cmd::Act::MaxPreview);

    // `t` is the new tab it sits next to; shifting it must not have moved it.
    let lower = km.mgr.iter().find(|b| crate::config::keys::render_seq(&b.on) == "t").expect("`t` is not bound");
    assert_eq!(lower.raw, "tab_create --current");
}
