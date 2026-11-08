//! `kura --keys` with a script that cannot be typed, or a bad run id, is refused
//! before a window opens (nothing here opens one), and every report in
//! `KURA_KEYS_DONE` carries one `run:` line.

use std::path::PathBuf;
use std::process::Command;

fn run(keys: &str, label: &str) -> (Option<i32>, String) {
    run_with_id(keys, label, None)
}

fn run_with_id(keys: &str, label: &str, run_id: Option<&str>) -> (Option<i32>, String) {
    let dir: PathBuf = std::env::temp_dir().join(format!("kura-keys-fail-closed-{}-{label}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let done = dir.join("done.txt");
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_kura"));
    cmd.args(["--keys", keys]).env("KURA_KEYS_DONE", &done).env("UCHMK_CONFIG_DIR", dir.join("cfg")).env_remove("KURA_KEYS_RUN_ID");
    if let Some(id) = run_id {
        cmd.env("KURA_KEYS_RUN_ID", id);
    }
    let out = cmd.output().unwrap();
    let said = std::fs::read_to_string(&done).unwrap_or_default();
    let _ = std::fs::remove_dir_all(&dir);
    (out.status.code(), said)
}

#[test]
fn a_script_that_cannot_be_typed_is_still_refused() {
    let (code, said) = run("<Tab", "refused");
    assert_eq!(code, Some(2), "{said}");
    assert!(said.starts_with("keys: refused"), "{said}");
}

fn run_lines(said: &str) -> Vec<&str> {
    said.lines().filter(|l| l.starts_with("run: ")).collect()
}

#[test]
fn a_run_id_is_made_when_none_is_given() {
    let (_, said) = run("<Tab", "runid-default");
    let lines = run_lines(&said);
    assert_eq!(lines.len(), 1, "{said}");
    assert!(lines[0].len() > "run: ".len() && !lines[0].contains("(none)"), "{said}");
}

#[test]
fn a_parse_refusal_carries_the_run_id_too() {
    let (code, said) = run_with_id("<Tab", "runid-refused", Some("abc_1"));
    assert_eq!(code, Some(2), "{said}");
    assert!(said.starts_with("keys: refused"), "{said}");
    assert_eq!(run_lines(&said), ["run: abc_1"], "{said}");
}

#[test]
fn a_bad_run_id_is_refused_before_any_window() {
    let (code, said) = run_with_id("j", "runid-bad", Some("a b"));
    assert_eq!(code, Some(3), "{said}");
    assert!(said.starts_with("keys: refused"), "{said}");
    assert!(said.contains("KURA_KEYS_RUN_ID"), "{said}");
    assert_eq!(run_lines(&said).len(), 1, "{said}");
}
