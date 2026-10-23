//! A change an AI client asked for through `kura mcp` waits for Allow (#315).

use super::*;
use crate::mcp::{Write, WriteTool};
use std::sync::atomic::{AtomicBool, Ordering};

fn app_in(dir: &Path) -> App {
    let mut a = App::new(Config::load(), dir.to_path_buf(), egui::Context::default());
    a.tabs[a.active].cwd = dir.to_path_buf();
    a
}

fn ask(a: &mut App, dir: &Path) -> (std::sync::mpsc::Receiver<crate::mcp::Reply>, Arc<AtomicBool>) {
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    let expired = Arc::new(AtomicBool::new(false));
    let w = Write { tool: WriteTool::Rename, paths: vec![dir.join("a.txt")], to: Some("b.txt".into()) };
    a.mcp_write(w, tx, expired.clone());
    (rx, expired)
}

fn files() -> (PathBuf, App) {
    let dir = crate::util::test_dir("mcp-writes");
    std::fs::write(dir.join("a.txt"), "x").unwrap();
    let a = app_in(&dir);
    (dir, a)
}

#[test]
fn nothing_changes_until_allow_and_enter_refuses() {
    let (dir, mut a) = files();
    let (rx, _) = ask(&mut a, &dir);
    let Overlay::Confirm(c) = &a.overlay else { panic!("no box") };
    assert_eq!(c.options.first().map(|o| o.0), Some('n'), "the default is Refuse");
    assert!(dir.join("a.txt").exists());
    a.answer_confirm('n');
    assert!(rx.recv().unwrap().is_err());
    assert!(dir.join("a.txt").exists() && !dir.join("b.txt").exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn allow_renames_and_can_be_undone() {
    let (dir, mut a) = files();
    let (rx, _) = ask(&mut a, &dir);
    a.answer_confirm('y');
    assert!(rx.recv().unwrap().is_ok());
    assert!(!dir.join("a.txt").exists() && dir.join("b.txt").exists());
    a.act(Act::Undo);
    assert!(dir.join("a.txt").exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_late_allow_does_nothing() {
    let (dir, mut a) = files();
    let (rx, expired) = ask(&mut a, &dir);
    expired.store(true, Ordering::Relaxed);
    a.answer_confirm('y');
    assert!(rx.recv().unwrap().is_err());
    assert!(dir.join("a.txt").exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_timed_out_box_closes_by_itself() {
    let (dir, mut a) = files();
    let (_rx, expired) = ask(&mut a, &dir);
    a.drop_expired_mcp_box();
    assert!(matches!(a.overlay, Overlay::Confirm(_)), "a live box stays");
    expired.store(true, Ordering::Relaxed);
    a.drop_expired_mcp_box();
    assert!(matches!(a.overlay, Overlay::None));
    assert!(dir.join("a.txt").exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_second_request_is_turned_away_while_a_box_is_open() {
    let (dir, mut a) = files();
    let _first = ask(&mut a, &dir);
    let (rx, _) = ask(&mut a, &dir);
    assert!(rx.recv().unwrap().unwrap_err().contains("another box"));
    let _ = std::fs::remove_dir_all(&dir);
}
