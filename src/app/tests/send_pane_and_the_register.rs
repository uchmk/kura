use super::*;

/// `<A-c>` leaves the yank register alone, which the README now promises.
///
/// It is the reason the key is not `<A-y>`: *yank* means "into the
/// register", and this never goes near it — so `p` after an `<A-c>` still
/// pastes whatever `y` last held. A name built on `y` would promise a `p`
/// that is not wanted and an overwrite that does not happen, and if this
/// ever started clearing or replacing the register, that promise would be
/// the thing that broke.
#[test]
fn a_send_does_not_disturb_what_is_yanked() {
    let root = crate::util::test_dir("send-pane");
    let (left, right) = (root.join("left"), root.join("right"));
    let _ = std::fs::create_dir_all(&left);
    let _ = std::fs::create_dir_all(&right);
    let sent = left.join("sent.txt");
    std::fs::write(&sent, "x").unwrap();

    let ctx = egui::Context::default();
    let mut a = App::new(Config::load(), std::env::temp_dir(), ctx);
    a.tabs[a.active].cwd = left.clone();
    a.tabs[a.active].current = Folder::from_entries(left.clone(), Arc::new(vec![crate::fs::Entry::from_path(sent.clone()).unwrap()]), true);
    // A second tab, shown in the other pane.
    let sort = a.tabs[a.active].sort;
    let mut other = crate::core::tab::Tab::new(right.clone(), sort, false, crate::fs::entry::Linemode::None);
    other.cwd = right.clone();
    a.tabs.push(other);
    a.split = Some(Split { other: 1, right: false });

    // Something else is held in the register.
    let held = PathBuf::from("held.txt");
    a.yank = Yank { paths: vec![held.clone()], cut: false, from_archive: false };

    a.act(Act::SendPane { cut: false });

    // Both of `send_to_pane`'s refusals leave the register alone too, so
    // without this the assertion below would pass on a send that never
    // happened.
    assert!(
        !a.toasts.iter().any(|t| t.level == Level::Error),
        "the send went through: {:?}",
        a.toasts.iter().map(|t| &t.text).collect::<Vec<_>>(),
    );
    assert!(!a.tasks.is_empty(), "a copy was actually queued");

    assert_eq!(a.yank.paths, vec![held], "the register is untouched by a send");
    assert!(!a.yank.cut, "and so is what it is holding it for");

    let _ = std::fs::remove_dir_all(&root);
}
