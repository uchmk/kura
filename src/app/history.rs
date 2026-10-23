//! Bookmarks and the jump history.

use super::*;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Bookmark {
    pub key: String,
    pub path: PathBuf,
    #[serde(default)]
    pub name: String,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BookmarkOp {
    Save,
    Jump,
    Delete,
}

#[derive(Default, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub(super) struct BookmarkFile {
    #[serde(default)]
    pub(super) bookmark: Vec<Bookmark>,
}

// --------------------------------------------------------------- jump history

/// One directory `z` can jump to, with what it takes to rank it: how often it
/// has been visited and when it last was.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Visit {
    pub path: PathBuf,
    pub hits: u32,
    /// Seconds since the epoch, or 0 for a visit read from a history file
    /// written before this was recorded.
    pub at: i64,
}

/// How highly a directory ranks in `z`: how often it has been visited, weighted
/// by how long ago that last was. The shape is zoxide's — the point of it is
/// that a directory visited twenty times last month does not outrank the one
/// being worked in today.
pub(super) fn frecency(hits: u32, age_secs: i64) -> f64 {
    let hits = hits as f64;
    // A clock that has gone backwards falls into the first arm, which is where
    // a just-visited directory belongs anyway.
    if age_secs < HOUR {
        hits * 4.0
    } else if age_secs < DAY {
        hits * 2.0
    } else if age_secs < 7 * DAY {
        hits * 0.5
    } else {
        hits * 0.25
    }
}

pub(super) const HOUR: i64 = 3_600;
pub(super) const DAY: i64 = 24 * HOUR;

/// How long ago, for the second column of the jump list. Deliberately coarse:
/// this is to tell today's directory from last month's, not to the minute.
pub(super) fn ago(secs: i64) -> String {
    if secs < 0 {
        String::new()
    } else if secs < 60 {
        "just now".into()
    } else if secs < HOUR {
        format!("{}m ago", secs / 60)
    } else if secs < DAY {
        format!("{}h ago", secs / HOUR)
    } else if secs < 30 * DAY {
        format!("{}d ago", secs / DAY)
    } else {
        format!("{}mo ago", secs / (30 * DAY))
    }
}

/// Now, as the history file counts it.
pub(super) fn epoch_secs() -> i64 {
    chrono::Utc::now().timestamp()
}

/// `history.toml`: the folders visited, `entries = [{ path, hits, at }, …]`.
#[derive(Serialize, Deserialize, Default)]
pub(super) struct HistoryFile {
    #[serde(default)]
    pub(super) entries: Vec<VisitRow>,
}

#[derive(Serialize, Deserialize)]
pub(super) struct VisitRow {
    pub(super) path: String,
    pub(super) hits: u32,
    pub(super) at: i64,
}

/// `search-history.toml` and `shell-history.toml`: `entries = ["…", …]`.
#[derive(Serialize, Deserialize, Default)]
pub(super) struct InputFile {
    #[serde(default)]
    pub(super) entries: Vec<String>,
}

/// The files the app keeps between runs, each with its `.txt` of an older
/// build (read once, never deleted).
pub(super) struct Kept {
    pub(super) history: ito_common::kept::Writer,
    pub(super) input: [ito_common::kept::Writer; 2],
}

impl Kept {
    pub(super) fn new() -> Self {
        // A test must not write the real state directory.
        let on = !cfg!(test);
        let dir = Config::state_dir();
        let w = |name: &str| ito_common::kept::Writer::new(dir.join(name), on);
        Self { history: w("history.toml"), input: [w("search-history.toml"), w("shell-history.toml")] }
    }
}

pub(super) fn visit_of(r: VisitRow) -> Visit {
    Visit { path: PathBuf::from(r.path), hits: r.hits.max(1), at: r.at }
}

/// One line of the old `history.txt`: `path`, a tab, the hit count, a tab, the
/// time. A line with no counts is one a version before this
/// wrote: it is a real visit, so it keeps a hit, and its age is unknown, which
/// leaves it ranked below anything visited since.
pub(super) fn parse_visit(line: &str) -> Visit {
    let mut fields = line.split('\t');
    let path = PathBuf::from(fields.next().unwrap_or_default());
    let hits = fields.next().and_then(|s| s.parse().ok()).unwrap_or(1).max(1);
    let at = fields.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    Visit { path, hits, at }
}
