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
use crate::keyscript::{self, Press};

/// How long a step waits for `App::settled()`, and a `<Shot:>` for its picture.
const DEFAULT_WAIT: Duration = Duration::from_secs(5);

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
}

/// What kura cannot carry out: `None` for the steps it can. An exhaustive
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
        Press::Read { tag, .. } => match tag.as_str() {
            "PaneText" | "PreviewText" => None,
            _ => Some("a reading kura does not have"),
        },
        Press::ClickText { .. } | Press::DClickText(_) => Some("a click on text"),
        Press::Assert { .. } | Press::Is { .. } | Press::Announced(_) | Press::Tree(_) => Some("a check of the window's tree"),
        Press::Clipboard(_) | Press::ClipboardHas(_) => Some("a check of the clipboard"),
    }
}

/// Said on standard error. Not `eprintln!`: started from `kura.com` this is a
/// pipe nobody reads once the window is up, and that would panic.
fn say_err(text: &str) {
    use std::io::Write;
    let _ = writeln!(std::io::stderr(), "kura --keys: {text}");
}

/// Write a report, and when it cannot be written say so and mark the run as
/// not succeeded: a done file that is missing reads as a stall, not as a result.
pub(crate) fn write_report(path: &Path, text: &str, failed: &AtomicBool) -> bool {
    match std::fs::write(path, text) {
        Ok(()) => true,
        Err(e) => {
            say_err(&format!("cannot write {}: {e}", path.display()));
            failed.store(true, Ordering::SeqCst);
            false
        }
    }
}

/// A script with a step kura does not carry out, refused before any window
/// opens: `step` is written as in the script, `i` of `n` from 1.
pub(crate) fn refuse_unsupported(step: &str, i: usize, n: usize) -> ! {
    let why = format!("step {i} of {n}: kura's --keys does not carry out this step yet");
    // Before the window, so on the console a person typed it in, like `refuse_keys`.
    crate::say_err(&format!("kura: --keys: failed: step {step}: {why}"));
    if let Some(done) = std::env::var_os("KURA_KEYS_DONE") {
        write_report(Path::new(&done), &keyscript::failed_report(step, &why, n), &AtomicBool::new(false));
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

/// Write `text` to `path`, the error naming the path.
fn write_text(path: &Path, text: &str) -> Result<(), String> {
    std::fs::write(path, text).map_err(|e| format!("could not write {}: {e}", path.display()))
}

impl ScriptRun {
    /// The script `steps` (from `keyscript::parse`), reporting to `done` and
    /// saving what it writes into `shot_dir`. `failed` is the flag `main`
    /// turns into the exit code.
    pub(crate) fn new(steps: &[keyscript::Step], done: Option<PathBuf>, shot_dir: PathBuf, failed: Arc<AtomicBool>) -> Self {
        Self {
            queue: steps.iter().enumerate().filter_map(|(i, s)| keyscript::press(s).map(|p| (i, p))).collect(),
            labels: steps.iter().map(keyscript::label).collect(),
            at: (0, Instant::now()),
            now: false,
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
        }
    }

    /// Start the thread that reports a script the frame loop has stopped
    /// pressing (`watch_script`); only with a done file to tell.
    pub(crate) fn start_watch(&mut self, ctx: &egui::Context) {
        let Some(done) = self.done.clone() else { return };
        let watch =
            Arc::new(Mutex::new(crate::ScriptWatch { left: self.left(None), at: Instant::now(), due: Duration::ZERO, finished: false }));
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
            w.at = self.at.1;
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
        self.failed.store(true, Ordering::SeqCst);
        say_err(&format!("failed: step {label}: {why}"));
        self.queue.clear();
        self.shot = None;
        let report = crate::state_report(app) + &keyscript::failed_report(&label, &why, left);
        self.write_done(&report);
        app.error(format!("{label}: {why}"));
        Outcome::Failed
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
        let ready = if self.now { frame > last_frame } else { frame >= last_frame + 2 && (settled || waited_long) };
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
            let tail = lines.as_deref().unwrap_or("keys: done\n");
            if let Some(lines) = &lines {
                self.failed.store(true, Ordering::SeqCst);
                say_err(&lines.lines().collect::<Vec<_>>().join("; "));
            }
            let ok = self.write_done(&(crate::state_report(app) + tail));
            self.note_progress();
            return match (ok, lines.is_some()) {
                (false, _) => Outcome::Failed,
                (true, true) => Outcome::Unsettled,
                (true, false) => Outcome::Continue,
            };
        };
        // A `<Wait:N>` presses nothing: it is not named when the screen was unsettled.
        let mut record = true;
        match press {
            // Counted from the key before it: the next key goes in at least
            // this long after that one, and still only once things settle.
            Press::Wait(d) => {
                record = false;
                let left = d.saturating_sub(since);
                if left.is_zero() {
                    self.at = (frame, now);
                } else {
                    self.queue.push_front((idx, Press::Wait(d)));
                    ctx.request_repaint_after(left);
                }
            }
            Press::Events(events) => {
                raw_input.events.extend(events);
                self.at = (frame, now);
                self.now = false;
            }
            // Read as it goes in, not when the script was parsed: an earlier
            // key of the script may have put something on the clipboard.
            Press::Paste => {
                raw_input.events.push(keyscript::paste_event());
                self.at = (frame, now);
                self.now = false;
            }
            Press::PasteText(text) => {
                raw_input.events.push(keyscript::paste_text_event(&text));
                self.at = (frame, now);
                self.now = false;
            }
            p @ (Press::Click { .. } | Press::DClick { .. } | Press::Hover { .. } | Press::Wheel { .. } | Press::Drag { .. }) => {
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
                self.at = (frame, now);
                self.now = false;
            }
            // Written now: the wait above is the one a key gets, so this is
            // the state the key before it left (#230).
            Press::State(name) => {
                let path = self.shot_dir.join(format!("{name}.txt"));
                let text = crate::state_report(app)
                    + &format!("cursor icon: {:?}\nsettled: {}\n", self.cursor_icon, if settled { "yes" } else { "no" });
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
            // Straight to the quit `ui` already handles, past anything that
            // would ask first or take `q` for itself (#236).
            Press::Quit => {
                app.quit = true;
                self.at = (frame, now);
            }
            // Taken in `ui`, which is where a viewport command can be sent.
            Press::Shot(name) => {
                self.shot = Some(Shot { name, sent: false, step: idx, since: now });
                self.at = (frame, now);
            }
            // The window lays out again at the new size before the next step.
            Press::Size { w, h } => {
                ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(w as f32, h as f32)));
                self.queue.push_front((idx, Press::Wait(Duration::from_millis(400))));
                self.at = (frame, now);
            }
            // The same write the settings screen makes: common.toml, applied
            // as the watcher would.
            Press::Theme(name) => {
                app.write_common(vec![ito_common::CommonChange { table: None, key: "theme", value: format!("{name:?}") }]);
                self.at = (frame, now);
            }
            Press::Lang(code) => {
                app.write_common(vec![ito_common::CommonChange { table: None, key: "language", value: format!("{code:?}") }]);
                self.at = (frame, now);
            }
            // Taken before the wait above; `parse` puts a key after every one.
            Press::Now => {}
            // Steps that read the window's tree or the clipboard: kura does not
            // take them. `parse_cli` refuses a script with one, so these are not
            // reached; if one is, the run fails rather than skip it.
            Press::ClickText { .. }
            | Press::DClickText(_)
            | Press::Assert { .. }
            | Press::Clipboard(_)
            | Press::ClipboardHas(_)
            | Press::Is { .. }
            | Press::Tree(_)
            | Press::Announced(_) => {
                return self.fail(app, idx, "kura's --keys does not carry out this step yet".to_owned());
            }
        }
        if record && late {
            self.record_unsettled(idx);
        }
        self.note_progress();
        Outcome::Continue
    }

    /// Where `<Shot:name>` saves (`name@preview` is the preview pane's crop of `name.png`).
    fn shot_path(&self, name: &str) -> PathBuf {
        let file = name.strip_suffix(keyscript::SHOT_PREVIEW).unwrap_or(name);
        self.shot_dir.join(format!("{file}.png"))
    }

    /// `<Shot:name>`: ask for the picture, then save it when it arrives.
    pub(crate) fn take_shot(&mut self, ctx: &egui::Context, app: &mut App) -> Outcome {
        let Some(shot) = &mut self.shot else { return Outcome::Continue };
        if !shot.sent {
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::new(shot.name.clone())));
            shot.sent = true;
            ctx.request_repaint();
            return Outcome::Continue;
        }
        let (name, step) = (shot.name.clone(), shot.step);
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
        match save_shot(&image, crop, app.preview.rect, ctx.content_rect().width(), &self.shot_path(&name)) {
            Ok(_) => {
                self.shot = None;
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
        let ok = self.write_done(&(crate::state_report(app) + &tail));
        self.note_progress();
        if !ok {
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
            let dir = crate::util::test_dir(label);
            std::fs::write(dir.join("a.txt"), "a").unwrap();
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

        fn flag(&self) -> bool {
            self.failed.load(Ordering::SeqCst)
        }
    }

    fn read(path: &Path) -> String {
        std::fs::read_to_string(path).unwrap()
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
        for bad in ["<Assert:foo>", "<Assert:!foo>", "<Click:Name>", "<Clipboard:x>", "<Tree:t>"] {
            assert!(unsupported(&p(bad)).is_some(), "{bad}");
        }
        let unknown = Press::Read { tag: "Bogus".into(), name: "n".into() };
        assert!(unsupported(&unknown).is_some());
    }

    #[test]
    fn a_write_that_fails_ends_the_run_failed() {
        for script in ["<State:a>", "<PaneText:a>", "<PreviewText:a>"] {
            let mut rig = Rig::new("keys-write-fails", true);
            let done = rig.dir.join("done.txt");
            let shot_dir = unwritable(&rig.dir);
            let mut run = rig.run(script, Some(done.clone()), shot_dir.clone());
            assert_eq!(rig.step(&mut run, 0), Outcome::Failed, "{script}");
            let said = read(&done);
            assert!(said.contains("keys: failed\n"), "{script}: {said}");
            assert!(said.contains(&format!("step: {script}\n")), "{script}: {said}");
            assert!(said.contains(&shot_dir.display().to_string()), "{script} names the path: {said}");
            assert!(said.ends_with("left: 1 not pressed\n"), "{script}: {said}");
            assert!(!said.contains("keys: done"), "{script}: {said}");
            assert!(rig.flag(), "{script}");
            assert!(run.queue.is_empty() && run.done.is_none(), "{script}: no more keys, one report");
        }
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

    #[test]
    fn a_step_kura_does_not_carry_out_fails_if_it_gets_this_far() {
        let mut rig = Rig::new("keys-unsupported", true);
        let done = rig.dir.join("done.txt");
        let mut run = rig.run("j", Some(done.clone()), rig.dir.clone());
        run.queue = [(0, Press::Assert { text: "foo".into(), present: true })].into();
        assert_eq!(rig.step(&mut run, 0), Outcome::Failed);
        assert!(read(&done).contains("keys: failed\n"));
        assert!(rig.flag());
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
        assert!(said.ends_with("left: 2 not pressed\n"), "the shot and the key after it: {said}");
        assert!(!rig.dir.join("x.png").exists());
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
            assert!(said.ends_with("left: 1 not pressed\n"), "{how:?}: {said}");
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
        assert!(said.ends_with("keys: unsettled\n"), "{said}");
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

    #[test]
    fn closing_a_finished_unsettled_run_stays_not_a_success() {
        let mut rig = Rig::new("keys-unsettled-closed", false);
        let done = rig.dir.join("done.txt");
        let mut run = rig.run("j", Some(done.clone()), rig.dir.clone());
        while rig.step(&mut run, 6) == Outcome::Continue {}
        assert!(rig.flag());
        // The window closes later: the report is not rewritten, the flag stays.
        assert_eq!(run.ended(&mut rig.screen.app, Ending::Closed), Outcome::Continue);
        assert!(read(&done).ends_with("keys: unsettled\n"));
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
        assert!(said.ends_with("keys: closed\nleft: 1 not pressed\n"), "{said}");
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
        assert!(said.ends_with("quit: yes\nkeys: done\n"), "{said}");
        assert!(!rig.flag());
    }
}
