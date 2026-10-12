//! Give `kura.exe` its own icon and version information.
//!
//! This is the icon Explorer draws on the file, and the one a shortcut or a
//! pinned taskbar entry uses. It is a different thing from the window's icon,
//! which `main.rs` sets at runtime from the SVG: a resource is baked into the
//! binary and belongs to the file rather than to the running program.
//!
//! The `.res` is written in Rust by ito's `ito-winres`, so no `rc.exe` is
//! needed and a cross build to Windows from Linux or macOS gets it too. Only
//! for the MSVC targets.

fn main() {
    println!("cargo:rerun-if-changed=assets/icon.ico");
    ito_winres::emit_ico("kura", include_bytes!("assets/icon.ico"));
}
