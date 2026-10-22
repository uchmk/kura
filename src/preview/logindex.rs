//! A line index for a text file too big to hold: where every line starts, built
//! on a thread by reading the file once from the front, so that any run of lines
//! can then be read by seeking to it. No memory map: a file on a network drive
//! that is cut short, or one another program holds open, fails a read with an
//! error here and not the whole process.
//!
//! The same index serves the find in the preview, which looks through the file
//! in windows of whole lines (see [`LogIndex::find`]).

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use ito_match::Matcher;

/// How much of the file one read takes when the index is built.
const CHUNK_BYTES: usize = 64 * 1024;
/// How much text one window of the find holds, and how much of the end of it
/// the next window starts again from, so that a match across lines near the
/// edge is seen whole.
const WINDOW_BYTES: u64 = 1024 * 1024;
const OVERLAP_BYTES: u64 = 64 * 1024;
/// A line is cut to this many bytes when read for the screen.
const MAX_LINE_BYTES: usize = 8 * 1024;
/// The most lines a find keeps.
pub const MAX_HITS: usize = 1_000_000;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Enc {
    Utf8,
    Utf16Le,
    Utf16Be,
}

/// What [`LogIndex::grow`] found.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Grow {
    Same,
    Grew,
    Rebuild,
}

#[derive(Default)]
struct Inner {
    /// Where each line starts. While the index is building the last entry may be
    /// the end of what has been read so far.
    starts: Vec<u64>,
    /// How far into the file the build has read.
    end: u64,
    /// The file ended in a newline, so the start that followed it was dropped.
    nl_end: bool,
}

pub struct LogIndex {
    pub path: PathBuf,
    pub enc: Enc,
    /// The file's length when the build last began.
    len: AtomicU64,
    inner: Mutex<Inner>,
    done: AtomicBool,
    cancel: AtomicBool,
}

impl LogIndex {
    /// Begin indexing `path` on a thread of its own and return at once.
    pub fn start(path: &Path) -> Result<Arc<Self>, String> {
        let mut f = File::open(path).map_err(|e| crate::spot::reason(&e))?;
        let len = f.metadata().map_err(|e| crate::spot::reason(&e))?.len();
        let mut head = [0u8; 3];
        let n = f.read(&mut head).map_err(|e| crate::spot::reason(&e))?;
        let (enc, bom) = match &head[..n] {
            [0xff, 0xfe, ..] => (Enc::Utf16Le, 2),
            [0xfe, 0xff, ..] => (Enc::Utf16Be, 2),
            [0xef, 0xbb, 0xbf] => (Enc::Utf8, 3),
            _ => (Enc::Utf8, 0),
        };
        let idx = Arc::new(Self {
            path: path.to_owned(),
            enc,
            len: AtomicU64::new(len),
            inner: Mutex::new(Inner { starts: vec![bom], end: bom, nl_end: false }),
            done: AtomicBool::new(false),
            cancel: AtomicBool::new(false),
        });
        let me = idx.clone();
        std::thread::Builder::new().name("log-index".into()).spawn(move || me.build(f, bom)).map_err(|e| e.to_string())?;
        Ok(idx)
    }

    fn build(&self, mut f: File, from: u64) {
        let mut buf = vec![0u8; CHUNK_BYTES];
        let mut pos = from;
        if f.seek(SeekFrom::Start(from)).is_err() {
            self.finish(pos);
            return;
        }
        loop {
            if self.cancel.load(Ordering::Relaxed) {
                return;
            }
            let mut got = 0;
            while got < buf.len() {
                match f.read(&mut buf[got..]) {
                    Ok(0) => break,
                    Ok(n) => got += n,
                    Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                    Err(_) => break,
                }
            }
            if got == 0 {
                break;
            }
            let mut found = Vec::new();
            match self.enc {
                Enc::Utf8 => {
                    for (i, b) in buf[..got].iter().enumerate() {
                        if *b == b'\n' {
                            found.push(pos + i as u64 + 1);
                        }
                    }
                }
                Enc::Utf16Le | Enc::Utf16Be => {
                    let (nl, zero) = if self.enc == Enc::Utf16Le { (0usize, 1usize) } else { (1, 0) };
                    for (i, pair) in buf[..got].as_chunks::<2>().0.iter().enumerate() {
                        if pair[nl] == b'\n' && pair[zero] == 0 {
                            found.push(pos + i as u64 * 2 + 2);
                        }
                    }
                }
            }
            pos += got as u64;
            let mut g = self.inner.lock().unwrap();
            g.starts.extend(found);
            g.end = pos;
            drop(g);
            if got < buf.len() {
                break;
            }
        }
        self.finish(pos);
    }

    fn finish(&self, pos: u64) {
        let mut g = self.inner.lock().unwrap();
        g.end = pos;
        // A file that ends in a newline has no line after it.
        g.nl_end = g.starts.last().is_some_and(|&s| s >= pos);
        if g.nl_end {
            g.starts.pop();
        }
        drop(g);
        self.done.store(true, Ordering::Release);
    }

    pub fn is_done(&self) -> bool {
        self.done.load(Ordering::Acquire)
    }

    /// Stop the build, and any find, for good: the file is no longer on show.
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    /// The lines known so far: all of them once [`Self::is_done`].
    pub fn lines(&self) -> usize {
        self.inner.lock().unwrap().starts.len()
    }

    /// How much of the file the build has read, in 0..=1.
    pub fn progress(&self) -> f32 {
        let len = self.len();
        if self.is_done() || len == 0 {
            return 1.0;
        }
        (self.inner.lock().unwrap().end as f32 / len as f32).min(1.0)
    }

    /// The file's length when the build last began.
    pub fn len(&self) -> u64 {
        self.len.load(Ordering::Relaxed)
    }

    /// Index what was appended to the file since the build finished, on a thread
    /// of its own. A file that is shorter than what was indexed has been cut or
    /// replaced: that is [`Grow::Rebuild`], and the caller starts over with a
    /// new index.
    pub fn grow(self: &Arc<Self>) -> Grow {
        if !self.is_done() {
            return Grow::Same;
        }
        let Ok(mut f) = File::open(&self.path) else { return Grow::Same };
        let Ok(len) = f.metadata().map(|m| m.len()) else { return Grow::Same };
        let mut g = self.inner.lock().unwrap();
        if len < g.end {
            return Grow::Rebuild;
        }
        if len == g.end {
            return Grow::Same;
        }
        let from = g.end;
        if g.nl_end {
            g.starts.push(from);
            g.nl_end = false;
        }
        drop(g);
        self.len.store(len, Ordering::Relaxed);
        self.done.store(false, Ordering::Release);
        let _ = f.seek(SeekFrom::Start(from));
        let me = self.clone();
        if std::thread::Builder::new().name("log-index".into()).spawn(move || me.build(f, from)).is_err() {
            self.done.store(true, Ordering::Release);
            return Grow::Same;
        }
        Grow::Grew
    }

    /// Lines `first..first + count` as text, without their line ends and cut to
    /// a length a screen can show. Fewer when the file ends first.
    pub fn read(&self, first: usize, count: usize) -> Vec<String> {
        let Some((from, to, starts)) = self.span(first, first.saturating_add(count)) else { return Vec::new() };
        let Ok(bytes) = self.read_bytes(from, to) else { return Vec::new() };
        starts
            .windows(2)
            .map(|w| {
                let a = (w[0] - from) as usize;
                let b = ((w[1] - from) as usize).min(bytes.len());
                clip(&self.decode(&bytes[a.min(b)..b]))
            })
            .collect()
    }

    /// The byte range of lines `first..end`, and the start of every one of them
    /// plus the end of the last, all in file offsets.
    fn span(&self, first: usize, end: usize) -> Option<(u64, u64, Vec<u64>)> {
        let g = self.inner.lock().unwrap();
        if first >= g.starts.len() {
            return None;
        }
        let end = end.min(g.starts.len());
        let mut starts = g.starts[first..end].to_vec();
        starts.push(g.starts.get(end).copied().unwrap_or(g.end));
        Some((starts[0], *starts.last().unwrap(), starts))
    }

    fn read_bytes(&self, from: u64, to: u64) -> std::io::Result<Vec<u8>> {
        let mut f = File::open(&self.path)?;
        f.seek(SeekFrom::Start(from))?;
        let mut v = Vec::with_capacity((to - from) as usize);
        f.take(to - from).read_to_end(&mut v)?;
        Ok(v)
    }

    fn decode(&self, b: &[u8]) -> String {
        match self.enc {
            Enc::Utf8 => String::from_utf8_lossy(b).into_owned(),
            enc => {
                let units: Vec<u16> = b
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|c| if enc == Enc::Utf16Be { u16::from_be_bytes([c[0], c[1]]) } else { u16::from_le_bytes([c[0], c[1]]) })
                    .collect();
                String::from_utf16_lossy(&units)
            }
        }
    }

    /// The lines that `matcher` matches somewhere, in order. The file is looked
    /// through in windows of whole lines, each starting again from the end of
    /// the one before, so a pattern across lines (`(?m)^a\nb$`) matches as long
    /// as its match is shorter than the overlap. A match is counted for the line
    /// it starts on and only by the window that owns that line, so one that falls
    /// in an overlap is not counted twice. `done` counts the bytes passed.
    /// Returns what was found so far if `cancel` is set.
    pub fn find(&self, matcher: &Matcher, cancel: &AtomicBool, done: &AtomicU64) -> Vec<u32> {
        let mut hits: Vec<u32> = Vec::new();
        let total = self.lines();
        let mut from = 0usize;
        let mut text = String::new();
        let mut at: Vec<usize> = Vec::new();
        while from < total {
            if cancel.load(Ordering::Relaxed) || self.cancel.load(Ordering::Relaxed) {
                break;
            }
            // Take lines up to a window's worth.
            let (begin, limit) = {
                let g = self.inner.lock().unwrap();
                (g.starts[from], g.starts.get(total).copied().unwrap_or(g.end))
            };
            let mut stop = from + 1;
            {
                let g = self.inner.lock().unwrap();
                while stop < total && g.starts[stop] - begin < WINDOW_BYTES {
                    stop += 1;
                }
            }
            let last = stop >= total;
            let Some((a, b, starts)) = self.span(from, stop) else { break };
            let Ok(bytes) = self.read_bytes(a, b) else { break };
            // The window's lines as one text, with where each begins in it.
            text.clear();
            at.clear();
            for w in starts.windows(2) {
                at.push(text.len());
                text.push_str(&self.decode(&bytes[(w[0] - a) as usize..((w[1] - a) as usize).min(bytes.len())]));
            }
            // This window owns the lines whose start is not within the overlap of its end.
            let own_to = if last {
                stop
            } else {
                let mut o = stop;
                while o > from + 1 && b - starts[o - from - 1] < OVERLAP_BYTES {
                    o -= 1;
                }
                o
            };
            let mut prev = usize::MAX;
            for r in matcher.ranges(&text) {
                let line = at.partition_point(|&s| s <= r.start).saturating_sub(1);
                let abs = from + line;
                if abs >= own_to || line == prev {
                    continue;
                }
                prev = line;
                hits.push(abs as u32);
                if hits.len() >= MAX_HITS {
                    return hits;
                }
            }
            let _ = limit;
            done.store(starts[own_to - from].min(b), Ordering::Relaxed);
            from = own_to;
        }
        hits
    }
}

impl Drop for LogIndex {
    fn drop(&mut self) {
        self.cancel();
    }
}

/// What the pane shows of a big file: the lines around the top of the screen,
/// read by a thread of its own as the pane scrolls (the newest ask wins), and
/// the result of the find in it.
pub struct LogView {
    pub index: Arc<LogIndex>,
    shown: Mutex<Shown>,
    ask: crossbeam_channel::Sender<(usize, usize)>,
    ctx: Mutex<Option<egui::Context>>,
    found: Arc<Mutex<Found>>,
    stop: Mutex<Arc<AtomicBool>>,
}

#[derive(Default)]
struct Shown {
    first: usize,
    lines: Vec<String>,
}

/// The find in a big file: its lines (a line once per match started on it), and
/// whether the look through the file is over.
#[derive(Default, Clone, Debug)]
pub struct Found {
    pub query: String,
    pub lines: Vec<u32>,
    pub done: bool,
    /// Bytes looked through so far.
    pub passed: u64,
}

impl std::fmt::Debug for LogView {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "LogView({})", self.index.path.display())
    }
}

/// Lines read around the ones asked for, so a few steps of scrolling need no read.
const MARGIN: usize = 200;

impl LogView {
    pub fn new(index: Arc<LogIndex>) -> Arc<Self> {
        let (ask, rx) = crossbeam_channel::unbounded::<(usize, usize)>();
        let view = Arc::new(Self {
            index,
            shown: Mutex::new(Shown::default()),
            ask,
            ctx: Mutex::new(None),
            found: Arc::new(Mutex::new(Found::default())),
            stop: Mutex::new(Arc::new(AtomicBool::new(false))),
        });
        let weak = Arc::downgrade(&view);
        std::thread::Builder::new()
            .name("log-view".into())
            .spawn(move || {
                while let Ok(mut want) = rx.recv() {
                    while let Ok(newer) = rx.try_recv() {
                        want = newer;
                    }
                    let Some(v) = weak.upgrade() else { return };
                    let first = want.0.saturating_sub(MARGIN);
                    let lines = v.index.read(first, want.1 + 2 * MARGIN);
                    *v.shown.lock().unwrap() = Shown { first, lines };
                    let ctx = v.ctx.lock().unwrap().clone();
                    if let Some(c) = ctx {
                        c.request_repaint();
                    }
                }
            })
            .expect("spawn log view");
        view
    }

    /// Note that lines `first..first + count` are about to be drawn, and give
    /// back the ones already read: `(first line, lines)`. Asks for a read when
    /// they do not cover the ask.
    pub fn window(&self, first: usize, count: usize, ctx: &egui::Context) -> (usize, Vec<String>) {
        let s = self.shown.lock().unwrap();
        let covered = first >= s.first && (first + count <= s.first + s.lines.len() || s.first + s.lines.len() >= self.index.lines());
        if !covered {
            *self.ctx.lock().unwrap() = Some(ctx.clone());
            let _ = self.ask.send((first, count));
        }
        (s.first, s.lines.clone())
    }

    /// Look for `matcher` through the whole file on a thread; `n`/`N` read the
    /// result from [`Self::found`] as it grows.
    pub fn find(&self, query: &str, matcher: Matcher) {
        let stop = Arc::new(AtomicBool::new(false));
        std::mem::replace(&mut *self.stop.lock().unwrap(), stop.clone()).store(true, Ordering::Relaxed);
        *self.found.lock().unwrap() = Found { query: query.to_owned(), ..Default::default() };
        let (index, found) = (self.index.clone(), self.found.clone());
        let ctx = self.ctx.lock().unwrap().clone();
        let _ = std::thread::Builder::new().name("log-find".into()).spawn(move || {
            let done = AtomicU64::new(0);
            let lines = index.find(&matcher, &stop, &done);
            if stop.load(Ordering::Relaxed) {
                return;
            }
            let mut f = found.lock().unwrap();
            f.lines = lines;
            f.passed = index.len();
            f.done = true;
            drop(f);
            if let Some(c) = ctx {
                c.request_repaint();
            }
        });
    }

    pub fn found(&self) -> Found {
        self.found.lock().unwrap().clone()
    }
}

impl Drop for LogView {
    fn drop(&mut self) {
        self.stop.lock().unwrap().store(true, Ordering::Relaxed);
    }
}

fn clip(s: &str) -> String {
    let s = s.trim_end_matches(['\n', '\r']);
    if s.len() <= MAX_LINE_BYTES {
        return s.to_owned();
    }
    let mut n = MAX_LINE_BYTES;
    while !s.is_char_boundary(n) {
        n -= 1;
    }
    s[..n].to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn wait(i: &LogIndex) {
        for _ in 0..2000 {
            if i.is_done() {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        panic!("index did not finish");
    }

    fn index(label: &str, bytes: &[u8]) -> Arc<LogIndex> {
        let dir = crate::util::test_dir(label);
        let p = dir.join("log.txt");
        std::fs::write(&p, bytes).unwrap();
        let i = LogIndex::start(&p).unwrap();
        wait(&i);
        i
    }

    fn hits(i: &LogIndex, q: &str) -> Vec<u32> {
        i.find(&Matcher::new(q).unwrap(), &AtomicBool::new(false), &AtomicU64::new(0))
    }

    #[test]
    fn lines_are_counted_with_lf_crlf_and_no_final_newline() {
        assert_eq!(index("li-lf", b"a\nb\nc\n").lines(), 3);
        assert_eq!(index("li-crlf", b"a\r\nb\r\n").read(0, 5), vec!["a", "b"]);
        let i = index("li-nofinal", b"a\nb\nc");
        assert_eq!((i.lines(), i.read(2, 3)), (3, vec!["c".to_owned()]));
        assert_eq!(index("li-empty", b"").lines(), 0);
        assert_eq!(index("li-blank", b"\n\n").read(0, 9), vec!["", ""]);
    }

    #[test]
    fn a_window_equals_the_same_lines_of_a_whole_read() {
        let text: String = (0..200_000).map(|n| format!("line {n} {}\n", "x".repeat(n % 17))).collect();
        let i = index("li-window", text.as_bytes());
        let all: Vec<&str> = text.lines().collect();
        assert_eq!(i.lines(), all.len());
        for (first, count) in [(0, 50), (12_345, 80), (199_990, 50), (65_535, 2)] {
            let want: Vec<&str> = all.iter().skip(first).take(count).copied().collect();
            assert_eq!(i.read(first, count), want, "lines {first}+{count}");
        }
    }

    #[test]
    fn utf16_files_are_indexed_on_two_byte_boundaries() {
        for (bom, be) in [([0xff, 0xfe], false), ([0xfe, 0xff], true)] {
            let mut b = bom.to_vec();
            for u in "あ\nい\r\nう".encode_utf16() {
                b.extend(if be { u.to_be_bytes() } else { u.to_le_bytes() });
            }
            let i = index("li-u16", &b);
            assert_eq!(i.read(0, 9), vec!["あ", "い", "う"], "big endian: {be}");
        }
    }

    #[test]
    fn a_utf8_bom_is_not_part_of_the_first_line() {
        assert_eq!(index("li-bom", b"\xef\xbb\xbfhello\nx\n").read(0, 2), vec!["hello", "x"]);
    }

    #[test]
    fn find_gives_the_lines_a_whole_read_gives() {
        let text: String = (0..300_000).map(|n| if n % 1000 == 7 { format!("hit {n}\n") } else { format!("pad {n}\n") }).collect();
        let i = index("li-find", text.as_bytes());
        let want: Vec<u32> = text.lines().enumerate().filter(|(_, l)| l.starts_with("hit")).map(|(n, _)| n as u32).collect();
        assert_eq!(hits(&i, "^hit"), want.to_vec(), "regex with ^ over plain lines");
        assert_eq!(hits(&i, "(?m)^hit"), want);
    }

    #[test]
    fn find_matches_a_pattern_across_lines_and_across_a_window_edge() {
        // Enough lines that the first window ends in the middle of the file.
        let mut text: String = (0..200_000).map(|n| format!("pad {n} ........\n")).collect();
        let at = text.lines().count();
        text.push_str("ERROR boom\n  at frame 1\n  at frame 2\n");
        text.push_str(&(0..10).map(|n| format!("tail {n}\n")).collect::<String>());
        let i = index("li-multi", text.as_bytes());
        assert_eq!(hits(&i, r"(?m)^ERROR.*\n  at frame 1"), vec![at as u32]);
        // Every pad line matches once (the pattern crosses its own newline but
        // not into the next line), wherever the window edges fall.
        let all = hits(&i, r"(?m)^pad \d+ \.+\n");
        assert_eq!(all.len(), at);
        assert!(all.windows(2).all(|w| w[0] + 1 == w[1]));
    }

    #[test]
    fn appended_lines_are_indexed() {
        let dir = crate::util::test_dir("li-grow");
        let path = dir.join("grow.log");
        std::fs::write(&path, "a\nb\n").unwrap();
        let i = LogIndex::start(&path).unwrap();
        while !i.is_done() {
            std::thread::yield_now();
        }
        assert_eq!((i.lines(), i.grow()), (2, Grow::Same));
        // A partial last line, then the rest of it and more.
        std::fs::OpenOptions::new().append(true).open(&path).unwrap().write_all(b"c\nd").unwrap();
        assert_eq!(i.grow(), Grow::Grew);
        while !i.is_done() {
            std::thread::yield_now();
        }
        assert_eq!(i.read(0, 10), ["a", "b", "c", "d"]);
        std::fs::OpenOptions::new().append(true).open(&path).unwrap().write_all(b"e\nf\n").unwrap();
        assert_eq!(i.grow(), Grow::Grew);
        while !i.is_done() {
            std::thread::yield_now();
        }
        assert_eq!(i.read(0, 10), ["a", "b", "c", "de", "f"]);
        std::fs::write(&path, "x\n").unwrap();
        assert_eq!(i.grow(), Grow::Rebuild);
    }

    #[test]
    fn find_stops_when_cancelled() {
        let text: String = (0..50_000).map(|n| format!("row {n}\n")).collect();
        let i = index("li-cancel", text.as_bytes());
        let stop = AtomicBool::new(true);
        assert!(i.find(&Matcher::new("row").unwrap(), &stop, &AtomicU64::new(0)).is_empty());
    }

    #[test]
    fn a_very_long_line_is_cut_for_the_screen() {
        let i = index("li-long", format!("{}\nshort\n", "y".repeat(100_000)).as_bytes());
        let l = i.read(0, 2);
        assert_eq!((l[0].len(), l[1].as_str()), (MAX_LINE_BYTES, "short"));
    }

    #[test]
    #[ignore = "timing: a million lines"]
    fn a_million_lines_index_quickly() {
        let text: String = (0..1_000_000).map(|n| format!("2026-10-22 12:00:00 INFO request {n} handled in {}ms\n", n % 97)).collect();
        let dir = crate::util::test_dir("li-million");
        let p = dir.join("big.log");
        std::fs::write(&p, &text).unwrap();
        let t = std::time::Instant::now();
        let i = LogIndex::start(&p).unwrap();
        wait(&i);
        let built = t.elapsed();
        let t = std::time::Instant::now();
        let l = i.read(777_777, 60);
        let read = t.elapsed();
        let t = std::time::Instant::now();
        let n = hits(&i, "request 999999 ").len();
        eprintln!("{} MB: index {built:?}, window {read:?}, find {:?}", text.len() / 1_000_000, t.elapsed());
        assert_eq!((i.lines(), l.len(), n), (1_000_000, 60, 1));
    }
}
