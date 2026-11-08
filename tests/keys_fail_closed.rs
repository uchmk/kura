//! `kura --keys` with a step kura cannot carry out is refused before a window
//! opens, as failed (exit 3), with the report in `KURA_KEYS_DONE`.

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
fn an_assert_step_is_refused_as_failed() {
    let (code, said) = run("<Assert:foo>", "assert");
    assert_eq!(code, Some(3), "{said}");
    assert_eq!(said.lines().next(), Some("keys: failed"), "{said}");
    assert!(said.contains("step 1 of 1"), "{said}");
    assert_eq!(said.lines().rev().nth(1), Some("left: 1 not pressed"), "the run line is last: {said}");
}

#[test]
fn an_unsupported_step_in_the_middle_names_its_place() {
    let (code, said) = run("j<Click:Name>k", "clicktext");
    assert_eq!(code, Some(3), "{said}");
    assert_eq!(said.lines().next(), Some("keys: failed"), "{said}");
    assert!(said.contains("step 2 of 3"), "{said}");
    assert_eq!(said.lines().rev().nth(1), Some("left: 3 not pressed"), "the run line is last: {said}");
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
fn a_failed_report_carries_one_run_line_and_keeps_its_first_line() {
    let (code, said) = run_with_id("<Assert:foo>", "runid", Some("my-run-1"));
    assert_eq!(code, Some(3), "{said}");
    assert_eq!(said.lines().next(), Some("keys: failed"), "{said}");
    assert_eq!(run_lines(&said), ["run: my-run-1"], "{said}");
}

#[test]
fn a_run_id_is_made_when_none_is_given() {
    let (_, said) = run("<Assert:foo>", "runid-default");
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
