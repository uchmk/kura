//! The `--keys` player: what `main.rs`'s frame loop calls to press the next
//! scripted step, and what decides how a run ends.
//!
//! A run must never end looking like a success when a step was skipped, a
//! file or picture is missing or a wait was not met. It ends one of five ways
//! in `KURA_KEYS_DONE`: `keys: done`, `keys: unsettled` (a key went in after
//! the settle wait with the screen still moving), `keys: failed`, or the
//! older `keys: closed` / `keys: quit` with keys left. `keys: unsettled`,
//! `keys: failed` and a close or quit with unsettled steps set the flag the
//! process turns into `keyscript::FAILED_EXIT`.
//!
//! The player takes the time and the frame number as arguments and sends no
//! window command of its own, so a test can put a step "6 s ago" instead of
//! sleeping, and read the [`Outcome`].

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::app::App;
use crate::keyscript::{self, evidence, pace, review, Press};

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

/// A `<Shot:name>` asked for and not yet on disk (Pace holds its deadline).
struct Shot {
    name: String,
    /// The screenshot command has been sent.
    sent: bool,
    /// The theme and language in force when the screenshot command was sent (empty until then).
    theme: String,
    lang: String,
}

/// A change of `common.toml` a step asks for, after the step has been marked: the key and its TOML value.
type CommonEdit = (&'static str, String);

/// The `--keys` host: ito's [`pace::Pace`] decides what comes next and when; this carries each step out in the
/// window and writes what the run leaves behind.
pub(crate) struct ScriptRun {
    pace: pace::Pace,
    /// Every step as it was written, by index (for the watchdog's report).
    labels: Vec<String>,
    /// The shape the window asked of the mouse cursor in the last frame, for `<State:>`.
    pub(crate) cursor_icon: egui::CursorIcon,
    shot: Option<Shot>,
    /// Where `<Shot:name>` and the readings save.
    shot_dir: PathBuf,
    /// `KURA_KEYS_DONE`, until the run's report has been written.
    done: Option<PathBuf>,
    watch: Option<Arc<Mutex<crate::ScriptWatch>>>,
    /// This run did not succeed. Outlives the window: `main` reads it after `run_native`.
    failed: Arc<AtomicBool>,
    /// The script has a step that reads the window's tree (`<Assert:>`, `<Click:text>`, `<Tree:>`, ...).
    reads_window: bool,
    /// What the app had copied and announced when the last action went in (`<Announced:>` looks after it).
    since: Option<keyscript::Mark>,
    /// The theme the last `<Theme:>` of this run asked for; empty when none did.
    theme_requested: String,
    /// The language the last `<Lang:>` of this run asked for; empty when none did.
    lang_requested: String,
    /// `<Quit>` has gone in: nothing more is pressed while the window closes.
    quitting: bool,
    /// `run.json`'s record, written beside the done file (only with one).
    run: evidence::Run,
    /// The first `run.json` has been tried.
    run_started: bool,
    /// The last `run.json` has been written, or there is no folder for it.
    run_closed: bool,
    /// `trace.jsonl`, once opened (only with a done file).
    trace: Option<evidence::TraceFile>,
    trace_started: bool,
    /// Why `trace.jsonl` could not be written, until it is reported.
    trace_err: Option<String>,
    trace_reported: bool,
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

/// Add `more` to what the ending says on standard error.
fn add_say(e: &mut pace::Ended, more: String) {
    e.say = Some(match e.say.take() {
        Some(old) => format!("{old}; {more}"),
        None => more,
    });
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
        Self::started(steps, done, shot_dir, failed, Instant::now(), |_| {})
    }

    /// [`Self::new`] with the moment the run starts and a say in Pace's waits (for a test that puts a step "6 s ago").
    fn started(
        steps: &[keyscript::Step],
        done: Option<PathBuf>,
        shot_dir: PathBuf,
        failed: Arc<AtomicBool>,
        now: Instant,
        tune: impl FnOnce(&mut pace::Config),
    ) -> Self {
        let presses: Vec<Press> = steps.iter().filter_map(keyscript::press).collect();
        let reads_window = presses.iter().any(reads_window);
        // As ito's `Runner::new`: announcements are recorded only for a script that looks at them.
        if presses.iter().any(|p| matches!(p, Press::Announced(_) | Press::Tree(_))) {
            ito_a11y::record_announcements(true);
        }
        let mut run = evidence::Run::new("kura::ScriptRun", steps.len());
        run.app = "kura".into();
        run.app_version = env!("CARGO_PKG_VERSION").into();
        // Without a done file there is no report: an empty queue just holds.
        let mut config = pace::Config { report: done.is_some(), ..pace::Config::new("kura") };
        tune(&mut config);
        Self {
            pace: pace::Pace::new(config, steps, now),
            labels: steps.iter().map(keyscript::label).collect(),
            cursor_icon: egui::CursorIcon::Default,
            shot: None,
            shot_dir,
            done,
            watch: None,
            failed,
            reads_window,
            since: None,
            theme_requested: String::new(),
            lang_requested: String::new(),
            quitting: false,
            run,
            run_started: false,
            run_closed: false,
            trace: None,
            trace_started: false,
            trace_err: None,
            trace_reported: false,
        }
    }

    /// Start the thread that reports a script the frame loop has stopped
    /// pressing (`watch_script`); only with a done file to tell.
    pub(crate) fn start_watch(&mut self, ctx: &egui::Context) {
        let Some(done) = self.done.clone() else { return };
        let p = self.pace.progress();
        let watch = Arc::new(Mutex::new(crate::ScriptWatch { left: p.left, at: p.at, due: Duration::ZERO, finished: false }));
        crate::watch_script(watch.clone(), self.labels.clone(), done, ctx.clone(), self.failed.clone());
        self.watch = Some(watch);
    }

    /// Tell the watchdog how far the script has got.
    fn note(&self) {
        let Some(watch) = &self.watch else { return };
        if let Ok(mut w) = watch.lock() {
            let p = self.pace.progress();
            w.left = p.left;
            w.at = p.at;
            w.due = p.due;
            w.finished = p.finished || self.done.is_none();
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

    /// `trace.jsonl`, created once (a run with a done file). Never truncates one that is open.
    fn open_trace(&mut self) {
        if self.trace_started || self.done.is_none() {
            return;
        }
        self.trace_started = true;
        match evidence::TraceFile::create(&self.shot_dir, keyscript::current_run_id()) {
            Ok(t) => self.trace = Some(t),
            Err(e) => self.trace_err = Some(format!("cannot write {}: {e}", evidence::TRACE)),
        }
    }

    /// Write the trace lines Pace has closed. The first write error drops the file.
    fn flush_trace(&mut self) {
        if !self.trace_started {
            if self.done.is_none() {
                // No trace will be written: do not let the lines pile up.
                let _ = self.pace.take_trace();
            }
            return;
        }
        let lines = self.pace.take_trace();
        let Some(trace) = self.trace.as_mut() else { return };
        for line in &lines {
            if let Err(e) = trace.write(line) {
                self.trace = None;
                self.trace_err = Some(format!("cannot write {}: {e}", evidence::TRACE));
                break;
            }
        }
    }

    /// A trace failure while the run goes on fails the run, the way a `run.json` failure does.
    fn trace_report(&mut self, app: &mut App) -> Outcome {
        if self.trace_reported || self.run_closed {
            return Outcome::Continue;
        }
        let Some(why) = self.trace_err.take() else { return Outcome::Continue };
        self.trace_reported = true;
        match self.pace.host_failed(evidence::TRACE_STEP, why) {
            Some(e) => self.keep(app, e),
            None => Outcome::Continue,
        }
    }

    /// After a step of the script: write the trace, report its failure, tell the watch.
    fn sync(&mut self, app: &mut App) -> Outcome {
        self.flush_trace();
        let out = self.trace_report(app);
        self.note();
        if self.pace.failed() {
            self.failed.store(true, Ordering::SeqCst);
        }
        out
    }

    /// The first `run.json`, `running`, at the first frame the player handles (ito's `start_run`). Only with a done
    /// file. A failure to write it fails the run at `<run.json>`.
    fn start_run(&mut self, app: &mut App) -> Option<Outcome> {
        if self.run_started || self.run_closed || self.done.is_none() {
            return None;
        }
        self.run_started = true;
        self.open_trace();
        self.run.run_id = keyscript::current_run_id().map(str::to_owned);
        if let Err(err) = self.run.write(&self.shot_dir) {
            let why = format!("cannot write {}: {err}", evidence::FILE);
            if let Some(mut e) = self.pace.host_failed(evidence::STEP, why) {
                // `keep` writes no evidence for a closed run: this failure's line goes to the trace here.
                self.run_closed = true;
                self.flush_trace();
                self.trace = None;
                if let Some(more) = self.trace_err.take() {
                    add_say(&mut e, more);
                }
                return Some(self.keep(app, e));
            }
        }
        None
    }

    /// The run's ending `e` meets a failure of the evidence (`step` is its label). The first failure is the ending's;
    /// one that follows only goes to `say`.
    fn end_failure(&mut self, e: &mut pace::Ended, step: &str, why: String) {
        self.pace.write_failed();
        e.failed = true;
        add_say(e, why.clone());
        // An ending that reads as a success becomes a failure at `step`; one that already fails keeps its tail and
        // carries the reason in `say`.
        if matches!(e.how, pace::How::Done) || e.tail.ends_with("keys: done\n") {
            e.tail = keyscript::failed_report(step, &why, e.left);
            e.how = pace::How::Failed { step: step.to_owned(), why };
        }
    }

    /// The run has ended as `e`: write `run.json` and `trace.jsonl` (a failure to is a failure of the run), the
    /// done file, the line on standard error and the toast for a person watching.
    fn keep(&mut self, app: &mut App, mut e: pace::Ended) -> Outcome {
        // A late picture is never saved.
        self.shot = None;
        let unsettled = !self.pace.unsettled().is_empty();
        // The evidence could not be written. A close or quit keeps its tail, so `e.how` does not show this.
        let mut lost = false;
        if !self.run_closed && self.done.is_some() {
            self.run_closed = true;
            self.run_started = true;
            // The trace first: a run that ends before its first input still gets one.
            self.open_trace();
            self.flush_trace();
            if let Some(why) = self.trace_err.take() {
                if !std::mem::replace(&mut self.trace_reported, true) {
                    self.end_failure(&mut e, evidence::TRACE_STEP, why);
                    lost = true;
                }
            }
            self.run.run_id = keyscript::current_run_id().map(str::to_owned);
            self.run.end(&e, evidence::now_ms());
            if let Err(err) = self.run.write(&self.shot_dir) {
                let why = format!("cannot write {}: {err}", evidence::FILE);
                self.end_failure(&mut e, evidence::STEP, why.clone());
                lost = true;
                // Pace has ended and cannot report it; the trace says so itself.
                if let Some(trace) = self.trace.as_mut() {
                    let line = self.pace.no_step_line(evidence::STEP, &why);
                    if let Err(err) = trace.write(&line) {
                        add_say(&mut e, format!("cannot write {}: {err}", evidence::TRACE));
                    }
                }
            }
            self.trace = None;
        }
        let ok = self.write_done(&(crate::state_report(app) + &e.tail));
        if let Some(say) = &e.say {
            say_err(say);
        }
        if e.failed {
            self.failed.store(true, Ordering::SeqCst);
        }
        if let pace::How::Failed { step, why } = &e.how {
            app.error(format!("{step}: {why}"));
        }
        self.note();
        match &e.how {
            _ if !ok || lost => Outcome::Failed,
            pace::How::Failed { .. } => Outcome::Failed,
            pace::How::Unsettled => Outcome::Unsettled,
            pace::How::Done => Outcome::Continue,
            pace::How::Closed | pace::How::Quit if unsettled => Outcome::Unsettled,
            pace::How::Closed | pace::How::Quit => Outcome::Continue,
        }
    }

    /// `--keys`: the next scripted key, in the raw input a keyboard would have
    /// filled -- so it takes every road a real press takes.
    ///
    /// Pace decides what is next and when (one step at a time, once what the last one
    /// started has landed, never more than the settle wait behind); this carries the step out.
    /// A key that went in after the settle wait is named in the report, and the run does
    /// not end as a success.
    pub(crate) fn hook(&mut self, ctx: &egui::Context, app: &mut App, raw_input: &mut egui::RawInput, frame: u64, now: Instant) -> Outcome {
        if self.pace.finished() {
            return Outcome::Continue;
        }
        let out = self.step(ctx, app, raw_input, frame, now);
        let synced = self.sync(app);
        if out == Outcome::Continue {
            synced
        } else {
            out
        }
    }

    fn step(&mut self, ctx: &egui::Context, app: &mut App, raw_input: &mut egui::RawInput, frame: u64, now: Instant) -> Outcome {
        if let Some(out) = self.before_step(ctx, app) {
            return out;
        }
        let viewport = (raw_input.viewport_id != egui::ViewportId::ROOT).then(|| format!("{:?}", raw_input.viewport_id));
        let status = app.config_writes.status(app.config_write_failed.as_deref());
        let f = pace::Frame {
            pass: frame,
            now,
            viewport: viewport.as_deref(),
            settled: Some(app.settled()),
            writes: Some(&status),
            generation: keyscript::generation(ctx),
        };
        self.advance(ctx, app, &f, |run, app, entry| run.carry_out(ctx, app, raw_input, entry, now))
    }

    /// What comes before a frame is handed to Pace: the first `run.json`, the window's reading (it must precede
    /// `keyscript::generation`), and a quit that has already been taken. `Some` ends the frame there.
    fn before_step(&mut self, ctx: &egui::Context, app: &mut App) -> Option<Outcome> {
        if let Some(out) = self.start_run(app) {
            return Some(out);
        }
        if self.reads_window {
            keyscript::observe(ctx);
        }
        self.quitting.then_some(Outcome::Continue)
    }

    /// Ask Pace what the frame `f` holds and carry it out with `carry`. A seam so that a test can feed its own frame.
    fn advance(
        &mut self,
        ctx: &egui::Context,
        app: &mut App,
        f: &pace::Frame,
        carry: impl FnOnce(&mut Self, &mut App, &pace::Entry) -> (pace::Taken, Option<CommonEdit>),
    ) -> Outcome {
        match self.pace.next(f) {
            pace::Next::Hold { wake } => {
                match wake {
                    None => {}
                    Some(d) if d.is_zero() => ctx.request_repaint(),
                    Some(d) => ctx.request_repaint_after(d),
                }
                Outcome::Continue
            }
            pace::Next::Take(entry) => {
                let (taken, edit) = carry(self, app, &entry);
                // An action's first input goes in now: what the app copied and announced from here on is what
                // `<Announced:>` looks at, and `<State:>` says how long ago it was. Once per script step.
                if pace::conformance::acts_taken(&entry, &taken) && self.pace.act(f.now) {
                    self.since = Some(keyscript::mark(ctx));
                }
                // The same write the settings screen makes: common.toml, applied as the watcher would.
                if let Some((key, value)) = edit {
                    app.write_common(vec![ito_common::CommonChange { table: None, key, value }]);
                }
                let quit = matches!(taken, pace::Taken::Quit);
                let ended = self.pace.taken(f, taken);
                ctx.request_repaint();
                if quit {
                    self.quitting = true;
                }
                match ended {
                    Some(e) => self.keep(app, e),
                    None => Outcome::Continue,
                }
            }
            pace::Next::End(e) => self.keep(app, e),
        }
    }

    /// Carry out one step in the window. Pacing is Pace's: this only does the step and says what came of it.
    /// A change of `common.toml` comes back to be written after the step has been marked.
    fn carry_out(
        &mut self,
        ctx: &egui::Context,
        app: &mut App,
        raw_input: &mut egui::RawInput,
        entry: &pace::Entry,
        now: Instant,
    ) -> (pace::Taken, Option<CommonEdit>) {
        use pace::{More, Taken};
        let input = |more: Vec<More>| Taken::Input { flight: true, more };
        let verdict = |r: Result<(), String>| match r {
            Ok(()) => Taken::Done,
            Err(why) => Taken::Again { why },
        };
        let mut edit = None;
        let taken = match &entry.press {
            Press::Events(events) => {
                raw_input.events.extend(events.iter().cloned());
                input(Vec::new())
            }
            // Read as it goes in, not when the script was parsed: an earlier
            // key of the script may have put something on the clipboard.
            Press::Paste => {
                raw_input.events.push(keyscript::paste_event());
                input(Vec::new())
            }
            Press::PasteText(text) => {
                raw_input.events.push(keyscript::paste_text_event(text));
                input(Vec::new())
            }
            p @ (Press::Click { .. } | Press::DClick { .. } | Press::Hover { .. } | Press::Wheel { .. } | Press::Drag { .. }) => {
                let rect = ctx.input(|i| i.viewport_rect());
                let mut events = keyscript::pointer_events(p, rect);
                // A drag one event per frame: in one frame egui never sees the
                // button held while the pointer moves, so nothing starts to
                // drag (#283, #284). The first goes in now, the rest follow
                // as continuations of the same step.
                let mut more = Vec::new();
                if matches!(p, Press::Drag { .. }) && events.len() > 1 {
                    more = events.split_off(1).into_iter().map(|e| More::Then(Press::Events(vec![e]))).collect();
                }
                raw_input.events.extend(events);
                input(more)
            }
            // Written now: Pace has waited for the screen the key before it left (#230).
            Press::State(name) => {
                let path = self.shot_dir.join(format!("{name}.txt"));
                let text = crate::state_report(app)
                    + &term_state_lines(app)
                    + &format!(
                        "cursor icon: {:?}\nsettled: {}\nconfig writes pending: {}\nconfig write failed: {}\n",
                        self.cursor_icon,
                        if app.settled() { "yes" } else { "no" },
                        if app.config_writes_pending() { "yes" } else { "no" },
                        app.config_write_failed.as_deref().unwrap_or("none"),
                    )
                    + &keyscript::since_action_line(self.pace.since_action(now));
                match write_text(&path, &text) {
                    Ok(()) => Taken::Done,
                    Err(why) => Taken::Failed(why),
                }
            }
            // The readings kura adds to the common script (`keyscript::DIALECT`).
            Press::Read { tag, name } => {
                let written = match tag.as_str() {
                    // The pane's own text, beside the state: `name.pane.txt`, so a
                    // `<State:name>` of the same name does not take its place (#229).
                    "PaneText" => write_text(&self.shot_dir.join(format!("{name}.pane.txt")), &reading(app, tag).unwrap_or_default()),
                    "PreviewText" => write_text(&self.shot_dir.join(format!("{name}.preview.txt")), &reading(app, tag).unwrap_or_default()),
                    other => Err(format!("kura's --keys has no reading called {other}")),
                };
                match written {
                    Ok(()) => Taken::Done,
                    Err(why) => Taken::Failed(why),
                }
            }
            // Pace reads again each frame until the text is in the reading.
            Press::ReadHas { tag, text } => {
                let got = reading(app, tag);
                if got.as_deref().is_some_and(|g| g.contains(text.as_str())) {
                    Taken::Done
                } else {
                    Taken::Again { why: pace::read_has_why(tag, text, got.as_deref()) }
                }
            }
            // Straight to the quit `ui` already handles, past anything that
            // would ask first or take `q` for itself (#236).
            Press::Quit => {
                app.quit = true;
                Taken::Quit
            }
            // Taken in `ui`, which is where a viewport command can be sent.
            Press::Shot(name) => {
                self.shot = Some(Shot { name: name.clone(), sent: false, theme: String::new(), lang: String::new() });
                Taken::Shot { path: self.shot_path(name).to_string_lossy().into_owned() }
            }
            // The window lays out again at the new size before the next step.
            Press::Size { w, h } => {
                ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(*w as f32, *h as f32)));
                Taken::Changed { more: vec![More::Step(Press::Wait(Duration::from_millis(400)))] }
            }
            Press::Theme(name) => {
                self.theme_requested = name.clone();
                edit = Some(("theme", format!("{name:?}")));
                Taken::Changed { more: Vec::new() }
            }
            Press::Lang(code) => {
                self.lang_requested = code.clone();
                edit = Some(("language", format!("{code:?}")));
                Taken::Changed { more: Vec::new() }
            }
            // Refused before the window opens (`unsupported`); a run handed one anyway fails on it.
            Press::Snap { name, .. } => Taken::Failed(format!("<Snap:{name}>: {UNSUPPORTED_WHY}")),
            // The window-reading steps. Each needs a tree drawn after the last action; Pace waits for
            // what it looks for, then fails the run with the reason.
            Press::ClickText { right, text } => match self.fresh(ctx).and_then(|_| keyscript::locate(ctx, text)) {
                Ok(pos) => {
                    raw_input.events.extend(click_events(pos, *right, false));
                    input(Vec::new())
                }
                Err(why) => Taken::Again { why },
            },
            Press::DClickText(text) => match self.fresh(ctx).and_then(|_| keyscript::locate(ctx, text)) {
                Ok(pos) => {
                    raw_input.events.extend(click_events(pos, false, true));
                    input(Vec::new())
                }
                Err(why) => Taken::Again { why },
            },
            Press::ClickId(name) => {
                let found = self.fresh(ctx).and_then(|o| match o.id(name)? {
                    Some(node) => node.click_at.clone(),
                    None => Err(format!("no widget has the test ID `{name}`")),
                });
                match found {
                    Ok(pos) => {
                        raw_input.events.extend(click_events(pos, false, false));
                        input(Vec::new())
                    }
                    Err(why) => Taken::Again { why },
                }
            }
            Press::Assert { text, present } => verdict(self.fresh(ctx).and_then(|_| keyscript::check(ctx, text, *present))),
            Press::Is { prop, text } => verdict(self.fresh(ctx).and_then(|_| keyscript::check_is(ctx, *prop, text))),
            Press::IdCheck(check) => verdict(self.fresh(ctx).and_then(|o| check.check(&o))),
            Press::Announced(text) => {
                verdict(self.since.map_or_else(|| keyscript::check_announced(text), |m| keyscript::check_announced_after(m, text)))
            }
            Press::Tree(name) => match self.fresh(ctx) {
                Ok(o) if o.tree.is_empty() => Taken::Again { why: "the window's tree was empty".to_owned() },
                Err(why) => Taken::Again { why },
                Ok(_) => {
                    // The same file name as a `<State:>` of that name, which it overwrites (as in ito).
                    let path = self.shot_dir.join(format!("{name}.txt"));
                    match std::fs::write(&path, keyscript::tree_text(ctx)) {
                        Ok(()) => Taken::Done,
                        Err(e) => Taken::Failed(format!("cannot write {}: {e}", path.display())),
                    }
                }
            },
            // What kura copied after the last action (ito's OS clipboard writer notes each copy for its record).
            Press::Clipboard(text) => verdict(keyscript::check_clipboard_after(ctx, self.since.unwrap_or_default(), text)),
            Press::ClipboardHas(text) => verdict(keyscript::check_clipboard_has_after(ctx, self.since.unwrap_or_default(), text)),
            // Pace spends these itself and never hands them out.
            Press::Wait(_) | Press::Now => Taken::Failed("internal error: Pace handed out a step it spends itself".to_owned()),
        };
        (taken, edit)
    }

    /// The window as a reading step may judge it: seen after the last input was drawn, in the main
    /// window, and not minimized (ito's `Runner::fresh`).
    fn fresh(&self, ctx: &egui::Context) -> Result<keyscript::Observation, String> {
        let o = keyscript::observation(ctx).ok_or_else(|| pace::fresh(0, 0, false, None).unwrap_err())?;
        pace::fresh(o.generation, self.pace.action_gen(), o.viewport == egui::ViewportId::ROOT, o.minimized)?;
        Ok(o)
    }

    /// When the next frame is wanted for a retry that waits: a repaint asked for in `hook` can be
    /// forgotten when the pass begins (egui resets the delay then), so a quiet window could stop
    /// drawing while a check waits, and the watchdog only pokes once a second.
    pub(crate) fn due(&self) -> Option<Duration> {
        self.pace.due()
    }

    /// Where `<Shot:name>` saves: `shots/name.png` beside the done file (`name@preview` is the preview pane's crop of `name.png`).
    fn shot_path(&self, name: &str) -> PathBuf {
        self.shot_dir.join("shots").join(format!("{}.png", shot_file(name)))
    }

    /// `<Shot:name>`: ask for the picture, then save it when it arrives.
    pub(crate) fn take_shot(&mut self, ctx: &egui::Context, app: &mut App) -> Outcome {
        let out = self.shot_step(ctx, app);
        let synced = self.sync(app);
        if out == Outcome::Continue {
            synced
        } else {
            out
        }
    }

    fn shot_step(&mut self, ctx: &egui::Context, app: &mut App) -> Outcome {
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
        let (name, in_force) = (shot.name.clone(), (shot.theme.clone(), shot.lang.clone()));
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
        self.shot_result(app, saved, Instant::now())
    }

    /// The picture of `<Shot:>` was saved or could not be: tell Pace. A seam so that a test can feed its own answer.
    fn shot_result(&mut self, app: &mut App, saved: Result<(), String>, now: Instant) -> Outcome {
        match saved {
            Ok(()) => {
                self.shot = None;
                self.pace.shot_saved(now);
                Outcome::Continue
            }
            Err(why) => match self.pace.shot_failed(why) {
                Some(e) => self.keep(app, e),
                None => Outcome::Continue,
            },
        }
    }

    /// The window was closed (the title bar's x) or the app quit while the
    /// script ran: write what is left of the report. The picture of a
    /// `<Shot:>` still awaited is an unfinished step, whatever the queue says.
    /// The caller runs the close action of its path either way.
    pub(crate) fn ended(&mut self, app: &mut App, how: Ending) -> Outcome {
        let status = app.config_writes.status(app.config_write_failed.as_deref());
        let how = match how {
            Ending::Closed => pace::Ending::Closed,
            Ending::Quit => pace::Ending::Quit,
        };
        let out = self.ended_at(app, how, Instant::now(), Some(&status));
        let synced = self.sync(app);
        if out == Outcome::Continue {
            synced
        } else {
            out
        }
    }

    /// Pace's end of the run at `now` (`writes` as of then), kept. A seam so that a test can pick the time.
    fn ended_at(&mut self, app: &mut App, how: pace::Ending, now: Instant, writes: Option<&ito_common::writes::Status>) -> Outcome {
        match self.pace.end(how, now, writes) {
            Some(e) => self.keep(app, e),
            None => Outcome::Continue,
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
            ScriptRun::started(&steps, done, shot_dir, self.failed.clone(), self.t0, |_| {})
        }

        /// `run`, from steps built by hand (a step the parser would not make).
        fn run_of(&self, steps: &[keyscript::Step], done: Option<PathBuf>, shot_dir: PathBuf) -> ScriptRun {
            ScriptRun::started(steps, done, shot_dir, self.failed.clone(), self.t0, |_| {})
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

    /// kura's `--keys` glue as a subject of ito's Pace conformance table (`pace::conformance`), the way ito runs it on its
    /// own `Runner`. The table checks the glue: the order of `act`, the mark and `taken`, `keep` and `ended`, the
    /// viewport, the shot events and the closing. It does not check `carry_out` (how a step becomes a press) or the
    /// `common.toml` edit: the answers come from the table.
    struct KuraSubject;

    impl pace::conformance::Subject for KuraSubject {
        fn skips(&self) -> &[pace::conformance::Need] {
            // kura writes no state JSON yet and refuses `<Snap:>`.
            &[pace::conformance::Need::StateJson, pace::conformance::Need::Snap]
        }

        fn run(&mut self, case: &pace::conformance::Case) -> pace::conformance::Trace {
            use pace::conformance::{Answer, ShotEvent, Trace};
            let steps = keyscript::parse_with(case.script, &pace::conformance::dialect()).unwrap_or_else(|e| panic!("{}: {e}", case.name));
            let mut rig = Rig::new(&format!("conformance-{}", case.name), true);
            let done = rig.dir.join("done.txt");
            let shot_dir = rig.dir.join("shots");
            std::fs::create_dir_all(&shot_dir).unwrap();
            let base = rig.t0;
            let at = |ms: u64| base + Duration::from_millis(ms);
            let mut run = ScriptRun::started(&steps, Some(done.clone()), shot_dir, rig.failed.clone(), base, |c| *c = case.config.clone());
            // Nothing draws during a case, so the state `keep` prefixes the report with does not change.
            let prefix = crate::state_report(&rig.screen.app);
            let ctx = rig.screen.ctx.clone();
            let app = &mut rig.screen.app;
            let mut tr = Trace::default();
            let mut end_ms = None;
            for tick in &case.ticks {
                assert!(tick.state.is_none(), "{}: a state event reached a subject that skips StateJson", case.name);
                let now = at(tick.ms);
                if let Some(event) = &tick.shot {
                    let saved = match event {
                        ShotEvent::Saved => Ok(()),
                        ShotEvent::Failed(why) => Err((*why).to_owned()),
                    };
                    let _ = run.shot_result(app, saved, now);
                    let _ = run.sync(app);
                    if run.done.is_none() {
                        end_ms = Some(tick.ms);
                        break;
                    }
                }
                if run.pace.finished() {
                    continue;
                }
                run.since = None;
                if run.before_step(&ctx, app).is_some() {
                    continue;
                }
                let status = tick.writes.as_ref().map(|w| w.status(base));
                let frame = pace::Frame {
                    pass: tick.pass,
                    now,
                    viewport: tick.viewport,
                    settled: tick.settled,
                    writes: status.as_ref(),
                    generation: tick.generation,
                };
                let mut idx = None;
                run.advance(&ctx, app, &frame, |_, _, entry| {
                    idx = Some(entry.idx);
                    tr.takes.push((entry.idx, tick.ms, entry.kind == pace::Kind::Then));
                    (tick.answer.clone().unwrap_or_else(|| Answer::default_for(entry)).taken(entry), None)
                });
                if let (Some(i), true) = (idx, run.since.is_some()) {
                    tr.marks.push((i, tick.ms));
                }
                let _ = run.sync(app);
                if run.done.is_none() {
                    end_ms = Some(tick.ms);
                    break;
                }
            }
            if let Some((ms, how)) = case.end {
                let _ = run.ended_at(app, how, at(ms), None);
                if end_ms.is_none() && run.done.is_none() {
                    end_ms = Some(ms);
                }
                if run.pace.end(how, at(ms), None).is_some() {
                    tr.why = Some("a second end returned Some".to_owned());
                }
            }
            if run.done.is_none() {
                let own = body(&read(&done));
                let tail =
                    own.strip_prefix(&prefix).unwrap_or_else(|| panic!("{}: the report does not start with the state:\n{own}", case.name));
                tr.tail = Some(tail.to_owned());
            }
            if run.run.status == evidence::Status::Failed && tr.why.is_none() {
                tr.why = run.run.why.clone();
            }
            tr.ended_left = run.run.left;
            tr.failed = run.pace.failed();
            tr.left = run.pace.left();
            tr.unsettled = run.pace.unsettled().to_vec();
            tr.action_gen = run.pace.action_gen();
            tr.end_ms = end_ms;
            tr
        }
    }

    #[test]
    fn kura_meets_the_conformance_table() {
        let cases = pace::conformance::cases();
        let skipped = pace::conformance::skipped(&KuraSubject, &cases).len();
        assert!(skipped * 2 <= cases.len(), "the table must not be passed by skipping it: {skipped} of {} skipped", cases.len());
        pace::conformance::check(&mut KuraSubject, &cases).unwrap();
    }

    /// `<Quit>` has gone in and the window takes a few frames to close: nothing more is pressed.
    /// The table has no frame after a quit, so this one is kura's own.
    #[test]
    fn nothing_is_pressed_while_the_window_closes() {
        let mut rig = Rig::new("keys-quit-closing", true);
        let mut run = rig.run("<Quit>j", None, rig.dir.clone());
        rig.hook_by(&mut run, Duration::from_secs(1));
        assert!(rig.screen.app.quit, "<Quit> has run");
        assert!(!run.pace.finished(), "`j` is still queued, so only the quit guard can hold it back");
        for _ in 0..4 {
            let (out, events) = rig.hook_events(&mut run, Duration::from_secs(1));
            assert_eq!(out, Outcome::Continue);
            assert!(events.is_empty(), "{events:?}");
        }
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
            run.ended(&mut rig.screen.app, how);
            let json = squeezed(&rig.dir.join("run.json"));
            assert!(json.contains(&format!("\"status\":\"{status}\"")), "{status}: {json}");
        }
    }

    /// A run with a done file writes `trace.jsonl` beside run.json, one line per step; a run without one writes neither.
    #[test]
    fn a_done_file_run_writes_a_trace_line_per_step() {
        let mut rig = Rig::new("keys-trace", true);
        let done = rig.dir.join("done.txt");
        let mut run = rig.run("jk<State:s>", Some(done), rig.dir.clone());
        assert_eq!(until_end(&mut rig, &mut run, 1, 20), Outcome::Continue);
        let trace = read(&rig.dir.join(evidence::TRACE));
        assert_eq!(trace.lines().count(), 3, "{trace}");

        let mut rig = Rig::new("keys-no-trace", true);
        let mut run = rig.run("j", None, rig.dir.clone());
        assert_eq!(until_end(&mut rig, &mut run, 1, 20), Outcome::Continue);
        assert!(!rig.dir.join(evidence::TRACE).exists() && !rig.dir.join(evidence::FILE).exists());
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
        assert_eq!(run.ended(&mut rig.screen.app, Ending::Quit), Outcome::Failed);
        let said = read(&done);
        assert!(said.contains("keys: quit\nleft: "), "{said}");
        assert!(!said.contains("step: <run.json>"), "{said}");
        assert!(rig.flag());
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
        let trace = read(&rig.dir.join(evidence::TRACE));
        assert!(trace.contains("<run.json>"), "the failure has its line in the trace: {trace}");
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
            assert!(run.pace.left() == 0 && run.done.is_none(), "{script}: no more keys, one report");
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
        let mut run = rig.run_of(&[keyscript::Step::Read { tag: "Bogus".into(), name: "n".into() }], Some(bad.clone()), rig.dir.clone());
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
        // No pane in the harness: the reading is "(no pane)".
        let mut run = rig.run("<ReadHas:PaneText=(no pane)>", Some(done.clone()), rig.dir.clone());
        assert_eq!(rig.step(&mut run, 0), Outcome::Continue);
        assert!(run.pace.left() == 0, "the text was there at once");

        let mut run = rig.run("<ReadHas:PaneText=never there>", Some(done.clone()), rig.dir.clone());
        assert_eq!(rig.step(&mut run, 0), Outcome::Continue, "waits");
        assert_eq!(run.pace.left(), 1);
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
        let mut run = rig.run("<ReadHas:PreviewText=late text>", Some(done.clone()), rig.dir.clone());
        rig.screen.app.preview.state = PreviewState::Loading;
        assert_eq!(rig.step(&mut run, 6), Outcome::Continue, "starts looking");
        assert_eq!(run.pace.left(), 1, "not there yet");
        rig.screen.app.preview.state = PreviewState::Ready(Payload::Error("some late text".into()));
        assert_eq!(rig.step(&mut run, 0), Outcome::Continue, "looks again without waiting to settle");
        assert!(run.pace.left() == 0, "found on the next frame");
        rig.screen.app.preview.in_flight = false;
        while rig.step(&mut run, 0) == Outcome::Continue && run.done.is_some() {}
        let said = read(&done);
        assert!(said.ends_with("keys: done\n"), "{said}");
        assert!(!said.lines().any(|l| l.starts_with("unsettled") || l.starts_with("keys: unsettled")), "{said}");
        assert!(!rig.flag());

        // After `<Now>`, the second look does not wait out the settle time either.
        rig.screen.app.preview.in_flight = true;
        rig.screen.app.preview.state = PreviewState::Loading;
        let mut run = rig.run("<Now><ReadHas:PreviewText=later>", Some(done.clone()), rig.dir.clone());
        assert_eq!(rig.step(&mut run, 0), Outcome::Continue);
        assert_eq!(run.pace.left(), 1, "looked once under <Now>");
        rig.screen.app.preview.state = PreviewState::Ready(Payload::Error("later".into()));
        assert_eq!(rig.step(&mut run, 0), Outcome::Continue);
        assert!(run.pace.left() == 0, "found on the next frame, not after the settle wait");
    }

    /// What a failed `<ReadHas:>` read is cut to its first 200 characters.
    #[test]
    fn read_has_names_at_most_200_characters_of_what_it_read() {
        use crate::app::PreviewState;
        use crate::preview::Payload;
        let mut rig = Rig::new("keys-read-has-long", true);
        let done = rig.dir.join("done.txt");
        let mut run = rig.run("<ReadHas:PreviewText=zzz>", Some(done.clone()), rig.dir.clone());
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
        let mut run = rig.run_of(&[keyscript::Step::Read { tag: "Bogus".into(), name: "n".into() }], Some(done.clone()), rig.dir.clone());
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
            assert_eq!(run.pace.left(), 1, "the shot is the step still to be answered");
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
        assert!(run.pace.left() > 0, "k is still to go");
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
        // A step can wait for the settle wait and then for the longest wait of its own.
        let c = pace::Config::new("kura");
        let longest = [
            c.settle_wait,
            c.shot_deadline,
            c.state_deadline,
            keyscript::DELIVER_WAIT,
            keyscript::TEXT_WAIT,
            keyscript::READ_WAIT,
            pace::CONFIG_WAIT,
        ]
        .into_iter()
        .max()
        .unwrap();
        assert!(c.settle_wait + longest < keyscript::STALL, "{:?} + {longest:?} < {:?}", c.settle_wait, keyscript::STALL);
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
        let before = run.pace.left();
        assert_eq!(rig.tick(&mut run, 0, 4, other), Outcome::Continue);
        assert!(run.pace.left() == before && !run.pace.in_flight());
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
            if run.pace.left() == 2 {
                break;
            }
        }
        assert_eq!(run.pace.left(), 2, "j went in and was drawn (so a tree exists)");
        assert_eq!(rig.hook_by(&mut run, Duration::ZERO), Outcome::Continue);
        assert_eq!(run.pace.left(), 1, "<C-,> went in, and is never drawn");
        assert_eq!(rig.hook_by(&mut run, Duration::ZERO), Outcome::Continue);
        assert!(run.due().is_some(), "the check waits for a newer tree");
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
        assert_eq!(run.pace.left(), 1, "the check has not passed on the old tree");
        assert!(run.due().is_some(), "it is waiting");
        for _ in 0..30 {
            rig.pass(&mut run, 0);
            if run.pace.left() == 0 {
                break;
            }
        }
        assert!(run.pace.left() == 0, "the check passed on a tree drawn after the answer");
    }

    /// A missing text is looked for until `TEXT_WAIT` is up: still waiting 1 ms short of it, failed at it.
    #[test]
    fn checks_retry_then_fail_exactly_at_text_wait() {
        let mut rig = Rig::new("keys-text-wait", true);
        let done = rig.dir.join("done.txt");
        let mut run = rig.run("<C-,><Assert:Not on this screen>", Some(done.clone()), rig.dir.clone());
        for _ in 0..20 {
            rig.pass(&mut run, 0);
            if run.pace.left() == 1 && run.due().is_some() {
                break;
            }
        }
        assert!(run.due().is_some(), "the check looked once and did not find it");
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

    /// `<Now>` is for the step after it: a look of that step that has to look again has not used it up. A `<Click:>` is
    /// what shows it (a check looks again each pass with or without `<Now>`, and is never named unsettled): on a screen
    /// still moving, the second look goes in on the next pass and is not named. The click is never drawn.
    #[test]
    fn now_survives_a_look_that_looks_again() {
        let mut rig = Rig::new("keys-now-retry", false);
        let done = rig.dir.join("done.txt");
        let mut run = rig.run("<Now><Click:Open config.toml>", Some(done), rig.dir.clone());
        assert_eq!(rig.pass(&mut run, 0), Outcome::Continue);
        assert_eq!(run.pace.left(), 1, "the look did not find it");
        rig.screen.app.toggle_settings();
        rig.screen.feed(Vec::new());
        assert!(!rig.screen.app.settled(), "the rig's screen is still moving");
        assert_eq!(rig.hook_by(&mut run, Duration::ZERO), Outcome::Continue);
        assert_eq!(run.pace.left(), 0, "found on the next pass, not after the settle wait");
        assert!(run.pace.unsettled().is_empty(), "it went in under <Now>, so it is not named");
    }

    /// A drag is several steps of one action: only its first frame marks the action.
    #[test]
    fn drag_marks_only_first_frame() {
        let mut rig = Rig::new("keys-drag-act", true);
        let mut run = rig.run("<Drag:0.2,0.3-0.6,0.3>", Some(rig.dir.join("done.txt")), rig.dir.clone());
        let (out, first) = rig.hook_events(&mut run, Duration::ZERO);
        assert_eq!(out, Outcome::Continue);
        let acted = rig.t0;
        let idx = run.pace.acted_idx();
        assert!(idx.is_some(), "the first frame acted");
        assert_eq!(run.pace.since_action(rig.t0), Some(Duration::ZERO), "the action is marked at the first frame");
        assert!(run.since.is_some());
        assert!(!first.is_empty(), "the first frame of the drag went in");
        let mut tail_frames = 0;
        while run.pace.left() > 0 && tail_frames < 20 {
            let (out, events) = rig.hook_events(&mut run, Duration::from_secs(1));
            assert_eq!(out, Outcome::Continue);
            assert!(!events.is_empty(), "each frame carries the drag on");
            tail_frames += 1;
            assert_eq!(run.pace.since_action(rig.t0), Some(rig.t0 - acted), "a tail frame of the drag did not act again");
            assert_eq!(run.pace.acted_idx(), idx);
        }
        assert!(run.pace.left() == 0 && tail_frames > 1, "the drag's tail frames went in");
    }

    /// A drag goes on frame by frame while the screen moves: its tail waits for no settling and is never named unsettled.
    #[test]
    fn a_drags_tail_does_not_wait_for_a_settled_screen() {
        let mut rig = Rig::new("keys-drag-unsettled", false);
        let mut run = rig.run("<Now><Drag:0.2,0.3-0.6,0.3>", Some(rig.dir.join("done.txt")), rig.dir.clone());
        assert_eq!(rig.hook_by(&mut run, Duration::ZERO), Outcome::Continue);
        let start = rig.t0;
        let mut tail_frames = 0;
        while run.pace.left() > 0 && tail_frames < 20 {
            let (out, events) = rig.hook_events(&mut run, Duration::from_millis(30));
            assert_eq!(out, Outcome::Continue);
            assert!(!events.is_empty(), "the screen is still moving and the drag goes on");
            tail_frames += 1;
        }
        assert!(run.pace.left() == 0 && tail_frames > 1, "the drag's tail frames went in");
        assert!(rig.t0 - start < pace::Config::new("kura").settle_wait, "well inside the settle wait");
        assert!(!rig.screen.app.settled(), "the rig's screen is still moving");
        assert!(run.pace.unsettled().is_empty(), "the tail is never named");
    }

    /// `<Now>` does not lift a check's deadline: a text that never comes still fails it.
    #[test]
    fn a_check_after_now_still_fails_at_its_deadline() {
        let mut rig = Rig::new("keys-now-deadline", true);
        let done = rig.dir.join("done.txt");
        let mut run = rig.run("<Now><Assert:Not on this screen>", Some(done.clone()), rig.dir.clone());
        assert_eq!(rig.pass(&mut run, 0), Outcome::Continue);
        assert_eq!(rig.pass(&mut run, 1), Outcome::Continue);
        assert_eq!(rig.pass(&mut run, 4), Outcome::Failed);
        assert!(read(&done).contains("keys: failed\n"));
    }

    /// `<Now>` is for the one step after it. Once the check has passed it is spent: the key after the check waits for
    /// the settled screen, and is named when it goes in without one. Nothing is clicked here, so no opener is launched.
    #[test]
    fn now_is_spent_by_the_check_it_precedes() {
        let mut rig = Rig::new("keys-now-spent", false);
        let mut run = rig.run("<Now><Assert:Open config.toml>j", Some(rig.dir.join("done.txt")), rig.dir.clone());
        assert_eq!(rig.pass(&mut run, 0), Outcome::Continue);
        assert_eq!(run.pace.left(), 2, "the check did not find it");
        rig.screen.app.toggle_settings();
        rig.screen.feed(Vec::new());
        assert_eq!(rig.pass(&mut run, 0), Outcome::Continue);
        assert_eq!(run.pace.left(), 1, "the check found it");
        assert_eq!(rig.pass(&mut run, 1), Outcome::Continue);
        assert_eq!(run.pace.left(), 1, "the key waits for the screen: <Now> is spent");
        assert!(rig.pass(&mut run, 6) != Outcome::Failed);
        assert_eq!(run.pace.left(), 0, "the settle wait is over");
        assert_eq!(run.pace.unsettled().len(), 1, "and the key is named");
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
        assert!(run.due().is_some(), "the tree waits for a newer one");
        assert_eq!(run.pace.left(), 2);
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
            if run.pace.left() == 1 {
                break;
            }
        }
        assert_eq!(run.pace.left(), 1, "only the Assert is left");
        // The Assert has its own 3 s, counted from when it began, not from the Tree's start.
        assert_eq!(rig.hook_by(&mut run, Duration::from_secs(2)), Outcome::Continue);
        assert_eq!(rig.hook_by(&mut run, Duration::from_secs(4)), Outcome::Failed);
    }

    /// A check that looks again has made no progress: the stall watchdog's clock stays where it was.
    #[test]
    fn retry_before_wait_preserves_watchdog() {
        let mut rig = Rig::new("keys-retry-progress", true);
        let mut run = rig.run("<C-,><Assert:Not on this screen>", Some(rig.dir.join("done.txt")), rig.dir.clone());
        for _ in 0..20 {
            rig.pass(&mut run, 0);
            if run.pace.left() == 1 && run.due().is_some() {
                break;
            }
        }
        assert!(run.due().is_some());
        let at = run.pace.progress().at;
        assert_eq!(rig.pass(&mut run, 1), Outcome::Continue);
        assert_eq!(rig.pass(&mut run, 1), Outcome::Continue);
        assert_eq!(run.pace.progress().at, at, "a retry is not progress");
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
        assert!(run.pace.left() == 0 && run.pace.finished(), "every step went in");
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
        assert!(run.since.is_some() && run.pace.acted_idx().is_some(), "the click is an action");

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
        assert!(run.pace.left() == 0 && run.pace.finished(), "every step passed");
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
