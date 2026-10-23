use super::*;

/// `g`+`f` on an ordinary file used to be indistinguishable from an
/// unbound key.
///
/// The row does say so — a link is drawn with `->` after its name — but
/// only if you already know to look, and nothing said that. The message
/// names both the one kind of thing the key works on and the mark that
/// identifies it.
#[test]
fn an_ordinary_file_is_told_that_it_is_not_a_link() {
    let dir = crate::util::test_dir("follow-msg");
    let plain = dir.join("plain.txt");
    std::fs::write(&plain, "x").unwrap();

    let ctx = egui::Context::default();
    let mut a = App::new(Config::load(), std::env::temp_dir(), ctx);
    a.tabs[a.active].cwd = dir.clone();
    a.tabs[a.active].current = Folder::from_entries(dir.clone(), Arc::new(vec![crate::fs::Entry::from_path(plain).unwrap()]), true);

    a.act(Act::Follow);

    let said = a.toasts.last().expect("it says something now");
    assert_eq!(said.level, Level::Error, "the key could not do its job");
    assert!(said.text.contains("symlink"), "it names what the key is for: {}", said.text);
    assert!(said.text.contains("->"), "and how to spot one: {}", said.text);

    // Nothing under the cursor at all stays quiet: there is no row to
    // describe, and every other key is silent there too.
    a.tabs[a.active].current = Folder::from_entries(dir.clone(), Arc::new(Vec::new()), true);
    a.toasts.clear();
    a.act(Act::Follow);
    assert!(a.toasts.is_empty(), "an empty directory says nothing");

    let _ = std::fs::remove_dir_all(&dir);
}
