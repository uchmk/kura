//! `kura shell-hook`: the lines that make a shell report its directory
//! (OSC 7), so `<A-Up>` in the terminal pane can follow it, and mark where
//! each prompt starts (OSC 133;A) with how the command before it ended
//! (`D;<code>`), and where its output starts (`C`) or where the prompt ends
//! (`B`), for the prompt jumps and copying the last output. These are the
//! same marks as tsumugi's `shell-hook`.
//!
//! They were only in the README, and the way to them was to find a 7-line
//! block there and copy it into a profile; one test run cut it out by line
//! number (#176, Q50). The README shows these same strings, and a test keeps
//! the two equal.
//!
//! Each starts with an empty line, so appending it to a file that does not
//! end in a newline does not glue it onto that file's last line.

/// PowerShell 7. Runs on each `cd` rather than replacing `prompt`, which
/// would break Starship and the other prompt generators, and calls whatever
/// handler was there before, which `mise activate pwsh` puts there.
pub const PWSH: &str = r#"
# kura: report the directory to kura's terminal pane (OSC 7), and mark each
# prompt, where it ends and how the command before it ended (OSC 133)
$prev = $ExecutionContext.SessionState.InvokeCommand.LocationChangedAction
$ExecutionContext.SessionState.InvokeCommand.LocationChangedAction = {
    param($sender, $e)
    if ($prev) { $prev.Invoke($sender, $e) }
    $p = $e.NewPath.ProviderPath -replace '\\', '/' -replace '^(?!/)', '/'
    [Console]::Write("$([char]27)]7;file://$p$([char]27)\")
}.GetNewClosure()
if (-not $global:__kura_marks_input) {
    $global:__kura_marks_input = $true
    if ("$function:prompt" -notmatch '__kura_prompt') { $global:__kura_prompt = $function:prompt }
    function global:prompt {
        $c = if ($?) { 0 } elseif ($LASTEXITCODE) { $LASTEXITCODE } else { 1 }
        "$([char]27)]133;D;$c$([char]27)\$([char]27)]133;A$([char]27)\" + (& $global:__kura_prompt) + "$([char]27)]133;B$([char]27)\"
    }
}
"#;

/// bash has no hook on `cd`, so this says it at every prompt; kura only acts
/// on a directory that differs from the one it already has.
pub const BASH: &str = r#"
# kura: report the directory to kura's terminal pane (OSC 7), and mark each
# prompt, each command's output and how it ended (OSC 133)
__kura_osc7() { printf '\e]133;D;%s\e\\\e]7;file://%s%s\e\\\e]133;A\e\\' "$?" "$HOSTNAME" "$PWD"; }
PROMPT_COMMAND="__kura_osc7${PROMPT_COMMAND:+;$PROMPT_COMMAND}"
PS0="${PS0}\e]133;C\a"
"#;

/// zsh has one: `chpwd`. It does not fire for the directory the shell starts
/// in, hence the call at the end.
pub const ZSH: &str = r#"
# kura: report the directory to kura's terminal pane (OSC 7), and mark each
# prompt, each command's output and how it ended (OSC 133)
__kura_osc7() { printf '\e]7;file://%s%s\e\\' "$HOST" "$PWD" }
__kura_mark() { printf '\e]133;D;%s\e\\\e]133;A\e\\' "$?" }
__kura_run() { printf '\e]133;C\e\\' }
autoload -Uz add-zsh-hook
add-zsh-hook chpwd __kura_osc7
add-zsh-hook precmd __kura_mark
add-zsh-hook preexec __kura_run
__kura_osc7
"#;

/// The shells there is a hook for, as `shell-hook` takes them.
pub const SHELLS: &str = "pwsh, bash, zsh";

/// The hook for `shell`, by the name its program goes by. With none named,
/// PowerShell's: it is the pane's own default and the one shell that sends
/// nothing unless told to.
pub fn text(shell: Option<&str>) -> Result<&'static str, String> {
    match shell.map(str::to_ascii_lowercase).as_deref() {
        None | Some("pwsh" | "pwsh.exe") => Ok(PWSH),
        Some("bash" | "bash.exe") => Ok(BASH),
        Some("zsh") => Ok(ZSH),
        Some("powershell" | "powershell.exe") => Err(
            "Windows PowerShell 5.1 has no LocationChangedAction; the hook needs PowerShell 7 (pwsh)".into(),
        ),
        Some(other) => Err(format!("no hook for {other:?} (one of: {SHELLS})")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Q50: the README's blocks are this command's output, character for
    /// character, so the two cannot drift apart.
    #[test]
    fn the_readme_shows_the_same_hooks() {
        let readme = include_str!("../README.md");
        for (fence, hook) in [("powershell", PWSH), ("bash", BASH), ("zsh", ZSH)] {
            let block = format!("```{fence}\n{hook}```");
            assert!(readme.contains(&block), "README lacks the {fence} hook as `kura shell-hook` prints it");
        }
    }

    #[test]
    fn each_shell_gets_its_own_and_the_rest_are_refused() {
        assert_eq!(text(None), Ok(PWSH));
        assert_eq!(text(Some("PWSH.EXE")), Ok(PWSH));
        assert_eq!(text(Some("bash")), Ok(BASH));
        assert_eq!(text(Some("zsh")), Ok(ZSH));
        assert!(text(Some("powershell")).unwrap_err().contains("PowerShell 7"));
        assert!(text(Some("fish")).unwrap_err().contains(SHELLS));
        for hook in [PWSH, BASH, ZSH] {
            assert!(hook.starts_with("\n#") && hook.ends_with('\n'), "{hook:?}");
            assert!(hook.contains("]7;file://"), "{hook:?}");
            // The prompt marks `<C-S-Up>` / `<C-S-l>` need, and how a command ended.
            assert!(hook.contains("]133;A") && hook.contains("133;D;"), "{hook:?}");
        }
    }
}
