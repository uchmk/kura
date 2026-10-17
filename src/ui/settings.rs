//! The settings screen (`<C-,>`), as every uchmk app draws it: `ito_prefs`
//! makes the frame, the nav and the search, and the shared Language and
//! CLOCK rows; kura hands it its pages and its colours.
//!
//! General writes common.toml (the language and the clock every uchmk app
//! shares) through `App::write_common`, off the UI thread. Theme picks the
//! common theme the same way, and shows it as kura's list with theme.toml's
//! colours drawn over it. config.toml shows
//! the main rows of kura's own file and writes a change through
//! `App::write_kura`, off the UI thread and keeping the file's comments.
//! Notifications is config.toml's `[notify]` as every uchmk app shows it
//! (`ito_prefs::notify_page`), written the same way.
//! Advanced says where the config is read from and what was wrong with it.

use std::path::PathBuf;

use egui::{Color32, Rect, RichText, Ui};
use ito_prefs::{
    button, clock_card, field, keyed_row, language_row, row, scale_row, section, select, sep, switch, Drafts, Look, Nav, Words,
};
use ito_theme::{mix, Colors};

use crate::app::{App, KuraChange, Overlay};
use crate::config::cmd::Act;
use crate::config::theme::{is_light, Theme};

/// kura's own words on the screen, in one language.
pub struct KuraWords {
    pub general: &'static str,
    pub general_lead: &'static str,
    pub theme: &'static str,
    pub theme_lead: &'static str,
    pub theme_note: &'static str,
    pub language_note: &'static str,
    pub scale_note: &'static str,
    pub advanced: &'static str,
    pub advanced_lead: &'static str,
    pub keys: &'static str,
    pub keys_lead: &'static str,
    /// Each keymap layer's name on the Keys page: the layer, its words.
    pub layers: [(&'static str, &'static str); 10],
    pub folders: &'static str,
    pub kura_note: &'static str,
    pub yazi_note: &'static str,
    pub common_note: &'static str,
    pub copy: &'static str,
    pub open: &'static str,
    pub copied: &'static str,
    pub problems: &'static str,
    pub no_problems: &'static str,
    pub no_problems_note: &'static str,
    pub reload: &'static str,
    pub reload_note: &'static str,
    pub reload_button: &'static str,
    pub no_folder: &'static str,
    pub kura_lead: &'static str,
    pub look: &'static str,
    pub font_size: &'static str,
    pub font_size_note: &'static str,
    pub row_padding: &'static str,
    pub row_padding_note: &'static str,
    pub icons: &'static str,
    pub icons_note: &'static str,
    pub minimap: &'static str,
    pub minimap_note: &'static str,
    pub preview: &'static str,
    pub render_markdown: &'static str,
    pub render_markdown_note: &'static str,
    pub debounce: &'static str,
    pub debounce_note: &'static str,
    pub max_text: &'static str,
    pub max_text_note: &'static str,
    pub list: &'static str,
    pub history: &'static str,
    pub history_note: &'static str,
    pub window: &'static str,
    pub width: &'static str,
    pub height: &'static str,
    pub size_note: &'static str,
    pub backend: &'static str,
    pub backend_note: &'static str,
    pub mcp: &'static str,
    pub mcp_note: &'static str,
    pub updates: &'static str,
    pub check_updates: &'static str,
    pub check_updates_note: &'static str,
    pub terminal: &'static str,
    pub shell: &'static str,
    pub shell_note: &'static str,
    pub shell_env_note: &'static str,
    pub shell_hint: &'static str,
    pub notify_lead: &'static str,
    pub notify_done: &'static str,
    pub notify_failed: &'static str,
    pub notify_done_sound: &'static str,
    pub notify_failed_sound: &'static str,
    pub notify_test_title: &'static str,
    pub notify_test_body: &'static str,
    /// What a number field says of words that are not a number it takes:
    /// the row, the words, the lowest and the highest.
    pub not_a_number: fn(&str, &str, f64, f64) -> String,
}

fn not_a_number_en(label: &str, text: &str, lo: f64, hi: f64) -> String {
    format!("{label}: \"{text}\" is not a number from {lo} to {hi}")
}

fn not_a_number_ja(label: &str, text: &str, lo: f64, hi: f64) -> String {
    format!("{label}: 「{text}」は {lo} から {hi} までの数ではない")
}

pub const EN: KuraWords = KuraWords {
    general: "General",
    general_lead: "The language, the scale and the clock, shared with every uchmk app through common.toml.",
    theme: "Theme",
    theme_lead: "The colours every uchmk app shares through common.toml. A theme.toml of yazi's is drawn over them.",
    theme_note: "Your own themes go in <config>/uchmk/themes/*.toml. Colours a theme.toml sets stay as it sets them.",
    language_note: "The words of this screen and the clock's weekday; the rest of kura is in English for now",
    scale_note: "The size of everything on screen (the same as Ctrl+= and Ctrl+-)",
    advanced: "Advanced",
    advanced_lead: "Where kura reads its settings, and what it found wrong in them.",
    keys: "Keys",
    keys_lead: "Every key, an operation to a row. Find one by its words, or with ⌨ Find by key and then the key; click a key to change it, + to add one. Only what differs from the defaults is written, to kura's own keymap.toml.",
    layers: [
        ("mgr", "list"),
        ("input", "prompt"),
        ("confirm", "question"),
        ("pick", "picker"),
        ("help", "help"),
        ("tasks", "tasks"),
        ("spot", "spot"),
        ("diff", "compare"),
        ("settings", "settings"),
        ("term", "terminal"),
    ],
    folders: "CONFIG FOLDERS",
    kura_note: "config.toml, and kura's own keymap.toml and theme.toml",
    yazi_note: "yazi's yazi.toml, keymap.toml and theme.toml, read as they are",
    common_note: "common.toml: the language, the clock and the theme of every uchmk app",
    copy: "Copy",
    open: "Open",
    copied: "Copied",
    problems: "PROBLEMS READING THE CONFIG",
    no_problems: "No problems",
    no_problems_note: "Every config file read cleanly",
    reload: "Read the config again",
    reload_note: "After editing a file by hand (the same as Ctrl+F5)",
    reload_button: "Reload",
    no_folder: "(no config folder on this system)",
    kura_lead: "kura's own settings. A change is written into config.toml at once, keeping its comments, and read back as Ctrl+F5 does.",
    look: "LOOK",
    font_size: "Font size",
    font_size_note: "Points, for the list, the preview and the panels",
    row_padding: "Row padding",
    row_padding_note: "Points added to the height of each row of the list",
    icons: "Icons",
    icons_note: "auto: a Nerd Font's icons when one is installed, else plain letters",
    minimap: "Minimap",
    minimap_note: "The shape of the whole file down the right of a text preview (Alt+N)",
    preview: "PREVIEW",
    render_markdown: "Rendered Markdown / HTML",
    render_markdown_note: "How a Markdown or HTML preview starts; M switches the one on screen. Read at start",
    debounce: "Wait before previewing",
    debounce_note: "Milliseconds the cursor rests before the preview is asked for",
    max_text: "Text read for a preview",
    max_text_note: "Bytes of a text file the preview reads",
    list: "LIST",
    history: "Folders remembered",
    history_note: "How many folders z can jump back to",
    window: "WINDOW",
    width: "Window width",
    height: "Window height",
    size_note: "Points, when kura starts",
    backend: "Drawing",
    backend_note: "What draws the window; read at start (WGPU_BACKEND wins for one run)",
    mcp: "kura mcp",
    mcp_note: "Lets `kura mcp` read this window, for the same user only. Read at start",
    updates: "UPDATES",
    check_updates: "Check for updates",
    check_updates_note: "Asks GitHub once at start whether a newer kura is out (through curl) and says so in a toast. Read at start",
    terminal: "TERMINAL",
    shell: "Shell",
    shell_note: "What Ctrl+T starts; empty for the system's default. A pane already open keeps its own",
    shell_env_note: "KURA_TERM_SHELL names the shell for this run",
    shell_hint: "default",
    notify_lead: "How a file job of 10 seconds or more (copy, move, delete, extract...) that ends while kura's window is behind another is told. Written to config.toml's [notify].",
    notify_done: "Finished",
    notify_failed: "Failed",
    notify_done_sound: "Sound when finished",
    notify_failed_sound: "Sound when failed",
    notify_test_title: "kura: test notification",
    notify_test_body: "A long job's end is told like this.",
    not_a_number: not_a_number_en,
};

pub const JA: KuraWords = KuraWords {
    general: "一般",
    general_lead: "言語・倍率・時計。common.toml を通して uchmk のどのアプリにも同じものが当たる。",
    theme: "テーマ",
    theme_lead: "common.toml を通して uchmk のどのアプリにも当たる色。yazi の theme.toml はその上に重なる。",
    theme_note: "自分のテーマは <設定のフォルダ>/uchmk/themes/*.toml に置く。theme.toml が決めた色はそのまま。",
    language_note: "この画面の言葉と時計の曜日。kura のほかの言葉はいまは英語",
    scale_note: "画面のすべての大きさ（Ctrl+= と Ctrl+- と同じ）",
    advanced: "詳細",
    advanced_lead: "kura が設定を読む場所と、読んだときに見つかった問題。",
    keys: "キー",
    keys_lead: "1 行に 1 つの操作とそのキー。言葉か「⌨ キーで探す」のあとに押したキーで探し、チップを押して変え、＋で足す。既定と違うところだけを kura の keymap.toml に書く。",
    layers: [
        ("mgr", "一覧"),
        ("input", "入力"),
        ("confirm", "確認"),
        ("pick", "選ぶ"),
        ("help", "ヘルプ"),
        ("tasks", "タスク"),
        ("spot", "詳しい情報"),
        ("diff", "比べる"),
        ("settings", "設定"),
        ("term", "ターミナル"),
    ],
    folders: "設定のフォルダ",
    kura_note: "config.toml と、kura だけの keymap.toml・theme.toml",
    yazi_note: "yazi の yazi.toml・keymap.toml・theme.toml をそのまま読む",
    common_note: "common.toml: uchmk のどのアプリにも当たる言語・時計・テーマ",
    copy: "コピー",
    open: "開く",
    copied: "コピーしました",
    problems: "設定を読んだときの問題",
    no_problems: "問題はありません",
    no_problems_note: "どの設定ファイルも問題なく読めた",
    reload: "設定を読み直す",
    reload_note: "ファイルを手で直したあとに（Ctrl+F5 と同じ）",
    reload_button: "読み直す",
    no_folder: "（この環境には設定のフォルダがない）",
    kura_lead: "kura だけの設定。変えるとすぐ config.toml に書き（コメントは残す）、Ctrl+F5 と同じに読み直す。",
    look: "見た目",
    font_size: "文字の大きさ",
    font_size_note: "ポイント。一覧・プレビュー・パネルの文字",
    row_padding: "行の余白",
    row_padding_note: "一覧の 1 行の高さに足すポイント",
    icons: "アイコン",
    icons_note: "auto は Nerd Font があればそのアイコン、無ければ文字",
    minimap: "ミニマップ",
    minimap_note: "テキストのプレビューの右にファイル全体の形（Alt+N）",
    preview: "プレビュー",
    render_markdown: "Markdown・HTML を整えて出す",
    render_markdown_note: "Markdown・HTML のプレビューの出し始め。画面のものは M で切り替える。起動時に読む",
    debounce: "プレビューまでの待ち",
    debounce_note: "カーソルが止まってからプレビューを頼むまでのミリ秒",
    max_text: "プレビューで読む量",
    max_text_note: "テキストファイルから読むバイト数",
    list: "一覧",
    history: "覚えるフォルダの数",
    history_note: "z で戻れるフォルダの数",
    window: "ウィンドウ",
    width: "ウィンドウの幅",
    height: "ウィンドウの高さ",
    size_note: "ポイント。起動時に読む",
    backend: "描画",
    backend_note: "ウィンドウを描くもの。起動時に読む（WGPU_BACKEND があればその回はそちら）",
    mcp: "kura mcp",
    mcp_note: "`kura mcp` がこのウィンドウを読めるようにする（同じユーザーだけ）。起動時に読む",
    updates: "更新",
    check_updates: "更新を確かめる",
    check_updates_note: "起動時に 1 回、GitHub に新しい kura が出ているか聞き（curl を使う）、あればトーストで知らせる。起動時に読む",
    terminal: "ターミナル",
    shell: "シェル",
    shell_note: "Ctrl+T で起動するもの。空なら OS の既定。開いているペインはそのまま",
    shell_env_note: "KURA_TERM_SHELL がこの回のシェルを決めている",
    shell_hint: "既定",
    notify_lead: "10 秒以上かかったファイルの作業（コピー・移動・削除・展開など）が、kura の窓が後ろにある間に終わったときの知らせ方。config.toml の [notify] に書く。",
    notify_done: "終わった",
    notify_failed: "失敗した",
    notify_done_sound: "終わったときの音",
    notify_failed_sound: "失敗したときの音",
    notify_test_title: "kura: 試しの通知",
    notify_test_body: "長い作業が終わるとこのように知らせる。",
    not_a_number: not_a_number_ja,
};

impl KuraWords {
    pub fn of(language: &str) -> &'static KuraWords {
        match language {
            "ja" => &JA,
            _ => &EN,
        }
    }
}

/// kura's theme in the shape uchmk's screens paint with: the list's
/// background, the header's band for the nav, the text and its dim, and the
/// status colours for what is on, waiting and wrong.
pub fn colors(t: &Theme) -> Colors {
    // Unless a theme.toml repainted the window, the screen is the common
    // theme itself, which is how the Theme page knows which one is in force.
    if !t.overall {
        return t.common;
    }
    let run = t.mode_normal.bg.unwrap_or(t.progress_fg);
    Colors {
        light: is_light(t.bg),
        bg: t.bg,
        side: t.bg_alt,
        panel: mix(t.bg, t.fg, 0.04),
        border: t.border,
        fg: t.fg,
        dim: t.fg_dim,
        wait: t.warning,
        run,
        err: t.progress_error,
        done: t.git_added,
        blue: run,
        magenta: t.marker_marked,
        ansi16: None,
    }
}

/// The pages, in the nav's order.
const GENERAL: usize = 0;
const THEME: usize = 1;
const KURA: usize = 2;
const KEYS: usize = 3;
const NOTIFY: usize = 4;
const ADVANCED: usize = 5;

/// The page's name for `<State:>`'s `settings page:` line (#336): a script
/// read which page was open off a picture before.
pub fn page_name(page: usize) -> &'static str {
    match page {
        GENERAL => "general",
        THEME => "theme",
        KURA => "kura",
        KEYS => "keys",
        NOTIFY => "notify",
        ADVANCED => "advanced",
        _ => "?",
    }
}

/// What the config.toml page shows: the settings in force, read before the
/// screen takes the overlay.
struct KuraNow {
    font_size: String,
    row_padding: String,
    icons: String,
    minimap: bool,
    render_markdown: bool,
    debounce: String,
    max_text: String,
    history: String,
    width: String,
    height: String,
    backend: String,
    mcp: bool,
    check_updates: bool,
    shell: String,
    shell_from_env: bool,
}

impl KuraNow {
    fn of(app: &App) -> Self {
        let u = &app.cfg.ui;
        Self {
            font_size: u.font_size.to_string(),
            row_padding: u.row_padding.to_string(),
            icons: u.icons.trim().to_ascii_lowercase(),
            minimap: u.minimap,
            render_markdown: u.render_markdown,
            debounce: u.preview_debounce_ms.to_string(),
            max_text: u.max_text_bytes.to_string(),
            history: u.max_history.to_string(),
            width: u.window_width.to_string(),
            height: u.window_height.to_string(),
            backend: u.backend_name().ok().flatten().unwrap_or("auto").to_owned(),
            mcp: app.cfg.mcp.enable,
            check_updates: u.check_updates,
            shell: app.cfg.term.shell.clone(),
            shell_from_env: app.cfg.term.from_env,
        }
    }
}

/// A number row of config.toml: its key in `[ui]`, its words, what it is now,
/// and the numbers it takes.
struct Num<'a> {
    key: &'static str,
    label: &'a str,
    note: &'a str,
    now: &'a str,
    lo: f64,
    hi: f64,
    /// An integer in the file (`max_history = 200`), not a float.
    whole: bool,
}

/// Draw a number row: a field that writes on Enter or on leaving it.
/// Words that are not a number in range are said in `bad`, and not written.
fn number_row(ui: &mut Ui, l: Look, drafts: &mut Drafts, n: Num, k: &KuraWords, out: &mut Vec<KuraChange>, bad: &mut Option<String>) {
    let key = format!("ui.{}", n.key);
    let Some(text) = keyed_row(ui, l, &key, n.label, n.note, |ui| field(ui, drafts, n.key, n.now, "", 110.0)) else { return };
    match number_toml(&text, n.lo, n.hi, n.whole) {
        Some(value) => out.push(KuraChange { table: "ui", key: n.key, value: Some(value) }),
        None => {
            *bad = Some((k.not_a_number)(n.label, text.trim(), n.lo, n.hi));
            // Enter took the caret out of the field; put it back on what was
            // typed (the draft is kept for the retry) so it can be corrected.
            ui.memory_mut(|m| m.request_focus(ito_prefs::field_id(n.key)));
        }
    }
}

/// `text` as the TOML for a number from `lo` to `hi`: `16.0` for a float,
/// so the file reads as the README writes it; `200` for a whole one.
fn number_toml(text: &str, lo: f64, hi: f64, whole: bool) -> Option<String> {
    let v: f64 = text.trim().parse().ok()?;
    if !v.is_finite() || v < lo || v > hi || (whole && v.fract() != 0.0) {
        return None;
    }
    Some(if whole { format!("{}", v as u64) } else { format!("{v:?}") })
}

/// The backends this system can draw with, as `[ui] backend` names them.
fn backends() -> Vec<(&'static str, &'static str)> {
    let mut list = vec![("auto", "auto"), ("vulkan", "Vulkan"), ("gl", "OpenGL")];
    if cfg!(windows) {
        list.push(("dx12", "DirectX 12"));
    }
    if cfg!(target_os = "macos") {
        list.push(("metal", "Metal"));
    }
    list
}

/// The keys of the config.toml page's rows, `table.key`, for the search to
/// find them by in any language (#388).
const KURA_KEYS: [&str; 14] = [
    "ui.font_size",
    "ui.row_padding",
    "ui.icons",
    "ui.minimap",
    "ui.render_markdown",
    "ui.preview_debounce_ms",
    "ui.max_text_bytes",
    "ui.max_history",
    "ui.window_width",
    "ui.window_height",
    "ui.backend",
    "mcp.enable",
    "ui.check_updates",
    "term.shell",
];

/// The config.toml page: the main rows of `[ui]`, `[mcp]` and `[term]`.
fn kura_page(ui: &mut Ui, l: Look, k: &KuraWords, now: &KuraNow, drafts: &mut Drafts, out: &mut Vec<KuraChange>, bad: &mut Option<String>) {
    let set = |key: &'static str, value: String| KuraChange { table: "ui", key, value: Some(value) };
    section(ui, l, k.look, |ui| {
        let n = Num { key: "font_size", label: k.font_size, note: k.font_size_note, now: &now.font_size, lo: 6.0, hi: 72.0, whole: false };
        number_row(ui, l, drafts, n, k, out, bad);
        sep(ui, l);
        let n = Num {
            key: "row_padding",
            label: k.row_padding,
            note: k.row_padding_note,
            now: &now.row_padding,
            lo: 0.0,
            hi: 40.0,
            whole: false,
        };
        number_row(ui, l, drafts, n, k, out, bad);
        sep(ui, l);
        let icons = [("auto", "auto"), ("nerd", "nerd"), ("ascii", "ascii"), ("none", "none")];
        let picked = keyed_row(ui, l, "ui.icons", k.icons, k.icons_note, |ui| select(ui, "icons", now.icons.as_str(), &icons));
        out.extend(picked.map(|v| set("icons", format!("\"{v}\""))));
        sep(ui, l);
        if keyed_row(ui, l, "ui.minimap", k.minimap, k.minimap_note, |ui| switch(ui, l, now.minimap)) {
            out.push(set("minimap", (!now.minimap).to_string()));
        }
    });
    section(ui, l, k.preview, |ui| {
        if keyed_row(ui, l, "ui.render_markdown", k.render_markdown, k.render_markdown_note, |ui| switch(ui, l, now.render_markdown)) {
            out.push(set("render_markdown", (!now.render_markdown).to_string()));
        }
        sep(ui, l);
        let n = Num {
            key: "preview_debounce_ms",
            label: k.debounce,
            note: k.debounce_note,
            now: &now.debounce,
            lo: 0.0,
            hi: 5000.0,
            whole: true,
        };
        number_row(ui, l, drafts, n, k, out, bad);
        sep(ui, l);
        let n = Num {
            key: "max_text_bytes",
            label: k.max_text,
            note: k.max_text_note,
            now: &now.max_text,
            lo: 1024.0,
            hi: 1073741824.0,
            whole: true,
        };
        number_row(ui, l, drafts, n, k, out, bad);
    });
    section(ui, l, k.list, |ui| {
        let n = Num { key: "max_history", label: k.history, note: k.history_note, now: &now.history, lo: 0.0, hi: 100000.0, whole: true };
        number_row(ui, l, drafts, n, k, out, bad);
    });
    section(ui, l, k.window, |ui| {
        let n = Num { key: "window_width", label: k.width, note: k.size_note, now: &now.width, lo: 200.0, hi: 20000.0, whole: false };
        number_row(ui, l, drafts, n, k, out, bad);
        sep(ui, l);
        let n = Num { key: "window_height", label: k.height, note: k.size_note, now: &now.height, lo: 150.0, hi: 20000.0, whole: false };
        number_row(ui, l, drafts, n, k, out, bad);
        sep(ui, l);
        let list = backends();
        let picked = keyed_row(ui, l, "ui.backend", k.backend, k.backend_note, |ui| select(ui, "backend", now.backend.as_str(), &list));
        out.extend(picked.map(|v| set("backend", format!("\"{v}\""))));
        sep(ui, l);
        if keyed_row(ui, l, "mcp.enable", k.mcp, k.mcp_note, |ui| switch(ui, l, now.mcp)) {
            out.push(KuraChange { table: "mcp", key: "enable", value: Some((!now.mcp).to_string()) });
        }
    });
    section(ui, l, k.updates, |ui| {
        if keyed_row(ui, l, "ui.check_updates", k.check_updates, k.check_updates_note, |ui| switch(ui, l, now.check_updates)) {
            out.push(set("check_updates", (!now.check_updates).to_string()));
        }
    });
    section(ui, l, k.terminal, |ui| {
        let note = match now.shell_from_env {
            true => format!("{}\n{}", k.shell_note, k.shell_env_note),
            false => k.shell_note.to_owned(),
        };
        if let Some(text) = keyed_row(ui, l, "term.shell", k.shell, &note, |ui| field(ui, drafts, "shell", &now.shell, k.shell_hint, 220.0))
        {
            let text = text.trim();
            // Empty is the system's default, which is the key not there.
            let value = (!text.is_empty()).then(|| toml::Value::String(text.to_owned()).to_string());
            out.push(KuraChange { table: "term", key: "shell", value });
        }
    });
}

/// The Theme page's preview: a few rows of kura's list in the theme in
/// force, theme.toml and all, with the cursor's bar, the markers and the
/// mode at the foot.
fn sample(ui: &mut Ui, t: &Theme) {
    use crate::fs::{Entry, Kind};
    let f = super::font(13.0);
    let row_h = 22.0;
    let rows: [(&str, bool, Option<Color32>); 7] = [
        ("docs", true, None),
        ("src", true, None),
        (".gitignore", false, None),
        ("Cargo.toml", false, Some(t.marker_selected)),
        ("README.md", false, None),
        ("logo.png", false, Some(t.marker_copied)),
        ("notes.txt", false, Some(t.marker_cut)),
    ];
    let hovered = 3;
    let (r, _) = ui.allocate_exact_size(egui::vec2(300.0, row_h * (rows.len() as f32 + 2.0) + 8.0), egui::Sense::hover());
    let p = ui.painter_at(r);
    p.rect_filled(r, 8.0, t.bg);
    p.rect_stroke(r, 8.0, egui::Stroke::new(1.0, t.border), egui::StrokeKind::Inside);
    let left = r.left() + 6.0;
    let width = r.width() - 12.0;
    let cwd = egui::pos2(left + 6.0, r.top() + 4.0 + row_h / 2.0);
    p.text(cwd, egui::Align2::LEFT_CENTER, "~/kura", f.clone(), t.cwd.fg.unwrap_or(t.fg));
    for (i, (name, dir, marker)) in rows.iter().enumerate() {
        let y = r.top() + 4.0 + row_h * (i as f32 + 1.0);
        let row = Rect::from_min_size(egui::pos2(left, y), egui::vec2(width, row_h));
        if i == hovered {
            p.rect_filled(row, 3.0, t.hovered_bg);
        }
        if let Some(m) = marker {
            p.rect_filled(Rect::from_min_size(row.left_top() + egui::vec2(1.0, 2.0), egui::vec2(3.0, row_h - 4.0)), 2.0, *m);
        }
        let entry = Entry {
            path: PathBuf::from(name),
            name: (*name).to_owned(),
            ext: name.rsplit_once('.').filter(|(stem, _)| !stem.is_empty()).map(|(_, e)| e.to_owned()),
            kind: if *dir { Kind::Dir } else { Kind::File },
            hidden: name.starts_with('.'),
            ..Default::default()
        };
        let style = t.style_for(&entry, crate::mime::guess(&entry));
        let color = style.fg.unwrap_or(t.fg);
        let icon = t.icon_for(&entry);
        let mid = row.center().y;
        let mut x = row.left() + 8.0;
        if !icon.text.is_empty() {
            p.text(egui::pos2(x, mid), egui::Align2::LEFT_CENTER, &icon.text, f.clone(), icon.fg.unwrap_or(color));
            x += f.size + 8.0;
        }
        p.text(egui::pos2(x, mid), egui::Align2::LEFT_CENTER, *name, f.clone(), color);
    }
    // The status bar: the mode's chip on the bar's ground.
    let bar = Rect::from_min_max(egui::pos2(r.left() + 1.0, r.bottom() - row_h - 1.0), egui::pos2(r.right() - 1.0, r.bottom() - 1.0));
    p.rect_filled(bar, egui::CornerRadius { nw: 0, ne: 0, sw: 7, se: 7 }, t.status_bg);
    let mode = p.layout_no_wrap(" NOR ".to_owned(), f.clone(), t.mode_normal.fg.unwrap_or(t.bg));
    let chip = Rect::from_min_size(egui::pos2(bar.left() + 6.0, bar.center().y - mode.size().y / 2.0), mode.size());
    p.rect_filled(chip, 3.0, t.mode_normal.bg.unwrap_or(t.progress_fg));
    p.galley(chip.min, mode, t.fg);
    p.text(egui::pos2(chip.right() + 8.0, bar.center().y), egui::Align2::LEFT_CENTER, "4/7", f, t.fg_dim);
}

/// A config folder on the Advanced page: its name, its note, and where it is.
struct Folder {
    name: &'static str,
    note: &'static str,
    dir: Option<PathBuf>,
}

/// Draw the screen over `full`, in place of everything else.
pub fn draw(app: &mut App, ui: &mut Ui, full: Rect, queued: &mut Vec<Act>) {
    let c = colors(&app.cfg.theme);
    ito_theme::set(c);
    let w = Words::of(&app.lang);
    let k = KuraWords::of(&app.lang);
    let language = app.common.language.clone().unwrap_or_else(|| "auto".to_owned());
    let clock = app.clock.clone();
    let scale = app.scale;
    let warnings = app.cfg.warnings.clone();
    let folders = [
        Folder { name: "kura", note: k.kura_note, dir: App::kura_config_dir() },
        Folder { name: "yazi", note: k.yazi_note, dir: crate::config::config_home("YAZI_CONFIG_HOME") },
        Folder { name: "uchmk", note: k.common_note, dir: app.common_base.clone() },
    ];
    let now = KuraNow::of(app);
    let themes = app.themes.clone();
    let (theme, dark, light) = app.common.theme_choice(None);
    let look = app.cfg.theme.clone();
    let notify = app.cfg.notify.clone();
    let lang = app.lang.clone();
    let mw = ito_prefs::MoreWords::of(&lang);
    let window = app.common.window_or_default();
    let own_keymap = app.own_keymap_named().unwrap_or_default();
    let keymap = &app.cfg.keymap;
    let Overlay::Settings(ov) = &mut app.overlay else { return };
    // While the Keys page takes keys, Esc and Ctrl+Tab are its own.
    let keys = if ov.keys_edit.busy() { ito_prefs::Keys::default() } else { std::mem::take(&mut ov.keys) };
    let keys_edit = &mut ov.keys_edit;
    let mut state = std::mem::take(&mut ov.state);
    let mut drafts = std::mem::take(&mut ov.drafts);

    let pages = [
        (k.general, k.general_lead),
        (k.theme, k.theme_lead),
        ("config.toml", k.kura_lead),
        (k.keys, k.keys_lead),
        (w.notify, k.notify_lead),
        (k.advanced, k.advanced_lead),
    ];
    let mut index = vec![
        (GENERAL, w.language),
        (GENERAL, k.language_note),
        (GENERAL, w.scale),
        (GENERAL, k.scale_note),
        (GENERAL, w.clock),
        (GENERAL, w.show_time),
        (GENERAL, w.time_format),
        (GENERAL, w.show_date),
        (GENERAL, w.date_format),
        (GENERAL, w.weekday),
        (THEME, w.mode),
        (THEME, w.follow_os),
        (THEME, w.light),
        (THEME, w.dark),
        (THEME, w.themes),
        (THEME, k.theme_note),
        (KURA, k.look),
        (KURA, k.font_size),
        (KURA, k.font_size_note),
        (KURA, k.row_padding),
        (KURA, k.row_padding_note),
        (KURA, k.icons),
        (KURA, k.icons_note),
        (KURA, k.minimap),
        (KURA, k.minimap_note),
        (KURA, k.preview),
        (KURA, k.render_markdown),
        (KURA, k.render_markdown_note),
        (KURA, k.debounce),
        (KURA, k.debounce_note),
        (KURA, k.max_text),
        (KURA, k.max_text_note),
        (KURA, k.list),
        (KURA, k.history),
        (KURA, k.history_note),
        (KURA, k.window),
        (KURA, k.width),
        (KURA, k.height),
        (KURA, k.size_note),
        (KURA, k.backend),
        (KURA, k.backend_note),
        (KURA, k.mcp),
        (KURA, k.mcp_note),
        (KURA, k.updates),
        (KURA, k.check_updates),
        (KURA, k.check_updates_note),
        (KURA, k.terminal),
        (KURA, k.shell),
        (KURA, k.shell_note),
        (ADVANCED, k.folders),
        (ADVANCED, k.problems),
        (ADVANCED, k.reload),
        (ADVANCED, k.reload_note),
    ];
    index.extend(ito_prefs::GENERAL_KEYS.map(|key| (GENERAL, key)));
    index.extend(ito_prefs::window_index(mw).into_iter().map(|t| (GENERAL, t)));
    index.extend(ito_prefs::WINDOW_KEYS.map(|key| (GENERAL, key)));
    index.extend(KURA_KEYS.map(|key| (KURA, key)));
    let kw = ito_prefs::KeyWords::of(&lang);
    index.extend(ito_prefs::keys_index(kw).into_iter().map(|t| (KEYS, t)));
    index.extend(k.layers.iter().map(|(_, n)| (KEYS, *n)));
    index.extend(ito_prefs::notify_index(w).into_iter().map(|t| (NOTIFY, t)));
    index.extend(ito_prefs::bug_index(w).into_iter().map(|t| (ADVANCED, t)));
    index.extend([(NOTIFY, k.notify_done), (NOTIFY, k.notify_failed), (NOTIFY, k.notify_done_sound), (NOTIFY, k.notify_failed_sound)]);
    for f in &folders {
        index.extend([(ADVANCED, f.name), (ADVANCED, f.note)]);
    }
    let nav = Nav { pages: &pages, index: &index, words: w, file: "config.toml" };

    let mut changes = Vec::new();
    let mut kura = Vec::new();
    let mut bad: Option<String> = None;
    let mut copy: Option<String> = None;
    let mut forget = false;
    let mut test: Option<(ito_prefs::Test, ito_prefs::Notify)> = None;
    let mut webhooks = None;
    let mut key_edits = Vec::new();
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(full));
    child.set_clip_rect(full);
    child.style_mut().visuals = c.visuals();
    let out = ito_prefs::show(&mut child, c, &mut state, &nav, keys, |ui, page, l| match page {
        GENERAL => {
            section(ui, l, &w.language.to_uppercase(), |ui| {
                changes.extend(language_row(ui, l, w, &language, k.language_note));
            });
            section(ui, l, &w.scale.to_uppercase(), |ui| {
                changes.extend(scale_row(ui, l, w, scale, k.scale_note));
            });
            changes.extend(clock_card(ui, l, w, &clock));
            let out = ito_prefs::window_card(ui, l, mw, &window);
            changes.extend(out.changes);
            forget |= out.forget;
        }
        THEME => {
            let choice = (theme.as_str(), dark.as_str(), light.as_str());
            changes.extend(ito_prefs::theme_page(ui, l, w, &themes, choice, k.theme_note, |ui| sample(ui, &look)));
        }
        KURA => kura_page(ui, l, k, &now, &mut drafts, &mut kura, &mut bad),
        KEYS => {
            let now = keymap.layers();
            let layer_name = |n: &str| k.layers.iter().find(|(id, _)| *id == n).map_or_else(|| n.to_owned(), |(_, w)| (*w).to_owned());
            let tr = |d: &str| crate::i18n::tr_desc(&lang, d);
            let table = ito_prefs::KeyTable {
                now: &now,
                built_in: crate::config::Keymap::built_in_layers(),
                noop: "noop",
                own: &own_keymap,
                layer_name: &layer_name,
                tr: &tr,
                extra: &|_, _| Vec::new(),
                unbound: &[],
                sup: false,
            };
            key_edits.extend(ito_prefs::keys_page(ui, l, kw, &table, keys_edit).unwrap_or_default());
        }
        NOTIFY => {
            let mut n = notify.page();
            let ends = [
                ito_prefs::End { name: k.notify_done, color: l.c.done, sound: k.notify_done_sound },
                ito_prefs::End { name: k.notify_failed, color: l.c.err, sound: k.notify_failed_sound },
            ];
            let defaults = crate::config::NotifyCfg::default().page();
            if let Some(what) = ito_prefs::notify_page(ui, l, w, &ends, &mut n, &defaults, &mut drafts) {
                test = Some((what, n.clone()));
            }
            webhooks = notify.webhooks_of(&n);
            kura.extend(notify.edits(&n).into_iter().map(|(key, value)| KuraChange { table: "notify", key, value: Some(value) }));
        }
        _ => {
            section(ui, l, k.folders, |ui| {
                for (i, f) in folders.iter().enumerate() {
                    if i > 0 {
                        sep(ui, l);
                    }
                    let path = f.dir.as_ref().map_or_else(|| k.no_folder.to_owned(), |d| d.display().to_string());
                    let note = format!("{}\n{path}", f.note);
                    let (copied, opened) = row(ui, l, f.name, &note, |ui| {
                        if f.dir.is_none() {
                            return (false, false);
                        }
                        // Right to left: Open ends the row.
                        let opened = button(ui, l, k.open);
                        let copied = button(ui, l, k.copy);
                        (copied, opened)
                    });
                    if copied {
                        copy = Some(path.clone());
                    }
                    if opened {
                        // Out of the screen and into the folder, in the list.
                        queued.push(Act::Settings);
                        queued.push(Act::Cd { target: path, interactive: false });
                    }
                }
            });
            section(ui, l, k.problems, |ui| {
                if warnings.is_empty() {
                    row(ui, l, k.no_problems, k.no_problems_note, |_| ());
                }
                for (i, text) in warnings.iter().enumerate() {
                    if i > 0 {
                        sep(ui, l);
                    }
                    ui.add_space(8.0);
                    ui.label(RichText::new(text).monospace().size(12.0).color(l.c.wait));
                    ui.add_space(8.0);
                }
                sep(ui, l);
                if row(ui, l, k.reload, k.reload_note, |ui| button(ui, l, k.reload_button)) {
                    queued.push(Act::ConfigReload);
                }
            });
            if ito_prefs::bug_card(ui, l, w) {
                queued.push(Act::BugReport);
            }
        }
    });
    if let Overlay::Settings(ov) = &mut app.overlay {
        // Keys taken on the Keys page stop with it: left to another page
        // by a click, they would keep Esc and Ctrl+Tab from the screen.
        if state.page != KEYS {
            ov.keys_edit.capture = None;
            ov.keys_edit.listening = false;
        }
        ov.state = state;
        ov.drafts = drafts;
    }
    if out.close {
        queued.push(Act::Settings);
    }
    if out.open_file {
        app.open_own_config();
    }
    if forget {
        match ito_prefs::forget_place(&crate::config::Config::state_dir()) {
            Ok(()) => app.toast("window.toml: forgotten".to_owned()),
            Err(e) => app.error(format!("Forget failed: {e}")),
        }
    }
    if let Some(path) = copy {
        match crate::exec::set_clipboard(&path) {
            Ok(()) => app.toast(format!("{}: {path}", k.copied)),
            Err(e) => app.error(format!("Copy failed: {e}")),
        }
    }
    if let Some((what, n)) = test {
        crate::notify::test(&n, what, k.notify_test_title, k.notify_test_body);
    }
    if let Some(places) = webhooks {
        app.write_webhooks(places);
    }
    app.write_common(changes);
    app.write_kura(kura);
    app.write_keymap(key_edits);
    if let Some(e) = bad {
        // The field reports its Enter on two frames in a row; one Enter is one toast.
        let id = egui::Id::new("kura-bad-said");
        let frame = ui.ctx().cumulative_frame_nr();
        let prev: Option<(String, u64)> = ui.ctx().data(|d| d.get_temp(id));
        let again = prev.is_some_and(|(text, at)| text == e && frame.saturating_sub(at) <= 1);
        ui.ctx().data_mut(|d| d.insert_temp(id, (e.clone(), frame)));
        if !again {
            app.error(e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A float is written as the README writes it, a whole number without a
    /// point, and words that are not a number in range are not written.
    #[test]
    fn a_number_row_writes_toml_only_for_a_number_in_range() {
        assert_eq!(number_toml("16", 6.0, 72.0, false).as_deref(), Some("16.0"));
        assert_eq!(number_toml(" 12.5 ", 6.0, 72.0, false).as_deref(), Some("12.5"));
        assert_eq!(number_toml("200", 0.0, 100000.0, true).as_deref(), Some("200"));
        assert_eq!(number_toml("abc", 6.0, 72.0, false), None);
        assert_eq!(number_toml("", 6.0, 72.0, false), None);
        assert_eq!(number_toml("5", 6.0, 72.0, false), None);
        assert_eq!(number_toml("73", 6.0, 72.0, false), None);
        assert_eq!(number_toml("inf", 6.0, f64::INFINITY, false), None);
        assert_eq!(number_toml("1.5", 0.0, 100.0, true), None);
    }

    /// The screen is drawn in the common theme itself, so the Theme page
    /// lights the one in force; a theme.toml that paints the window makes
    /// the screen its colours.
    #[test]
    fn the_screen_is_the_common_theme_unless_theme_toml_paints_the_window() {
        let t = Theme::default();
        assert_eq!(colors(&t), t.common);
        let toml = "[app]\noverall = { bg = \"#ffffff\" }\n";
        let t = Theme::layered(&t.common, vec![toml::from_str(toml).unwrap()]);
        let c = colors(&t);
        assert_eq!(c.bg, egui::Color32::WHITE);
        assert!(c.light);
    }

    /// Q98: the Theme page lists the themes beside kura's list in the one in
    /// force, and a theme clicked goes into common.toml.
    #[test]
    fn the_theme_page_shows_kura_and_writes_the_theme() {
        let dir = crate::util::test_dir("settings-theme");
        let base = dir.join("uchmk");
        std::fs::create_dir_all(&base).unwrap();
        std::fs::write(base.join("common.toml"), "# mine\n").unwrap();
        let mut s = crate::ui::harness::Screen::open(dir.clone());
        s.app.common_base = Some(base.clone());
        s.app.overlay = Overlay::Settings(Default::default());
        if let Overlay::Settings(ov) = &mut s.app.overlay {
            ov.state.page = THEME;
        }
        let f = s.draw();
        assert!(f.says("tsumugi Dark") && f.says("Nord"), "the dark themes are listed");
        assert!(f.says("Cargo.toml") && f.says("~/kura"), "kura's list is the preview");
        let hovered = s.app.cfg.theme.hovered_bg;
        assert!(!f.filled(hovered).is_empty(), "with the cursor's bar");
        let at = f.placed("Nord").expect("the theme is drawn") + egui::vec2(4.0, 6.0);
        let click = |pressed| egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        s.feed(vec![egui::Event::PointerMoved(at)]);
        s.feed(vec![click(true)]);
        s.feed(vec![click(false)]);
        let file = base.join("common.toml");
        let mut text = String::new();
        for _ in 0..500 {
            text = std::fs::read_to_string(&file).unwrap();
            if text.contains("theme") {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(text.contains("theme = \"Nord\"") && text.contains("# mine"), "{text}");
    }

    /// #340: Tab walks from the search to the first field of the page, and
    /// kura's key handling does not take it on the way.
    #[test]
    fn tab_goes_from_the_search_to_the_font_size() {
        let dir = crate::util::test_dir("settings-tab");
        let mut s = crate::ui::harness::Screen::open(dir);
        s.app.overlay = Overlay::Settings(Default::default());
        if let Overlay::Settings(ov) = &mut s.app.overlay {
            ov.state.page = KURA;
        }
        let tab = |pressed| egui::Event::Key {
            key: egui::Key::Tab,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        };
        s.draw();
        s.feed(vec![tab(true), tab(false)]);
        s.feed(vec![tab(true), tab(false)]);
        let p = s.draw();
        let focused = s.app_ctx().memory(|m| m.focused());
        assert_eq!(focused, Some(egui::Id::new(("prefs-field", "font_size"))));
        // The ring round it shows where the keys are (#340).
        let run = ito_theme::colors().run;
        assert!(p.strokes.iter().any(|(r, c)| *c == run && r.width() > 100.0), "{:?}", p.strokes);
        assert!(matches!(s.app.overlay, Overlay::Settings(_)), "the screen stays open");
    }

    /// #388: with the words in Japanese the search still finds a config.toml
    /// row by its key, and Enter puts the keys in its field, so a `--keys`
    /// script reaches it in any language.
    #[test]
    fn the_search_finds_a_row_by_its_key_in_any_language() {
        let dir = crate::util::test_dir("settings-key");
        let mut s = crate::ui::harness::Screen::open(dir);
        s.app.lang = "ja".into();
        s.app.overlay = Overlay::Settings(Default::default());
        let f = s.draw();
        assert!(f.says("一般"), "the screen is in Japanese");
        s.app_ctx().memory_mut(|m| m.request_focus(egui::Id::new("prefs-search")));
        s.draw();
        s.feed(vec![egui::Event::Text("font_size".into())]);
        let enter = |pressed| egui::Event::Key {
            key: egui::Key::Enter,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        };
        s.feed(vec![enter(true), enter(false)]);
        s.draw();
        let Overlay::Settings(ov) = &s.app.overlay else { panic!("the screen stays open") };
        assert_eq!(ov.state.page, KURA, "the search went to the config.toml page");
        let focused = s.app_ctx().memory(|m| m.focused());
        assert_eq!(focused, Some(egui::Id::new(("prefs-field", "font_size"))));
    }

    /// The Notifications page shows `[notify]` as every uchmk app does, a
    /// search finds its rows, and a switch turned writes config.toml.
    #[test]
    fn the_notify_page_shows_and_writes_notify() {
        let dir = crate::util::test_dir("settings-notify");
        let mut s = crate::ui::harness::Screen::open(dir.clone());
        s.app.overlay = Overlay::Settings(Default::default());
        if let Overlay::Settings(ov) = &mut s.app.overlay {
            ov.state.page = NOTIFY;
        }
        let w = Words::of("en");
        let f = s.draw();
        assert_eq!(page_name(NOTIFY), "notify");
        for text in [w.notify_test, w.notify_system, w.focus, w.webhook_none, EN.notify_done, EN.notify_failed] {
            assert!(f.says(text), "{text} is on the page");
        }
        let file = dir.join("config.toml");
        let mut n = s.app.cfg.notify.page();
        n.sound[1] = true;
        let edits = s.app.cfg.notify.edits(&n);
        let changes = edits.into_iter().map(|(key, value)| KuraChange { table: "notify", key, value: Some(value) }).collect();
        s.app.write_kura_to(file.clone(), vec![dir.clone()], changes);
        let mut text = String::new();
        for _ in 0..500 {
            text = std::fs::read_to_string(&file).unwrap_or_default();
            if text.contains("[notify]") {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(text.contains("[notify]") && text.contains("sound = [\"failed\"]"), "{text}");
    }

    /// The Keys page lists kura's operations with their layer, takes the
    /// keys pressed while a key is being set (Esc and Enter included, which
    /// would close the screen), and writes only the key set to kura's own
    /// keymap.toml once Enter sets it.
    #[test]
    fn the_keys_page_takes_a_key_and_writes_it_on_enter() {
        let dir = crate::util::test_dir("settings-keys");
        let mut s = crate::ui::harness::Screen::open(dir.clone());
        crate::app::TEST_KEYMAP.with(|t| *t.borrow_mut() = Some(dir.join("keymap.toml")));
        s.app.overlay = Overlay::Settings(Default::default());
        if let Overlay::Settings(ov) = &mut s.app.overlay {
            ov.state.page = KEYS;
        }
        assert_eq!(page_name(KEYS), "keys");
        let f = s.draw();
        assert!(f.says("Quit the process") && f.says("list"), "kura's operations and their layer are listed");
        let key = |key, modifiers, pressed| egui::Event::Key { key, physical_key: None, pressed, repeat: false, modifiers };
        let press = |s: &mut crate::ui::harness::Screen, k, m| {
            s.feed(vec![key(k, m, true)]);
            s.feed(vec![key(k, m, false)]);
        };
        let start = |s: &mut crate::ui::harness::Screen| {
            if let Overlay::Settings(ov) = &mut s.app.overlay {
                ov.keys_edit.capture =
                    Some(ito_prefs::Capture { layer: "mgr".into(), run: "paste".into(), keys: Vec::new(), replace: None });
            }
        };
        let file = dir.join("keymap.toml");
        // Esc gives up the key, and leaves the screen open.
        start(&mut s);
        press(&mut s, egui::Key::Escape, egui::Modifiers::NONE);
        s.draw();
        let Overlay::Settings(ov) = &s.app.overlay else { panic!("Esc while a key is set keeps the screen") };
        assert!(ov.keys_edit.capture.is_none(), "Esc gave the key up");
        // Ctrl+Shift+Enter is a key; nothing is written before Enter.
        start(&mut s);
        let cs = egui::Modifiers { ctrl: true, shift: true, command: true, ..Default::default() };
        press(&mut s, egui::Key::Enter, cs);
        s.draw();
        assert!(!file.exists(), "a key pressed is not written yet");
        let Overlay::Settings(ov) = &s.app.overlay else { panic!("the screen stays open") };
        assert_eq!(ov.keys_edit.capture.as_ref().map(|c| c.keys.len()), Some(1), "{:?}", ov.keys_edit.capture);
        press(&mut s, egui::Key::Enter, egui::Modifiers::NONE);
        s.draw();
        let mut text = String::new();
        for _ in 0..500 {
            text = std::fs::read_to_string(&file).unwrap_or_default();
            if text.contains("paste") {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(text.contains("<C-S-Enter>") && text.contains("run = \"paste\"") && text.contains("prepend_keymap"), "{text}");
        assert!(!text.contains("yank") && !text.contains("noop"), "only the key set is written: {text}");
        assert!(matches!(s.app.overlay, Overlay::Settings(_)), "Enter set the key, and the screen stays open");
        // Taking keys, then gone to another page (a click on it): the keys
        // stop being taken, and Esc closes the screen again.
        start(&mut s);
        if let Overlay::Settings(ov) = &mut s.app.overlay {
            ov.keys_edit.listening = true;
            ov.state.page = NOTIFY;
        }
        s.draw();
        let Overlay::Settings(ov) = &s.app.overlay else { panic!("the screen stays open") };
        assert!(!ov.keys_edit.busy(), "another page takes no keys for the Keys page");
        press(&mut s, egui::Key::Escape, egui::Modifiers::NONE);
        s.draw();
        assert!(!matches!(s.app.overlay, Overlay::Settings(_)), "Esc closes the screen");
        crate::app::TEST_KEYMAP.with(|t| *t.borrow_mut() = None);
    }

    /// What was typed and the range it takes are said, in either language.
    #[test]
    fn the_kura_toml_page_says_why_a_number_is_not_written() {
        let en = (KuraWords::of("en").not_a_number)("Font size", "abc", 6.0, 72.0);
        assert!(en.contains("abc") && en.contains("6") && en.contains("72"), "{en}");
        let ja = (KuraWords::of("ja").not_a_number)("文字の大きさ", "abc", 6.0, 72.0);
        assert!(ja.contains("abc") && ja.contains("72"), "{ja}");
    }
}
