//! `<Esc>` gets out of a maximized preview, and does it in the right order.

use super::*;

fn app() -> App {
    let ctx = egui::Context::default();
    App::new(Config::load(), std::env::temp_dir(), ctx)
}

/// Reported from use: `<Esc>` did nothing under a maximized preview, so the
/// next key tried was `q` -- which quits. The list is squeezed to nothing
/// there, so the screen reads as a panel even though it is a column width.
#[test]
fn a_bare_escape_restores_the_columns() {
    let mut a = app();
    a.max_preview = true;
    a.escape(EscapeWhat::default());
    assert!(!a.max_preview, "`Esc` has to be a way out of this");
}

/// Maximising is not offered where it cannot be honoured, and it never
/// leaves the keys somewhere off screen.
///
/// `Esc` is deliberately *not* a way out of this one, unlike the maximised
/// preview above. Inside the pane it belongs to the shell -- a TUI wants its
/// own -- and outside the pane this state cannot arise, because maximising
/// gives the pane the keys and every way out of the pane restores the size.
#[test]
fn maximising_the_pane_takes_the_keys_with_it() {
    // No pane: nothing happens, and nothing is remembered for the next one.
    let mut a = app();
    assert!(a.term.is_none(), "no pane in a fresh app");
    a.act(Act::MaxTerm);
    assert!(!a.max_term, "nothing to maximise, so nothing is armed");
    assert!(!a.term_focus, "and no keys are sent anywhere");
}

/// Q77: the corner tag names the pane's own way out, as bound, and is not
/// there without a full-screen program to cover the start toast's advice.
#[test]
fn the_pane_tag_names_the_key_that_leaves() {
    let mut a = app();
    assert_eq!(a.term_leave_key().as_deref(), Some("<C-t>"));
    a.term_focus = true;
    assert_eq!(a.term_leave_badge(), None, "no pane, no full-screen program");
    a.cfg.keymap.term.retain(|b| !matches!(b.run.as_slice(), [Act::Close]));
    assert_eq!(a.term_leave_key(), None, "nothing bound, nothing to name");
}

/// Leaving the pane hands the window back, by every route out of it.
///
/// `<C-t>` is the one anybody presses; the other two are the pane going
/// away underneath a maximised flag, which would otherwise be waiting for
/// whoever opens the next one.
#[test]
fn leaving_the_pane_un_maximises_it() {
    // `<C-t>` from inside: `close` on the terminal keymap.
    let mut a = app();
    a.max_term = true;
    a.term_focus = true;
    a.feed_term_key(crate::config::keys::Key::ctrl('t'), None);
    assert!(!a.term_focus, "the keys go back to the list");
    assert!(!a.max_term, "and the window with them");

    // The shell was told to go: `terminal close`.
    let mut a = app();
    a.max_term = true;
    a.act(Act::Terminal(Some(false)));
    assert!(!a.max_term, "no pane left to be maximised");
}

/// Q49: `<C-F5>` that changes `[term]` while a pane runs says the pane keeps
/// its shell until `<C-S-t>`. Without a pane, or with `[term]` unchanged,
/// the toast is the plain one -- the next `<C-t>` picks the new shell up.
#[test]
fn a_reload_that_changes_the_shell_says_the_pane_keeps_its_own() {
    let mut a = app();
    let last = |a: &App| a.toasts.last().map(|t| t.text.clone()).unwrap_or_default();
    a.act(Act::Terminal(Some(true)));
    if a.term.is_none() {
        if cfg!(any(windows, target_os = "linux")) {
            panic!("the terminal did not start: {}", last(&a));
        }
        return;
    }
    // What the files on disk will not say, whatever they hold.
    a.cfg.term.shell = "not-the-configured-shell".into();
    a.reload_config();
    assert!(last(&a).ends_with("— the pane keeps its shell until <C-S-t> closes it"), "{}", last(&a));

    a.reload_config();
    assert!(last(&a).starts_with("Reloaded") && !last(&a).contains("shell"), "unchanged: {}", last(&a));

    a.term = None;
    a.cfg.term.shell = "not-the-configured-shell".into();
    a.reload_config();
    assert!(!last(&a).contains("shell"), "no pane to keep one: {}", last(&a));
}

/// Q53: `<C-S-t>` from the list ends the shell too, not only from inside
/// the pane: `<C-t>` back to the list and then `<C-S-t>` did nothing, and a
/// re-test took the old shell for a reload that had not worked (#182). With
/// no pane it says so rather than nothing.
#[test]
fn ending_the_shell_from_the_list() {
    let mut a = app();
    let key = |a: &mut App| a.feed_key(crate::config::keys::Key::parse("<C-S-t>").unwrap());
    key(&mut a);
    assert_eq!(a.toasts.last().map(|t| t.text.as_str()), Some("No terminal to close"));

    a.act(Act::Terminal(Some(true)));
    if a.term.is_none() {
        if cfg!(any(windows, target_os = "linux")) {
            panic!("the terminal did not start");
        }
        return;
    }
    a.term_focus = false;
    key(&mut a);
    // A shell still reading its profile has children of its own, and then
    // the question comes first -- the same as from inside the pane.
    if matches!(&a.overlay, Overlay::Confirm(c) if matches!(c.action, ConfirmAction::EndShell)) {
        a.answer_confirm('y');
    }
    assert!(a.term.is_none(), "the list's `<C-S-t>` ended the shell");
    assert_eq!(a.toasts.last().map(|t| t.text.as_str()), Some("Ended the shell"));
    assert!(a.toasts.iter().all(|t| !t.text.starts_with("Started ")), "the Started toast went");
}

/// `<C-S-t>` asks before ending a shell that is running something, and
/// does what the answer says (Q21).
///
/// A real shell on a real PTY, because the question is about a real child
/// process: the shell is given a long-running command and the test waits,
/// with a deadline, until that command shows up under it. Only the busy
/// path is asserted -- whether an idle shell keeps a helper process of its
/// own is the platform's business, and "busy" errs toward asking anyway.
#[test]
fn ending_a_busy_shell_asks_first() {
    let mut a = app();
    a.act(Act::Terminal(Some(true)));
    let Some(t) = a.term.as_ref() else {
        // Not a quiet pass: this used to `return` here, and a green run
        // could not say whether the shell had ever started (TODO, v0.52.1).
        // Windows (ConPTY) and Linux (a pty pair) always have one, so no
        // terminal there is a failure, with the reason the app gave.
        let why: Vec<&str> = a.toasts.iter().map(|t| t.text.as_str()).collect();
        if cfg!(any(windows, target_os = "linux")) {
            panic!("the terminal did not start: {why:?}");
        }
        eprintln!("skipped: no terminal on this platform: {why:?}");
        return;
    };
    // A shell starting up runs short-lived children of its own (profile
    // scripts), and the first version of this test caught one of those
    // instead of the command and then found it gone. So let it settle
    // first: idle for a stretch, or give up waiting after ten seconds
    // (a shell that keeps a helper for ever never goes idle, and that is
    // fine -- the command still has to show up below).
    let (settle, mut idle_since) = (std::time::Instant::now(), std::time::Instant::now());
    while settle.elapsed() < std::time::Duration::from_secs(10) {
        if t.busy() {
            idle_since = std::time::Instant::now();
        } else if idle_since.elapsed() > std::time::Duration::from_millis(500) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
    let cmd: &[u8] = if cfg!(windows) { b"ping -n 60 127.0.0.1\r" } else { b"sleep 60\r" };
    t.send(cmd.to_vec());
    // On Windows `ping` prints its first line at once (in the OS language,
    // so the address is what is counted: once in the echoed command, once
    // in that line), and that line is the proof the command is running, whatever a slow profile script
    // does first (a loaded machine took 8 s to read its profiles, and a
    // profile child standing in for the command failed this test).
    if cfg!(windows) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
        let pinging = |a: &App| {
            a.term.as_ref().is_some_and(|t| {
                t.with_grid(|g| {
                    let rows = crate::terminal::cell_rows(g);
                    rows.iter().flat_map(|row| row.iter().map(|c| c.c)).collect::<String>().matches("127.0.0.1").count() >= 2
                })
            })
        };
        while !pinging(&a) {
            assert!(std::time::Instant::now() < deadline, "ping never started under the shell");
            a.pump_terminal();
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    }
    // Busy for a stretch, not for one look: the settling above gives up
    // after ten seconds, and on a loaded machine a profile script's child
    // was still about, so one glance saw it and went on before the command
    // had started (#122 saw this once in 553 on ARM64). The command runs
    // for a minute; a child that is gone within half a second is not it.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(40);
    let mut busy_since: Option<std::time::Instant> = None;
    loop {
        match a.term.as_ref().is_some_and(|t| t.busy()) {
            true => {
                let since = *busy_since.get_or_insert_with(std::time::Instant::now);
                if since.elapsed() > std::time::Duration::from_millis(500) {
                    break;
                }
            }
            false => busy_since = None,
        }
        assert!(std::time::Instant::now() < deadline, "the command never showed up under the shell");
        std::thread::sleep(std::time::Duration::from_millis(50));
    }

    a.act(Act::Terminal(Some(false)));
    assert!(matches!(&a.overlay, Overlay::Confirm(c) if matches!(c.action, ConfirmAction::EndShell)), "a running program is asked about",);
    a.answer_confirm('n');
    assert!(a.term.is_some(), "`n` keeps the shell and what it runs");

    a.act(Act::Terminal(Some(false)));
    a.answer_confirm('y');
    assert!(a.term.is_none(), "`y` ends it");
}

/// Q35: `<A-t>` with the pane closed opens it and types the name once the
/// shell has drawn something, rather than saying the pane is not open.
/// A real shell, because "ready" is a question only a real one answers:
/// the name has to come back on the screen as the shell's own echo.
#[test]
fn sending_a_name_opens_a_closed_pane() {
    let dir = crate::util::test_dir("q35-send");
    let file = dir.join("q35-marker.txt");
    std::fs::write(&file, "x").unwrap();
    let mut a = app();
    a.tabs[a.active].cwd = dir.clone();
    let entries = vec![crate::fs::Entry::from_path(file.clone()).unwrap()];
    a.tabs[a.active].current = Folder::from_entries(dir, Arc::new(entries), true);
    assert!(a.term.is_none());

    a.act(Act::TermSend);
    if a.term.is_none() {
        let why: Vec<&str> = a.toasts.iter().map(|t| t.text.as_str()).collect();
        if cfg!(any(windows, target_os = "linux")) {
            panic!("the terminal did not start: {why:?}");
        }
        eprintln!("skipped: no terminal on this platform: {why:?}");
        return;
    }
    assert!(!a.toasts.iter().any(|t| t.text.contains("not open")), "no refusal");

    // Generous: a loaded machine took 8 s for pwsh to read its profiles.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    // The rows joined before searching: the pane is 80 columns here, and
    // with a long `TEMP` the echoed path wraps, putting the name across
    // two rows. Looked for row by row, it was never found -- always, on a
    // machine with no RAM disk, whose scratch path is long (#203).
    // The pane's text, joined, and how wide each row it came from is: on
    // a failure, the two say whether the name never arrived or arrived
    // wrapped. #203 and #206 each had to add a print to tell (#206).
    let pane = |a: &App| {
        a.term.as_ref().map_or((String::new(), Vec::new()), |t| {
            t.with_grid(|g| {
                let rows = crate::terminal::cell_rows(g);
                let text = rows.iter().flat_map(|row| row.iter().map(|c| c.c)).collect::<String>();
                let widths = rows.iter().map(|row| row.iter().filter(|c| c.c != ' ').count()).collect();
                (text, widths)
            })
        })
    };
    while !(a.term_pending.is_none() && pane(&a).0.contains("q35-marker.txt")) {
        if std::time::Instant::now() >= deadline {
            let (text, widths) = pane(&a);
            panic!(
                "the name never reached the shell; the pane held {:?}, rows {widths:?} wide",
                text.split_whitespace().collect::<Vec<_>>().join(" "),
            );
        }
        a.pump_terminal();
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    // The whole path, not only its end: with a scratch longer than a row
    // this holds only because the rows were joined, so a re-test can read
    // that from the test passing instead of undoing the fix to see (#206).
    let (text, _) = pane(&a);
    let sent = file.display().to_string();
    assert!(text.contains(&sent), "the whole path arrived ({} characters): {sent}", sent.len());
    a.act(Act::Terminal(Some(false)));
    if a.term.is_some() {
        a.answer_confirm('y');
    }
}

/// `copy all` lays the panel out as text a spreadsheet or a bug report can
/// take: titles alone, rows as `Label<TAB>value`, sections apart (Q18).
#[test]
fn the_whole_spot_panel_copies_as_labelled_lines() {
    let sections = vec![
        Section { title: "File".into(), rows: vec![("Name".into(), "a b.txt".into()), ("Size".into(), "3 B".into())] },
        Section { title: "Git".into(), rows: vec![("Came in via".into(), "#71  48b6c9c".into())] },
    ];
    assert_eq!(spot_text(&sections), "File\nName\ta b.txt\nSize\t3 B\n\nGit\nCame in via\t#71\t48b6c9c");
}

/// `C` over a selection: a `####` heading per file, then its panel.
#[test]
fn the_spot_text_of_a_selection_has_a_heading_per_file() {
    let dir = util::test_dir("spot-many");
    let (a, b) = (dir.join("a.txt"), dir.join("b.txt"));
    std::fs::write(&a, "x").unwrap();
    std::fs::write(&b, "yy").unwrap();
    let entries: Vec<Entry> = [&a, &b].iter().map(|p| Entry::from_path((*p).clone()).unwrap()).collect();
    let text = spot_text_of(&entries);
    let heads: Vec<&str> = text.lines().filter(|l| l.starts_with("#### ")).collect();
    assert_eq!(heads, [format!("#### {}", a.display()), format!("#### {}", b.display())]);
}

/// A selection made in another folder still copies: the files come from
/// the selected paths, and one that is gone is skipped.
#[test]
fn a_selection_from_another_folder_still_has_entries() {
    let dir = util::test_dir("spot-other");
    let (a, gone) = (dir.join("a.txt"), dir.join("gone.txt"));
    std::fs::write(&a, "x").unwrap();
    let entries = entries_of_paths(vec![a.clone(), gone]);
    assert_eq!(entries.iter().map(|e| e.path.clone()).collect::<Vec<_>>(), [a]);
}

/// The default spot keys reach the two new commands: `C` copies it all,
/// and `<Enter>` is `enter`, which opens a pull request on its rows.
#[test]
fn the_spot_keys_for_copy_all_and_enter() {
    use crate::config::keys::Key;
    let km = &Config::load().keymap;
    let run = |k: Key| km.spot.iter().find(|b| b.on == vec![k]).map(|b| b.run.clone());
    assert_eq!(run(Key::parse("C").unwrap()), Some(vec![Act::Copy(CopyWhat::All)]));
    assert_eq!(run(Key::parse("<Enter>").unwrap()), Some(vec![Act::Enter]));
}

/// Q48: `C` in the help panel copies the list it shows, as text -- every
/// list key as `keys<TAB>description<TAB>command` under its heading, so a
/// check can ask "is my new key listed" without a screenshot (#171).
#[test]
fn the_help_panel_copies_as_text() {
    use crate::config::keys::{render_seq, Key};
    let a = App::new(Config::load(), std::env::temp_dir(), egui::Context::default());
    let km = &a.cfg.keymap;
    let run = km.help.iter().find(|b| b.on == vec![Key::parse("C").unwrap()]).map(|b| b.run.clone());
    assert_eq!(run, Some(vec![Act::Copy(CopyWhat::All)]));

    let (text, keys) = crate::ui::overlay::help_text(&a);
    assert_eq!(keys, km.mgr.len() + km.pick.len() + km.input.len() + km.confirm.len(), "one line per key shown");
    assert!(text.lines().any(|l| l == "keys"), "the heading on its own line");
    let b = &km.mgr[0];
    let want = format!("{}\t{}\t{}", render_seq(&b.on), if b.desc.is_empty() { &b.raw } else { &b.desc }, b.raw);
    assert!(text.lines().any(|l| l == want), "{want:?} in the copy");
    assert!(text.lines().any(|l| l == "config"), "the config section comes too");
}

/// Q36: `m u` puts the usage numbers back inside `gu`'s view after another
/// `m` key took them away; before it there was no key for them at all.
#[test]
fn m_u_is_the_usage_line_mode() {
    use crate::config::keys::Key;
    use crate::fs::entry::Linemode;
    let km = &Config::load().keymap;
    let on = vec![Key::parse("m").unwrap(), Key::parse("u").unwrap()];
    let run = km.mgr.iter().find(|b| b.on == on).map(|b| b.run.clone());
    assert_eq!(run, Some(vec![Act::Linemode(Linemode::Usage)]));
}

/// `escape --filter` is aimed at one thing and must stay aimed at it.
#[test]
fn a_targeted_escape_leaves_it_alone() {
    let mut a = app();
    a.max_preview = true;
    a.escape(EscapeWhat { filter: true, ..Default::default() });
    assert!(a.max_preview);
    // `--all` is explicitly everything, so it does clear it.
    a.escape(EscapeWhat { all: true, ..Default::default() });
    assert!(!a.max_preview);
}

/// With both up, the panel is what is in front, so it goes first and the
/// maximized column survives that press.
#[test]
fn the_quick_panel_goes_first() {
    let mut a = app();
    a.max_preview = true;
    a.quick = true;
    a.escape(EscapeWhat::default());
    assert!(!a.quick && a.max_preview, "one press should not undo two states");
    a.escape(EscapeWhat::default());
    assert!(!a.max_preview);
}

/// Turning it on unhides the parent pane, which means `T` twice is not a
/// round trip. Pinned because it is deliberate and looks like a bug.
#[test]
fn turning_it_on_unhides_the_parent() {
    let mut a = app();
    a.hide_parent = true;
    a.act(Act::MaxPreview);
    assert!(!a.hide_parent);
    a.act(Act::MaxPreview);
    assert!(!a.max_preview && !a.hide_parent, "the parent stays back");
}
