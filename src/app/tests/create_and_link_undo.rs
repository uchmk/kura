use super::*;

fn app(dir: &Path) -> App {
    App::new(Config::load(), dir.to_path_buf(), egui::Context::default())
}

/// `a` with folders on the way: `u` removes the file and the folders it
/// had to make, and `U` makes them all again.
#[test]
fn a_create_goes_and_comes_back() {
    let dir = util::test_dir("create-undo");
    let mut a = app(&dir);
    a.do_create("new/deep/note.txt");
    let file = dir.join("new/deep/note.txt");
    assert!(file.is_file());

    a.undo_step();
    assert!(!dir.join("new").exists(), "the file and both folders made for it are gone");

    a.redo_step();
    assert!(file.is_file(), "U makes the file again, folders and all");
}

fn toasts(a: &App) -> Vec<&str> {
    a.toasts.iter().map(|t| t.text.as_str()).collect()
}

/// #168: the toast counts the folders `u` took with the file, and only
/// those -- one someone has since put something in stays, and is not
/// counted.
#[test]
fn undoing_a_create_counts_the_folders_that_went() {
    let dir = util::test_dir("create-undo-said");
    let mut a = app(&dir);
    a.do_create("new/deep/note.txt");
    a.undo_step();
    assert!(toasts(&a).contains(&"Removed note.txt and 2 folder(s)"), "{:?}", toasts(&a));

    a.redo_step();
    std::fs::write(dir.join("new/other.txt"), b"").unwrap();
    a.undo_step();
    assert!(dir.join("new").is_dir() && !dir.join("new/deep").exists());
    assert!(toasts(&a).contains(&"Removed note.txt and 1 folder(s)"), "{:?}", toasts(&a));

    a.do_create("plain.txt");
    a.undo_step();
    assert!(toasts(&a).contains(&"Removed plain.txt"), "{:?}", toasts(&a));
}

/// #168, proposal 4: `--keys` waits on `settled`, and a job only says how
/// it went once it is over, so a job queued or running is not settled.
/// Without this `u<Shot:x>` pictured the job at 0% and missed its toast.
#[test]
fn a_job_still_going_is_not_settled() {
    let dir = util::test_dir("settled-job");
    std::fs::write(dir.join("a.txt"), b"a").unwrap();
    std::fs::create_dir(dir.join("sub")).unwrap();
    let mut s = crate::ui::harness::Screen::open(&dir);
    let settle = |s: &mut crate::ui::harness::Screen| {
        for _ in 0..1000 {
            s.turn();
            if s.app.settled() {
                return true;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        false
    };
    assert!(settle(&mut s), "the listing and preview come up");

    s.app.submit_op(OpKind::Copy, vec![dir.join("a.txt")], dir.join("sub"), false);
    assert!(!s.app.settled(), "queued");
    assert!(settle(&mut s));
    assert!(dir.join("sub/a.txt").is_file(), "settled only once the copy was there");
    assert!(s.app.tasks.iter().all(|t| t.state == TaskState::Done));

    // Copied again, the name is taken and the job asks: that question is
    // waiting on a key, so it is settled.
    s.app.submit_op(OpKind::Copy, vec![dir.join("a.txt")], dir.join("sub"), false);
    for _ in 0..1000 {
        s.turn();
        if matches!(s.app.overlay, Overlay::Confirm(_)) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(matches!(s.app.overlay, Overlay::Confirm(_)), "the conflict is asked");
    assert!(s.app.settled(), "the job waits on the answer");
}

fn finished(linked: Vec<ops::Link>, errors: Vec<String>) -> ops::OpEvent {
    ops::OpEvent::Finished {
        id: 3,
        kind: OpKind::Hardlink,
        errors,
        cancelled: false,
        moved: Vec::new(),
        linked,
        junctions: Vec::new(),
        made: Vec::new(),
        trashed: Vec::new(),
    }
}

/// #168: `=`, `-` and `_` say what they made, as `d` does, since a new
/// link barely changes the listing.
#[test]
fn a_finished_link_says_what_it_made() {
    let dir = util::test_dir("link-said");
    let mut a = app(&dir);
    let link = |name: &str| ops::Link { at: dir.join(name), target: dir.join("src.txt"), dir: false, hard: true, junction: false };
    a.on_op_event(finished(vec![link("one.txt")], Vec::new()));
    assert!(toasts(&a).contains(&"Linked one.txt — u to undo"), "{:?}", toasts(&a));

    a.on_op_event(finished(vec![link("two.txt"), link("three.txt")], Vec::new()));
    assert!(toasts(&a).contains(&"Made 2 link(s) — u to undo"), "{:?}", toasts(&a));
}

/// #185: `u` and `U` on junctions say junction, as `y` did; on links,
/// link, as before.
#[test]
fn undoing_a_junction_says_junction() {
    let made = |junction| ops::Link { at: PathBuf::from("w/alias"), target: PathBuf::from("w/real"), dir: true, hard: false, junction };
    let step = UndoStep::Link { links: vec![made(true)] };
    assert_eq!(step.undone_label(), "Removed the junction alias");
    assert_eq!(step.redone_label(), "Made the junction alias again");
    let step = UndoStep::Link { links: vec![made(true), made(true)] };
    assert_eq!((step.undone_label().as_str(), step.redone_label().as_str()), ("Removed 2 junction(s)", "Made 2 junctions again"));
    let step = UndoStep::Link { links: vec![made(false)] };
    assert_eq!((step.undone_label().as_str(), step.redone_label().as_str()), ("Removed the link alias", "Linked alias"));
}

/// Some made and some not: the error is what to read, so no success line
/// goes up next to it. What was made is still a step `u` can take back.
#[test]
fn a_link_that_partly_failed_says_only_the_error() {
    let dir = util::test_dir("link-said-partial");
    let mut a = app(&dir);
    let link = ops::Link { at: dir.join("one.txt"), target: dir.join("src.txt"), dir: false, hard: true, junction: false };
    a.on_op_event(finished(vec![link], vec!["two.txt: denied".into()]));
    assert!(!toasts(&a).iter().any(|t| t.contains("u to undo")), "{:?}", toasts(&a));
    assert_eq!(a.undos.undo.len(), 1);
}

/// Q46: folder symlinks Windows refused come back as junctions to offer.
/// The question names both paths and what a junction is not; `n` leaves
/// everything as it was.
#[test]
fn a_refused_folder_link_is_offered_as_a_junction() {
    let dir = util::test_dir("junction-offer");
    let mut a = app(&dir);
    let offered = ops::Link { at: dir.join("alias"), target: dir.join("real"), dir: true, hard: false, junction: true };
    let mut ev = finished(Vec::new(), vec!["real: refused".into()]);
    if let ops::OpEvent::Finished { junctions, .. } = &mut ev {
        junctions.push(offered.clone());
    }
    a.on_op_event(ev);
    let Overlay::Confirm(c) = &a.overlay else { panic!("no question: {:?}", toasts(&a)) };
    assert!(matches!(&c.action, ConfirmAction::Junctions { links } if links == &vec![offered.clone()]));
    let body = c.body.join("\n");
    assert!(body.contains("not a relative one") && body.contains("network"), "{body}");
    assert!(body.contains(&format!("{}  →  {}", offered.at.display(), offered.target.display())), "{body}");
    assert!(
        body.contains("`c` copies:") && body.contains(&ops::mklink_line(&offered.at, &offered.target)),
        "the command is on the screen: {body}"
    );
    assert_eq!(c.options[0], ('y', "Make the junction".into()));
    assert_eq!(c.options[1], ('c', "Copy the mklink command".into()));

    a.answer_confirm('n');
    assert!(a.overlay.is_none() && a.undos.undo.is_empty() && !offered.at.exists());
}

/// Q64, found by the QA agent's tests for section 26: an error naming a
/// path under the home folder no longer carries the user's name into the
/// report's link or its panel.
#[test]
fn the_report_names_no_home_folder_from_the_last_error() {
    let Some(home) = ito_common::home_dir().filter(|h| h.as_os_str().len() >= 4) else { return };
    let dir = util::test_dir("report-home");
    let mut a = app(&dir);
    a.error(format!("{}: denied", home.join("secret").display()));
    a.bug_report();
    let Overlay::Confirm(c) = &a.overlay else { panic!("no panel") };
    let ConfirmAction::BugReport { url } = &c.action else { panic!("not the report") };
    let url = url.clone();
    let shown = home.display().to_string();
    assert!(!c.body.join("\n").contains(&shown), "{:?}", c.body);
    assert!(c.body.iter().any(|l| l.starts_with("Last error: ~")), "{:?}", c.body);
    let encoded = crate::bugreport::url(&[("x", shown.clone())]);
    let needle = encoded.rsplit("x=").next().unwrap_or_default();
    assert!(!url.contains(needle), "the link carries the home folder: {url}");
    // And `c` copies it to this thread's fake clipboard, never the machine's.
    a.answer_confirm('c');
    assert_eq!(crate::exec::get_clipboard(), Ok(url));
}

/// Q71: a key the box does not offer leaves it open, so a stray key no
/// longer closes it in silence and sends the next `<Enter>` to the list.
/// `n` still answers a box that has no `n` button (the overwrite one).
#[test]
fn a_key_the_box_does_not_offer_leaves_it_open() {
    let dir = util::test_dir("confirm-stray");
    let mut a = app(&dir);
    a.bug_report();
    for stray in ['(', 'x', 'j', 'Y'] {
        a.answer_confirm(stray);
        assert!(matches!(a.overlay, Overlay::Confirm(_)), "{stray:?} closed the box");
    }
    assert!(a.last_report.is_none(), "and did nothing else");
    a.answer_confirm('n');
    assert!(a.overlay.is_none(), "`n` closes it");
}

/// Q56: `c` copies the `mklink /J` line -- the one the refusal names --
/// and makes nothing. Where there is no clipboard (a headless test run)
/// it says so instead.
#[test]
fn c_copies_the_mklink_line_and_makes_nothing() {
    let dir = util::test_dir("junction-copy");
    let mut a = app(&dir);
    let offered = ops::Link { at: dir.join("alias"), target: dir.join("real"), dir: true, hard: false, junction: true };
    let mut ev = finished(Vec::new(), vec!["real: refused".into()]);
    if let ops::OpEvent::Finished { junctions, .. } = &mut ev {
        junctions.push(offered.clone());
    }
    a.on_op_event(ev);
    a.answer_confirm('c');
    assert!(a.overlay.is_none() && a.undos.undo.is_empty() && !offered.at.exists());
    let said = toasts(&a);
    assert!(
        said.contains(&"Copied the mklink command — it runs in cmd or PowerShell") || said.iter().any(|t| t.starts_with("Clipboard:")),
        "{said:?}"
    );
    assert_eq!(
        ops::mklink_line(&offered.at, &offered.target),
        format!("cmd /d /c mklink /J \"{}\" \"{}\"", offered.at.display(), offered.target.display())
    );
}

/// The `y`: a real junction on Windows, which `u` removes without touching
/// the folder and `U` makes again. Only Windows has junctions.
#[cfg(windows)]
#[test]
fn a_junction_is_made_undone_and_redone() {
    let dir = util::test_dir("junction-made");
    std::fs::create_dir(dir.join("real")).unwrap();
    std::fs::write(dir.join("real/inside.txt"), b"x").unwrap();
    let mut a = app(&dir);
    let link = ops::Link { at: dir.join("alias"), target: dir.join("real"), dir: true, hard: false, junction: true };
    a.make_junctions(vec![link.clone()]);
    assert!(toasts(&a).contains(&"Made a junction alias — u to undo"), "{:?}", toasts(&a));
    assert!(dir.join("alias/inside.txt").is_file(), "through the junction");

    a.undo_step();
    assert!(!ops::exists(&link.at), "the junction is gone");
    assert!(dir.join("real/inside.txt").is_file(), "the folder and its file are not");

    a.redo_step();
    assert!(dir.join("alias/inside.txt").is_file(), "U makes the junction again");
}

/// A file written in since is not a slip any more: `u` keeps it, says
/// why, and leaves the step to try again.
#[test]
fn a_created_file_with_something_in_it_stays() {
    let dir = util::test_dir("create-undo-written");
    let mut a = app(&dir);
    a.do_create("kept.txt");
    std::fs::write(dir.join("kept.txt"), b"work").unwrap();

    a.undo_step();
    assert!(dir.join("kept.txt").is_file(), "not removed");
    assert_eq!(a.undos.undo.len(), 1, "still there for u once it is empty again");
}

/// `a` on a folder that is already there makes nothing, so `u` has
/// nothing of its to remove.
#[test]
fn a_folder_that_was_there_is_not_recorded() {
    let dir = util::test_dir("create-undo-existing");
    std::fs::create_dir(dir.join("was")).unwrap();
    let mut a = app(&dir);
    a.do_create("was/");
    assert!(a.undos.undo.is_empty());
}

/// Undoing a hardlink removes the second name and leaves the file.
#[test]
fn a_hardlink_is_removed_and_the_file_stays() {
    let dir = util::test_dir("link-undo-hard");
    let src = dir.join("data.txt");
    std::fs::write(&src, b"data").unwrap();
    let link = ops::Link { at: dir.join("again.txt"), target: src.clone(), dir: false, hard: true, junction: false };
    link.make().unwrap();

    let mut a = app(&dir);
    a.undos.land(UndoStep::Link { links: vec![link.clone()] }, Land::Fresh);
    a.undo_step();
    assert!(!link.at.exists(), "the link is gone");
    assert_eq!(std::fs::read(&src).unwrap(), b"data", "the file it named is not");

    a.redo_step();
    assert_eq!(std::fs::read(&link.at).unwrap(), b"data", "U links it again");
}

/// Something that took the link's name since is not the link: `u` leaves
/// it alone.
#[test]
fn a_file_that_took_the_name_is_not_removed() {
    let dir = util::test_dir("link-undo-replaced");
    let src = dir.join("data.txt");
    std::fs::write(&src, b"data").unwrap();
    let link = ops::Link { at: dir.join("again.txt"), target: src.clone(), dir: false, hard: true, junction: false };
    std::fs::write(&link.at, b"another file of a different size").unwrap();

    let mut a = app(&dir);
    a.undos.land(UndoStep::Link { links: vec![link.clone()] }, Land::Fresh);
    a.undo_step();
    assert!(link.at.exists(), "not ours to remove");
    assert_eq!(a.undos.undo.len(), 1, "the step stays");
}

/// A symlink to a folder: `u` removes the link, not the folder or what
/// is in it.
#[cfg(unix)]
#[test]
fn a_folder_symlink_is_removed_and_the_folder_stays() {
    let dir = util::test_dir("link-undo-sym");
    std::fs::create_dir(dir.join("real")).unwrap();
    std::fs::write(dir.join("real/inside.txt"), b"x").unwrap();
    let link = ops::Link { at: dir.join("alias"), target: PathBuf::from("real"), dir: true, hard: false, junction: false };
    link.make().unwrap();

    let mut a = app(&dir);
    a.undos.land(UndoStep::Link { links: vec![link.clone()] }, Land::Fresh);
    a.undo_step();
    assert!(!ops::exists(&link.at), "the link is gone");
    assert!(dir.join("real/inside.txt").is_file(), "the folder and its file are not");

    a.redo_step();
    assert!(dir.join("alias/inside.txt").is_file(), "U makes the same relative link again");
}
