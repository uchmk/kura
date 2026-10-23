//! Application state and the action dispatcher.
//!
//! Everything the UI does goes through [`Act`], so keys, mouse clicks and
//! internal follow-ups all take the same path.

use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crossbeam_channel::Sender;
use serde::{Deserialize, Serialize};

use crate::config::cmd::{Act, CopyWhat, EscapeWhat, RenameCursor, SearchVia, Step, Tri, ZoomTo};
use crate::config::keys::{Code, Key};
use crate::config::{keymap, Config};
use crate::core::folder::{Filter, Folder, LoadState};
use crate::core::fuzzy;
use crate::core::tab::{CdFallout, Finder, PendingCd, Tab};
use crate::diff;
use crate::exec;
use crate::fs::archive;
use crate::fs::git;
use crate::fs::ops::{self, OpKind, OpRequest, Resolution};
use crate::fs::restore;
use crate::fs::scan::{ScanResult, Scanner};
use crate::fs::{Entry, Kind, SortSpec};
use crate::preview::{self, Payload, Previewer, TocEntry};
use crate::rename;
use crate::spot::{self, Section, Spotter};
use crate::util::{self, Lru};
use ito_common::dirwatch::DirWatcher;

pub const MAX_TABS: usize = 9;

mod overlay;
pub use overlay::*;
mod task;
pub use task::*;
mod preview_state;
pub use preview_state::*;
mod split;
pub use split::*;
mod history;
pub use history::*;
mod undo;
#[cfg(test)]
use self::spot_panel::page_for;
#[cfg(test)]
use self::term_pane::{kura_command, no_osc7, program_label, same_file};
pub use undo::*;
mod bookmarks;
mod compare;
mod completion;
mod disk_usage;
mod dispatch;
mod file_ops;
mod in_archive;
mod input;
mod junctions_undo;
mod keys;
mod nav;
mod panes;
mod persist;
mod preview_pane;
mod renaming;
mod scan;
mod search;
mod settings;
mod spot_panel;
mod term_pane;

// ---------------------------------------------------------------------- app

pub struct Yank {
    pub paths: Vec<PathBuf>,
    pub cut: bool,
    /// Yanked inside an archive (`l`): the paths name members, which `p`
    /// takes out of the archive rather than copying.
    pub from_archive: bool,
}

/// A `Tab` completion in the `cd` prompt that is waiting for its listing.
/// `dir` and `prefix` are what the input line said when the key was pressed,
/// so an answer that arrives over newer text can be told apart and dropped.
struct PendingCompletion {
    id: u64,
    dir: PathBuf,
    prefix: String,
    since: Instant,
}

pub struct App {
    pub cfg: Config,
    pub tabs: Vec<Tab>,
    pub active: usize,
    /// The second pane, when the view is split.
    pub split: Option<Split>,
    pub cache: Lru<PathBuf, Arc<Vec<Entry>>>,

    pub scanner: Scanner,
    pub previewer: Previewer,
    pub ops: ops::Runner,
    pub watcher: DirWatcher,
    pub git: git::Git,
    /// The shell in the bottom pane, while there is one.
    pub term: Option<crate::terminal::Terminal>,
    /// The terminal has the keys, so they go to the shell rather than here.
    pub term_focus: bool,
    /// The file the spot worker was last asked about, so the panel can be
    /// pointed at a new one without asking again every frame.
    pub spot_asked: Option<PathBuf>,
    /// Wheel movement not yet worth a whole row, kept so that it becomes one.
    /// One per surface that scrolls by rows: sharing a single remainder would
    /// make the view jump when the pointer crossed between them mid-turn.
    /// See [`crate::ui::wheel_whole`] for why the remainder has to be kept.
    pub term_scroll_rows: f32,
    pub preview_scroll_rows: f32,
    /// One per place a file list is drawn: the left of the split, and the
    /// middle column (the right of the split). The two lists of a split are
    /// two surfaces, and sharing one remainder let a quarter turn over the
    /// left finish a quarter turn over the right (#202, 19.7).
    pub list_scroll_rows: [f32; 2],
    /// What the terminal was last searched for, so the key repeats it.
    term_needle: String,
    log_needle: String,
    /// The compiled `log_needle`, which the preview marks in the window.
    pub log_matcher: Option<ito_match::Matcher>,
    /// Keep the huge log on show scrolled to its end as it grows.
    pub log_follow: bool,
    log_polled: Option<Instant>,
    /// The shell the pane started, as its first toast named it.
    term_shell: String,
    /// A file to put the cursor on when its directory is next read, ahead of
    /// whatever was hovered: an archive that did not exist a moment ago, so
    /// the listing that is on screen cannot select it yet (Q25). Spent by the
    /// next read of the current directory, so leaving that directory drops it.
    land_on: Option<PathBuf>,
    /// The last jump that failed and was taken back. The parent columns it
    /// asked for fail too, for the same reason, and each one used to add its
    /// own toast naming a fragment of the path (#107).
    cd_refused: Option<PathBuf>,
    /// A drag in flight between the panes.
    pub drag: Option<Drag>,
    /// Where each pane was drawn this frame, so a drop can be placed.
    pub pane_rects: Vec<(usize, egui::Rect)>,
    /// A drag let go this frame, and whether Shift was held: placed once
    /// every pane has been drawn, not by the pane it started in (#208).
    pub drag_released: Option<bool>,
    /// What git says about each directory on screen, by directory.
    git_status: Lru<PathBuf, Arc<git::Status>>,
    pub differ: diff::Differ,
    pub spotter: Spotter,
    /// The spot worker's latest findings, for the file named.
    pub spotted: Option<(PathBuf, Vec<Section>)>,
    /// The answer of a `C` over a selection, built off the UI thread.
    spot_copy_rx: Option<crossbeam_channel::Receiver<Result<usize, String>>>,

    pub pending: Vec<Key>,
    pub which: Vec<(String, String, String)>,
    pub overlay: Overlay,
    pub pending_bookmark: Option<BookmarkOp>,

    pub yank: Yank,
    /// The status bar's clock and the language its words use, from the
    /// shared common.toml (`set_common`); the defaults until then.
    pub clock: ito_common::Clock,
    pub lang: String,
    /// common.toml read again by its watcher each time the file changes
    /// (`load_common`), so another uchmk app's settings screen reaches here.
    common_rx: Option<crossbeam_channel::Receiver<CommonRead>>,
    /// The same channel's other end, for the settings screen's writes: what
    /// it wrote is read back and applied at once, not at the next look.
    common_tx: Option<crossbeam_channel::Sender<CommonRead>>,
    /// The `uchmk` folder common.toml is in, and what it said last.
    pub common_base: Option<PathBuf>,
    pub common: ito_common::Common,
    /// The themes common.toml's `theme` chooses among (Q98), read again
    /// with common.toml; and what was said last about one that does not read.
    pub themes: Vec<ito_theme::Theme>,
    themes_bad: Vec<String>,
    themes_rx: crossbeam_channel::Receiver<Themes>,
    themes_tx: crossbeam_channel::Sender<Themes>,
    /// Whether the OS is in light mode, for `theme = "system"`; the frame
    /// loop keeps it.
    pub os_light: bool,
    /// The last problem said about common.toml's `theme`, to say it once.
    theme_said: Option<String>,
    /// What went wrong opening config.toml from the settings screen, said
    /// from the thread that tried.
    /// `(is an error, text)`.
    settings_rx: crossbeam_channel::Receiver<(bool, String)>,
    settings_tx: crossbeam_channel::Sender<(bool, String)>,
    /// The thread that writes config.toml for the settings screen, started at
    /// its first write; one thread, so two quick changes land in order.
    kura_jobs: Option<Sender<KuraJob>>,
    /// What it wrote, with the config read back after.
    kura_rx: crossbeam_channel::Receiver<KuraWritten>,
    kura_tx: Sender<KuraWritten>,
    /// When that write was read back: the watcher sees the same change a
    /// moment later, and its reload then says nothing the write did not.
    kura_wrote: Option<Instant>,
    pub preview: PreviewSlot,
    pub max_preview: bool,
    /// The terminal pane has the window. A third of the height is right for a
    /// shell and too little for a full-screen program, so this is the way out.
    pub max_term: bool,
    /// The terminal pane's share of the window's height, dragged by the border
    /// above it (Q94). Kept while the window is open, not written anywhere:
    /// the next window starts at 35% again.
    pub term_share: f32,
    /// The quick-look panel is up: the hovered file, big, over the panes.
    pub quick: bool,
    pub hide_parent: bool,
    /// Markdown is shown rendered rather than as source.
    pub render_markdown: bool,
    /// A bold face was loaded as the `bold` font family.
    pub bold_font: bool,
    /// The fonts named in the config have changed and must be installed again.
    /// Only `main` can do that, so it is left as a flag for the frame loop.
    pub refont: bool,

    pub tasks: Vec<Task>,
    pub toasts: Vec<Toast>,
    /// Every toast raised, newest last, at most [`TOAST_LOG`] of them --
    /// including those that have already faded. `KURA_KEYS_DONE` writes it
    /// out, so a check whose expected result is a toast does not have to
    /// catch it on screen before it goes (#176, #190, #196).
    pub toast_log: std::collections::VecDeque<String>,
    /// What the bell in the title bar lists: every toast raised, oldest
    /// first, at most [`NOTICES`] of them, unread until the list is opened on
    /// it. The toasts fade; these stay.
    pub notices: Vec<ito_notices::Notice<()>>,
    next_notice: u64,
    pub bell_open: bool,
    /// Where the bell was drawn last frame, for the list to hang from.
    pub bell_at: egui::Rect,
    /// The last keys pressed, rendered, newest last, for `<F12>`'s report
    /// (Q64). Keys only: text typed into a prompt is not kept.
    pub recent_keys: std::collections::VecDeque<String>,
    /// The newest error raised, for the same report.
    pub last_error: Option<String>,
    /// The report link `<F12>` last opened or copied, for the state file.
    pub last_report: Option<String>,
    /// The newest line [`App::launch`] ran, after the PID of the process it
    /// started (`-` when there is none to give), for the state file.
    pub last_launch: Option<String>,
    /// The parent column's rows as the last frame drew them, cut to its
    /// width, for the state file; empty while it is not drawn.
    pub parent_shown: Vec<String>,
    /// Openers still young enough to fail on us; drained in
    /// [`App::drain_channels`].
    launches: Vec<(exec::Launch, String, String, String)>,
    pub bookmarks: Vec<Bookmark>,
    /// Where `z` can jump, in visit order (newest last). Ranked by [`frecency`]
    /// when the picker opens.
    pub history: Vec<Visit>,
    /// What was typed into the prompts that take a search word and the ones that
    /// take a shell command, oldest first: `<Up>` brings them back, as a terminal does.
    pub input_history: [Vec<String>; 2],
    kept: Kept,
    /// Which of them the prompt is showing, and what was being typed before the
    /// first `<Up>`, so `<Down>` past the newest gives it back.
    input_recall: Option<(usize, String)>,
    pub undos: Undos,
    /// Jobs whose step joins a stack once they finish, and where it goes. A job
    /// that fails puts its step back rather than losing it.
    op_undo: HashMap<u64, (UndoStep, Land)>,
    /// What each running `D` is deleting, for the toast its end replaces.
    deleting: HashMap<u64, Vec<PathBuf>>,
    /// What each running cut paste emptied the register of, to put back when
    /// nothing moved (Q72: a declined overwrite used to lose the cut).
    cut_jobs: HashMap<u64, Vec<PathBuf>>,

    pub search: Option<crate::search::Handle>,
    /// The disk-usage walk, while one is running. Dropping it stops the walk, so
    /// leaving the view is all it takes to end one.
    pub usage: Option<crate::fs::usage::Handle>,
    /// The biggest total in the usage view, which the rows' bars are drawn
    /// against. Zero when the view is not up.
    pub usage_max: u64,
    /// Where `gu` was pressed. `h` climbs inside the view down to here, and
    /// leaves it from here.
    usage_root: Option<PathBuf>,
    /// The archive `l` went into (Q75), while its members are on screen.
    archive_view: Option<ArchiveView>,
    /// A listing or a member copy on its way from a worker; see
    /// [`App::drain_archive`].
    archive_rx: Option<crossbeam_channel::Receiver<ArchiveMsg>>,
    /// A member being unpacked for the preview, apart so that it never
    /// stands in the way of the listing or of `l` opening a copy.
    archive_preview_rx: Option<crossbeam_channel::Receiver<ArchiveMsg>>,
    /// The folder `h` came up out of, put under the cursor once the parent's
    /// walk reports it.
    usage_want: Option<String>,
    /// The tab's linemode from before `gu`, put back on the way out. The view
    /// shows sizes whatever the tab was showing (Q33): its bars alone could
    /// not be read as numbers, and `linemode = "usage"` in the config made every
    /// ordinary folder read `0 B` instead.
    usage_linemode: Option<crate::fs::entry::Linemode>,
    /// What `<A-t>` asked to type into a pane it had to open first, held until
    /// the shell has drawn something (Q35); see [`App::pump_terminal`].
    term_pending: Option<(Vec<u8>, Instant)>,
    pub ctx: egui::Context,
    /// How much bigger everything is drawn. Held here rather than read back
    /// from egui: `set_zoom_factor` only takes effect at the start of the next
    /// frame, so stepping from what egui currently reports would lose every
    /// press after the first in any one frame.
    pub scale: f32,
    /// `KURA_SCALE` set the scale for this run, so common.toml's is neither
    /// written nor followed.
    scale_pinned: bool,
    /// When a zoom key was last pressed. common.toml read back within a few
    /// seconds of that can hold a step the writer has not reached yet.
    scale_pressed: Option<Instant>,
    /// The newest scale, for the thread that writes it to common.toml.
    scale_tx: Option<Sender<f32>>,
    /// What [`App::start_watching`] saw change, read off the UI thread.
    watched_rx: Option<crossbeam_channel::Receiver<Watched>>,
    watched_tx: Option<Sender<Watched>>,
    /// A copy job is blocked waiting for the new name being typed.
    pending_conflict: Option<Sender<Resolution>>,

    dirty: HashMap<PathBuf, Instant>,
    /// Directories whose child count has already been asked for.
    counted: std::collections::HashSet<PathBuf>,
    /// A path completion waiting on the scan pool. Only the newest one
    /// answers; anything the user typed over is dropped.
    pending_completion: Option<PendingCompletion>,
    /// Scans in flight, so results can be routed back to the right folder.
    inflight: HashMap<u64, PathBuf>,
    pub quit: bool,
    pub cwd_file: Option<PathBuf>,
    pub chooser_file: Option<PathBuf>,
    /// The first line of the help panel drawn. A scroll position, not a cursor,
    /// so the last useful value is the one that puts the last line at the bottom
    /// of the panel rather than at the top.
    pub help_scroll: usize,
    /// Lines the help panel can show, and how many it has. Set by the renderer,
    /// the way `page_rows` is for the file list and `rows` for the comparison:
    /// without them the keys cannot tell where a page ends or where scrolling
    /// stops, and `help_scroll` ran off past the end of the list -- every press
    /// back up then moved a number nothing was drawing from, so the keys looked
    /// dead for as many presses as the reader had overshot.
    pub help_rows: usize,
    pub help_lines: usize,
    /// The wheel's leftover fraction of a row over the help panel.
    pub help_scroll_rows: f32,
    pub last_action: Instant,
}

impl App {
    pub fn new(cfg: Config, start: PathBuf, ctx: egui::Context) -> Self {
        let wake = {
            let ctx = ctx.clone();
            move || ctx.request_repaint()
        };
        // yazi's `micro_workers` is the nearest equivalent knob for "small IO jobs".
        let threads = (cfg.yazi.tasks.micro_workers as usize).clamp(2, 6);
        let scanner = Scanner::new(threads, wake.clone());
        let previewer = Previewer::new(wake.clone());
        let opsr = ops::Runner::new(wake.clone());
        let spotter = Spotter::new(wake.clone());
        let differ = diff::Differ::new(wake.clone());
        let git = git::Git::new(wake.clone());
        let watcher = DirWatcher::new(wake);

        let sort = SortSpec {
            by: cfg.yazi.mgr.sort_by,
            reverse: cfg.yazi.mgr.sort_reverse,
            dir_first: cfg.yazi.mgr.sort_dir_first,
            sensitive: cfg.yazi.mgr.sort_sensitive,
        };
        let tab = Tab::new(start, sort, cfg.yazi.mgr.show_hidden, cfg.yazi.mgr.linemode);
        let render_markdown = cfg.ui.render_markdown;

        let (settings_tx, settings_rx) = crossbeam_channel::unbounded();
        let (kura_tx, kura_rx) = crossbeam_channel::unbounded();
        let (themes_tx, themes_rx) = crossbeam_channel::unbounded();
        let mut app = Self {
            clock: ito_common::Clock::default(),
            lang: "en".into(),
            common_rx: None,
            common_tx: None,
            common_base: None,
            common: ito_common::Common::default(),
            themes: ito_theme::builtin(),
            themes_bad: Vec::new(),
            themes_rx,
            themes_tx,
            os_light: false,
            theme_said: None,
            settings_rx,
            settings_tx,
            kura_jobs: None,
            kura_rx,
            kura_tx,
            kura_wrote: None,
            cfg,
            tabs: vec![tab],
            active: 0,
            split: None,
            cache: Lru::new(64),
            scanner,
            previewer,
            ops: opsr,
            watcher,
            git,
            // One per pane, plus the few a walk just left behind.
            git_status: Lru::new(8),
            term: None,
            term_focus: false,
            spot_asked: None,
            term_scroll_rows: 0.0,
            preview_scroll_rows: 0.0,
            list_scroll_rows: [0.0; 2],
            term_needle: String::new(),
            log_needle: String::new(),
            log_matcher: None,
            log_follow: false,
            log_polled: None,
            term_shell: String::new(),
            land_on: None,
            cd_refused: None,
            drag: None,
            pane_rects: Vec::new(),
            drag_released: None,
            differ,
            spotter,
            spotted: None,
            spot_copy_rx: None,
            pending: Vec::new(),
            which: Vec::new(),
            overlay: Overlay::None,
            pending_bookmark: None,
            yank: Yank { paths: Vec::new(), cut: false, from_archive: false },
            preview: PreviewSlot::default(),
            max_preview: false,
            max_term: false,
            term_share: crate::ui::TERM_SHARE,
            quick: false,
            hide_parent: false,
            render_markdown,
            bold_font: false,
            refont: false,
            tasks: Vec::new(),
            toasts: Vec::new(),
            toast_log: std::collections::VecDeque::new(),
            notices: Vec::new(),
            next_notice: 1,
            bell_open: false,
            bell_at: egui::Rect::NOTHING,
            recent_keys: std::collections::VecDeque::new(),
            last_error: None,
            last_report: None,
            last_launch: None,
            parent_shown: Vec::new(),
            launches: Vec::new(),
            bookmarks: Vec::new(),
            history: Vec::new(),
            input_history: Default::default(),
            kept: Kept::new(),
            input_recall: None,
            undos: Undos::default(),
            op_undo: HashMap::new(),
            cut_jobs: HashMap::new(),
            deleting: HashMap::new(),
            search: None,
            usage: None,
            usage_max: 0,
            usage_linemode: None,
            usage_root: None,
            archive_view: None,
            archive_rx: None,
            archive_preview_rx: None,
            usage_want: None,
            term_pending: None,
            ctx,
            scale: 1.0,
            scale_pinned: false,
            scale_pressed: None,
            scale_tx: None,
            watched_rx: None,
            watched_tx: None,
            pending_conflict: None,
            dirty: HashMap::new(),
            counted: std::collections::HashSet::new(),
            pending_completion: None,
            inflight: HashMap::new(),
            quit: false,
            cwd_file: None,
            chooser_file: None,
            help_scroll: 0,
            help_rows: 0,
            help_lines: 0,
            help_scroll_rows: 0.0,
            last_action: Instant::now(),
        };
        app.load_state();
        app.kick_scans();
        // A config problem that nobody is told about is one the reader spends
        // the evening blaming the program for. The `~` panel lists them all;
        // this is the line that says to go and look.
        if let Some(text) = config_toast(&app.cfg.warnings) {
            app.warn(text);
        }
        app
    }

    // ------------------------------------------------------------- accessors

    pub fn tab(&self) -> &Tab {
        &self.tabs[self.active]
    }

    /// The tab in the pane that does not have the keys.
    pub fn other_pane(&self) -> Option<usize> {
        self.split.map(|s| s.other)
    }

    /// Every tab on screen: the focused one, plus the other pane's.
    fn pane_tabs(&self) -> Vec<usize> {
        match self.other_pane() {
            Some(i) if i != self.active && i < self.tabs.len() => vec![self.active, i],
            _ => vec![self.active],
        }
    }

    pub fn toast(&mut self, text: impl Into<String>) {
        self.raise(text.into(), Level::Info);
    }

    /// Something the reader should see, about work that was not stopped.
    pub fn warn(&mut self, text: impl Into<String>) {
        self.raise(text.into(), Level::Warn);
    }

    pub fn error(&mut self, text: impl Into<String>) {
        let text = text.into();
        self.last_error = Some(text.clone());
        self.raise(text, Level::Error);
    }

    /// Keep `k` among the last keys pressed.
    fn note_key(&mut self, k: &Key) {
        if self.recent_keys.len() == RECENT_KEYS {
            self.recent_keys.pop_front();
        }
        self.recent_keys.push_back(keymap_render(k));
    }

    /// A toast that supersedes the others of its family -- those starting
    /// with `family` -- rather than standing beside them. `At the last
    /// difference` and `At the first difference` side by side contradicted
    /// each other; the newer is the true one.
    fn toast_instead(&mut self, family: &[&str], text: String) {
        self.toasts.retain(|t| t.text == text || !family.iter().any(|f| t.text.starts_with(f)));
        self.toast(text);
    }

    /// Put a line up, or tick the one already saying it.
    ///
    /// Repeating rather than stacking: the same text arriving again is the same
    /// news, and stacking it spends the five slots the toast area has on one
    /// message. The timer restarts so a repeat stays up as long as a first.
    fn raise(&mut self, text: String, level: Level) {
        if self.toast_log.len() == TOAST_LOG {
            self.toast_log.pop_front();
        }
        self.toast_log.push_back(text.clone());
        self.note(&text, level);
        if let Some(t) = self.toasts.iter_mut().rev().find(|t| t.text == text && t.level == level) {
            t.count += 1;
            t.at = Instant::now();
            return;
        }
        self.toasts.push(Toast { text, level, at: Instant::now(), count: 1 });
    }

    /// Add `text` to the bell's list, or refresh the unread notice already
    /// saying it (the same news again is not a second line).
    fn note(&mut self, text: &str, level: Level) {
        let kind = match level {
            Level::Info => ito_notices::Kind::Info,
            Level::Warn => ito_notices::Kind::Warn,
            Level::Error => ito_notices::Kind::Err,
        };
        let now = std::time::SystemTime::now();
        if let Some(n) = self.notices.iter_mut().rev().find(|n| !n.read && n.kind == kind && n.title == text) {
            n.at = now;
            return;
        }
        if self.notices.len() == NOTICES {
            self.notices.remove(0);
        }
        let id = self.next_notice;
        self.next_notice += 1;
        self.notices.push(ito_notices::Notice { id, at: now, kind, title: text.to_owned(), note: None, go: None, read: false });
    }
}

/// The commands that write to the disk, refused inside an archive.
fn changes_files(a: &Act) -> bool {
    matches!(
        a,
        // `y` is allowed: it is how a member is taken out (`p` in a folder).
        // `x` would take it out of the archive too, which this never writes.
        Act::Yank { cut: true }
            | Act::Paste { .. }
            | Act::Link { .. }
            | Act::Hardlink
            | Act::Remove { .. }
            | Act::Create { .. }
            | Act::Rename { .. }
            | Act::BulkRename
            | Act::Extract
            | Act::Compress
            | Act::SendPane { .. }
    )
}

/// What the refusal says: the commands that would rewrite the archive (cut,
/// remove, create, rename, paste, links) are read-only by design, the rest are
/// not offered inside one yet (#252).
fn archive_refusal(a: &Act) -> &'static str {
    match a {
        Act::Remove { .. }
        | Act::Create { .. }
        | Act::Rename { .. }
        | Act::BulkRename
        | Act::Yank { cut: true }
        | Act::Paste { .. }
        | Act::Link { .. }
        | Act::Hardlink => {
            "Inside an archive: read only — this would change the archive, which kura never writes. y then p in a folder takes a copy out, Esc leaves"
        }
        _ => "Inside an archive: not available here yet — y then p in a folder takes a copy out, Esc leaves",
    }
}

/// How many entries of an archive are listed before stopping; the rest are
/// said to be there.
const ARCHIVE_LIMIT: usize = 100_000;

/// The archive `l` went into (Q75).
pub(crate) struct ArchiveView {
    archive: PathBuf,
    /// The folder inside it on screen, `/`-separated; empty at the top.
    inner: String,
    members: Arc<Vec<archive::Listed>>,
    /// The listing stopped at `ARCHIVE_LIMIT`.
    more: bool,
    /// Members unpacked for the preview, by the path the row shows.
    copies: HashMap<PathBuf, PathBuf>,
    /// The one being unpacked now, and ones that would not unpack.
    copying: Option<PathBuf>,
    refused: BTreeSet<PathBuf>,
    /// Where the preview's copies go: a folder of this view's own, so two
    /// archives of one name -- or two views in one process -- never share it.
    scratch: PathBuf,
}

/// Members up to this size are unpacked to be previewed; a bigger one keeps
/// its card. Reading a member means reading the archive up to it, and a tar
/// has to be read from the start.
const ARCHIVE_PREVIEW_LIMIT: u64 = 4 << 20;

impl Drop for ArchiveView {
    /// The preview's copies go with the view: nothing outside kura has
    /// them open. The copies `l` opened stay -- an editor may be holding one
    /// -- and are swept by a later start (`util::sweep_archive_scratch`).
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.scratch);
        // And the folders it sat in, when that left them empty (#253, #259):
        // `remove_dir` refuses a folder with anything in it, so a copy `l`
        // opened, or another view's previews, keep theirs.
        if let Some(preview) = self.scratch.parent() {
            let _ = std::fs::remove_dir(preview);
            if let Some(root) = preview.parent() {
                let _ = std::fs::remove_dir(root);
            }
        }
    }
}

/// A fresh folder name for one view's preview copies of `archive`.
fn preview_scratch(archive: &Path) -> PathBuf {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    util::archive_scratch().join("preview").join(format!("{n}-{}", util::file_name(archive)))
}

impl ArchiveView {
    /// What the preview says about a member, which is not a file on disk
    /// until it is unpacked.
    fn card(&self, entry: &Entry) -> crate::preview::Payload {
        let mut rows = vec![("Name".to_owned(), entry.name.clone())];
        match entry.is_dir_like() {
            true => {
                let inner = match self.inner.is_empty() {
                    true => entry.name.clone(),
                    false => format!("{}/{}", self.inner, entry.name),
                };
                rows.push(("Holds".into(), util::items(archive::level(&self.members, &inner).len())));
                rows.push(("Open".into(), "l to go in, h to come back".into()));
            }
            false => {
                rows.push(("Size".into(), util::human_size(entry.len)));
                rows.push(("Open".into(), "l or Enter opens a copy".into()));
            }
        }
        rows.push(("In".into(), self.archive.display().to_string()));
        crate::preview::Payload::Meta { rows }
    }
}

/// From a worker, for the archive view.
pub(crate) enum ArchiveMsg {
    Listed(PathBuf, std::io::Result<(Vec<archive::Listed>, bool)>),
    Copied(std::io::Result<PathBuf>),
    /// A member unpacked for the preview: the row's path and the copy's.
    Previewed(PathBuf, std::io::Result<PathBuf>),
}

/// The archive view's synthetic path, as the header shows it. Not a real
/// directory, so `in_search_view` holds and `<Esc>` leaves this view too.
fn archive_path(archive: &Path, inner: &str) -> PathBuf {
    match inner.is_empty() {
        true => PathBuf::from(format!("archive: {}", archive.display())),
        false => PathBuf::from(format!("archive: {} : {inner}", archive.display())),
    }
}

/// The usage view's synthetic path. Not a real directory, which is exactly what
/// `in_search_view` tests for, so `<Esc>` leaves this view too.
fn usage_path(root: &Path) -> PathBuf {
    PathBuf::from(format!("usage:  {}", root.display()))
}

fn search_path(query: &str, root: &Path) -> PathBuf {
    PathBuf::from(format!("search: {query}  in  {}", root.display()))
}

/// Split what has been typed into the directory a completion has to list and
/// the prefix its names have to carry on from. A trailing separator means the
/// directory itself is the question, so every child answers it. The text is
/// resolved the way `cd` resolves it, so a relative path completes against the
/// tab it was typed in.
fn completion_target(text: &str, cwd: &Path) -> (PathBuf, String) {
    let p = util::resolve_against(cwd, text);
    if text.ends_with('/') || text.ends_with('\\') {
        return (p, String::new());
    }
    // `\\host\Ba`: `std` folds host and share into one prefix with no parent, but
    // the question is the host's shares that carry on from `Ba`.
    if cfg!(windows) {
        if let Some(t) = share_completion(&p) {
            return t;
        }
    }
    let prefix = util::file_name(&p);
    match p.parent() {
        Some(dir) => (dir.to_path_buf(), prefix),
        // A drive or share root has nothing above it: list the root itself.
        None => (p, String::new()),
    }
}

/// A share root being typed is a prefix of the host's share names: the host to
/// list and the share typed so far. `None` for anything else.
fn share_completion(p: &Path) -> Option<(PathBuf, String)> {
    if util::host_only_unc(p) {
        return Some((p.to_path_buf(), String::new()));
    }
    let host = util::unc_host(p)?;
    let share = p.to_str()?.rsplit(['\\', '/']).find(|s| !s.is_empty())?;
    Some((host, share.to_owned()))
}

/// The directories in a listing that carry on from `prefix`, in the order the
/// prompt walks them. Links to directories count, the same as entering one.
fn completion_hits(entries: &[Entry], prefix: &str) -> Vec<String> {
    let prefix = prefix.to_lowercase();
    let mut hits: Vec<String> = entries
        .iter()
        .filter(|e| e.is_dir_like())
        .map(|e| e.name.clone())
        .filter(|n| prefix.is_empty() || n.to_lowercase().starts_with(&prefix))
        .collect();
    hits.sort_by(|a, b| util::natural_cmp(a, b, false));
    hits
}

/// What the watcher thread of [`App::start_watching`] read, or what failed
/// off the UI thread.
enum Watched {
    /// kura's own files changed; boxed, as a whole config is large.
    Config(Box<crate::config::Fresh>),
    Failed(String),
}

/// Whether two paths name the same file by their spelling: normalized, and
/// without regard to case on Windows (as the file system sees it).
fn same_place(a: &Path, b: &Path) -> bool {
    let (a, b) = (util::normalize(a), util::normalize(b));
    match cfg!(windows) {
        true => a.to_string_lossy().to_lowercase() == b.to_string_lossy().to_lowercase(),
        false => a == b,
    }
}

/// `to-pack.zip` -> `to-pack.tar.gz` -> `to-pack.7z` -> `to-pack.zip`. A name
/// with some other ending (or none) gets `.zip` added, and `.tar` -- which
/// `E` can write, but is not worth a stop on the way round -- becomes `.zip`.
fn next_archive_name(name: &str) -> String {
    const ENDS: [&str; 3] = [".zip", ".tar.gz", ".7z"];
    let lower = name.to_ascii_lowercase();
    match ENDS.iter().position(|e| lower.ends_with(e)) {
        Some(i) => format!("{}{}", &name[..name.len() - ENDS[i].len()], ENDS[(i + 1) % ENDS.len()]),
        None => {
            let stem = match lower.ends_with(".tar") {
                true => &name[..name.len() - 4],
                false => name,
            };
            format!("{stem}.zip")
        }
    }
}

/// The folder a completion just made is being cycled in: `text` is still what
/// the last `<Tab>` wrote (the folder, then the hit just taken, then a
/// separator), so the next `<Tab>` goes on to the following hit in `hits`
/// instead of listing what is inside the one just chosen (#222).
fn cycling_in(text: &str, hits: &[String], at: usize) -> Option<PathBuf> {
    if hits.len() < 2 || at == 0 {
        return None;
    }
    let last = &hits[(at - 1) % hits.len()];
    let tail = format!("{last}{}", std::path::MAIN_SEPARATOR);
    let dir = text.strip_suffix(&tail)?;
    Some(PathBuf::from(dir))
}

/// What the input line reads once a name is chosen: the directory, ready for
/// the next component to be typed or completed.
fn completed_text(dir: &Path, name: &str) -> String {
    format!("{}{}", dir.join(name).display(), std::path::MAIN_SEPARATOR)
}

fn preview_paths(paths: &[PathBuf]) -> Vec<String> {
    let mut out: Vec<String> = paths.iter().take(8).map(|p| util::file_name(p)).collect();
    if paths.len() > 8 {
        out.push(format!("… and {} more", paths.len() - 8));
    }
    out
}

/// The spot panel as text: each section's title on a line of its own, then
/// one `Label<TAB>value` line per row, and a blank line between sections. A tab
/// because values hold spaces and colons, and a spreadsheet splits on it.
/// `Came in via` holds two values, the pull request and the merge, and they
/// get a tab of their own so each lands in a cell (#263); the panel is unchanged.
fn spot_text(sections: &[Section]) -> String {
    sections
        .iter()
        .map(|s| {
            let rows: Vec<String> = s
                .rows
                .iter()
                .map(|(k, v)| match k.as_str() {
                    "Came in via" => format!("{k}\t{}", v.replacen("  ", "\t", 1)),
                    _ => format!("{k}\t{v}"),
                })
                .collect();
            format!("{}\n{}", s.title, rows.join("\n"))
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// An `Entry` per path that can still be read; the rest are skipped.
fn entries_of_paths(paths: Vec<PathBuf>) -> Vec<Entry> {
    paths.into_iter().filter_map(|p| Entry::from_path(p).ok()).collect()
}

/// `spot_text` for several files, each under `#### <path>`, blank-line apart.
fn spot_text_of(entries: &[Entry]) -> String {
    entries
        .iter()
        .map(|e| {
            let mut sections = vec![spot::base(e)];
            sections.extend(spot::inspect(&e.path));
            format!("#### {}\n{}", e.path.display(), spot_text(&sections))
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

#[cfg(test)]
#[path = "app/tests/general.rs"]
mod tests;

#[cfg(test)]
#[path = "app/tests/preview_delivery.rs"]
mod preview_delivery;

#[cfg(test)]
#[path = "app/tests/watcher_frames.rs"]
mod watcher_frames;

#[cfg(test)]
#[path = "app/tests/host_sizes.rs"]
mod host_sizes;

#[cfg(test)]
#[path = "app/tests/preview_scroll.rs"]
mod preview_scroll;

#[cfg(test)]
#[path = "app/tests/extract_message.rs"]
mod extract_message;

#[cfg(test)]
#[path = "app/tests/create_and_link_undo.rs"]
mod create_and_link_undo;

#[cfg(test)]
#[path = "app/tests/move_undo.rs"]
mod move_undo;

#[cfg(test)]
#[path = "app/tests/config_warnings.rs"]
mod config_warnings;

#[cfg(test)]
#[path = "app/tests/term_scroll_direction.rs"]
mod term_scroll_direction;

#[cfg(test)]
#[path = "app/tests/spot_keys.rs"]
mod spot_keys;

#[cfg(test)]
#[path = "app/tests/panel_layers.rs"]
mod panel_layers;

#[cfg(test)]
#[path = "app/tests/diff_tree_keys.rs"]
mod diff_tree_keys;

#[cfg(test)]
#[path = "app/tests/no_osc7_message.rs"]
mod no_osc7_message;

#[cfg(test)]
#[path = "app/tests/spot_pages.rs"]
mod spot_pages;

#[cfg(test)]
#[path = "app/tests/archive_view.rs"]
mod archive_view;

#[cfg(test)]
#[path = "app/tests/usage_view.rs"]
mod usage_view;

#[cfg(test)]
#[path = "app/tests/spot_follows_the_cursor.rs"]
mod spot_follows_the_cursor;

#[cfg(test)]
#[path = "app/tests/diff_scrolling.rs"]
mod diff_scrolling;

#[cfg(test)]
#[path = "app/tests/outline_jump.rs"]
mod outline_jump;

#[cfg(test)]
#[path = "app/tests/goto_and_history_keys.rs"]
mod goto_and_history_keys;

#[cfg(test)]
#[path = "app/tests/send_pane_and_the_register.rs"]
mod send_pane_and_the_register;

#[cfg(test)]
#[path = "app/tests/follow_says_what_it_is_for.rs"]
mod follow_says_what_it_is_for;

#[cfg(test)]
#[path = "app/tests/window_scale.rs"]
mod window_scale;

#[cfg(test)]
#[path = "app/tests/help_keys.rs"]
mod help_keys;

#[cfg(test)]
#[path = "app/tests/escape_and_max_preview.rs"]
mod escape_and_max_preview;

#[cfg(test)]
#[path = "app/tests/q_closes_the_panel_in_front.rs"]
mod q_closes_the_panel_in_front;

#[cfg(test)]
#[path = "app/tests/alt_jk_scrolls_every_pane.rs"]
mod alt_jk_scrolls_every_pane;

#[cfg(test)]
#[path = "app/tests/said_out_loud.rs"]
mod said_out_loud;

#[cfg(test)]
#[path = "app/tests/term_prompt_keys.rs"]
mod term_prompt_keys;

#[cfg(test)]
#[path = "app/tests/program_label_tests.rs"]
mod program_label_tests;

#[cfg(test)]
#[path = "app/tests/find_marks.rs"]
mod find_marks;

#[cfg(test)]
#[path = "app/tests/mcp_writes.rs"]
mod mcp_writes;
