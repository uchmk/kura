//! `kura.com`: the console-subsystem front for `kura.exe` (Q44).
//!
//! `kura.exe` is a windowed program, so PowerShell does not wait for it at
//! the end of a pipeline: `$v = & kura env` came back empty, as did
//! `& kura --version`, and the text arrived after the prompt (#160, #176,
//! #183). The release zip ships this binary beside `kura.exe` as `kura.com`.
//! `PATHEXT` lists `.COM` before `.EXE`, so typing `kura` finds this one
//! first. It is the shape of Visual Studio's `devenv.com` and `devenv.exe`.
//!
//! The front itself is ito's `ito_console::front` (v0.114.0), shared with
//! yagura, tsumugi, minato and michi; what is kura's is which command lines
//! answer in text. For those it runs `kura.exe` with the same arguments and
//! the same standard handles, waits, and passes on the exit code. Anything
//! else opens the window: it starts `kura.exe` and returns as soon as the
//! window is taking input, so that a command line `kura.exe` refuses before
//! opening a window (`--keys "<Tab"`, two paths) still prints its one line
//! here and passes on its exit code. The window does not hold this
//! console's pipe (#192): from a pipe, it writes into a file of its own in
//! the temporary folder, copied out here once it is up.
//!
//! It holds no logic of its own beyond that choice: the report, the help and
//! the hooks all come from `kura.exe`, so the two cannot disagree.

/// The arguments that make `kura.exe` print and exit rather than open a
/// window. Kept in step with `parse_cli` in `main.rs`.
/// `mcp` too: it talks on the standard handles until its client closes
/// them, and they are inherited, so `kura.com mcp` serves as `kura.exe mcp` does.
const ANSWERS: &[&str] = &["env", "--env", "--version", "-V", "--help", "-h", "shell-hook", "mcp"];

/// The options that take the next argument as their value, which is then
/// never a command: `--keys env` types three letters.
const TAKES_VALUE: &[&str] = &["--cwd-file", "--chooser-file", "--keys"];

/// Whether `kura.exe` will answer this command line in text and exit, read
/// the way `parse_cli` reads it: left to right, skipping option values.
fn answers_in_text(args: &[std::ffi::OsString]) -> bool {
    ito_console::any_of(args, ANSWERS, TAKES_VALUE)
}

fn main() {
    ito_console::front("kura", answers_in_text)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(args: &[&str]) -> Vec<std::ffi::OsString> {
        args.iter().map(Into::into).collect()
    }

    /// The text commands are waited for; anything that opens the window is not.
    #[test]
    fn it_waits_only_for_what_answers_in_text() {
        for waits in [
            &["env"][..],
            &["env", "--out", "r.txt"],
            &["--version"],
            &["-V"],
            &["--help"],
            &["shell-hook", "bash"],
            &["mcp"],
            &["C:\\dev", "env"],
            &["--keys", "j", "-V"],
        ] {
            assert!(answers_in_text(&line(waits)), "{waits:?}");
        }
        for opens in [
            &[][..],
            &["C:\\dev"],
            &["--keys", "<Tab>"],
            &["--cwd-file", "x"],
            &["environment"],
            &["--keys", "env"],
            &["--cwd-file", "--help"],
        ] {
            assert!(!answers_in_text(&line(opens)), "{opens:?}");
        }
    }
}
