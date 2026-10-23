use super::*;

fn app() -> App {
    let ctx = egui::Context::default();
    App::new(Config::load(), std::env::temp_dir(), ctx)
}

/// The panel describes whatever is hovered, so it has to be asked about it.
///
/// `h` and `l` move to another directory, and which file ends up hovered
/// there is decided by a listing that arrives later on a worker thread —
/// asking at the moment the key is pressed asks about nothing at all.
/// `sync_spot` runs once a frame for exactly that, and the assertion is on
/// what the worker was asked, because the panel is silently right either
/// way: its first section is read from the listing live, so a stale worker
/// answer just leaves the per-type rows missing.
#[test]
fn a_new_hover_is_asked_about_once_it_exists() {
    let mut a = app();
    let dir = crate::util::test_dir("spot-follow");
    let one = dir.join("one.txt");
    let two = dir.join("two.txt");
    std::fs::write(&one, "1").unwrap();
    std::fs::write(&two, "2").unwrap();

    let entries = Arc::new(vec![crate::fs::Entry::from_path(one.clone()).unwrap(), crate::fs::Entry::from_path(two.clone()).unwrap()]);
    a.tabs[a.active].current = Folder::from_entries(dir.clone(), entries, true);
    a.overlay = Overlay::Spot(SpotOverlay { cursor: 0, scroll: 0 });

    a.sync_spot();
    assert_eq!(a.spot_asked.as_deref(), Some(one.as_path()), "the hovered one");

    // Asking again for the same file does not re-ask.
    a.spot_asked = None;
    a.sync_spot();
    assert_eq!(a.spot_asked.as_deref(), Some(one.as_path()));

    // Moving the cursor moves the panel with it.
    a.spot_act(Act::Swipe(1));
    assert_eq!(a.spot_asked.as_deref(), Some(two.as_path()), "it followed the cursor");

    // Closing the panel lets go, so reopening asks afresh.
    a.overlay = Overlay::None;
    a.sync_spot();
    assert_eq!(a.spot_asked, None);

    let _ = std::fs::remove_dir_all(&dir);
}
