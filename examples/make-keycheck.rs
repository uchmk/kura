//! Write `TESTING-KEYS.md`: one tickable line per binding of the default
//! keymap, ticks carried over by layer and key.
//!
//! The generator is ito's `ito-testcheck` (shared with tsumugi); this is
//! kura's configuration of it: the paths, the preamble and a line per layer.
//!
//!     cargo run --example make-keycheck [-- --check | --stats]
//!
//! `--check` writes nothing and exits 1 when the file no longer matches the
//! keymap (what CI runs); `--stats` prints the counts, which are kept out of
//! the file so that two pull requests ticking keys do not conflict.

use ito_testcheck::keycheck::{run, Config};

fn main() {
    run(&Config {
        keymap: "src/config/defaults/keymap.toml",
        out: "TESTING-KEYS.md",
        command: "cargo run --example make-keycheck",
        preamble,
        layer_notes: &[
            ("mgr", "The file list: what is in front of you unless an overlay is."),
            ("input", "The one-line prompt — `cd`, rename, filter, search."),
            ("spot", "The details panel (`<Tab>`)."),
            ("term", "While the terminal pane holds the keys. Everything not listed here goes to the shell."),
            ("diff", "The side-by-side comparison (`<A-d>`)."),
            ("tasks", "The task manager (`w`)."),
            ("pick", "A chooser — the command palette, the context menu."),
            ("confirm", "A yes/no prompt."),
            ("help", "This panel (`~` or `<F1>`)."),
        ],
        marks: &[],
    });
}

fn preamble(n: usize) -> String {
    format!(
        "# Key checklist\n\n\
         Generated from `src/config/defaults/keymap.toml` by `cargo run --example make-keycheck`. Ticks are kept\n\
         across regenerations, so re-running after a keymap change costs nothing already done.\n\
         Edit the ticks, not the rows: anything else here is overwritten.\n\n\
         {n} keys. How many are checked is not written here, so that two pull requests ticking\n\
         keys do not conflict over a total: `cargo run --example make-keycheck -- --stats`.\n\n\
         A key is checked when it did what the description says _and_ did nothing else —\n\
         `<A-m>` once ran its own command and the unmodified `m` as well, and both halves\n\
         looked correct on their own. Anything surprising goes in an issue (`<F12>`).\n"
    )
}
