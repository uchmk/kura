//! What the last run actually used, written down so `kura env` can say it.
//!
//! The record, its file and its shared fields live in ito
//! (`ito_common::runinfo`): which GPU adapter and backend egui ended up on,
//! the font files loaded and the window as laid out are only knowable with a
//! window open, so the run that knows writes them down and `kura env` (which
//! exits before any window) reads them back. It reports the *last* run, which
//! is the right one: the run that went wrong, not the one typing `kura env`.
//!
//! What only kura knows (the terminal pane's grid and shell, the command lines
//! it launched) is kept under the record's `extra` table; the accessors below
//! read and write it. A record from before this move had those at the top
//! level and they are not read: the next run writes them again.

pub use ito_common::runinfo::RunInfo;
use ito_common::runinfo::{push_recent, FILE};
use std::time::SystemTime;
use toml::Value;

/// How many launches `kura env` shows.
pub const LAUNCHES_KEPT: usize = 5;

/// kura's own part of the record.
pub trait KuraRun {
    /// The terminal pane's grid as `[lines, columns]`, the last time the run
    /// drew it; `[0, 0]` if the pane was never opened.
    fn pane(&self) -> [usize; 2];
    fn set_pane(&mut self, grid: [usize; 2]);
    /// The last command lines `<Enter>` or `:` / `!` ran, newest last.
    fn launched(&self) -> Vec<String>;
    fn push_launch(&mut self, line: &str);
    /// The shell the pane started in that run; empty when not opened.
    fn pane_shell(&self) -> &str;
    fn set_pane_shell(&mut self, shell: &str);
    /// Why the pane's shell did not start; empty otherwise.
    fn pane_failed(&self) -> &str;
    fn set_pane_failed(&mut self, why: &str);
    /// `2026-10-04 08:59 (12m00s ago)`, or nothing for a record without a start.
    fn started_line(&self, now: SystemTime) -> Option<String>;
}

fn text<'a>(info: &'a RunInfo, key: &str) -> &'a str {
    info.extra.get(key).and_then(Value::as_str).unwrap_or_default()
}

impl KuraRun for RunInfo {
    fn pane(&self) -> [usize; 2] {
        let n = |i: usize| self.extra.get("pane").and_then(Value::as_array).and_then(|a| a.get(i)).and_then(Value::as_integer);
        [n(0).unwrap_or(0).max(0) as usize, n(1).unwrap_or(0).max(0) as usize]
    }

    fn set_pane(&mut self, grid: [usize; 2]) {
        self.extra.insert("pane".into(), Value::Array(grid.iter().map(|n| Value::Integer(*n as i64)).collect()));
    }

    fn launched(&self) -> Vec<String> {
        let list = self.extra.get("launched").and_then(Value::as_array);
        list.map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_owned)).collect()).unwrap_or_default()
    }

    fn push_launch(&mut self, line: &str) {
        let mut list = self.launched();
        push_recent(&mut list, line, LAUNCHES_KEPT);
        self.extra.insert("launched".into(), Value::Array(list.into_iter().map(Value::String).collect()));
    }

    fn pane_shell(&self) -> &str {
        text(self, "pane_shell")
    }

    fn set_pane_shell(&mut self, shell: &str) {
        self.extra.insert("pane_shell".into(), shell.into());
    }

    fn pane_failed(&self) -> &str {
        text(self, "pane_failed")
    }

    fn set_pane_failed(&mut self, why: &str) {
        self.extra.insert("pane_failed".into(), why.into());
    }

    fn started_line(&self, now: SystemTime) -> Option<String> {
        let ago = self.started_ago(now)?;
        let at = std::time::UNIX_EPOCH + std::time::Duration::from_secs(self.started);
        Some(format!("{} ({} ago)", crate::util::fmt_time(Some(at), "%Y-%m-%d %H:%M"), crate::util::fmt_duration(ago)))
    }
}

/// Record a launch in `last-run.toml`, off the UI thread.
pub fn remember_launch(line: &str) {
    // A test launching something must not write into the real state folder.
    if cfg!(test) {
        return;
    }
    let line = line.to_owned();
    std::thread::spawn(move || {
        let mut info = load().unwrap_or_default();
        info.push_launch(&line);
        save(&info);
    });
}

/// Record the shell the pane just started.
pub fn remember_shell(label: &str) {
    if cfg!(test) {
        return;
    }
    let label = label.to_owned();
    std::thread::spawn(move || {
        let mut info = load().unwrap_or_default();
        info.set_pane_shell(&label);
        info.set_pane_failed("");
        save(&info);
    });
}

/// Record why the pane's shell did not start.
pub fn remember_shell_failed(why: &str) {
    if cfg!(test) {
        return;
    }
    let why = why.to_owned();
    std::thread::spawn(move || {
        let mut info = load().unwrap_or_default();
        info.set_pane_failed(&why);
        save(&info);
    });
}

fn path() -> std::path::PathBuf {
    crate::config::Config::state_dir().join(FILE)
}

/// Write it, best effort: a diagnostic that could stop a program from starting
/// would be worse than no diagnostic.
pub fn save(info: &RunInfo) {
    ito_common::runinfo::save_to(&path(), info);
}

/// What the last run wrote, or nothing when kura has not run on this machine
/// — which is itself worth printing rather than papering over.
pub fn load() -> Option<RunInfo> {
    ito_common::runinfo::load_from(&path())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// #244: the record says when its run started, and how long ago; one
    /// from before the field reads as not recorded.
    #[test]
    fn the_record_says_when_its_run_started() {
        let at = std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_800_000_000);
        let info = RunInfo { started: 1_800_000_000, ..Default::default() };
        let line = info.started_line(at + std::time::Duration::from_secs(750)).unwrap();
        assert!(line.ends_with(" (12m30s ago)"), "{line}");
        assert_eq!(line.len(), "2027-01-15 08:00".len() + " (12m30s ago)".len(), "{line}");
        assert_eq!(RunInfo::default().started_line(at), None);
    }

    /// Q40: the last five launches are kept, newest last.
    #[test]
    fn the_last_five_launches_are_kept() {
        let mut info = RunInfo::default();
        for i in 1..=7 {
            info.push_launch(&format!("run {i}"));
        }
        assert_eq!(info.launched(), ["run 3", "run 4", "run 5", "run 6", "run 7"]);
    }

    /// Records written by older kuras still load: before `pane` existed, and
    /// with the pane fields at the top level (they read as not recorded).
    #[test]
    fn an_older_record_still_loads() {
        let old = "version = \"0.58.0\"\nadapter = \"\"\nbackend = \"\"\ndevice = \"\"\nfonts = []\nbold = []\nwindow_pt = [1.0, 1.0]\nppp = 1.0\n";
        let info: RunInfo = toml::from_str(old).expect("loads");
        assert_eq!((info.pane(), info.started), ([0, 0], 0));
        let old = format!("{old}pane = [12, 159]\nlaunched = [\"a\"]\npane_shell = \"pwsh\"\n");
        let info: RunInfo = toml::from_str(&old).expect("loads");
        assert_eq!((info.pane(), info.launched().len(), info.pane_shell()), ([0, 0], 0, ""));
        let bare: RunInfo = toml::from_str("version = \"0.1.0\"\n").expect("every field has a default");
        assert_eq!(bare.version, "0.1.0");
    }

    /// It survives the round trip, kura's own part included, and an absent one
    /// is absent rather than empty: "kura has never opened a window here" and
    /// "it opened one and found no fonts" are different answers.
    #[test]
    fn it_is_written_and_read_back() {
        let dir = crate::util::test_dir("runinfo");
        let p = dir.join(FILE);

        assert_eq!(ito_common::runinfo::load_from(&p), None, "nothing written yet");

        let mut info = RunInfo {
            version: "9.9.9".into(),
            adapter: "NVIDIA GeForce RTX 4070".into(),
            backend: "Vulkan".into(),
            device: "DiscreteGpu".into(),
            fonts: vec![std::path::PathBuf::from(r"C:\fonts\HackGen35ConsoleNF-Regular.ttf")],
            window_pt: [1360.0, 860.0],
            ppp: 1.5,
            started: 1_800_000_000,
            backend_setting: "[ui] backend = \"gl\"".into(),
            ..Default::default()
        };
        info.set_pane([12, 159]);
        info.push_launch("code -g a.txt:3");
        info.set_pane_shell("powershell (Windows PowerShell 5.1)");
        info.set_pane_failed("`nosuch` was not found on PATH");
        ito_common::runinfo::save_to(&p, &info);
        let back = ito_common::runinfo::load_from(&p).unwrap();
        assert_eq!(back, info);
        assert_eq!(
            (back.pane(), back.launched(), back.pane_shell()),
            ([12, 159], vec!["code -g a.txt:3".to_owned()], "powershell (Windows PowerShell 5.1)")
        );
        // A backslashed Windows path has to come back as it went in.
        assert_eq!(back.fonts[0], info.fonts[0]);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
