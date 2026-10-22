//! `kura --keys "<Tab>C"`: keys pressed by kura itself, once it has started.
//!
//! The script's words, the events they become and the reports a run leaves in
//! `KURA_KEYS_DONE` live in ito's `ito-keyscript`, shared with the other
//! uchmk apps. What is kura's own stays here: the readings of the terminal
//! pane and the preview (`<PaneText:name>`, `<PreviewText:name>`), the
//! `@preview` crop of a shot, and the clipboard `<Paste>` reads. The frame
//! loop that plays the script is `main.rs`'s, because it waits for kura's
//! panes to settle between keys.
//!
//! The real-machine test sessions drove kura from outside, and every run
//! rebuilt the same Win32 scaffolding to do it. This does it from inside, for
//! a kura this command line starts, and for no other: it opens no door into a
//! kura that is already running (Q24).

pub use ito_keyscript::*;

/// The suffix that crops a shot to the preview pane.
pub const SHOT_PREVIEW: &str = "@preview";

/// What kura adds to the common script words.
pub const DIALECT: Dialect<'static> =
    Dialect { reads: &[("PaneText", "pane text"), ("PreviewText", "preview text")], shot_suffixes: &[SHOT_PREVIEW] };

/// [`ito_keyscript::parse_with`] with kura's [`DIALECT`].
pub fn parse(script: &str) -> Result<Vec<Step>, String> {
    parse_with(script, &DIALECT)
}

/// Ctrl+V as the platform delivers it: the clipboard's text, now.
pub fn paste_event() -> egui::Event {
    egui::Event::Paste(crate::exec::get_clipboard().unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::keys::Key;

    /// The common crate's tests cover the script itself; these are what
    /// needs kura: the dialect, and the real frame loop.
    #[test]
    fn the_dialect_is_kuras() {
        assert_eq!(parse("<Shot:p@preview>").unwrap(), [Step::Shot("p@preview".into())]);
        assert!(parse("<PreviewText:p>").is_ok());
        assert!(parse("<Size:480x600><Theme:light><Lang:ja>").is_ok(), "the common window and settings steps");
    }

    /// The list takes `<Space>` once: the key event is not a key to it, so the
    /// text alone toggles the file and moves on.
    #[test]
    fn space_selects_one_file_in_the_list() {
        let dir = crate::util::test_dir("keyscript-space");
        for name in ["a.txt", "b.txt", "c.txt"] {
            std::fs::write(dir.join(name), name).unwrap();
        }
        let mut s = crate::ui::harness::Screen::open(dir);
        s.settle();
        s.feed(events(&Key::parse("<Space>").unwrap()).unwrap());
        assert_eq!(s.app.tab().selected.len(), 1, "one file toggled");
        assert_eq!(s.app.tab().current.cursor, 1, "and the cursor moved one row");
    }

    /// The events reach the app through the door a keyboard uses: `<Tab>` opens
    /// the spot panel and `q` closes it, in a real frame loop.
    #[test]
    fn scripted_keys_drive_the_real_frame_loop() {
        let dir = crate::util::test_dir("keyscript");
        std::fs::write(dir.join("a.txt"), "a").unwrap();
        let mut s = crate::ui::harness::Screen::open(dir);
        s.settle();
        let key = |t| Key::parse(t).unwrap();
        s.feed(events(&key("<Tab>")).unwrap());
        assert!(matches!(s.app.overlay, crate::app::Overlay::Spot(_)), "`<Tab>` opened spot");
        s.feed(events(&key("q")).unwrap());
        assert!(s.app.overlay.is_none(), "`q` closed it");
    }

    /// #260: `<C-v>` and `<Paste>` paste into a prompt, as the platform's
    /// paste event, with the clipboard as it is when they are pressed.
    #[test]
    fn ctrl_v_pastes_into_a_prompt() {
        let dir = crate::util::test_dir("keyscript-paste");
        let mut s = crate::ui::harness::Screen::open(dir);
        s.settle();
        let key = |t| Key::parse(t).unwrap();
        assert_eq!(press(&Step::Key(key("<C-v>"))), Some(Press::Paste));
        assert_eq!(parse("<Paste>").unwrap(), [Step::Paste]);
        crate::exec::fake_clipboard("pasted-name");
        s.feed(events(&key("g")).unwrap());
        s.feed(events(&key("<Space>")).unwrap());
        s.feed(vec![paste_event()]);
        match &s.app.overlay {
            crate::app::Overlay::Input(ov) => assert!(ov.text.contains("pasted-name"), "{:?}", ov.text),
            _ => panic!("the cd prompt is not open"),
        }
    }

    /// #229: `<PaneText:name>` parses and keeps its name.
    #[test]
    fn pane_text_is_a_named_step() {
        let read = |name: &str| Step::Read { tag: "PaneText".into(), name: name.into() };
        assert_eq!(parse("<PaneText:footer>").unwrap(), [read("footer")]);
        assert_eq!(label(&read("footer")), "<PaneText:footer>");
        assert!(parse("<PaneText:>").unwrap_err().contains("not a pane text"));
        assert!(parse("<PaneText:../x>").is_err());
    }
}
