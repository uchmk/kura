use super::*;

/// An app on a folder with one long text file hovered, whose preview has been
/// asked for and has "arrived" as an empty `Ready(Text)` (what matters here is
/// the slot, not the lines).
fn on_show(label: &str, debounce_ms: u64) -> App {
    let dir = crate::util::test_dir(label);
    std::fs::write(dir.join("long.txt"), "line\n".repeat(500)).unwrap();
    let entries = vec![crate::fs::Entry::from_path(dir.join("long.txt")).unwrap()];
    let mut a = App::new(Config::load(), crate::util::test_tmp(), egui::Context::default());
    a.cfg.ui.preview_debounce_ms = debounce_ms;
    a.tabs[a.active].current = Folder::from_entries(dir, Arc::new(entries), true);
    assert!(a.tabs[a.active].current.select_name("long.txt"));
    a.request_preview(false);
    if debounce_ms > 0 {
        a.preview.pending_since = Some(Instant::now() - Duration::from_millis(debounce_ms + 1));
        a.request_preview(false);
    }
    assert!(a.preview.key.is_some() && a.preview.in_flight, "the first look was asked for");
    a.preview.state = PreviewState::Ready(Payload::Text {
        lines: vec![],
        map: vec![],
        extent: Default::default(),
        outline: vec![],
        tabs: Default::default(),
    });
    a.preview.in_flight = false;
    a.preview.max_offset = 400;
    a.tabs[a.active].preview_offset = 40;
    a
}

/// #530: a theme switch threw the slot away, the next frame drew `Loading`,
/// and that clamped the scroll to the first line.
#[test]
fn a_theme_switch_rereads_the_file_and_keeps_the_scroll() {
    let mut a = on_show("theme-scroll", 0);
    let id = a.preview.request_id;
    let key = a.preview.key.clone().unwrap();
    a.preview.cache.put(key.clone(), CachedPreview { payload: Payload::Error("old colours".into()), texture: None });

    a.look_changed();
    assert!(matches!(a.preview.state, PreviewState::Ready(Payload::Text { .. })), "the old copy stays up");
    assert!(a.preview.cache.get(&key).is_none(), "the copy in the old colours is gone");

    a.request_preview(false);
    assert!(matches!(a.preview.state, PreviewState::Ready(Payload::Text { .. })), "never `Loading`");
    assert!(a.preview.in_flight && a.preview.request_id != id, "the file is read again");
    assert!(!a.preview.recolor, "and only once");
    assert_eq!(a.tabs[a.active].preview_offset, 40);
}

/// With the default debounce the forced request used to be lost: the first
/// frame only started the timer, and the next one found the key unchanged.
#[test]
fn the_reread_survives_the_debounce() {
    let mut a = on_show("theme-debounce", 40);
    let id = a.preview.request_id;

    a.look_changed();
    a.request_preview(false);
    assert!(a.preview.pending_since.is_some() && !a.preview.in_flight, "waiting on the timer");
    a.request_preview(false);
    assert!(a.preview.recolor, "still wanted on the next frame");
    assert!(matches!(a.preview.state, PreviewState::Ready(_)));

    a.preview.pending_since = Some(Instant::now() - Duration::from_millis(41));
    a.request_preview(false);
    assert!(a.preview.in_flight && a.preview.request_id != id, "sent once the debounce has elapsed");
    assert!(!a.preview.recolor);
    assert_eq!(a.tabs[a.active].preview_offset, 40);
}

#[test]
fn a_theme_switch_keeps_the_zoom_and_the_page() {
    let mut a = on_show("theme-zoom", 0);
    a.preview.zoom = Some(3.0);
    a.preview.pan = egui::vec2(5.0, 7.0);
    a.preview.n = 2;
    a.look_changed();
    a.request_preview(false);
    assert_eq!(a.preview.zoom, Some(3.0));
    assert_eq!(a.preview.pan, egui::vec2(5.0, 7.0));
    assert_eq!(a.preview.n, 2);
}

/// A log holds no colours from the worker, so a new scheme has nothing to
/// re-read, and reading it again would drop the find running on it.
#[test]
fn a_log_on_show_is_not_reread_for_a_theme_switch() {
    let mut a = on_show("theme-log", 0);
    let path = a.preview.key.as_ref().unwrap().path.clone();
    let index = crate::preview::logindex::LogIndex::start(&path).unwrap();
    a.preview.state = PreviewState::Ready(Payload::Log { view: crate::preview::logindex::LogView::new(index) });
    let id = a.preview.request_id;

    a.look_changed();
    a.request_preview(false);
    assert!(matches!(a.preview.state, PreviewState::Ready(Payload::Log { .. })));
    assert!(!a.preview.in_flight && a.preview.request_id == id, "no new request");
    assert!(!a.preview.recolor, "and the wish does not linger");
}

/// The old colours' answer can land after the switch: it is accepted (the key
/// matches) and cached, and the cache hit used to swallow the wanted re-read.
#[test]
fn an_old_answer_landing_after_the_switch_does_not_swallow_the_reread() {
    let mut a = on_show("theme-stale", 40);
    let ctx = egui::Context::default();
    let key = a.preview.key.clone().unwrap();
    let id = a.preview.request_id;
    a.preview.in_flight = true;

    a.look_changed();
    a.request_preview(false);
    assert!(a.preview.pending_since.is_some() && a.preview.recolor, "waiting on the debounce");

    let old = Payload::Text { lines: vec![], map: vec![], extent: Default::default(), outline: vec![], tabs: Default::default() };
    a.on_preview(preview::Response { key: key.clone(), payload: old, partial: false }, &ctx);
    assert!(a.preview.cache.get(&key).is_some(), "the old copy was cached");

    a.preview.pending_since = Some(Instant::now() - Duration::from_millis(41));
    a.request_preview(false);
    assert!(a.preview.in_flight && a.preview.request_id != id, "the re-read still goes out");
    assert!(!a.preview.recolor);
    assert_eq!(a.tabs[a.active].preview_offset, 40);
}
