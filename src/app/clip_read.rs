//! Reading the clipboard without holding the window.
//!
//! `ito_clipboard::Clipboard::get_text` can block for seconds on X11, so a read
//! kura starts itself (a right-click paste) happens on a thread of its own and
//! is picked up by a later frame. The `--keys` Ctrl+V stays a read on the UI
//! thread on purpose; see `keyscript::paste_event`.

use std::time::{Duration, Instant};

type Answer = Result<Option<String>, String>;

/// How long a paste waits for the clipboard before it gives up. X11 can hold
/// a read for seconds; one that never answers must not keep a prompt, the
/// terminal or a `--keys` step waiting for ever.
const WAIT: Duration = Duration::from_secs(10);

/// A clipboard read on a thread of its own. The answer is taken with
/// [`ClipRead::poll`] on a later frame; the window is woken when it comes.
pub struct ClipRead {
    rx: crossbeam_channel::Receiver<Answer>,
    until: Instant,
}

/// The reader's end of the channel. Dropped when the reader ends, however it
/// ends (a panic included): the channel closes first, then the window is
/// woken, so the frame that wakes sees the answer or the closed channel.
struct Reply {
    tx: Option<crossbeam_channel::Sender<Answer>>,
    ctx: egui::Context,
}

impl Drop for Reply {
    fn drop(&mut self) {
        drop(self.tx.take());
        self.ctx.request_repaint();
    }
}

impl ClipRead {
    /// Read `clipboard` -- picked with `clipboard!()` on the UI thread by the caller, so a test's fake
    /// is the one read -- and wake `ctx` when it answers.
    pub fn start(clipboard: ito_clipboard::Clipboard, ctx: &egui::Context) -> ClipRead {
        Self::spawn(move || clipboard.get_text().map_err(|e| e.to_string()), ctx)
    }

    /// The same with any reader, so a test can hold the answer back.
    pub(crate) fn spawn(read: impl FnOnce() -> Answer + Send + 'static, ctx: &egui::Context) -> ClipRead {
        Self::spawn_for(read, ctx, WAIT)
    }

    /// The same, giving up after `wait`.
    fn spawn_for(read: impl FnOnce() -> Answer + Send + 'static, ctx: &egui::Context, wait: Duration) -> ClipRead {
        let (tx, rx) = crossbeam_channel::bounded(1);
        let reply = Reply { tx: Some(tx.clone()), ctx: ctx.clone() };
        let started = std::thread::Builder::new().name("kura-clip-read".into()).spawn(move || {
            if let Some(tx) = &reply.tx {
                let _ = tx.send(read());
            }
        });
        if let Err(e) = started {
            // The receiver must still answer, so the waiting paste ends.
            let _ = tx.send(Err(format!("could not start the clipboard reader: {e}")));
            ctx.request_repaint();
        }
        // An idle window looks again when the wait runs out.
        ctx.request_repaint_after(wait);
        ClipRead { rx, until: Instant::now() + wait }
    }

    /// The answer once it has come; `None` while the read is still going.
    /// A reader that died without answering, or one still reading when the
    /// wait has run out, is an error, not a wait that never ends.
    pub fn poll(&self) -> Option<Answer> {
        match self.rx.try_recv() {
            Ok(v) => Some(v),
            Err(crossbeam_channel::TryRecvError::Empty) if Instant::now() < self.until => None,
            Err(crossbeam_channel::TryRecvError::Empty) => Some(Err("the clipboard did not answer".into())),
            Err(crossbeam_channel::TryRecvError::Disconnected) => Some(Err("the clipboard reader stopped".into())),
        }
    }
}

/// A right-click paste into a one-line field, waiting for the clipboard.
pub struct FieldPaste {
    pub read: ClipRead,
    /// The field's text at the click, and where in it (in chars) the paste goes:
    /// at the click, or over the selection the click fell on.
    pub before: String,
    pub at: std::ops::Range<usize>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wait(read: &ClipRead) -> Option<Answer> {
        let end = Instant::now() + Duration::from_secs(10);
        while Instant::now() < end {
            if let Some(a) = read.poll() {
                return Some(a);
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        None
    }

    #[test]
    fn clip_read_waits_for_a_held_reader_then_answers() {
        let (go, held) = std::sync::mpsc::channel::<()>();
        let read = ClipRead::spawn(
            move || {
                held.recv().ok();
                Ok(Some("late".into()))
            },
            &egui::Context::default(),
        );
        assert!(read.poll().is_none(), "still going while the reader is held");
        go.send(()).unwrap();
        assert_eq!(wait(&read), Some(Ok(Some("late".to_owned()))));
    }

    #[test]
    fn clip_read_of_a_reader_that_panics_is_an_error() {
        let read = ClipRead::spawn(|| panic!("reader died"), &egui::Context::default());
        assert!(matches!(wait(&read), Some(Err(_))));
    }

    #[test]
    fn clip_read_of_a_reader_that_never_answers_gives_up() {
        let (go, held) = std::sync::mpsc::channel::<()>();
        let read = ClipRead::spawn_for(
            move || {
                held.recv().ok();
                Ok(Some("too late".into()))
            },
            &egui::Context::default(),
            Duration::from_millis(50),
        );
        assert!(read.poll().is_none(), "still within the wait");
        assert_eq!(wait(&read), Some(Err("the clipboard did not answer".to_owned())));
        go.send(()).unwrap();
    }

    #[test]
    fn clip_read_start_reads_the_handle_it_was_given() {
        ito_clipboard::Fake::this_thread().put(Some("from the fake"));
        let read = ClipRead::start(ito_clipboard::clipboard!(), &egui::Context::default());
        assert_eq!(wait(&read), Some(Ok(Some("from the fake".to_owned()))));
    }
}
