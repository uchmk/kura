//! `/` `?` `S` `F` find with the matcher the other searches use, and what they
//! find is marked in the list and in the preview body.

use super::*;
use crate::preview::Span;

fn app_in(dir: &Path) -> App {
    let mut a = App::new(Config::load(), dir.to_path_buf(), egui::Context::default());
    a.tabs[a.active].cwd = dir.to_path_buf();
    a.tabs[a.active].current = Folder::loading(dir.to_path_buf(), None);
    a
}

fn text(lines: &[&str]) -> PreviewState {
    let lines = lines.iter().map(|l| vec![Span { text: (*l).to_owned(), ..Default::default() }]).collect();
    PreviewState::Ready(Payload::Text { lines, map: Vec::new(), extent: Default::default(), outline: Vec::new() })
}

fn typed(a: &mut App, prev: bool, query: &str) {
    a.open_input(InputKind::Find { prev }, "Find next", String::new());
    if let Overlay::Input(ov) = &mut a.overlay {
        ov.text = query.to_owned();
    }
    a.input_changed();
}

/// `cc` and its siblings copy every marked file, one line each; the
/// hovered row is only the answer when nothing is marked.
#[test]
fn copy_covers_every_selected_path() {
    let paths = vec![PathBuf::from("/d/a.txt"), PathBuf::from("/d/sub/b.tar.gz")];
    let sep = |p: &str| PathBuf::from(p).display().to_string();
    assert_eq!(App::copy_lines(&paths, CopyWhat::Path), format!("{}\n{}", sep("/d/a.txt"), sep("/d/sub/b.tar.gz")));
    assert_eq!(App::copy_lines(&paths, CopyWhat::Filename), "a.txt\nb.tar.gz");
    assert_eq!(App::copy_lines(&paths, CopyWhat::NameWithoutExt), "a\nb.tar");
    assert_eq!(App::copy_lines(&paths, CopyWhat::Dirname), format!("{}\n{}", sep("/d"), sep("/d/sub")));
    assert_eq!(App::copy_lines(&paths[..1], CopyWhat::Filename), "a.txt", "one path stays one line");
}

#[test]
fn copy_targets_follow_the_selection() {
    let dir = std::env::temp_dir().join(format!("kura-copy-{}", std::process::id()));
    let mut a = app_in(&dir);
    a.tabs[a.active].selected.insert(dir.join("x"));
    a.tabs[a.active].selected.insert(dir.join("y"));
    assert_eq!(a.tabs[a.active].targets(), vec![dir.join("x"), dir.join("y")]);
}

/// `/` takes a regular expression, in smart case, and jumps to the first
/// name it matches.
#[test]
fn slash_finds_by_expression() {
    let dir = crate::util::test_dir("find-marks-slash");
    for n in ["alpha.txt", "beta.log", "Gamma.log"] {
        std::fs::write(dir.join(n), "x").unwrap();
    }
    let mut a = app_in(&dir);
    a.tabs[a.active].current = Folder::from_entries(
        dir.clone(),
        Arc::new(["alpha.txt", "beta.log", "Gamma.log"].iter().map(|n| Entry::from_path(dir.join(n)).unwrap()).collect()),
        false,
    );

    typed(&mut a, false, r"\.log$");
    let cur = a.tabs[a.active].current.cursor;
    assert_eq!(a.tabs[a.active].current.at(cur).unwrap().name, "beta.log");
    let f = a.tabs[a.active].finder.clone().unwrap();
    assert!(f.names && !f.body && f.matcher.is_some());

    typed(&mut a, false, "GAMMA");
    let cur = a.tabs[a.active].current.cursor;
    assert_ne!(a.tabs[a.active].current.at(cur).unwrap().name, "Gamma.log", "a capital makes it case-sensitive");
    typed(&mut a, false, "gamma");
    let cur = a.tabs[a.active].current.cursor;
    assert_eq!(a.tabs[a.active].current.at(cur).unwrap().name, "Gamma.log");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A half-typed expression leaves the finder and the cursor alone and says
/// why in the prompt.
#[test]
fn a_half_typed_expression_says_why() {
    let dir = crate::util::test_dir("find-marks-bad");
    let mut a = app_in(&dir);
    typed(&mut a, false, "a");
    let before = a.tabs[a.active].finder.clone().unwrap();
    typed(&mut a, false, "a(");
    let Overlay::Input(ov) = &a.overlay else { panic!("the prompt is still up") };
    assert!(ov.title.starts_with("Find next -- "), "{}", ov.title);
    assert_eq!(a.tabs[a.active].finder.as_ref().unwrap().query, before.query);
    let _ = std::fs::remove_dir_all(&dir);
}

/// `S` marks the body, `s` only names, `F` both.
#[test]
fn each_search_marks_what_it_searches() {
    let dir = crate::util::test_dir("find-marks-via");
    let mut a = app_in(&dir);
    for (via, names, body) in [(SearchVia::Name, true, false), (SearchVia::Content, false, true), (SearchVia::Fuzzy, true, true)] {
        a.start_search("needle", via);
        let f = a.tabs[a.active].finder.clone().unwrap();
        assert_eq!((f.names, f.body), (names, body), "{via:?}");
        assert_eq!(f.matcher.as_ref().map(|m| m.is_fuzzy()), Some(via == SearchVia::Fuzzy));
        a.exit_search_view();
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_bad_expression_starts_no_search() {
    let dir = crate::util::test_dir("find-marks-start");
    let mut a = app_in(&dir);
    a.start_search("a(", SearchVia::Content);
    assert!(a.search.is_none() && !a.in_search_view());
    assert!(a.tabs[a.active].finder.is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

/// The lines of the file on show that the body search finds, and the file
/// opening at the first of them.
#[test]
fn the_body_matches_are_the_lines_with_the_query() {
    let dir = crate::util::test_dir("find-marks-body");
    let mut a = app_in(&dir);
    assert!(a.body_matches().is_empty(), "no search, no matches");
    a.preview.state = text(&["a", "needle one", "b", "c", "d", "e", "needle two"]);
    assert!(a.body_matches().is_empty(), "a name search does not look in the body");

    a.start_search("needle", SearchVia::Content);
    a.preview.state = text(&["a", "needle one", "b", "c", "d", "e", "needle two"]);
    assert_eq!(a.body_matches(), [1, 6]);
    a.show_first_match();
    assert_eq!(a.tabs[a.active].preview_offset, 0, "a few lines of context, not below the top");

    a.preview.state = text(&["x", "x", "x", "x", "x", "x", "x", "x", "needle"]);
    a.show_first_match();
    assert_eq!(a.tabs[a.active].preview_offset, 5);
    let _ = std::fs::remove_dir_all(&dir);
}

/// A Markdown file is searched where it is drawn: the rendered lines
/// (markup gone, a match may span several styled spans) while rendered,
/// the source lines once toggled to the source.
#[test]
fn markdown_matches_are_found_in_what_is_drawn() {
    use crate::preview::{Doc, DocLine};
    let dir = crate::util::test_dir("find-marks-md");
    let mut a = app_in(&dir);
    a.start_search("auto.*ids", SearchVia::Content);
    let span = |t: &str, bold: bool| Span { text: t.to_owned(), bold, ..Default::default() };
    let doc = Doc {
        lines: vec![
            DocLine { spans: vec![span("intro", false)], ..Default::default() },
            DocLine { spans: vec![span("the ", false), span("automated", true), span("_ids()", false)], ..Default::default() },
        ],
        ..Default::default()
    };
    let source = vec![vec![span("intro", false)], vec![span("blank", false)], vec![span("the **automated**_ids()", false)]];
    a.preview.state = PreviewState::Ready(Payload::Markdown { doc, source, map: Vec::new(), extent: Default::default() });
    a.render_markdown = true;
    assert_eq!(a.body_matches(), [1], "rendered: the second line, split over three spans");
    a.render_markdown = false;
    assert_eq!(a.body_matches(), [2], "source: its own line numbers");
    // Toggling keeps `n` going on from the match on show, in the new view's numbers.
    a.render_markdown = true;
    a.show_match(1);
    a.act(Act::ToggleRender);
    assert_eq!(at(&a), Some(2), "`n` after `M` must not start again from the top");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A paragraph the pane cut in two is one run of text to the search: `a.*b`
/// with the `a` on one line and the `b` on the next colours both.
#[test]
fn a_match_over_a_wrapped_line_is_found_on_both_halves() {
    use crate::preview::{Doc, DocLine};
    let dir = crate::util::test_dir("find-marks-wrap");
    let mut a = app_in(&dir);
    a.start_search("auto.*ids", SearchVia::Content);
    let line = |t: &str, wrap: bool| DocLine { spans: vec![Span { text: t.to_owned(), ..Default::default() }], wrap, ..Default::default() };
    let doc = Doc {
        lines: vec![line("one", false), line("an auto", false), line("matic list of ids", true), line("tail", false)],
        ..Default::default()
    };
    let m = ito_match::Matcher::new("auto.*ids").unwrap();
    let one = |a: usize, b: usize| std::iter::once(a..b).collect::<Vec<_>>();
    assert_eq!(doc.marks(&m, 1, 3), [one(3, 7), one(0, 17)]);
    assert_eq!(doc.marks(&m, 2, 3), [one(0, 17)], "asked for the second half alone");
    a.preview.state = PreviewState::Ready(Payload::Markdown { doc, source: Vec::new(), map: Vec::new(), extent: Default::default() });
    a.render_markdown = true;
    assert_eq!(a.body_matches(), [1, 2]);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Puts the hovered file's lines in the preview as the worker's answer
/// would, and opens it at its first (or, after `N`, last) match.
fn land(a: &mut App, lines: &[&str]) {
    let path = a.tabs[a.active].current.hovered().unwrap().path.clone();
    a.preview.key = Some(preview::Key { path, len: 0, mtime: None, box_size: (0, 0), cols: 0, n: 0, deep: false });
    a.preview.in_flight = false;
    a.preview.state = text(lines);
    a.show_first_match();
}

fn at(a: &App) -> Option<usize> {
    a.tabs[a.active].finder.as_ref().and_then(|f| f.at)
}

fn name(a: &App) -> String {
    a.tabs[a.active].current.hovered().unwrap().name.clone()
}

/// `n` / `N` after `S` walk the matching lines of the file on show, step
/// over to the next / previous file past its last / first one, and wrap
/// round the list.
#[test]
fn n_and_shift_n_walk_the_body_across_files() {
    const A: [&str; 7] = ["x", "needle", "x", "x", "x", "x", "needle"];
    const B: [&str; 3] = ["needle", "y", "needle"];
    let dir = crate::util::test_dir("find-marks-walk");
    for n in ["a.txt", "b.txt"] {
        std::fs::write(dir.join(n), "x").unwrap();
    }
    let mut a = app_in(&dir);
    a.start_search("needle", SearchVia::Content);
    a.search = None;
    let entries = ["a.txt", "b.txt"].iter().map(|n| Entry::from_path(dir.join(n)).unwrap()).collect();
    let mut found = Folder::from_entries(dir.join("search"), Arc::new(entries), true);
    found.state = LoadState::Ready;
    a.tabs[a.active].current = found;
    assert!(a.in_search_view());

    land(&mut a, &A);
    assert_eq!((name(&a), at(&a)), ("a.txt".into(), Some(1)), "the file opens at its first match");
    a.act(Act::FindArrow { prev: false });
    assert_eq!(at(&a), Some(6));
    assert_eq!(a.tabs[a.active].preview_offset, 3);

    a.act(Act::FindArrow { prev: false });
    assert_eq!(name(&a), "b.txt", "past the last match, the next file");
    a.act(Act::FindArrow { prev: false });
    assert_eq!(name(&a), "b.txt", "and a press before it has landed waits");
    land(&mut a, &B);
    assert_eq!(at(&a), Some(0));
    a.act(Act::FindArrow { prev: false });
    assert_eq!(at(&a), Some(2));

    a.act(Act::FindArrow { prev: true });
    assert_eq!(at(&a), Some(0), "N goes back up the same file");
    a.act(Act::FindArrow { prev: true });
    assert_eq!(name(&a), "a.txt", "and before the first match, to the file before");
    land(&mut a, &A);
    assert_eq!(at(&a), Some(6), "which opens at its last match");
    a.act(Act::FindArrow { prev: true });
    assert_eq!(at(&a), Some(1));

    a.act(Act::FindArrow { prev: true });
    assert_eq!(name(&a), "b.txt", "the list is a ring");
    land(&mut a, &B);
    assert_eq!(at(&a), Some(2));
    let _ = std::fs::remove_dir_all(&dir);
}

/// The result of `S` opens at its first match as soon as the uncoloured
/// lines arrive, not when the last of them is coloured, and the coloured
/// whole that follows leaves the reader where they have got to.
#[test]
fn a_deep_preview_opens_at_its_match_on_the_early_payload() {
    let dir = crate::util::test_dir("find-marks-early");
    std::fs::write(dir.join("a.txt"), "x").unwrap();
    let ctx = egui::Context::default();
    let mut a = app_in(&dir);
    a.start_search("needle", SearchVia::Content);
    a.search = None;
    let entries = vec![Entry::from_path(dir.join("a.txt")).unwrap()];
    let mut found = Folder::from_entries(dir.join("search"), Arc::new(entries), true);
    found.state = LoadState::Ready;
    a.tabs[a.active].current = found;
    let path = a.tabs[a.active].current.hovered().unwrap().path.clone();
    let key = preview::Key { path, len: 0, mtime: None, box_size: (0, 0), cols: 0, n: 0, deep: true };
    a.preview.key = Some(key.clone());
    a.preview.in_flight = true;
    let lines = ["x", "x", "x", "x", "x", "x", "needle"];
    let payload = || match text(&lines) {
        PreviewState::Ready(p) => p,
        _ => unreachable!(),
    };

    a.on_preview(preview::Response { key: key.clone(), payload: payload(), partial: true }, &ctx);
    assert!(a.preview.early && a.preview.in_flight);
    assert_eq!((a.tabs[a.active].preview_offset, at(&a)), (3, Some(6)), "opened at the match already");

    a.tabs[a.active].preview_offset = 1;
    a.on_preview(preview::Response { key, payload: payload(), partial: false }, &ctx);
    assert!(!a.preview.early && !a.preview.in_flight);
    assert_eq!(a.tabs[a.active].preview_offset, 1, "the coloured whole does not scroll back");
    let _ = std::fs::remove_dir_all(&dir);
}

/// `<Up>` in a prompt brings back what was submitted before, newest first,
/// `<Down>` walks forward to the line that was being written, and a repeat
/// is kept once. Search words are shared by `s S F f / ?`; a shell command
/// has a list of its own.
#[test]
fn up_in_a_prompt_recalls_what_was_typed_before() {
    let dir = std::env::temp_dir().join(format!("kura-recall-{}", std::process::id()));
    let mut a = app_in(&dir);
    // `app_in` loads the real state directory's lists, which have words in them.
    a.input_history = Default::default();
    let submit = |a: &mut App, kind: InputKind, text: &str| {
        a.open_input(kind, "x", text.to_owned());
        a.submit_input();
    };
    submit(&mut a, InputKind::Filter, "alpha");
    submit(&mut a, InputKind::Find { prev: false }, "beta");
    submit(&mut a, InputKind::Filter, "alpha");
    submit(&mut a, InputKind::Shell { block: false }, "echo hi");
    submit(&mut a, InputKind::Filter, "");
    assert_eq!(a.input_history[0], vec!["beta", "alpha"], "a repeat moves up, empty is not kept");
    assert_eq!(a.input_history[1], vec!["echo hi"]);

    let text = |a: &App| match &a.overlay {
        Overlay::Input(ov) => ov.text.clone(),
        _ => panic!("no prompt"),
    };
    a.open_input(InputKind::Search { via: SearchVia::Name }, "Search", "dra".to_owned());
    a.recall_input(true);
    assert_eq!(text(&a), "alpha");
    a.recall_input(true);
    assert_eq!(text(&a), "beta");
    a.recall_input(true);
    assert_eq!(text(&a), "beta", "the oldest stays put");
    a.recall_input(false);
    assert_eq!(text(&a), "alpha");
    a.recall_input(false);
    assert_eq!(text(&a), "dra", "past the newest comes the unfinished line back");
    a.recall_input(false);
    assert_eq!(text(&a), "dra");

    a.open_input(InputKind::Shell { block: false }, "Shell", String::new());
    a.recall_input(true);
    assert_eq!(text(&a), "echo hi");
    a.open_input(InputKind::Create, "New", String::new());
    a.recall_input(true);
    assert_eq!(text(&a), "", "a file name prompt has no history");
}

/// Every toast also lands in the bell's list, unread; the same unread
/// news twice is one line, and the list stops at [`NOTICES`].
#[test]
fn a_toast_is_kept_in_the_bells_list() {
    let dir = crate::util::test_dir("bell-notices");
    let mut a = app_in(&dir);
    a.notices.clear();
    a.raise("one".into(), Level::Info);
    a.raise("one".into(), Level::Info);
    a.raise("bad".into(), Level::Error);
    let kinds: Vec<_> = a.notices.iter().map(|n| (n.title.as_str(), n.kind)).collect();
    assert_eq!(kinds, [("one", ito_notices::Kind::Info), ("bad", ito_notices::Kind::Err)]);
    assert_eq!(ito_notices::unread(&a.notices), 2);
    assert!(ito_notices::any_error(&a.notices));
    a.notices[0].read = true;
    a.raise("one".into(), Level::Info);
    assert_eq!(a.notices.len(), 3, "a read notice does not take the repeat");
    for i in 0..NOTICES + 5 {
        a.raise(format!("n{i}"), Level::Warn);
    }
    assert_eq!(a.notices.len(), NOTICES);
    assert_eq!(a.notices.last().map(|n| n.title.as_str()), Some(format!("n{}", NOTICES + 4).as_str()));
    let _ = std::fs::remove_dir_all(&dir);
}

/// The end of a search says how many it found and where it looked, a miss
/// as well as a hit.
#[test]
fn a_finished_search_says_where_it_looked() {
    let dir = crate::util::test_dir("find-marks-scope");
    std::fs::write(dir.join("a.txt"), "needle").unwrap();
    let mut a = app_in(&dir);
    for (query, want) in [("needle", "1 match(es) in "), ("zzzz", "No matches in ")] {
        a.toast_log.clear();
        a.start_search(query, SearchVia::Content);
        let t = Instant::now();
        while a.search.is_some() && t.elapsed().as_secs() < 10 {
            std::thread::sleep(std::time::Duration::from_millis(5));
            a.drain_search();
        }
        let said = a.toast_log.back().cloned().unwrap_or_default();
        assert!(said.starts_with(want), "{said}");
        assert!(said.contains("hidden files skipped") && said.contains(".gitignore"), "{said}");
        a.tabs[a.active].current = Folder::loading(dir.clone(), None);
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// `ext:log` in a search narrows it to those files, the rest of the query
/// is what to look for (and what gets coloured), and the toast says so.
#[test]
fn ext_narrows_a_search_to_those_files() {
    let dir = crate::util::test_dir("find-marks-ext");
    std::fs::write(dir.join("a.log"), "needle").unwrap();
    std::fs::write(dir.join("b.txt"), "needle").unwrap();
    let mut a = app_in(&dir);
    a.start_search("needle ext:log", SearchVia::Content);
    let t = Instant::now();
    while a.search.is_some() && t.elapsed().as_secs() < 10 {
        std::thread::sleep(std::time::Duration::from_millis(5));
        a.drain_search();
    }
    let names: Vec<_> = a.tabs[a.active].current.entries.iter().map(|e| e.name.clone()).collect();
    assert_eq!(names, ["a.log"]);
    assert_eq!(a.tabs[a.active].finder.as_ref().map(|f| f.query.as_str()), Some("needle"));
    assert!(a.toast_log.back().is_some_and(|s| s.contains("only .log")), "{:?}", a.toast_log);

    a.tabs[a.active].current = Folder::loading(dir.clone(), None);
    a.toast_log.clear();
    a.start_search("ext:log", SearchVia::Content);
    assert!(a.search.is_none(), "an extension alone has nothing to look for in a file");
    assert!(a.last_error.as_deref().is_some_and(|e| e.contains("ext:")));
    let _ = std::fs::remove_dir_all(&dir);
}

/// A name search keeps `n` for names, and the body of the file on show is
/// left alone.
#[test]
fn a_name_search_still_steps_over_names() {
    let dir = crate::util::test_dir("find-marks-names");
    for n in ["one.log", "two.txt", "three.log"] {
        std::fs::write(dir.join(n), "x").unwrap();
    }
    let mut a = app_in(&dir);
    let entries = ["one.log", "three.log", "two.txt"].iter().map(|n| Entry::from_path(dir.join(n)).unwrap()).collect();
    a.tabs[a.active].current = Folder::from_entries(dir.clone(), Arc::new(entries), false);
    typed(&mut a, false, r"\.log$");
    assert_eq!(name(&a), "one.log");
    a.act(Act::FindArrow { prev: false });
    assert_eq!(name(&a), "three.log");
    assert!(!a.find_in_body(false));
    let _ = std::fs::remove_dir_all(&dir);
}
