//! A frame-by-frame record of what the details view's Dimensions, Owner, Permissions and Extension cells painted,
//! frozen at each `<State:>` checkpoint of a `--keys` run (TESTING.md 69.15).
//!
//! Pure and deterministic: it is fed one [`MetaFrame`] per accepted pass and knows nothing about egui. Memory is the
//! previous frame's rows, the rows still waiting for a value and a handful of scalars; nothing grows with the number of
//! frames.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

/// Names of the four watched columns, in `MetaFrame::shown` order.
pub const COLUMNS: [&str; 4] = ["Dimensions", "Owner", "Permissions", "Extension"];
const OWNER: usize = 1;
const DIMS: usize = 0;
/// A value still missing this many frames after its row arrived counts as late.
const LATE_AT: u64 = 3;
/// How many blink / change examples are kept as text.
const EXAMPLES: usize = 5;

/// One row of the details view as painted.
#[derive(Clone, Debug, PartialEq)]
pub struct MetaCell {
    /// The row's index in the list.
    pub index: usize,
    /// The entry's full path: a stable identity.
    pub path: PathBuf,
    /// A regular file whose extension kura reads dimensions for.
    pub image: bool,
    /// The text handed to the painter for Dimensions, Owner, Permissions and Extension; empty when the column is hidden
    /// or the cell painted nothing.
    pub cells: [String; 4],
}

/// The cells a details view painted in one pass, top to bottom.
#[derive(Clone, Debug, PartialEq)]
pub struct MetaFrame {
    /// Which of the four columns the layout kept at this width.
    pub shown: [bool; 4],
    pub rows: Vec<MetaCell>,
}

/// What a frame is a frame *of*: when it changes, rows cannot be compared with the last one.
#[derive(Clone, Debug, PartialEq)]
pub struct Scope {
    pub tab: usize,
    pub folder: PathBuf,
    pub shown: [bool; 4],
}

/// How long the values of one column took to appear after their row scrolled in.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Lag {
    /// A value in the arrival frame itself.
    pub at_once: usize,
    /// A value 1 or 2 frames later.
    pub soon: usize,
    /// Still empty 3 frames after the arrival.
    pub late: usize,
    /// The largest distance at which a late value came, or the age of a late row when it left.
    pub max_late: u64,
    /// The row left before frame 3 without a value.
    pub left_early: usize,
    /// Still waiting, younger than 3 frames, when the interval was frozen.
    pub pending: usize,
    /// Arrivals that got a value at all.
    pub seen: usize,
}

/// One frozen interval, from one checkpoint to the next.
#[derive(Clone, Debug, PartialEq)]
pub struct Summary {
    pub from: String,
    pub to: String,
    pub frames: usize,
    pub gaps: usize,
    pub interruptions: usize,
    pub moved_down: bool,
    pub moved_up: bool,
    pub arrived: usize,
    pub images: usize,
    /// `None` when the Owner column was not shown.
    pub owner: Option<Lag>,
    /// `None` when the Dimensions column was not shown.
    pub dims: Option<Lag>,
    pub blinks: usize,
    pub blink_examples: Vec<String>,
    pub changed: usize,
    pub changed_examples: Vec<String>,
    pub shown: [bool; 4],
    /// Whether the interval saw the details view at all.
    pub saw_details: bool,
}

impl Summary {
    /// Rows scrolled in, Owner given, and (when the column is shown) an image whose Dimensions came: the interval
    /// exercised what row 69.15 is about.
    pub fn covered(&self) -> bool {
        self.arrived > 0
            && self.owner.as_ref().is_some_and(|o| o.seen > 0)
            && self.dims.as_ref().is_none_or(|d| self.images > 0 && d.seen > 0)
    }
}

/// Where one waiting value stands.
#[derive(Clone, Copy, PartialEq)]
enum Wait {
    /// Not tracked (column hidden, or not an image row for Dimensions).
    No,
    /// Empty so far, younger than `LATE_AT`.
    Young,
    /// Empty `LATE_AT` frames after the arrival (already counted late); still watched for the value.
    Late,
    Done,
}

struct Waiting {
    arrival: u64,
    cols: [Wait; 2],
}

struct Acc {
    from: String,
    frames: usize,
    gaps: usize,
    interruptions: usize,
    moved_down: bool,
    moved_up: bool,
    arrived: usize,
    images: usize,
    lags: [Lag; 2],
    blinks: usize,
    blink_examples: Vec<String>,
    changed: usize,
    changed_examples: Vec<String>,
    shown: [bool; 4],
    saw_details: bool,
}

impl Acc {
    fn new(from: &str) -> Self {
        Acc {
            from: from.to_owned(),
            frames: 0,
            gaps: 0,
            interruptions: 0,
            moved_down: false,
            moved_up: false,
            arrived: 0,
            images: 0,
            lags: Default::default(),
            blinks: 0,
            blink_examples: Vec::new(),
            changed: 0,
            changed_examples: Vec::new(),
            shown: [false; 4],
            saw_details: false,
        }
    }
}

#[derive(Default)]
pub struct DetailsWatch {
    acc: Option<Acc>,
    /// Passes observed since the run began; the clock lags are measured on.
    pass: u64,
    prev_scope: Option<Scope>,
    prev: Vec<MetaCell>,
    waiting: HashMap<PathBuf, Waiting>,
    last: Option<Summary>,
}

fn name_of(path: &std::path::Path) -> String {
    path.file_name().map_or_else(|| path.to_string_lossy().into_owned(), |n| n.to_string_lossy().into_owned())
}

impl DetailsWatch {
    /// The interval frozen by the last checkpoint.
    pub fn last(&self) -> Option<&Summary> {
        self.last.as_ref()
    }

    /// Feed one accepted pass. `frame` is `None` when the details view was not laid out in it.
    pub fn observe(&mut self, scope: Scope, _now_ms: u64, frame: Option<&MetaFrame>) {
        self.pass += 1;
        let pass = self.pass;
        let acc = self.acc.get_or_insert_with(|| Acc::new("start"));
        acc.frames += 1;
        let Some(frame) = frame else {
            if acc.saw_details {
                acc.gaps += 1;
            }
            return;
        };
        acc.saw_details = true;
        acc.shown = frame.shown;
        if self.prev_scope.as_ref().is_some_and(|s| *s != scope) {
            acc.interruptions += 1;
            Self::drop_waiting(&mut self.waiting, acc, pass);
            self.prev.clear();
            self.prev_scope = None;
        }
        let has_prev = self.prev_scope.is_some();
        self.prev_scope = Some(scope);
        if !frame.shown.iter().any(|s| *s) {
            self.prev.clear();
            return;
        }
        if let (Some(before), Some(now)) = (self.prev.first(), frame.rows.first()) {
            acc.moved_down |= now.index > before.index;
            acc.moved_up |= now.index < before.index;
        }
        let before: HashMap<&PathBuf, &MetaCell> = self.prev.iter().map(|r| (&r.path, r)).collect();
        for row in &frame.rows {
            match before.get(&row.path) {
                None if has_prev => {
                    acc.arrived += 1;
                    acc.images += usize::from(row.image);
                    let tracks = [row.image && frame.shown[DIMS], frame.shown[OWNER]];
                    let cols = tracks.map(|t| if t { Wait::Young } else { Wait::No });
                    self.waiting.insert(row.path.clone(), Waiting { arrival: pass, cols });
                }
                None => {}
                Some(was) => {
                    for col in 0..4 {
                        if !frame.shown[col] {
                            continue;
                        }
                        let (a, b) = (&was.cells[col], &row.cells[col]);
                        if !a.is_empty() && b.is_empty() {
                            acc.blinks += 1;
                            if acc.blink_examples.len() < EXAMPLES {
                                acc.blink_examples.push(format!("{} {} `{a}` → blank at frame {pass}", name_of(&row.path), COLUMNS[col]));
                            }
                        } else if !a.is_empty() && a != b {
                            acc.changed += 1;
                            if acc.changed_examples.len() < EXAMPLES {
                                acc.changed_examples.push(format!("{} {} `{a}` → `{b}` at frame {pass}", name_of(&row.path), COLUMNS[col]));
                            }
                        }
                    }
                }
            }
        }
        // Rows that left the screen: settle them.
        let now: HashSet<&PathBuf> = frame.rows.iter().map(|r| &r.path).collect();
        let gone: Vec<PathBuf> = self.waiting.keys().filter(|p| !now.contains(p)).cloned().collect();
        for path in gone {
            if let Some(w) = self.waiting.remove(&path) {
                Self::settle(&w, acc, pass, false);
            }
        }
        // Rows still on screen: look for the values they were waiting for.
        let mut finished = Vec::new();
        for row in &frame.rows {
            let Some(w) = self.waiting.get_mut(&row.path) else { continue };
            let dist = pass - w.arrival;
            for (slot, col) in [(0, DIMS), (1, OWNER)] {
                let lag = &mut acc.lags[slot];
                match w.cols[slot] {
                    Wait::No | Wait::Done => {}
                    state if !row.cells[col].is_empty() => {
                        lag.seen += 1;
                        match (state, dist) {
                            (Wait::Late, d) => lag.max_late = lag.max_late.max(d),
                            (_, d) if d >= LATE_AT => {
                                lag.late += 1;
                                lag.max_late = lag.max_late.max(d);
                            }
                            (_, 0) => lag.at_once += 1,
                            _ => lag.soon += 1,
                        }
                        w.cols[slot] = Wait::Done;
                    }
                    Wait::Young if dist >= LATE_AT => {
                        lag.late += 1;
                        lag.max_late = lag.max_late.max(dist);
                        w.cols[slot] = Wait::Late;
                    }
                    _ => {}
                }
            }
            if w.cols.iter().all(|c| matches!(c, Wait::No | Wait::Done)) {
                finished.push(row.path.clone());
            }
        }
        for path in finished {
            self.waiting.remove(&path);
        }
        self.prev = frame.rows.clone();
    }

    /// A row that left (or an interval that ended) while values were still missing.
    fn settle(w: &Waiting, acc: &mut Acc, pass: u64, checkpoint: bool) {
        // `pass - 1` is the last frame the row was seen in on a leave; at a checkpoint it is the current pass.
        let age = pass.saturating_sub(w.arrival).saturating_sub(u64::from(!checkpoint));
        for slot in 0..2 {
            let lag = &mut acc.lags[slot];
            match w.cols[slot] {
                Wait::Young if checkpoint => lag.pending += 1,
                Wait::Young => lag.left_early += 1,
                Wait::Late => lag.max_late = lag.max_late.max(age),
                Wait::No | Wait::Done => {}
            }
        }
    }

    fn drop_waiting(waiting: &mut HashMap<PathBuf, Waiting>, acc: &mut Acc, pass: u64) {
        for (_, w) in waiting.drain() {
            Self::settle(&w, acc, pass, false);
        }
    }

    /// Freeze the interval that ends here under `label` and start the next one. The rows on screen now are the next
    /// interval's baseline, so they are not arrivals in it.
    pub fn checkpoint(&mut self, label: &str) {
        let mut acc = self.acc.take().unwrap_or_else(|| Acc::new("start"));
        let pass = self.pass;
        for (_, w) in self.waiting.drain() {
            Self::settle(&w, &mut acc, pass, true);
        }
        let [dims, owner] = acc.lags;
        self.last = Some(Summary {
            from: acc.from,
            to: label.to_owned(),
            frames: acc.frames,
            gaps: acc.gaps,
            interruptions: acc.interruptions,
            moved_down: acc.moved_down,
            moved_up: acc.moved_up,
            arrived: acc.arrived,
            images: acc.images,
            owner: acc.shown[1].then_some(owner),
            dims: acc.shown[0].then_some(dims),
            blinks: acc.blinks,
            blink_examples: acc.blink_examples,
            changed: acc.changed,
            changed_examples: acc.changed_examples,
            shown: acc.shown,
            saw_details: acc.saw_details,
        });
        self.acc = Some(Acc::new(label));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scope(folder: &str) -> Scope {
        Scope { tab: 0, folder: PathBuf::from(folder), shown: [true; 4] }
    }

    fn row(i: usize, owner: &str, dims: &str) -> MetaCell {
        let image = i.is_multiple_of(10);
        MetaCell {
            index: i,
            path: PathBuf::from(format!("/d/f{i:05}.{}", if image { "png" } else { "txt" })),
            image,
            cells: [if image { dims.to_owned() } else { String::new() }, owner.to_owned(), "-a--".into(), "txt".into()],
        }
    }

    fn frame(top: usize, n: usize, owner: &str) -> MetaFrame {
        MetaFrame { shown: [true; 4], rows: (top..top + n).map(|i| row(i, owner, "16 × 12")).collect() }
    }

    fn feed(w: &mut DetailsWatch, f: &MetaFrame) {
        w.observe(scope("/d"), 0, Some(f));
    }

    #[test]
    fn rows_scrolling_in_with_their_owner_at_once_are_all_at_once() {
        let mut w = DetailsWatch::default();
        feed(&mut w, &frame(0, 30, "alice"));
        w.checkpoint("before");
        for top in 1..=270 {
            feed(&mut w, &frame(top, 30, "alice"));
        }
        w.checkpoint("down");
        let s = w.last().unwrap();
        assert_eq!((s.from.as_str(), s.to.as_str()), ("before", "down"));
        assert_eq!((s.frames, s.arrived, s.gaps, s.interruptions, s.blinks), (270, 270, 0, 0, 0));
        assert!(s.moved_down && !s.moved_up);
        let o = s.owner.as_ref().unwrap();
        assert_eq!((o.at_once, o.soon, o.late, o.left_early, o.pending, o.seen), (270, 0, 0, 0, 0, 270));
        // Every tenth row is an image: indexes 30..=299 hold 27 of them.
        assert_eq!(s.images, 27);
        assert_eq!(s.dims.as_ref().unwrap().at_once, 27);
        assert!(s.covered());
        // Scrolling back up is the next interval.
        for top in (0..270).rev() {
            feed(&mut w, &frame(top, 30, "alice"));
        }
        w.checkpoint("up");
        let s = w.last().unwrap();
        assert_eq!((s.from.as_str(), s.arrived, s.blinks), ("down", 270, 0));
        assert!(s.moved_up && !s.moved_down);
    }

    fn lagged(owner_from: u64) -> (DetailsWatch, usize) {
        // One new row (index 30, a plain file after f00029) arrives at frame 1 and gets its owner `owner_from` frames later.
        let mut w = DetailsWatch::default();
        feed(&mut w, &frame(0, 30, "alice"));
        w.checkpoint("before");
        for k in 0..=owner_from.max(4) {
            let mut f = frame(1, 30, "alice");
            f.rows.last_mut().unwrap().cells[1] = if k >= owner_from { "bob".into() } else { String::new() };
            feed(&mut w, &f);
        }
        w.checkpoint("x");
        (w, 0)
    }

    #[test]
    fn an_owner_two_frames_late_is_soon_and_one_three_frames_late_is_late() {
        let (w, _) = lagged(2);
        let o = w.last().unwrap().owner.clone().unwrap();
        assert_eq!((o.at_once, o.soon, o.late, o.seen), (0, 1, 0, 1));
        let (w, _) = lagged(3);
        let o = w.last().unwrap().owner.clone().unwrap();
        assert_eq!((o.at_once, o.soon, o.late, o.max_late, o.seen), (0, 0, 1, 3, 1));
        let (w, _) = lagged(5);
        let o = w.last().unwrap().owner.clone().unwrap();
        assert_eq!((o.late, o.max_late, o.seen), (1, 5, 1));
    }

    #[test]
    fn a_row_that_leaves_empty_is_left_early_and_one_waiting_at_a_checkpoint_is_pending() {
        let mut w = DetailsWatch::default();
        feed(&mut w, &frame(0, 30, "alice"));
        // Row 30 arrives empty, row 31 arrives and is still there.
        let mut f = frame(1, 30, "alice");
        f.rows.last_mut().unwrap().cells[1] = String::new();
        feed(&mut w, &f);
        // Scrolled on: row 30 is still there at the bottom, then it goes back out of view the other way.
        feed(&mut w, &frame(0, 30, "alice"));
        w.checkpoint("x");
        let o = w.last().unwrap().owner.clone().unwrap();
        // Row 0 came back into view with its owner: the only arrival that got a value.
        assert_eq!((o.left_early, o.pending, o.late, o.seen), (1, 0, 0, 1));
        let mut w = DetailsWatch::default();
        feed(&mut w, &frame(0, 30, "alice"));
        let mut f = frame(1, 30, "alice");
        f.rows.last_mut().unwrap().cells[1] = String::new();
        feed(&mut w, &f);
        feed(&mut w, &f);
        w.checkpoint("x");
        let o = w.last().unwrap().owner.clone().unwrap();
        assert_eq!((o.pending, o.left_early, o.late), (1, 0, 0));
    }

    #[test]
    fn a_visible_value_going_blank_is_a_blink_and_a_different_one_is_a_change() {
        let mut w = DetailsWatch::default();
        feed(&mut w, &frame(0, 5, "alice"));
        feed(&mut w, &frame(0, 5, ""));
        feed(&mut w, &frame(0, 5, "alice"));
        w.checkpoint("x");
        let s = w.last().unwrap();
        // The first row is an image, so its Dimensions stay; every row's Owner blinked.
        assert_eq!(s.blinks, 5);
        assert_eq!(s.blink_examples[0], "f00000.png Owner `alice` → blank at frame 2");
        assert_eq!(s.changed, 0);
        let mut w = DetailsWatch::default();
        feed(&mut w, &frame(0, 3, "alice"));
        feed(&mut w, &frame(0, 3, "bob"));
        w.checkpoint("x");
        let s = w.last().unwrap();
        assert_eq!((s.blinks, s.changed), (0, 3));
        assert_eq!(s.changed_examples[0], "f00000.png Owner `alice` → `bob` at frame 2");
    }

    #[test]
    fn a_scope_change_is_an_interruption_and_a_missing_frame_is_a_gap() {
        let mut w = DetailsWatch::default();
        feed(&mut w, &frame(0, 5, "alice"));
        feed(&mut w, &frame(0, 5, ""));
        w.observe(scope("/e"), 0, Some(&frame(0, 5, "alice")));
        w.observe(scope("/e"), 0, None);
        w.observe(scope("/e"), 0, None);
        w.checkpoint("x");
        let s = w.last().unwrap();
        assert_eq!((s.interruptions, s.blinks, s.gaps, s.frames), (1, 5, 2, 5));
        // Rows of the new scope are not arrivals: there was nothing to compare with.
        assert_eq!(s.arrived, 0);
    }

    #[test]
    fn a_checkpoint_freezes_the_interval() {
        let mut w = DetailsWatch::default();
        assert!(w.last().is_none());
        feed(&mut w, &frame(0, 5, "alice"));
        w.checkpoint("a");
        let frozen = w.last().unwrap().clone();
        feed(&mut w, &frame(1, 5, "alice"));
        feed(&mut w, &frame(2, 5, "alice"));
        assert_eq!(w.last(), Some(&frozen));
        w.checkpoint("b");
        let s = w.last().unwrap();
        assert_eq!((s.from.as_str(), s.to.as_str(), s.frames, s.arrived), ("a", "b", 2, 2));
    }

    #[test]
    fn dimensions_lag_only_for_images_and_a_hidden_column_is_none() {
        let mut w = DetailsWatch::default();
        feed(&mut w, &frame(0, 30, "alice"));
        // Row 30 is an image whose Dimensions are not there yet; row 31 is plain.
        let mut f = frame(1, 30, "alice");
        let blank = |c: &mut MetaCell| c.cells[0].clear();
        f.rows.iter_mut().filter(|r| r.image).for_each(blank);
        feed(&mut w, &f);
        feed(&mut w, &f);
        w.checkpoint("x");
        let s = w.last().unwrap();
        assert_eq!(s.images, 1);
        let d = s.dims.as_ref().unwrap();
        assert_eq!((d.at_once, d.soon, d.pending), (0, 0, 1));
        let mut w = DetailsWatch::default();
        let hidden = |top: usize| MetaFrame { shown: [false, true, true, true], ..frame(top, 30, "alice") };
        w.observe(Scope { shown: [false, true, true, true], ..scope("/d") }, 0, Some(&hidden(0)));
        w.observe(Scope { shown: [false, true, true, true], ..scope("/d") }, 0, Some(&hidden(1)));
        w.checkpoint("x");
        let s = w.last().unwrap();
        assert!(s.dims.is_none() && s.owner.is_some());
        assert!(s.covered());
    }

    #[test]
    fn nothing_is_covered_without_arrivals() {
        let mut w = DetailsWatch::default();
        feed(&mut w, &frame(0, 5, "alice"));
        feed(&mut w, &frame(0, 5, "alice"));
        w.checkpoint("x");
        assert!(!w.last().unwrap().covered());
    }
}
