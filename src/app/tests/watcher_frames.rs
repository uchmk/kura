use super::*;

/// #108: a directory the watcher flagged asks for the frame that will read
/// it, no sooner than the quiet period and not much later; with nothing
/// flagged, nothing is asked for, so an idle window stays idle (47).
#[test]
fn a_flagged_directory_asks_for_its_frame() {
    let mut a = App::new(Config::load(), std::env::temp_dir(), egui::Context::default());
    a.dirty.clear();
    assert_eq!(a.rescan_due(), None, "nothing flagged, nothing owed");

    a.dirty.insert(std::env::temp_dir(), Instant::now());
    let due = a.rescan_due().expect("a frame is owed");
    assert!(due >= App::RESCAN_QUIET, "not before the quiet period: {due:?}");
    assert!(due < App::RESCAN_QUIET * 2, "and not long after it: {due:?}");

    std::thread::sleep(App::RESCAN_QUIET + Duration::from_millis(20));
    a.flush_dirty();
    assert_eq!(a.rescan_due(), None, "read, so nothing more is owed");
}
