//! `kura shell-hook`: the lines that make a shell report its directory
//! (OSC 7), so `<A-Up>` in the terminal pane can follow it, and mark where
//! each prompt starts (OSC 133;A) with how the command before it ended
//! (`D;<code>`), and where its output starts (`C`) or where the prompt ends
//! (`B`), for the prompt jumps and copying the last output. These are the
//! same marks as tsumugi's `shell-hook`, and the same lines: ito's
//! `ito_common::shellhook` writes both, with the app's name.
//!
//! They were only in the README, and the way to them was to find a 7-line
//! block there and copy it into a profile; one test run cut it out by line
//! number (#176, Q50). The README shows these same strings, and a test keeps
//! the two equal.

use ito_common::shellhook as common;
pub use ito_common::shellhook::SHELLS;

const APP: &str = "kura";
const PURPOSE: &str = "for the terminal pane to follow";

/// The hook for `shell`, by the name its program goes by. With none named,
/// PowerShell's: it is the pane's own default and the one shell that sends
/// nothing unless told to.
pub fn text(shell: Option<&str>) -> Result<String, String> {
    common::text(APP, PURPOSE, shell)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Q50: the README's blocks are this command's output, character for
    /// character, so the two cannot drift apart.
    #[test]
    fn the_readme_shows_the_same_hooks() {
        let readme = include_str!("../README.md");
        for (fence, shell) in [("powershell", "pwsh"), ("bash", "bash"), ("zsh", "zsh")] {
            let block = format!("```{fence}\n{}```", text(Some(shell)).unwrap());
            assert!(readme.contains(&block), "README lacks the {fence} hook as `kura shell-hook` prints it");
        }
    }

    #[test]
    fn the_hooks_are_kuras_and_the_rest_are_refused() {
        for shell in [None, Some("bash"), Some("zsh")] {
            let hook = text(shell).unwrap();
            assert!(hook.starts_with("\n# kura: say where the shell is") && !hook.contains("tsumugi"), "{hook:?}");
        }
        assert!(text(Some("powershell")).unwrap_err().contains("PowerShell 7"));
        assert!(text(Some("fish")).unwrap_err().contains(SHELLS));
    }
}
