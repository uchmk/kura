use super::*;
use crate::config::keys::Key;

/// Following a huge log picks up appended lines and sits on the last one.
#[test]
fn preview_follow_tracks_an_appended_log() {
    use std::io::Write;
    let dir = crate::util::test_dir("app-log-follow");
    let f = dir.join("grow.log");
    std::fs::write(&f, (0..100).map(|i| format!("line {i}\n")).collect::<String>()).unwrap();
    let mut a = App::new(Config::load(), dir.clone(), egui::Context::default());
    let index = crate::preview::logindex::LogIndex::start(&f).unwrap();
    while !index.is_done() {
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    let view = crate::preview::logindex::LogView::new(index.clone());
    a.preview.state = PreviewState::Ready(Payload::Log { view });
    a.preview_follow();
    assert!(a.log_follow);
    std::fs::OpenOptions::new().append(true).open(&f).unwrap().write_all(b"more 1\nmore 2\n").unwrap();
    assert!(a.tick_log().is_some());
    while !index.is_done() {
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    a.log_polled = None;
    a.tick_log();
    assert_eq!(index.lines(), 102);
    assert_eq!(a.tabs[a.active].preview_offset, 101, "follows to the last line");
    a.preview_follow();
    assert!(!a.log_follow);
}

/// A huge log's preview search finds across lines and `n` / `N` step
/// through the hits.
#[test]
fn preview_find_steps_through_a_huge_log() {
    let dir = crate::util::test_dir("app-log-find");
    let f = dir.join("big.log");
    let text: String = (0..3000).map(|i| if i % 1000 == 500 { format!("hit {i}\nnext\n") } else { format!("line {i}\n") }).collect();
    std::fs::write(&f, text).unwrap();
    let mut a = App::new(Config::load(), dir.clone(), egui::Context::default());
    let index = crate::preview::logindex::LogIndex::start(&f).unwrap();
    while !index.is_done() {
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    let view = crate::preview::logindex::LogView::new(index);
    a.preview.state = PreviewState::Ready(Payload::Log { view: view.clone() });
    a.preview_find_submit("hit \\d+\\nnext");
    a.log_needle = "hit \\d+\\nnext".into();
    while !view.found().done {
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert_eq!(view.found().lines.len(), 3);
    assert!(a.log_matcher.is_some(), "the preview marks what was searched for");
    a.find_arrow(false);
    let first = a.tabs[a.active].preview_offset;
    a.find_arrow(false);
    assert!(a.tabs[a.active].preview_offset > first, "n goes on to the next hit");
    a.find_arrow(true);
    assert_eq!(a.tabs[a.active].preview_offset, first, "N goes back");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Several lines into a program without bracketed paste are asked about,
/// as tsumugi does; one copied line (with its end) and a bracketed paste
/// go in without a word.
#[test]
fn several_lines_are_asked_about_unless_bracketed() {
    assert_eq!(multiline_paste("ls -l", false), None);
    assert_eq!(multiline_paste("ls -l\n", false), None, "one copied line with its end");
    assert_eq!(multiline_paste("ls -l\r\n", false), None);
    assert_eq!(multiline_paste("cd /\nrm -rf x\n", false), Some(2));
    assert_eq!(multiline_paste("a\r\nb\r\nc", false), Some(3), "CRLF is one break");
    assert_eq!(multiline_paste("cd /\nrm -rf x\n", true), None, "bracketed: only text until Enter");
}

#[test]
fn the_paste_preview_is_the_first_lines() {
    assert_eq!(paste_preview("a\nb"), ["a", "b"]);
    let many = (1..=8).map(|n| n.to_string()).collect::<Vec<_>>().join("\n");
    assert_eq!(paste_preview(&many), ["1", "2", "3", "4", "5", "… 3 more lines"]);
    assert_eq!(paste_preview(&"y".repeat(100))[0].chars().count(), 81);
}

/// Q84: the toast is the first line, and says where the rest is.
#[test]
fn a_config_toast_is_one_line() {
    let w = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    assert_eq!(config_toast(&[]), None);
    assert_eq!(config_toast(&w(&["unknown key `x`"])).unwrap(), "Config: unknown key `x`");
    let parse = "C:/cfg/yazi.toml: TOML parse error at line 1, column 5\n  |\n1 | [mgr\n  |     ^\ninvalid table header";
    assert_eq!(config_toast(&w(&[parse])).unwrap(), "Config: C:/cfg/yazi.toml: TOML parse error at line 1, column 5 — the rest in `~`",);
    assert_eq!(config_toast(&w(&["a", "b", "c"])).unwrap(), "Config: a (+2 more, see `~`)");
    assert_eq!(
        config_toast(&w(&[parse, "b"])).unwrap(),
        "Config: C:/cfg/yazi.toml: TOML parse error at line 1, column 5 — the rest and 1 more in `~`",
    );
}

/// #295: the file's path under home is shown as `~`, so the toast stays short.
#[test]
fn a_config_toast_shortens_home() {
    let home = Path::new("/home/someone/with/a/long/name");
    let w = vec!["/home/someone/with/a/long/name/.config/yazi/yazi.toml: TOML parse error at line 1\n  |".to_string()];
    assert_eq!(config_toast_at(&w, Some(home)).unwrap(), "Config: ~/.config/yazi/yazi.toml: TOML parse error at line 1 — the rest in `~`",);
}

#[test]
fn archive_refusal_splits_writes_from_not_yet() {
    let ro = "read only";
    for a in [
        Act::Yank { cut: true },
        Act::Remove { permanently: false, force: false, hovered: false },
        Act::Create { dir: false, force: false },
        Act::Rename { force: false, cursor: RenameCursor::End },
        Act::Paste { force: false, follow: false },
        Act::Link { relative: false },
        Act::Hardlink,
    ] {
        assert!(archive_refusal(&a).contains(ro), "{a:?}");
    }
    for a in [Act::Extract, Act::Compress] {
        let m = archive_refusal(&a);
        assert!(m.contains("not available here yet") && !m.contains(ro), "{a:?}");
    }
}

/// The bare bones of a listing row: a path and whether entering it means
/// changing directory.
fn entry(path: &str, dir: bool) -> Entry {
    let path = PathBuf::from(path);
    Entry { name: util::file_name(&path), path, kind: if dir { Kind::Dir } else { Kind::File }, ..Default::default() }
}

fn binding(on: &str, run: &str, desc: &str) -> keymap::Binding {
    keymap::Binding {
        on: on.chars().map(Key::char).collect(),
        run: vec![crate::config::cmd::parse(run)],
        desc: desc.into(),
        raw: run.into(),
        from: keymap::BUILT_IN.into(),
    }
}

#[test]
fn the_other_pane_follows_its_tab_when_a_tab_closes() {
    // Tabs before it keep their index; tabs after it shift down by one.
    assert_eq!(split_after_remove(0, 2), Some(0));
    assert_eq!(split_after_remove(3, 1), Some(2));
    // Closing the tab the other pane shows leaves nothing to split with.
    assert_eq!(split_after_remove(2, 2), None);
}

/// A new tab opens where it was asked to, with no `is_dir` on the way: the
/// second half of the pair says whether the scan still has to confirm it.
#[test]
fn a_new_tab_opens_on_a_path_nobody_checked() {
    let base = Path::new("/a");
    let home = Some(PathBuf::from("/home"));

    // A typed path is unproven and may name a file, hence the fallback.
    let (to, fallback) = new_tab_target(base, false, Some("b/c"), None, home.clone());
    assert_eq!(to, PathBuf::from("/a/b/c"));
    assert!(fallback, "a typed path falls back to the parent");

    // No path and nothing to follow: home, and home is not typed.
    let (to, fallback) = new_tab_target(base, false, Some(""), None, home.clone());
    assert_eq!(to, PathBuf::from("/home"));
    assert!(!fallback);

    // Without a home directory the tab stays where it was opened from.
    let (to, _) = new_tab_target(base, false, None, None, None);
    assert_eq!(to, base.to_path_buf());
}

/// Tab asks the scan pool a question, and the answer may arrive over text
/// that has since moved on. The question is the pair below, so an answer
/// to an older one can be recognised and dropped.
/// #83: the verb once, and no byte counts for a job that has none.
#[test]
fn a_task_row_says_each_thing_once() {
    let mut t = task(0);
    t.kind = OpKind::Trash;
    t.label = "Trash 5 item(s)".into();
    t.files = 5;
    assert!(t.headline().starts_with("Trash 5 item(s)  ["), "{}", t.headline());
    assert_eq!(t.detail(), "0/5 files");
    let mut c = task(2048);
    c.bytes_done = 1024;
    assert!(c.detail().starts_with("0/1 files · 1.0 K / 2.0 K"), "{}", c.detail());
}

#[test]
fn a_long_job_behind_another_window_is_told_about_and_nothing_else() {
    let mut t = task(0);
    t.label = "Copy 3 item(s)".into();
    t.state = TaskState::Done;
    // Short, or in front: nothing.
    assert_eq!(long_op_notice(&t, true), None);
    t.started_at = Instant::now() - Duration::from_secs(11);
    assert_eq!(long_op_notice(&t, false), None);
    // Long and behind: the toast.
    let (title, body) = long_op_notice(&t, true).unwrap();
    assert_eq!((title.as_str(), body.as_str()), ("Copy finished", "Copy 3 item(s)"));
    // An error is in the toast; a cancel is the reader's own and silent.
    t.state = TaskState::Failed;
    t.errors = vec!["disk full".into()];
    let (title, body) = long_op_notice(&t, true).unwrap();
    assert_eq!(title, "Copy failed");
    assert!(body.ends_with("disk full"));
    t.state = TaskState::Cancelled;
    assert_eq!(long_op_notice(&t, true), None);
}

fn task(bytes: u64) -> Task {
    Task {
        id: 1,
        kind: OpKind::Copy,
        label: "Copy".into(),
        files: 1,
        bytes,
        files_done: 0,
        bytes_done: 0,
        current: String::new(),
        state: TaskState::Running,
        errors: Vec::new(),
        finished: None,
        speed: 0.0,
        sampled_at: Instant::now(),
        sampled_bytes: 0,
        started_at: Instant::now(),
    }
}

/// The speed is measured over a window, not per report: reports arrive
/// about twenty times a second and the gap between two of them says
/// nothing useful.
#[test]
fn a_running_job_reports_a_speed_and_what_is_left() {
    let mut t = task(3_000_000);
    // Too soon to measure: the sample is ignored and there is no answer.
    t.sample(500_000);
    assert_eq!(t.speed(), None, "a fraction of a second is not a measurement");

    // A second's worth of work, a megabyte of it.
    t.sampled_at = Instant::now() - Duration::from_secs(1);
    t.sampled_bytes = 0;
    t.sample(1_000_000);
    t.bytes_done = 1_000_000;
    let speed = t.speed().expect("a second of copying is measurable");
    assert!((900_000..=1_100_000).contains(&speed), "got {speed} B/s");

    // The first two seconds say nothing: a pile of small files looks
    // nearly done on its bytes alone, and "0s" would be a lie.
    assert_eq!(t.eta(), None, "the pace is not known yet");

    // Two of the three megabytes are left, at about a megabyte a second.
    t.started_at = Instant::now() - Duration::from_secs(3);
    let eta = t.eta().expect("bytes are known, so the rest can be timed");
    assert!((1..=7).contains(&eta.as_secs()), "got {eta:?}");
}

/// #241: a job of many small files does not read nearly done on its bytes.
#[test]
fn many_small_files_weigh_more_than_their_bytes() {
    let mut t = task(1_000_000);
    t.files = 1000;
    t.files_done = 500;
    t.bytes_done = 500_000;
    // Half the bytes and half the files: half.
    assert!((t.fraction() - 0.5).abs() < 0.01, "{}", t.fraction());
    // All the bytes, few of the files: the files are what is left.
    t.bytes_done = 1_000_000;
    t.files_done = 100;
    assert!(t.fraction() < 0.5, "{}", t.fraction());
    // A pace of 100 files in 10 s says the other 900 take 90 s, whatever
    // the byte speed was in the last quarter second.
    t.state = TaskState::Running;
    t.speed = 5_000_000.0;
    t.started_at = Instant::now() - Duration::from_secs(10);
    t.bytes_done = 999_999;
    let eta = t.eta().expect("a running job with bytes left");
    assert!(eta.as_secs() >= 20, "got {eta:?}");
}

#[test]
fn a_job_that_is_not_running_says_nothing_about_speed() {
    let mut t = task(3_000_000);
    t.sampled_at = Instant::now() - Duration::from_secs(1);
    t.sample(1_000_000);
    t.bytes_done = 1_000_000;

    // Parked: the number would be a memory, not a measurement.
    t.state = TaskState::Paused;
    assert_eq!(t.speed(), None);
    assert_eq!(t.eta(), None);

    // A job counted in files rather than bytes cannot say how long the
    // rest will take, however fast it is going.
    t.state = TaskState::Running;
    t.bytes = 0;
    assert_eq!(t.eta(), None);
    assert!(t.speed().is_some());
}

#[test]
fn a_completion_asks_about_one_directory_and_one_prefix() {
    let cwd = Path::new("/here");
    assert_eq!(completion_target("/a/b/sr", cwd), (PathBuf::from("/a/b"), "sr".into()));
    // A trailing separator asks about the directory itself.
    assert_eq!(completion_target("/a/b/", cwd), (PathBuf::from("/a/b"), String::new()));
    // Another keystroke is another question, so an answer to the old one
    // can be told apart and dropped.
    assert_ne!(completion_target("/a/b/src", cwd), completion_target("/a/b/sr", cwd));
    // A relative name completes where it was typed, as `cd` would take it.
    assert_eq!(completion_target("sr", cwd), (PathBuf::from("/here"), "sr".into()));
}

#[test]
fn a_typed_share_root_completes_against_its_host() {
    assert_eq!(share_completion(Path::new(r"\\192.168.0.150\Ba")), Some((PathBuf::from(r"\\192.168.0.150"), "Ba".into())));
    assert_eq!(share_completion(Path::new(r"\\h\s\sub")), None);
    assert_eq!(share_completion(Path::new("/a/b")), None);
}

#[test]
fn a_completion_offers_the_directories_that_carry_on_from_the_prefix() {
    let entries = vec![entry("/a/src10", true), entry("/a/Src2", true), entry("/a/srcs.txt", false), entry("/a/target", true)];
    // Case is ignored on the way in, and the order is the listing's own.
    assert_eq!(completion_hits(&entries, "sr"), vec!["Src2", "src10"]);
    // An empty prefix offers every directory, files still left out.
    assert_eq!(completion_hits(&entries, ""), vec!["Src2", "src10", "target"]);
    assert!(completion_hits(&entries, "zz").is_empty());
    // The chosen name comes back ready for the next component to be typed,
    // with the separator this platform spells paths with.
    let done = completed_text(Path::new("/a"), "Src2");
    assert!(done.ends_with(std::path::MAIN_SEPARATOR), "{done}");
    assert!(done.contains("Src2"), "{done}");
}

#[test]
fn a_new_tab_follows_the_cursor_only_onto_a_directory() {
    let base = Path::new("/a");
    let dir = entry("/a/sub", true);
    let file = entry("/a/note.txt", false);

    let (to, fallback) = new_tab_target(base, true, None, Some(&dir), None);
    assert_eq!(to, PathBuf::from("/a/sub"));
    assert!(!fallback, "the entry came from a listing, so the parent is no help");

    // The cursor on a file opens a second view of the directory instead.
    let (to, _) = new_tab_target(base, true, None, Some(&file), None);
    assert_eq!(to, base.to_path_buf());
    let (to, _) = new_tab_target(base, true, None, None, None);
    assert_eq!(to, base.to_path_buf());
}

#[test]
fn the_other_pane_follows_its_tab_when_two_tabs_swap() {
    assert_eq!(split_after_swap(1, 1, 3), 3);
    assert_eq!(split_after_swap(3, 1, 3), 1);
    // A swap between two tabs neither pane shows changes nothing.
    assert_eq!(split_after_swap(2, 0, 4), 2);
}

#[test]
fn the_default_keymap_splits_the_view() {
    let (km, _) = keymap::Keymap::load(&[]);
    let runs: Vec<&Act> = km.mgr.iter().flat_map(|b| b.run.iter()).collect();
    assert!(runs.contains(&&Act::PaneFocus(None)), "<C-w> moves between panes");
    assert!(runs.contains(&&Act::Split(Some(false))), "a key closes the split");
}

#[test]
fn palette_lists_every_command_once() {
    let bindings = vec![
        binding("k", "arrow -1", "Move cursor up"),
        // The same command on a second key: listed once, under the first.
        binding("K", "arrow -1", "Move cursor up"),
        binding("w", "tasks_show", ""),
        binding("z", "chmod", "Unimplemented"),
        binding("x", "noop", "Nothing"),
    ];
    let (items, details, runs) = palette_items(&bindings, &[]);
    assert_eq!(items, vec!["Move cursor up  ·  arrow -1".to_string(), "tasks_show".to_string()]);
    assert_eq!(details, vec!["k".to_string(), "w".to_string()]);
    assert_eq!(runs, vec![vec![Act::Arrow(Step::Rel(-1))], vec![Act::TasksShow]]);

    // The openers for the hovered file ride along at the end, named so it
    // is clear what picking one does.
    let openers = vec![(r"code %s".to_string(), false, true, "VS Code".to_string())];
    let (items, details, runs) = palette_items(&bindings, &openers);
    assert_eq!(items.last().unwrap(), "Open with VS Code");
    assert_eq!(details.last().unwrap(), "code %s");
    assert_eq!(runs.last().unwrap(), &vec![Act::Shell { run: "code %s".into(), block: false, confirm: false, orphan: true }]);
}

/// The context menu is the config read back: what `yazi.toml` opens this
/// file with, the custom `shell` actions, then the file commands. Moving
/// around and changing the view are not offered.
#[test]
fn the_menu_offers_the_openers_then_the_custom_actions_then_the_file_commands() {
    let bindings = vec![
        binding("k", "arrow -1", "Move cursor up"),
        binding("d", "remove", "Delete"),
        binding("E", "shell 'explorer %s' --orphan", "Reveal in Explorer"),
        binding("z", "chmod", "Unimplemented"),
        binding("gg", "arrow top", "Go to top"),
    ];
    let openers = vec![("notepad %s".to_string(), true, false, "Notepad".to_string())];
    let (items, details, runs) = menu_items(&bindings, &openers);

    assert_eq!(
        items,
        vec!["Notepad".to_string(), "Reveal in Explorer  ·  shell 'explorer %s' --orphan".to_string(), "Delete  ·  remove".to_string(),],
        "moving the cursor is not a thing to do to a file"
    );
    assert_eq!(details[0], "notepad %s", "the opener shows the command it runs");
    assert_eq!(details[2], "d", "a binding shows the key that also runs it");
    assert_eq!(runs[2], vec![Act::Remove { permanently: false, force: false, hovered: false }]);
    // An opener is run as the shell command it is, `block` and all.
    assert!(matches!(runs[0][0], Act::Shell { block: true, orphan: false, .. }));
}

#[test]
fn palette_filters_on_both_the_description_and_the_command() {
    let (km, warnings) = keymap::Keymap::load(&[]);
    assert!(warnings.is_empty(), "the built-in keymap must load clean: {warnings:?}");
    let (items, details, runs) = palette_items(&km.mgr, &[]);
    assert!(items.len() > 30, "got {} commands", items.len());
    // The context menu has a key of its own, so it is reachable without a
    // mouse and the palette lists it like any other command.
    assert!(runs.iter().any(|r| r == &[Act::Menu]), "the palette lists the context menu");

    // The terminal layer is loaded, and holds only the few keys the pane
    // keeps for itself — everything else has to reach the shell.
    assert!(!km.term.is_empty(), "the [term] section is read");
    assert!(km.term.iter().all(|b| b.on.len() == 1), "only single keys are consulted there, so only single keys belong");
    assert!(km.term.iter().any(|b| b.run == vec![Act::Terminal(Some(false))]), "there is a way to close it");
    assert!(km.term.iter().any(|b| matches!(b.run.first(), Some(Act::TermScroll(_)))), "and a way into the scrollback");

    assert_eq!(items.len(), details.len());
    assert_eq!(items.len(), runs.len());
    assert!(runs.iter().any(|r| r == &[Act::Palette]), "the palette lists itself");

    let mut pick = PickOverlay {
        title: "Commands".into(),
        items,
        details,
        query: "tasks_show".into(),
        matches: Vec::new(),
        cursor: 0,
        action: PickAction::Command { runs },
        focused: false,
    };
    pick.refilter();
    let by_command = pick.selected().expect("the command text must match");

    pick.query = "task manager".into();
    pick.refilter();
    assert_eq!(pick.selected(), Some(by_command), "the description finds the same row");
}

/// The point of zooming about the pointer: the pixel under the cursor is
/// the one being looked at, and it must not move.
#[test]
fn zooming_keeps_what_is_under_the_pointer_under_it() {
    let center = egui::pos2(100.0, 100.0);
    let pointer = egui::pos2(160.0, 80.0);
    let (zoom, pan) = (1.0, egui::vec2(10.0, -5.0));

    // Where in the image the pointer is, before and after.
    let at = |z: f32, p: egui::Vec2| (pointer - center - p) / z;
    let before = at(zoom, pan);
    let (z2, p2) = zoom_at(zoom, pan, center, pointer, 2.0);

    assert_eq!(z2, 2.0);
    let after = at(z2, p2);
    assert!((after - before).length() < 0.001, "{before:?} vs {after:?}");
}

#[test]
fn the_zoom_stays_within_its_bounds() {
    let (c, p) = (egui::Pos2::ZERO, egui::Pos2::ZERO);
    assert_eq!(zoom_at(ZOOM_MAX, egui::Vec2::ZERO, c, p, 4.0).0, ZOOM_MAX);
    assert_eq!(zoom_at(ZOOM_MIN, egui::Vec2::ZERO, c, p, 0.25).0, ZOOM_MIN);
}

#[test]
fn an_image_smaller_than_the_pane_cannot_be_dragged_off_center() {
    let avail = egui::vec2(400.0, 300.0);

    let stuck = clamp_pan(egui::vec2(50.0, 50.0), egui::vec2(100.0, 80.0), avail);
    assert_eq!(stuck, egui::Vec2::ZERO);

    // Twice the pane wide: it may travel half the overhang either way.
    let free = clamp_pan(egui::vec2(999.0, 0.0), egui::vec2(800.0, 300.0), avail);
    assert_eq!(free.x, 200.0);
}

#[test]
fn fit_never_magnifies_a_small_image() {
    let avail = egui::vec2(400.0, 400.0);
    assert_eq!(image_fit(avail, 40.0, 40.0), 1.0, "a small image sits at its own size");
    assert_eq!(image_fit(avail, 800.0, 400.0), 0.5, "the wider side decides");
    assert_eq!(image_fit(avail, 0.0, 0.0), 1.0, "a zero-sized image cannot divide");
}

/// The decode box steps in powers of two, so dragging the zoom about costs
/// a handful of decodes rather than one a frame, and is capped so a huge
/// photo cannot ask for a huge texture.
#[test]
fn the_decode_box_grows_in_steps_and_stops() {
    assert_eq!(zoom_box((800, 600), None, 1.0), (800, 600));
    assert_eq!(zoom_box((800, 600), Some(0.4), 1.0), (800, 600), "fitting needs no more");
    assert_eq!(zoom_box((800, 600), Some(1.5), 1.0), (1600, 1200));
    assert_eq!(zoom_box((800, 600), Some(2.0), 1.0), (1600, 1200), "same step as 1.5");
    assert_eq!(zoom_box((800, 600), Some(3.0), 1.0), (3200, 2400));
    assert_eq!(zoom_box((800, 600), Some(32.0), 1.0), (4096, 4096), "capped");

    // A big photo fits at 5%, so a tenth of full size is already twice the
    // detail the pane holds.
    assert_eq!(zoom_box((800, 600), Some(0.1), 0.05), (1600, 1200));
    assert_eq!(zoom_box((800, 600), Some(0.05), 0.05), (800, 600), "still fitting");
}

/// The whole point of weighting by age: the directory being worked in today
/// beats the one that was busy last month.
#[test]
fn frecency_puts_today_ahead_of_a_bigger_count_long_ago() {
    let today = frecency(3, 10 * 60);
    let last_month = frecency(40, 40 * DAY);

    assert!(today > last_month, "{today} vs {last_month}");
}

#[test]
fn frecency_breaks_ties_within_an_age_by_the_count() {
    assert!(frecency(5, 30) > frecency(2, 30));
    // And a clock that has run backwards is treated as "just now".
    assert_eq!(frecency(2, -500), frecency(2, 0));
}

#[test]
fn ago_is_coarse_but_never_wrong() {
    assert_eq!(ago(0), "just now");
    assert_eq!(ago(59), "just now");
    assert_eq!(ago(60), "1m ago");
    assert_eq!(ago(HOUR), "1h ago");
    assert_eq!(ago(DAY + 1), "1d ago");
    assert_eq!(ago(45 * DAY), "1mo ago");
    assert_eq!(ago(-1), "", "a time in the future says nothing at all");
}

/// A `history.txt` from before the counts were written still lists the
/// directories; they simply rank below anything visited since.
#[test]
fn a_history_line_without_counts_still_reads_as_a_visit() {
    let old = parse_visit(r"C:\work\kura");
    assert_eq!(old, Visit { path: PathBuf::from(r"C:\work\kura"), hits: 1, at: 0 });

    let new = Visit { path: PathBuf::from(r"C:\work\kura"), hits: 7, at: 1_700_000_000 };
    let row = VisitRow { path: new.path.to_string_lossy().into_owned(), hits: new.hits, at: new.at };
    let text = toml::to_string(&HistoryFile { entries: vec![row] }).unwrap();
    let back: HistoryFile = toml::from_str(&text).unwrap();
    assert_eq!(visit_of(back.entries.into_iter().next().unwrap()), new, "a round trip keeps everything");
}

fn renamed(from: &str, to: &str) -> UndoStep {
    UndoStep::Rename { from: PathBuf::from(from), to: PathBuf::from(to) }
}

#[test]
fn undoing_and_redoing_pass_the_step_between_the_stacks() {
    let mut u = Undos::default();
    u.land(renamed("a", "b"), Land::Fresh);
    assert_eq!(u.undo.len(), 1);

    // `u`: the step comes off the undo stack and lands on the redo stack.
    let step = u.undo.pop().unwrap();
    u.land(step, Land::Undone);
    assert!(u.undo.is_empty());
    assert_eq!(u.redo, vec![renamed("a", "b")]);

    // `U`: and back again, as many times as the person likes.
    let step = u.redo.pop().unwrap();
    u.land(step, Land::Redone);
    assert_eq!(u.undo, vec![renamed("a", "b")]);
    assert!(u.redo.is_empty());
}

#[test]
fn a_new_action_after_an_undo_drops_what_could_have_been_redone() {
    let mut u = Undos::default();
    u.land(renamed("a", "b"), Land::Fresh);
    let step = u.undo.pop().unwrap();
    u.land(step, Land::Undone);
    assert_eq!(u.redo.len(), 1);

    u.land(renamed("c", "d"), Land::Fresh);

    assert_eq!(u.undo, vec![renamed("c", "d")]);
    assert!(u.redo.is_empty(), "history forked, so there is no way forward");
}

/// Work that did not go through leaves the stacks as they were, so the key
/// can be pressed again once whatever was in the way is gone.
#[test]
fn a_step_that_fails_goes_back_where_it_came_from() {
    let mut u = Undos::default();
    u.keep(renamed("a", "b"), Land::Undone);
    assert_eq!(u.undo, vec![renamed("a", "b")]);

    u.keep(renamed("c", "d"), Land::Redone);
    assert_eq!(u.redo, vec![renamed("c", "d")]);

    // A fresh action that failed never happened; there is nothing to take back.
    let mut u = Undos::default();
    u.keep(renamed("a", "b"), Land::Fresh);
    assert!(u.undo.is_empty() && u.redo.is_empty());
}

#[test]
fn the_oldest_steps_fall_off_the_bottom() {
    let mut u = Undos::default();
    for i in 0..Undos::MAX + 10 {
        u.land(renamed(&format!("a{i}"), &format!("b{i}")), Land::Fresh);
    }

    assert_eq!(u.undo.len(), Undos::MAX);
    assert_eq!(u.undo[0], renamed("a10", "b10"), "the first ten are gone");
}
