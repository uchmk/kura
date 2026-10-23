/// #101: the toast names the shell whose profile the hook belongs in.
#[test]
fn it_names_the_shell() {
    let said = super::no_osc7("powershell (Windows PowerShell 5.1)", "kura");
    assert!(said.starts_with("`powershell (Windows PowerShell 5.1)` has not said where it is"), "{said}");
    assert!(said.contains("needs PowerShell 7"), "{said}");
    assert!(super::no_osc7("", "kura").starts_with("The shell has not said"));
}

/// Q50: the command that adds the hook, for the shell the pane runs.
#[test]
fn it_gives_the_command_for_that_shell() {
    let said = super::no_osc7("pwsh", "& 'C:\\x\\kura.exe'");
    assert!(said.contains("In that shell: & 'C:\\x\\kura.exe' shell-hook | Add-Content $PROFILE, then"), "{said}");
    assert!(super::no_osc7("bash", "kura").contains("kura shell-hook bash >> ~/.bashrc"));
    assert!(super::no_osc7("zsh", "kura").contains("kura shell-hook zsh >> ~/.zshrc"));
    assert!(super::no_osc7("cmd.exe", "kura").contains("hooks for pwsh, bash, zsh"));
    assert!(super::no_osc7("", "kura").contains("kura shell-hook | Add-Content $PROFILE"));
}
