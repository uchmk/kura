//! The overlays (input, confirm, pick, settings, diff, ...) and the keymap layer each answers to.

use super::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InputKind {
    Create,
    Rename {
        from: PathBuf,
    },
    /// A rule to rename all of these at once, previewed as it is typed.
    Bulk {
        paths: Vec<PathBuf>,
    },
    Filter,
    Find {
        prev: bool,
    },
    Cd,
    /// A string to find in the terminal's scrollback.
    TermFind,
    /// A pattern to find in a huge log shown in the preview.
    PreviewFind,
    /// The name of the archive to pack the selection into.
    Compress,
    Shell {
        block: bool,
    },
    Search {
        via: SearchVia,
    },
    ConflictRename {
        job: u64,
    },
}

pub struct InputOverlay {
    pub kind: InputKind,
    pub title: String,
    pub text: String,
    /// Where to put the caret when the widget first gains focus.
    pub initial_selection: Option<(usize, usize)>,
    pub focused: bool,
    pub completion: Vec<String>,
    pub completion_at: usize,
}

#[derive(Clone, Debug)]
pub enum ConfirmAction {
    Conflict {
        reply: Sender<Resolution>,
        job: u64,
    },
    DeleteForever {
        paths: Vec<PathBuf>,
    },
    BookmarkDeleteAll,
    /// `<C-S-t>` while a program runs under the shell.
    EndShell,
    /// Several lines pasted into a program that did not ask for bracketed
    /// paste, where each line break is an Enter. Holds the text until answered.
    PasteToTerm {
        text: String,
    },
    /// Folder symlinks Windows refused, offered again as junctions (Q46).
    Junctions {
        links: Vec<ops::Link>,
    },
    /// `<F12>`'s report, shown before anything leaves the machine (Q62).
    BugReport {
        url: String,
    },
    /// A change an AI client asked for through `kura mcp` (#315). `Refuse` is
    /// the first button, so `<Enter>` says no. `expired` is set once the
    /// client stopped waiting; a late Allow then does nothing.
    Mcp {
        write: crate::mcp::Write,
        reply: std::sync::mpsc::SyncSender<crate::mcp::Reply>,
        expired: std::sync::Arc<std::sync::atomic::AtomicBool>,
    },
}

pub struct ConfirmOverlay {
    pub title: String,
    pub body: Vec<String>,
    pub options: Vec<(char, String)>,
    pub action: ConfirmAction,
    /// The colliding destination, when the dialog offers a rename.
    pub dest: Option<PathBuf>,
}

impl ConfirmOverlay {
    /// What button `i` says. The first one also names `<Enter>`, which picks
    /// it: on the Report a bug panel that opens a browser, the one key in the
    /// box that leaves the program, and nothing on screen said so (Q69, #230).
    pub fn button_label(&self, i: usize) -> String {
        let (k, l) = &self.options[i];
        match i {
            0 => format!("[{k}] / <Enter> {l}"),
            _ => format!("[{k}] {l}"),
        }
    }
}

/// How many lines `text` pastes as, when that is more than one and the program
/// did not ask for bracketed paste. One trailing line break is the usual end of
/// a copied line, and CRLF is one break.
pub(super) fn multiline_paste(text: &str, bracketed: bool) -> Option<usize> {
    let body = text.strip_suffix("\r\n").or_else(|| text.strip_suffix('\n')).or_else(|| text.strip_suffix('\r')).unwrap_or(text);
    let lines = body.replace("\r\n", "\n").split(['\n', '\r']).count();
    (!bracketed && lines >= 2).then_some(lines)
}

/// The first lines of a paste, as the question shows them.
pub(super) fn paste_preview(text: &str) -> Vec<String> {
    const LINES: usize = 5;
    const WIDTH: usize = 80;
    let lines: Vec<&str> = text.lines().collect();
    let mut out: Vec<String> = lines
        .iter()
        .take(LINES)
        .map(|l| if l.chars().count() > WIDTH { format!("{}…", l.chars().take(WIDTH).collect::<String>()) } else { (*l).to_owned() })
        .collect();
    if lines.len() > LINES {
        out.push(format!("… {} more lines", lines.len() - LINES));
    }
    out
}

#[derive(Clone, Debug)]
pub enum PickAction {
    /// `line` (1-based) is handed to editors that take one.
    OpenWith {
        paths: Vec<PathBuf>,
        runs: Vec<(String, bool, bool)>,
        line: Option<usize>,
    },
    Jump {
        paths: Vec<PathBuf>,
    },
    /// One keymap binding's command list per item.
    Command {
        runs: Vec<Vec<Act>>,
    },
}

pub struct PickOverlay {
    pub title: String,
    pub items: Vec<String>,
    pub details: Vec<String>,
    pub query: String,
    pub matches: Vec<(usize, i32, Vec<usize>)>,
    pub cursor: usize,
    pub action: PickAction,
    pub focused: bool,
}

impl PickOverlay {
    pub fn refilter(&mut self) {
        let cs = fuzzy::is_case_sensitive(&self.query, true, false);
        let mut out: Vec<(usize, i32, Vec<usize>)> = Vec::new();
        for (i, label) in self.items.iter().enumerate() {
            if let Some(hit) = fuzzy::match_str(&self.query, label, cs) {
                out.push((i, hit.score, hit.positions));
            }
        }
        out.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        self.matches = out;
        self.cursor = self.cursor.min(self.matches.len().saturating_sub(1));
    }

    pub fn selected(&self) -> Option<usize> {
        self.matches.get(self.cursor).map(|m| m.0)
    }
}

/// One opener from `yazi.toml`: `(run, block, orphan, label)`, as the pick
/// overlays want it.
pub type OpenerRow = (String, bool, bool, String);

/// Rows of a pick overlay: `(labels, details, runs)`. The three always have
/// the same length; the overlay indexes all of them by the row picked.
pub type PickRows = (Vec<String>, Vec<String>, Vec<Vec<Act>>);

/// common.toml as `ito_common::Common::read` gives it: the settings and what
/// in them is not known, or why the file did not read.
pub type CommonRead = Result<(ito_common::Common, Vec<String>), String>;

/// The themes there are to choose from: the built-in ones, then a person's
/// own in `<config>/uchmk/themes/`, with what is wrong with those that do
/// not read.
pub type Themes = (Vec<ito_theme::Theme>, Vec<String>);

/// [`Themes`] read from the `uchmk` folder `base`. On the disk, so off the
/// UI thread but at start-up.
pub fn read_themes(base: &Path) -> Themes {
    let (own, bad) = ito_theme::load_dir(&ito_common::themes_dir(base));
    let mut all = ito_theme::builtin();
    all.extend(own);
    (all, bad)
}

/// Turn the `mgr` bindings into palette rows.
///
/// The label carries both the description and the command text so either one
/// can be typed at the filter. Commands bound to several keys appear once,
/// under the first key the keymap gives them. `openers` are what `yazi.toml`
/// offers for the file under the cursor; they come last, since the palette is
/// a list of commands first.
pub fn palette_items(bindings: &[keymap::Binding], openers: &[OpenerRow]) -> PickRows {
    let (mut items, mut details, mut runs) = (Vec::new(), Vec::new(), Vec::new());
    let mut seen: Vec<&str> = Vec::new();
    for b in bindings {
        let skip = b.run.is_empty() || b.run.iter().all(|a| *a == Act::Noop) || b.run.iter().any(|a| matches!(a, Act::Unsupported(_)));
        if skip || b.raw.is_empty() || seen.contains(&b.raw.as_str()) {
            continue;
        }
        seen.push(&b.raw);
        items.push(binding_label(b));
        details.push(crate::config::keys::display_seq(&b.on));
        runs.push(b.run.clone());
    }
    push_openers(&mut items, &mut details, &mut runs, openers, "Open with ");
    (items, details, runs)
}

/// The context menu for the file under the cursor: everything the config says
/// can be done with it, without a key having to be pressed for it.
///
/// `yazi.toml`'s openers come first, then the keymap's own `shell` actions —
/// the custom actions a yazi config would write as plugins — then the rest of
/// the bindings that act on the file rather than on the view.
pub fn menu_items(bindings: &[keymap::Binding], openers: &[OpenerRow]) -> PickRows {
    let (mut items, mut details, mut runs) = (Vec::new(), Vec::new(), Vec::new());
    push_openers(&mut items, &mut details, &mut runs, openers, "");

    let mut seen: Vec<&str> = Vec::new();
    // Two passes so the custom actions sit together at the top, above the
    // ordinary file commands.
    for shell_pass in [true, false] {
        for b in bindings {
            let usable = !b.run.is_empty()
                && !b.raw.is_empty()
                && b.run.iter().any(acts_on_file)
                && !b.run.iter().any(|a| matches!(a, Act::Unsupported(_)));
            let is_shell = b.run.iter().any(|a| matches!(a, Act::Shell { .. }));
            if !usable || is_shell != shell_pass || seen.contains(&b.raw.as_str()) {
                continue;
            }
            seen.push(&b.raw);
            items.push(binding_label(b));
            details.push(crate::config::keys::display_seq(&b.on));
            runs.push(b.run.clone());
        }
    }
    (items, details, runs)
}

/// An opener is a shell command with the file substituted in, so it runs as
/// one. `prefix` says what to call it where the file is not already named.
pub(super) fn push_openers(
    items: &mut Vec<String>,
    details: &mut Vec<String>,
    runs: &mut Vec<Vec<Act>>,
    openers: &[OpenerRow],
    prefix: &str,
) {
    let mut seen: Vec<&str> = Vec::new();
    for (run, block, orphan, label) in openers {
        if seen.contains(&run.as_str()) {
            continue;
        }
        seen.push(run);
        items.push(format!("{prefix}{label}"));
        details.push(run.clone());
        runs.push(vec![Act::Shell { run: run.clone(), block: *block, confirm: false, orphan: *orphan }]);
    }
}

pub(super) fn binding_label(b: &keymap::Binding) -> String {
    match b.desc.is_empty() {
        true => b.raw.clone(),
        false => format!("{}  ·  {}", b.desc, b.raw),
    }
}

/// Whether a command does something to the file under the cursor (or to the
/// selection), as opposed to moving around or changing what the view shows.
/// It decides what the context menu is worth offering.
pub(super) fn acts_on_file(a: &Act) -> bool {
    matches!(
        a,
        Act::Open { .. }
            | Act::Yank { .. }
            | Act::Unyank
            | Act::Paste { .. }
            | Act::Link { .. }
            | Act::Hardlink
            | Act::Remove { .. }
            | Act::Create { .. }
            | Act::Rename { .. }
            | Act::BulkRename
            | Act::Compare
            | Act::Copy(_)
            | Act::Shell { .. }
            | Act::Extract
            | Act::Compress
            | Act::SendPane { .. }
            | Act::TermSend
            | Act::Spot
            | Act::Quick(_)
            | Act::Follow
            | Act::Reveal(_)
            | Act::Toggle { .. }
    )
}

/// The task panel, which is a list now that its rows can be acted on.
pub struct TasksOverlay {
    pub cursor: usize,
}

/// The spot panel on the hovered file.
pub struct SpotOverlay {
    /// The selected value row, counted across all sections.
    pub cursor: usize,
    /// The first row on screen; the renderer keeps the cursor in view.
    pub scroll: usize,
}

pub enum Overlay {
    None,
    Input(InputOverlay),
    Confirm(ConfirmOverlay),
    Pick(PickOverlay),
    Help,
    Tasks(TasksOverlay),
    Spot(SpotOverlay),
    Diff(DiffOverlay),
    Settings(SettingsOverlay),
}

/// The settings screen (`<C-,>`), drawn by `ito_prefs` over the whole window.
#[derive(Default)]
pub struct SettingsOverlay {
    /// The page, the search, and whether a control had the keys last frame.
    pub state: ito_prefs::State,
    /// The config.toml page's text fields while they are typed in.
    pub drafts: ito_prefs::Drafts,
    /// The Shell card's environment variables while they are edited.
    pub shell_edit: ito_prefs::ShellEdit,
    /// What the `[settings]` layer asked of the screen since it was last
    /// drawn: the next page, the search. Taken by the draw.
    pub keys: ito_prefs::Keys,
    /// The Keys page: its search and filters, and keys being pressed for an
    /// operation. While it takes keys ([`ito_prefs::KeysEdit::busy`]) no
    /// key is the screen's.
    pub keys_edit: ito_prefs::KeysEdit,
    /// Whether kura starts at sign-in, as the OS says; `None` until the
    /// worker answers (the registry and the disk are not asked on the UI thread).
    pub startup: Option<bool>,
    pub startup_rx: Option<crossbeam_channel::Receiver<(bool, Option<String>)>>,
}

/// Ask the OS whether kura starts at sign-in, setting it first when `set` is
/// given, on a thread of its own. The answer is `(enabled now, error)`.
pub(super) fn startup_worker(set: Option<bool>, ctx: egui::Context) -> crossbeam_channel::Receiver<(bool, Option<String>)> {
    let (tx, rx) = crossbeam_channel::bounded(1);
    std::thread::spawn(move || {
        let err = set.and_then(|on| {
            let exe = std::env::current_exe().map_err(|e| e.to_string());
            exe.and_then(|exe| ito_common::startup::set("kura", &exe, &[], on)).err()
        });
        let _ = tx.send((ito_common::startup::enabled("kura"), err));
        ctx.request_repaint();
    });
    rx
}

/// Which keymap layer a panel reads, and which dispatcher runs what it names.
///
/// Exists so [`App::feed_overlay_key`] can be one function rather than one per
/// panel. Overlays absent from `of` have no layer of their own: `Input`,
/// `Confirm` and `Pick` are native widgets, and `None` is the file list.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum PanelLayer {
    Help,
    Tasks,
    Spot,
    Diff,
    Settings,
}

impl PanelLayer {
    pub(super) fn of(ov: &Overlay) -> Option<Self> {
        match ov {
            Overlay::Help => Some(Self::Help),
            Overlay::Tasks(_) => Some(Self::Tasks),
            Overlay::Spot(_) => Some(Self::Spot),
            Overlay::Diff(_) => Some(Self::Diff),
            Overlay::Settings(_) => Some(Self::Settings),
            Overlay::None | Overlay::Input(_) | Overlay::Confirm(_) | Overlay::Pick(_) => None,
        }
    }

    pub(super) fn bindings(self, km: &crate::config::keymap::Keymap) -> &[crate::config::keymap::Binding] {
        match self {
            Self::Help => &km.help,
            Self::Tasks => &km.tasks,
            Self::Spot => &km.spot,
            Self::Diff => &km.diff,
            Self::Settings => &km.settings,
        }
    }

    pub(super) fn act(self, app: &mut App, a: Act) {
        match self {
            Self::Help => app.help_act(a),
            Self::Tasks => app.tasks_act(a),
            Self::Spot => app.spot_act(a),
            Self::Diff => app.diff_act(a),
            Self::Settings => app.settings_act(a),
        }
    }
}

/// Two files side by side. `outcome` is `None` until the diff worker answers,
/// which is what the view shows as *Comparing…*.
pub struct DiffOverlay {
    pub left: PathBuf,
    pub right: PathBuf,
    pub outcome: Option<diff::Outcome>,
    /// The first row drawn. A scroll position, not a cursor: there is nothing
    /// selected here, so the last useful value is the one that puts the last
    /// row at the bottom of the pane, not the one that puts it at the top.
    pub offset: usize,
    /// Rows the pane can show; set by the renderer, as `page_rows` is for the
    /// file list. Without it the keys cannot tell where the scrolling stops.
    pub rows: usize,
    /// The selected row, for a tree comparison. A file comparison has nothing to
    /// select -- the rows are lines, not things you act on -- so it leaves this
    /// where it is and shows no cursor. An index into [`DiffOverlay::shown`],
    /// not into the rows themselves, so that it means the row on screen
    /// whether or not `=` rows are hidden.
    pub cursor: usize,
    /// A tree comparison with its `=` rows hidden (`z`, Q23), so a large pair
    /// can be walked difference by difference. The footer still counts them.
    pub hide_same: bool,
    /// The folder comparison this file comparison was opened from with
    /// `<Enter>`, where `q` goes back to -- on the row it was opened from.
    pub back: Option<Box<DiffOverlay>>,
}

impl DiffOverlay {
    /// The worker's answer, in place. A tree opens on its first difference,
    /// not its first row: 497 matching rows of 500 used to fill the screen
    /// with no difference on it, and nothing said there was one until the
    /// footer was read (Q23). `gg` is still the top.
    pub fn arrive(&mut self, outcome: diff::Outcome) {
        if let diff::Outcome::Tree { rows, .. } = &outcome {
            self.cursor = rows.iter().position(|r| !matches!(r.state, diff::TreeState::Same)).unwrap_or(0);
        }
        self.outcome = Some(outcome);
    }

    /// The tree rows on screen, as indices into the outcome's rows: all of
    /// them, or all but the matches when `hide_same` is on. Empty for a file
    /// comparison, which has lines rather than rows.
    pub fn shown(&self) -> Vec<usize> {
        let Some(diff::Outcome::Tree { rows, .. }) = &self.outcome else { return Vec::new() };
        (0..rows.len()).filter(|&i| !(self.hide_same && matches!(rows[i].state, diff::TreeState::Same))).collect()
    }

    /// The rows on screen as text for the clipboard (`C`, Q91): one
    /// `state<TAB>relative path` line each, a folder's path ending in `/`.
    /// `z`'s hidden matches are left out, as they are from the screen.
    pub fn shown_text(&self) -> String {
        let Some(diff::Outcome::Tree { rows, .. }) = &self.outcome else { return String::new() };
        let lines: Vec<String> = self
            .shown()
            .into_iter()
            .map(|i| {
                let row = &rows[i];
                let dir = if row.dir { "/" } else { "" };
                format!("{}\t{}{dir}", row.state.word(), row.rel.display())
            })
            .collect();
        lines.join("\n")
    }
}

impl Overlay {
    pub fn is_none(&self) -> bool {
        matches!(self, Overlay::None)
    }

    /// Whether this one is a panel drawn over the file list.
    ///
    /// It decides who gets the wheel. The list is still painted underneath, and
    /// still sees the pointer over its own rect, so without this a turn of the
    /// wheel over a panel scrolled both -- the panel visibly and the list
    /// invisibly, which only showed up as a jumped cursor once the panel was
    /// closed. `Input` is not one of these: the prompt is a single row at the
    /// bottom and the list above it is being read while the filter is typed.
    pub fn is_modal(&self) -> bool {
        use Overlay::*;
        matches!(self, Help | Tasks(_) | Spot(_) | Diff(_) | Pick(_) | Confirm(_))
    }
}
