//! The `--keys` player: what `main.rs`'s frame loop calls to press the next
//! scripted step, and what decides how a run ends.
//!
//! A run must never end looking like a success when a step was skipped, a
//! file or picture is missing or a wait was not met. It ends one of five ways
//! in `KURA_KEYS_DONE`: `keys: done`, `keys: unsettled` (a key went in after
//! the settle wait with the screen still moving), `keys: failed`, or the
//! older `keys: closed` / `keys: quit` with keys left. The last three of the
//! first two, and `failed`, set the flag the process turns into
//! `keyscript::FAILED_EXIT`.
//!
//! The player takes the time and the frame number as arguments and sends no
//! window command of its own, so a test can put a step "6 s ago" instead of
//! sleeping, and read the [`Outcome`].

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::app::App;
use crate::keyscript::{self, evidence, pace, review, Press};

/// How long a step waits for `App::settled()`, and a `<Shot:>` for its picture.
const DEFAULT_WAIT: Duration = Duration::from_secs(5);

/// How long an input that went in may wait to be drawn before the run fails.
const DELIVER_WAIT: Duration = Duration::from_secs(10);

/// How long a config write may wait for the writer's answer before the run fails.
const CONFIG_WAIT: Duration = Duration::from_secs(10);

/// The label a failure about the config writers goes under: it belongs to no script step.
const CONFIG_STEP: &str = "<config write>";

/// An input handed to the main window that has not been drawn yet.
struct Inflight {
    /// The root viewport's frame it went in on.
    frame: u64,
    at: Instant,
    /// The script step it came from.
    step: usize,
}

/// What the frame loop does after the player has looked at a frame.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Outcome {
    /// Nothing to do: the script goes on, or has ended well.
    Continue,
    /// The run failed (the report is written as far as it can be): close the window.
    Failed,
    /// The script ended, but some key went in on a screen that had not settled.
    /// The window stays open as for any end; the exit code is `FAILED_EXIT`.
    Unsettled,
}

/// How the window ended while the script was running.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Ending {
    /// The title bar's x.
    Closed,
    /// `q` or `<Quit>`.
    Quit,
}

/// A `<Shot:name>` asked for and not yet on disk.
struct Shot {
    name: String,
    /// The screenshot command has been sent.
    sent: bool,
    /// The script step it came from.
    step: usize,
    /// When the step went in; the picture is waited for from here.
    since: Instant,
    /// The theme and language in force when the screenshot command was sent (empty until then).
    theme: String,
    lang: String,
}

pub(crate) struct ScriptRun {
    /// The steps still to press, each with the index of the script step it came from
    /// (a split drag and the wait `Size` adds carry their parent's).
    queue: VecDeque<(usize, Press)>,
    /// Every step as it was written, by index.
    labels: Vec<String>,
    /// The frame and the moment the last scripted key went in.
    at: (u64, Instant),
    /// `<Now>` was read: the next key goes in a frame after the last, without the settled wait.
    now: bool,
    /// The last input handed to the main window, until a later frame confirms it was drawn.
    inflight: Option<Inflight>,
    /// When the `<ReadHas:>` at the front of the queue was first tried.
    read_has_since: Option<Instant>,
    /// When the script last made progress, for the watchdog: a step ran, an
    /// input was drawn, a config write was answered or a picture was saved.
    progress_at: Instant,
    /// `ConfigWrites::answers` as last seen, to notice an answer.
    answers_seen: u64,
    /// The shape the window asked of the mouse cursor in the last frame, for `<State:>`.
    pub(crate) cursor_icon: egui::CursorIcon,
    shot: Option<Shot>,
    /// Where `<Shot:name>` and the readings save.
    shot_dir: PathBuf,
    /// `KURA_KEYS_DONE`, until the run's report has been written.
    done: Option<PathBuf>,
    watch: Option<Arc<Mutex<crate::ScriptWatch>>>,
    /// How long a step waits for `App::settled()` before it goes in anyway.
    pub(crate) settle_wait: Duration,
    /// How long a `<Shot:>` waits for its picture.
    pub(crate) shot_deadline: Duration,
    /// This run did not succeed. Outlives the window: `main` reads it after `run_native`.
    failed: Arc<AtomicBool>,
    /// Steps that went in after the settle wait on a screen that had not settled.
    unsettled: Vec<usize>,
    /// The script ended after the settle wait on a screen that had not settled.
    unsettled_end: bool,
    /// The end has been reached and reported.
    finished: bool,
    /// The script has a step that reads the window's tree (`<Assert:>`, `<Click:text>`, `<Tree:>`, ...).
    reads_window: bool,
    /// The tree generation as of the last action having been drawn; a reading step needs a newer tree.
    action_gen: u64,
    /// What the app had copied and announced when the last action went in (`<Announced:>` looks after it).
    since: Option<keyscript::Mark>,
    /// When the last action went in, for `<State:>`'s `since last action:` line.
    acted: Option<Instant>,
    /// The script step that last acted: a drag's queued tail frames carry the same index and do not act again.
    acted_idx: Option<usize>,
    /// When the window-reading step at the front of the queue first failed to find what it looks for.
    check_since: Option<Instant>,
    /// The theme the last `<Theme:>` of this run asked for; empty when none did.
    theme_requested: String,
    /// The language the last `<Lang:>` of this run asked for; empty when none did.
    lang_requested: String,
    /// `run.json`'s record, written beside the done file (only with one).
    run: evidence::Run,
    /// The first `run.json` has been tried.
    run_started: bool,
    /// The last `run.json` has been written, or there is no folder for it.
    run_closed: bool,
}

/// What kura cannot carry out: `None` for the steps it can (today, every parsed step but `<Snap:>`). An exhaustive
/// match with no wildcard, so a step ito adds does not go in unnoticed when
/// kura bumps its ito rev: somebody has to decide.
pub(crate) fn unsupported(press: &Press) -> Option<&'static str> {
    match press {
        Press::Events(_)
        | Press::Wait(_)
        | Press::Now
        | Press::Shot(_)
        | Press::State(_)
        | Press::Quit
        | Press::Paste
        | Press::PasteText(_)
        | Press::Click { .. }
        | Press::DClick { .. }
        | Press::Hover { .. }
        | Press::Wheel { .. }
        | Press::Drag { .. }
        | Press::Size { .. }
        | Press::Theme(_)
        | Press::Lang(_) => None,
        Press::Read { tag, .. } | Press::ReadHas { tag, .. } => match tag.as_str() {
            "PaneText" | "PreviewText" => None,
            _ => Some("a reading kura does not have"),
        },
        Press::ClickText { .. }
        | Press::DClickText(_)
        | Press::Assert { .. }
        | Press::Is { .. }
        | Press::Announced(_)
        | Press::Tree(_)
        | Press::ClickId(_)
        | Press::IdCheck(_) => None,
        Press::Clipboard(_) | Press::ClipboardHas(_) => None,
        // ito v0.276.0's state, picture, state step: kura writes no state JSON yet (`<State:>` is the text report).
        Press::Snap { .. } => Some("<Snap:> needs the state JSON, which kura does not write yet"),
    }
}

/// A step that reads the window's tree (the ones that need `keyscript::observe`).
fn reads_window(press: &Press) -> bool {
    matches!(
        press,
        Press::ClickText { .. }
            | Press::DClickText(_)
            | Press::Assert { .. }
            | Press::Is { .. }
            | Press::Announced(_)
            | Press::Tree(_)
            | Press::ClickId(_)
            | Press::IdCheck(_)
            | Press::Clipboard(_)
            | Press::ClipboardHas(_)
    )
}

/// A step that only looks (presses nothing): it retries on every frame, settled or not, and is never named as unsettled.
fn pure_check(press: &Press) -> bool {
    matches!(
        press,
        Press::Assert { .. } | Press::Is { .. } | Press::Announced(_) | Press::IdCheck(_) | Press::Clipboard(_) | Press::ClipboardHas(_)
    )
}

/// A click at `pos` in one frame, the shape of kura's own `<Click:>` / `<DClick:>`: egui reads
/// the press and release of one frame as a click, and the two pairs as a double click.
fn click_events(pos: egui::Pos2, right: bool, double: bool) -> Vec<egui::Event> {
    let button = if right { egui::PointerButton::Secondary } else { egui::PointerButton::Primary };
    let modifiers = egui::Modifiers::NONE;
    let pair = [true, false].map(|pressed| egui::Event::PointerButton { pos, button, pressed, modifiers });
    let mut events = vec![egui::Event::PointerMoved(pos)];
    events.extend(pair.clone());
    if double {
        events.extend(pair);
    }
    events
}

/// Said on standard error. Not `eprintln!`: started from `kura.com` this is a
/// pipe nobody reads once the window is up, and that would panic.
fn say_err(text: &str) {
    use std::io::Write;
    let _ = writeln!(std::io::stderr(), "kura --keys: {text}");
}

/// Write a report, and when it cannot be written say so and mark the run as
/// not succeeded: a done file that is missing reads as a stall, not as a result.
/// Every report from a running script gets the run's `run: <id>` line here (#203);
/// the refusals before the window add it in `main.rs`.
pub(crate) fn write_report(path: &Path, text: &str, failed: &AtomicBool) -> bool {
    match std::fs::write(path, keyscript::with_current_run(text)) {
        Ok(()) => true,
        Err(e) => {
            say_err(&format!("cannot write {}: {e}", path.display()));
            failed.store(true, Ordering::SeqCst);
            false
        }
    }
}

const UNSUPPORTED_WHY: &str = "kura's --keys does not carry out this step yet";

/// The report of a refused step: `failed_report` (first line `keys: failed`) plus the run's `run:` line.
fn unsupported_report(step: &str, i: usize, n: usize) -> String {
    let why = format!("step {i} of {n}: {UNSUPPORTED_WHY}");
    keyscript::with_current_run(&keyscript::failed_report(step, &why, n))
}

/// A script with a step kura does not carry out, refused before any window
/// opens: `step` is written as in the script, `i` of `n` from 1.
pub(crate) fn refuse_unsupported(step: &str, i: usize, n: usize) -> ! {
    // Before the window, so on the console a person typed it in, like `refuse_keys`.
    crate::say_err(&format!("kura: --keys: failed: step {step}: step {i} of {n}: {UNSUPPORTED_WHY}"));
    if let Some(done) = std::env::var_os("KURA_KEYS_DONE") {
        write_report(Path::new(&done), &unsupported_report(step, i, n), &AtomicBool::new(false));
    }
    std::process::exit(keyscript::FAILED_EXIT);
}

/// A taken screenshot as a PNG at `path`: the whole picture, or with `crop`
/// the preview pane's rectangle (`preview`, in points; `content_width` is the
/// window's width in points, to turn them into the picture's pixels).
pub(crate) fn save_shot(
    image: &egui::ColorImage,
    crop: bool,
    preview: Option<egui::Rect>,
    content_width: f32,
    path: &Path,
) -> Result<PathBuf, String> {
    let [w, h] = image.size;
    let rgba: Vec<u8> = image.pixels.iter().flat_map(|c| c.to_srgba_unmultiplied()).collect();
    let mut img = image::RgbaImage::from_raw(w as u32, h as u32, rgba).ok_or_else(|| "the picture had the wrong size".to_owned())?;
    if crop {
        let r = preview.ok_or_else(|| "no preview pane is on screen".to_owned())?;
        let k = w as f32 / content_width.max(1.0);
        let (x0, y0) = ((r.min.x * k).max(0.0) as u32, (r.min.y * k).max(0.0) as u32);
        let (x1, y1) = (((r.max.x * k).ceil() as u32).min(w as u32), ((r.max.y * k).ceil() as u32).min(h as u32));
        if x1 <= x0 || y1 <= y0 {
            return Err("the preview pane is off the picture".to_owned());
        }
        img = image::imageops::crop_imm(&img, x0, y0, x1 - x0, y1 - y0).to_image();
    }
    img.save(path).map_err(|e| format!("could not save {}: {e}", path.display()))?;
    Ok(path.to_path_buf())
}

/// The file name of `<Shot:name>`: `name` without the `@preview` suffix.
fn shot_file(name: &str) -> &str {
    name.strip_suffix(keyscript::SHOT_PREVIEW).unwrap_or(name)
}

/// What a `shots/<name>.json` says about its picture, as plain values.
struct Seen<'a> {
    /// The file name, without `@preview`.
    name: &'a str,
    /// In force when the screenshot command was sent.
    theme: &'a str,
    lang: &'a str,
    /// What the last `<Theme:>` / `<Lang:>` asked for; empty for none.
    requested: (&'a str, &'a str),
    /// The window in points.
    size: [f32; 2],
    /// The preview pane in points when the picture is its crop.
    preview: Option<egui::Rect>,
    pixels_per_point: f32,
}

/// The sidecar `path` (`shots/<name>.json`) beside a shot's picture.
fn write_shot_json(path: &Path, seen: &Seen) -> Result<(), String> {
    let [w, h] = seen.size;
    let (component, rect) = match seen.preview {
        Some(r) => ("preview", [r.min.x, r.min.y, r.width(), r.height()]),
        None => ("window", [0.0, 0.0, w, h]),
    };
    let meta = review::Meta {
        name: seen.name.to_owned(),
        app: "kura".to_owned(),
        version: env!("CARGO_PKG_VERSION").to_owned(),
        theme: seen.theme.to_owned(),
        theme_requested: seen.requested.0.to_owned(),
        lang: seen.lang.to_owned(),
        lang_requested: seen.requested.1.to_owned(),
        size: [w, h],
        zoom: 1.0,
        state: String::new(),
        component: component.to_owned(),
        rect,
        time: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs()),
        pixels_per_point: seen.pixels_per_point,
    };
    write_text(path, &meta.to_json())
}

/// What kura reads under `tag` (`PaneText`, `PreviewText`); `None` for a tag it does not have.
fn reading(app: &App, tag: &str) -> Option<String> {
    match tag {
        "PaneText" => Some(app.term.as_ref().map_or_else(|| "(no pane)\n".to_owned(), |t| t.screen_text())),
        "PreviewText" => Some(crate::preview_text(&app.preview.state)),
        _ => None,
    }
}

/// The terminal lines of `<State:>`: the args and env names the settings page edits (what a new pane
/// would get). The env VALUES are never written: they can be secrets. The names are quoted like the
/// args, so a name with a line break in it cannot add a line to the report. Whether a pane is open is
/// the report's own `pane:` line.
fn term_state_lines(app: &App) -> String {
    let names: Vec<&str> = app.cfg.term.env.keys().map(String::as_str).collect();
    format!("term args: {:?}\nterm env names: {names:?}\n", app.cfg.term.args)
}

/// Write `text` to `path`, the error naming the path.
fn write_text(path: &Path, text: &str) -> Result<(), String> {
    std::fs::write(path, text).map_err(|e| format!("could not write {}: {e}", path.display()))
}

impl ScriptRun {
    /// The script `steps` (from `keyscript::parse`), reporting to `done` and
    /// saving what it writes into `shot_dir`. `failed` is the flag `main`
    /// turns into the exit code.
    pub(crate) fn new(steps: &[keyscript::Step], done: Option<PathBuf>, shot_dir: PathBuf, failed: Arc<AtomicBool>) -> Self {
        let started = Instant::now();
        let queue: VecDeque<(usize, Press)> = steps.iter().enumerate().filter_map(|(i, s)| keyscript::press(s).map(|p| (i, p))).collect();
        let reads_window = queue.iter().any(|(_, p)| reads_window(p));
        // As ito's `Runner::new`: announcements are recorded only for a script that looks at them.
        if queue.iter().any(|(_, p)| matches!(p, Press::Announced(_) | Press::Tree(_))) {
            ito_a11y::record_announcements(true);
        }
        let mut run = evidence::Run::new("kura::ScriptRun", steps.len());
        run.app = "kura".into();
        run.app_version = env!("CARGO_PKG_VERSION").into();
        Self {
            queue,
            labels: steps.iter().map(keyscript::label).collect(),
            at: (0, started),
            now: false,
            inflight: None,
            read_has_since: None,
            progress_at: started,
            answers_seen: 0,
            cursor_icon: egui::CursorIcon::Default,
            shot: None,
            shot_dir,
            done,
            watch: None,
            settle_wait: DEFAULT_WAIT,
            shot_deadline: DEFAULT_WAIT,
            failed,
            unsettled: Vec::new(),
            unsettled_end: false,
            finished: false,
            reads_window,
            action_gen: 0,
            since: None,
            acted: None,
            acted_idx: None,
            check_since: None,
            theme_requested: String::new(),
            lang_requested: String::new(),
            run,
            run_started: false,
            run_closed: false,
        }
    }

    /// Start the thread that reports a script the frame loop has stopped
    /// pressing (`watch_script`); only with a done file to tell.
    pub(crate) fn start_watch(&mut self, ctx: &egui::Context) {
        let Some(done) = self.done.clone() else { return };
        let watch =
            Arc::new(Mutex::new(crate::ScriptWatch { left: self.left(None), at: self.progress_at, due: Duration::ZERO, finished: false }));
        crate::watch_script(watch.clone(), self.labels.clone(), done, ctx.clone(), self.failed.clone());
        self.watch = Some(watch);
    }

    /// The steps of the script not yet carried out: distinct step indices,
    /// not queue entries, and the step whose picture is awaited (or `extra`,
    /// the one that has just failed) counts as not carried out.
    fn left(&self, extra: Option<usize>) -> usize {
        let mut steps: Vec<usize> = self.queue.iter().map(|(i, _)| *i).chain(extra).chain(self.shot.as_ref().map(|s| s.step)).collect();
        steps.sort_unstable();
        steps.dedup();
        steps.len()
    }

    fn label(&self, step: usize) -> String {
        self.labels.get(step).cloned().unwrap_or_else(|| format!("step {}", step + 1))
    }

    /// Tell the watchdog the script has moved.
    fn note_progress(&self) {
        let Some(watch) = &self.watch else { return };
        if let Ok(mut w) = watch.lock() {
            w.left = self.left(None);
            w.at = self.progress_at;
            w.due = match self.queue.front() {
                Some((_, Press::Wait(d))) => *d,
                _ => Duration::ZERO,
            };
            w.finished = self.done.is_none();
        }
    }

    /// Write the run's report once. The write and `finished` are one step
    /// under the watchdog's lock, so its stalled report cannot land after
    /// (and replace) this one. `true` when there was nothing to write or it was written.
    fn write_done(&mut self, text: &str) -> bool {
        let Some(path) = self.done.take() else { return true };
        let guard = self.watch.as_ref().and_then(|w| w.lock().ok());
        let ok = write_report(&path, text, &self.failed);
        if let Some(mut w) = guard {
            w.finished = true;
        }
        ok
    }

    /// End the run as failed at script step `step`: the report, the line on
    /// standard error, the toast for a person watching, and no more keys.
    fn fail(&mut self, app: &mut App, step: usize, why: String) -> Outcome {
        let label = self.label(step);
        let left = self.left(Some(step));
        self.fail_as(app, &label, left, why)
    }

    /// [`Self::fail`] under a label of its own, for a failure that is not a script step's.
    fn fail_as(&mut self, app: &mut App, label: &str, left: usize, why: String) -> Outcome {
        self.failed.store(true, Ordering::SeqCst);
        say_err(&format!("failed: step {label}: {why}"));
        self.queue.clear();
        self.shot = None;
        // Nothing waits any more, so `due` stops asking for frames.
        self.check_since = None;
        self.read_has_since = None;
        let how = pace::How::Failed { step: label.to_owned(), why: why.clone() };
        // An ending that already fails keeps its tail, whether or not `run.json` could be written.
        let (tail, _) = self.keep_run(how, keyscript::failed_report(label, &why, left), left);
        let report = crate::state_report(app) + &tail;
        self.write_done(&report);
        app.error(format!("{label}: {why}"));
        Outcome::Failed
    }

    /// The first `run.json`, `running`, at the first frame the player handles (ito's `start_run`). Only with a done
    /// file. A failure to write it fails the run at `<run.json>`; there is no folder for the last one either.
    fn start_run(&mut self, app: &mut App) -> Option<Outcome> {
        if self.run_started || self.run_closed || self.done.is_none() {
            return None;
        }
        self.run_started = true;
        self.run.run_id = keyscript::current_run_id().map(str::to_owned);
        if let Err(err) = self.run.write(&self.shot_dir) {
            self.run_closed = true;
            let left = self.left(None);
            let why = format!("cannot write {}: {err}", evidence::FILE);
            return Some(self.fail_as(app, evidence::STEP, left, why));
        }
        None
    }

    /// The run ends as `how` says: write the last `run.json` (ito's `keep`). `tail` is what follows the state report
    /// in the done file. Returns the tail to write and whether a failure to write `run.json` turned a success into a
    /// failure (the file on disk then stays at its `running` record). Nothing without a done file or a second time.
    fn keep_run(&mut self, how: pace::How, tail: String, left: usize) -> (String, bool) {
        if self.run_closed || self.done.is_none() {
            return (tail, false);
        }
        self.run_closed = true;
        self.run_started = true;
        self.run.run_id = keyscript::current_run_id().map(str::to_owned);
        let ended = pace::Ended { how, tail, say: None, failed: self.failed.load(Ordering::SeqCst), left };
        self.run.end(&ended, evidence::now_ms());
        let Err(err) = self.run.write(&self.shot_dir) else { return (ended.tail, false) };
        let why = format!("cannot write {}: {err}", evidence::FILE);
        {
            use std::io::Write;
            let _ = writeln!(std::io::stderr(), "keys: {why}");
        }
        self.failed.store(true, Ordering::SeqCst);
        if matches!(ended.how, pace::How::Done) || ended.tail.ends_with("keys: done\n") {
            return (keyscript::failed_report(evidence::STEP, &why, left), true);
        }
        (ended.tail, false)
    }

    /// Remember that step `idx` went in on a screen that had not settled.
    fn record_unsettled(&mut self, idx: usize) {
        self.failed.store(true, Ordering::SeqCst);
        if !self.unsettled.contains(&idx) {
            self.unsettled.push(idx);
        }
    }

    /// The last lines of the report when anything went in unsettled, in place of `keys: done`.
    fn unsettled_lines(&self) -> Option<String> {
        if self.unsettled.is_empty() && !self.unsettled_end {
            return None;
        }
        let secs = self.settle_wait.as_secs();
        let mut out = String::new();
        if !self.unsettled.is_empty() {
            let named: Vec<String> = self.unsettled.iter().map(|i| self.label(*i)).collect();
            out += &format!(
                "unsettled: {} of {} steps went in after {secs} s without settling: {}\n",
                named.len(),
                self.labels.len(),
                named.join(" ")
            );
        }
        if self.unsettled_end {
            out += &format!("unsettled: the script ended after {secs} s without settling\n");
        }
        out += "keys: unsettled\n";
        Some(out)
    }

    /// `--keys`: the next scripted key, in the raw input a keyboard would have
    /// filled -- so it takes every road a real press takes.
    ///
    /// One key at a time, and only once what the last one started has landed
    /// (`App::settled`): `<Tab>C` copying a panel whose sections are still on
    /// the worker would copy half of it. Two frames apart at least, so what the
    /// key opened has been drawn once; and never more than the settle wait
    /// behind, so a thing that never settles delays the script rather than
    /// stopping it -- but a key that went in that way is named in the report,
    /// and the run does not end as a success.
    pub(crate) fn hook(&mut self, ctx: &egui::Context, app: &mut App, raw_input: &mut egui::RawInput, frame: u64, now: Instant) -> Outcome {
        if self.finished || (self.queue.is_empty() && self.shot.is_none() && self.done.is_none()) {
            return Outcome::Continue;
        }
        if let Some(out) = self.start_run(app) {
            return out;
        }
        if self.reads_window {
            keyscript::observe(ctx);
        }
        // Another window's frame is not the main window's: nothing goes in on
        // it. An input waiting to be drawn that meets one at the same frame
        // went to a window that was not drawn.
        if raw_input.viewport_id != egui::ViewportId::ROOT {
            if let Some(f) = self.inflight.as_ref().filter(|f| f.frame == frame) {
                let why = format!("the input went to viewport {:?} while the main window was not drawn", raw_input.viewport_id);
                let step = f.step;
                return self.fail(app, step, why);
            }
            return Outcome::Continue;
        }
        if self.inflight.as_ref().is_some_and(|f| frame > f.frame) {
            self.inflight = None;
            self.action_gen = keyscript::generation(ctx);
            self.progress_at = now;
            self.note_progress();
        }
        let answers = app.config_writes.answers();
        if answers != self.answers_seen {
            self.answers_seen = answers;
            // A reading after `<Theme:>` / `<Lang:>` needs a tree drawn after the new config was applied.
            self.action_gen = keyscript::generation(ctx);
            self.progress_at = now;
            self.note_progress();
        }
        // A config write that failed, or has not been answered, holds the
        // script: what it reads next would be the old config.
        if let Some(msg) = app.config_write_failed.clone() {
            let left = self.left(None);
            return self.fail_as(app, CONFIG_STEP, left, msg);
        }
        if app.config_writes_pending() {
            if let Some((seq, file)) = app.config_writes.overdue(now, CONFIG_WAIT) {
                let why = format!("no answer from the config writer within {} s of write #{seq} ({file})", CONFIG_WAIT.as_secs());
                let left = self.left(None);
                return self.fail_as(app, CONFIG_STEP, left, why);
            }
            ctx.request_repaint();
            return Outcome::Continue;
        }
        if let Some(f) = &self.inflight {
            if now.saturating_duration_since(f.at) >= DELIVER_WAIT {
                let why =
                    format!("the input was not drawn within {} s (the window may be hidden, minimized or covered)", DELIVER_WAIT.as_secs());
                let step = f.step;
                return self.fail(app, step, why);
            }
        }
        let (last_frame, last_at) = self.at;
        let since = now.saturating_duration_since(last_at);
        let settled = app.settled();
        // Inside a `<Wait:N>` once the key before it has been drawn and has
        // settled: sleep out the rest instead of drawing frames that change
        // nothing. Repainting through the wait cost about 1 CPU second in 10
        // on every backend (#287, proposal 1), which is what 47.1 measures.
        if let Some((_, Press::Wait(d))) = self.queue.front() {
            let left = d.saturating_sub(since);
            if !left.is_zero() && self.shot.is_none() && frame >= last_frame + 2 && settled {
                ctx.request_repaint_after(left);
                return Outcome::Continue;
            }
        }
        ctx.request_repaint();
        let waited_long = since > self.settle_wait;
        // A picture being taken holds the next key until it is saved. One that
        // never comes is a missing file: the run failed.
        if let Some(shot) = &self.shot {
            if now.saturating_duration_since(shot.since) <= self.shot_deadline {
                return Outcome::Continue;
            }
            let (step, path) = (shot.step, self.shot_path(&shot.name));
            let why = format!("no picture arrived within {} s; {} was not saved", self.shot_deadline.as_secs(), path.display());
            return self.fail(app, step, why);
        }
        if matches!(self.queue.front(), Some((_, Press::Now))) {
            self.queue.pop_front();
            self.now = true;
        }
        // `<Now>`: one frame after the last key is enough -- it has been
        // handed to the app, which is all the next one needs.
        // A `<ReadHas:>` that has started looking reads again every frame, settled or not.
        // A retrying pure check (`<Assert:>` and kin) does the same.
        let ready = if self.now || self.read_has_since.is_some() || self.pure_retry() {
            frame > last_frame
        } else {
            frame >= last_frame + 2 && (settled || waited_long)
        };
        if !ready {
            return Outcome::Continue;
        }
        // Past the settle wait on a screen that is still moving, outside `<Now>`.
        let late = !self.now && !settled;
        // The same wait as before a key: the last one has been drawn and has
        // settled, so what is on screen now is its result.
        let Some((idx, press)) = self.queue.pop_front() else {
            self.finished = true;
            self.unsettled_end = late;
            let lines = self.unsettled_lines();
            let tail = lines.clone().unwrap_or_else(|| "keys: done\n".to_owned());
            if let Some(lines) = &lines {
                self.failed.store(true, Ordering::SeqCst);
                say_err(&lines.lines().collect::<Vec<_>>().join("; "));
            }
            let how = if lines.is_some() { pace::How::Unsettled } else { pace::How::Done };
            let (tail, run_failed) = self.keep_run(how, tail, 0);
            let ok = self.write_done(&(crate::state_report(app) + &tail));
            self.note_progress();
            return match (ok && !run_failed, lines.is_some()) {
                (false, _) => Outcome::Failed,
                (true, true) => Outcome::Unsettled,
                (true, false) => Outcome::Continue,
            };
        };
        // A `<Wait:N>` presses nothing: it is not named when the screen was unsettled.
        let mut record = true;
        // `<Now>` is for the step after it only; a wait is not a step it can be spent on.
        let is_wait = matches!(press, Press::Wait(_));
        let mut ran = true;
        // A window-reading step that has to look again goes back on the queue as it was.
        let again = reads_window(&press).then(|| press.clone());
        match press {
            // Counted from the key before it: the next key goes in at least
            // this long after that one, and still only once things settle.
            Press::Wait(d) => {
                record = false;
                let left = d.saturating_sub(since);
                if left.is_zero() {
                    self.at = (frame, now);
                } else {
                    ran = false;
                    self.queue.push_front((idx, Press::Wait(d)));
                    ctx.request_repaint_after(left);
                }
            }
            Press::Events(events) => {
                self.act(ctx, now, idx);
                raw_input.events.extend(events);
                self.inject(idx, frame, now);
            }
            // Read as it goes in, not when the script was parsed: an earlier
            // key of the script may have put something on the clipboard.
            Press::Paste => {
                self.act(ctx, now, idx);
                raw_input.events.push(keyscript::paste_event());
                self.inject(idx, frame, now);
            }
            Press::PasteText(text) => {
                self.act(ctx, now, idx);
                raw_input.events.push(keyscript::paste_text_event(&text));
                self.inject(idx, frame, now);
            }
            p @ (Press::Click { .. } | Press::DClick { .. } | Press::Hover { .. } | Press::Wheel { .. } | Press::Drag { .. }) => {
                self.act(ctx, now, idx);
                let rect = ctx.input(|i| i.viewport_rect());
                let mut events = keyscript::pointer_events(&p, rect);
                // A drag one event per frame: in one frame egui never sees the
                // button held while the pointer moves, so nothing starts to
                // drag (#283, #284). The first goes in now, the rest follow
                // as steps of their own, each after the frame has settled.
                if matches!(p, Press::Drag { .. }) && events.len() > 1 {
                    let rest = events.split_off(1);
                    for e in rest.into_iter().rev() {
                        self.queue.push_front((idx, Press::Events(vec![e])));
                    }
                }
                raw_input.events.extend(events);
                self.inject(idx, frame, now);
            }
            // Written now: the wait above is the one a key gets, so this is
            // the state the key before it left (#230).
            Press::State(name) => {
                let path = self.shot_dir.join(format!("{name}.txt"));
                let text = crate::state_report(app)
                    + &term_state_lines(app)
                    + &format!(
                        "cursor icon: {:?}\nsettled: {}\nconfig writes pending: {}\nconfig write failed: {}\n",
                        self.cursor_icon,
                        if settled { "yes" } else { "no" },
                        if app.config_writes_pending() { "yes" } else { "no" },
                        app.config_write_failed.as_deref().unwrap_or("none"),
                    )
                    + &keyscript::since_action_line(self.acted.map(|t| now.saturating_duration_since(t)));
                if let Err(why) = write_text(&path, &text) {
                    return self.fail(app, idx, why);
                }
                self.at = (frame, now);
            }
            // The readings kura adds to the common script (`keyscript::DIALECT`).
            Press::Read { tag, name } => {
                let written = match tag.as_str() {
                    // The pane's own text, beside the state: `name.pane.txt`, so a
                    // `<State:name>` of the same name does not take its place (#229).
                    "PaneText" => {
                        let text = app.term.as_ref().map_or_else(|| "(no pane)\n".to_owned(), |t| t.screen_text());
                        write_text(&self.shot_dir.join(format!("{name}.pane.txt")), &text)
                    }
                    "PreviewText" => {
                        write_text(&self.shot_dir.join(format!("{name}.preview.txt")), &crate::preview_text(&app.preview.state))
                    }
                    other => Err(format!("kura's --keys has no reading called {other}")),
                };
                if let Err(why) = written {
                    return self.fail(app, idx, why);
                }
                self.at = (frame, now);
            }
            // Reads again each frame until the text is in the reading, for up to `READ_WAIT`.
            // It presses nothing and is a check that tolerates a moving screen, so it is not named as unsettled.
            Press::ReadHas { tag, text } => {
                record = false;
                let since = *self.read_has_since.get_or_insert(now);
                let got = reading(app, &tag).unwrap_or_default();
                if got.contains(&text) {
                    self.read_has_since = None;
                    self.at = (frame, now);
                } else if now.saturating_duration_since(since) < keyscript::READ_WAIT {
                    ran = false;
                    self.queue.push_front((idx, Press::ReadHas { tag, text }));
                } else {
                    self.read_has_since = None;
                    let shown: String = got.chars().take(200).collect();
                    let more = if got.chars().count() > 200 { "..." } else { "" };
                    let why =
                        format!("{tag} did not contain {text:?} within {} s; it read {shown:?}{more}", keyscript::READ_WAIT.as_secs());
                    return self.fail(app, idx, why);
                }
            }
            // Straight to the quit `ui` already handles, past anything that
            // would ask first or take `q` for itself (#236).
            Press::Quit => {
                app.quit = true;
                self.at = (frame, now);
            }
            // Taken in `ui`, which is where a viewport command can be sent.
            Press::Shot(name) => {
                self.shot = Some(Shot { name, sent: false, step: idx, since: now, theme: String::new(), lang: String::new() });
                self.at = (frame, now);
            }
            // The window lays out again at the new size before the next step.
            Press::Size { w, h } => {
                self.act(ctx, now, idx);
                ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(w as f32, h as f32)));
                self.queue.push_front((idx, Press::Wait(Duration::from_millis(400))));
                self.at = (frame, now);
            }
            // The same write the settings screen makes: common.toml, applied
            // as the watcher would.
            Press::Theme(name) => {
                self.act(ctx, now, idx);
                self.theme_requested = name.clone();
                app.write_common(vec![ito_common::CommonChange { table: None, key: "theme", value: format!("{name:?}") }]);
                self.at = (frame, now);
            }
            Press::Lang(code) => {
                self.act(ctx, now, idx);
                self.lang_requested = code.clone();
                app.write_common(vec![ito_common::CommonChange { table: None, key: "language", value: format!("{code:?}") }]);
                self.at = (frame, now);
            }
            // Taken before the wait above; `parse` puts a key after every one.
            Press::Now => {}
            // Refused before the window opens (`unsupported`); a run handed one anyway fails on it.
            Press::Snap { name, .. } => {
                return self.fail(app, idx, format!("<Snap:{name}>: {UNSUPPORTED_WHY}"));
            }
            // The window-reading steps. Each needs a tree drawn after the last action; what it looks
            // for is waited for up to `TEXT_WAIT`, then the run fails with the reason.
            Press::ClickText { right, text } => {
                let found = self.fresh(ctx).and_then(|_| keyscript::locate(ctx, &text));
                match found {
                    Ok(pos) => {
                        self.check_since = None;
                        self.act(ctx, now, idx);
                        raw_input.events.extend(click_events(pos, right, false));
                        self.inject(idx, frame, now);
                    }
                    Err(why) => return self.look_again(app, idx, again, why, now, &mut ran, &mut record),
                }
            }
            Press::DClickText(text) => {
                let found = self.fresh(ctx).and_then(|_| keyscript::locate(ctx, &text));
                match found {
                    Ok(pos) => {
                        self.check_since = None;
                        self.act(ctx, now, idx);
                        raw_input.events.extend(click_events(pos, false, true));
                        self.inject(idx, frame, now);
                    }
                    Err(why) => return self.look_again(app, idx, again, why, now, &mut ran, &mut record),
                }
            }
            Press::ClickId(name) => {
                let found = self.fresh(ctx).and_then(|o| match o.id(&name)? {
                    Some(node) => node.click_at.clone(),
                    None => Err(format!("no widget has the test ID `{name}`")),
                });
                match found {
                    Ok(pos) => {
                        self.check_since = None;
                        self.act(ctx, now, idx);
                        raw_input.events.extend(click_events(pos, false, false));
                        self.inject(idx, frame, now);
                    }
                    Err(why) => return self.look_again(app, idx, again, why, now, &mut ran, &mut record),
                }
            }
            // Pure checks: they press nothing and tolerate a moving screen, like `<ReadHas:>`.
            Press::Assert { text, present } => {
                record = false;
                let checked = self.fresh(ctx).and_then(|_| keyscript::check(ctx, &text, present));
                if let Err(out) = self.checked(app, idx, again, checked, frame, now, &mut ran) {
                    return out;
                }
            }
            Press::Is { prop, text } => {
                record = false;
                let checked = self.fresh(ctx).and_then(|_| keyscript::check_is(ctx, prop, &text));
                if let Err(out) = self.checked(app, idx, again, checked, frame, now, &mut ran) {
                    return out;
                }
            }
            Press::IdCheck(check) => {
                record = false;
                let checked = self.fresh(ctx).and_then(|o| check.check(&o));
                if let Err(out) = self.checked(app, idx, again, checked, frame, now, &mut ran) {
                    return out;
                }
            }
            Press::Announced(text) => {
                record = false;
                let checked = self.since.map_or_else(|| keyscript::check_announced(&text), |m| keyscript::check_announced_after(m, &text));
                if let Err(out) = self.checked(app, idx, again, checked, frame, now, &mut ran) {
                    return out;
                }
            }
            Press::Tree(name) => {
                let tree = match self.fresh(ctx) {
                    Ok(o) if o.tree.is_empty() => Err("the window's tree was empty".to_owned()),
                    other => other,
                };
                match tree {
                    Ok(_) => {
                        self.check_since = None;
                        // The same file name as a `<State:>` of that name, which it overwrites (as in ito).
                        let path = self.shot_dir.join(format!("{name}.txt"));
                        if let Err(e) = std::fs::write(&path, keyscript::tree_text(ctx)) {
                            return self.fail(app, idx, format!("cannot write {}: {e}", path.display()));
                        }
                        self.at = (frame, now);
                    }
                    Err(why) => return self.look_again(app, idx, again, why, now, &mut ran, &mut record),
                }
            }
            // What kura copied after the last action (ito's OS clipboard writer notes each copy for its record).
            Press::Clipboard(text) => {
                record = false;
                let checked = keyscript::check_clipboard_after(ctx, self.since.unwrap_or_default(), &text);
                if let Err(out) = self.checked(app, idx, again, checked, frame, now, &mut ran) {
                    return out;
                }
            }
            Press::ClipboardHas(text) => {
                record = false;
                let checked = keyscript::check_clipboard_has_after(ctx, self.since.unwrap_or_default(), &text);
                if let Err(out) = self.checked(app, idx, again, checked, frame, now, &mut ran) {
                    return out;
                }
            }
        }
        if record && late {
            self.record_unsettled(idx);
        }
        // A step that looked again has not been spent on `<Now>`.
        if !is_wait && ran {
            self.now = false;
        }
        if ran {
            self.progress_at = now;
        }
        self.note_progress();
        Outcome::Continue
    }

    /// An action's first input goes in now: what the app copied and announced from here on is what
    /// `<Announced:>` looks at, and `<State:>` says how long ago it was. Once per script step.
    fn act(&mut self, ctx: &egui::Context, now: Instant, idx: usize) {
        if self.acted_idx != Some(idx) {
            self.since = Some(keyscript::mark(ctx));
            self.acted = Some(now);
            self.acted_idx = Some(idx);
        }
    }

    /// The window as a reading step may judge it: seen after the last input was drawn, in the main
    /// window, and not minimized (ito's `Runner::fresh`).
    fn fresh(&self, ctx: &egui::Context) -> Result<keyscript::Observation, String> {
        match keyscript::observation(ctx) {
            Some(o) if o.generation > self.action_gen && o.viewport == egui::ViewportId::ROOT => {
                if o.minimized == Some(true) {
                    Err("the window is minimized".into())
                } else {
                    Ok(o)
                }
            }
            _ => Err("the window's tree was not updated after the last action (the window may be hidden, minimized or covered)".into()),
        }
    }

    /// A pure check is waiting for what it looks for: it looks again every frame.
    fn pure_retry(&self) -> bool {
        self.check_since.is_some() && self.queue.front().is_some_and(|(_, p)| pure_check(p))
    }

    /// The end of a pure check: passed (the retry state is cleared and the step counts as run), or
    /// `Err` with the outcome to return (look again, or the run failed).
    #[allow(clippy::too_many_arguments)]
    fn checked(
        &mut self,
        app: &mut App,
        idx: usize,
        again: Option<Press>,
        result: Result<(), String>,
        frame: u64,
        now: Instant,
        ran: &mut bool,
    ) -> Result<(), Outcome> {
        match result {
            Ok(()) => {
                self.check_since = None;
                self.at = (frame, now);
                Ok(())
            }
            Err(why) => {
                let mut record = false;
                Err(self.look_again(app, idx, again, why, now, ran, &mut record))
            }
        }
    }

    /// A window-reading step did not find what it looks for: put it back to look again, until
    /// `TEXT_WAIT` has passed, then fail the run with `why`. Nothing was pressed, so it neither
    /// moves `at` nor counts as progress or as unsettled.
    #[allow(clippy::too_many_arguments)]
    fn look_again(
        &mut self,
        app: &mut App,
        idx: usize,
        again: Option<Press>,
        why: String,
        now: Instant,
        ran: &mut bool,
        record: &mut bool,
    ) -> Outcome {
        let since = *self.check_since.get_or_insert(now);
        match again {
            Some(press) if now.saturating_duration_since(since) < keyscript::TEXT_WAIT => {
                *ran = false;
                *record = false;
                self.queue.push_front((idx, press));
                self.note_progress();
                Outcome::Continue
            }
            _ => {
                self.check_since = None;
                self.fail(app, idx, why)
            }
        }
    }

    /// When the next frame is wanted for a retry that waits: a repaint asked for in `hook` can be
    /// forgotten when the pass begins (egui resets the delay then), so a quiet window could stop
    /// drawing while a check waits, and the watchdog only pokes once a second.
    pub(crate) fn due(&self) -> Option<Duration> {
        (self.check_since.is_some() || self.read_has_since.is_some()).then_some(Duration::from_millis(16))
    }

    /// A step handed input to the main window on this frame: it is in flight until a later one is drawn.
    fn inject(&mut self, step: usize, frame: u64, now: Instant) {
        self.at = (frame, now);
        self.now = false;
        self.inflight = Some(Inflight { frame, at: now, step });
    }

    /// Where `<Shot:name>` saves: `shots/name.png` beside the done file (`name@preview` is the preview pane's crop of `name.png`).
    fn shot_path(&self, name: &str) -> PathBuf {
        self.shot_dir.join("shots").join(format!("{}.png", shot_file(name)))
    }

    /// `<Shot:name>`: ask for the picture, then save it when it arrives.
    pub(crate) fn take_shot(&mut self, ctx: &egui::Context, app: &mut App) -> Outcome {
        let Some(shot) = &mut self.shot else { return Outcome::Continue };
        if !shot.sent {
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::new(shot.name.clone())));
            // What is in force now is what the picture shows.
            shot.theme = app.theme_name.clone().unwrap_or_default();
            shot.lang = app.lang.clone();
            shot.sent = true;
            ctx.request_repaint();
            return Outcome::Continue;
        }
        let (name, step, in_force) = (shot.name.clone(), shot.step, (shot.theme.clone(), shot.lang.clone()));
        let image = ctx.input(|i| {
            i.raw.events.iter().find_map(|e| match e {
                egui::Event::Screenshot { user_data, image, .. }
                    if user_data.data.as_ref().and_then(|d| d.downcast_ref::<String>()) == Some(&name) =>
                {
                    Some(image.clone())
                }
                _ => None,
            })
        });
        let Some(image) = image else {
            ctx.request_repaint();
            return Outcome::Continue;
        };
        // `name@preview` is the preview pane's rectangle, in the picture's pixels.
        let crop = name.ends_with(keyscript::SHOT_PREVIEW);
        let png = self.shot_path(&name);
        let saved = png
            .parent()
            .map_or(Ok(()), |d| std::fs::create_dir_all(d).map_err(|e| format!("could not make {}: {e}", d.display())))
            .and_then(|()| save_shot(&image, crop, app.preview.rect, ctx.content_rect().width(), &png))
            .and_then(|_| {
                let screen = ctx.content_rect();
                let seen = Seen {
                    name: shot_file(&name),
                    theme: &in_force.0,
                    lang: &in_force.1,
                    requested: (&self.theme_requested, &self.lang_requested),
                    size: [screen.width(), screen.height()],
                    preview: if crop { app.preview.rect } else { None },
                    pixels_per_point: ctx.pixels_per_point(),
                };
                write_shot_json(&png.with_extension("json"), &seen)
            });
        match saved {
            Ok(()) => {
                self.shot = None;
                self.progress_at = Instant::now();
                self.note_progress();
                Outcome::Continue
            }
            Err(why) => self.fail(app, step, why),
        }
    }

    /// The window was closed (the title bar's x) or the app quit while the
    /// script ran: write what is left of the report. The picture of a
    /// `<Shot:>` still awaited is an unfinished step, whatever the queue says.
    /// The caller runs the close action of its path either way.
    pub(crate) fn ended(&mut self, app: &mut App, how: Ending) -> Outcome {
        if let Some(shot) = &self.shot {
            let (step, name) = (shot.step, shot.name.clone());
            let what = if how == Ending::Closed { "the window was closed" } else { "kura quit" };
            return self.fail(app, step, format!("{what} before the picture of <Shot:{name}> was saved"));
        }
        if self.done.is_none() {
            return Outcome::Continue;
        }
        if let Some(msg) = app.config_write_failed.clone() {
            let left = self.left(None);
            return self.fail_as(app, CONFIG_STEP, left, msg);
        }
        if app.config_writes_pending() {
            let file = app.config_writes.oldest_file().unwrap_or("config");
            let what = if how == Ending::Closed { "the window was closed" } else { "kura quit" };
            let why = format!("{what} before the config writer answered ({file})");
            let left = self.left(None);
            return self.fail_as(app, CONFIG_STEP, left, why);
        }
        let left = self.left(None);
        let (lines, base) = match how {
            Ending::Closed => (self.unsettled_lines(), keyscript::closed_report(left)),
            Ending::Quit => (self.unsettled_lines(), keyscript::quit_report(left)),
        };
        // A `keys: done` ending gives way to the unsettled lines. `keys: closed` /
        // `keys: quit` stay last, since they say what was left, with the unsettled lines before them.
        let tail = match (&lines, base.strip_suffix("keys: done\n")) {
            (Some(lines), Some(head)) => format!("{head}{lines}"),
            (Some(lines), None) => format!("{}{base}", lines.strip_suffix("keys: unsettled\n").unwrap_or(lines)),
            (None, _) => base,
        };
        if lines.is_some() {
            self.failed.store(true, Ordering::SeqCst);
        }
        self.finished = true;
        let how = if how == Ending::Closed { pace::How::Closed } else { pace::How::Quit };
        let (tail, run_failed) = self.keep_run(how, tail, left);
        let ok = self.write_done(&(crate::state_report(app) + &tail));
        self.note_progress();
        if !ok || run_failed {
            Outcome::Failed
        } else if lines.is_some() {
            Outcome::Unsettled
        } else {
            Outcome::Continue
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::harness::Screen;

    struct Rig {
        dir: PathBuf,
        screen: Screen,
        failed: Arc<AtomicBool>,
        t0: Instant,
        frame: u64,
    }

    impl Rig {
        /// A screen whose `settled()` is what the test says, in a fresh folder.
        fn new(label: &str, settled: bool) -> Self {
            Self::with_files(label, settled, &[])
        }

        /// `new`, with more files in the folder before the screen opens it.
        fn with_files(label: &str, settled: bool, files: &[&str]) -> Self {
            let dir = crate::util::test_dir(label);
            std::fs::write(dir.join("a.txt"), "a").unwrap();
            for f in files {
                std::fs::write(dir.join(f), "x").unwrap();
            }
            let mut screen = Screen::open(dir.clone());
            screen.settle();
            if !settled {
                screen.app.preview.in_flight = true;
            }
            assert_eq!(screen.app.settled(), settled, "the rig's screen");
            Rig { dir, screen, failed: Arc::new(AtomicBool::new(false)), t0: Instant::now(), frame: 2 }
        }

        fn run(&self, script: &str, done: Option<PathBuf>, shot_dir: PathBuf) -> ScriptRun {
            let steps = keyscript::parse(script).unwrap();
            let mut run = ScriptRun::new(&steps, done, shot_dir, self.failed.clone());
            run.at = (0, self.t0);
            run.progress_at = self.t0;
            run
        }

        /// One frame of the player, `secs` after the last, two frames on.
        fn step(&mut self, run: &mut ScriptRun, secs: u64) -> Outcome {
            self.frame += 2;
            self.t0 += Duration::from_secs(secs);
            let mut raw = egui::RawInput::default();
            let ctx = self.screen.ctx.clone();
            run.hook(&ctx, &mut self.screen.app, &mut raw, self.frame, self.t0)
        }

        /// The hook alone (nothing is drawn), `by` after the last, on a frame later than any drawn so far.
        fn hook_by(&mut self, run: &mut ScriptRun, by: Duration) -> Outcome {
            self.hook_events(run, by).0
        }

        /// `hook_by`, with the events the hook injected (to be fed later, or never).
        fn hook_events(&mut self, run: &mut ScriptRun, by: Duration) -> (Outcome, Vec<egui::Event>) {
            let ctx = self.screen.ctx.clone();
            self.frame = self.frame.max(ctx.cumulative_frame_nr_for(egui::ViewportId::ROOT)) + 2;
            self.t0 += by;
            let mut raw = egui::RawInput::default();
            let out = run.hook(&ctx, &mut self.screen.app, &mut raw, self.frame, self.t0);
            (out, raw.events)
        }

        /// A frame of the player `secs` after the last, `frames` on, in the viewport `id`.
        fn tick(&mut self, run: &mut ScriptRun, secs: u64, frames: u64, id: egui::ViewportId) -> Outcome {
            self.frame += frames;
            self.t0 += Duration::from_secs(secs);
            let mut raw = egui::RawInput { viewport_id: id, ..Default::default() };
            let ctx = self.screen.ctx.clone();
            run.hook(&ctx, &mut self.screen.app, &mut raw, self.frame, self.t0)
        }

        /// A folder for common.toml under the rig's, with a file in it, so a
        /// `<Theme:>` has a real writer to wait for.
        fn common_folder(&mut self) {
            let base = self.dir.join("uchmk");
            std::fs::create_dir_all(&base).unwrap();
            std::fs::write(base.join("common.toml"), "theme = \"light\"\n").unwrap();
            self.screen.app.common_base = Some(base);
        }

        /// Let the writers answer: drain the channels until nothing is pending.
        fn answer(&mut self) {
            let ctx = self.screen.ctx.clone();
            let until = Instant::now() + Duration::from_secs(10);
            while self.screen.app.config_writes_pending() {
                assert!(Instant::now() < until, "the writer answered");
                self.screen.app.drain_channels(&ctx);
                std::thread::sleep(Duration::from_millis(5));
            }
        }

        /// A real egui pass after the hook: what the hook injected is fed to the screen, so the
        /// AccessKit tree, the Tap and its generations are the real ones.
        fn pass(&mut self, run: &mut ScriptRun, secs: u64) -> Outcome {
            self.frame += 2;
            self.t0 += Duration::from_secs(secs);
            let mut raw = egui::RawInput::default();
            let ctx = self.screen.ctx.clone();
            let out = run.hook(&ctx, &mut self.screen.app, &mut raw, ctx.cumulative_frame_nr_for(egui::ViewportId::ROOT), self.t0);
            self.screen.feed(std::mem::take(&mut raw.events));
            out
        }

        fn flag(&self) -> bool {
            self.failed.load(Ordering::SeqCst)
        }
    }

    fn read(path: &Path) -> String {
        std::fs::read_to_string(path).unwrap()
    }

    /// A report without its `run:` line, to look at how the rest ends.
    fn body(report: &str) -> String {
        report.lines().filter(|l| !l.starts_with("run: ")).map(|l| format!("{l}\n")).collect()
    }

    /// A path under a regular file: nothing can be created there, on any OS.
    fn unwritable(dir: &Path) -> PathBuf {
        let file = dir.join("in-the-way");
        std::fs::write(&file, "x").unwrap();
        file.join("sub")
    }

    #[test]
    fn only_steps_kura_carries_out_are_accepted() {
        let p = |s: &str| keyscript::press(&keyscript::parse(s).unwrap()[0]).unwrap();
        for ok in
            ["j", "<Wait:5>", "<Shot:x>", "<State:x>", "<PaneText:x>", "<PreviewText:x>", "<Quit>", "<Click:0.1,0.1>", "<Size:640x480>"]
        {
            assert_eq!(unsupported(&p(ok)), None, "{ok}");
        }
        assert_eq!(unsupported(&p("<ReadHas:PaneText=x>")), None);
        assert_eq!(unsupported(&p("<ReadHas:PreviewText=x>")), None);
        assert!(unsupported(&Press::ReadHas { tag: "Bogus".into(), text: "x".into() }).is_some());
        for ok in [
            "<Assert:foo>",
            "<Assert:!foo>",
            "<Click:Name>",
            "<RClick:a>",
            "<DClick:a>",
            "<Tree:t>",
            "<ClickId:a><Id:a>",
            "<Id:a>",
            "<ValueId:a=b>",
            "<Announced:x>",
        ] {
            assert_eq!(unsupported(&p(ok)), None, "{ok}");
        }
        // The clipboard steps are carried out too.
        for ok in ["<Clipboard:x>", "<ClipboardHas:x>"] {
            assert_eq!(unsupported(&p(ok)), None, "{ok}");
        }
        let unknown = Press::Read { tag: "Bogus".into(), name: "n".into() };
        assert!(unsupported(&unknown).is_some());
        // ito v0.276.0's `<Snap:>` writes state JSON, which kura does not: refused before the first key.
        assert!(unsupported(&p("<Snap:x>")).is_some());
    }

    /// The key script `scripts/check-term-settings.sh` sends runs only on Linux CI, so a step kura
    /// would refuse (a typo, a reading it does not have) is caught here, on every machine, first.
    #[test]
    fn the_term_settings_check_sends_only_steps_kura_carries_out() {
        let sh = include_str!("../scripts/check-term-settings.sh");
        let mut lines = sh.lines().map(|l| l.trim_end_matches('\r'));
        assert!(lines.any(|l| l == "read -r -d '' KEYS <<'EOF'"), "no KEYS heredoc");
        let script: String = lines.take_while(|l| *l != "EOF").collect();
        assert!(script.contains("<Quit>"), "{script}");
        let steps = keyscript::parse(&script).unwrap_or_else(|why| panic!("{why}"));
        for (i, step) in steps.iter().enumerate() {
            let press = keyscript::press(step).unwrap_or_else(|| panic!("step {i} is not a key: {step:?}"));
            assert_eq!(unsupported(&press), None, "step {i}: {press:?}");
        }
    }

    /// The picture of `name` arrives in one pass, and the player takes it.
    fn deliver(rig: &mut Rig, run: &mut ScriptRun, name: &str) -> Outcome {
        let ctx = rig.screen.ctx.clone();
        // `ui` asks for the picture first, and takes it on a later pass.
        ctx.begin_pass(egui::RawInput::default());
        assert_eq!(run.take_shot(&ctx, &mut rig.screen.app), Outcome::Continue);
        let _ = ctx.end_pass();
        let image = std::sync::Arc::new(egui::ColorImage::filled([40, 30], egui::Color32::RED));
        let raw = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(40.0, 30.0))),
            events: vec![egui::Event::Screenshot {
                viewport_id: egui::ViewportId::ROOT,
                user_data: egui::UserData::new(name.to_owned()),
                image,
            }],
            ..Default::default()
        };
        ctx.begin_pass(raw);
        let out = run.take_shot(&ctx, &mut rig.screen.app);
        let _ = ctx.end_pass();
        out
    }

    /// A run with a done file in `out/`, the shot step sent and its picture delivered.
    fn shot_run(label: &str, script: &str, name: &str, preview: Option<egui::Rect>) -> (Rig, ScriptRun, PathBuf) {
        let mut rig = Rig::new(label, true);
        let out = rig.dir.join("out");
        std::fs::create_dir_all(&out).unwrap();
        let mut run = rig.run(script, Some(out.join("done.txt")), out.clone());
        assert_eq!(rig.step(&mut run, 0), Outcome::Continue, "the shot is asked for");
        rig.screen.app.preview.rect = preview;
        assert_eq!(deliver(&mut rig, &mut run, name), Outcome::Continue);
        (rig, run, out)
    }

    /// A file's text without spaces and newlines, to look for `"key":"value"` whatever the layout.
    fn squeezed(path: &Path) -> String {
        read(path).replace([' ', '\n'], "")
    }

    #[test]
    fn a_shot_lands_in_shots_with_its_json() {
        let (_rig, run, out) = shot_run("keys-shot-json", "<Shot:x>", "x", None);
        assert!(run.shot.is_none());
        assert!(out.join("shots").join("x.png").is_file());
        assert!(!out.join("x.png").exists(), "not beside the done file any more");
        let json = squeezed(&out.join("shots").join("x.json"));
        assert!(json.contains("\"name\":\"x\""), "{json}");
        assert!(json.contains("\"app\":\"kura\""), "{json}");
        assert!(json.contains("\"component\":\"window\""), "{json}");
    }

    #[test]
    fn a_preview_shot_is_the_previews_component_and_rect() {
        let rect = egui::Rect::from_min_max(egui::pos2(10.0, 5.0), egui::pos2(30.0, 25.0));
        let (_rig, _run, out) = shot_run("keys-shot-preview", "<Shot:x@preview>", "x@preview", Some(rect));
        assert!(out.join("shots").join("x.png").is_file());
        assert!(!out.join("shots").join("x@preview.png").exists());
        let json = squeezed(&out.join("shots").join("x.json"));
        assert!(json.contains("\"component\":\"preview\""), "{json}");
        assert!(json.contains("\"name\":\"x\""), "{json}");
    }

    #[test]
    fn a_shot_records_the_theme_and_language_in_force_and_asked_for() {
        let mut rig = Rig::new("keys-shot-asked", true);
        let out = rig.dir.join("out");
        std::fs::create_dir_all(&out).unwrap();
        let mut run = rig.run("<Shot:y>", Some(out.join("done.txt")), out.clone());
        run.theme_requested = "dark".into();
        run.lang_requested = "ja".into();
        rig.screen.app.theme_name = Some("dark".into());
        rig.screen.app.lang = "ja".into();
        assert_eq!(rig.step(&mut run, 0), Outcome::Continue);
        assert_eq!(deliver(&mut rig, &mut run, "y"), Outcome::Continue);
        let json = squeezed(&out.join("shots").join("y.json"));
        assert!(json.contains("\"theme\":\"dark\"") && json.contains("\"theme_requested\":\"dark\""), "{json}");
        assert!(json.contains("\"lang\":\"ja\"") && json.contains("\"lang_requested\":\"ja\""), "{json}");
    }

    #[test]
    fn a_file_named_shots_fails_the_shot_step() {
        let mut rig = Rig::new("keys-shots-file", true);
        let out = rig.dir.join("out");
        std::fs::create_dir_all(&out).unwrap();
        std::fs::write(out.join("shots"), "in the way").unwrap();
        let done = out.join("done.txt");
        let mut run = rig.run("<Shot:x>", Some(done.clone()), out.clone());
        assert_eq!(rig.step(&mut run, 0), Outcome::Continue);
        assert_eq!(deliver(&mut rig, &mut run, "x"), Outcome::Failed);
        let said = read(&done);
        assert!(said.contains("keys: failed\n") && said.contains("step: <Shot:x>\n"), "{said}");
        assert!(said.contains(&out.join("shots").display().to_string()), "names the path: {said}");
        assert!(!out.join("x.png").exists() && !out.join("x.json").exists());
        assert_eq!(read(&out.join("shots")), "in the way");
    }

    #[test]
    fn run_json_is_written_running_at_the_first_frame() {
        let mut rig = Rig::new("keys-runjson-start", true);
        let mut run = rig.run("<Shot:x>", Some(rig.dir.join("done.txt")), rig.dir.clone());
        assert!(!rig.dir.join("run.json").exists());
        assert_eq!(rig.step(&mut run, 0), Outcome::Continue);
        let json = squeezed(&rig.dir.join("run.json"));
        assert!(json.contains("\"status\":\"running\""), "{json}");
        assert!(json.contains("\"app\":{\"name\":\"kura\""), "{json}");
    }

    #[test]
    fn run_json_ends_done() {
        let mut rig = Rig::new("keys-runjson-done", true);
        let mut run = rig.run("j", Some(rig.dir.join("done.txt")), rig.dir.clone());
        for _ in 0..8 {
            if rig.step(&mut run, 6) != Outcome::Continue {
                break;
            }
        }
        assert!(body(&read(&rig.dir.join("done.txt"))).ends_with("keys: done\n"));
        let json = squeezed(&rig.dir.join("run.json"));
        assert!(json.contains("\"status\":\"done\""), "{json}");
    }

    #[test]
    fn run_json_ends_failed_with_step_why_and_left() {
        let mut rig = Rig::new("keys-runjson-failed", true);
        let mut run = rig.run("<Shot:x>j", Some(rig.dir.join("done.txt")), rig.dir.clone());
        assert_eq!(rig.step(&mut run, 0), Outcome::Continue);
        assert_eq!(rig.step(&mut run, 7), Outcome::Failed);
        let json = squeezed(&rig.dir.join("run.json"));
        assert!(json.contains("\"status\":\"failed\""), "{json}");
        assert!(json.contains("<Shot:x>") && json.contains("\"why\"") && json.contains("\"left\""), "{json}");
    }

    #[test]
    fn run_json_ends_unsettled() {
        let mut rig = Rig::new("keys-runjson-unsettled", false);
        let mut run = rig.run("j", Some(rig.dir.join("done.txt")), rig.dir.clone());
        while rig.step(&mut run, 6) == Outcome::Continue {}
        let json = squeezed(&rig.dir.join("run.json"));
        assert!(json.contains("\"status\":\"unsettled\""), "{json}");
    }

    #[test]
    fn run_json_ends_closed_and_quit() {
        for (how, status) in [(Ending::Closed, "closed"), (Ending::Quit, "quit")] {
            let mut rig = Rig::new("keys-runjson-ended", true);
            let mut run = rig.run("j", Some(rig.dir.join("done.txt")), rig.dir.clone());
            assert_eq!(rig.step(&mut run, 0), Outcome::Continue);
            run.queue.clear();
            run.ended(&mut rig.screen.app, how);
            let json = squeezed(&rig.dir.join("run.json"));
            assert!(json.contains(&format!("\"status\":\"{status}\"")), "{status}: {json}");
        }
    }

    /// run.json replaced by a folder after the start: the last write cannot go through.
    fn block_run_json(dir: &Path) {
        std::fs::remove_file(dir.join("run.json")).unwrap();
        std::fs::create_dir(dir.join("run.json")).unwrap();
    }

    #[test]
    fn a_final_run_json_that_cannot_be_written_fails_a_done_ending() {
        let mut rig = Rig::new("keys-runjson-final", true);
        let done = rig.dir.join("done.txt");
        let mut run = rig.run("j", Some(done.clone()), rig.dir.clone());
        assert_eq!(rig.step(&mut run, 0), Outcome::Continue);
        block_run_json(&rig.dir);
        run.queue.clear();
        assert_eq!(run.ended(&mut rig.screen.app, Ending::Quit), Outcome::Failed);
        let said = read(&done);
        assert!(said.contains("keys: failed\n") && said.contains("step: <run.json>\n"), "{said}");
        assert!(!said.contains("keys: done"), "{said}");
        assert!(rig.flag());
    }

    #[test]
    fn a_failing_ending_keeps_its_tail_when_run_json_cannot_be_written() {
        let mut rig = Rig::new("keys-runjson-final-failing", true);
        let done = rig.dir.join("done.txt");
        let mut run = rig.run("<Shot:x>j", Some(done.clone()), rig.dir.clone());
        assert_eq!(rig.step(&mut run, 0), Outcome::Continue);
        block_run_json(&rig.dir);
        assert_eq!(rig.step(&mut run, 7), Outcome::Failed);
        let said = read(&done);
        assert!(said.contains("step: <Shot:x>\n"), "{said}");
        assert!(!said.contains("step: <run.json>"), "{said}");
    }

    #[test]
    fn a_quit_with_steps_left_keeps_its_tail_when_run_json_cannot_be_written() {
        let mut rig = Rig::new("keys-runjson-quit-left", true);
        let done = rig.dir.join("done.txt");
        let mut run = rig.run("jjj", Some(done.clone()), rig.dir.clone());
        assert_eq!(rig.step(&mut run, 0), Outcome::Continue);
        block_run_json(&rig.dir);
        run.ended(&mut rig.screen.app, Ending::Quit);
        let said = read(&done);
        assert!(said.contains("keys: quit\nleft: "), "{said}");
        assert!(!said.contains("step: <run.json>"), "{said}");
    }

    #[test]
    fn a_run_json_that_cannot_be_started_fails_the_run() {
        let mut rig = Rig::new("keys-runjson-start-fails", true);
        std::fs::create_dir(rig.dir.join("run.json")).unwrap();
        let done = rig.dir.join("done.txt");
        let mut run = rig.run("j", Some(done.clone()), rig.dir.clone());
        assert_eq!(rig.step(&mut run, 0), Outcome::Failed);
        let said = read(&done);
        assert!(said.contains("keys: failed\n") && said.contains("step: <run.json>\n"), "{said}");
        assert!(rig.flag());
    }

    #[test]
    fn no_run_json_without_a_done_file() {
        let mut rig = Rig::new("keys-runjson-none", true);
        let mut run = rig.run("j", None, rig.dir.clone());
        for _ in 0..4 {
            rig.step(&mut run, 6);
        }
        run.ended(&mut rig.screen.app, Ending::Quit);
        assert!(!rig.dir.join("run.json").exists());
    }

    #[test]
    fn a_write_that_fails_ends_the_run_failed() {
        for script in ["<State:a>", "<PaneText:a>", "<PreviewText:a>"] {
            let mut rig = Rig::new("keys-write-fails", true);
            let done = rig.dir.join("done.txt");
            let shot_dir = unwritable(&rig.dir);
            let mut run = rig.run(script, Some(done.clone()), shot_dir.clone());
            // This is about the step's own write; run.json (also in `shot_dir`) is opted out of.
            run.run_closed = true;
            assert_eq!(rig.step(&mut run, 0), Outcome::Failed, "{script}");
            let said = read(&done);
            assert!(said.contains("keys: failed\n"), "{script}: {said}");
            assert!(said.contains(&format!("step: {script}\n")), "{script}: {said}");
            assert!(said.contains(&shot_dir.display().to_string()), "{script} names the path: {said}");
            assert!(body(&said).ends_with("left: 1 not pressed\n"), "{script}: {said}");
            assert!(!said.contains("keys: done"), "{script}: {said}");
            assert!(rig.flag(), "{script}");
            assert!(run.queue.is_empty() && run.done.is_none(), "{script}: no more keys, one report");
        }
    }

    /// Every report kura writes carries exactly one `run:` line; a success still ends with `keys: done`.
    #[test]
    fn reports_carry_one_run_line() {
        keyscript::run_id("KURA").unwrap();
        let id = keyscript::current_run_id().unwrap();
        let mut rig = Rig::new("keys-run-line", true);
        let done = rig.dir.join("done.txt");
        let mut run = rig.run("j", Some(done.clone()), rig.dir.clone());
        while rig.step(&mut run, 0) == Outcome::Continue && run.done.is_some() {}
        let said = read(&done);
        assert_eq!(said.lines().filter(|l| l.starts_with("run: ")).count(), 1, "{said}");
        assert!(said.ends_with(&format!("run: {id}\nkeys: done\n")), "{said}");

        let bad = rig.dir.join("failed.txt");
        let mut run = rig.run("j", Some(bad.clone()), rig.dir.clone());
        run.queue = [(0, Press::Read { tag: "Bogus".into(), name: "n".into() })].into();
        assert_eq!(rig.step(&mut run, 0), Outcome::Failed);
        let said = read(&bad);
        assert_eq!(said.lines().filter(|l| l.starts_with("run: ")).count(), 1, "{said}");
        assert!(said.lines().any(|l| l == format!("run: {id}")), "{said}");
        assert!(said.contains("keys: failed\n") && !said.contains("keys: done"), "{said}");
        assert!(said.lines().filter(|l| l.starts_with("keys: ")).eq(["keys: failed"]), "{said}");
    }

    #[test]
    fn read_has_waits_for_the_text_then_fails_naming_what_it_read() {
        let mut rig = Rig::new("keys-read-has", true);
        let done = rig.dir.join("done.txt");
        let mut run = rig.run("j", Some(done.clone()), rig.dir.clone());
        // No pane in the harness: the reading is "(no pane)".
        run.queue = [(0, Press::ReadHas { tag: "PaneText".into(), text: "(no pane)".into() })].into();
        assert_eq!(rig.step(&mut run, 0), Outcome::Continue);
        assert!(run.queue.is_empty(), "the text was there at once");

        let mut run = rig.run("j", Some(done.clone()), rig.dir.clone());
        run.queue = [(0, Press::ReadHas { tag: "PaneText".into(), text: "never there".into() })].into();
        assert_eq!(rig.step(&mut run, 0), Outcome::Continue, "waits");
        assert_eq!(run.queue.len(), 1);
        assert_eq!(rig.step(&mut run, keyscript::READ_WAIT.as_secs() + 1), Outcome::Failed);
        let said = read(&done);
        assert!(said.contains("keys: failed\n") && said.contains("PaneText") && said.contains("never there"), "{said}");
        assert!(said.contains("(no pane)"), "what it read: {said}");
        assert!(rig.flag());
    }

    /// A `<ReadHas:>` on a screen that never settles looks again on the next frame,
    /// finds its text there, and is not named as a step that went in unsettled.
    #[test]
    fn read_has_on_a_moving_screen_finds_late_text_and_is_not_unsettled() {
        use crate::app::PreviewState;
        use crate::preview::Payload;
        let mut rig = Rig::new("keys-read-has-moving", false);
        let done = rig.dir.join("done.txt");
        let mut run = rig.run("j", Some(done.clone()), rig.dir.clone());
        run.queue = [(0, Press::ReadHas { tag: "PreviewText".into(), text: "late text".into() })].into();
        rig.screen.app.preview.state = PreviewState::Loading;
        assert_eq!(rig.step(&mut run, 6), Outcome::Continue, "starts looking");
        assert_eq!(run.queue.len(), 1, "not there yet");
        rig.screen.app.preview.state = PreviewState::Ready(Payload::Error("some late text".into()));
        assert_eq!(rig.step(&mut run, 0), Outcome::Continue, "looks again without waiting to settle");
        assert!(run.queue.is_empty(), "found on the next frame");
        rig.screen.app.preview.in_flight = false;
        while rig.step(&mut run, 0) == Outcome::Continue && run.done.is_some() {}
        let said = read(&done);
        assert!(said.ends_with("keys: done\n"), "{said}");
        assert!(!said.lines().any(|l| l.starts_with("unsettled") || l.starts_with("keys: unsettled")), "{said}");
        assert!(!rig.flag());

        // After `<Now>`, the second look does not wait out the settle time either.
        rig.screen.app.preview.in_flight = true;
        rig.screen.app.preview.state = PreviewState::Loading;
        let mut run = rig.run("j", Some(done.clone()), rig.dir.clone());
        run.queue = [(0, Press::Now), (1, Press::ReadHas { tag: "PreviewText".into(), text: "later".into() })].into();
        assert_eq!(rig.step(&mut run, 0), Outcome::Continue);
        assert_eq!(run.queue.len(), 1, "looked once under <Now>");
        rig.screen.app.preview.state = PreviewState::Ready(Payload::Error("later".into()));
        assert_eq!(rig.step(&mut run, 0), Outcome::Continue);
        assert!(run.queue.is_empty(), "found on the next frame, not after the settle wait");
    }

    /// What a failed `<ReadHas:>` read is cut to its first 200 characters.
    #[test]
    fn read_has_names_at_most_200_characters_of_what_it_read() {
        use crate::app::PreviewState;
        use crate::preview::Payload;
        let mut rig = Rig::new("keys-read-has-long", true);
        let done = rig.dir.join("done.txt");
        let mut run = rig.run("j", Some(done.clone()), rig.dir.clone());
        run.queue = [(0, Press::ReadHas { tag: "PreviewText".into(), text: "zzz".into() })].into();
        rig.screen.app.preview.state = PreviewState::Ready(Payload::Error("x".repeat(300)));
        assert_eq!(rig.step(&mut run, 0), Outcome::Continue);
        assert_eq!(rig.step(&mut run, keyscript::READ_WAIT.as_secs() + 1), Outcome::Failed);
        let said = read(&done);
        let why = said.lines().find(|l| l.starts_with("why: ")).expect("a why line");
        assert!(why.ends_with(&format!("it read {:?}...", "x".repeat(200))), "{why}");
    }

    #[test]
    fn an_unknown_reading_fails_instead_of_being_skipped() {
        let mut rig = Rig::new("keys-bad-read", true);
        let done = rig.dir.join("done.txt");
        let mut run = rig.run("j", Some(done.clone()), rig.dir.clone());
        run.queue = [(0, Press::Read { tag: "Bogus".into(), name: "n".into() })].into();
        assert_eq!(rig.step(&mut run, 0), Outcome::Failed);
        let said = read(&done);
        assert!(said.contains("keys: failed\n") && said.contains("Bogus"), "{said}");
        assert!(rig.flag());
    }

    /// The guard against a step a newer ito adds (today `<Snap:>`; the whole run is in tests/keys_fail_closed.rs).
    /// Its report as text: a failed report naming the step and its place, with one `run:` line.
    #[test]
    fn a_refused_step_is_reported_as_failed_with_its_place_and_one_run_line() {
        let text = unsupported_report("<Bogus:x>", 3, 5);
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines[0], "keys: failed", "{text}");
        assert!(lines.contains(&"step: <Bogus:x>"), "{text}");
        let why = lines.iter().find(|l| l.starts_with("why: ")).expect("a why line");
        assert!(why.contains("step 3 of 5") && why.contains("does not carry out this step yet"), "{why}");
        assert_eq!(lines.iter().filter(|l| l.starts_with("run: ")).count(), 1, "{text}");
    }

    #[test]
    fn the_final_report_that_cannot_be_written_sets_the_flag() {
        let mut rig = Rig::new("keys-done-unwritable", true);
        let bad = unwritable(&rig.dir);
        let mut run = rig.run("j", Some(bad.clone()), rig.dir.clone());
        assert_eq!(rig.step(&mut run, 0), Outcome::Continue, "j goes in");
        assert!(!rig.flag());
        assert_eq!(rig.step(&mut run, 0), Outcome::Failed, "the end cannot be written");
        assert!(rig.flag());
        assert!(!bad.exists());
    }

    #[test]
    fn a_closed_or_quit_report_that_cannot_be_written_sets_the_flag() {
        for how in [Ending::Closed, Ending::Quit] {
            let mut rig = Rig::new("keys-ended-unwritable", true);
            let mut run = rig.run("j", Some(unwritable(&rig.dir)), rig.dir.clone());
            run.queue.clear();
            let ctx_app = &mut rig.screen.app;
            assert_eq!(run.ended(ctx_app, how), Outcome::Failed, "{how:?}");
            assert!(rig.flag(), "{how:?}");
        }
    }

    #[test]
    fn a_report_write_sets_the_flag_only_when_it_fails() {
        let dir = crate::util::test_dir("keys-write-report");
        let flag = AtomicBool::new(false);
        assert!(write_report(&dir.join("ok.txt"), "x", &flag));
        assert!(!flag.load(Ordering::SeqCst));
        assert!(!write_report(&unwritable(&dir), "x", &flag), "the watchdog's stalled report goes through this");
        assert!(flag.load(Ordering::SeqCst));
    }

    #[test]
    fn a_picture_becomes_a_png_or_says_why_not() {
        let dir = crate::util::test_dir("keys-save-shot");
        let image = egui::ColorImage::filled([8, 8], egui::Color32::RED);
        let path = dir.join("x.png");

        let none = save_shot(&image, true, None, 8.0, &path).unwrap_err();
        assert_eq!(none, "no preview pane is on screen");
        let off = egui::Rect::from_min_max(egui::pos2(100.0, 100.0), egui::pos2(120.0, 120.0));
        assert_eq!(save_shot(&image, true, Some(off), 8.0, &path).unwrap_err(), "the preview pane is off the picture");
        assert!(!path.exists());

        let bad = unwritable(&dir).join("x.png");
        let err = save_shot(&image, false, None, 8.0, &bad).unwrap_err();
        assert!(err.contains(&bad.display().to_string()), "{err}");

        assert_eq!(save_shot(&image, false, None, 8.0, &path).unwrap(), path);
        assert!(path.exists());
        let part = dir.join("part.png");
        let inside = egui::Rect::from_min_max(egui::pos2(1.0, 1.0), egui::pos2(5.0, 6.0));
        save_shot(&image, true, Some(inside), 8.0, &part).unwrap();
        assert_eq!(image::open(&part).unwrap().width(), 4);
    }

    #[test]
    fn a_picture_that_never_arrives_fails_the_run() {
        let mut rig = Rig::new("keys-shot-late", true);
        let done = rig.dir.join("done.txt");
        let mut run = rig.run("<Shot:x>j", Some(done.clone()), rig.dir.clone());
        assert_eq!(rig.step(&mut run, 0), Outcome::Continue, "the shot is asked for");
        assert_eq!(rig.step(&mut run, 1), Outcome::Continue, "still within the deadline");
        assert!(!done.exists());
        assert_eq!(rig.step(&mut run, 6), Outcome::Failed);
        let said = read(&done);
        assert!(said.contains("keys: failed\n") && said.contains("step: <Shot:x>\n"), "{said}");
        assert!(body(&said).ends_with("left: 2 not pressed\n"), "the shot and the key after it: {said}");
        assert!(!rig.dir.join("shots").join("x.png").exists());
        assert!(rig.flag());
    }

    #[test]
    fn closing_or_quitting_before_the_last_picture_is_a_failure() {
        for how in [Ending::Closed, Ending::Quit] {
            let mut rig = Rig::new("keys-shot-closed", true);
            let done = rig.dir.join("done.txt");
            let mut run = rig.run("<Shot:x>", Some(done.clone()), rig.dir.clone());
            assert_eq!(rig.step(&mut run, 0), Outcome::Continue);
            assert!(run.queue.is_empty(), "the queue says nothing is left");
            assert_eq!(run.ended(&mut rig.screen.app, how), Outcome::Failed, "{how:?}");
            let said = read(&done);
            assert!(said.contains("keys: failed\n"), "{how:?}: {said}");
            assert!(!said.contains("keys: done"), "{how:?}: {said}");
            assert!(body(&said).ends_with("left: 1 not pressed\n"), "{how:?}: {said}");
            assert!(rig.flag(), "{how:?}");
        }
    }

    #[test]
    fn keys_that_went_in_unsettled_are_named_and_do_not_succeed() {
        let mut rig = Rig::new("keys-unsettled", false);
        let done = rig.dir.join("done.txt");
        let mut run = rig.run("j<Drag:0.2,0.3-0.6,0.3><Size:700x500><Wait:10>k", Some(done.clone()), rig.dir.clone());
        let mut last = Outcome::Continue;
        for _ in 0..40 {
            last = rig.step(&mut run, 6);
            if last != Outcome::Continue {
                break;
            }
        }
        assert_eq!(last, Outcome::Unsettled);
        let said = read(&done);
        assert!(body(&said).ends_with("keys: unsettled\n"), "{said}");
        assert!(said.contains("unsettled: 4 of 5 steps went in after 5 s without settling: j <Drag:"), "{said}");
        assert_eq!(said.matches("<Drag:").count(), 1, "a split drag is named once: {said}");
        assert!(!said.contains("<Wait:400>"), "the wait Size adds is not a step: {said}");
        assert!(!said.contains(" <Wait:10> "), "a wait is not named: {said}");
        assert!(said.contains("unsettled: the script ended after 5 s without settling\n"), "{said}");
        assert!(!said.contains("keys: done"), "{said}");
        assert!(rig.flag());
    }

    #[test]
    fn a_settled_run_ends_done() {
        let mut rig = Rig::new("keys-settled", true);
        let done = rig.dir.join("done.txt");
        let mut run = rig.run("j<State:a>", Some(done.clone()), rig.dir.clone());
        for _ in 0..3 {
            assert_eq!(rig.step(&mut run, 0), Outcome::Continue);
        }
        let said = read(&done);
        assert!(said.ends_with("keys: done\n"), "{said}");
        assert!(read(&rig.dir.join("a.txt")).contains("cursor icon: Default\nsettled: yes\n"));
        assert!(!rig.flag());
    }

    /// `<State:>` names the terminal args and the env NAMES, and whether a pane is open, before
    /// `cursor icon:`; an env value is never in it.
    #[test]
    fn state_names_the_terminal_args_and_env_names_but_never_a_value() {
        let mut rig = Rig::new("keys-state-term", true);
        let done = rig.dir.join("done.txt");
        rig.screen.app.cfg.term.args = vec!["-NoLogo".to_owned()];
        rig.screen.app.cfg.term.env.insert("KURA_T".to_owned(), "hello".to_owned());
        rig.screen.app.cfg.term.env.insert("KURA_A".to_owned(), "s3cr3t-value".to_owned());
        let mut run = rig.run("<State:s>", Some(done), rig.dir.clone());
        for _ in 0..3 {
            assert_eq!(rig.step(&mut run, 0), Outcome::Continue);
        }
        let said = read(&rig.dir.join("s.txt"));
        assert!(said.contains("term args: [\"-NoLogo\"]\nterm env names: [\"KURA_A\", \"KURA_T\"]\ncursor icon: "), "{said}");
        assert!(said.contains("\npane: closed\n"), "{said}");
        assert!(!said.contains("hello") && !said.contains("s3cr3t-value"), "{said}");

        let mut rig = Rig::new("keys-state-term-empty", true);
        let done = rig.dir.join("done.txt");
        let mut run = rig.run("<State:s>", Some(done), rig.dir.clone());
        for _ in 0..3 {
            assert_eq!(rig.step(&mut run, 0), Outcome::Continue);
        }
        assert!(read(&rig.dir.join("s.txt")).contains("term args: []\nterm env names: []\ncursor icon: "));
    }

    #[test]
    fn closing_a_finished_unsettled_run_stays_not_a_success() {
        let mut rig = Rig::new("keys-unsettled-closed", false);
        let done = rig.dir.join("done.txt");
        let mut run = rig.run("j", Some(done.clone()), rig.dir.clone());
        while rig.step(&mut run, 6) == Outcome::Continue {}
        assert!(rig.flag());
        // The window closes later: the report is not rewritten, the flag stays.
        assert_eq!(run.ended(&mut rig.screen.app, Ending::Closed), Outcome::Continue);
        assert!(body(&read(&done)).ends_with("keys: unsettled\n"));
        assert!(rig.flag());
    }

    #[test]
    fn closing_with_steps_left_still_names_what_went_in_unsettled() {
        let mut rig = Rig::new("keys-unsettled-left", false);
        let done = rig.dir.join("done.txt");
        let mut run = rig.run("jk", Some(done.clone()), rig.dir.clone());
        while !rig.flag() {
            assert_eq!(rig.step(&mut run, 6), Outcome::Continue);
        }
        assert!(!run.queue.is_empty(), "k is still to go");
        assert_eq!(run.ended(&mut rig.screen.app, Ending::Closed), Outcome::Unsettled);
        let said = read(&done);
        assert!(said.contains("unsettled: 1 of 2 steps went in after 5 s without settling: j\n"), "{said}");
        assert!(body(&said).ends_with("keys: closed\nleft: 1 not pressed\n"), "{said}");
        assert_eq!(said.matches("keys: ").count(), 1, "one ending: {said}");
        assert!(rig.flag());
    }

    #[test]
    fn a_closed_window_with_nothing_unsettled_ends_done() {
        let mut rig = Rig::new("keys-closed-ok", true);
        let done = rig.dir.join("done.txt");
        let mut run = rig.run("j", Some(done.clone()), rig.dir.clone());
        assert_eq!(rig.step(&mut run, 0), Outcome::Continue);
        run.queue.clear();
        assert_eq!(run.ended(&mut rig.screen.app, Ending::Quit), Outcome::Continue);
        let said = read(&done);
        assert!(body(&said).ends_with("quit: yes\nkeys: done\n"), "{said}");
        assert!(!rig.flag());
    }

    use crate::app::Writer;

    /// Past the 10 s a write may wait, counted from a send that happened a
    /// moment after the rig's clock started.
    const PAST_CONFIG_WAIT: u64 = 11;

    #[test]
    fn a_held_answer_holds_the_script_and_fails_it_at_ten_seconds() {
        let mut rig = Rig::new("keys-held-answer", true);
        rig.common_folder();
        let done = rig.dir.join("done.txt");
        let mut run = rig.run("<Theme:dark><Now><State:mid><Quit>", Some(done.clone()), rig.dir.clone());
        assert_eq!(rig.step(&mut run, 0), Outcome::Continue, "the theme is written");
        assert!(rig.screen.app.config_writes_pending());
        // The answer is held: nothing is drained.
        assert_eq!(rig.step(&mut run, 0), Outcome::Continue);
        assert_eq!(rig.step(&mut run, 5), Outcome::Continue);
        assert!(!rig.dir.join("mid.txt").exists(), "State is not written");
        assert!(!rig.screen.app.quit, "<Quit> has not run");
        assert!(!rig.flag());
        assert_eq!(rig.step(&mut run, PAST_CONFIG_WAIT - 5), Outcome::Failed);
        let said = read(&done);
        assert!(said.contains("keys: failed\n") && said.contains("step: <config write>\n"), "{said}");
        assert!(said.contains("no answer from the config writer within 10 s of write #1 (common.toml)"), "{said}");
        assert!(!said.contains("keys: done"), "{said}");
        assert!(!rig.dir.join("mid.txt").exists() && !rig.screen.app.quit);
        assert!(rig.flag());
    }

    #[test]
    fn the_next_step_does_not_run_while_a_write_is_unanswered() {
        let mut rig = Rig::new("keys-unanswered", true);
        rig.common_folder();
        let done = rig.dir.join("done.txt");
        let mut run = rig.run("<Theme:dark><State:after>", Some(done.clone()), rig.dir.clone());
        assert_eq!(rig.step(&mut run, 0), Outcome::Continue);
        // Settled and long past the settle wait, and still it does not go on.
        for _ in 0..5 {
            assert_eq!(rig.step(&mut run, 1), Outcome::Continue);
            assert!(!rig.dir.join("after.txt").exists());
        }
        assert_eq!(rig.step(&mut run, PAST_CONFIG_WAIT - 5), Outcome::Failed);
        assert!(!rig.dir.join("after.txt").exists());
        assert!(read(&done).contains("step: <config write>\n"));
    }

    #[test]
    fn an_answer_in_time_lets_the_script_go_on_to_done() {
        let mut rig = Rig::new("keys-answered", true);
        rig.common_folder();
        let done = rig.dir.join("done.txt");
        let mut run = rig.run("<Theme:dark><State:after>", Some(done.clone()), rig.dir.clone());
        assert_eq!(rig.step(&mut run, 0), Outcome::Continue);
        assert_eq!(rig.step(&mut run, 5), Outcome::Continue);
        rig.answer();
        for _ in 0..3 {
            assert_eq!(rig.step(&mut run, 0), Outcome::Continue);
        }
        let state = read(&rig.dir.join("after.txt"));
        assert!(state.contains("config writes pending: no\nconfig write failed: none\n"), "{state}");
        let said = read(&done);
        assert!(said.ends_with("keys: done\n"), "{said}");
        assert!(!rig.flag());
    }

    #[test]
    fn now_is_for_the_next_step_only() {
        let mut rig = Rig::new("keys-now-one-step", false);
        let done = rig.dir.join("done.txt");
        let mut run = rig.run("<A-c><Now><State:mid><State:after>", Some(done), rig.dir.clone());
        // The screen is not settled: the first key goes in after the settle wait.
        assert_eq!(rig.step(&mut run, 0), Outcome::Continue);
        assert_eq!(rig.step(&mut run, 6), Outcome::Continue);
        // `mid` is read at once; `after` is back to waiting for the screen.
        assert_eq!(rig.step(&mut run, 0), Outcome::Continue);
        assert!(rig.dir.join("mid.txt").exists(), "<Now> let the next step go without waiting");
        assert!(!run.now, "and was spent on it");
        assert_eq!(rig.step(&mut run, 0), Outcome::Continue);
        assert!(!rig.dir.join("after.txt").exists(), "an unsettled screen holds the step after");
        assert_eq!(rig.step(&mut run, 6), Outcome::Continue);
        assert!(rig.dir.join("after.txt").exists(), "until the settle wait is over");
    }

    #[test]
    fn closing_or_quitting_with_a_write_pending_fails_the_run() {
        for (how, why) in [
            (Ending::Closed, "the window was closed before the config writer answered (keymap.toml)"),
            (Ending::Quit, "kura quit before the config writer answered (keymap.toml)"),
        ] {
            let mut rig = Rig::new("keys-ended-pending", true);
            let done = rig.dir.join("done.txt");
            let mut run = rig.run("j", Some(done.clone()), rig.dir.clone());
            rig.screen.app.config_writes.sent(Writer::Kura, "keymap.toml", rig.t0);
            assert_eq!(run.ended(&mut rig.screen.app, how), Outcome::Failed, "{how:?}");
            let said = read(&done);
            assert!(said.contains("keys: failed\n") && said.contains("step: <config write>\n"), "{said}");
            assert!(said.contains(why), "{said}");
            assert!(!said.contains("keys: done") && !said.contains("keys: closed") && !said.contains("keys: quit"), "{said}");
            assert!(rig.flag());
        }
    }

    #[test]
    fn a_failed_config_write_ends_the_run_with_its_message() {
        let mut rig = Rig::new("keys-write-failed-ended", true);
        let done = rig.dir.join("done.txt");
        let mut run = rig.run("j", Some(done.clone()), rig.dir.clone());
        rig.screen.app.fail_config_write("common.toml: no space".into());
        assert_eq!(run.ended(&mut rig.screen.app, Ending::Closed), Outcome::Failed);
        let said = read(&done);
        assert!(said.contains("step: <config write>\n") && said.contains("common.toml: no space"), "{said}");
        assert!(rig.flag());

        let mut rig = Rig::new("keys-write-failed-hook", true);
        let done = rig.dir.join("done.txt");
        let mut run = rig.run("j", Some(done.clone()), rig.dir.clone());
        rig.screen.app.fail_config_write("common.toml: no space".into());
        rig.screen.app.fail_config_write("a second one".into());
        assert_eq!(rig.step(&mut run, 0), Outcome::Failed);
        let said = read(&done);
        assert!(said.contains("step: <config write>\n") && said.contains("common.toml: no space"), "the first wins: {said}");
        assert_eq!(rig.screen.app.config_write_failed.as_deref(), Some("common.toml: no space"));
    }

    #[test]
    fn a_write_is_judged_by_its_own_age_not_the_one_before_it() {
        let mut rig = Rig::new("keys-two-writes", true);
        let done = rig.dir.join("done.txt");
        let mut run = rig.run("<State:s>", Some(done), rig.dir.clone());
        let start = rig.t0;
        // A goes out at 0 s and is answered at 6 s; B goes out at 5 s and is answered at 11 s.
        let a = rig.screen.app.config_writes.sent(Writer::Kura, "config.toml", start);
        let b = rig.screen.app.config_writes.sent(Writer::Common, "common.toml", start + Duration::from_secs(5));
        assert_eq!(b, 1);
        for sec in 1..=12u64 {
            if sec == 7 {
                rig.screen.app.config_writes.answered(Writer::Kura, a).unwrap();
            }
            if sec == 12 {
                rig.screen.app.config_writes.answered(Writer::Common, b).unwrap();
            }
            assert_eq!(rig.step(&mut run, 1), Outcome::Continue, "no failure at {sec} s");
        }
        assert!(!rig.flag());
        assert!(rig.dir.join("s.txt").exists(), "and the script went on once both were answered");
    }

    #[test]
    fn a_write_nobody_answers_fails_at_ten_seconds_naming_it() {
        let mut rig = Rig::new("keys-never-answered", true);
        let done = rig.dir.join("done.txt");
        let mut run = rig.run("<State:s>", Some(done.clone()), rig.dir.clone());
        let a = rig.screen.app.config_writes.sent(Writer::Kura, "config.toml", rig.t0);
        assert_eq!(rig.step(&mut run, 9), Outcome::Continue);
        assert_eq!(rig.step(&mut run, 1), Outcome::Failed);
        let said = read(&done);
        assert!(said.contains(&format!("no answer from the config writer within 10 s of write #{a} (config.toml)")), "{said}");
    }

    #[test]
    fn the_waits_fit_inside_the_watchdogs() {
        // ito's TEXT_WAIT and READ_WAIT are checked against STALL in ito.
        let longest = DELIVER_WAIT.max(DEFAULT_WAIT).max(CONFIG_WAIT);
        assert!(DEFAULT_WAIT + longest < keyscript::STALL, "{DEFAULT_WAIT:?} + {longest:?} < {:?}", keyscript::STALL);
    }

    #[test]
    fn the_watchdog_clock_moves_with_the_script_and_not_with_the_waiting() {
        let mut rig = Rig::new("keys-watchdog", true);
        let start = rig.t0;
        let mut run = rig.run("j<State:s>", Some(rig.dir.join("done.txt")), rig.dir.clone());
        run.watch = Some(Arc::new(Mutex::new(crate::ScriptWatch { left: 2, at: start, due: Duration::ZERO, finished: false })));
        let at = |run: &ScriptRun| run.watch.as_ref().unwrap().lock().unwrap().at;
        let secs = |t: Instant| t.saturating_duration_since(start).as_secs();
        // An input goes in at 0 s.
        assert_eq!(rig.step(&mut run, 0), Outcome::Continue);
        assert_eq!(secs(at(&run)), 0);
        // It is drawn at 9 s; a write goes out then.
        rig.screen.app.config_writes.sent(Writer::Kura, "config.toml", start + Duration::from_secs(9));
        assert_eq!(rig.step(&mut run, 9), Outcome::Continue);
        assert_eq!(secs(at(&run)), 9, "delivery is progress");
        // Frames drawn while the write is out move nothing.
        assert_eq!(rig.step(&mut run, 3), Outcome::Continue);
        assert_eq!(rig.step(&mut run, 2), Outcome::Continue);
        assert_eq!(secs(at(&run)), 9, "waiting for the answer is not progress");
        // Answered at 18 s.
        rig.screen.app.config_writes.answered(Writer::Kura, 1).unwrap();
        assert_eq!(rig.step(&mut run, 4), Outcome::Continue);
        assert_eq!(secs(at(&run)), 18, "an answer is progress");
        let w = run.watch.as_ref().unwrap().lock().unwrap();
        assert!(!crate::stalled(&w, start + Duration::from_secs(32)));
        assert!(!crate::stalled(&w, start + Duration::from_secs(18) + keyscript::STALL));
        assert!(crate::stalled(&w, start + Duration::from_secs(18 + 31)));
    }

    #[test]
    fn a_wait_keeps_its_grace_before_the_watchdog_calls_a_stall() {
        let mut rig = Rig::new("keys-watchdog-wait", true);
        let start = rig.t0;
        let mut run = rig.run("j<Wait:20000>k", Some(rig.dir.join("done.txt")), rig.dir.clone());
        run.watch = Some(Arc::new(Mutex::new(crate::ScriptWatch { left: 3, at: start, due: Duration::ZERO, finished: false })));
        assert_eq!(rig.step(&mut run, 0), Outcome::Continue);
        let w = run.watch.as_ref().unwrap().lock().unwrap();
        assert_eq!(w.due, Duration::from_secs(20));
        assert!(!crate::stalled(&w, start + Duration::from_secs(20) + keyscript::STALL));
        assert!(crate::stalled(&w, start + Duration::from_secs(20 + 31)));
    }

    #[test]
    fn an_input_that_is_never_drawn_fails_after_ten_seconds() {
        let mut rig = Rig::new("keys-not-drawn", true);
        let done = rig.dir.join("done.txt");
        let mut run = rig.run("j<State:s>", Some(done.clone()), rig.dir.clone());
        let root = egui::ViewportId::ROOT;
        assert_eq!(rig.tick(&mut run, 0, 2, root), Outcome::Continue, "the key goes in");
        // The window is hidden: the frame counter does not move.
        assert_eq!(rig.tick(&mut run, 5, 0, root), Outcome::Continue);
        assert_eq!(rig.tick(&mut run, 4, 0, root), Outcome::Continue);
        assert_eq!(rig.tick(&mut run, 1, 0, root), Outcome::Failed);
        let said = read(&done);
        assert!(said.contains("the input was not drawn within 10 s (the window may be hidden, minimized or covered)"), "{said}");
        assert!(!rig.dir.join("s.txt").exists());
    }

    #[test]
    fn an_input_in_flight_that_meets_another_window_fails_but_an_idle_hook_does_nothing() {
        let other = egui::ViewportId::from_hash_of("another window");
        let mut rig = Rig::new("keys-other-viewport", true);
        let done = rig.dir.join("done.txt");
        let mut run = rig.run("j<State:s>", Some(done.clone()), rig.dir.clone());
        assert_eq!(rig.tick(&mut run, 0, 2, egui::ViewportId::ROOT), Outcome::Continue);
        assert_eq!(rig.tick(&mut run, 0, 0, other), Outcome::Failed);
        let said = read(&done);
        assert!(said.contains("the input went to viewport "), "{said}");

        // With nothing in flight another window's hook injects nothing and changes nothing.
        let mut rig = Rig::new("keys-other-idle", true);
        let mut run = rig.run("j", Some(rig.dir.join("done.txt")), rig.dir.clone());
        let before = run.queue.len();
        assert_eq!(rig.tick(&mut run, 0, 4, other), Outcome::Continue);
        assert!(run.queue.len() == before && run.inflight.is_none());
    }

    /// Passes until the run ends or `n` passes have gone by.
    fn until_end(rig: &mut Rig, run: &mut ScriptRun, secs: u64, n: usize) -> Outcome {
        let mut out = Outcome::Continue;
        for _ in 0..n {
            out = rig.pass(run, secs);
            if out != Outcome::Continue {
                break;
            }
        }
        out
    }

    /// `<C-,>` goes in on a hook whose input is never drawn: the tree is still the one from before it,
    /// in which `Open config.toml` is not there, so `<Assert:!Open config.toml>` would pass wrongly on
    /// it. The check must wait for a tree drawn after the input, and fail at 3 s saying there was none.
    #[test]
    fn a_reading_step_waits_for_a_tree_drawn_after_the_last_input() {
        let mut rig = Rig::new("keys-fresh", true);
        let done = rig.dir.join("done.txt");
        let mut run = rig.run("j<C-,><Assert:!Open config.toml>", Some(done.clone()), rig.dir.clone());
        for _ in 0..20 {
            rig.pass(&mut run, 0);
            if run.queue.len() == 2 {
                break;
            }
        }
        assert_eq!(run.queue.len(), 2, "j went in and was drawn (so a tree exists)");
        assert_eq!(rig.hook_by(&mut run, Duration::ZERO), Outcome::Continue);
        assert_eq!(run.queue.len(), 1, "<C-,> went in, and is never drawn");
        assert_eq!(rig.hook_by(&mut run, Duration::ZERO), Outcome::Continue);
        assert!(run.check_since.is_some() && run.due().is_some(), "the check waits for a newer tree");
        assert_eq!(rig.hook_by(&mut run, Duration::from_secs(1)), Outcome::Continue);
        assert_eq!(rig.hook_by(&mut run, Duration::from_secs(4)), Outcome::Failed);
        let said = read(&done);
        assert!(said.contains("the window's tree was not updated after the last action"), "{said}");
        assert!(run.due().is_none());

        // With the key drawn, the check reads the settings screen and fails on what it finds there.
        let mut rig = Rig::new("keys-fresh-drawn", true);
        let done = rig.dir.join("done.txt");
        let mut run = rig.run("<C-,><Assert:!Open config.toml>", Some(done.clone()), rig.dir.clone());
        assert_eq!(until_end(&mut rig, &mut run, 1, 12), Outcome::Failed);
        let said = read(&done);
        assert!(said.contains("Open config.toml") && !said.contains("not updated"), "{said}");
    }

    /// After `<Theme:>`'s write is answered, a reading needs a tree drawn after the answer, though an
    /// older tree is there (and `!zzz` would pass on it).
    #[test]
    fn config_answer_requires_new_tree() {
        let mut rig = Rig::new("keys-answer-tree", true);
        rig.common_folder();
        let done = rig.dir.join("done.txt");
        let mut run = rig.run("<Theme:dark><Assert:!zzz-not-here>", Some(done), rig.dir.clone());
        assert_eq!(rig.hook_by(&mut run, Duration::ZERO), Outcome::Continue, "the theme is written");
        assert!(rig.screen.app.config_writes_pending());
        // A tree exists (from before the answer).
        rig.screen.feed(Vec::new());
        rig.screen.feed(Vec::new());
        rig.answer();
        assert_eq!(rig.hook_by(&mut run, Duration::ZERO), Outcome::Continue);
        assert!(matches!(run.queue.front(), Some((_, Press::Assert { .. }))), "the check has not passed on the old tree");
        assert!(run.check_since.is_some(), "it is waiting");
        for _ in 0..30 {
            rig.pass(&mut run, 0);
            if run.queue.is_empty() {
                break;
            }
        }
        assert!(run.queue.is_empty(), "the check passed on a tree drawn after the answer");
        assert!(run.check_since.is_none());
    }

    /// A missing text is looked for until `TEXT_WAIT` is up: still waiting 1 ms short of it, failed at it.
    #[test]
    fn checks_retry_then_fail_exactly_at_text_wait() {
        let mut rig = Rig::new("keys-text-wait", true);
        let done = rig.dir.join("done.txt");
        let mut run = rig.run("<C-,><Assert:Not on this screen>", Some(done.clone()), rig.dir.clone());
        for _ in 0..20 {
            rig.pass(&mut run, 0);
            if run.check_since.is_some() {
                break;
            }
        }
        assert_eq!(run.check_since, Some(rig.t0), "the check looked once and did not find it");
        let wait = keyscript::TEXT_WAIT;
        assert_eq!(rig.hook_by(&mut run, wait - Duration::from_millis(1)), Outcome::Continue, "1 ms short of the wait");
        assert!(!done.exists());
        assert_eq!(rig.hook_by(&mut run, Duration::from_millis(1)), Outcome::Failed, "at the wait");
        let said = read(&done);
        assert!(said.contains("keys: failed") && said.contains("Not on this screen"), "{said}");
    }

    /// A `<Tree:t>` that cannot write its file fails at once, naming the path.
    #[test]
    fn tree_write_failure_names_the_path() {
        let mut rig = Rig::new("keys-tree-unwritable", true);
        let done = rig.dir.join("done.txt");
        std::fs::create_dir(rig.dir.join("t.txt")).unwrap();
        let mut run = rig.run("<C-,><Tree:t>", Some(done.clone()), rig.dir.clone());
        assert_eq!(until_end(&mut rig, &mut run, 1, 12), Outcome::Failed);
        let said = read(&done);
        assert!(said.contains("keys: failed\n") && said.contains("cannot write ") && said.contains("t.txt"), "{said}");
        assert!(rig.flag());
    }

    /// `<Now>` is for the step after it: a check of that step that has to look again has not used it up.
    #[test]
    fn now_survives_check_retry() {
        let mut rig = Rig::new("keys-now-retry", true);
        let done = rig.dir.join("done.txt");
        let mut run = rig.run("<Now><Assert:Not on this screen>", Some(done.clone()), rig.dir.clone());
        assert_eq!(rig.pass(&mut run, 0), Outcome::Continue);
        assert!(run.check_since.is_some(), "the check looked and did not find it");
        assert!(run.now, "the retry has not spent <Now>");
        assert_eq!(rig.pass(&mut run, 1), Outcome::Continue);
        assert!(run.now, "still not");
        assert_eq!(rig.pass(&mut run, 4), Outcome::Failed);
    }

    /// A drag is several steps of one action: only its first frame marks the action.
    #[test]
    fn drag_marks_only_first_frame() {
        let mut rig = Rig::new("keys-drag-act", true);
        let mut run = rig.run("<Drag:0.2,0.3-0.6,0.3>", Some(rig.dir.join("done.txt")), rig.dir.clone());
        assert_eq!(rig.hook_by(&mut run, Duration::ZERO), Outcome::Continue);
        let acted = run.acted.expect("the first frame acted");
        let idx = run.acted_idx;
        assert!(run.since.is_some());
        assert!(run.queue.len() > 1, "the rest of the drag is still to go in");
        let mut frames = 0;
        while !run.queue.is_empty() && frames < 20 {
            rig.hook_by(&mut run, Duration::from_secs(1));
            frames += 1;
            assert_eq!(run.acted, Some(acted), "a tail frame of the drag did not act again");
            assert_eq!(run.acted_idx, idx);
        }
        assert!(run.queue.is_empty() && frames > 1, "the drag's tail frames went in");
    }

    /// A `<Tree:>` that looked more than once clears the retry: the check after it gets its own 3 s.
    #[test]
    fn tree_success_clears_retry_state() {
        let mut rig = Rig::new("keys-tree-clears", true);
        let done = rig.dir.join("done.txt");
        let mut run = rig.run("<C-,><Tree:t><Assert:Not on this screen>", Some(done), rig.dir.clone());
        let (out, held) = rig.hook_events(&mut run, Duration::ZERO);
        assert_eq!(out, Outcome::Continue, "the key goes in, undrawn for now");
        assert_eq!(rig.hook_by(&mut run, Duration::ZERO), Outcome::Continue);
        let first = run.check_since.expect("the tree waits for a newer one");
        assert!(matches!(run.queue.front(), Some((_, Press::Tree(_)))));
        assert_eq!(rig.hook_by(&mut run, Duration::from_secs(2)), Outcome::Continue);
        rig.screen.feed(held);
        for _ in 0..20 {
            rig.pass(&mut run, 0);
            if rig.dir.join("t.txt").exists() {
                break;
            }
        }
        assert!(rig.dir.join("t.txt").exists(), "the tree was written once a tree was drawn");
        for _ in 0..20 {
            rig.pass(&mut run, 0);
            if run.check_since.is_some() {
                break;
            }
        }
        let second = run.check_since.expect("the Assert looked and did not find it");
        assert!(second > first, "the Assert has its own clock, not the Tree's");
    }

    /// A check that looks again has made no progress: the stall watchdog's clock stays where it was.
    #[test]
    fn retry_before_wait_preserves_watchdog() {
        let mut rig = Rig::new("keys-retry-progress", true);
        let mut run = rig.run("<C-,><Assert:Not on this screen>", Some(rig.dir.join("done.txt")), rig.dir.clone());
        for _ in 0..20 {
            rig.pass(&mut run, 0);
            if run.check_since.is_some() {
                break;
            }
        }
        assert!(run.check_since.is_some());
        let at = run.progress_at;
        assert_eq!(rig.pass(&mut run, 1), Outcome::Continue);
        assert_eq!(rig.pass(&mut run, 1), Outcome::Continue);
        assert_eq!(run.progress_at, at, "a retry is not progress");
    }

    /// ito's copy record is process-wide: tests that note copies take turns, each starting from an empty record.
    /// The fake comes first so it is let go before the lock (a tuple drops in order).
    fn noting_fake() -> (ito_clipboard::Entered, std::sync::MutexGuard<'static, ()>) {
        static ONE_AT_A_TIME: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let guard = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
        ito_clipboard::record::reset();
        (ito_clipboard::Fake::new().noting().enter(), guard)
    }

    /// `<Clipboard:>` and `<ClipboardHas:>` check what kura copied after the last action.
    #[test]
    fn clipboard_checks_read_what_kura_copied() {
        let _noting = noting_fake();
        let name = "0-kura-clip-ok.txt";
        let mut rig = Rig::with_files("keys-clip-ok", true, &[name]);
        let done = rig.dir.join("done.txt");
        let mut run = rig.run(&format!("cf<Clipboard:{name}><ClipboardHas:kura-clip>"), Some(done.clone()), rig.dir.clone());
        assert_eq!(until_end(&mut rig, &mut run, 1, 40), Outcome::Continue);
        assert!(run.queue.is_empty() && run.finished, "every step went in");
        assert!(read(&done).ends_with("keys: done\n"), "{}", read(&done));
        assert!(!rig.flag());

        // Not that text: ito's reason, after the wait.
        let mut rig = Rig::with_files("keys-clip-not", true, &[name]);
        let done = rig.dir.join("done.txt");
        let mut run = rig.run("cf<Clipboard:zzz-not-it>", Some(done.clone()), rig.dir.clone());
        assert_eq!(until_end(&mut rig, &mut run, 1, 40), Outcome::Failed);
        let said = read(&done);
        assert!(said.contains("keys: failed\n") && said.contains("not `zzz-not-it`"), "{said}");
    }

    /// A copy from before the last action does not count: ito's own words for it, naming that copy.
    #[test]
    fn clipboard_check_without_a_copy_after_the_action_fails() {
        let _noting = noting_fake();
        let name = "0-kura-clip-none.txt";
        let mut rig = Rig::with_files("keys-clip-none", true, &[name]);
        let done = rig.dir.join("done.txt");
        let mut run = rig.run(&format!("cf<Clipboard:{name}>j<Clipboard:{name}>"), Some(done.clone()), rig.dir.clone());
        assert_eq!(until_end(&mut rig, &mut run, 1, 60), Outcome::Failed);
        let said = read(&done);
        assert!(said.contains("nothing copied after the last action") && said.contains(name), "{said}");
    }

    #[test]
    fn assert_finds_text_in_the_window_and_fails_naming_what_is_there() {
        let mut rig = Rig::new("keys-assert", true);
        let done = rig.dir.join("done.txt");
        let mut run = rig.run(
            "<C-,><Assert:Open config.toml><Assert:!Not on this screen><Assert:Not on this screen>",
            Some(done.clone()),
            rig.dir.clone(),
        );
        assert_eq!(until_end(&mut rig, &mut run, 0, 8), Outcome::Continue, "the first two checks pass and the third waits");
        assert!(run.due().is_some());
        assert_eq!(rig.pass(&mut run, 4), Outcome::Failed);
        let said = read(&done);
        assert!(said.contains("Not on this screen") && said.contains("keys: failed"), "{said}");
        assert!(!said.contains("unsettled"), "a pure check is never unsettled: {said}");
    }

    /// The settings screen's side-nav rows are painted without a node in the tree, so they cannot be
    /// clicked by text; the General page's language box is a node, and opening it writes nothing.
    /// No click here is on a button that writes or removes anything (Forget, Reset, Import).
    #[test]
    fn click_text_clicks_the_label_and_waits_for_a_missing_one() {
        let mut rig = Rig::new("keys-click-text", true);
        let done = rig.dir.join("done.txt");
        let mut run = rig.run("<C-,><Assert:!日本語><Click:Auto (system)><Assert:日本語><State:s>", Some(done), rig.dir.clone());
        assert_eq!(until_end(&mut rig, &mut run, 1, 40), Outcome::Continue);
        let said = read(&rig.dir.join("s.txt"));
        assert!(said.contains("settings page: general"), "{said}");
        assert!(said.contains("since last action: "), "{said}");
        assert!(run.since.is_some() && run.acted.is_some(), "the click is an action");

        let mut rig = Rig::new("keys-click-text-missing", true);
        let done = rig.dir.join("done.txt");
        let mut run = rig.run("<C-,><Click:No such label>", Some(done.clone()), rig.dir.clone());
        assert_eq!(until_end(&mut rig, &mut run, 1, 12), Outcome::Failed);
        assert!(read(&done).contains("No such label"));
        assert_eq!(run.due(), None, "a failed run asks for no more frames");
    }

    /// Test IDs on the config.toml page (ito-prefs' Shell card): a click on `settings.shell.env.edit`
    /// opens the variables editor, which shows `settings.shell.env.add`. Nothing is typed or saved.
    #[test]
    fn id_clicks_reach_widgets() {
        let mut rig = Rig::new("keys-click-id", true);
        let done = rig.dir.join("done.txt");
        let mut run = rig.run(
            "<C-,><C-Tab><C-Tab><State:s><Wheel:-30@0.6,0.5><Id:settings.shell.program><ClickId:settings.shell.env.edit><Id:settings.shell.env.add>",
            Some(done),
            rig.dir.clone(),
        );
        assert_eq!(until_end(&mut rig, &mut run, 1, 60), Outcome::Continue);
        assert!(run.queue.is_empty() && run.finished, "every step passed");
        assert!(read(&rig.dir.join("s.txt")).contains("settings page: kura"));

        // Without the click the editor is not open.
        let mut rig = Rig::new("keys-click-id-not", true);
        let done = rig.dir.join("done.txt");
        let mut run = rig.run("<C-,><C-Tab><C-Tab><Id:settings.shell.env.add>", Some(done.clone()), rig.dir.clone());
        assert_eq!(until_end(&mut rig, &mut run, 1, 40), Outcome::Failed);
        assert!(read(&done).contains("settings.shell.env.add"));
    }

    #[test]
    fn right_and_double_clicks_are_one_frame_each() {
        let at = egui::pos2(3.0, 4.0);
        let n = |ev: Vec<egui::Event>| ev.iter().filter(|e| matches!(e, egui::Event::PointerButton { pressed: true, .. })).count();
        let one = click_events(at, false, false);
        let two = click_events(at, false, true);
        assert_eq!((n(one.clone()), n(two.clone())), (1, 2));
        assert!(matches!(one[0], egui::Event::PointerMoved(p) if p == at));
        let right = click_events(at, true, false);
        assert!(right.iter().any(|e| matches!(e, egui::Event::PointerButton { button: egui::PointerButton::Secondary, .. })));
    }

    #[test]
    fn an_id_check_without_a_test_id_fails_naming_it() {
        let mut rig = Rig::new("keys-id", true);
        let done = rig.dir.join("done.txt");
        let mut run = rig.run("<C-,><ClickId:nothing><Id:nothing>", Some(done.clone()), rig.dir.clone());
        assert_eq!(until_end(&mut rig, &mut run, 1, 12), Outcome::Failed);
        assert!(read(&done).contains("no widget has the test ID `nothing`"));
    }

    /// The first step has no action before it, so this looks at everything the process has announced: the needle is one
    /// no other test says.
    #[test]
    fn announced_fails_when_nothing_said_the_text() {
        let mut rig = Rig::new("keys-announced", true);
        let done = rig.dir.join("done.txt");
        let mut run = rig.run("<Announced:never-said-by-kura-7f3a>", Some(done.clone()), rig.dir.clone());
        assert_eq!(rig.step(&mut run, 0), Outcome::Continue);
        assert_eq!(rig.step(&mut run, 4), Outcome::Failed);
        assert!(read(&done).contains("keys: failed"));
    }

    /// `y` raises the toast `Yanked 1 item`, which kura reads aloud when it draws it. `<Announced:>` only looks after
    /// this run's last action, and the yank is this run's own. Another test drawing a one-item yank at the same moment
    /// would be heard too, which could only let this pass, never make it fail.
    #[test]
    fn announced_passes_for_the_yank_toast() {
        let mut rig = Rig::new("keys-announced-yank", true);
        let done = rig.dir.join("done.txt");
        let mut run = rig.run("y<Announced:Yanked 1 item>", Some(done.clone()), rig.dir.clone());
        assert_eq!(until_end(&mut rig, &mut run, 1, 12), Outcome::Continue);
        assert!(read(&done).trim_end().ends_with("keys: done"), "{}", read(&done));
    }

    #[test]
    fn tree_writes_the_window_tree_beside_the_report() {
        let mut rig = Rig::new("keys-tree", true);
        let done = rig.dir.join("done.txt");
        let mut run = rig.run("<C-,><Tree:t>", Some(done), rig.dir.clone());
        until_end(&mut rig, &mut run, 0, 10);
        let tree = read(&rig.dir.join("t.txt"));
        assert!(tree.contains("Open config.toml"), "{tree}");
    }

    #[test]
    fn a_waiting_check_asks_for_the_next_frame_and_a_finished_one_stops_asking() {
        let mut rig = Rig::new("keys-due", true);
        let mut run = rig.run("<C-,><Assert:Open config.toml>", Some(rig.dir.join("done.txt")), rig.dir.clone());
        assert!(run.due().is_none());
        until_end(&mut rig, &mut run, 0, 10);
        assert!(run.due().is_none(), "a passed check clears the retry");
    }
}
