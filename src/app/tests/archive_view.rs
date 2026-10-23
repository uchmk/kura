//! Going into an archive with `l` (Q75): its members on a `Folder` whose path
//! is not a real directory, the way the usage view is.

use super::*;

/// An archive of `docs/a/readme.md` and `top.txt`, and an app in its
/// folder with the cursor on it.
fn app_on_archive() -> (App, PathBuf) {
    let dir = crate::util::test_dir("archive-view");
    let src = dir.join("src");
    std::fs::create_dir_all(src.join("docs").join("a")).unwrap();
    std::fs::write(src.join("docs").join("a").join("readme.md"), "read me").unwrap();
    std::fs::write(src.join("top.txt"), "top").unwrap();
    let zip = dir.join("pack.zip");
    archive::compress(&[src.join("docs"), src.join("top.txt")], &src, &zip, archive::Format::Zip, &mut |_, _| true).unwrap();
    let mut a = App::new(Config::load(), dir.clone(), egui::Context::default());
    a.tabs[a.active].cwd = dir.clone();
    let entries = vec![Entry::from_path(zip.clone()).unwrap()];
    a.tabs[a.active].current = Folder::from_entries(dir, Arc::new(entries), true);
    (a, zip)
}

fn names(a: &App) -> Vec<String> {
    a.tabs[a.active].current.view.iter().filter_map(|&i| a.tabs[a.active].current.entries.get(i as usize)).map(|e| e.name.clone()).collect()
}

/// The listing arrives from a worker.
fn wait(a: &mut App) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while a.archive_rx.is_some() && Instant::now() < deadline {
        a.drain_archive();
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn l_goes_in_and_h_comes_back_out() {
    let (mut a, zip) = app_on_archive();
    a.act(Act::Enter);
    assert!(a.in_archive_view() && a.in_search_view(), "a view, not a directory");
    wait(&mut a);
    assert_eq!(names(&a), ["docs", "top.txt"]);
    a.act(Act::Enter);
    a.act(Act::Enter);
    assert_eq!(names(&a), ["readme.md"], "two levels down");
    assert!(a.archive_at().is_some_and(|p| p.ends_with(Path::new("pack.zip").join("docs").join("a"))), "{:?}", a.archive_at());
    a.act(Act::Leave);
    assert_eq!(names(&a), ["a"]);
    assert_eq!(a.tabs[a.active].current.hovered().map(|e| e.name.as_str()), Some("a"), "the cursor on the folder come up out of");
    a.act(Act::Leave);
    a.act(Act::Leave);
    assert!(!a.in_archive_view(), "`h` at the top leaves");
    // Back on the real folder, listed afresh, with the cursor remembered
    // on the archive for when the listing lands.
    let dir = zip.parent().unwrap().to_path_buf();
    assert_eq!(a.tabs[a.active].current.path, dir);
    assert_eq!(a.tabs[a.active].memo.get(&dir).map(String::as_str), Some("pack.zip"));
}

/// Each token one key through the list's keymap, as the window sends it.
fn keys(a: &mut App, tokens: &[&str]) {
    for t in tokens {
        a.feed_key(crate::config::keys::Key::parse(t).expect("notation the keymap can spell"));
    }
}

fn hovered(a: &App) -> Option<String> {
    a.tabs[a.active].current.hovered().map(|e| e.name.clone())
}

/// TESTING.md 21.17 (Q75): `l` / `h` and `<Right>` / `<Left>` through
/// the keymap go in, down, up with the cursor on the folder come out
/// of, and out at the top; `<Esc>` leaves from any level.
#[test]
fn the_arrow_keys_and_esc_walk_the_archive() {
    let (mut a, zip) = app_on_archive();
    let dir = zip.parent().unwrap().to_path_buf();
    keys(&mut a, &["l"]);
    wait(&mut a);
    assert!(a.in_archive_view());
    assert_eq!(names(&a), ["docs", "top.txt"]);
    assert!(a.tabs[a.active].current.hovered().is_some_and(|e| e.path == zip.join("docs")), "hovered: names the member under the archive");
    keys(&mut a, &["<Right>"]);
    assert_eq!(names(&a), ["a"]);
    keys(&mut a, &["l"]);
    assert_eq!(names(&a), ["readme.md"]);
    keys(&mut a, &["h"]);
    assert_eq!((names(&a), hovered(&a)), (vec!["a".to_owned()], Some("a".to_owned())));
    keys(&mut a, &["<Left>"]);
    assert_eq!((names(&a), hovered(&a)), (vec!["docs".to_owned(), "top.txt".to_owned()], Some("docs".to_owned())));
    keys(&mut a, &["<Left>"]);
    assert!(!a.in_archive_view(), "`<Left>` at the top leaves");
    assert_eq!(a.tabs[a.active].current.path, dir);
    assert_eq!(a.tabs[a.active].memo.get(&dir).map(String::as_str), Some("pack.zip"));

    // `<Esc>` from two levels down leaves the whole view at once.
    let entries = vec![Entry::from_path(zip.clone()).unwrap()];
    a.tabs[a.active].current = Folder::from_entries(dir.clone(), Arc::new(entries), true);
    keys(&mut a, &["<Right>"]);
    wait(&mut a);
    keys(&mut a, &["l", "l"]);
    assert_eq!(names(&a), ["readme.md"]);
    keys(&mut a, &["<Esc>"]);
    assert!(!a.in_archive_view(), "`<Esc>` leaves from any level");
    assert_eq!(a.tabs[a.active].current.path, dir);
}

/// TESTING.md 21.19: `x`, `d`, `a`, `r` and `p` through the keymap each
/// say the archive is read only, open nothing, and leave the folder the
/// archive sits in as it was.
#[test]
fn the_writing_keys_are_refused_inside_an_archive() {
    let (mut a, zip) = app_on_archive();
    let dir = zip.parent().unwrap().to_path_buf();
    fn tree(d: &Path) -> Vec<PathBuf> {
        let mut all = Vec::new();
        for e in std::fs::read_dir(d).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                all.extend(tree(&p));
            }
            all.push(p);
        }
        all.sort();
        all
    }
    let before = tree(&dir);
    let zip_bytes = std::fs::read(&zip).unwrap();
    // Something in the register, so `p` has something it could paste.
    a.yank.paths = vec![dir.join("src").join("top.txt")];
    keys(&mut a, &["l"]);
    wait(&mut a);
    for k in ["x", "d", "a", "r", "p"] {
        a.toasts.clear();
        keys(&mut a, &[k]);
        let said: Vec<&String> = a.toasts.iter().map(|t| &t.text).collect();
        assert_eq!(
                said,
                ["Inside an archive: read only — this would change the archive, which kura never writes. y then p in a folder takes a copy out, Esc leaves"],
                "`{k}`",
            );
        assert!(matches!(a.overlay, Overlay::None), "`{k}` opened nothing");
        assert!(a.in_archive_view(), "`{k}` stays in the view");
    }
    assert!(!a.yank.cut, "nothing was cut");
    std::thread::sleep(Duration::from_millis(50));
    assert_eq!(tree(&dir), before, "the folder is unchanged");
    assert_eq!(std::fs::read(&zip).unwrap(), zip_bytes, "and so is the archive");
}

/// Nothing that writes runs on rows that are not files yet.
#[test]
fn inside_an_archive_is_read_only() {
    let (mut a, _) = app_on_archive();
    a.act(Act::Enter);
    wait(&mut a);
    a.act(Act::Yank { cut: true });
    assert!(a.yank.paths.is_empty(), "nothing cut: it would take it out of the archive");
    assert!(
        a.toasts.iter().any(|t| t.text.starts_with("Inside an archive: read only")),
        "{:?}",
        a.toasts.iter().map(|t| &t.text).collect::<Vec<_>>()
    );
}

/// `y` on a member, then `p` in a folder: the member comes out under its
/// own name, a folder with what is under it, and the register stays.
#[test]
fn y_then_p_takes_a_member_out() {
    let (mut a, zip) = app_on_archive();
    a.act(Act::Enter);
    wait(&mut a);
    // `docs` is the first row: take the folder out.
    a.act(Act::Yank { cut: false });
    assert!(a.yank.from_archive && a.yank.paths.len() == 1, "{:?}", a.yank.paths);
    let out = zip.parent().unwrap().join("out");
    std::fs::create_dir_all(&out).unwrap();
    a.exit_search_view();
    a.cd(out.clone(), true);
    a.act(Act::Paste { force: false, follow: false });
    let deadline = Instant::now() + Duration::from_secs(10);
    while !out.join("docs").join("a").join("readme.md").exists() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(std::fs::read_to_string(out.join("docs").join("a").join("readme.md")).unwrap(), "read me");
    assert!(!out.join("top.txt").exists(), "only what was yanked");
    assert!(a.yank.from_archive, "the register stays, as a copy's does");
    // Nothing of the job's own left behind.
    let deadline = Instant::now() + Duration::from_secs(5);
    while std::fs::read_dir(&out).unwrap().count() > 1 && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    let names: Vec<String> = std::fs::read_dir(&out).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    assert_eq!(names, ["docs"]);
}

/// A small member is unpacked for the preview and previewed as the copy;
/// leaving the view takes the copies with it.
#[test]
fn a_member_is_previewed_from_a_copy() {
    let (mut a, zip) = app_on_archive();
    a.act(Act::Enter);
    wait(&mut a);
    a.act(Act::Arrow(Step::Rel(1)));
    assert_eq!(a.tabs[a.active].current.hovered().map(|e| e.name.as_str()), Some("top.txt"));
    a.request_preview(true);
    let deadline = Instant::now() + Duration::from_secs(10);
    while a.archive_preview_rx.is_some() && Instant::now() < deadline {
        a.drain_archive();
        std::thread::sleep(Duration::from_millis(5));
    }
    let scratch = a.archive_view.as_ref().map(|v| v.scratch.clone()).expect("a view");
    let real = a.archive_view.as_ref().and_then(|v| v.copies.get(&zip.join("top.txt")).cloned()).expect("a copy");
    assert_eq!(std::fs::read_to_string(&real).unwrap(), "top");
    assert!(matches!(&a.preview.key, Some(k) if k.path == real) || a.preview.pending_since.is_some(), "the copy is what is previewed");
    a.act(Act::Escape(EscapeWhat::default()));
    assert!(!real.exists(), "the copy went with the view");
    // Its own folder went too. The folders above it are other tests' as
    // well, so they are not looked at here (found flaky: another view
    // makes `preview` a moment before its own folder in it); see
    // `a_view_takes_the_empty_folders_above_its_copies_with_it`.
    assert!(!scratch.exists(), "the view's own folder was left");
}

/// #259: a view that goes takes its folder and the empty ones above it,
/// in a tree of its own so no other test is in the way.
#[test]
fn a_view_takes_the_empty_folders_above_its_copies_with_it() {
    let root = crate::util::test_dir("view-scratch");
    let scratch = root.join("archive").join("preview").join("0-a.zip");
    std::fs::create_dir_all(&scratch).unwrap();
    std::fs::write(scratch.join("top.txt"), "x").unwrap();
    let (mut a, _zip) = app_on_archive();
    a.act(Act::Enter);
    wait(&mut a);
    a.archive_view.as_mut().expect("in the archive").scratch = scratch.clone();
    a.act(Act::Escape(EscapeWhat::default()));
    assert!(!scratch.exists() && !root.join("archive").exists(), "left: {:?}", std::fs::read_dir(&root).map(|d| d.count()));
}

/// #198: `<Enter>` with nothing left after the filter keeps the picker
/// open and says why, instead of closing as if something were chosen.
#[test]
fn a_picker_filtered_to_nothing_stays_open() {
    let (mut a, zip) = app_on_archive();
    let mut pick = PickOverlay {
        title: "Jump to".into(),
        items: vec!["one".into()],
        details: vec![String::new()],
        query: "zzz".into(),
        matches: Vec::new(),
        cursor: 0,
        action: PickAction::Jump { paths: vec![zip] },
        focused: false,
    };
    pick.refilter();
    a.overlay = Overlay::Pick(pick);
    a.submit_pick();
    assert!(matches!(a.overlay, Overlay::Pick(_)), "still open");
    assert!(
        a.toasts.iter().any(|t| t.text == "Nothing matches `zzz` — <Esc> closes"),
        "{:?}",
        a.toasts.iter().map(|t| &t.text).collect::<Vec<_>>()
    );
}

/// A jump elsewhere leaves the view rather than carry it along.
#[test]
fn a_jump_leaves_the_archive() {
    let (mut a, zip) = app_on_archive();
    a.act(Act::Enter);
    wait(&mut a);
    let elsewhere = zip.parent().unwrap().join("src");
    a.cd(elsewhere, true);
    assert!(!a.in_archive_view());
}

/// #251: members selected inside the view are not selected after it,
/// whichever way it is left; what was selected outside stays.
#[test]
fn leaving_the_archive_drops_its_selection() {
    let (mut a, zip) = app_on_archive();
    let dir = zip.parent().unwrap().to_path_buf();
    let outside = dir.join("other.txt");
    a.tabs[a.active].selected.insert(zip.clone());
    a.tabs[a.active].selected.insert(outside.clone());
    for leave in [0, 1] {
        a.act(Act::Enter);
        wait(&mut a);
        a.tabs[a.active].selected.insert(zip.join("docs"));
        a.tabs[a.active].selected.insert(zip.join("top.txt"));
        match leave {
            0 => a.act(Act::Escape(EscapeWhat::default())),
            _ => a.cd(dir.join("src"), true),
        }
        assert!(!a.in_archive_view());
        let left: Vec<&PathBuf> = a.tabs[a.active].selected.iter().collect();
        assert_eq!(left, [&outside, &zip], "leave {leave}");
        a.cd(dir.clone(), true);
        let entries = vec![Entry::from_path(zip.clone()).unwrap()];
        a.tabs[a.active].current = Folder::from_entries(dir.clone(), Arc::new(entries), true);
    }
}

/// #251: the copy `l` opens keeps the member's folders, so two members
/// of one name in different folders do not share a copy. The message is
/// read here rather than drained, which would start the default app.
#[test]
fn an_opened_copy_keeps_its_folders() {
    let (mut a, _) = app_on_archive();
    a.act(Act::Enter);
    wait(&mut a);
    a.act(Act::Enter);
    a.act(Act::Enter);
    let entry = a.tabs[a.active].current.hovered().cloned().expect("readme.md");
    a.open_member(&entry);
    let got = a.archive_rx.take().unwrap().recv_timeout(Duration::from_secs(10)).unwrap();
    let ArchiveMsg::Copied(Ok(path)) = got else { panic!("no copy") };
    assert!(path.ends_with(Path::new("pack.zip").join("docs").join("a").join("readme.md")), "{}", path.display());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "read me");
}

/// #251's other half: `d` on a path that is not there names it and is
/// not said to have trashed it. Nothing here reaches the real trash.
#[test]
fn trashing_what_is_not_there_says_so() {
    let (mut a, zip) = app_on_archive();
    a.tabs[a.active].selected.insert(zip.join("top.txt"));
    a.act(Act::Remove { permanently: false, force: false, hovered: false });
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut done = false;
    while !done && Instant::now() < deadline {
        while let Ok(ev) = a.ops.rx.try_recv() {
            done |= matches!(ev, ops::OpEvent::Finished { .. });
            a.on_op_event(ev);
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    let said: Vec<&String> = a.toasts.iter().map(|t| &t.text).collect();
    assert!(said.iter().any(|t| t.ends_with("top.txt: not there any more")), "{said:?}");
    assert!(!said.iter().any(|t| t.starts_with("Trashed")), "{said:?}");
    assert!(zip.exists());
}
