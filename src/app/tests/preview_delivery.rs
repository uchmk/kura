use super::*;

/// The debounce's timer ends with the look it was for, however that look
/// ends -- not only by the worker being asked.
///
/// `main` repaints every 16 ms while `pending_since` is set, which is what
/// the timer needs to fire. `request_preview` took several early returns
/// that left it set: onto a directory, onto a file already cached, back
/// onto the file already shown. After any of those nothing ever cleared
/// it, and an idle window -- minimised, even -- drew 60 frames a second
/// for good: the "1 CPU-second per second" the Windows machine measured
/// (#86). Each case here is the cursor leaving a file whose timer is
/// running.
#[test]
fn the_debounce_timer_does_not_outlive_the_look() {
    let dir = crate::util::test_dir("debounce-timer");
    std::fs::write(dir.join("a.txt"), "a").unwrap();
    std::fs::write(dir.join("b.txt"), "b").unwrap();
    // An hour apart, on purpose. The cache key carries the file's mtime,
    // and this test once built b.txt's key from a.txt's: on Linux the two
    // were written in the same clock tick and it passed, on NTFS they were
    // not and it failed now and then. Apart, the mistake fails everywhere.
    let hour_ago = std::time::SystemTime::now() - std::time::Duration::from_secs(3600);
    std::fs::File::options().write(true).open(dir.join("b.txt")).unwrap().set_modified(hour_ago).unwrap();
    std::fs::create_dir(dir.join("sub")).unwrap();
    let entries: Vec<crate::fs::Entry> =
        ["a.txt", "b.txt", "sub"].iter().map(|n| crate::fs::Entry::from_path(dir.join(n)).unwrap()).collect();

    let fresh = || {
        let mut a = App::new(Config::load(), std::env::temp_dir(), egui::Context::default());
        a.cfg.ui.preview_debounce_ms = 10_000;
        a.tabs[a.active].current = Folder::from_entries(dir.clone(), Arc::new(entries.clone()), true);
        a
    };
    let hover = |a: &mut App, name: &str| {
        assert!(a.tabs[a.active].current.select_name(name));
        a.request_preview(false);
    };

    // Onto a directory while the timer runs.
    let mut a = fresh();
    hover(&mut a, "a.txt");
    assert!(a.preview.pending_since.is_some(), "the timer starts on a file");
    hover(&mut a, "sub");
    assert!(a.preview.pending_since.is_none(), "a directory ends it");

    // Onto a file the cache already has.
    let mut a = fresh();
    let cached = preview::Key {
        path: dir.join("b.txt"),
        len: 1,
        mtime: entries[1].modified, // b.txt's own
        box_size: a.preview.box_size,
        cols: 0,
        n: 0,
        deep: false,
        images: Default::default(),
    };
    a.preview.cache.put(cached, CachedPreview { payload: Payload::Error("x".into()), texture: None });
    hover(&mut a, "a.txt");
    assert!(a.preview.pending_since.is_some());
    hover(&mut a, "b.txt");
    assert!(matches!(a.preview.state, PreviewState::Ready(_)), "served from the cache");
    assert!(a.preview.pending_since.is_none(), "a cache hit ends it");

    // Back onto the file already on screen.
    hover(&mut a, "a.txt");
    assert!(a.preview.pending_since.is_some());
    hover(&mut a, "b.txt");
    assert!(a.preview.pending_since.is_none(), "the file already shown ends it");
}

/// The answer from the preview worker has to reach the screen, not just the
/// cache.
///
/// This is here because it did not, for six versions. v0.5.0 added the
/// image-fit seeding by replacing `state = Ready(payload)` rather than
/// following it, so every preview of a file not already cached stayed on
/// `…` for ever. The cache hid it: coming back to a file worked, because
/// `request_preview` sets `Ready` on a cache hit, so only the *first* look
/// at a file was broken and that reads like a slow load.
///
/// Asserting on `PreviewState` rather than on the cache is the whole point.
/// The old code passed every test there was.
#[test]
fn a_reply_puts_the_payload_on_screen() {
    let ctx = egui::Context::default();
    let mut app = App::new(Config::load(), std::env::temp_dir(), ctx.clone());

    let key = preview::Key {
        path: PathBuf::from("/nowhere/readme.md"),
        len: 7,
        mtime: None,
        box_size: (640, 480),
        cols: 80,
        n: 0,
        deep: false,
        images: Default::default(),
    };
    // What `request_preview` leaves behind once it has dispatched.
    app.preview.key = Some(key.clone());
    app.preview.state = PreviewState::Loading;

    app.on_preview(preview::Response { key, payload: Payload::Error("x".into()), partial: false }, &ctx);

    assert!(
        matches!(app.preview.state, PreviewState::Ready(_)),
        "a matching reply must leave the pane showing the payload, not `…`; \
             got {:?}",
        std::mem::discriminant(&app.preview.state),
    );
}

/// #166: a request that keeps the old preview up meanwhile -- a resize,
/// or an external preview's next page -- is still waited for: `settled()`
/// says no until its answer is on screen, so `<A-j><Shot:p2>` cannot
/// picture the page before.
#[test]
fn a_relayout_is_waited_for() {
    let dir = crate::util::test_dir("relayout-wait");
    std::fs::write(dir.join("a.txt"), "one\ntwo\n").unwrap();
    let ctx = egui::Context::default();
    let mut a = App::new(Config::load(), dir.clone(), ctx.clone());
    a.cfg.ui.preview_debounce_ms = 0;
    a.tabs[a.active].cwd = dir.clone();
    let entries = vec![crate::fs::Entry::from_path(dir.join("a.txt")).unwrap()];
    a.tabs[a.active].current = Folder::from_entries(dir, Arc::new(entries), true);
    let answer = |a: &mut App| {
        let deadline = Instant::now() + Duration::from_secs(10);
        while a.preview.in_flight && Instant::now() < deadline {
            if let Ok(res) = a.previewer.rx.recv_timeout(Duration::from_millis(50)) {
                a.on_preview(res, &ctx);
            }
        }
    };
    a.request_preview(true);
    assert!(a.preview.in_flight && !a.settled());
    answer(&mut a);
    assert!(matches!(a.preview.state, PreviewState::Ready(Payload::Text { .. })));
    assert!(a.settled(), "the first answer is on screen");
    a.preview.box_size.0 += 10;
    a.request_preview(true);
    assert!(matches!(a.preview.state, PreviewState::Ready(Payload::Text { .. })), "the old text stays up");
    assert!(!a.settled(), "but the new one is still to come");
    answer(&mut a);
    assert!(a.settled());
}

/// The first screen of a long text is shown at once, but it is not the
/// answer: the request is still in flight and nothing is cached until the
/// whole one comes.
#[test]
fn a_first_screen_is_shown_but_not_cached() {
    let ctx = egui::Context::default();
    let mut app = App::new(Config::load(), std::env::temp_dir(), ctx.clone());
    let key = preview::Key {
        path: PathBuf::from("/nowhere/todo.md"),
        len: 7,
        mtime: None,
        box_size: (640, 480),
        cols: 80,
        n: 0,
        deep: false,
        images: Default::default(),
    };
    app.preview.key = Some(key.clone());
    app.preview.state = PreviewState::Loading;
    app.preview.in_flight = true;
    let text = || Payload::Text { lines: vec![], map: vec![], extent: Default::default(), outline: vec![], tabs: Default::default() };

    app.on_preview(preview::Response { key: key.clone(), payload: text(), partial: true }, &ctx);
    assert!(matches!(app.preview.state, PreviewState::Ready(Payload::Text { .. })));
    assert!(app.preview.in_flight && app.preview.early && !app.settled());
    assert!(app.preview.cache.get(&key).is_none());

    app.on_preview(preview::Response { key: key.clone(), payload: text(), partial: false }, &ctx);
    assert!(!app.preview.in_flight && !app.preview.early);
    assert!(app.preview.cache.get(&key).is_some());
}

/// The other half: a reply for a file the cursor has already left is still
/// dropped, and dropping it must not knock out the preview on screen.
#[test]
fn a_stale_reply_changes_nothing() {
    let ctx = egui::Context::default();
    let mut app = App::new(Config::load(), std::env::temp_dir(), ctx.clone());

    let wanted = preview::Key {
        path: PathBuf::from("/nowhere/wanted.md"),
        len: 1,
        mtime: None,
        box_size: (640, 480),
        cols: 80,
        n: 0,
        deep: false,
        images: Default::default(),
    };
    let old = preview::Key { path: PathBuf::from("/nowhere/old.md"), ..wanted.clone() };
    app.preview.key = Some(wanted);
    app.preview.state = PreviewState::Loading;

    app.on_preview(preview::Response { key: old, payload: Payload::Error("x".into()), partial: false }, &ctx);

    assert!(matches!(app.preview.state, PreviewState::Loading), "a reply for another file must not be shown",);
}

fn timing_key(path: &str) -> preview::Key {
    preview::Key {
        path: PathBuf::from(path),
        len: 7,
        mtime: None,
        box_size: (640, 480),
        cols: 80,
        n: 0,
        deep: false,
        images: Default::default(),
    }
}

/// `preview timing:` (row 27.6): a timing starts when the cursor reaches a file, stays while it does, and is
/// replaced on another file. Going through a folder and back is a new look, timed afresh.
#[test]
fn a_timing_starts_on_the_file_and_is_replaced_on_another() {
    let dir = crate::util::test_dir("timing-start");
    std::fs::write(dir.join("a.txt"), "a").unwrap();
    std::fs::write(dir.join("b.txt"), "b").unwrap();
    std::fs::create_dir(dir.join("sub")).unwrap();
    let entries: Vec<crate::fs::Entry> =
        ["a.txt", "b.txt", "sub"].iter().map(|n| crate::fs::Entry::from_path(dir.join(n)).unwrap()).collect();
    let mut a = App::new(Config::load(), std::env::temp_dir(), egui::Context::default());
    a.cfg.ui.preview_debounce_ms = 10_000;
    a.tabs[a.active].current = Folder::from_entries(dir.clone(), Arc::new(entries), true);

    assert!(a.tabs[a.active].current.select_name("a.txt"));
    a.request_preview(false);
    let since = a.preview.timing.as_ref().expect("a timing starts").since;
    assert_eq!(a.preview.timing.as_ref().unwrap().path, dir.join("a.txt"));
    a.request_preview(false);
    assert_eq!(a.preview.timing.as_ref().unwrap().since, since, "kept while the cursor stays");

    assert!(a.tabs[a.active].current.select_name("b.txt"));
    a.request_preview(false);
    assert_eq!(a.preview.timing.as_ref().unwrap().path, dir.join("b.txt"), "replaced on another file");
    let b_since = a.preview.timing.as_ref().unwrap().since;

    assert!(a.tabs[a.active].current.select_name("sub"));
    a.request_preview(false);
    assert!(a.preview.timing.is_none(), "a folder ends the file's timing");
    std::thread::sleep(Duration::from_millis(2));
    assert!(a.tabs[a.active].current.select_name("b.txt"));
    a.request_preview(false);
    assert!(a.preview.timing.as_ref().unwrap().since > b_since, "back on the file, it is timed from now");
}

#[test]
fn a_timing_keeps_the_first_values() {
    let ctx = egui::Context::default();
    let mut app = App::new(Config::load(), std::env::temp_dir(), ctx.clone());
    let key = timing_key("/nowhere/t.md");
    app.preview.key = Some(key.clone());
    app.preview.state = PreviewState::Loading;
    app.preview.timing = Some(PreviewTiming::new(key.path.clone()));
    let text = || Payload::Text { lines: vec![], map: vec![], extent: Default::default(), outline: vec![], tabs: Default::default() };

    app.on_preview(preview::Response { key: key.clone(), payload: text(), partial: true }, &ctx);
    let t = app.preview.timing.as_ref().unwrap();
    let early = t.first.expect("the first screen is noted");
    assert_eq!(early.1, "early");
    assert!(t.whole.is_none());

    app.on_preview(preview::Response { key: key.clone(), payload: text(), partial: false }, &ctx);
    let t = app.preview.timing.as_ref().unwrap();
    assert_eq!(t.first, Some(early), "the whole reply keeps the first screen's kind and time");
    let whole = t.whole.expect("the whole one is noted");

    std::thread::sleep(Duration::from_millis(5));
    app.on_preview(preview::Response { key, payload: text(), partial: false }, &ctx);
    let t = app.preview.timing.as_ref().unwrap();
    assert_eq!((t.first, t.whole), (Some(early), Some(whole)), "a later reply moves nothing");
}

#[test]
fn a_cache_hit_is_timed_as_cached() {
    let dir = crate::util::test_dir("timing-cached");
    std::fs::write(dir.join("a.txt"), "a").unwrap();
    let entries = vec![crate::fs::Entry::from_path(dir.join("a.txt")).unwrap()];
    let mut a = App::new(Config::load(), std::env::temp_dir(), egui::Context::default());
    a.tabs[a.active].current = Folder::from_entries(dir.clone(), Arc::new(entries.clone()), true);
    let key = preview::Key {
        path: dir.join("a.txt"),
        len: 1,
        mtime: entries[0].modified,
        box_size: a.preview.box_size,
        cols: 0,
        n: 0,
        deep: false,
        images: Default::default(),
    };
    a.preview.cache.put(key, CachedPreview { payload: Payload::Error("x".into()), texture: None });
    assert!(a.tabs[a.active].current.select_name("a.txt"));
    a.request_preview(false);
    let t = a.preview.timing.as_ref().expect("timing");
    assert_eq!(t.first.map(|f| f.1), Some("cached"));
    assert!(t.whole.is_some());
}
