//! The usage view rides on the same machinery the search view does: a `Folder`
//! whose path is not a real directory. These pin the parts that make that work.

use super::*;

/// One directory per test. They run in parallel in one process, so a name
/// built from the pid alone has them wiping each other's trees mid-walk.
fn tree() -> PathBuf {
    let dir = crate::util::test_dir("usage-view");
    std::fs::create_dir_all(dir.join("fat").join("inner")).unwrap();
    std::fs::create_dir_all(dir.join("thin")).unwrap();
    std::fs::write(dir.join("fat").join("inner").join("a"), vec![b'x'; 900]).unwrap();
    std::fs::write(dir.join("thin").join("b"), vec![b'x'; 20]).unwrap();
    std::fs::write(dir.join("loose"), vec![b'x'; 100]).unwrap();
    dir
}

fn app_in(dir: &Path) -> App {
    let mut a = App::new(Config::load(), dir.to_path_buf(), egui::Context::default());
    a.tabs[a.active].cwd = dir.to_path_buf();
    a.tabs[a.active].current = Folder::loading(dir.to_path_buf(), None);
    a
}

/// A common.toml its watcher read again is applied by the next frame;
/// one that does not read leaves the last good settings, with a toast.
#[test]
fn a_changed_common_toml_applies_without_a_restart() {
    let dir = util::test_dir("common-watch");
    let mut a = app_in(&dir);
    let (tx, rx) = crossbeam_channel::unbounded();
    a.common_rx = Some(rx);
    let ctx = a.ctx.clone();
    tx.send(ito_common::Common::parse("language = \"ja\"\n[clock]\nhour24 = false\n")).unwrap();
    a.drain_channels(&ctx);
    assert_eq!(a.lang, "ja");
    assert!(!a.clock.hour24 && a.clock.show);
    assert!(a.toasts.is_empty(), "a good file says nothing");
    tx.send(ito_common::Common::parse("language = \"e")).unwrap();
    a.drain_channels(&ctx);
    assert_eq!(a.lang, "ja", "a file caught half-saved keeps the last good one");
    assert!(!a.clock.hour24);
    assert!(a.toasts.last().is_some_and(|t| t.text.starts_with("common.toml:")), "{:?}", a.toasts.last().map(|t| &t.text));
    assert!(!a.toasts.last().unwrap().text.starts_with("common.toml: common.toml:"), "said once");
    tx.send(ito_common::Common::parse("language = \"en\"\n[clock]\nshow = false\n")).unwrap();
    a.drain_channels(&ctx);
    assert_eq!(a.lang, "en");
    assert!(!a.clock.show);
    // The file removed reads as empty: the defaults again.
    tx.send(Ok(Default::default())).unwrap();
    a.drain_channels(&ctx);
    assert_eq!(a.clock, ito_common::Clock::default());
}

/// What the settings screen changes goes into common.toml, keeping what
/// was in it, and comes back through the watcher's channel to be applied.
#[test]
fn the_settings_screen_writes_common_toml_and_applies_it() {
    let dir = util::test_dir("settings-common");
    let base = dir.join("uchmk");
    std::fs::create_dir_all(&base).unwrap();
    std::fs::write(base.join("common.toml"), "# mine\ntheme = \"dark\"\n").unwrap();
    let mut a = app_in(&dir);
    let (tx, rx) = crossbeam_channel::unbounded();
    a.common_base = Some(base.clone());
    a.common_tx = Some(tx);
    a.common_rx = Some(rx.clone());
    a.write_common(vec![
        ito_common::CommonChange { table: None, key: "language", value: "\"ja\"".into() },
        ito_common::CommonChange { table: Some("clock"), key: "hour24", value: "false".into() },
    ]);
    let read = rx.recv_timeout(Duration::from_secs(10)).expect("the write reports back");
    a.set_common(read);
    assert_eq!(a.lang, "ja");
    assert!(!a.clock.hour24);
    let text = std::fs::read_to_string(base.join("common.toml")).unwrap();
    assert!(text.starts_with("# mine\ntheme = \"dark\"\n"), "kept what was there: {text}");
    // Nothing changed is nothing written.
    a.write_common(Vec::new());
    assert!(rx.recv_timeout(Duration::from_millis(300)).is_err());
}

/// With no config folder there is nowhere to write, and the screen says so.
#[test]
fn the_settings_screen_without_a_folder_says_so() {
    let dir = util::test_dir("settings-no-base");
    let mut a = app_in(&dir);
    a.common_base = None;
    a.write_common(vec![ito_common::CommonChange { table: None, key: "language", value: "\"en\"".into() }]);
    assert!(a.toasts.last().is_some_and(|t| t.text.starts_with("common.toml:")));
}

/// Wait for the config.toml writer and apply what it read back.
fn kura_written(a: &mut App) {
    let w = a.kura_rx.recv_timeout(Duration::from_secs(10)).expect("the writer answered");
    a.kura_written(w);
}

fn change(table: &'static str, key: &'static str, value: Option<&str>) -> KuraChange {
    KuraChange { table, key: key.into(), value: value.map(str::to_owned) }
}

/// What a press of `on` in the list runs now.
fn runs(a: &App, on: &str) -> String {
    let keys: Vec<ito_keys::Key> = on.split(' ').map(|k| ito_keys::Key::parse(k).unwrap()).collect();
    match ito_keymap::resolve(&a.cfg.keymap.mgr, &keys) {
        ito_keymap::Match::Exact(b) => b.raw.clone(),
        _ => String::new(),
    }
}

/// The Keys page writes only what it changed, into kura's own
/// keymap.toml, keeping the comments; the new keys work at once, a
/// default key taken away is a `noop`, and a reset leaves nothing behind.
#[test]
fn the_keys_page_writes_only_what_changed() {
    let dir = util::test_dir("settings-keys");
    let cfg = dir.join("cfg");
    std::fs::create_dir_all(&cfg).unwrap();
    let file = cfg.join("keymap.toml");
    std::fs::write(
        &file,
        "# my keys
",
    )
    .unwrap();
    let mut a = app_in(&dir);
    let seq = |s: &str| -> Vec<ito_keys::Key> { s.split(' ').map(|k| ito_keys::Key::parse(k).unwrap()).collect() };
    let add = |on: &str, run: &str| ito_keymap::Edit::Add { layer: "mgr".into(), on: seq(on), run: run.into(), desc: String::new() };
    let edits = vec![
        add("<C-S-v>", "paste"),
        add("g h", "paste"),
        ito_keymap::Edit::Remove { layer: "mgr".into(), on: seq("p"), run: "paste".into() },
    ];
    a.write_keymap_to(file.clone(), vec![cfg.clone()], edits);
    kura_written(&mut a);
    assert_eq!(runs(&a, "<C-S-v>"), "paste", "applied without a restart");
    assert_eq!(runs(&a, "g h"), "paste");
    assert_eq!(runs(&a, "p"), "noop", "the default key taken away");
    assert_eq!(runs(&a, "y"), "yank", "the rest stays as it was");
    let text = std::fs::read_to_string(&file).unwrap();
    assert!(
        text.starts_with(
            "# my keys
"
        ),
        "{text}"
    );
    assert!(text.contains("<C-S-v>") && text.contains("noop"), "{text}");
    assert!(!text.contains("yank"), "no default is copied in: {text}");
    assert!(a.toasts.last().is_some_and(|t| t.text.starts_with("Saved keymap.toml:")), "{:?}", a.toasts.last().map(|t| &t.text));

    // Back to the defaults: nothing of the page's is left in the file.
    a.write_keymap_to(file.clone(), vec![cfg.clone()], vec![ito_keymap::Edit::Reset { layer: "mgr".into(), run: "paste".into() }]);
    kura_written(&mut a);
    assert_eq!(runs(&a, "p"), "paste");
    assert_eq!(runs(&a, "<C-S-v>"), "");
    let text = std::fs::read_to_string(&file).unwrap();
    assert!(!text.contains("prepend_keymap") && !text.contains("paste"), "{text}");
    assert!(
        text.starts_with(
            "# my keys
"
        ),
        "{text}"
    );
}

/// With nothing that differs from the defaults to write, no keymap.toml
/// is started.
#[test]
fn the_keys_page_starts_no_file_for_nothing() {
    let dir = util::test_dir("settings-keys-none");
    let cfg = dir.join("cfg");
    let file = cfg.join("keymap.toml");
    let mut a = app_in(&dir);
    a.write_keymap_to(file.clone(), vec![cfg.clone()], vec![ito_keymap::Edit::Reset { layer: "mgr".into(), run: "paste".into() }]);
    kura_written(&mut a);
    assert!(!file.exists());
    assert_eq!(runs(&a, "p"), "paste");
    assert!(a.kura_wrote.is_none(), "no watcher event to keep quiet");
    assert!(!a.toasts.iter().any(|t| t.text.starts_with("Saved")), "nothing was saved");
}

/// Reset moves the files that exist aside and leaves the rest alone.
#[test]
fn move_aside_renames_only_what_exists() {
    let dir = util::test_dir("move-aside");
    let (a, b) = (dir.join("config.toml"), dir.join("keymap.toml"));
    std::fs::write(&a, "x").unwrap();
    assert_eq!(move_aside(&[a.clone(), b.clone()]).unwrap(), 1);
    assert!(!a.exists());
    let kept: Vec<_> = std::fs::read_dir(&dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    assert!(kept.len() == 1 && kept[0].starts_with("config.toml.bak-"), "{kept:?}");
    assert_eq!(move_aside(&[a, b]).unwrap(), 0);
}

/// The config.toml page writes its rows into the file in force, keeping the
/// comments around them, and the config read back is applied at once.
#[test]
fn the_own_config_page_writes_and_keeps_comments() {
    let dir = util::test_dir("settings-kura");
    let cfg = dir.join("cfg");
    std::fs::create_dir_all(&cfg).unwrap();
    let file = cfg.join("config.toml");
    std::fs::write(&file, "# mine\n[ui]\nfont_size = 14.0 # small\n").unwrap();
    let mut a = app_in(&dir);
    let changes = vec![change("ui", "font_size", Some("16.0")), change("term", "shell", Some("\"bash\""))];
    a.write_kura_to(file.clone(), vec![cfg.clone()], changes);
    kura_written(&mut a);
    assert_eq!(a.cfg.ui.font_size, 16.0, "applied without a restart");
    let text = std::fs::read_to_string(&file).unwrap();
    assert!(text.starts_with("# mine\n[ui]\nfont_size = 16.0 # small\n"), "kept what was there: {text}");
    assert!(text.contains("[term]\nshell = \"bash\"\n"), "{text}");
    assert!(
        a.toasts.last().is_some_and(|t| t.text.starts_with("Saved config.toml: font_size = 16.0")),
        "{:?}",
        a.toasts.last().map(|t| &t.text)
    );
    // An emptied row takes the key out, and the default comes back.
    a.write_kura_to(file.clone(), vec![cfg.clone()], vec![change("ui", "font_size", None)]);
    kura_written(&mut a);
    let text = std::fs::read_to_string(&file).unwrap();
    assert!(!text.contains("font_size"), "{text}");
    assert!(text.starts_with("# mine\n[ui]\n"), "{text}");
    assert_eq!(a.cfg.ui.font_size, crate::config::Ui::default().font_size);
    assert!(a.toasts.last().is_some_and(|t| t.text == "Saved config.toml: font_size removed"));
}

/// `[term.env]` is written one quoted key per variable and read back into
/// the config the terminal starts with; removing one leaves the others.
#[test]
fn term_env_is_written_per_variable_and_read_back() {
    let dir = util::test_dir("settings-term-env");
    let cfg = dir.join("cfg");
    std::fs::create_dir_all(&cfg).unwrap();
    let file = cfg.join("config.toml");
    std::fs::write(&file, "[term]\nshell = \"bash\"\n").unwrap();
    let mut a = app_in(&dir);
    let var = |name: &str, value: Option<&str>| change("term.env", Box::leak(format!("\"{name}\"").into_boxed_str()), value);
    a.write_kura_to(file.clone(), vec![cfg.clone()], vec![var("EDITOR", Some("\"vim\"")), var("MY.TAG", Some("\"a b\""))]);
    kura_written(&mut a);
    let text = std::fs::read_to_string(&file).unwrap();
    assert!(text.starts_with("[term]\nshell = \"bash\"\n"), "{text}");
    assert!(text.contains("[term.env]\n") && text.contains("EDITOR = \"vim\"") && text.contains("\"MY.TAG\" = \"a b\""), "{text}");
    let env = |a: &App| a.cfg.term.env.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>();
    assert_eq!(env(&a), ["EDITOR=vim", "MY.TAG=a b"]);
    a.write_kura_to(file.clone(), vec![cfg.clone()], vec![var("EDITOR", None)]);
    kura_written(&mut a);
    assert_eq!(env(&a), ["MY.TAG=a b"]);
}

/// A config.toml that is not there yet is started with its comment; one
/// that does not read as TOML is left as it is, and a toast says why.
#[test]
fn the_own_config_page_starts_a_file_and_leaves_a_broken_one() {
    let dir = util::test_dir("settings-kura-new");
    let cfg = dir.join("cfg").join("kura");
    let file = cfg.join("config.toml");
    let mut a = app_in(&dir);
    a.write_kura_to(file.clone(), vec![cfg.clone()], vec![change("ui", "max_history", Some("50"))]);
    kura_written(&mut a);
    let text = std::fs::read_to_string(&file).unwrap();
    assert!(text.starts_with(OWN_FILE_START), "{text}");
    assert!(
        text.ends_with("theme.toml.\n# Every key, with what it does, is in the README: \"config.toml\".\n\n[ui]\nmax_history = 50\n"),
        "{text}"
    );
    std::fs::write(&file, "[ui\nfont_size = 14.0\n").unwrap();
    a.write_kura_to(file.clone(), vec![cfg.clone()], vec![change("ui", "font_size", Some("16.0"))]);
    kura_written(&mut a);
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "[ui\nfont_size = 14.0\n", "left alone");
    let last = a.toasts.last().unwrap();
    assert!(last.level == Level::Error && last.text.contains("config.toml"), "{}", last.text);
}

/// The page writes to the config.toml read last, which is the one in
/// force; without one, to kura's own folder.
#[test]
fn the_own_config_in_force_is_the_last_one_read() {
    let dir = util::test_dir("settings-kura-force");
    let mut a = app_in(&dir);
    a.cfg.loaded = vec![dir.join("yazi").join("config.toml"), dir.join("yazi").join("theme.toml")];
    assert_eq!(a.own_config_in_force(), Some(dir.join("yazi").join("config.toml")));
    a.cfg.loaded.push(dir.join("kura").join("config.toml"));
    assert_eq!(a.own_config_in_force(), Some(dir.join("kura").join("config.toml")));
    a.cfg.loaded.clear();
    assert_eq!(a.own_config_in_force(), App::kura_config_dir().map(|d| d.join(crate::config::OWN_FILE)));
    // Nothing changed is nothing written.
    a.write_kura(Vec::new());
    assert!(a.kura_rx.recv_timeout(Duration::from_millis(300)).is_err());
}

#[test]
fn the_keys_page_names_its_keymap_as_the_config_read_it() {
    let dir = util::test_dir("settings-keys-own");
    let mut a = app_in(&dir);
    // yazi's config.toml is the one in force, but its keymap.toml is
    // yazi's to read too: the Keys page keeps to kura's folder.
    a.cfg.loaded = vec![dir.join("yazi").join("config.toml"), dir.join("yazi").join("keymap.toml")];
    assert_eq!(App::own_keymap(), None, "a test writes no keymap.toml it did not name");
    let own = dir.join("kura").join("keymap.toml");
    TEST_KEYMAP.with(|t| *t.borrow_mut() = Some(own.clone()));
    assert_eq!(App::own_keymap(), Some(own.clone()));
    assert_eq!(a.own_keymap_named(), Some(own.display().to_string()));
    // Read by another spelling of the same file, it is named as read.
    if cfg!(windows) {
        let other = PathBuf::from(own.display().to_string().to_uppercase());
        a.cfg.loaded.push(other.clone());
        assert_eq!(a.own_keymap_named(), Some(other.display().to_string()));
    }
    TEST_KEYMAP.with(|t| *t.borrow_mut() = None);
}

/// `<C-,>` opens the screen and closes it; Esc closes it; the page and
/// search chords reach `ito_prefs` as its `Keys`, not the list behind.
#[test]
fn the_settings_screen_keys() {
    let dir = util::test_dir("settings-keys");
    let mut a = app_in(&dir);
    let opens = a.cfg.keymap.mgr.iter().find(|b| b.on == vec![Key::parse("<C-,>").unwrap()]);
    assert_eq!(opens.map(|b| b.run.clone()), Some(vec![Act::Settings]), "`<C-,>` opens it from the list");
    let in_pane = a.cfg.keymap.term.iter().any(|b| b.on == vec![Key::parse("<C-,>").unwrap()]);
    assert!(!in_pane, "in the terminal pane `<C-,>` is the shell's");
    a.act(Act::Settings);
    assert!(matches!(a.overlay, Overlay::Settings(_)));
    let keys = |a: &App| match &a.overlay {
        Overlay::Settings(ov) => (ov.keys.next, ov.keys.back, ov.keys.find),
        _ => panic!("the screen closed"),
    };
    for (k, want) in [
        ("<C-Tab>", (true, false, false)),
        ("<C-PageDown>", (true, false, false)),
        ("<C-BackTab>", (false, true, false)),
        ("<C-PageUp>", (false, true, false)),
        ("<C-f>", (false, false, true)),
    ] {
        a.overlay = Overlay::Settings(Default::default());
        a.feed_overlay_key(Key::parse(k).unwrap());
        assert_eq!(keys(&a), want, "{k}");
    }
    // A plain key is the controls', and moves nothing behind the screen.
    let before = a.tabs[a.active].current.cursor;
    a.feed_overlay_key(Key::parse("j").unwrap());
    assert_eq!(a.tabs[a.active].current.cursor, before);
    assert!(matches!(a.overlay, Overlay::Settings(_)));
    a.feed_overlay_key(Key::parse("<C-,>").unwrap());
    assert!(matches!(a.overlay, Overlay::None), "`<C-,>` closes it again");
    a.act(Act::Settings);
    a.feed_overlay_key(Key::parse("<Esc>").unwrap());
    assert!(matches!(a.overlay, Overlay::None), "Esc closes it");
}

#[test]
fn a_completion_names_its_folder_once_it_has_waited_long_enough() {
    let dir = util::test_dir("slow-completion");
    let mut a = app_in(&dir);
    assert_eq!(a.slow_completion(), None);
    let dir_name = dir.join("share");
    let pending = |since| PendingCompletion { id: 1, dir: dir_name.clone(), prefix: String::new(), since };
    a.pending_completion = Some(pending(Instant::now()));
    assert!(a.completing());
    assert_eq!(a.slow_completion(), None);
    a.pending_completion = Some(pending(Instant::now() - App::SLOW_COMPLETION));
    assert_eq!(a.slow_completion(), Some(dir_name.display().to_string()));
}

/// Largest first, folders measured through their children, and the bars'
/// scale taken from the biggest row. Also TESTING.md 44.15 — once the walk
/// is done only the total's toast is left.
#[test]
fn the_biggest_thing_comes_first() {
    let dir = tree();
    let mut a = app_in(&dir);
    a.start_usage();
    assert!(a.usage.is_some(), "a walk is running");
    // Drain until the worker says it is done.
    for _ in 0..2000 {
        a.drain_usage();
        if a.usage.is_none() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert!(a.usage.is_none(), "the walk finished");
    // #114: the total replaces "Measuring…" rather than standing beside it.
    assert!(a.toasts.iter().any(|t| t.text.contains("in total")));
    assert!(!a.toasts.iter().any(|t| t.text.starts_with("Measuring")), "no longer measuring");
    let names: Vec<&str> = a.tabs[a.active].current.entries.iter().map(|e| e.name.as_str()).collect();
    assert_eq!(names, ["fat", "loose", "thin"], "largest first: {names:?}");
    assert_eq!(a.usage_max, 900, "the bars are drawn against the biggest row");
    let fat = a.tabs[a.active].current.entries.iter().find(|e| e.name == "fat").unwrap();
    assert_eq!(fat.usage_bytes(), 900, "a folder is worth what is under it");
    let _ = std::fs::remove_dir_all(&dir);
}

/// `l` on a folder measures that folder and stays in the view; `h` comes
/// back up with the cursor on the folder it left, and from where `gu` was
/// pressed, leaves.
#[test]
fn the_view_goes_down_and_comes_back_up() {
    let dir = tree();
    let mut a = app_in(&dir);
    let settle = |a: &mut App| {
        for _ in 0..2000 {
            a.drain_usage();
            if a.usage.is_none() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    };
    let names = |a: &App| -> Vec<String> { a.tabs[a.active].current.entries.iter().map(|e| e.name.clone()).collect() };
    a.start_usage();
    settle(&mut a);
    assert!(a.tabs[a.active].current.select_name("fat"));
    // `<Enter>` is bound to `open`, which reaches folders the same way.
    a.act(Act::Open { interactive: false, hovered: false });
    settle(&mut a);
    assert!(a.in_usage_view(), "still measuring, one level down");
    assert_eq!(a.tabs[a.active].cwd, dir.join("fat"));
    assert_eq!(names(&a), ["inner"]);
    assert_eq!(a.tabs[a.active].current.entries[0].usage, Some(900), "measured, not just listed");
    assert_eq!(a.usage_max, 900);
    assert_eq!(a.toasts.iter().filter(|t| t.text.contains(" in total")).count(), 1, "this level's total only");
    // The plain listing of `fat` arriving late must not replace the view.
    a.apply_listing(&dir.join("fat"), Arc::new(Vec::new()));
    assert_eq!(names(&a), ["inner"], "the view keeps its rows");

    a.act(Act::Leave);
    settle(&mut a);
    assert!(a.in_usage_view(), "back up, still in the view");
    assert_eq!(a.tabs[a.active].cwd, dir);
    assert_eq!(a.tabs[a.active].current.hovered_name(), Some("fat"), "on the folder it came out of");

    a.act(Act::Leave);
    assert!(!a.in_usage_view(), "from where `gu` was pressed, `h` leaves");
    assert!(!a.toasts.iter().any(|t| t.text.contains("to leave")), "nothing left saying how to leave");
    assert_eq!(a.tabs[a.active].cwd, dir);
    let _ = std::fs::remove_dir_all(&dir);
}

/// #109: re-sorting the usage view keeps the hidden rows it counted, and
/// `,s` sorts by the measured totals rather than by each entry's own length.
#[test]
fn re_sorting_keeps_every_row_and_sorts_by_total() {
    use crate::fs::SortBy;
    let dir = tree();
    std::fs::write(dir.join(".hidden"), vec![b'x'; 500]).unwrap();
    let mut a = app_in(&dir);
    a.tabs[a.active].show_hidden = false;
    a.start_usage();
    for _ in 0..2000 {
        a.drain_usage();
        if a.usage.is_none() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    let rows = |a: &App| a.tabs[a.active].current.view.len();
    assert_eq!(rows(&a), 4, "fat, .hidden, loose, thin");

    a.act(Act::Sort { by: Some(SortBy::Alphabetical), reverse: Some(false), dir_first: Some(false) });
    assert_eq!(rows(&a), 4, "the hidden row is still there after a re-sort");

    a.act(Act::Sort { by: Some(SortBy::Size), reverse: Some(true), dir_first: Some(false) });
    let names: Vec<String> = (0..rows(&a)).filter_map(|i| a.tabs[a.active].current.at(i).map(|e| e.name.clone())).collect();
    assert_eq!(names, ["fat", ".hidden", "loose", "thin"], "largest total first: {names:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Q33: the view shows sizes whatever the tab was showing, and gives the
/// tab its own mode back on the way out -- along with dropping the
/// "Measuring…" toast, which stopped being true when the walk did (#109).
#[test]
fn the_view_shows_sizes_and_puts_the_linemode_back() {
    use crate::fs::entry::Linemode;
    let dir = tree();
    let mut a = app_in(&dir);
    a.tabs[a.active].linemode = Linemode::Mtime;

    a.start_usage();
    assert_eq!(a.tabs[a.active].linemode, Linemode::Usage, "numbers without a config change");
    assert!(a.toasts.iter().any(|t| t.text.starts_with("Measuring")));

    a.exit_search_view();
    assert_eq!(a.tabs[a.active].linemode, Linemode::Mtime, "the tab's own mode again");
    assert!(!a.toasts.iter().any(|t| t.text.starts_with("Measuring")), "no longer measuring");
    let _ = std::fs::remove_dir_all(&dir);
}

/// The view is a `Folder` with a path that is not a directory, which is what
/// `in_search_view` tests -- so `<Esc>` and `leave` already handle it, and
/// dropping the handle stops the walk.
#[test]
fn leaving_the_view_stops_the_walk() {
    let dir = tree();
    let mut a = app_in(&dir);
    a.start_usage();
    assert!(a.in_search_view(), "the usage view is not a real directory");
    a.exit_search_view();
    assert!(a.usage.is_none(), "the handle is dropped, so the walk is cancelled");
    assert_eq!(a.usage_max, 0);
    assert!(!a.in_search_view(), "and we are back in the directory");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Starting a usage walk from a view that is already synthetic would leave
/// no directory to go back to.
#[test]
fn it_refuses_to_start_from_another_synthetic_view() {
    let dir = tree();
    let mut a = app_in(&dir);
    a.start_usage();
    let path = a.tabs[a.active].current.path.clone();
    a.start_usage();
    assert_eq!(a.tabs[a.active].current.path, path, "the view did not change");
    let _ = std::fs::remove_dir_all(&dir);
}
