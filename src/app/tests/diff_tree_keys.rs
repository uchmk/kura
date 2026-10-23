//! A tree comparison reuses `Overlay::Diff` and the `[diff]` keymap, so the same
//! keys have to mean the right thing for a list with a cursor rather than for two
//! columns of lines.

use super::*;
use crate::diff::{Outcome, TreeRow, TreeState};

fn row(rel: &str, state: TreeState) -> TreeRow {
    TreeRow { rel: PathBuf::from(rel), state, dir: false, left: 0, right: 0 }
}

fn app_with(rows: Vec<TreeRow>) -> App {
    let mut a = App::new(Config::load(), std::env::temp_dir(), egui::Context::default());
    a.overlay = Overlay::Diff(DiffOverlay {
        left: PathBuf::from("l"),
        right: PathBuf::from("r"),
        outcome: Some(Outcome::Tree { counts: crate::diff::TreeCounts::of(&rows), rows, truncated: false }),
        offset: 0,
        rows: 10,
        cursor: 0,
        hide_same: false,
        back: None,
    });
    a
}

fn cursor(a: &App) -> usize {
    match &a.overlay {
        Overlay::Diff(ov) => ov.cursor,
        _ => panic!("the overlay closed"),
    }
}

/// `C` copies the rows on screen, one `state<TAB>path` line each, and
/// leaves out the matches `z` hid (Q91).
#[test]
fn c_copies_the_rows_on_screen() {
    let km = &Config::load().keymap;
    let c = km.diff.iter().find(|b| crate::config::keys::render_seq(&b.on) == "C");
    assert_eq!(c.map(|b| b.run.clone()), Some(vec![Act::Copy(CopyWhat::All)]), "C is bound in [diff]");
    let mut a = app_with(vec![row("a", TreeState::Same), row("b", TreeState::LeftOnly), row("c", TreeState::Differ)]);
    a.diff_act(Act::Copy(CopyWhat::All));
    assert_eq!(exec::get_clipboard().unwrap(), "same\ta\nleft only\tb\ndiffer\tc");
    a.diff_act(Act::HideSame);
    a.diff_act(Act::Copy(CopyWhat::All));
    assert_eq!(exec::get_clipboard().unwrap(), "left only\tb\ndiffer\tc");
    assert!(matches!(a.overlay, Overlay::Diff(_)), "copying leaves the comparison open");
}

/// A tree opens on its first difference, and on its top row when there is
/// none (Q23).
#[test]
fn a_tree_opens_on_its_first_difference() {
    let rows = vec![row("a", TreeState::Same), row("b", TreeState::Same), row("c", TreeState::Differ)];
    let mut ov = DiffOverlay {
        left: PathBuf::from("l"),
        right: PathBuf::from("r"),
        outcome: None,
        offset: 0,
        rows: 10,
        cursor: 0,
        hide_same: false,
        back: None,
    };
    let counts = crate::diff::TreeCounts::of(&rows);
    ov.arrive(Outcome::Tree { rows, counts, truncated: false });
    assert_eq!(ov.cursor, 2, "on `c`, the one difference");

    let same = vec![row("a", TreeState::Same), row("b", TreeState::Same)];
    let counts = crate::diff::TreeCounts::of(&same);
    ov.arrive(Outcome::Tree { rows: same, counts, truncated: false });
    assert_eq!(ov.cursor, 0, "nothing differs, so the top");
}

/// `z` hides the matches and brings them back, keeping the cursor on the
/// row it was on -- or, when that row is a match going out of sight, on the
/// next one that stays. `j` and `n` then walk the rows that are shown (Q23).
#[test]
fn z_hides_the_matching_rows_and_the_cursor_stays_put() {
    let mut a = app_with(vec![
        row("a", TreeState::Same),
        row("b", TreeState::Differ),
        row("c", TreeState::Same),
        row("d", TreeState::LeftOnly),
        row("e", TreeState::Same),
    ]);
    let shown = |a: &App| match &a.overlay {
        Overlay::Diff(ov) => ov.shown(),
        _ => panic!("the overlay closed"),
    };
    // On `c`, a match.
    a.diff_act(Act::Arrow(crate::config::cmd::Step::Rel(2)));
    a.diff_act(Act::HideSame);
    assert_eq!(shown(&a), vec![1, 3], "only `b` and `d`");
    assert_eq!(shown(&a)[cursor(&a)], 3, "`c` went, so the cursor is on `d`, the next that stayed");

    a.diff_act(Act::Arrow(crate::config::cmd::Step::Rel(-1)));
    assert_eq!(shown(&a)[cursor(&a)], 1, "`k` steps over the hidden `c` onto `b`");

    a.diff_act(Act::HideSame);
    assert_eq!(shown(&a).len(), 5, "all back");
    assert_eq!(shown(&a)[cursor(&a)], 1, "still on `b`");
}

/// `j`/`k` move the selection, not the scroll -- the rows are things you pick,
/// unlike the lines of a file comparison.
#[test]
fn the_arrows_move_the_cursor() {
    let mut a = app_with(vec![row("a", TreeState::Same), row("b", TreeState::Differ), row("c", TreeState::Same)]);
    a.diff_act(Act::Arrow(crate::config::cmd::Step::Rel(1)));
    assert_eq!(cursor(&a), 1);
    a.diff_act(Act::Arrow(crate::config::cmd::Step::Bot));
    assert_eq!(cursor(&a), 2);
    a.diff_act(Act::Arrow(crate::config::cmd::Step::Rel(1)));
    assert_eq!(cursor(&a), 2, "the last row is the last row");
    a.diff_act(Act::Arrow(crate::config::cmd::Step::Top));
    assert_eq!(cursor(&a), 0);
}

/// `n`/`N` walk to the next path that is not a match, skipping the rows there
/// is nothing to look at.
#[test]
fn n_walks_between_the_paths_that_are_not_matches() {
    let mut a = app_with(vec![
        row("a", TreeState::Same),
        row("b", TreeState::Same),
        row("c", TreeState::LeftOnly),
        row("d", TreeState::Same),
        row("e", TreeState::Differ),
    ]);
    a.diff_act(Act::FindArrow { prev: false });
    assert_eq!(cursor(&a), 2, "past the two matches");
    a.diff_act(Act::FindArrow { prev: false });
    assert_eq!(cursor(&a), 4);
    // Nothing further: the cursor stays and the view says so.
    a.diff_act(Act::FindArrow { prev: false });
    assert_eq!(cursor(&a), 4);
    a.diff_act(Act::FindArrow { prev: true });
    assert_eq!(cursor(&a), 2, "and back");
}

#[test]
fn close_still_closes() {
    let mut a = app_with(vec![row("a", TreeState::Same)]);
    a.diff_act(Act::Close);
    assert!(matches!(a.overlay, Overlay::None));
}

/// An empty comparison must not index anything.
#[test]
fn no_rows_is_not_a_panic() {
    let mut a = app_with(Vec::new());
    a.diff_act(Act::Arrow(crate::config::cmd::Step::Rel(1)));
    a.diff_act(Act::FindArrow { prev: false });
    assert_eq!(cursor(&a), 0);
}

/// `<Enter>` on a row that differs compares that pair of files, and `q`
/// goes back to the folders, on the same row.
#[test]
fn enter_opens_the_pair_and_q_comes_back() {
    let km = &Config::load().keymap;
    let enter = km.diff.iter().find(|b| crate::config::keys::render_seq(&b.on) == "<Enter>");
    assert_eq!(enter.map(|b| b.run.clone()), Some(vec![Act::Enter]), "<Enter> is bound in [diff]");

    let mut a = app_with(vec![row("a", TreeState::Same), row("sub/b.txt", TreeState::Differ)]);
    a.diff_act(Act::Arrow(Step::Rel(1)));
    a.diff_act(Act::Enter);
    match &a.overlay {
        Overlay::Diff(ov) => {
            assert_eq!((ov.left.as_path(), ov.right.as_path()), (Path::new("l/sub/b.txt"), Path::new("r/sub/b.txt")));
            assert!(ov.back.is_some(), "with the folders to go back to");
        }
        _ => panic!("the overlay closed"),
    }
    a.diff_act(Act::Close);
    assert_eq!(cursor(&a), 1, "back on the row it was opened from");
    assert!(matches!(&a.overlay, Overlay::Diff(ov) if matches!(ov.outcome, Some(Outcome::Tree { .. }))));
}

/// A row with nothing to pair says why and stays where it is.
#[test]
fn enter_on_a_one_sided_row_stays() {
    let mut a = app_with(vec![row("only-left.txt", TreeState::LeftOnly)]);
    a.diff_act(Act::Enter);
    assert!(matches!(&a.overlay, Overlay::Diff(ov) if ov.back.is_none() && ov.outcome.is_some()));
    assert!(a.toasts.iter().any(|t| t.text.contains("one side only")));
}
