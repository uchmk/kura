use super::*;

fn app() -> App {
    let ctx = egui::Context::default();
    App::new(Config::load(), std::env::temp_dir(), ctx)
}

/// A file path inside a directory shared by this test.
///
/// Deliberately **not** `util::test_dir`: this is called several times in one
/// test for several file names, and `test_dir` wipes what it hands back, so
/// the second call would delete the first file. The three tests here use
/// distinct names, so the process id is uniqueness enough. Named the way
/// `test_dir` names its own, so its sweep clears what a run leaves: as
/// `kura-move-undo-<pid>` one was left per run, 448 in the cloud
/// container by v0.77.0.
fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("kura-test-move-undo-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&d);
    d.join(name)
}

/// `x` then `p` is a move, and a move is undoable. It was not: only `d`,
/// `r` and `R` were, and the README's reason for leaving the rest out —
/// that undoing a copy would mean deleting files to tidy up — is about
/// copies. Putting a moved file back deletes nothing.
#[test]
fn a_move_goes_back_where_it_came_from() {
    let from = tmp("origin.txt");
    let to = tmp("landed.txt");
    let _ = std::fs::remove_file(&from);
    std::fs::write(&to, b"x").unwrap();

    let mut a = app();
    a.undos.land(UndoStep::Move { pairs: vec![(from.clone(), to.clone())] }, Land::Fresh);
    a.undo_step();

    assert!(from.exists(), "the file must be back at its original path");
    assert!(!to.exists(), "and gone from where it was moved to");
    let _ = std::fs::remove_file(&from);
}

/// The pair that matters. A paste onto a taken name lands as `_1`, and the
/// undo has to start from where the file actually is — an undo built from
/// the name that was *asked* for would go looking for a file that was never
/// created.
#[test]
fn it_starts_from_where_the_file_actually_landed() {
    let from = tmp("report.txt");
    let asked = tmp("dest.txt");
    let landed = tmp("dest_1.txt");
    let _ = std::fs::remove_file(&from);
    std::fs::write(&asked, b"in the way").unwrap();
    std::fs::write(&landed, b"the moved one").unwrap();

    let mut a = app();
    a.undos.land(UndoStep::Move { pairs: vec![(from.clone(), landed.clone())] }, Land::Fresh);
    a.undo_step();

    assert!(from.exists(), "back at the original name");
    assert!(!landed.exists(), "no longer at the conflict-resolved name");
    assert!(asked.exists(), "the file that was in the way is untouched");
    for p in [&from, &asked] {
        let _ = std::fs::remove_file(p);
    }
}

/// And `U` puts it back again.
#[test]
fn redo_moves_it_forward_again() {
    let from = tmp("there.txt");
    let to = tmp("here.txt");
    let _ = std::fs::remove_file(&to);
    std::fs::write(&from, b"x").unwrap();

    let mut a = app();
    a.undos.land(UndoStep::Move { pairs: vec![(from.clone(), to.clone())] }, Land::Undone);
    a.redo_step();

    assert!(to.exists() && !from.exists());
    let _ = std::fs::remove_file(&to);
}
