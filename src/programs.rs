//! Whether the programs the openers name are installed, answered without
//! touching the disk on the UI thread.
//!
//! Looking a program up walks every `PATH` folder (and, for `start`, the App
//! Paths in the registry): about 40 ms for one missing name on a PC with a long
//! `PATH`. The "Open with" list and `<Enter>` ask about every opener, so the
//! asking is done by one worker; the UI reads the answers it has so far.

use crate::exec::{wanted, Look};
use crossbeam_channel::Sender;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

/// How a program is looked up; `Look::find` outside the tests.
pub type Finder = Arc<dyn Fn(&str, Look) -> bool + Send + Sync>;

type Key = (String, Look);

#[derive(Default)]
struct State {
    /// The last answer for a program: `true` when it is installed.
    known: HashMap<Key, bool>,
    /// Asked for, and the worker has not started on it.
    queued: HashSet<Key>,
    /// The worker is looking it up now. Asked for again meanwhile, it is queued again.
    in_flight: HashSet<Key>,
}

fn locked(state: &Mutex<State>) -> MutexGuard<'_, State> {
    state.lock().unwrap_or_else(PoisonError::into_inner)
}

pub struct Programs {
    state: Arc<Mutex<State>>,
    /// `None`: nothing is looked up, and every program counts as installed.
    tx: Option<Sender<Key>>,
}

impl Programs {
    /// The real lookup. In the tests, one that answers nothing, so an `App`
    /// neither walks the real `PATH` nor depends on this machine's programs.
    pub fn new(ctx: egui::Context) -> Self {
        #[cfg(test)]
        {
            let _ = ctx;
            Self { state: Arc::default(), tx: None }
        }
        #[cfg(not(test))]
        Self::with_finder(ctx, Arc::new(|name, look| look.find(name)))
    }

    pub fn with_finder(ctx: egui::Context, finder: Finder) -> Self {
        let state = Arc::new(Mutex::new(State::default()));
        let (tx, rx) = crossbeam_channel::unbounded::<Key>();
        let shared = state.clone();
        // Ends when `Programs` is dropped: the channel closes.
        let spawned = std::thread::Builder::new().name("kura-programs".into()).spawn(move || {
            while let Ok(key) = rx.recv() {
                {
                    let mut st = locked(&shared);
                    st.queued.remove(&key);
                    st.in_flight.insert(key.clone());
                }
                // A lookup that panics counts as installed, and is no longer in flight:
                // `pending()` staying true would hold `settled()` (and `--keys`) forever.
                let found = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| finder(&key.0, key.1))).unwrap_or(true);
                {
                    let mut st = locked(&shared);
                    st.in_flight.remove(&key);
                    st.known.insert(key, found);
                }
                // Every answer, changed or not: `settled()` is looked at again.
                ctx.request_repaint();
            }
        });
        // No thread, no answers: every program counts as installed.
        Self { state, tx: spawned.ok().map(|_| tx) }
    }

    /// Ask about the program of every line. One already queued is not queued
    /// twice; one already known, or being looked up right now, is asked again,
    /// so an install or an uninstall since is seen.
    pub fn check<'a>(&self, lines: impl IntoIterator<Item = &'a str>) {
        let Some(tx) = &self.tx else { return };
        for line in lines {
            let Some(key) = wanted(line) else { continue };
            if locked(&self.state).queued.insert(key.clone()) && tx.send(key.clone()).is_err() {
                locked(&self.state).queued.remove(&key);
            }
        }
    }

    /// The program `cmdline` needs, when it is known not to be installed. From
    /// the answers so far: one not answered yet counts as installed, and the
    /// launch's own failure names a missing program anyway.
    pub fn missing(&self, cmdline: &str) -> Option<String> {
        let key = wanted(cmdline)?;
        let installed = locked(&self.state).known.get(&key).copied().unwrap_or(true);
        (!installed).then_some(key.0)
    }

    /// Something is queued or being looked up.
    pub fn pending(&self) -> bool {
        let st = locked(&self.state);
        !st.queued.is_empty() || !st.in_flight.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossbeam_channel::{unbounded, Receiver};
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::time::{Duration, Instant};

    fn wait_settled(p: &Programs) {
        let end = Instant::now() + Duration::from_secs(10);
        while p.pending() {
            assert!(Instant::now() < end, "the worker never answered");
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    /// A finder that tells `started` when it is called and then waits for a
    /// `()` on the returned sender, so the tests say exactly when an answer comes.
    fn held(answer: impl Fn(&str) -> bool + Send + Sync + 'static) -> (Finder, Receiver<String>, Sender<()>) {
        let (started_tx, started) = unbounded::<String>();
        let (release_tx, release) = unbounded::<()>();
        let finder: Finder = Arc::new(move |name, _| {
            let _ = started_tx.send(name.to_owned());
            let _ = release.recv();
            answer(name)
        });
        (finder, started, release_tx)
    }

    #[test]
    fn an_answer_comes_from_the_worker_and_unknown_counts_as_installed() {
        let (finder, started, release) = held(|name| name == "code");
        let p = Programs::with_finder(egui::Context::default(), finder);
        p.check(["code -g %s", "no-such-tool %s"]);
        assert!(p.pending());
        assert_eq!(p.missing("no-such-tool %s"), None, "not answered yet");
        started.recv().unwrap();
        release.send(()).unwrap();
        started.recv().unwrap();
        release.send(()).unwrap();
        wait_settled(&p);
        assert_eq!(p.missing("no-such-tool %s").as_deref(), Some("no-such-tool"));
        assert_eq!(p.missing("code -g %s"), None);
        assert_eq!(p.missing("echo %s"), None, "nothing to look for");
    }

    #[test]
    fn a_second_check_asks_again() {
        let installed = Arc::new(AtomicBool::new(false));
        let i = installed.clone();
        let finder: Finder = Arc::new(move |_, _| i.load(Ordering::SeqCst));
        let p = Programs::with_finder(egui::Context::default(), finder);
        p.check(["tool %s"]);
        wait_settled(&p);
        assert_eq!(p.missing("tool %s").as_deref(), Some("tool"));
        installed.store(true, Ordering::SeqCst);
        assert_eq!(p.missing("tool %s").as_deref(), Some("tool"), "until it is asked again");
        p.check(["tool %s"]);
        wait_settled(&p);
        assert_eq!(p.missing("tool %s"), None);
    }

    #[test]
    fn a_lookup_that_panics_counts_as_installed_and_the_worker_goes_on() {
        let finder: Finder = Arc::new(|name, _| {
            assert_ne!(name, "boom", "a finder that panics");
            false
        });
        let p = Programs::with_finder(egui::Context::default(), finder);
        p.check(["boom %s", "tool %s"]);
        wait_settled(&p);
        assert_eq!(p.missing("boom %s"), None);
        assert_eq!(p.missing("tool %s").as_deref(), Some("tool"));
    }

    #[test]
    fn a_program_queued_is_not_queued_twice_but_one_in_flight_is() {
        let calls = Arc::new(AtomicUsize::new(0));
        let c = calls.clone();
        let (inner, started, release) = held(|_| true);
        let finder: Finder = Arc::new(move |n, l| {
            c.fetch_add(1, Ordering::SeqCst);
            inner(n, l)
        });
        let p = Programs::with_finder(egui::Context::default(), finder);
        p.check(["slow"]);
        assert_eq!(started.recv().unwrap(), "slow");
        // The worker is busy with `slow`: `tool` waits in the queue, once.
        p.check(["tool", "tool --x", "tool"]);
        // `slow` is in flight: asked again, it is queued again.
        p.check(["slow"]);
        for _ in 0..3 {
            release.send(()).unwrap();
        }
        wait_settled(&p);
        assert_eq!(calls.load(Ordering::SeqCst), 3, "slow, tool, slow");
    }

    #[test]
    fn the_test_resolver_answers_nothing() {
        let p = Programs::new(egui::Context::default());
        p.check(["no-such-tool %s"]);
        assert!(!p.pending());
        assert_eq!(p.missing("no-such-tool %s"), None);
    }
}
