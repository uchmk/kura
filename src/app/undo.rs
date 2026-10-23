//! Undo and redo steps and the two stacks they land on.

use super::*;

/// A step that `u` can take back. Each variant holds enough to go either way,
/// so the same value moves between the two stacks as it is undone and redone.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum UndoStep {
    /// `from` became `to`.
    Rename { from: PathBuf, to: PathBuf },
    /// These paths, which were in `dir`, went to the trash.
    Trash { paths: Vec<PathBuf>, dir: PathBuf },
    /// A bulk rename: every `from` became its `to`. One step, so a single `u`
    /// takes the whole batch back.
    Bulk { pairs: Vec<(PathBuf, PathBuf)> },
    /// Files moved by `x` then `p`. The pairs come back from the worker rather
    /// than from what was asked for: a name already taken is resolved down
    /// there, so the file sent to `x.pdf` may have landed as `x_1.pdf`.
    ///
    /// A move is here while a copy is not, and the difference is real. Undoing
    /// a copy would mean deleting the new files to tidy up — worse to get wrong
    /// than the thing being undone. Undoing a move puts a file back where it
    /// came from, which is a rename across directories and deletes nothing.
    Move { pairs: Vec<(PathBuf, PathBuf)> },
    /// What `a` made: the folders it had to make on the way, outermost first,
    /// then the thing asked for -- a file when `file`, else a folder. Taken
    /// back only while that is still empty: a file someone has since written
    /// in is not a slip any more, and removing it would lose the writing.
    Create { paths: Vec<PathBuf>, file: bool },
    /// Links made by `-`, `_` or `=`. Taking one back removes the link
    /// and never what it points at.
    Link { links: Vec<ops::Link> },
}

impl UndoStep {
    /// What a toast says about the step once it has been taken back.
    pub(super) fn undone_label(&self) -> String {
        match self {
            Self::Rename { from, .. } => format!("Renamed back to {}", util::file_name(from)),
            Self::Trash { paths, .. } => match paths.len() {
                1 => format!("Restored {}", util::file_name(&paths[0])),
                n => format!("Restored {n} item(s)"),
            },
            Self::Bulk { pairs } => format!("Put {} name(s) back", pairs.len()),
            Self::Move { pairs } => match pairs.len() {
                1 => format!("Moved {} back", util::file_name(&pairs[0].0)),
                n => format!("Moved {n} item(s) back"),
            },
            Self::Create { paths, .. } => format!("Removed {}", created_name(paths)),
            Self::Link { links } => match links.len() {
                1 => format!("Removed the {} {}", link_word(links), util::file_name(&links[0].at)),
                n => format!("Removed {n} {}(s)", link_word(links)),
            },
        }
    }

    /// And once it has been done again.
    pub(super) fn redone_label(&self) -> String {
        match self {
            Self::Rename { to, .. } => format!("Renamed to {}", util::file_name(to)),
            Self::Trash { paths, .. } => match paths.len() {
                1 => format!("Trashed {}", util::file_name(&paths[0])),
                n => format!("Trashed {n} item(s)"),
            },
            Self::Bulk { pairs } => format!("Renamed {} file(s)", pairs.len()),
            Self::Move { pairs } => match pairs.len() {
                1 => format!("Moved {}", util::file_name(&pairs[0].0)),
                n => format!("Moved {n} item(s)"),
            },
            Self::Create { paths, .. } => format!("Created {}", created_name(paths)),
            Self::Link { links } if links.iter().all(|l| l.junction) => match links.len() {
                1 => format!("Made the junction {} again", util::file_name(&links[0].at)),
                n => format!("Made {n} junctions again"),
            },
            Self::Link { links } => match links.len() {
                1 => format!("Linked {}", util::file_name(&links[0].at)),
                n => format!("Made {n} link(s)"),
            },
        }
    }
}

/// The `cd` prompt's starting text: the folder with this platform's separator
/// after it, ready for a name. It used to end in `\\` everywhere, so Linux
/// and macOS showed `/home/me\\` (found on the virtual display), and a root
/// that already ends in one (`/`, `C:\\`) is not given a second.
pub(super) fn dir_prefill(dir: &Path) -> String {
    let shown = dir.display().to_string();
    match shown.ends_with(['/', '\\']) {
        true => shown,
        false => format!("{shown}{}", std::path::MAIN_SEPARATOR),
    }
}

/// What a typed path that named nothing says when its parent is shown instead
/// (#98): one sentence, whichever of the two listings arrived first (#242).
/// It names the whole path that was missing, not just the last part (#241),
/// with the home folder as `~`. When the cursor went to the nearest name in
/// the folder, the toast says which (`(nearest: typo)`), so the jump is not a
/// mystery (#285).
pub(super) fn nothing_there(name: &str, shown: &Path, near: Option<&str>) -> String {
    let home = ito_common::home_dir();
    let mut said = format!("No such file or folder: {} — showing {}", shown.join(name).display(), shown.display());
    if let Some(near) = near {
        said.push_str(&format!(" (nearest: {near})"));
    }
    crate::bugreport::without_home(&said, home.as_deref())
}

/// What `u` calls the links it took back. A junction stays a junction in the
/// toast: `y` said `Made a junction`, and `u` then `U` used to say `link`, the
/// word for the symlink Windows had refused (#185).
pub(super) fn link_word(links: &[ops::Link]) -> &'static str {
    if links.iter().all(|l| l.junction) {
        "junction"
    } else {
        "link"
    }
}

/// The name a create is known by: the thing asked for, the last of its paths.
pub(super) fn created_name(paths: &[PathBuf]) -> String {
    paths.last().map(|p| util::file_name(p)).unwrap_or_default()
}

/// The paths `a` will have to make for `target`: it and every parent that is
/// not there yet, outermost first. Empty when `target` is already there, so
/// that `a` on an existing folder -- which `create_dir_all` lets through --
/// leaves nothing for `u` to remove.
pub(super) fn paths_to_make(target: &Path) -> Vec<PathBuf> {
    let mut made: Vec<PathBuf> =
        target.ancestors().take_while(|p| !p.as_os_str().is_empty() && !ops::exists(p)).map(Path::to_path_buf).collect();
    made.reverse();
    made
}

/// Take back a create: the thing asked for goes only while it is still empty,
/// then each folder made on the way, as far as each is empty too. A parent
/// something else has been put into since stays, without an error: the slip
/// being undone is the one name, and that is gone.
pub(super) fn unmake(paths: &[PathBuf], file: bool) -> std::io::Result<usize> {
    let Some((leaf, parents)) = paths.split_last() else { return Ok(0) };
    if file {
        if std::fs::metadata(leaf)?.len() > 0 {
            return Err(std::io::Error::other(format!("{} has been written to since", util::file_name(leaf))));
        }
        std::fs::remove_file(leaf)?;
    } else {
        // `remove_dir` refuses a folder with anything in it, which is the
        // check itself.
        std::fs::remove_dir(leaf)?;
    }
    // A parent someone has since put something in stays, and so does every
    // folder above it; the count is of the folders that actually went.
    Ok(parents.iter().rev().take_while(|dir| std::fs::remove_dir(dir).is_ok()).count())
}

/// What a finished `E` or `e` says it made. An unpacked folder ends in a
/// separator, so it reads as a folder and not as one more file.
pub(super) fn made_says(kind: OpKind, made: &[PathBuf]) -> Option<String> {
    let folder = |p: &PathBuf| format!("{}{}", util::file_name(p), std::path::MAIN_SEPARATOR);
    match (kind, made) {
        (OpKind::Compress(_), [archive]) => Some(format!("Packed into {}", util::file_name(archive))),
        (OpKind::TakeOut, [one]) => Some(format!("Took {} out of the archive", util::file_name(one))),
        (OpKind::TakeOut, many) if !many.is_empty() => Some(format!("Took {} out of the archive", util::items(many.len()))),
        (OpKind::Extract, [one]) => Some(format!("Unpacked into {}", folder(one))),
        (OpKind::Extract, [first, rest @ ..]) => {
            Some(format!("Unpacked {} archives into {} and {} more", rest.len() + 1, folder(first), rest.len()))
        }
        _ => None,
    }
}

/// The toast for a create taken back. The folders made on the way go with it,
/// and the toast counts them, so that `u` after `a new/deep/note.txt` does not
/// read as if only the file went.
pub(super) fn removed_label(step: &UndoStep, folders: usize) -> String {
    match folders {
        0 => step.undone_label(),
        n => format!("{} and {n} folder(s)", step.undone_label()),
    }
}

/// Make again what [`unmake`] took back.
pub(super) fn remake(paths: &[PathBuf], file: bool) -> std::io::Result<()> {
    let Some((leaf, parents)) = paths.split_last() else { return Ok(()) };
    for dir in parents {
        match std::fs::create_dir(dir) {
            Err(e) if e.kind() != std::io::ErrorKind::AlreadyExists => return Err(e),
            _ => {}
        }
    }
    match file {
        true => std::fs::OpenOptions::new().write(true).create_new(true).open(leaf).map(|_| ()),
        false => std::fs::create_dir(leaf),
    }
}

/// Run `f` over each link, splitting them into those it worked on and those
/// it did not, with the first error.
pub(super) fn each_link(
    links: Vec<ops::Link>,
    f: fn(&ops::Link) -> std::io::Result<()>,
) -> (Vec<ops::Link>, Vec<ops::Link>, Option<std::io::Error>) {
    let (mut done, mut left, mut first) = (Vec::new(), Vec::new(), None);
    for l in links {
        match f(&l) {
            Ok(()) => done.push(l),
            Err(e) => {
                first.get_or_insert(e);
                left.push(l);
            }
        }
    }
    (done, left, first)
}

/// Where a step belongs once the work behind it has succeeded.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Land {
    /// Something the person just did: `u` can take it back, and whatever was
    /// undone before is no longer on the way forward.
    Fresh,
    /// `u` took it back, so `U` can do it again.
    Undone,
    /// `U` did it again, so `u` can take it back again.
    Redone,
}

/// The two stacks behind `u` and `U`. Apart from `App` so that the rules about
/// which stack a step lands on can be tested without a window.
#[derive(Default, Debug)]
pub struct Undos {
    /// What `u` takes back, newest last.
    pub undo: Vec<UndoStep>,
    /// What `U` does again, newest last.
    pub redo: Vec<UndoStep>,
}

impl Undos {
    /// Older steps than this fall off the bottom. Undo is for the slip that was
    /// just made, not a journal of the session.
    pub(super) const MAX: usize = 50;

    pub(super) fn land(&mut self, step: UndoStep, how: Land) {
        match how {
            Land::Fresh => {
                // A new action forks history; what was undone cannot be
                // reached from here any more.
                self.redo.clear();
                Self::push(&mut self.undo, step);
            }
            Land::Undone => Self::push(&mut self.redo, step),
            Land::Redone => Self::push(&mut self.undo, step),
        }
    }

    /// Put a step back where it came from, for work that did not go through.
    pub(super) fn keep(&mut self, step: UndoStep, how: Land) {
        match how {
            // The action itself failed, so there is nothing to take back.
            Land::Fresh => {}
            Land::Undone => Self::push(&mut self.undo, step),
            Land::Redone => Self::push(&mut self.redo, step),
        }
    }

    pub(super) fn push(stack: &mut Vec<UndoStep>, step: UndoStep) {
        stack.push(step);
        if stack.len() > Self::MAX {
            stack.remove(0);
        }
    }
}
