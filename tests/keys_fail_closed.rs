//! `kura --keys` with a step kura cannot carry out is refused before a window
//! opens, as failed (exit 3), with the report in `KURA_KEYS_DONE`.

use std::path::PathBuf;
use std::process::Command;

fn run(keys: &str, label: &str) -> (Option<i32>, String) {
    let dir: PathBuf = std::env::temp_dir().join(format!("kura-keys-fail-closed-{}-{label}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let done = dir.join("done.txt");
    let out = Command::new(env!("CARGO_BIN_EXE_kura"))
        .args(["--keys", keys])
        .env("KURA_KEYS_DONE", &done)
        .env("UCHMK_CONFIG_DIR", dir.join("cfg"))
        .output()
        .unwrap();
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
    assert_eq!(said.lines().last(), Some("left: 1 not pressed"), "{said}");
}

#[test]
fn an_unsupported_step_in_the_middle_names_its_place() {
    let (code, said) = run("j<Click:Name>k", "clicktext");
    assert_eq!(code, Some(3), "{said}");
    assert_eq!(said.lines().next(), Some("keys: failed"), "{said}");
    assert!(said.contains("step 2 of 3"), "{said}");
    assert_eq!(said.lines().last(), Some("left: 3 not pressed"), "{said}");
}

#[test]
fn a_script_that_cannot_be_typed_is_still_refused() {
    let (code, said) = run("<Tab", "refused");
    assert_eq!(code, Some(2), "{said}");
    assert!(said.starts_with("keys: refused"), "{said}");
}
