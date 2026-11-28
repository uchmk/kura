//! The usage view rides on the same machinery the search view does: a `Folder`
//! whose path is not a real directory. These pin the parts that make that work.

use super::*;
use crate::fs::entry::Linemode;

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
/// was in it, and comes back through the writer's own channel to be applied.
#[test]
fn the_settings_screen_writes_common_toml_and_applies_it() {
    let dir = util::test_dir("settings-common");
    let base = dir.join("uchmk");
    std::fs::create_dir_all(&base).unwrap();
    std::fs::write(base.join("common.toml"), "# mine\ntheme = \"dark\"\n").unwrap();
    let mut a = app_in(&dir);
    a.common_base = Some(base.clone());
    a.write_common(vec![
        ito_common::CommonChange { table: None, key: "language", value: "\"ja\"".into() },
        ito_common::CommonChange { table: Some("clock"), key: "hour24", value: "false".into() },
    ]);
    assert!(a.config_writes_pending(), "sent, not yet answered");
    answer_config_writes(&mut a);
    assert_eq!(a.lang, "ja");
    assert!(!a.clock.hour24);
    let text = std::fs::read_to_string(base.join("common.toml")).unwrap();
    assert!(text.starts_with("# mine\ntheme = \"dark\"\n"), "kept what was there: {text}");
    // Nothing changed is nothing written.
    a.write_common(Vec::new());
    assert!(!a.config_writes_pending());
    assert!(a.config_write_failed().is_none());
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

/// Drain the channels until every config write sent has been answered.
fn answer_config_writes(a: &mut App) {
    let ctx = a.ctx.clone();
    let until = Instant::now() + Duration::from_secs(10);
    while a.config_writes_pending() {
        assert!(Instant::now() < until, "the writers answered");
        a.drain_channels(&ctx);
        std::thread::sleep(Duration::from_millis(5));
    }
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
    assert!(a.kura_stamps.is_some(), "the writer read the config back even though nothing was written");
    assert!(!a.toasts.iter().any(|t| t.text.starts_with("Saved")), "nothing was saved");
}

/// Reset moves the files that exist aside and leaves the rest alone.
#[test]
fn move_aside_renames_only_what_exists() {
    let dir = util::test_dir("move-aside");
    let (a, b) = (dir.join("config.toml"), dir.join("keymap.toml"));
    std::fs::write(&a, "x").unwrap();
    assert_eq!(move_aside(&[a.clone(), b.clone()]).0, 1);
    assert!(!a.exists());
    let kept: Vec<_> = std::fs::read_dir(&dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    assert!(kept.len() == 1 && kept[0].starts_with("config.toml.bak-"), "{kept:?}");
    let (n, r) = move_aside(&[a, b]);
    assert!(n == 0 && r.is_ok());
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
    crate::util::remove_test_dir(&dir);
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
    crate::util::remove_test_dir(&dir);
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
    crate::util::remove_test_dir(&dir);
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
    crate::util::remove_test_dir(&dir);
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
    crate::util::remove_test_dir(&dir);
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
    crate::util::remove_test_dir(&dir);
}

fn settle(a: &mut App) {
    for _ in 0..2000 {
        a.drain_usage();
        if a.usage.is_none() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
}

/// The plain listing of `dir` arriving, and the frame that follows it.
fn listed(a: &mut App, dir: &Path) {
    let v: Vec<crate::fs::Entry> =
        std::fs::read_dir(dir).unwrap().filter_map(|e| crate::fs::Entry::from_path(e.unwrap().path()).ok()).collect();
    a.apply_listing(dir, Arc::new(v));
    a.kick_scans();
}

fn rows(a: &App) -> Vec<(String, Option<u64>)> {
    a.tabs[a.active].current.entries.iter().map(|e| (e.name.clone(), e.usage)).collect()
}

/// Details measures like `gu`, and `h` goes up from where it started rather
/// than dropping the sizes.
#[test]
fn details_goes_up_past_where_it_started_and_stays_measured() {
    let dir = tree();
    let mut a = app_in(&dir.join("fat"));
    a.act(Act::Linemode(Linemode::All));
    settle(&mut a);
    assert!(a.in_usage_view());
    a.act(Act::Leave);
    settle(&mut a);
    assert!(a.in_usage_view(), "still in the usage view");
    assert_eq!(a.tabs[a.active].cwd, dir);
    assert_eq!(a.tabs[a.active].linemode, Linemode::All);
    assert_eq!(a.tabs[a.active].current.hovered_name(), Some("fat"), "on the folder it came from");
    let r = rows(&a);
    assert_eq!(r.len(), 3, "{r:?}");
    assert!(r.iter().all(|(_, u)| u.is_some()), "every row measured: {r:?}");
    crate::util::remove_test_dir(&dir);
}

/// A jump in Details lands, and the folder it lands on is measured. The old
/// walk's rows do not follow it there.
#[test]
fn details_measures_the_folder_a_jump_lands_on() {
    let dir = tree();
    let mut a = app_in(&dir);
    a.act(Act::Linemode(Linemode::All));
    a.cd(dir.join("thin"), true);
    assert!(!a.in_usage_view(), "the old view ended with the jump");
    assert!(a.usage.is_none(), "and its walk with it");
    listed(&mut a, &dir.join("thin"));
    assert!(a.in_usage_view(), "measuring where it landed");
    settle(&mut a);
    assert_eq!(rows(&a), [("b".to_string(), Some(20))], "only this folder's children");
    assert_eq!(a.tabs[a.active].linemode, Linemode::All);
    crate::util::remove_test_dir(&dir);
}

/// Out of the `gu` view a jump is a plain move.
#[test]
fn a_jump_out_of_the_gu_view_leaves_it() {
    let dir = tree();
    let mut a = app_in(&dir);
    let before = a.tabs[a.active].linemode;
    a.start_usage();
    settle(&mut a);
    a.cd(dir.join("thin"), true);
    assert!(!a.in_usage_view());
    assert!(a.usage.is_none(), "the walk is gone");
    assert_eq!(a.tabs[a.active].linemode, before, "the linemode is back");
    listed(&mut a, &dir.join("thin"));
    assert!(a.usage.is_none() && !a.in_usage_view(), "nothing measures it");
    assert_eq!(a.tabs[a.active].current.path, dir.join("thin"));
    assert_eq!(rows(&a), [("b".to_string(), None)]);
    crate::util::remove_test_dir(&dir);
}

/// Leaving Details before the landing settles cancels the measurement.
#[test]
fn leaving_details_after_a_jump_measures_nothing() {
    let dir = tree();
    let mut a = app_in(&dir);
    a.act(Act::Linemode(Linemode::All));
    a.cd(dir.join("thin"), true);
    a.act(Act::Linemode(Linemode::All));
    assert_ne!(a.tabs[a.active].linemode, Linemode::All);
    listed(&mut a, &dir.join("thin"));
    assert!(a.usage.is_none() && !a.in_usage_view());
    assert!(a.usage_follow.is_none());
    crate::util::remove_test_dir(&dir);
}

/// The walk is the app's, and `drain_usage` fills the active tab: changing
/// tabs ends it first, so no tab gets rows that are another's.
#[test]
fn a_tab_change_during_a_walk_mixes_nothing() {
    let dir = tree();
    let mut a = app_in(&dir);
    a.act(Act::Linemode(Linemode::All));
    a.create_tab(true, None);
    assert_eq!(a.active, 1);
    assert!(a.usage.is_none(), "the first tab's walk ended");
    assert!(a.tabs[0].current.path == dir && a.tabs[0].current.entries.iter().all(|e| e.usage.is_none()), "tab 0 is a plain listing");
    a.drain_usage();
    assert!(a.tabs[1].current.entries.is_empty(), "nothing leaked into the new tab");
    listed(&mut a, &dir);
    settle(&mut a);
    let r = rows(&a);
    assert_eq!(r.len(), 3, "{r:?}");
    assert!(r.iter().all(|(_, u)| u.is_some()), "the new tab, in Details, is measured: {r:?}");
    assert!(a.tabs[0].current.entries.iter().all(|e| e.usage.is_none()));

    a.switch_tab(0);
    assert!(a.tabs[1].current.entries.iter().all(|e| e.usage.is_none()), "tab 1 is plain again");
    if !a.in_usage_view() {
        listed(&mut a, &dir);
    }
    assert!(a.in_usage_view());
    settle(&mut a);
    assert!(rows(&a).iter().all(|(_, u)| u.is_some()), "and tab 0 is measured when it comes back");
    crate::util::remove_test_dir(&dir);
}

/// Details as the default linemode (yazi.toml) is a plain listing: neither a
/// jump nor coming back to the tab starts a walk.
#[test]
fn details_by_default_measures_nothing() {
    let dir = tree();
    let mut a = app_in(&dir);
    a.tabs[a.active].linemode = Linemode::All;
    listed(&mut a, &dir);
    a.cd(dir.join("thin"), true);
    listed(&mut a, &dir.join("thin"));
    assert!(a.usage.is_none() && !a.in_usage_view(), "a jump does not measure");
    a.create_tab(true, None);
    a.switch_tab(0);
    listed(&mut a, &dir.join("thin"));
    assert!(a.usage.is_none() && !a.in_usage_view(), "a tab change does not measure");
    assert_eq!(a.tabs[0].linemode, Linemode::All);
    crate::util::remove_test_dir(&dir);
}

/// A second jump before the first one has landed still measures where the
/// tab ends up.
#[test]
fn details_follows_a_jump_made_before_the_last_one_landed() {
    let dir = tree();
    let mut a = app_in(&dir);
    a.act(Act::Linemode(Linemode::All));
    a.cd(dir.join("thin"), true);
    a.cd(dir.join("fat"), true);
    listed(&mut a, &dir.join("fat"));
    assert!(a.in_usage_view(), "measuring where it landed");
    settle(&mut a);
    assert_eq!(rows(&a), [("inner".to_string(), Some(900))]);
    crate::util::remove_test_dir(&dir);
}

/// Two moves aside in one second keep both copies: the second does not
/// rename onto the first.
#[test]
fn move_aside_keeps_both_copies_made_in_one_second() {
    let dir = util::test_dir("move-aside-twice");
    let f = dir.join("config.toml");
    std::fs::write(&f, "first").unwrap();
    assert_eq!(move_aside_at(std::slice::from_ref(&f), 1000).0, 1);
    std::fs::write(&f, "second").unwrap();
    assert_eq!(move_aside_at(std::slice::from_ref(&f), 1000).0, 1);
    let kept = bak_of(&dir, "config.toml");
    assert_eq!(kept.len(), 2, "{kept:?}");
    let mut texts: Vec<_> = kept.iter().map(|p| std::fs::read_to_string(p).unwrap()).collect();
    texts.sort();
    assert_eq!(texts, ["first", "second"], "{kept:?}");
}

/// A folder holding a config.toml that sets the font size, and a keymap.toml.
fn reset_dir(label: &str) -> (PathBuf, PathBuf, PathBuf) {
    let dir = util::test_dir(label);
    let cfg = dir.join("cfg");
    std::fs::create_dir_all(&cfg).unwrap();
    std::fs::write(cfg.join("config.toml"), "[ui]\nfont_size = 19.0\n").unwrap();
    std::fs::write(cfg.join("keymap.toml"), "# mine\n").unwrap();
    (dir, cfg.join("config.toml"), cfg.join("keymap.toml"))
}

/// The copies `name` was moved to in `dir`.
fn bak_of(dir: &Path, name: &str) -> Vec<PathBuf> {
    let prefix = format!("{name}.bak-");
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.file_name().is_some_and(|n| n.to_string_lossy().starts_with(&prefix)))
        .collect();
    v.sort();
    v
}

fn toast_texts(a: &App) -> Vec<&str> {
    a.toasts.iter().map(|t| t.text.as_str()).collect()
}

/// Reset is a write like the others: pending until the writer answers, then
/// the files are aside, the defaults are read back and the toast says how many.
#[test]
fn reset_goes_through_the_writer_and_reads_the_defaults_back() {
    let (dir, config, keymap) = reset_dir("settings-reset");
    let cfg = config.parent().unwrap().to_path_buf();
    let mut a = app_in(&dir);
    a.cfg.ui.font_size = 19.0;
    a.settings_reset_files(vec![config.clone(), keymap.clone()], vec![cfg.clone()]);
    assert!(a.config_writes_pending());
    answer_config_writes(&mut a);
    assert!(!config.exists() && !keymap.exists());
    assert_eq!(bak_of(&cfg, "config.toml").len(), 1);
    assert_eq!(bak_of(&cfg, "keymap.toml").len(), 1);
    assert_eq!(a.cfg.ui.font_size, crate::config::Ui::default().font_size, "the defaults are back");
    assert!(a.toasts.iter().any(|t| t.text.starts_with("Settings reset; 2 file(s)")), "{:?}", toast_texts(&a));
    assert_eq!(a.config_write_failed(), None);
    assert!(!a.config_writes_pending());
}

/// With nothing to move, Reset says so and fails nothing.
#[test]
fn reset_with_no_files_says_so() {
    let dir = util::test_dir("settings-reset-none");
    let mut a = app_in(&dir);
    a.settings_reset_files(vec![dir.join("config.toml"), dir.join("keymap.toml")], vec![dir.clone()]);
    answer_config_writes(&mut a);
    assert!(a.toasts.last().is_some_and(|t| t.text.starts_with("Nothing to reset")), "{:?}", toast_texts(&a));
    assert_eq!(a.config_write_failed(), None);
    assert!(!a.config_writes_pending());
}

/// A config.toml write sent before a Reset lands before it, and one sent
/// after lands after it: the order of sending is the order of landing.
#[test]
fn reset_keeps_its_place_among_config_writes() {
    let (dir, config, keymap) = reset_dir("settings-reset-order");
    let cfg = config.parent().unwrap().to_path_buf();
    let mut a = app_in(&dir);
    a.write_kura_to(config.clone(), vec![cfg.clone()], vec![change("ui", "max_history", Some("77"))]);
    a.settings_reset_files(vec![config.clone(), keymap], vec![cfg.clone()]);
    a.write_kura_to(config.clone(), vec![cfg.clone()], vec![change("ui", "font_size", Some("21.0"))]);
    answer_config_writes(&mut a);
    let baks = bak_of(&cfg, "config.toml");
    assert_eq!(baks.len(), 1, "{baks:?}");
    let old = std::fs::read_to_string(&baks[0]).unwrap();
    assert!(old.contains("max_history = 77"), "the earlier write landed before the reset: {old}");
    let new = std::fs::read_to_string(&config).unwrap();
    assert!(new.contains("font_size = 21.0") && !new.contains("max_history"), "the later write landed after it: {new}");
    assert_eq!(a.config_write_failed(), None);
}

/// A bundle that is not there fails the write, and the run can see it.
#[test]
fn import_of_a_missing_file_fails_the_write() {
    let dir = util::test_dir("settings-import-missing");
    let mut a = app_in(&dir);
    a.settings_backup_in(ito_prefs::Backup::Import(dir.join("nope.toml")), dir.clone(), dir.join("kura"), vec![dir.join("kura")]);
    assert!(a.config_writes_pending());
    answer_config_writes(&mut a);
    assert!(a.config_write_failed().as_deref().is_some_and(|e| e.starts_with("Import failed:")), "{:?}", a.config_write_failed());
    assert!(!a.config_writes_pending());
}

/// An export to a path that cannot be written fails the write.
#[test]
fn export_to_an_unwritable_path_fails_the_write() {
    let dir = util::test_dir("settings-export-fails");
    let blocker = dir.join("file");
    std::fs::write(&blocker, "x").unwrap();
    let mut a = app_in(&dir);
    a.settings_backup_in(ito_prefs::Backup::Export(blocker.join("out.toml")), dir.clone(), dir.join("kura"), vec![dir.join("kura")]);
    answer_config_writes(&mut a);
    assert!(a.config_write_failed().as_deref().is_some_and(|e| e.starts_with("Export failed:")), "{:?}", a.config_write_failed());
    assert!(!a.config_writes_pending());
}

/// Export, change everything on disk, Import: the files, the theme and the
/// language are back, applied without a restart.
#[test]
fn export_then_import_brings_the_settings_back() {
    let dir = util::test_dir("settings-roundtrip");
    let (base, app_dir) = (dir.join("uchmk"), dir.join("uchmk").join("kura"));
    let themes = base.join("themes");
    std::fs::create_dir_all(&themes).unwrap();
    std::fs::create_dir_all(&app_dir).unwrap();
    let (common, theme, config) = (base.join("common.toml"), themes.join("roundtrip.toml"), app_dir.join("config.toml"));
    let common_text = "language = \"ja\"\ntheme = \"roundtrip\"\n";
    std::fs::write(&common, common_text).unwrap();
    std::fs::write(&theme, "name = \"roundtrip\"\n").unwrap();
    std::fs::write(&config, "[ui]\nfont_size = 18.0\n").unwrap();
    let bundle = dir.join("bundle.toml");
    let mut a = app_in(&dir);
    a.settings_backup_in(ito_prefs::Backup::Export(bundle.clone()), base.clone(), app_dir.clone(), vec![app_dir.clone()]);
    answer_config_writes(&mut a);
    assert!(a.toasts.iter().any(|t| t.text.starts_with("Wrote ") && t.text.contains("settings files")), "{:?}", toast_texts(&a));
    assert_eq!(a.config_write_failed(), None);

    std::fs::write(&common, "language = \"en\"\n").unwrap();
    std::fs::remove_file(&theme).unwrap();
    std::fs::write(&config, "[ui]\nfont_size = 12.0\n").unwrap();
    a.settings_backup_in(ito_prefs::Backup::Import(bundle), base.clone(), app_dir.clone(), vec![app_dir.clone()]);
    answer_config_writes(&mut a);
    assert_eq!(std::fs::read_to_string(&common).unwrap(), common_text);
    assert!(theme.exists(), "the theme file is back");
    assert!(
        a.kura_stamps.as_ref().is_some_and(|s| s.iter().any(|(p, _)| *p == app_dir.join("config.toml"))),
        "the times of the folder it read back are kept: {:?}",
        a.kura_stamps
    );
    assert!(std::fs::read_to_string(&config).unwrap().contains("font_size = 18.0"));
    assert_eq!(a.common.language.as_deref(), Some("ja"));
    assert_eq!(a.cfg.ui.font_size, 18.0, "the config read back is applied");
    assert!(a.themes.iter().any(|t| t.name == "roundtrip"), "the App knows the theme");
    assert!(a.toasts.iter().any(|t| t.text.starts_with("Read ") && t.text.contains("settings files")), "{:?}", toast_texts(&a));
    assert_eq!(a.config_write_failed(), None);
}

/// A write that stopped part way still applies the config it read back, and
/// then records the failure as the newest toast.
#[test]
fn a_write_that_failed_part_way_applies_the_config_then_fails() {
    let (dir, config, _keymap) = reset_dir("settings-partial");
    let cfg = config.parent().unwrap().to_path_buf();
    let mut a = app_in(&dir);
    a.cfg.ui.font_size = 11.0;
    let w = KuraWritten {
        tickets: Vec::new(),
        said: "ignored".into(),
        read: Ok(Some(Config::read_fresh(std::slice::from_ref(&cfg)))),
        common: None,
        failed: Some("Reset failed: boom".into()),
        stamps: None,
        reread: false,
    };
    a.apply_kura_written(w);
    assert_eq!(a.cfg.ui.font_size, 19.0, "the config read back is applied");
    assert!(a.config_write_failed().as_deref().is_some_and(|e| e.starts_with("Reset failed:")), "{:?}", a.config_write_failed());
    assert!(a.toasts.last().is_some_and(|t| t.text.starts_with("Reset failed:")), "{:?}", toast_texts(&a));
}

/// A move that stops part way says how many it moved, and leaves the rest.
#[test]
fn move_aside_reports_a_partial_count() {
    let dir = util::test_dir("move-aside-partial");
    let (a, b) = (dir.join("config.toml"), dir.join("keymap.toml"));
    std::fs::write(&a, "a").unwrap();
    std::fs::write(&b, "b").unwrap();
    std::fs::write(dir.join("keymap.toml.bak-1000"), "taken").unwrap();
    for n in 2..=99 {
        std::fs::write(dir.join(format!("keymap.toml.bak-1000-{n}")), "taken").unwrap();
    }
    let (n, r) = move_aside_at(&[a.clone(), b.clone()], 1000);
    assert_eq!(n, 1);
    assert!(r.is_err_and(|e| e.to_string().contains("no free name")));
    assert!(!a.exists() && b.exists(), "the first moved, the second stays");
    assert_eq!(std::fs::read_to_string(dir.join("keymap.toml.bak-1000")).unwrap(), "taken");
}

/// A Reset that moved one file and then failed says how many it kept, and
/// keeps the times of the files it read back.
#[test]
fn a_reset_that_kept_one_file_says_so_and_keeps_its_times() {
    let (dir, config, _keymap) = reset_dir("settings-partial-kept");
    let cfg = config.parent().unwrap().to_path_buf();
    let mut a = app_in(&dir);
    let w = KuraWritten {
        tickets: Vec::new(),
        said: String::new(),
        read: Ok(Some(Config::read_fresh(std::slice::from_ref(&cfg)))),
        common: None,
        failed: Some("Reset failed after keeping 1 file(s) as *.bak-…: boom".into()),
        stamps: Some(crate::config::stamps(std::slice::from_ref(&cfg))),
        reread: false,
    };
    a.apply_kura_written(w);
    assert_eq!(a.kura_stamps, Some(crate::config::stamps(std::slice::from_ref(&cfg))));
    assert!(a.toasts.last().is_some_and(|t| t.text.starts_with("Reset failed after keeping 1 file(s)")), "{:?}", toast_texts(&a));
}

fn reloaded(a: &App) -> bool {
    a.toasts.iter().any(|t| t.text.starts_with("Reloaded"))
}

/// A folder with a config.toml written by kura's writer (the font size), answered.
fn written_dir(label: &str) -> (App, PathBuf, PathBuf) {
    let dir = util::test_dir(label);
    let cfg = dir.join("cfg");
    std::fs::create_dir_all(&cfg).unwrap();
    let config = cfg.join("config.toml");
    let mut a = app_in(&dir);
    a.write_kura_to(config.clone(), vec![cfg.clone()], vec![change("ui", "font_size", Some("21.0"))]);
    answer_config_writes(&mut a);
    assert_eq!(a.cfg.ui.font_size, 21.0);
    (a, cfg, config)
}

/// kura's own write, then the watcher's re-reads of it (one or two events):
/// nothing is said and the written setting stays.
#[test]
fn a_reread_of_kuras_own_write_says_nothing() {
    let (mut a, cfg, _config) = written_dir("reread-own");
    a.toasts.clear();
    for _ in 0..2 {
        a.reread_config_in(vec![cfg.clone()]);
        answer_config_writes(&mut a);
        assert!(!reloaded(&a), "{:?}", toast_texts(&a));
        assert_eq!(a.cfg.ui.font_size, 21.0);
    }
}

/// The watcher's event arriving before the writer's answer is a job behind
/// the write, so it neither toasts nor puts the older config back.
#[test]
fn a_reread_sent_right_after_a_write_keeps_the_write() {
    let dir = util::test_dir("reread-before-answer");
    let cfg = dir.join("cfg");
    std::fs::create_dir_all(&cfg).unwrap();
    let mut a = app_in(&dir);
    a.write_kura_to(cfg.join("config.toml"), vec![cfg.clone()], vec![change("ui", "font_size", Some("22.0"))]);
    a.reread_config_in(vec![cfg.clone()]);
    a.toasts.clear();
    answer_config_writes(&mut a);
    assert!(!reloaded(&a), "{:?}", toast_texts(&a));
    assert_eq!(a.cfg.ui.font_size, 22.0);
}

/// A change from outside has other times, so it toasts and applies; the next
/// re-read with nothing changed is quiet.
#[test]
fn a_reread_of_an_outside_edit_says_reloaded_once() {
    let (mut a, cfg, config) = written_dir("reread-outside");
    a.reread_config_in(vec![cfg.clone()]);
    answer_config_writes(&mut a);
    let t = std::fs::metadata(&config).unwrap().modified().unwrap();
    std::fs::write(&config, "[ui]\nfont_size = 23.0\n").unwrap();
    // Not left to the file system's time resolution.
    let f = std::fs::File::options().write(true).open(&config).unwrap();
    f.set_modified(t + Duration::from_secs(10)).unwrap();
    drop(f);
    a.toasts.clear();
    a.reread_config_in(vec![cfg.clone()]);
    answer_config_writes(&mut a);
    assert!(reloaded(&a), "{:?}", toast_texts(&a));
    assert_eq!(a.cfg.ui.font_size, 23.0);
    a.toasts.clear();
    a.reread_config_in(vec![cfg]);
    answer_config_writes(&mut a);
    assert!(!reloaded(&a), "{:?}", toast_texts(&a));
}

/// With no read-back before it, a re-read has nothing to compare with.
#[test]
fn a_reread_with_no_earlier_read_back_says_reloaded() {
    let dir = util::test_dir("reread-first");
    let cfg = dir.join("cfg");
    std::fs::create_dir_all(&cfg).unwrap();
    let mut a = app_in(&dir);
    assert!(a.kura_stamps.is_none());
    a.reread_config_in(vec![cfg]);
    answer_config_writes(&mut a);
    assert!(reloaded(&a), "{:?}", toast_texts(&a));
}

/// An Export reads nothing back, so the times stay as they were.
#[test]
fn an_export_leaves_the_times_alone() {
    let (mut a, cfg, _config) = written_dir("reread-export");
    let before = a.kura_stamps.clone();
    assert!(before.is_some());
    let dir = cfg.parent().unwrap().to_path_buf();
    let (base, app_dir) = (dir.join("uchmk"), dir.join("uchmk").join("kura"));
    std::fs::create_dir_all(&app_dir).unwrap();
    a.settings_backup_in(ito_prefs::Backup::Export(dir.join("bundle.toml")), base, app_dir, vec![cfg]);
    answer_config_writes(&mut a);
    assert!(a.toasts.iter().any(|t| t.text.starts_with("Wrote ")), "{:?}", toast_texts(&a));
    assert_eq!(a.kura_stamps, before);
}

/// An Import that wrote config.toml, and a Reset that moved files, need no
/// special rule: the re-read after them is quiet.
#[test]
fn a_reread_after_an_import_or_a_reset_is_quiet() {
    let dir = util::test_dir("reread-import-reset");
    let (base, app_dir) = (dir.join("uchmk"), dir.join("uchmk").join("kura"));
    std::fs::create_dir_all(&app_dir).unwrap();
    let config = app_dir.join("config.toml");
    std::fs::write(&config, "[ui]\nfont_size = 18.0\n").unwrap();
    let bundle = dir.join("bundle.toml");
    let mut a = app_in(&dir);
    a.settings_backup_in(ito_prefs::Backup::Export(bundle.clone()), base.clone(), app_dir.clone(), vec![app_dir.clone()]);
    answer_config_writes(&mut a);
    std::fs::write(&config, "[ui]\nfont_size = 12.0\n").unwrap();
    a.settings_backup_in(ito_prefs::Backup::Import(bundle), base, app_dir.clone(), vec![app_dir.clone()]);
    answer_config_writes(&mut a);
    assert_eq!(a.cfg.ui.font_size, 18.0);
    a.toasts.clear();
    a.reread_config_in(vec![app_dir.clone()]);
    answer_config_writes(&mut a);
    assert!(!reloaded(&a), "after an import: {:?}", toast_texts(&a));
    assert_eq!(a.cfg.ui.font_size, 18.0);

    a.settings_reset_files(vec![config.clone()], vec![app_dir.clone()]);
    answer_config_writes(&mut a);
    assert!(!config.exists());
    a.toasts.clear();
    a.reread_config_in(vec![app_dir]);
    answer_config_writes(&mut a);
    assert!(!reloaded(&a), "after a reset: {:?}", toast_texts(&a));
}

/// Any number of watcher events in one frame make one re-read job.
#[test]
fn watcher_events_make_one_reread_job() {
    let dir = util::test_dir("reread-watcher");
    let mut a = app_in(&dir);
    let (tx, rx) = crossbeam_channel::unbounded();
    a.watched_rx = Some(rx);
    a.watched_tx = Some(tx.clone());
    tx.send(Watched::Config).unwrap();
    tx.send(Watched::Config).unwrap();
    let before = a.config_status().answers;
    a.drain_watched();
    assert!(a.config_writes_pending());
    answer_config_writes(&mut a);
    assert_eq!(a.config_status().answers, before + 1);
}

/// A measuring fed by hand: `feed` pushes one batch and drains it.
fn manual(a: &mut App) -> crossbeam_channel::Sender<crate::fs::usage::Msg> {
    a.start_usage();
    let (tx, h) = crate::fs::usage::test_handle();
    a.usage = Some(h);
    tx
}

fn feed(a: &mut App, tx: &crossbeam_channel::Sender<crate::fs::usage::Msg>, rows: &[(&str, u64)]) {
    let v = rows
        .iter()
        .map(|&(n, b)| {
            let e = crate::fs::Entry { path: PathBuf::from(n), name: n.into(), kind: crate::fs::entry::Kind::Dir, ..Default::default() };
            (e, b, 1, true)
        })
        .collect();
    tx.send(crate::fs::usage::Msg::Sized(v)).unwrap();
    a.drain_usage();
}

fn hovered(a: &App) -> Option<&str> {
    a.tabs[a.active].current.hovered_name()
}

/// #525: the wanted row is found at the top, then a bigger folder arrives.
#[test]
fn a_wanted_row_found_at_the_top_is_followed_when_it_sinks() {
    let dir = tree();
    let mut a = app_in(&dir);
    let tx = manual(&mut a);
    a.usage_want = Some("b".into());
    feed(&mut a, &tx, &[("b", 100)]);
    assert_eq!(hovered(&a), Some("b"));
    feed(&mut a, &tx, &[("c", 500)]);
    assert_eq!(a.tabs[a.active].current.cursor, 1);
    assert_eq!(hovered(&a), Some("b"));
    crate::util::remove_test_dir(&dir);
}

/// #525: batches landing above and below the wanted row leave the cursor on it.
#[test]
fn a_wanted_row_found_lower_stays_under_the_cursor() {
    let dir = tree();
    let mut a = app_in(&dir);
    let tx = manual(&mut a);
    a.usage_want = Some("m".into());
    feed(&mut a, &tx, &[("a", 900), ("m", 500), ("z", 100)]);
    assert_eq!(hovered(&a), Some("m"));
    feed(&mut a, &tx, &[("b", 700), ("y", 50)]);
    assert_eq!(hovered(&a), Some("m"));
    feed(&mut a, &tx, &[("c", 800), ("d", 300), ("e", 1000)]);
    assert_eq!(hovered(&a), Some("m"));
    crate::util::remove_test_dir(&dir);
}

/// A fresh view keeps the top row, the biggest so far, through the batches.
#[test]
fn a_fresh_view_stays_on_the_biggest_row() {
    let dir = tree();
    let mut a = app_in(&dir);
    let tx = manual(&mut a);
    for (i, n) in ["a", "b", "c", "d"].into_iter().enumerate() {
        feed(&mut a, &tx, &[(n, 100 * (i as u64 + 1)), ("tiny", 1)]);
        assert_eq!(a.tabs[a.active].current.cursor, 0);
        assert_eq!(hovered(&a), Some(n));
    }
    crate::util::remove_test_dir(&dir);
}

/// The same with a filter set, where `view` is not the identity: read through
/// the old `view`, row 0 is `x5` once the batch has moved the entries.
#[test]
fn a_fresh_filtered_view_stays_on_its_top_row() {
    let dir = tree();
    let mut a = app_in(&dir);
    let tx = manual(&mut a);
    a.tabs[a.active].current.filter = Some(Filter::new("x".into()).unwrap());
    feed(&mut a, &tx, &[("y1", 50), ("x1", 10)]);
    assert_eq!(hovered(&a), Some("x1"));
    feed(&mut a, &tx, &[("x9", 900), ("x5", 500)]);
    assert_eq!(a.tabs[a.active].current.cursor, 0);
    assert_eq!(hovered(&a), Some("x9"));
    crate::util::remove_test_dir(&dir);
}

/// A cursor the user moved stays on its file through later batches.
#[test]
fn a_moved_cursor_stays_on_its_file() {
    let dir = tree();
    let mut a = app_in(&dir);
    let tx = manual(&mut a);
    feed(&mut a, &tx, &[("a", 900), ("b", 500), ("c", 100)]);
    a.tabs[a.active].current.cursor = 1;
    assert_eq!(hovered(&a), Some("b"));
    feed(&mut a, &tx, &[("d", 700), ("e", 50)]);
    assert_eq!(hovered(&a), Some("b"));
    feed(&mut a, &tx, &[("f", 2000)]);
    assert_eq!(hovered(&a), Some("b"));
    crate::util::remove_test_dir(&dir);
}

/// A new measuring starts fresh: a want placed in the last one does not carry over.
#[test]
fn measuring_again_forgets_a_placed_want() {
    let dir = tree();
    let mut a = app_in(&dir);
    let tx = manual(&mut a);
    a.usage_want = Some("c".into());
    feed(&mut a, &tx, &[("a", 900), ("c", 100)]);
    assert!(a.usage_placed);
    assert_eq!(hovered(&a), Some("c"));
    drop(tx);
    let tx = manual_again(&mut a);
    assert!(!a.usage_placed);
    feed(&mut a, &tx, &[("p", 10)]);
    feed(&mut a, &tx, &[("q", 20)]);
    assert_eq!(hovered(&a), Some("q"));
    crate::util::remove_test_dir(&dir);
}

fn manual_again(a: &mut App) -> crossbeam_channel::Sender<crate::fs::usage::Msg> {
    a.measure_here(false);
    let (tx, h) = crate::fs::usage::test_handle();
    a.usage = Some(h);
    tx
}

/// `m a` builds the measured list afresh; the cursor stays on the file it was on
/// (until v0.167.2 it went to the top row).
#[test]
fn details_keeps_the_hovered_file() {
    let dir = tree();
    let mut a = app_in(&dir);
    listed(&mut a, &dir);
    assert!(a.tabs[a.active].current.select_name("loose"));
    assert_ne!(a.tabs[a.active].current.cursor, 0, "below the first row");
    a.act(Act::Linemode(Linemode::All));
    settle(&mut a);
    assert!(a.in_usage_view());
    assert_eq!(rows(&a).len(), 3);
    assert_eq!(a.tabs[a.active].current.hovered_name(), Some("loose"));
    crate::util::remove_test_dir(&dir);
}

/// Drain the walk until `done` holds.
fn until(a: &mut App, done: impl Fn(&App) -> bool) {
    for _ in 0..10_000 {
        a.drain_usage();
        if done(a) {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    panic!("the walk did not get there");
}

fn texts(a: &App) -> Vec<String> {
    a.toasts.iter().map(|t| t.text.clone()).collect()
}

/// `tree()` and one more small file, so two rows are measured by the time
/// the folders are left for `gu`.
fn tree_with_tiny() -> PathBuf {
    let dir = tree();
    std::fs::write(dir.join("tiny"), vec![b'x'; 5]).unwrap();
    dir
}

/// A limited walk fed by hand.
fn manual_limited(a: &mut App) -> crossbeam_channel::Sender<crate::fs::usage::Msg> {
    a.start_usage_auto();
    let (tx, h) = crate::fs::usage::test_handle_limited();
    a.usage = Some(h);
    tx
}

fn row(name: &str) -> crate::fs::Entry {
    crate::fs::Entry { path: PathBuf::from(name), name: name.into(), kind: crate::fs::entry::Kind::Dir, ..Default::default() }
}

fn escape(a: &mut App) {
    a.act(Act::Escape(crate::config::cmd::EscapeWhat::default()));
}

/// The sender of a walk's channel is its walker thread: it is gone once the
/// walker has ended.
fn walker_ended(rx: &crossbeam_channel::Receiver<crate::fs::usage::Msg>) {
    let until = Instant::now() + Duration::from_secs(10);
    loop {
        match rx.recv_timeout(Duration::from_millis(20)) {
            Err(crossbeam_channel::RecvTimeoutError::Disconnected) => return,
            _ => assert!(Instant::now() < until, "the walker ended"),
        }
    }
}

/// `m a` lists everything, sizes the files and, with no time to
/// measure by itself, leaves the folders blank and says so; `gu` finishes.
#[test]
fn details_pauses_and_gu_measures_the_rest() {
    let dir = tree_with_tiny();
    let mut a = app_in(&dir);
    a.cfg.ui.usage_auto_ms = 0;
    a.act(Act::Linemode(Linemode::All));
    until(&mut a, |a| a.usage_paused);
    assert!(a.usage_listed && a.in_usage_view());
    let mut r = rows(&a);
    r.sort();
    assert_eq!(
        r,
        [("fat".to_string(), None), ("loose".to_string(), Some(100)), ("thin".to_string(), None), ("tiny".to_string(), Some(5))],
        "every row listed, the files sized, the folders blank"
    );
    let t = texts(&a);
    assert!(t.iter().any(|t| t.starts_with("Big folder: paused") && t.ends_with("<Esc> to leave")), "{t:?}");
    assert!(!t.iter().any(|t| t.starts_with("Measuring")), "the pause replaces Measuring...: {t:?}");
    assert!(a.usage.as_ref().is_some_and(|h| h.limited()));
    // A walk waiting on `gu` is not one `--keys` waits on: the run would end
    // `keys: unsettled` on the next key.
    assert!(a.settled(), "a paused walk is settled");

    a.act(Act::Usage);
    assert!(!a.usage_paused);
    assert!(!a.settled(), "measuring the rest is not");
    let t = texts(&a);
    assert!(t.iter().any(|t| t == "Measuring the rest… <Esc> to leave"), "{t:?}");
    assert!(!t.iter().any(|t| t.starts_with("Big folder")), "{t:?}");
    settle(&mut a);
    assert!(a.usage.is_none() && !a.usage_paused);
    let by: std::collections::BTreeMap<_, _> = rows(&a).into_iter().collect();
    assert_eq!(by["fat"], Some(900));
    assert_eq!(by["thin"], Some(20));
    assert_eq!(a.usage_max, 900);
    let t = texts(&a);
    assert!(t.iter().any(|t| t.contains(" in total")), "{t:?}");
    assert!(!t.iter().any(|t| t.starts_with("Measuring") || t.starts_with("Big folder")), "{t:?}");

    // A finished walk: `gu` says nothing and changes nothing.
    let before = texts(&a);
    a.act(Act::Usage);
    assert_eq!(texts(&a), before);
    assert!(a.in_usage_view());
    crate::util::remove_test_dir(&dir);
}

/// A light folder is measured at once: nothing waits for `gu`.
#[test]
fn a_light_folder_is_measured_by_itself() {
    let dir = tree();
    let mut a = app_in(&dir);
    a.cfg.ui.usage_auto_ms = 60_000;
    a.act(Act::Linemode(Linemode::All));
    settle(&mut a);
    assert!(a.usage.is_none() && !a.usage_paused);
    assert!(rows(&a).iter().all(|(_, u)| u.is_some()));
    crate::util::remove_test_dir(&dir);
}

/// A `Paused` read after `gu` was pressed is stale and sets nothing; one from
/// a walk still limited does.
#[test]
fn a_stale_pause_is_ignored() {
    let dir = tree();
    let mut a = app_in(&dir);
    let tx = manual_limited(&mut a);
    tx.send(crate::fs::usage::Msg::Paused).unwrap();
    a.drain_usage();
    assert!(a.usage_paused, "a limited walk that says it paused");
    assert!(texts(&a).iter().any(|t| t.starts_with("Big folder")));

    a.act(Act::Usage);
    assert!(!a.usage_paused);
    tx.send(crate::fs::usage::Msg::Paused).unwrap();
    a.drain_usage();
    assert!(!a.usage_paused, "it was sent before `gu`, and read after");
    assert!(!texts(&a).iter().any(|t| t.starts_with("Big folder")), "{:?}", texts(&a));
    crate::util::remove_test_dir(&dir);
}

/// `gu` that starts the view never pauses, whatever `usage_auto_ms` says.
#[test]
fn plain_gu_never_pauses() {
    let dir = tree();
    let mut a = app_in(&dir);
    a.cfg.ui.usage_auto_ms = 0;
    a.act(Act::Usage);
    assert!(!a.usage.as_ref().unwrap().limited());
    settle(&mut a);
    assert!(a.usage.is_none() && !a.usage_paused);
    assert!(rows(&a).iter().all(|(_, u)| u.is_some()), "{:?}", rows(&a));
    crate::util::remove_test_dir(&dir);
}

/// `l` in a view that `m a` started is a new walk that may pause; in one that
/// `gu` started it is not.
#[test]
fn going_down_keeps_the_way_the_view_started() {
    let dir = tree();
    let mut a = app_in(&dir);
    a.cfg.ui.usage_auto_ms = 0;
    a.act(Act::Linemode(Linemode::All));
    until(&mut a, |a| a.usage_paused);
    assert!(a.tabs[a.active].current.select_name("fat"));
    a.act(Act::Open { interactive: false, hovered: false });
    assert_eq!(a.tabs[a.active].cwd, dir.join("fat"));
    assert!(a.usage_auto && !a.usage_paused, "a new walk, not the old pause");
    until(&mut a, |a| a.usage_paused);
    assert_eq!(rows(&a), [("inner".to_string(), None)]);
    a.act(Act::Usage);
    settle(&mut a);
    assert_eq!(rows(&a), [("inner".to_string(), Some(900))]);

    let mut b = app_in(&dir);
    b.cfg.ui.usage_auto_ms = 0;
    b.act(Act::Usage);
    assert!(!b.usage_auto);
    settle(&mut b);
    assert!(b.tabs[b.active].current.select_name("fat"));
    b.act(Act::Open { interactive: false, hovered: false });
    assert!(!b.usage.as_ref().unwrap().limited(), "a gu view goes down in full");
    settle(&mut b);
    assert!(!b.usage_paused);
    assert_eq!(rows(&b), [("inner".to_string(), Some(900))]);

    // Details as the tab's own line mode (yazi.toml), `m a` never pressed:
    // `gu` is still a full walk, and so is the level `l` goes down to.
    let mut c = app_in(&dir);
    c.cfg.ui.usage_auto_ms = 0;
    c.tabs[c.active].linemode = Linemode::All;
    c.act(Act::Usage);
    assert!(!c.usage_auto && !c.usage.as_ref().unwrap().limited(), "gu in a details tab is full");
    settle(&mut c);
    assert!(!c.usage_paused);
    assert!(c.tabs[c.active].current.select_name("fat"));
    c.act(Act::Open { interactive: false, hovered: false });
    assert_eq!(c.tabs[c.active].cwd, dir.join("fat"));
    assert!(!c.usage.as_ref().unwrap().limited(), "and goes down in full");
    settle(&mut c);
    assert!(!c.usage_paused);
    assert_eq!(rows(&c), [("inner".to_string(), Some(900))]);
    crate::util::remove_test_dir(&dir);
}

/// A jump while paused ends the paused walker and measures the folder it lands
/// on; `<Esc>` leaves, clears everything and ends the walker.
#[test]
fn a_jump_while_paused_starts_over_and_escape_ends_it() {
    let dir = tree();
    let mut a = app_in(&dir);
    a.cfg.ui.usage_auto_ms = 0;
    a.act(Act::Linemode(Linemode::All));
    until(&mut a, |a| a.usage_paused);
    let old = a.usage.as_ref().unwrap().rx.clone();
    a.cd(dir.join("fat"), true);
    assert!(!a.in_usage_view() && a.usage.is_none() && !a.usage_paused, "the old walk and its pause are gone");
    walker_ended(&old);
    listed(&mut a, &dir.join("fat"));
    assert!(a.in_usage_view());
    until(&mut a, |a| a.usage_paused);
    assert_eq!(rows(&a), [("inner".to_string(), None)], "only this folder's children");

    let rx = a.usage.as_ref().unwrap().rx.clone();
    escape(&mut a);
    assert!(!a.in_usage_view() && a.usage.is_none());
    assert!(!a.usage_paused && !a.usage_listed && !a.usage_auto);
    assert!(!texts(&a).iter().any(|t| t.ends_with("<Esc> to leave")), "{:?}", texts(&a));
    walker_ended(&rx);
    crate::util::remove_test_dir(&dir);
}

/// The wanted row now arrives with the listing, before it is sized, and the
/// cursor stays on it when its size comes.
#[test]
fn a_wanted_row_listed_before_it_is_sized_keeps_the_cursor() {
    use crate::fs::usage::Msg;
    let dir = tree();
    let mut a = app_in(&dir);
    let tx = manual(&mut a);
    a.usage_want = Some("m".into());
    tx.send(Msg::Listed(vec![row("z"), row("m"), row("a")])).unwrap();
    a.drain_usage();
    assert_eq!(hovered(&a), Some("m"), "found as soon as it is listed");
    assert!(a.usage_placed && a.usage_want.is_none());
    assert!(rows(&a).iter().all(|(_, u)| u.is_none()));
    feed(&mut a, &tx, &[("a", 900)]);
    assert_eq!(hovered(&a), Some("m"));
    feed(&mut a, &tx, &[("m", 10), ("z", 500)]);
    assert_eq!(hovered(&a), Some("m"));
    assert_eq!(rows(&a), [("a".to_string(), Some(900)), ("z".to_string(), Some(500)), ("m".to_string(), Some(10))]);
    assert_eq!(a.usage_max, 900);
    crate::util::remove_test_dir(&dir);
}

/// A sized child updates the row listed under its path and keeps what else the
/// row had gained; one never listed is added.
#[test]
fn a_sized_child_finds_its_row_by_path() {
    use crate::fs::usage::Msg;
    let dir = tree();
    let mut a = app_in(&dir);
    let tx = manual(&mut a);
    let mut listed_row = row("d");
    listed_row.dir_size = Some(7);
    tx.send(Msg::Listed(vec![listed_row, row("e")])).unwrap();
    tx.send(Msg::AllListed).unwrap();
    a.drain_usage();
    assert!(a.usage_listed);
    let mut new = row("new");
    new.kind = crate::fs::entry::Kind::File;
    tx.send(Msg::Sized(vec![(row("d"), 40, 2, false), (new, 3, 1, true)])).unwrap();
    a.drain_usage();
    let f = &a.tabs[a.active].current;
    assert_eq!(f.entries.len(), 3, "d was found, new was added");
    let d = f.entries.iter().find(|e| e.name == "d").unwrap();
    assert_eq!((d.usage, d.usage_cut, d.dir_size), (Some(40), true, Some(7)));
    assert_eq!(rows(&a).last().unwrap(), &("e".to_string(), None), "unmeasured last");
    crate::util::remove_test_dir(&dir);
}

/// In the usage view an unmeasured folder sorts after the measured rows under
/// `,s` and `,S`, and the plain `gu` view puts it last too.
#[test]
fn unmeasured_rows_sort_last_in_the_usage_view() {
    use crate::fs::SortBy;
    let dir = tree_with_tiny();
    let order = |a: &App| -> Vec<String> {
        let f = &a.tabs[a.active].current;
        (0..f.view.len()).filter_map(|i| f.at(i).map(|e| e.name.clone())).collect()
    };
    let mut a = app_in(&dir);
    a.cfg.ui.usage_auto_ms = 0;
    a.act(Act::Linemode(Linemode::All));
    until(&mut a, |a| a.usage_paused);
    for (by, reverse) in [(SortBy::Size, false), (SortBy::Size, true), (SortBy::Usage, false), (SortBy::Usage, true)] {
        a.act(Act::Sort { by: Some(by), reverse: Some(reverse), dir_first: Some(false) });
        let o = order(&a);
        assert_eq!(o.len(), 4, "{o:?}");
        let mut tail = o[2..].to_vec();
        tail.sort();
        assert_eq!(tail, ["fat", "thin"], "{by:?} reverse {reverse}: the blanks stay last: {o:?}");
        let head: Vec<&str> = o[..2].iter().map(String::as_str).collect();
        assert_eq!(head, if reverse { ["loose", "tiny"] } else { ["tiny", "loose"] }, "{by:?} reverse {reverse}");
    }

    let mut b = app_in(&dir);
    let tx = manual(&mut b);
    tx.send(crate::fs::usage::Msg::Listed(vec![row("blank")])).unwrap();
    feed(&mut b, &tx, &[("small", 5), ("big", 50)]);
    assert_eq!(order(&b), ["big", "small", "blank"]);
    crate::util::remove_test_dir(&dir);
}
