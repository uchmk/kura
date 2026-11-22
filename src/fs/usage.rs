//! What is eating the space: the children of one directory, each with the total
//! of everything underneath it.
//!
//! The list already answers "how big is this file"; what it cannot answer is
//! "how big is this folder", because a directory's own length is the size of its
//! inode and `Entry::dir_size` is a count of children one level down. Sorting by
//! size therefore cannot find the folder that is full, which is the one question
//! worth asking about a disk.
//!
//! Two phases. The first lists the children of the root as they are read: a
//! file is sized from the metadata that listed it, a folder goes out unsized.
//! The second measures the folders one at a time, largest last to arrive is
//! fine -- the view re-sorts as results come in. A walk started with a
//! `pause_after` stops once the second phase has run that long and waits to be
//! told to go on, so a folder that turns out heavy costs a bounded wait rather
//! than a long one. Nothing here uses the `ignore` walker that search uses: its
//! defaults respect `.gitignore` and skip hidden files, and a folder does not
//! stop taking up room because git was told to overlook it.

use std::collections::HashSet;
use std::fs::ReadDir;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, Sender};

use crate::fs::Entry;

/// Entries visited before the walk gives up, shared across all the children.
/// The same budget `ops::measure` uses, and for the same reason: a pathological
/// tree must not hold the worker for ever.
const BUDGET: usize = 200_000;
/// How often partial results are sent. A size walk produces far more events than
/// a name search, so this is the throttle `ops` reports progress through.
const TICK: Duration = Duration::from_millis(50);

/// A child and what is under it: bytes, the number of files counted, and
/// whether the walk got to the end of it. A child the budget ran out in, or
/// never reached, is a floor and not an answer (44.7, #109).
///
/// The `Entry` is built here, on the walker thread, so the UI thread never
/// stats a child of a 68,000-entry folder.
pub type Child = (Entry, u64, u64, bool);

pub enum Msg {
    /// Folders of the root, not measured yet. Files never come this way: they
    /// are sized as they are listed.
    Listed(Vec<Entry>),
    /// Children with their totals. A row `Listed` earlier is found by path.
    Sized(Vec<Child>),
    /// Every child of the root has been sent.
    AllListed,
    /// The walk has run for as long as it was allowed to by itself and waits
    /// for [`Handle::measure_all`].
    Paused,
    Done {
        total: u64,
        /// Whether the walk ran out of budget, so the totals are floors rather
        /// than answers.
        capped: bool,
    },
}

pub struct Handle {
    pub rx: Receiver<Msg>,
    cancel: Arc<AtomicBool>,
    all: Arc<AtomicBool>,
    resume: Sender<()>,
    /// The walk was started with a `pause_after`.
    auto: bool,
}

impl Handle {
    /// Stop the walk. A paused one is woken so it can end.
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
        let _ = self.resume.try_send(());
    }

    /// Measure everything without pausing, from where the walk stands. Sent just
    /// before the pause it is not lost: the token waits in the channel.
    pub fn measure_all(&self) {
        self.all.store(true, Ordering::Relaxed);
        let _ = self.resume.try_send(());
    }

    /// The walk may still pause: it has a `pause_after` and has not been told
    /// to measure everything.
    pub fn limited(&self) -> bool {
        self.auto && !self.all.load(Ordering::Relaxed)
    }
}

/// Dropping the handle stops the walk, so leaving the view is enough to end it.
/// A paused walker's resume channel is disconnected by the same drop.
impl Drop for Handle {
    fn drop(&mut self) {
        self.cancel();
    }
}

/// Where the walk's results go: batched, and flushed at most once per [`TICK`].
struct Out {
    tx: Sender<Msg>,
    wake: Box<dyn Fn() + Send>,
    listed: Vec<Entry>,
    sized: Vec<Child>,
    last: Option<Instant>,
}

impl Out {
    fn send(&self, msg: Msg) {
        let _ = self.tx.send(msg);
    }

    fn flush(&mut self) {
        let mut sent = false;
        if !self.listed.is_empty() {
            let v = std::mem::take(&mut self.listed);
            self.send(Msg::Listed(v));
            sent = true;
        }
        if !self.sized.is_empty() {
            let v = std::mem::take(&mut self.sized);
            self.send(Msg::Sized(v));
            sent = true;
        }
        if sent {
            (self.wake)();
        }
    }

    /// Flush if it has been a tick since the last time. The first call does.
    fn tick(&mut self) {
        if self.last.is_none_or(|t| t.elapsed() >= TICK) {
            self.flush();
            self.last = Some(Instant::now());
        }
    }
}

/// The pause: once, in the second phase, when the walk has run long enough by
/// itself.
struct Pause {
    after: Option<Duration>,
    started: Instant,
    paused: bool,
    all: Arc<AtomicBool>,
    resume: Receiver<()>,
    /// Tests pause after this many entries rather than after a time.
    #[cfg(test)]
    gate: Option<usize>,
    #[cfg(test)]
    seen: usize,
}

impl Pause {
    /// Called before every entry the second phase takes. `false` means stop.
    fn poll(&mut self, out: &mut Out, stop: &AtomicBool) -> bool {
        if stop.load(Ordering::Relaxed) {
            return false;
        }
        let Some(after) = self.after.filter(|_| !self.paused) else { return true };
        #[cfg(test)]
        let gated = {
            let hit = self.gate.is_some_and(|g| self.seen >= g);
            self.seen += 1;
            hit
        };
        #[cfg(not(test))]
        let gated = false;
        if self.all.load(Ordering::Relaxed) || !(gated || self.started.elapsed() >= after) {
            return true;
        }
        self.paused = true;
        out.flush();
        out.send(Msg::Paused);
        (out.wake)();
        // Blocked until `measure_all` or `cancel`; the handle going away ends it too.
        if self.resume.recv().is_err() {
            return false;
        }
        !stop.load(Ordering::Relaxed)
    }
}

/// Measure `root`'s children. `pause_after` is how long the folders may be
/// measured by themselves before the walk stops and waits for
/// [`Handle::measure_all`]; `None` never pauses and `Some(ZERO)` pauses before
/// the first folder.
pub fn spawn(root: &Path, pause_after: Option<Duration>, wake: impl Fn() + Send + 'static) -> Handle {
    spawn_inner(root, pause_after, None, wake)
}

/// `spawn`, pausing after `gate` entries of the second phase as well, so a
/// pause inside a folder is deterministic.
#[cfg(test)]
pub(crate) fn spawn_gated(root: &Path, pause_after: Duration, gate: usize, wake: impl Fn() + Send + 'static) -> Handle {
    spawn_inner(root, Some(pause_after), Some(gate), wake)
}

#[cfg_attr(not(test), allow(unused_variables))]
fn spawn_inner(root: &Path, pause_after: Option<Duration>, gate: Option<usize>, wake: impl Fn() + Send + 'static) -> Handle {
    let (tx, rx) = crossbeam_channel::unbounded();
    let (resume, resume_rx) = crossbeam_channel::bounded(1);
    let cancel = Arc::new(AtomicBool::new(false));
    let all = Arc::new(AtomicBool::new(false));
    let stop = cancel.clone();
    let pause = Pause {
        after: pause_after,
        started: Instant::now(),
        paused: false,
        all: all.clone(),
        resume: resume_rx,
        #[cfg(test)]
        gate,
        #[cfg(test)]
        seen: 0,
    };
    let walk_root = root.to_path_buf();
    let out = Out { tx, wake: Box::new(wake), listed: Vec::new(), sized: Vec::new(), last: None };
    std::thread::Builder::new().name("usage".into()).spawn(move || walk(&walk_root, out, pause, &stop)).expect("spawn usage thread");
    Handle { rx, cancel, all, resume, auto: pause_after.is_some() }
}

fn walk(root: &Path, mut out: Out, mut pause: Pause, stop: &AtomicBool) {
    let mut budget = BUDGET;
    let mut seen = HashSet::new();
    let mut total = 0u64;
    let mut queue: Vec<Entry> = Vec::new();

    // Phase 1: list as the directory is read. Every child is listed even past
    // the budget, so the folder is shown whole.
    if let Ok(rd) = std::fs::read_dir(root) {
        for de in rd.filter_map(|e| e.ok()) {
            if stop.load(Ordering::Relaxed) {
                return;
            }
            let path = de.path();
            let Ok(md) = std::fs::symlink_metadata(&path) else { continue };
            budget = budget.saturating_sub(1);
            let entry = Entry::from_metadata(path, &md);
            if md.is_dir() && !md.file_type().is_symlink() {
                queue.push(entry.clone());
                out.listed.push(entry);
            } else {
                // A file, or a link of any kind: one entry, never followed.
                let (bytes, files) = if link_key(&md).is_some_and(|k| !seen.insert(k)) { (0, 0) } else { (md.len(), 1) };
                total += bytes;
                out.sized.push((entry, bytes, files, true));
            }
            out.tick();
        }
    }
    out.flush();
    out.send(Msg::AllListed);
    (out.wake)();

    // Phase 2: the folders, in listing order.
    pause.started = Instant::now();
    let mut cut = false;
    for entry in queue {
        if !pause.poll(&mut out, stop) {
            return;
        }
        let (bytes, files, whole) = measure(&entry.path, &mut budget, &mut seen, &mut || pause.poll(&mut out, stop));
        if stop.load(Ordering::Relaxed) {
            return;
        }
        cut |= !whole;
        total += bytes;
        out.sized.push((entry, bytes, files, whole));
        out.tick();
    }
    out.flush();
    out.send(Msg::Done { total, capped: cut });
    (out.wake)();
}

/// Everything under the directory `dir` (which the caller counted already), as
/// bytes, a file count, and whether all of it was reached before the budget ran
/// out.
///
/// An explicit stack of open directories rather than recursion, read one entry
/// at a time so that `go` -- asked before each -- can stop or pause the walk in
/// the middle of a folder of 50,000 entries without losing the place.
/// `symlink_metadata` so a link to a directory is one entry rather than a second
/// copy of a tree -- or a loop. A file with more than one hard link is counted
/// once, under the first name the walk meets, as `du` does: `seen` holds the ones
/// met so far across the whole walk, so two folders sharing a file do not both
/// claim it. Unix only; see [`link_key`].
///
/// `false` from `go` ends the walk and reads as cut short.
fn measure(dir: &Path, budget: &mut usize, seen: &mut HashSet<(u64, u64)>, go: &mut dyn FnMut() -> bool) -> (u64, u64, bool) {
    let (mut bytes, mut files) = (0u64, 0u64);
    // Opened even with no budget left: an empty folder is whole, not a `?`,
    // and only an entry the budget refuses makes it cut.
    let Ok(rd) = std::fs::read_dir(dir) else { return (0, 0, true) };
    let mut stack: Vec<ReadDir> = vec![rd];
    while let Some(top) = stack.last_mut() {
        let Some(next) = top.next() else {
            stack.pop();
            continue;
        };
        if !go() {
            return (bytes, files, false);
        }
        let Ok(de) = next else { continue };
        if *budget == 0 {
            return (bytes, files, false);
        }
        *budget -= 1;
        let p = de.path();
        let Ok(md) = std::fs::symlink_metadata(&p) else { continue };
        if md.is_dir() && !md.file_type().is_symlink() {
            if let Ok(rd) = std::fs::read_dir(&p) {
                stack.push(rd);
            }
        } else {
            if link_key(&md).is_some_and(|k| !seen.insert(k)) {
                continue;
            }
            files += 1;
            bytes += md.len();
        }
    }
    (bytes, files, true)
}

/// The device and inode of a file other names also point at, or `None` for a
/// file with only the one name -- the common case, kept out of `seen`.
///
/// Windows has the same idea (a volume serial and a file index) but std does
/// not expose it on stable, and reading it means opening every file, which on
/// a walk of 200,000 entries is not a price worth paying for links NTFS users
/// rarely make. There a tree that uses them still reads high (README).
#[cfg(unix)]
fn link_key(md: &std::fs::Metadata) -> Option<(u64, u64)> {
    use std::os::unix::fs::MetadataExt;
    (md.nlink() > 1).then(|| (md.dev(), md.ino()))
}

#[cfg(not(unix))]
fn link_key(_md: &std::fs::Metadata) -> Option<(u64, u64)> {
    None
}

/// A handle no walker backs, for tests that feed `drain_usage` by hand.
#[cfg(test)]
pub(crate) fn test_handle() -> (Sender<Msg>, Handle) {
    test_handle_with(false)
}

/// The same, as a walk that was started with a `pause_after`.
#[cfg(test)]
pub(crate) fn test_handle_limited() -> (Sender<Msg>, Handle) {
    test_handle_with(true)
}

#[cfg(test)]
fn test_handle_with(auto: bool) -> (Sender<Msg>, Handle) {
    let (tx, rx) = crossbeam_channel::unbounded();
    let (resume, _) = crossbeam_channel::bounded(1);
    (tx, Handle { rx, cancel: Arc::new(AtomicBool::new(false)), all: Arc::new(AtomicBool::new(false)), resume, auto })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    fn tree() -> PathBuf {
        let dir = crate::util::test_dir("usage");
        std::fs::create_dir_all(dir.join("big").join("deep")).unwrap();
        std::fs::create_dir_all(dir.join("small")).unwrap();
        std::fs::write(dir.join("big").join("a"), vec![b'x'; 300]).unwrap();
        std::fs::write(dir.join("big").join("deep").join("b"), vec![b'x'; 700]).unwrap();
        std::fs::write(dir.join("small").join("c"), vec![b'x'; 50]).unwrap();
        std::fs::write(dir.join("loose"), vec![b'x'; 10]).unwrap();
        dir
    }

    /// What a walk sent, in the order it sent it.
    #[derive(Default)]
    struct Run {
        listed: Vec<Entry>,
        sized: Vec<Child>,
        /// `L`isted, `S`ized, `A`llListed, `P`aused, `D`one.
        order: String,
        total: u64,
        capped: bool,
        done: bool,
    }

    impl Run {
        /// Name -> (bytes, files, whole) of every child sized so far.
        fn by_name(&self) -> BTreeMap<String, (u64, u64, bool)> {
            self.sized.iter().map(|(e, b, f, w)| (e.name.clone(), (*b, *f, *w))).collect()
        }
    }

    /// Read messages until the walk is `Done`, or -- with `stop_at_pause` --
    /// until it says it paused.
    fn read(h: &Handle, run: &mut Run, stop_at_pause: bool) {
        loop {
            match h.rx.recv_timeout(Duration::from_secs(10)).expect("the walk answered") {
                Msg::Listed(v) => {
                    run.order.push('L');
                    run.listed.extend(v);
                }
                Msg::Sized(v) => {
                    run.order.push('S');
                    run.sized.extend(v);
                }
                Msg::AllListed => run.order.push('A'),
                Msg::Paused => {
                    run.order.push('P');
                    if stop_at_pause {
                        return;
                    }
                }
                Msg::Done { total, capped } => {
                    run.order.push('D');
                    (run.total, run.capped, run.done) = (total, capped, true);
                    return;
                }
            }
        }
    }

    fn collect(root: &Path) -> Run {
        let h = spawn(root, None, || {});
        let mut run = Run::default();
        read(&h, &mut run, false);
        run
    }

    fn names(v: &[Entry]) -> Vec<String> {
        let mut n: Vec<String> = v.iter().map(|e| e.name.clone()).collect();
        n.sort();
        n
    }

    /// The whole point: a folder's total is everything underneath it, not the
    /// size of its own directory entry.
    #[test]
    fn a_folder_is_worth_what_is_under_it() {
        let dir = tree();
        let run = collect(&dir);
        assert!(!run.capped);
        let by = run.by_name();
        assert_eq!(by["big"].0, 1000, "300 and 700, counted through the subfolder: {by:?}");
        assert_eq!(by["small"].0, 50);
        assert_eq!(by["loose"].0, 10, "a plain file is worth its own length");
        assert_eq!(run.total, 1060);
        // Three children of the root, and no grandchildren among them.
        assert_eq!(by.len(), 3, "{by:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_file_counts_come_back_too() {
        let dir = tree();
        let by = collect(&dir).by_name();
        assert_eq!(by["big"].1, 2);
        assert_eq!(by["small"].1, 1);
        assert_eq!(by["loose"].1, 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Phase 1: the folders are listed unsized, the file is sized from the
    /// listing, and the end of the listing is announced before any folder is
    /// measured.
    #[test]
    fn the_children_are_listed_before_the_folders_are_measured() {
        let dir = tree();
        let run = collect(&dir);
        assert_eq!(names(&run.listed), ["big", "small"], "folders only: files are sized at once");
        let a = run.order.find('A').expect("the listing ended");
        assert!(run.order[..a].contains('L') && run.order[..a].contains('S'), "{}", run.order);
        assert!(run.order.ends_with('D'), "{}", run.order);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 44.7: a child the budget ran out in says so, and so does one it never
    /// reached -- the second used to come back as a plain 0 B, which is what
    /// `C:\Windows` read at the root of a drive.
    #[test]
    fn a_walk_cut_short_says_so() {
        let dir = tree();
        let mut budget = 2;
        let (_, _, whole) = measure(&dir.join("big"), &mut budget, &mut HashSet::new(), &mut || true);
        assert!(!whole, "three entries under `big`, two allowed");
        let (bytes, files, whole) = measure(&dir.join("small"), &mut budget, &mut HashSet::new(), &mut || true);
        assert_eq!((bytes, files, whole), (0, 0, false), "nothing left for `small`: unknown, not empty");

        let mut budget = 1000;
        assert!(measure(&dir.join("small"), &mut budget, &mut HashSet::new(), &mut || true).2, "with room, the walk ends");
        let run = collect(&dir);
        assert!(run.sized.iter().all(|t| t.3), "and a whole walk marks every child whole: {:?}", run.by_name());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A budget that reaches zero exactly on the last entry is not a cut.
    #[test]
    fn a_budget_that_ends_on_the_last_entry_is_not_a_cut() {
        let dir = tree();
        let mut budget = 1;
        let (bytes, files, whole) = measure(&dir.join("small"), &mut budget, &mut HashSet::new(), &mut || true);
        assert_eq!((bytes, files, whole), (50, 1, true));
        assert_eq!(budget, 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A refusal from the pause hook ends the folder as cut short.
    #[test]
    fn a_stop_is_a_cut() {
        let dir = tree();
        let mut n = 0;
        let (_, _, whole) = measure(&dir.join("big"), &mut 100, &mut HashSet::new(), &mut || {
            n += 1;
            n < 2
        });
        assert!(!whole);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A link to a directory is one entry and is not followed: its target is
    /// counted once, as itself.
    #[cfg(unix)]
    #[test]
    fn a_link_to_a_directory_is_one_entry() {
        let dir = crate::util::test_dir("usage-dirlink");
        std::fs::create_dir_all(dir.join("real")).unwrap();
        std::fs::write(dir.join("real").join("f"), vec![b'x'; 100]).unwrap();
        std::os::unix::fs::symlink(dir.join("real"), dir.join("ln")).unwrap();
        let run = collect(&dir);
        assert_eq!(names(&run.listed), ["real"], "the link is not a folder to walk");
        let by = run.by_name();
        assert_eq!(by["real"], (100, 1, true));
        assert_eq!(by["ln"].1, 1, "one entry: {by:?}");
        assert!(by["ln"].0 < 100, "its own length, not the tree's: {by:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A file hard-linked into two folders is counted once, under the first
    /// folder the walk meets, and the total is the bytes on disk.
    #[cfg(unix)]
    #[test]
    fn a_hard_link_is_counted_once() {
        let dir = crate::util::test_dir("usage-links");
        std::fs::create_dir_all(dir.join("one")).unwrap();
        std::fs::create_dir_all(dir.join("two")).unwrap();
        std::fs::write(dir.join("one").join("a"), vec![b'x'; 400]).unwrap();
        std::fs::hard_link(dir.join("one").join("a"), dir.join("two").join("a")).unwrap();
        std::fs::write(dir.join("two").join("b"), vec![b'x'; 5]).unwrap();
        let run = collect(&dir);
        assert_eq!(run.total, 405, "400 once, and 5: {:?}", run.by_name());
        let by = run.by_name();
        assert!(matches!((by["one"].0, by["two"].0), (400, 5) | (0, 405)), "{by:?}");
    }

    /// A file hard-linked directly in the root and again inside a folder is
    /// counted once, under the direct file (listed first).
    #[cfg(unix)]
    #[test]
    fn a_hard_link_between_the_root_and_a_child_is_counted_once() {
        let dir = crate::util::test_dir("usage-links-root");
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        std::fs::write(dir.join("top"), vec![b'x'; 400]).unwrap();
        std::fs::hard_link(dir.join("top"), dir.join("sub").join("again")).unwrap();
        let run = collect(&dir);
        assert_eq!(run.total, 400, "{:?}", run.by_name());
        let by = run.by_name();
        assert_eq!((by["top"].0, by["sub"].0), (400, 0), "the direct file claims it: {by:?}");
    }

    /// An empty or unreadable directory answers rather than hanging.
    #[test]
    fn an_empty_directory_finishes_at_zero() {
        let dir = crate::util::test_dir("usage-empty");
        let run = collect(&dir);
        assert!(run.sized.is_empty() && run.listed.is_empty());
        assert_eq!(run.total, 0);
        assert!(!run.capped);
        assert_eq!(run.order, "AD");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A pause at zero comes before the first folder: everything is listed and
    /// the files sized, no folder is, and `measure_all` finishes with the totals
    /// an unlimited walk gives.
    #[test]
    fn a_pause_at_zero_comes_before_the_first_folder() {
        let dir = tree();
        let h = spawn(&dir, Some(Duration::ZERO), || {});
        assert!(h.limited());
        let mut run = Run::default();
        read(&h, &mut run, true);
        assert!(!run.done);
        assert_eq!(names(&run.listed), ["big", "small"]);
        assert_eq!(run.by_name(), BTreeMap::from([("loose".to_string(), (10, 1, true))]), "only the file is sized");
        assert!(run.order.contains('A') && run.order.ends_with('P'), "{}", run.order);
        h.measure_all();
        assert!(!h.limited());
        read(&h, &mut run, false);
        assert!(run.done && !run.capped);
        let plain = collect(&dir);
        assert_eq!(run.by_name(), plain.by_name());
        assert_eq!(run.total, plain.total);
        assert_eq!(run.order.matches('P').count(), 1, "one pause per walk: {}", run.order);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// No folder, no pause: there is nothing to stop before.
    #[test]
    fn a_pause_at_zero_needs_a_folder() {
        let dir = crate::util::test_dir("usage-flat");
        std::fs::write(dir.join("f"), b"abc").unwrap();
        let h = spawn(&dir, Some(Duration::ZERO), || {});
        let mut run = Run::default();
        read(&h, &mut run, false);
        assert!(run.done && !run.order.contains('P'), "{}", run.order);
        assert_eq!(run.total, 3);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A pause in the middle of a folder keeps its place: the totals after
    /// resuming are those of the walk that never paused.
    #[test]
    fn a_pause_inside_a_folder_loses_nothing() {
        let dir = tree();
        let plain = collect(&dir);
        for gate in 1..4 {
            let h = spawn_gated(&dir, Duration::MAX, gate, || {});
            let mut run = Run::default();
            read(&h, &mut run, true);
            assert!(!run.done, "gate {gate}: paused, not finished");
            assert!(run.order.ends_with('P'), "{}", run.order);
            h.measure_all();
            read(&h, &mut run, false);
            assert_eq!(run.by_name(), plain.by_name(), "gate {gate}");
            assert_eq!(run.total, plain.total, "gate {gate}");
            assert!(!run.capped);
            assert_eq!(run.order.matches('P').count(), 1, "gate {gate}: {}", run.order);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// `measure_all` sent before the pause point: either no `Paused` comes, or
    /// it comes after the handle was told, which the app ignores as stale. The
    /// walk ends without a second `measure_all`.
    #[test]
    fn measure_all_before_the_pause_point_is_not_lost() {
        let dir = tree();
        for _ in 0..20 {
            let h = spawn_gated(&dir, Duration::MAX, 1, || {});
            h.measure_all();
            assert!(!h.limited(), "a Paused read now is stale");
            let mut run = Run::default();
            read(&h, &mut run, false);
            assert!(run.done);
            assert_eq!(run.total, 1060);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The channel is empty and disconnected: the walker thread is gone.
    fn ends(rx: &Receiver<Msg>) -> bool {
        let until = Instant::now() + Duration::from_secs(10);
        while Instant::now() < until {
            match rx.recv_timeout(Duration::from_millis(50)) {
                Err(crossbeam_channel::RecvTimeoutError::Disconnected) => return true,
                Err(crossbeam_channel::RecvTimeoutError::Timeout) => {}
                Ok(Msg::Done { .. }) => return false,
                Ok(_) => {}
            }
        }
        false
    }

    /// A paused walker ends when its handle is dropped, without a `Done`.
    #[test]
    fn dropping_a_paused_handle_ends_the_thread() {
        let dir = tree();
        let h = spawn(&dir, Some(Duration::ZERO), || {});
        let mut run = Run::default();
        read(&h, &mut run, true);
        let rx = h.rx.clone();
        drop(h);
        assert!(ends(&rx), "the thread returned");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// `cancel` wakes a paused walker the same way.
    #[test]
    fn cancelling_a_paused_handle_ends_the_thread() {
        let dir = tree();
        let h = spawn(&dir, Some(Duration::ZERO), || {});
        let mut run = Run::default();
        read(&h, &mut run, true);
        h.cancel();
        assert!(ends(&h.rx), "the thread returned");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
