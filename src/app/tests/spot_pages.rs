use super::page_for;

/// The three rows that name a place on GitHub all open it; the rest do not.
#[test]
fn each_git_row_opens_its_own_page() {
    let rows: Vec<(String, String)> = [
        ("Came in via", "#71  48b6c9c"),
        ("From branch", "claude/task-09i0cs"),
        (crate::spot::PR_ROW, "https://github.com/uchmk/kura/pull/71"),
        ("Subject", "x"),
    ]
    .iter()
    .map(|(k, v)| (k.to_string(), v.to_string()))
    .collect();
    assert_eq!(page_for(&rows, "Came in via").as_deref(), Some("https://github.com/uchmk/kura/pull/71"));
    assert_eq!(page_for(&rows, "From branch").as_deref(), Some("https://github.com/uchmk/kura/tree/claude/task-09i0cs"));
    assert_eq!(page_for(&rows, "Subject"), None);
    // No pull request, no repository to build a branch page from.
    assert_eq!(page_for(&rows[1..2], "From branch"), None);
}
