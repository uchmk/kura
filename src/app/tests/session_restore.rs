//! The tabs open when kura closed come back when it starts with no path.

use super::*;

fn app(dir: &str) -> App {
    App::new(Config::load(), PathBuf::from(dir), egui::Context::default())
}

#[test]
fn every_tab_and_the_one_in_front_come_back() {
    let mut a = app("/one");
    a.tabs.push(Tab::new(PathBuf::from("/two"), a.tabs[0].sort, true, a.tabs[0].linemode));
    a.tabs.push(Tab::new(PathBuf::from("/three"), a.tabs[0].sort, true, a.tabs[0].linemode));
    a.active = 1;
    let text = a.session_text().expect("the session serializes");

    let mut b = app("/home");
    assert!(b.restore_session(&text, Path::new("/home"), Restore::Tabs));
    let cwds: Vec<_> = b.tabs.iter().map(|t| t.cwd.clone()).collect();
    assert_eq!(cwds, [PathBuf::from("/one"), PathBuf::from("/two"), PathBuf::from("/three")]);
    assert_eq!(b.active, 1);
    assert!(b.tabs[0].pending_cd.is_some(), "a folder that has gone falls back to home instead of hanging");
}

#[test]
fn a_file_that_is_not_a_session_changes_nothing() {
    let mut a = app("/home");
    assert!(!a.restore_session("tabs = 3", Path::new("/home"), Restore::Tabs));
    assert!(!a.restore_session("tabs = []", Path::new("/home"), Restore::Tabs));
    assert_eq!(a.tabs.len(), 1);
    assert_eq!(a.tabs[0].cwd, PathBuf::from("/home"));
}

#[test]
fn folder_mode_brings_back_only_the_front_tab() {
    let mut a = app("/one");
    a.tabs.push(Tab::new(PathBuf::from("/two"), a.tabs[0].sort, true, a.tabs[0].linemode));
    a.active = 1;
    let text = a.session_text().unwrap();

    let mut b = app("/home");
    assert!(b.restore_session(&text, Path::new("/home"), Restore::Folder));
    assert_eq!(b.tabs.len(), 1);
    assert_eq!(b.tabs[0].cwd, PathBuf::from("/two"));
    assert_eq!(b.active, 0);
}

#[test]
fn off_restores_nothing() {
    let mut a = app("/one");
    let text = a.session_text().unwrap();
    assert!(!a.restore_session(&text, Path::new("/home"), Restore::Off));
    let mut ui = crate::config::Ui::default();
    assert_eq!(ui.restore_mode(), Restore::Tabs);
    ui.restore = "Off".into();
    assert_eq!(ui.restore_mode(), Restore::Off);
    ui.restore = "folder".into();
    assert_eq!(ui.restore_mode(), Restore::Folder);
    ui.restore = "nonsense".into();
    assert_eq!(ui.restore_mode(), Restore::Tabs);
}
