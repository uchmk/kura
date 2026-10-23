use super::program_label;

#[test]
fn full_path_becomes_the_program_name() {
    assert_eq!(program_label("C:\\Program Files\\PowerShell\\7\\pwsh.exe"), "pwsh");
    assert_eq!(program_label("/usr/bin/zsh"), "zsh");
    assert_eq!(program_label("lazygit"), "lazygit");
    assert_eq!(program_label("vim notes/a.txt"), "vim notes/a.txt");
}
