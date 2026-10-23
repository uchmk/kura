//! The task list: one row per file operation, with its progress and the change records kura makes.

use super::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TaskState {
    /// Waiting its turn: one job runs at a time.
    Queued,
    Running,
    Paused,
    Done,
    Failed,
    Cancelled,
}

impl TaskState {
    pub fn label(&self) -> &'static str {
        match self {
            TaskState::Queued => "queued",
            TaskState::Running => "running",
            TaskState::Paused => "paused",
            TaskState::Done => "done",
            TaskState::Failed => "failed",
            TaskState::Cancelled => "cancelled",
        }
    }

    /// Whether the job still has somewhere to go, and so is worth a key.
    pub fn is_live(&self) -> bool {
        matches!(self, TaskState::Queued | TaskState::Running | TaskState::Paused)
    }
}

pub struct Task {
    pub id: u64,
    pub kind: OpKind,
    pub label: String,
    pub files: u64,
    pub bytes: u64,
    pub files_done: u64,
    pub bytes_done: u64,
    pub current: String,
    pub state: TaskState,
    pub errors: Vec<String>,
    pub finished: Option<Instant>,
    /// Bytes per second, smoothed. Zero until there is enough to say.
    pub(super) speed: f64,
    /// The last point the speed was measured from.
    pub(super) sampled_at: Instant,
    pub(super) sampled_bytes: u64,
    /// When the job started running, for the average pace over its whole
    /// run (`eta`).
    pub(super) started_at: Instant,
}

#[cfg(test)]
impl Task {
    /// A job as the tasks list holds it, for tests of what reads the list.
    pub fn for_test(kind: OpKind, label: &str, state: TaskState, files: u64, files_done: u64) -> Self {
        Self {
            id: 1,
            kind,
            label: label.into(),
            files,
            bytes: 0,
            files_done,
            bytes_done: 0,
            current: String::new(),
            state,
            errors: Vec::new(),
            finished: None,
            speed: 0.0,
            sampled_at: Instant::now(),
            sampled_bytes: 0,
            started_at: Instant::now(),
        }
    }
}

/// A job that ran this long is one to be told about when it ends.
pub(super) const LONG_OP: Duration = Duration::from_secs(10);

/// The system toast for a finished job: only for one that ran [`LONG_OP`] or
/// more, ended on its own or with an error (a cancel is the reader's own act),
/// while the window was behind another (`behind`).
pub(super) fn long_op_notice(t: &Task, behind: bool) -> Option<(String, String)> {
    if !behind || t.state == TaskState::Cancelled || t.started_at.elapsed() < LONG_OP {
        return None;
    }
    let (title, tail) = match t.errors.first() {
        Some(e) => (format!("{} failed", t.kind.verb()), e.clone()),
        None => (format!("{} finished", t.kind.verb()), String::new()),
    };
    let body = if tail.is_empty() {
        t.label.clone()
    } else {
        format!(
            "{}
{tail}",
            t.label
        )
    };
    Some((title, body))
}

impl Task {
    /// The row's first line. The label already starts with the verb, so it is
    /// not put in front a second time (`Trash  Trash 5 item(s)`).
    pub fn headline(&self) -> String {
        format!("{}  [{}]", self.label, self.state.label())
    }

    /// The row's second line. A job that counts no bytes -- the trash, a
    /// delete -- shows files only: `0 B / 0 B` read as a row of empty files.
    pub fn detail(&self) -> String {
        let mut out = format!("{}/{} files", self.files_done, self.files);
        if self.bytes > 0 {
            out.push_str(&format!(" · {} / {}", crate::util::human_size(self.bytes_done), crate::util::human_size(self.bytes)));
        }
        if let Some(s) = self.speed() {
            out.push_str(&format!(" · {}/s", crate::util::human_size(s)));
        }
        if let Some(eta) = self.eta() {
            out.push_str(&format!(" · {} left", crate::util::fmt_duration(eta)));
        }
        out
    }

    /// The work the job has, in bytes with each file counted as a fixed
    /// amount more ([`FILE_COST`]): a folder of small files is mostly the
    /// opening and closing, and bytes alone read `97% … 1s` with ten seconds
    /// to go (#241). `(done, total)`.
    pub(super) fn work(&self) -> (u64, u64) {
        (self.bytes_done + self.files_done * FILE_COST, self.bytes + self.files * FILE_COST)
    }

    pub fn fraction(&self) -> f32 {
        let (done, total) = self.work();
        if total > 0 {
            (done as f32 / total as f32).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }

    /// Fold a new byte count into the running speed. Reports arrive about 20
    /// times a second, which is far too often to measure over: a quarter of a
    /// second of work is the shortest window that reads steadily.
    pub(super) fn sample(&mut self, bytes_done: u64) {
        let dt = self.sampled_at.elapsed().as_secs_f64();
        if dt < 0.25 {
            return;
        }
        let now = bytes_done.saturating_sub(self.sampled_bytes) as f64 / dt;
        // Smoothed, so a run of small files does not make the number jump
        // about faster than it can be read.
        self.speed = match self.speed {
            0.0 => now,
            prev => prev * 0.7 + now * 0.3,
        };
        self.sampled_at = Instant::now();
        self.sampled_bytes = bytes_done;
    }

    /// Bytes per second, or nothing while the job is too young or too still
    /// to have a useful answer.
    pub fn speed(&self) -> Option<u64> {
        (self.state == TaskState::Running && self.speed >= 1.0).then_some(self.speed as u64)
    }

    /// How long the rest should take: the slower of the current byte speed
    /// and the pace the whole run has kept in weighted work, so a job that
    /// is mostly small files is not told it is nearly done. Only bytes can
    /// answer this: a file count says nothing about how big the files are.
    pub fn eta(&self) -> Option<Duration> {
        let speed = self.speed()? as f64;
        let left = self.bytes.checked_sub(self.bytes_done)?;
        if left == 0 || self.bytes == 0 {
            return None;
        }
        let by_speed = left as f64 / speed;
        let (done, total) = self.work();
        let elapsed = self.started_at.elapsed().as_secs_f64();
        // Until the pace is known the bytes alone say "nearly done" for a pile
        // of small files, so say nothing yet.
        if done == 0 || elapsed < 2.0 {
            return None;
        }
        let by_pace = elapsed * (total - done) as f64 / done as f64;
        // Round up: 0.97 s left is "1s", and "0s" would claim the job is over.
        let secs = by_speed.max(by_pace).ceil();
        (secs.is_finite() && secs < 86_400.0).then(|| Duration::from_secs_f64(secs))
    }
}

/// The one line a config warning gets in the corner (Q84). A `toml` parse
/// error is five lines and covered half the window and the top of the preview
/// on every start (#174), while all of it is already in `~` and `kura env`.
/// The first line names the file and the place, which is what the toast is
/// for; the rest is a key away.
pub fn config_toast(warnings: &[String]) -> Option<String> {
    config_toast_at(warnings, ito_common::home_dir().as_deref())
}

/// [`config_toast`] with the home folder given. The first line starts with the
/// file's path, and under a long one the toast wrapped to three lines and
/// covered the preview's top (#295); `~` for home keeps it to one.
pub(super) fn config_toast_at(warnings: &[String], home: Option<&Path>) -> Option<String> {
    let first = warnings.first()?;
    let line = crate::bugreport::without_home(first.lines().next().unwrap_or_default(), home);
    let cut = first.lines().nth(1).is_some();
    let tail = match (cut, warnings.len() - 1) {
        (false, 0) => String::new(),
        (true, 0) => " — the rest in `~`".to_owned(),
        (false, more) => format!(" (+{more} more, see `~`)"),
        (true, more) => format!(" — the rest and {more} more in `~`"),
    };
    Some(format!("Config: {line}{tail}"))
}

/// Whether `path` is `above` or inside it. `Path::starts_with` compares
/// components, and on Windows a bare `\\host` is not a prefix of
/// `\\host\share`, so the parent column of a mistyped host was not seen as
/// belonging to the jump that failed (#270).
pub(super) fn is_within(path: &Path, above: &Path) -> bool {
    if path.starts_with(above) {
        return true;
    }
    let (p, a) = (path.to_string_lossy().to_lowercase(), above.to_string_lossy().to_lowercase());
    let a = a.trim_end_matches(['/', '\\']);
    p.strip_prefix(a).is_some_and(|rest| rest.is_empty() || rest.starts_with(['/', '\\']))
}

/// What opening, creating and closing one file is worth, as bytes of copying.
pub(super) const FILE_COST: u64 = 64 * 1024;

/// How loud a toast is, which is to say what colour it takes.
///
/// The distinction that matters is [`Level::Warn`] against [`Level::Error`]:
/// one says the program could not do the thing, the other says it went ahead
/// and there is something you may want to know. Painting both red leaves the
/// reader no way to tell a broken startup from a stale line in their keymap.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Level {
    Info,
    Warn,
    Error,
}

/// How many notices [`App::notices`] keeps.
pub(super) const NOTICES: usize = 100;

/// How many keys [`App::recent_keys`] keeps.
pub(super) const RECENT_KEYS: usize = 20;

pub(super) fn keymap_render(k: &Key) -> String {
    crate::config::keys::render_seq(std::slice::from_ref(k))
}

/// How many toasts [`App::toast_log`] keeps.
pub const TOAST_LOG: usize = 16;

/// A config.toml the settings screen's "Open config.toml" makes when there
/// is none yet: only comments, so it changes nothing until a line is added.
pub(super) const OWN_FILE_START: &str = "# kura's own settings, beside yazi's yazi.toml, keymap.toml and theme.toml.
# Every key, with what it does, is in the README: \"config.toml\".
";

/// A row of config.toml the settings screen changed: its table, its key, and
/// the value written as TOML, or `None` to take the key out.
#[derive(Debug, Clone, PartialEq)]
pub struct KuraChange {
    /// May be dotted (`term.env`).
    pub table: &'static str,
    /// Borrowed for the settings' own rows, owned for a name the person typed
    /// (an environment variable, written as a quoted TOML key).
    pub key: std::borrow::Cow<'static, str>,
    pub value: Option<String>,
}

/// One write for the config.toml writer: the file, the folders to read the
/// config from afterwards, and the changes in the order they were made.
pub(super) struct KuraJob {
    pub(super) path: PathBuf,
    pub(super) dirs: Vec<PathBuf>,
    pub(super) changes: Vec<KuraChange>,
    /// For a keymap.toml (the Keys page), the edits to make to it instead.
    pub(super) keys: Vec<ito_keymap::Edit>,
    /// Every `[[notify.webhooks]]` to write after the changes, in place of the
    /// ones there and of the old one address.
    pub(super) webhooks: Option<Vec<ito_notify::webhook::Place>>,
}

/// What a write of config.toml came to: the rows written, as a toast says
/// them, and the config read back -- or what stopped it.
pub(crate) struct KuraWritten {
    pub(super) said: String,
    pub(super) read: Result<crate::config::Fresh, String>,
    /// Whether the file changed: an edit that leaves it as it was starts no
    /// watcher event to keep quiet, and says so instead of "Saved".
    pub(super) wrote: bool,
}

/// The writer's thread: each job, and every job queued behind it, as one
/// write of the newest values, and the config read back after it.
pub(super) fn kura_writer(done: Sender<KuraWritten>, ctx: egui::Context) -> Option<Sender<KuraJob>> {
    let (tx, rx) = crossbeam_channel::unbounded::<KuraJob>();
    let spawned = std::thread::Builder::new().name("kura-toml".into()).spawn(move || {
        while let Ok(first) = rx.recv() {
            let mut jobs: Vec<KuraJob> = vec![first];
            for j in rx.try_iter() {
                match jobs.last_mut() {
                    Some(m) if m.path == j.path => {
                        m.changes.extend(j.changes);
                        m.keys.extend(j.keys);
                        m.webhooks = j.webhooks.or(m.webhooks.take());
                        m.dirs = j.dirs;
                    }
                    _ => jobs.push(j),
                }
            }
            for j in jobs {
                let written = if j.keys.is_empty() { write_kura_toml(j) } else { write_keymap_toml(j) };
                if done.send(written).is_err() {
                    return;
                }
                ctx.request_repaint();
            }
        }
    });
    spawned.ok().map(|_| tx)
}

/// Rename each of `files` that exists to `<name>.bak-<seconds>`; how many moved.
pub(super) fn move_aside(files: &[PathBuf]) -> std::io::Result<usize> {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
    let mut moved = 0;
    for f in files.iter().filter(|f| f.exists()) {
        let mut aside = f.clone().into_os_string();
        aside.push(format!(".bak-{secs}"));
        std::fs::rename(f, aside)?;
        moved += 1;
    }
    Ok(moved)
}

/// Change the rows in config.toml and keep the rest of it -- comments, order,
/// blank lines -- as it was; a file that is not there yet is started with
/// [`OWN_FILE_START`]. A file that does not read as TOML is left alone.
pub(super) fn write_kura_toml(job: KuraJob) -> KuraWritten {
    let file = job.path.file_name().map_or_else(|| crate::config::OWN_FILE.into(), |n| n.to_string_lossy().into_owned());
    let said = job
        .changes
        .iter()
        .map(|c| match &c.value {
            Some(v) => format!("{} = {v}", c.key),
            None => format!("{} removed", c.key),
        })
        .chain(job.webhooks.iter().map(|w| format!("{} webhook(s)", w.len())))
        .collect::<Vec<_>>()
        .join(", ");
    let said = format!("Saved {file}: {said}");
    let at = |e: String| format!("{}: {e}", job.path.display());
    let read = (|| {
        let text = match std::fs::read_to_string(&job.path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => OWN_FILE_START.to_owned(),
            Err(e) => return Err(at(e.to_string())),
        };
        // A file of comments only keeps them at the top: toml_edit would
        // hold them as the document's tail and put the new table above.
        let only_comments = text.lines().all(|l| l.trim().is_empty() || l.trim_start().starts_with('#'));
        let (head, body) = match only_comments {
            true if text.trim().is_empty() => (String::new(), String::new()),
            true => (format!("{}\n\n", text.trim_end()), String::new()),
            false => (String::new(), text),
        };
        let new = job
            .changes
            .iter()
            .try_fold(body, |t, c| match &c.value {
                Some(v) => ito_common::set_key(&t, Some(c.table), &c.key, v),
                None => ito_common::remove_key(&t, Some(c.table), &c.key),
            })
            .map_err(at)?;
        let new = match &job.webhooks {
            Some(places) => ito_prefs::write_webhooks(&new, places, true).map_err(at)?,
            None => new,
        };
        let new = head + &new;
        ito_common::write_atomic(&job.path, new)?;
        Ok(Config::read_fresh(&job.dirs))
    })();
    KuraWritten { said, read, wrote: true }
}

// The keymap.toml a test's Keys page writes: none unless the test names one,
// so no test can reach the real config folder.
#[cfg(test)]
thread_local! {
    pub(crate) static TEST_KEYMAP: std::cell::RefCell<Option<PathBuf>> = const { std::cell::RefCell::new(None) };
}

/// Make the Keys page's edits to a keymap.toml, keeping the rest of it as it
/// was, and read the config back. Only what differs from the defaults is
/// written: with nothing left to write, no file is started.
pub(super) fn write_keymap_toml(job: KuraJob) -> KuraWritten {
    let said = job
        .keys
        .iter()
        .map(|e| match e {
            ito_keymap::Edit::Add { on, run, .. } => format!("{} runs {run}", crate::config::keys::display_seq(on)),
            ito_keymap::Edit::Remove { on, run, .. } => format!("{} no longer runs {run}", crate::config::keys::display_seq(on)),
            ito_keymap::Edit::Reset { run, .. } => format!("{run} back to its defaults"),
        })
        .collect::<Vec<_>>()
        .join(", ");
    let at = |e: String| format!("{}: {e}", job.path.display());
    let mut wrote = false;
    let read = (|| {
        let text = match std::fs::read_to_string(&job.path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(e) => return Err(at(e.to_string())),
        };
        let new = ito_keymap::edit(&text, &crate::config::keymap::SPEC, &job.keys, "noop").map_err(at)?;
        if new != text {
            if let Some(dir) = job.path.parent() {
                std::fs::create_dir_all(dir).map_err(|e| at(e.to_string()))?;
            }
            ito_common::write_atomic(&job.path, new)?;
            wrote = true;
        }
        Ok(Config::read_fresh(&job.dirs))
    })();
    let said = if wrote { format!("Saved keymap.toml: {said}") } else { format!("keymap.toml already says so: {said}") };
    KuraWritten { said, read, wrote }
}

pub struct Toast {
    pub text: String,
    pub level: Level,
    pub at: Instant,
    /// How many times this same line has been raised. A held-down key can
    /// produce five identical warnings, and five is the whole toast area, so
    /// one repeated message would hide every other for six seconds — including
    /// the header summary underneath it, which is where the reason usually is.
    pub count: u32,
}
