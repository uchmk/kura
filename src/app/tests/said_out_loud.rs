//! Keys that used to finish without a word, which on screen is the same as a
//! key bound to nothing (#108).

use super::*;

#[test]
fn a_bare_unc_host_is_above_its_share() {
    assert!(is_within(Path::new(r"\\h\share"), Path::new(r"\\h")));
    assert!(is_within(Path::new(r"\\H\share\a"), Path::new(r"\\h\")));
    assert!(!is_within(Path::new(r"\\hh\share"), Path::new(r"\\h")));
}

fn app_in(dir: &Path) -> App {
    let mut a = App::new(Config::load(), dir.to_path_buf(), egui::Context::default());
    a.tabs[a.active].cwd = dir.to_path_buf();
    a.tabs[a.active].current = Folder::loading(dir.to_path_buf(), None);
    a
}

/// TESTING.md 10.10 — `c c` in an empty folder. The clipboard still holds
/// the last path, so staying quiet lets the old one be pasted as this one.
#[test]
fn copying_from_an_empty_list_says_there_is_nothing() {
    let dir = crate::util::test_dir("said-copy");
    let mut a = app_in(&dir);
    a.act(Act::Copy(CopyWhat::Path));
    assert!(a.toasts.iter().any(|t| t.text.starts_with("Nothing to copy")), "{:?}", a.toasts.iter().map(|t| &t.text).collect::<Vec<_>>());
}

/// `d` names what went to the trash and how to get it back; `D` asks first
/// and so needs no such line.
#[test]
fn a_finished_trash_says_what_went() {
    let dir = crate::util::test_dir("said-trash");
    let mut a = app_in(&dir);
    let gone = dir.join("a.txt");
    a.record_job(7, UndoStep::Trash { paths: vec![gone], dir: dir.clone() }, Land::Fresh);
    a.on_op_event(ops::OpEvent::Finished {
        id: 7,
        kind: OpKind::Trash,
        errors: Vec::new(),
        cancelled: false,
        moved: Vec::new(),
        linked: Vec::new(),
        junctions: Vec::new(),
        made: Vec::new(),
        trashed: vec![dir.join("a.txt")],
    });
    assert!(a.toasts.iter().any(|t| t.text == "Trashed a.txt — u to undo"), "{:?}", a.toasts.iter().map(|t| &t.text).collect::<Vec<_>>());
}

/// 21.22c / #265: packing `to-pack.zip` offers another name, and typing
/// its own name back is refused before any job is queued.
#[test]
fn an_archive_is_not_packed_into_itself() {
    let dir = crate::util::test_dir("pack-self");
    std::fs::write(dir.join("to-pack.zip"), "x").unwrap();
    let mut a = app_in(&dir);
    a.apply_listing(&dir, Arc::new(vec![Entry::from_path(dir.join("to-pack.zip")).unwrap()]));
    a.ask_compress();
    let Overlay::Input(ov) = &a.overlay else { panic!("no prompt") };
    assert_eq!(ov.text, "to-pack-packed.zip");
    a.do_compress("to-pack.zip");
    assert!(a.tasks.is_empty(), "no job for packing a file over itself");
    assert!(
        a.toasts.iter().any(|t| t.text.starts_with("Can't pack to-pack.zip into itself")),
        "{:?}",
        a.toasts.iter().map(|t| &t.text).collect::<Vec<_>>()
    );
}

/// #222: a second `<Tab>` goes on to the next folder that carried on from
/// the first prefix, rather than into the one just chosen.
#[test]
fn tab_goes_round_the_candidates() {
    let sep = std::path::MAIN_SEPARATOR;
    let hits = vec!["alpha".to_owned(), "alps".to_owned(), "also".to_owned()];
    let dir = PathBuf::from("base");
    let first = completed_text(&dir, &hits[0]);
    assert_eq!(cycling_in(&first, &hits, 1), Some(dir.clone()));
    assert_eq!(cycling_in(&first, &hits, 0), None, "nothing completed yet");
    assert_eq!(cycling_in(&format!("{first}x"), &hits, 1), None, "typed on since");
    assert_eq!(cycling_in(&format!("base{sep}al"), &hits, 1), None, "not what the last press wrote");
    assert_eq!(cycling_in(&first, &hits[..1], 1), None, "one candidate has nothing to cycle to");
    let wrapped = completed_text(&dir, &hits[2]);
    assert_eq!(cycling_in(&wrapped, &hits, 3), Some(dir), "the last one wraps to the first");
}

/// #228: `KURA_SCALE` names a scale the keys could also reach, and holds
/// it for the run: common.toml's is not followed.
#[test]
fn the_scale_variable_is_a_number_the_keys_reach() {
    let dir = crate::util::test_dir("pinned-scale");
    let mut a = app_in(&dir);
    a.start_scaled(Some("1.26"));
    assert_eq!(a.scale, 1.3);
    a.set_common(Ok((ito_common::Common { scale: Some(2.0), ..Default::default() }, Vec::new())));
    assert_eq!(a.scale, 1.3, "pinned");
    let mut b = app_in(&dir);
    b.start_scaled(Some("120%"));
    assert_eq!(b.scale, 1.2);
    for bad in ["0.1", "5.1", "x", "NaN", "-1"] {
        b.start_scaled(Some(bad));
        assert_eq!(b.scale, 1.2, "{bad} is ignored");
    }
    assert!(b.toasts.iter().any(|t| t.text.contains("KURA_SCALE=5.1")));
}

/// Since v0.99.0 another uchmk app's scale is followed, except just after
/// a key here moved it.
#[test]
fn the_common_scale_is_followed() {
    use crate::config::cmd::ScaleTo;
    let dir = crate::util::test_dir("common-scale");
    let mut a = app_in(&dir);
    let common = |s| Ok((ito_common::Common { scale: Some(s), ..Default::default() }, Vec::new()));
    a.set_common(common(1.5));
    assert_eq!(a.scale, 1.5);
    a.act(Act::Scale(ScaleTo::In));
    a.set_common(common(1.5));
    assert_eq!(a.scale, 1.6, "a step the writer has not reached yet is not undone");
}

/// Q98: common.toml's theme is kura's colours, followed as the file
/// changes; a name no theme has is said once and keeps the colours.
#[test]
fn the_common_theme_colours_kura() {
    let dir = crate::util::test_dir("common-theme");
    let mut a = app_in(&dir);
    let theme = |name: &str| Ok((ito_common::Common { theme: Some(name.into()), ..Default::default() }, Vec::new()));
    assert!(!a.cfg.theme.light());
    a.set_common(theme("tsumugi Light"));
    assert!(a.cfg.theme.light());
    a.set_common(theme("No Such Theme"));
    a.set_common(theme("No Such Theme"));
    assert!(a.cfg.theme.light(), "kept");
    assert_eq!(a.toasts.iter().filter(|t| t.text.contains("No Such Theme")).count(), 1, "said once");
    a.set_common(theme("system"));
    assert!(!a.cfg.theme.light(), "the OS is dark here");
    a.set_os_light(true);
    assert!(a.cfg.theme.light(), "and turned light");
}

/// #174: the packing job names the format the typed name made.
#[test]
fn a_packing_job_says_which_format_it_makes() {
    let dir = util::test_dir("pack-label");
    std::fs::write(dir.join("a.txt"), "x").unwrap();
    let mut a = app_in(&dir);
    a.apply_listing(&dir, Arc::new(vec![Entry::from_path(dir.join("a.txt")).unwrap()]));
    a.do_compress("tar.gz.zip");
    let label = a.tasks.last().map(|t| t.label.clone()).unwrap_or_default();
    assert_eq!(label, "Compress 1 item(s) into tar.gz.zip (as zip)");
}

/// Q79: `<Tab>` in `E`'s field walks the three formats and leaves the stem.
#[test]
fn tab_turns_an_archive_name_into_the_next_format() {
    assert_eq!(next_archive_name("to-pack.zip"), "to-pack.tar.gz");
    assert_eq!(next_archive_name("to-pack.tar.gz"), "to-pack.7z");
    assert_eq!(next_archive_name("to-pack.7z"), "to-pack.zip");
    assert_eq!(next_archive_name("A.b.ZIP"), "A.b.tar.gz");
    assert_eq!(next_archive_name("x.tar"), "x.zip");
    assert_eq!(next_archive_name("x"), "x.zip");
}

/// #241: a copy stopped from the tasks list says so; one that ran out
/// says nothing extra.
#[test]
fn a_stopped_copy_says_it_stopped() {
    let dir = crate::util::test_dir("said-stopped");
    let mut a = app_in(&dir);
    let finish = |a: &mut App, id, cancelled| {
        a.on_op_event(ops::OpEvent::Finished {
            id,
            kind: OpKind::Copy,
            errors: Vec::new(),
            cancelled,
            moved: Vec::new(),
            linked: Vec::new(),
            junctions: Vec::new(),
            made: Vec::new(),
            trashed: Vec::new(),
        })
    };
    finish(&mut a, 1, false);
    assert!(a.toasts.is_empty(), "{:?}", a.toasts.iter().map(|t| &t.text).collect::<Vec<_>>());
    finish(&mut a, 2, true);
    assert!(a.toasts.iter().any(|t| t.text.starts_with("Copy stopped")), "{:?}", a.toasts.iter().map(|t| &t.text).collect::<Vec<_>>());
}

/// #238: a cut pasted into the folder it came from moves nothing and
/// keeps the register, rather than renaming the file `same_1.txt`.
#[test]
fn a_cut_pasted_where_it_is_stays_put() {
    let dir = crate::util::test_dir("cut-here");
    let mut a = app_in(&dir);
    let src = dir.join("same.txt");
    a.yank = Yank { paths: vec![src.clone()], cut: true, from_archive: false };
    let before = a.tasks.len();
    a.paste(false, false);
    assert_eq!(a.tasks.len(), before, "nothing was queued");
    assert_eq!(a.yank.paths, vec![src]);
    assert!(
        a.toasts.iter().any(|t| t.text == "Already here — the cut is still there"),
        "{:?}",
        a.toasts.iter().map(|t| &t.text).collect::<Vec<_>>()
    );
}

/// Q72: a cut pasted where every clash was declined moved nothing, so
/// the register keeps it and says so; a paste that moved something
/// leaves it empty, as before.
#[test]
fn a_paste_that_moved_nothing_keeps_the_cut() {
    let dir = crate::util::test_dir("cut-kept");
    let mut a = app_in(&dir);
    // Cut from another folder: one cut where it already is never runs.
    let src = dir.join("from").join("a.txt");
    let finished = |id, moved| ops::OpEvent::Finished {
        id,
        kind: OpKind::Move,
        errors: Vec::new(),
        cancelled: false,
        moved,
        linked: Vec::new(),
        junctions: Vec::new(),
        made: Vec::new(),
        trashed: Vec::new(),
    };
    a.yank = Yank { paths: vec![src.clone()], cut: true, from_archive: false };
    a.paste(false, false);
    assert!(a.yank.paths.is_empty(), "the register empties while the job runs");
    let id = a.tasks.last().unwrap().id;
    a.on_op_event(finished(id, Vec::new()));
    assert_eq!(a.yank.paths, vec![src.clone()]);
    assert!(a.yank.cut);
    assert!(
        a.toasts.iter().any(|t| t.text == "Nothing moved — the cut is still there"),
        "{:?}",
        a.toasts.iter().map(|t| &t.text).collect::<Vec<_>>()
    );

    a.paste(false, false);
    let id = a.tasks.last().unwrap().id;
    a.on_op_event(finished(id, vec![(src.clone(), dir.join("b/a.txt"))]));
    assert!(a.yank.paths.is_empty(), "a paste that moved something used the cut up");

    // Yanked something else meanwhile: that is what the register holds.
    a.yank = Yank { paths: vec![src.clone()], cut: true, from_archive: false };
    a.paste(false, false);
    let id = a.tasks.last().unwrap().id;
    let other = dir.join("other.txt");
    a.yank = Yank { paths: vec![other.clone()], cut: false, from_archive: false };
    a.on_op_event(finished(id, Vec::new()));
    assert_eq!(a.yank.paths, vec![other]);
}

/// #225: a rename says what it did and that `u` takes it back, as `d`
/// and `-` do. It used to say nothing at all.
#[test]
fn a_rename_says_what_it_did() {
    let dir = crate::util::test_dir("said-rename");
    std::fs::write(dir.join("a.txt"), "x").unwrap();
    let mut a = app_in(&dir);
    a.do_rename(&dir.join("a.txt"), "b.txt");
    assert!(dir.join("b.txt").exists() && !dir.join("a.txt").exists());
    assert!(
        a.toasts.iter().any(|t| t.text == "Renamed to b.txt — u to undo"),
        "{:?}",
        a.toasts.iter().map(|t| &t.text).collect::<Vec<_>>()
    );
}

/// #207: `d` failed and said to use `D`; once `D` has gone through, the
/// red line about that file gives way to `Deleted <name>`. A `D` with no
/// such line to answer says nothing, as before.
#[test]
fn a_delete_replaces_the_trash_error_it_answers() {
    let dir = crate::util::test_dir("said-delete");
    let mut a = app_in(&dir);
    let finished = |id| ops::OpEvent::Finished {
        id,
        kind: OpKind::Delete,
        errors: Vec::new(),
        cancelled: false,
        moved: Vec::new(),
        linked: Vec::new(),
        junctions: Vec::new(),
        made: Vec::new(),
        trashed: Vec::new(),
    };
    a.error("Trash: a.txt: the Recycle Bin can't take files from R: (…). Use D to delete permanently");
    a.error("Trash: b.txt: it is open in another program");
    let id = a.submit_op_to(OpKind::Delete, vec![dir.join("a.txt")], dir.clone(), None, false);
    a.on_op_event(finished(id));
    let said: Vec<&str> = a.toasts.iter().map(|t| t.text.as_str()).collect();
    assert!(!said.iter().any(|t| t.starts_with("Trash: a.txt")), "the answered line is gone: {said:?}");
    assert!(said.contains(&"Trash: b.txt: it is open in another program"), "another file's stays: {said:?}");
    assert!(said.contains(&"Deleted a.txt"), "{said:?}");

    let quiet = a.toasts.len();
    let id = a.submit_op_to(OpKind::Delete, vec![dir.join("c.txt")], dir.clone(), None, false);
    a.on_op_event(finished(id));
    assert_eq!(a.toasts.len(), quiet, "nothing to answer, nothing said");
}

/// #205: a finished pack names its archive, an unpack the folder the
/// contents are in -- several of them by the first and a count.
#[test]
fn a_pack_and_an_unpack_say_what_they_made() {
    let sep = std::path::MAIN_SEPARATOR;
    let zip = OpKind::Compress(crate::fs::archive::Format::Zip);
    assert_eq!(made_says(zip, &[PathBuf::from("x/out.zip")]).as_deref(), Some("Packed into out.zip"));
    assert_eq!(made_says(OpKind::Extract, &[PathBuf::from("x/out")]), Some(format!("Unpacked into out{sep}")));
    assert_eq!(
        made_says(OpKind::Extract, &[PathBuf::from("x/a"), PathBuf::from("x/b"), PathBuf::from("x/c")]),
        Some(format!("Unpacked 3 archives into a{sep} and 2 more")),
    );
    assert_eq!(made_says(OpKind::Extract, &[]), None, "nothing unpacked, nothing said");
    assert_eq!(made_says(OpKind::Copy, &[PathBuf::from("x/a")]), None);
}

/// #208: a drop onto a list showing the same folder says why it did
/// nothing, in `<A-c>`'s words, and queues no job.
#[test]
fn a_drop_into_the_same_folder_says_so() {
    let dir = crate::util::test_dir("said-drop");
    std::fs::write(dir.join("a.txt"), "a").unwrap();
    let mut a = app_in(&dir);
    let mut other = crate::core::tab::Tab::new(dir.clone(), a.tabs[0].sort, false, crate::fs::entry::Linemode::None);
    other.cwd = dir.clone();
    a.tabs.push(other);
    a.drag = Some(Drag { from: 0, paths: vec![dir.join("a.txt")], label: "a.txt".into() });
    a.drop_drag(Some(1), false);
    assert!(a.tasks.is_empty(), "no job");
    assert!(
        a.toasts.iter().any(|t| t.text == "Both panes are in the same directory"),
        "{:?}",
        a.toasts.iter().map(|t| &t.text).collect::<Vec<_>>()
    );
}

/// #163: `<C-F5>` with the help panel open re-reads the config, as the
/// panel's own rows say it does, and the panel stays open to show it.
#[test]
fn the_reload_key_works_with_help_open() {
    let dir = crate::util::test_dir("help-reload");
    let mut a = app_in(&dir);
    a.overlay = Overlay::Help;
    a.feed_overlay_key(Key::parse("<C-F5>").unwrap());
    assert!(matches!(a.overlay, Overlay::Help), "still open");
    assert!(a.toasts.iter().any(|t| t.text.starts_with("Reloaded")), "{:?}", a.toasts.iter().map(|t| &t.text).collect::<Vec<_>>());
}

/// Q62 / Q64: `<F12>` shows what the report carries before anything is
/// opened -- the keys before it, the last error, the config by name --
/// with opening first, so `<Enter>` opens and `<Esc>` drops it.
#[test]
fn f12_shows_the_report_before_opening_it() {
    let dir = crate::util::test_dir("f12-panel");
    std::fs::write(dir.join("a.txt"), "a").unwrap();
    let mut a = app_in(&dir);
    a.error("Copy: a.txt: denied");
    for k in ["j", "k", "<F12>"] {
        a.feed_key(Key::parse(k).unwrap());
    }
    let Overlay::Confirm(c) = &a.overlay else { panic!("no panel: {:?}", a.toasts.iter().map(|t| &t.text).collect::<Vec<_>>()) };
    let ConfirmAction::BugReport { url } = &c.action else { panic!("not the report") };
    assert_eq!(c.options.first().map(|o| o.0), Some('o'), "<Enter> opens");
    assert!(c.body.iter().any(|l| l == "Last keys: j k"), "the keys before <F12>, not <F12>: {:?}", c.body);
    assert!(c.body.iter().any(|l| l == "Last error: Copy: a.txt: denied"), "{:?}", c.body);
    assert!(url.contains("&keys=Last%20keys%2C%20oldest%20first%3A%20j%20k"), "{url}");
    assert!(url.contains("&context=Last%20error%3A%20Copy%3A%20a.txt%3A%20denied"), "{url}");
    let url = url.clone();

    // `c` copies the link and leaves it where the state file can read it.
    a.answer_confirm('c');
    assert!(matches!(a.overlay, Overlay::None));
    assert_eq!(a.last_report.as_deref(), Some(url.as_str()));
}

/// #83: a trash where four of five went leaves a step for the four, so
/// `u` brings them back rather than saying there is nothing to undo.
#[test]
fn a_partly_failed_trash_can_still_be_undone() {
    let dir = crate::util::test_dir("said-partial-trash");
    let mut a = app_in(&dir);
    let (gone, stuck) = (dir.join("a.txt"), dir.join("locked.txt"));
    a.record_job(9, UndoStep::Trash { paths: vec![gone.clone(), stuck], dir: dir.clone() }, Land::Fresh);
    a.on_op_event(ops::OpEvent::Finished {
        id: 9,
        kind: OpKind::Trash,
        errors: vec!["locked.txt: in use".into()],
        cancelled: false,
        moved: Vec::new(),
        linked: Vec::new(),
        junctions: Vec::new(),
        made: Vec::new(),
        trashed: vec![gone.clone()],
    });
    match a.undos.undo.last() {
        Some(UndoStep::Trash { paths, .. }) => assert_eq!(paths, &vec![gone], "only what went"),
        other => panic!("no trash step to undo: {other:?}"),
    }
}

/// TESTING.md 15.8 — a held zoom key stops at the ends, and the toast says
/// it has stopped.
#[test]
fn the_scale_says_when_it_hits_an_end() {
    use crate::config::cmd::ScaleTo;
    let dir = crate::util::test_dir("said-scale");
    let mut a = app_in(&dir);
    a.act(Act::Scale(ScaleTo::In));
    assert!(a.toasts.iter().any(|t| t.text == "Scale 110%"));
    for _ in 0..60 {
        a.act(Act::Scale(ScaleTo::In));
    }
    assert!(a.toasts.iter().any(|t| t.text == "Scale 500% (maximum)"));
    for _ in 0..60 {
        a.act(Act::Scale(ScaleTo::Out));
    }
    assert!(a.toasts.iter().any(|t| t.text == "Scale 20% (minimum)"));
    a.act(Act::Scale(ScaleTo::Reset));
    assert!(a.toasts.iter().any(|t| t.text == "Scale 100%"));
    // One at a time: each step replaces the last rather than stacking.
    assert_eq!(a.toasts.iter().filter(|t| t.text.starts_with("Scale ")).count(), 1);
}

/// #107: one failed jump, one toast. The parent columns it asked for fail
/// for the same reason, before and after the jump itself is taken back,
/// and each used to add a toast naming only a fragment of the path.
#[test]
fn a_failed_jump_says_so_once() {
    let dir = crate::util::test_dir("said-cd");
    let mut a = app_in(&dir);
    let mid = dir.join("no").join("such");
    let bad = mid.join("place");
    a.cd(bad.clone(), true);
    let fail = |path: &Path| ScanResult::Failed { id: 0, path: path.to_path_buf(), error: "os error 123".into() };
    a.toasts.clear();
    a.on_scan(fail(&mid));
    a.on_scan(fail(&bad));
    a.on_scan(fail(&dir.join("no")));
    let said: Vec<&String> = a.toasts.iter().map(|t| &t.text).collect();
    assert_eq!(said.len(), 1, "{said:?}");
    assert!(said[0].contains("place"), "the whole path is named: {said:?}");
    assert_eq!(a.tabs[a.active].cwd, dir, "taken back");

    // A failure that has nothing to do with the jump is still heard.
    a.on_scan(fail(&std::env::temp_dir().join("elsewhere")));
    assert_eq!(a.toasts.len(), 2);
}

/// TESTING.md 23.6 — a typed path that names nothing falls back to its
/// parent as before, and now says so; one naming a file stays quiet.
#[test]
fn a_path_that_names_nothing_says_where_it_landed() {
    let dir = crate::util::test_dir("said-reveal");
    std::fs::write(dir.join("here.txt"), "x").unwrap();
    let listing = || Arc::new(vec![Entry::from_path(dir.join("here.txt")).unwrap()]);

    let mut a = app_in(&dir.join("elsewhere"));
    a.cd_or_reveal(dir.join("here.txt"));
    a.on_scan(ScanResult::Failed { id: 0, path: dir.join("here.txt"), error: "not a dir".into() });
    a.apply_listing(&dir, listing());
    assert!(a.toasts.is_empty(), "the file is under the cursor: {:?}", a.toasts.iter().map(|t| &t.text).collect::<Vec<_>>());
    assert_eq!(a.tabs[a.active].current.hovered_name(), Some("here.txt"));

    let mut a = app_in(&dir.join("elsewhere"));
    a.cd_or_reveal(dir.join("tpyo"));
    a.on_scan(ScanResult::Failed { id: 0, path: dir.join("tpyo"), error: "not found".into() });
    a.apply_listing(&dir, listing());
    let said: Vec<&String> = a.toasts.iter().map(|t| &t.text).collect();
    assert_eq!(said.len(), 1, "{said:?}");
    assert!(said[0].contains("tpyo — showing"), "{said:?}");
}

/// #245: a typed name that is not there puts the cursor on the nearest
/// one (`tpyo` -> `typo`), not on the first row.
#[test]
fn a_mistyped_name_lands_the_cursor_on_the_nearest() {
    let dir = crate::util::test_dir("said-reveal-near");
    for n in ["alpha.txt", "typo", "zeta.txt"] {
        std::fs::write(dir.join(n), "x").unwrap();
    }
    let listing = || Arc::new(["alpha.txt", "typo", "zeta.txt"].iter().map(|n| Entry::from_path(dir.join(n)).unwrap()).collect::<Vec<_>>());
    let mut a = app_in(&dir.join("elsewhere"));
    a.cd_or_reveal(dir.join("tpyo"));
    a.on_scan(ScanResult::Failed { id: 0, path: dir.join("tpyo"), error: "not found".into() });
    a.apply_listing(&dir, listing());
    assert_eq!(a.tabs[a.active].current.hovered_name(), Some("typo"));
    assert!(a.toasts.iter().any(|t| t.text.ends_with(" (nearest: typo)")), "{:?}", a.toasts.iter().map(|t| &t.text).collect::<Vec<_>>());

    let mut a = app_in(&dir.join("elsewhere"));
    a.cd_or_reveal(dir.join("qqqqqq"));
    a.on_scan(ScanResult::Failed { id: 0, path: dir.join("qqqqqq"), error: "not found".into() });
    a.apply_listing(&dir, listing());
    assert_eq!(a.tabs[a.active].current.hovered_name(), Some("alpha.txt"), "nothing near: the top");
    assert!(a.toasts.iter().all(|t| !t.text.contains("(nearest: ")), "no name to give");
}

/// The `cd` prompt starts with the folder and this platform's separator,
/// and a root gets no second one.
#[test]
fn the_cd_prompt_ends_in_this_platforms_separator() {
    let sep = std::path::MAIN_SEPARATOR;
    let dir = std::env::temp_dir().join("x");
    assert_eq!(dir_prefill(&dir), format!("{}{sep}", dir.display()));
    let root = PathBuf::from(if cfg!(windows) { "C:\\" } else { "/" });
    assert_eq!(dir_prefill(&root), root.display().to_string());
    assert!(!dir_prefill(&dir).ends_with(if cfg!(windows) { '/' } else { '\\' }));
}

/// #242: the same, when the parent's listing has already arrived -- the
/// scan that fills the parent column often answers before the jump's own
/// fails. The name was then never checked, and the tab landed on the
/// parent in silence about one run in two.
#[test]
fn a_path_that_names_nothing_says_so_whichever_listing_lands_first() {
    let dir = crate::util::test_dir("said-reveal-race");
    std::fs::write(dir.join("here.txt"), "x").unwrap();
    let listing = || vec![Entry::from_path(dir.join("here.txt")).unwrap()];

    let mut a = app_in(&dir.join("elsewhere"));
    a.cd_or_reveal(dir.join("tpyo"));
    a.on_scan(ScanResult::Listed { id: 0, path: dir.clone(), entries: listing() });
    a.on_scan(ScanResult::Failed { id: 0, path: dir.join("tpyo"), error: "not found".into() });
    let said: Vec<&String> = a.toasts.iter().map(|t| &t.text).collect();
    assert_eq!(said.len(), 1, "{said:?}");
    assert!(said[0].contains("tpyo — showing"), "{said:?}");
    assert_eq!(a.tabs[a.active].cwd, dir, "on the parent");

    // A file there is still revealed without a word, in this order too.
    let mut a = app_in(&dir.join("elsewhere"));
    a.cd_or_reveal(dir.join("here.txt"));
    a.on_scan(ScanResult::Listed { id: 0, path: dir.clone(), entries: listing() });
    a.on_scan(ScanResult::Failed { id: 0, path: dir.join("here.txt"), error: "not a dir".into() });
    assert!(a.toasts.is_empty(), "{:?}", a.toasts.iter().map(|t| &t.text).collect::<Vec<_>>());
    assert_eq!(a.tabs[a.active].current.hovered_name(), Some("here.txt"));
}

/// 12.19: a listing sent before `a` made the file, answering after it,
/// leaves the cursor's landing to the rescan `a` sent; once no scan of
/// the folder is left, a name that never showed up is let go.
#[test]
fn a_new_name_waits_for_the_listing_that_has_it() {
    let dir = crate::util::test_dir("land-on-late");
    std::fs::write(dir.join("b.txt"), "x").unwrap();
    let mut a = app_in(&dir);
    a.apply_listing(&dir, Arc::new(vec![Entry::from_path(dir.join("b.txt")).unwrap()]));
    let before = vec![Entry::from_path(dir.join("b.txt")).unwrap()];

    a.inflight.clear(); // App::new's own scans, never answered here
    a.inflight.insert(901, dir.clone());
    a.do_create("aa.txt");
    a.on_scan(ScanResult::Listed { id: 901, path: dir.clone(), entries: before.clone() });
    assert_eq!(a.tabs[a.active].current.hovered_name(), Some("b.txt"), "not there yet");
    assert!(a.land_on.is_some(), "so the landing still waits");

    let id = *a.inflight.iter().find(|(_, p)| **p == dir).map(|(id, _)| id).expect("a's own rescan");
    let after = vec![Entry::from_path(dir.join("aa.txt")).unwrap(), Entry::from_path(dir.join("b.txt")).unwrap()];
    a.on_scan(ScanResult::Listed { id, path: dir.clone(), entries: after });
    assert_eq!(a.tabs[a.active].current.hovered_name(), Some("aa.txt"), "onto the new file");
    assert!(a.land_on.is_none());

    a.land_on = Some(dir.join("gone.txt"));
    a.on_scan(ScanResult::Listed { id: 902, path: dir.clone(), entries: before });
    assert!(a.land_on.is_none(), "no scan left to wait for");
}

/// #105: `<Esc>` while a jump is still waiting takes the tab
/// back, and the late failure of the abandoned scan says nothing.
#[test]
fn escape_gives_up_on_a_jump_still_waiting() {
    let dir = crate::util::test_dir("said-abandon");
    let mut a = app_in(&dir);
    let far = dir.join("slow");
    a.cd(far.clone(), true);
    assert!(a.tabs[a.active].pending_cd.is_some(), "waiting");
    a.act(Act::Escape(EscapeWhat::default()));
    assert_eq!(a.tabs[a.active].cwd, dir, "back where it came from");
    assert!(a.tabs[a.active].back.is_empty(), "and the jump is not in the history");
    assert!(a.toasts.iter().any(|t| t.text.starts_with("Stopped waiting for")));
    a.toasts.clear();
    a.on_scan(ScanResult::Failed { id: 0, path: far, error: "timed out".into() });
    assert!(a.toasts.is_empty(), "{:?}", a.toasts.iter().map(|t| &t.text).collect::<Vec<_>>());

    // With nothing waiting, `<Esc>` is what it was.
    a.act(Act::Escape(EscapeWhat::default()));
    assert_eq!(a.tabs[a.active].cwd, dir);
}

/// The same through a typed path, which first falls back to the parent in
/// case it named a file: the parent then fails and is the one taken back.
#[test]
fn a_failed_typed_jump_says_so_once() {
    let dir = crate::util::test_dir("said-cd-typed");
    let mut a = app_in(&dir);
    let mid = dir.join("no").join("such");
    let bad = mid.join("place");
    a.cd_or_reveal(bad.clone());
    let fail = |path: &Path| ScanResult::Failed { id: 0, path: path.to_path_buf(), error: "os error 123".into() };
    a.toasts.clear();
    a.on_scan(fail(&mid));
    a.on_scan(fail(&bad));
    a.on_scan(fail(&mid));
    a.on_scan(fail(&dir.join("no")));
    let said: Vec<&String> = a.toasts.iter().map(|t| &t.text).collect();
    assert_eq!(said.len(), 1, "{said:?}");
    assert_eq!(a.tabs[a.active].cwd, dir, "taken back");
}
