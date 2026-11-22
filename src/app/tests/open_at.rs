//! The editor opens at the line on screen in `T` and `<F3>`; `<C-S-g>` asks for a line
//! (and column); `/` `?` `n` `N` search the text in `<F3>` as they do in `T`.

use super::*;
use crate::app::dispatch::parse_line_col;
use crate::config::keymap::{self, Keymap, Match};
use crate::config::keys::Key;
use crate::exec::{Col, Pos};
use crate::preview::{Span, TabStops};

fn lines_of(rows: &[String]) -> Vec<Vec<Span>> {
    rows.iter().map(|l| vec![Span { text: l.clone(), ..Default::default() }]).collect()
}

fn payload(rows: &[String], tabs: TabStops) -> PreviewState {
    PreviewState::Ready(Payload::Text { lines: lines_of(rows), map: Vec::new(), extent: Default::default(), outline: Vec::new(), tabs })
}

fn numbered(n: usize) -> Vec<String> {
    (0..n).map(|i| format!("line {i}")).collect()
}

/// An app hovering `name` in a fresh folder, with `rows` as the text on show.
fn app_with(label: &str, name: &str, rows: &[String], tabs: TabStops) -> (App, PathBuf) {
    let dir = crate::util::test_dir(label);
    std::fs::write(dir.join(name), "x").unwrap();
    let mut a = App::new(Config::load(), dir.clone(), egui::Context::default());
    a.tabs[a.active].cwd = dir.clone();
    a.tabs[a.active].current = Folder::from_entries(dir.clone(), Arc::new(vec![Entry::from_path(dir.join(name)).unwrap()]), false);
    let path = dir.join(name);
    a.preview.key = Some(preview::Key {
        path: path.clone(),
        len: 0,
        mtime: None,
        box_size: (0, 0),
        cols: 0,
        n: 0,
        deep: false,
        images: Default::default(),
    });
    a.preview.in_flight = false;
    a.preview.state = payload(rows, tabs);
    a.max_preview = true;
    a.quick = false;
    (a, path)
}

fn bound(section: &[keymap::Binding], key: &str) -> Vec<Act> {
    let k = Key::parse(key).unwrap();
    match keymap::resolve(section, &[k]) {
        Match::Exact(b) => b.run.clone(),
        _ => panic!("`{key}` is not bound on its own"),
    }
}

#[test]
fn a_typed_position_is_read_strictly() {
    assert_eq!(parse_line_col("120"), Some((120, None)));
    assert_eq!(parse_line_col("120:5"), Some((120, Some(5))));
    assert_eq!(parse_line_col(" 120 "), Some((120, None)));
    assert_eq!(parse_line_col("\u{ff11}\u{ff12}\u{ff10}"), Some((120, None)));
    assert_eq!(parse_line_col("\u{ff11}\u{ff12}\u{ff10}\u{ff1a}\u{ff15}"), Some((120, Some(5))));
    assert_eq!(parse_line_col("\u{3000}120\u{3000}"), Some((120, None)));
    assert_eq!(parse_line_col("120\t"), Some((120, None)));
    for bad in ["", "0", "120:0", "+120", "-1", "120:", ":5", "1 20", "120:5:1", "abc", "\u{ff11}a", "\u{ff10}", "99999999999999999999999"]
    {
        assert_eq!(parse_line_col(bad), None, "`{bad}`");
    }
}

#[test]
fn the_view_gives_its_top_line_only_in_t_and_quick_look() {
    let (mut a, _) = app_with("open-at-top", "a.txt", &numbered(300), TabStops::default());
    a.tabs[a.active].preview_offset = 40;
    assert_eq!(a.viewed_pos(), Some(Pos::line(41)));
    a.max_preview = false;
    assert_eq!(a.viewed_pos(), None, "not in the column");
    a.quick = true;
    assert_eq!(a.viewed_pos(), Some(Pos::line(41)));
}

#[test]
fn a_document_or_an_archive_shown_as_text_gives_no_line() {
    for name in ["a.docx", "b.xlsx", "c.zip", "d.tar.gz"] {
        let (mut a, _) = app_with("open-at-not-own", name, &numbered(300), TabStops::default());
        a.tabs[a.active].preview_offset = 40;
        assert_eq!(a.viewed_pos(), None, "{name}: its rows are not its lines");
    }
}

#[test]
fn another_files_payload_gives_nothing() {
    let (mut a, _) = app_with("open-at-other", "a.txt", &numbered(10), TabStops::default());
    a.preview.key.as_mut().unwrap().path = PathBuf::from("/elsewhere/b.txt");
    assert_eq!(a.viewed_pos(), None);
}

#[test]
fn a_goto_mark_wins_until_the_view_moves() {
    let (mut a, _) = app_with("open-at-goto", "a.txt", &numbered(300), TabStops::default());
    a.preview.max_offset = 280;
    a.preview_goto_submit("295");
    assert_eq!(a.tabs[a.active].preview_offset, 280);
    assert_eq!(a.viewed_pos(), Some(Pos::line(295)), "the typed line, though the view stops short of it");
    a.tabs[a.active].preview_offset = 100;
    assert_eq!(a.viewed_pos(), Some(Pos::line(101)), "the top row once it moved");
    a.preview_goto_submit("0");
    assert_eq!(a.tabs[a.active].preview_offset, 0, "`0` still goes to the top");
}

/// A huge log in the column: `<A-/>` made after a list find leaves `n` with the list, as before;
/// in `T` the newer find owns it.
#[test]
fn a_list_find_keeps_n_in_the_column() {
    let dir = crate::util::test_dir("open-at-colfind");
    let f = dir.join("big.log");
    std::fs::write(&f, (0..300).map(|i| if i % 100 == 50 { format!("hit {i}\n") } else { format!("line {i}\n") }).collect::<String>())
        .unwrap();
    let mut a = App::new(Config::load(), dir.clone(), egui::Context::default());
    let index = crate::preview::logindex::LogIndex::start(&f).unwrap();
    while !index.is_done() {
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    let view = crate::preview::logindex::LogView::new(index);
    a.preview.state = PreviewState::Ready(Payload::Log { view: view.clone() });
    a.max_preview = false;
    a.quick = false;
    let matcher = ito_match::Matcher::new("x").ok();
    a.tabs[a.active].finder = Some(crate::core::tab::Finder { query: "x".into(), matcher, names: true, ..Default::default() });
    a.preview_find_submit("hit");
    while !view.found().done {
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    a.find_arrow(false);
    assert_eq!(a.tabs[a.active].preview_offset, 0, "in the column `n` goes to the list find");
    a.max_preview = true;
    a.find_arrow(false);
    assert_ne!(a.tabs[a.active].preview_offset, 0, "in T the newer preview find owns `n`");
}

#[test]
fn a_mark_for_another_file_is_ignored() {
    let (mut a, _) = app_with("open-at-mark", "a.txt", &numbered(300), TabStops::default());
    a.preview_goto_submit("50");
    a.tabs[a.active].jump.as_mut().unwrap().path = PathBuf::from("/elsewhere/b.txt");
    assert_eq!(a.viewed_pos(), Some(Pos::line(50)), "the top row, which is the same line here");
    a.tabs[a.active].preview_offset = 10;
    assert_eq!(a.viewed_pos(), Some(Pos::line(11)));
}

fn hit(rows: &[&str], tabs: TabStops, needle: &str) -> Pos {
    let rows: Vec<String> = rows.iter().map(|r| (*r).to_owned()).collect();
    let (mut a, _) = app_with("open-at-hit", "a.txt", &rows, tabs);
    a.preview_find_submit(needle);
    a.viewed_pos().unwrap()
}

#[test]
fn a_find_hit_gives_the_source_column() {
    let p = hit(&["  foo bar"], TabStops::default(), "bar");
    assert_eq!((p.line, p.col), (1, Some(Col::new(7, 7))));
    // A tab shows as four columns but is one character of the file.
    let tabs = TabStops::of("\tfoo bar", 4);
    let p = hit(&["    foo bar"], tabs, "bar");
    assert_eq!((p.line, p.col), (1, Some(Col::new(6, 6))));
    // Three bytes a character: `bar` is the fourth character but starts at the eighth byte.
    let p = hit(&["\u{65e5}\u{672c} bar"], TabStops::default(), "bar");
    assert_eq!((p.line, p.col), (1, Some(Col::new(4, 8))));
}

#[test]
fn the_editor_position_is_asked_for_a_file_only() {
    let (mut a, _) = app_with("open-at-prompt", "a.txt", &numbered(300), TabStops::default());
    a.max_preview = false;
    a.act(Act::OpenAt);
    match &a.overlay {
        Overlay::Input(ov) => assert_eq!(ov.text, "", "nothing on show in the list"),
        _ => panic!("no prompt"),
    }
    a.overlay = Overlay::None;
    a.max_preview = true;
    a.tabs[a.active].preview_offset = 40;
    a.act(Act::OpenAt);
    match &a.overlay {
        Overlay::Input(ov) => assert_eq!(ov.text, "41", "T prefills the line on screen"),
        _ => panic!("no prompt"),
    }

    let dir = crate::util::test_dir("open-at-dir");
    std::fs::create_dir_all(dir.join("sub")).unwrap();
    let mut b = App::new(Config::load(), dir.clone(), egui::Context::default());
    b.tabs[b.active].current = Folder::from_entries(dir.clone(), Arc::new(vec![Entry::from_path(dir.join("sub")).unwrap()]), false);
    b.act(Act::OpenAt);
    assert!(!matches!(b.overlay, Overlay::Input(_)));
    assert_eq!(b.toasts.last().map(|t| t.text.as_str()), Some("Open at line works on a file"));
}

#[test]
fn a_submit_after_the_cursor_moved_is_refused() {
    let (mut a, _) = app_with("open-at-changed", "a.txt", &numbered(5), TabStops::default());
    a.open_at_submit(Path::new("/elsewhere/b.txt"), "3");
    assert_eq!(a.toasts.last().map(|t| t.text.as_str()), Some("Open at line: the file under the cursor changed"));
    a.open_at_submit(Path::new("/elsewhere/b.txt"), "zero");
    assert_eq!(a.toasts.last().map(|t| t.text.as_str()), Some("Open at line: enter a line number, or line:column"));
}

#[test]
fn the_keys_reach_the_command() {
    let (km, warnings) = Keymap::load(&[]);
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(bound(&km.mgr, "<C-S-g>"), vec![Act::OpenAt]);
    assert!(km.preview.iter().all(|b| !b.run.contains(&Act::OpenAt)), "[preview] does not bind it");
    let (_, _, runs) = crate::app::overlay::palette_items(&km.mgr, &[]);
    assert_eq!(runs.iter().filter(|r| r.contains(&Act::OpenAt)).count(), 1);
}

#[test]
fn the_editor_gets_the_line_and_column_its_template_asks_for() {
    let paths = vec![PathBuf::from("/d/a.txt")];
    let at = Pos { line: 12, col: Some(Col::new(5, 7)) };
    let mut custom = crate::exec::LineArgs::new();
    custom.insert(crate::exec::editor_key("myedit"), "{path}:{line}:{col}".to_owned());
    assert!(crate::exec::command_line("myedit", &paths, Some(at), &custom).contains(":12:5"));
    custom.insert(crate::exec::editor_key("myedit"), "{path}:{line}:{bcol}".to_owned());
    assert!(crate::exec::command_line("myedit", &paths, Some(at), &custom).contains(":12:7"));
    let none = crate::exec::LineArgs::new();
    assert!(crate::exec::command_line("vi", &paths, Some(at), &none).contains("12"));
    assert!(!crate::exec::command_line("unknown-viewer", &paths, Some(at), &none).contains("12"));
}

#[test]
fn quick_look_finds_in_the_text() {
    let rows = vec!["alpha".to_owned(), "beta".to_owned(), "gamma".to_owned(), "beta again".to_owned()];
    let (mut a, _) = app_with("open-at-f3find", "a.txt", &rows, TabStops::default());
    a.max_preview = false;
    a.quick = true;
    a.act(Act::Find { prev: false, smart: false, insensitive: false });
    assert!(matches!(&a.overlay, Overlay::Input(ov) if matches!(ov.kind, InputKind::PreviewFind)));
    a.overlay = Overlay::None;
    a.preview_find_submit("beta");
    assert_eq!(a.tabs[a.active].find_at, Some(1));
    a.find_arrow(false);
    assert_eq!(a.tabs[a.active].find_at, Some(3), "`n` steps the text");
    a.find_arrow(true);
    assert_eq!(a.tabs[a.active].find_at, Some(1));
}

#[test]
fn the_list_find_still_opens_without_quick_look() {
    let (mut a, _) = app_with("open-at-listfind", "a.txt", &numbered(3), TabStops::default());
    a.max_preview = false;
    a.act(Act::Find { prev: false, smart: false, insensitive: false });
    assert!(matches!(&a.overlay, Overlay::Input(ov) if matches!(ov.kind, InputKind::Find { .. })));
}
