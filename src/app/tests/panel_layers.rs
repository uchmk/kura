//! The one thing a copy-paste could get wrong in `PanelLayer` and no other test
//! would notice: a layer wired to somebody else's keymap section. Pointer
//! equality, so it is the section itself and not a section that happens to look
//! like it.

use super::*;

#[test]
fn each_panel_reads_its_own_section() {
    let km = &Config::load().keymap;
    for (layer, want, name) in [
        (PanelLayer::Help, &km.help, "help"),
        (PanelLayer::Tasks, &km.tasks, "tasks"),
        (PanelLayer::Spot, &km.spot, "spot"),
        (PanelLayer::Diff, &km.diff, "diff"),
        (PanelLayer::Settings, &km.settings, "settings"),
    ] {
        let got = layer.bindings(km);
        assert!(std::ptr::eq(got, want.as_slice()), "{layer:?} should read `[{name}]`");
        assert!(!got.is_empty(), "`[{name}]` has default bindings, so this is wired up");
    }
}

/// The overlays that are native widgets, or no overlay at all, have no layer
/// -- so a key pressed there must not be resolved against somebody else's.
#[test]
fn the_widget_overlays_have_no_layer() {
    assert_eq!(PanelLayer::of(&Overlay::None), None);
    assert_eq!(PanelLayer::of(&Overlay::Help), Some(PanelLayer::Help));
}
