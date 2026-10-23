//! The second pane of the split view.

use super::*;

/// The second pane. The pane holding the keys always shows `App::active`, so
/// every action keeps working on one tab and never has to know about the split.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Split {
    /// The tab shown in the pane that does not have the keys.
    pub other: usize,
    /// The pane holding the keys is the right-hand one.
    pub right: bool,
}

/// Files being dragged from one pane towards the other.
///
/// It lives on `App` rather than in the list because the two panes are drawn
/// separately: the one the drag began in has no idea where it ends.
pub struct Drag {
    /// The tab the drag started in.
    pub from: usize,
    pub paths: Vec<PathBuf>,
    /// What to draw under the pointer while it is in flight.
    pub label: String,
}

/// Follow the other pane's tab index after the tab at `removed` is dropped.
/// `None` means that pane's own tab went away, so the split closes.
pub(super) fn split_after_remove(other: usize, removed: usize) -> Option<usize> {
    match other.cmp(&removed) {
        std::cmp::Ordering::Equal => None,
        std::cmp::Ordering::Less => Some(other),
        std::cmp::Ordering::Greater => Some(other - 1),
    }
}

/// Where `tab_create` opens, and whether the target still has to prove itself
/// a directory. A path someone typed gets the same benefit of the doubt a
/// typed `cd` gets — it may name a file, and the parent is then what was
/// meant. `hovered` is the entry under the cursor, which came from a listing.
pub(super) fn new_tab_target(
    base: &Path,
    current: bool,
    path: Option<&str>,
    hovered: Option<&Entry>,
    home: Option<PathBuf>,
) -> (PathBuf, bool) {
    match path {
        Some(p) if !p.is_empty() => (util::resolve_against(base, p), true),
        _ if current => match hovered {
            Some(e) if e.is_dir_like() => (e.path.clone(), false),
            _ => (base.to_path_buf(), false),
        },
        _ => (home.unwrap_or_else(|| base.to_path_buf()), false),
    }
}

/// Follow the other pane's tab index across `tabs.swap(a, b)`.
pub(super) fn split_after_swap(other: usize, a: usize, b: usize) -> usize {
    if other == a {
        b
    } else if other == b {
        a
    } else {
        other
    }
}
