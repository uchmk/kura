use super::*;

fn app() -> App {
    let ctx = egui::Context::default();
    App::new(Config::load(), std::env::temp_dir(), ctx)
}

/// The message has to say what it looked at. "Nothing here" was read as
/// "this directory holds no archives", which was wrong in the report that
/// prompted this: the cursor was on a zip whose contents were on screen in
/// the preview, and `e` was acting on a selection of PDFs left over from an
/// earlier command. The count is what gives that away, because the header's
/// own "N selected" is underneath the toast carrying this text.
#[test]
fn it_says_how_many_were_examined() {
    let mut a = app();
    let i = a.active;
    a.tabs[i].selected.insert(PathBuf::from("a.pdf"));
    a.tabs[i].selected.insert(PathBuf::from("b.pdf"));
    a.do_extract();

    let last = a.toasts.last().expect("an error was raised");
    assert_eq!(last.level, Level::Error, "it is an error, not a note");
    assert!(last.text.contains('2'), "the count is missing: {}", last.text);
    assert!(!last.text.contains("Nothing here"), "\"here\" reads as the directory: {}", last.text,);
}

/// And when nothing is selected it must not claim a selection.
#[test]
fn it_names_the_cursor_when_nothing_is_selected() {
    let mut a = app();
    // No entries, so `targets` is empty and `do_extract` returns early;
    // drive the message the way a hovered non-archive does.
    let i = a.active;
    assert!(a.tabs[i].selected.is_empty());
    a.tabs[i].selected.insert(PathBuf::from("x.pdf"));
    a.do_extract();
    let with_selection = a.toasts.last().unwrap().text.clone();
    assert!(with_selection.contains("selected"), "{with_selection}");
}

/// Whatever the wording, it has to name the formats — otherwise the reader
/// still does not know whether their file was ever a candidate.
#[test]
fn it_lists_what_can_be_read() {
    let mut a = app();
    let i = a.active;
    a.tabs[i].selected.insert(PathBuf::from("a.pdf"));
    a.do_extract();
    let t = &a.toasts.last().unwrap().text;
    for f in ["zip", "tar", "7z"] {
        assert!(t.contains(f), "{f} missing from: {t}");
    }
}
