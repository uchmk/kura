//! yazi's key notation: `a`, `Z`, `<C-a>`, `<Enter>`, `<F5>`, `<A-S-Up>` …
//!
//! The parsing, printing, the egui conversion and `display_seq` (how a reader
//! sees a key: `⌘Q` on macOS) live in the shared crate `ito-keys` (in
//! uchmk/ito, where this file started), so the apps spell keys the same way;
//! `C-` is Ctrl on Windows and Linux and Cmd on macOS. What a key does (the
//! layers in `keymap.rs`, the commands in `cmd.rs`) stays here.

pub use ito_keys::*;
