//! #531: a content search's view keeps the match the reader is on when the same
//! file is read again (a theme switch, a resize of a pane-width layout), and
//! only a new file or a new search opens on the first match.

use super::*;
use crate::preview::Span;

/// `rows` lines of `x`, with `needle` on the rows in `at`.
fn lines(rows: usize, at: &[usize]) -> Payload {
    let lines =
        (0..rows).map(|i| vec![Span { text: if at.contains(&i) { "needle" } else { "x" }.to_owned(), ..Default::default() }]).collect();
    Payload::Text { lines, map: Vec::new(), extent: Default::default(), outline: Vec::new(), tabs: Default::default() }
}

/// A content search's result list of `names`, as `S` leaves it once the scan is done.
fn searched(label: &str, names: &[&str]) -> (App, PathBuf) {
    let dir = crate::util::test_dir(label);
    for n in names {
        std::fs::write(dir.join(n), "x").unwrap();
    }
    let mut a = App::new(Config::load(), dir.clone(), egui::Context::default());
    a.cfg.ui.preview_debounce_ms = 0;
    a.tabs[a.active].cwd = dir.clone();
    a.tabs[a.active].current = Folder::loading(dir.clone(), None);
    a.start_search("needle", SearchVia::Content);
    a.search = None;
    let entries = names.iter().map(|n| Entry::from_path(dir.join(n)).unwrap()).collect();
    let mut found = Folder::from_entries(dir.join("search"), Arc::new(entries), true);
    found.state = LoadState::Ready;
    a.tabs[a.active].current = found;
    assert!(a.in_search_view());
    (a, dir)
}

/// The worker's answer to the request in flight.
fn answer(a: &mut App, payload: Payload, partial: bool) {
    let key = a.preview.key.clone().unwrap();
    a.on_preview(preview::Response { key, payload, partial }, &egui::Context::default());
}

/// Asks for the hovered file and has the worker answer with `payload`.
fn land(a: &mut App, payload: Payload) {
    a.request_preview(false);
    assert!(a.preview.in_flight, "the file was asked for");
    answer(a, payload, false);
}

fn at(a: &App) -> Option<usize> {
    a.tabs[a.active].finder.as_ref().and_then(|f| f.at)
}

fn next(a: &mut App) {
    a.act(Act::FindArrow { prev: false });
}

/// Lands `a.txt` and walks to its third match (row 11).
fn on_third_match(label: &str) -> (App, PathBuf) {
    let (mut a, dir) = searched(label, &["a.txt", "b.txt"]);
    land(&mut a, lines(20, &[1, 6, 11, 16]));
    assert_eq!(at(&a), Some(1));
    next(&mut a);
    next(&mut a);
    assert_eq!(at(&a), Some(11));
    (a, dir)
}

#[test]
fn a_theme_switch_keeps_the_match_and_the_scroll() {
    let (mut a, dir) = on_third_match("keep-match-theme");
    a.tabs[a.active].preview_offset = 9;

    a.look_changed();
    a.request_preview(false);
    assert!(a.preview.in_flight, "the file is read again");
    assert!(a.preview.keep_match.is_some(), "with the match the reader is on");

    answer(&mut a, lines(20, &[1, 6, 11, 16]), false);
    assert_eq!(at(&a), Some(11));
    assert_eq!(a.tabs[a.active].preview_offset, 9, "the reader's scroll is left alone");
    assert!(a.preview.keep_match.is_none());
    next(&mut a);
    assert_eq!(at(&a), Some(16), "and `n` goes on from there");
    crate::util::remove_test_dir(&dir);
}

#[test]
fn a_relayout_keeps_the_nth_match_on_its_new_row() {
    let (mut a, dir) = on_third_match("keep-match-relayout");
    a.look_changed();
    a.request_preview(false);
    answer(&mut a, lines(24, &[2, 8, 14, 20]), false);
    assert_eq!(at(&a), Some(14));
    assert_eq!(a.tabs[a.active].preview_offset, 11);
    crate::util::remove_test_dir(&dir);

    // The early answer (every line, no colours) places it, and the whole one
    // that follows leaves the reader where they have got to.
    let (mut a, dir) = on_third_match("keep-match-partial");
    a.look_changed();
    a.request_preview(false);
    answer(&mut a, lines(24, &[2, 8, 14, 20]), true);
    assert_eq!(at(&a), Some(14));
    a.tabs[a.active].preview_offset = 5;
    answer(&mut a, lines(24, &[2, 8, 14, 20]), false);
    assert_eq!(at(&a), Some(14));
    assert_eq!(a.tabs[a.active].preview_offset, 5);
    crate::util::remove_test_dir(&dir);
}

#[test]
fn a_cache_hit_for_the_same_file_keeps_the_match() {
    let (mut a, dir) = on_third_match("keep-match-cache");
    let key = a.preview.key.clone().unwrap();
    a.preview.cache.put(key, CachedPreview { payload: lines(24, &[2, 8, 14, 20]), texture: None });
    a.request_preview(true);
    assert!(!a.preview.in_flight, "answered from the cache");
    assert_eq!(at(&a), Some(14));
    crate::util::remove_test_dir(&dir);
}

#[test]
fn a_new_file_or_a_new_search_still_opens_on_the_first_match() {
    let (mut a, dir) = on_third_match("keep-match-new");
    a.tabs[a.active].current.cursor = 1;
    a.request_preview(false);
    assert!(a.preview.keep_match.is_none(), "another file keeps nothing");
    answer(&mut a, lines(20, &[4, 9]), false);
    assert_eq!(at(&a), Some(4));
    crate::util::remove_test_dir(&dir);

    let (mut a, dir) = on_third_match("keep-match-query");
    a.look_changed();
    a.request_preview(false);
    assert!(a.preview.keep_match.is_some());
    a.tabs[a.active].finder.as_mut().unwrap().query = "other".into();
    answer(&mut a, lines(20, &[1, 6, 11, 16]), false);
    assert_eq!(at(&a), Some(1), "a new search starts at its first match");
    crate::util::remove_test_dir(&dir);
}
