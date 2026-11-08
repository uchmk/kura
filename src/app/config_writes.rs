//! Which config writes were sent and which have been answered.
//!
//! The settings screen and `--keys` scripts write config.toml, keymap.toml and
//! common.toml from writer threads. A script that reads the state straight
//! after a write would read the old one, so the player waits for every answer;
//! this is the bookkeeping for that. It is pure: every clock reading is passed
//! in.

use super::*;
use std::collections::VecDeque;

/// The two writer threads. config.toml and keymap.toml share the first.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Writer {
    Kura,
    Common,
}

#[derive(Default)]
struct Lane {
    /// The number the next write gets, minus one: the last one sent.
    sent: u64,
    /// The newest answer taken.
    answered: u64,
    /// Sent and not yet answered, oldest first.
    waiting: VecDeque<(u64, Instant, &'static str)>,
}

#[derive(Default)]
pub(crate) struct ConfigWrites {
    kura: Lane,
    common: Lane,
    /// Answers taken so far. A script that sees this move knows the app made
    /// progress.
    answers: u64,
}

impl ConfigWrites {
    fn lane(&self, w: Writer) -> &Lane {
        match w {
            Writer::Kura => &self.kura,
            Writer::Common => &self.common,
        }
    }

    fn lane_mut(&mut self, w: Writer) -> &mut Lane {
        match w {
            Writer::Kura => &mut self.kura,
            Writer::Common => &mut self.common,
        }
    }

    /// The number [`Self::sent`] will give the next write, for the job to carry.
    pub(crate) fn next_seq(&self, w: Writer) -> u64 {
        self.lane(w).sent + 1
    }

    /// A write of `file` went to the writer at `at`. Call it only once the
    /// send succeeded.
    pub(crate) fn sent(&mut self, w: Writer, file: &'static str, at: Instant) -> u64 {
        let lane = self.lane_mut(w);
        lane.sent += 1;
        let seq = lane.sent;
        lane.waiting.push_back((seq, at, file));
        seq
    }

    /// The writer answered `seq`. A kura answer may stand for several jobs it
    /// merged, so any number above the last answer and up to the last one sent
    /// is fine; a common answer is the very next one. Anything else is a
    /// writer that lost or repeated a write.
    pub(crate) fn answered(&mut self, w: Writer, seq: u64) -> Result<(), String> {
        let lane = self.lane_mut(w);
        let (ok, expected) = match w {
            Writer::Kura => (seq > lane.answered && seq <= lane.sent, lane.answered + 1),
            Writer::Common => (seq == lane.answered + 1 && seq <= lane.sent, lane.answered + 1),
        };
        if !ok {
            // With nothing waiting, the kura writer's answer could be about
            // either of its files.
            let file = lane.waiting.front().map_or(
                match w {
                    Writer::Kura => "config.toml or keymap.toml",
                    Writer::Common => "common.toml",
                },
                |(.., file)| file,
            );
            return Err(format!("{file}: answer {seq} came out of order (expected {expected})"));
        }
        lane.answered = seq;
        let before = lane.waiting.len();
        lane.waiting.retain(|(s, ..)| *s > seq);
        if lane.waiting.len() != before {
            self.answers += 1;
        }
        Ok(())
    }

    /// Whether any write is still unanswered.
    pub(crate) fn pending(&self) -> bool {
        !self.kura.waiting.is_empty() || !self.common.waiting.is_empty()
    }

    /// How many answers have been taken; it only goes up.
    #[cfg(test)]
    pub(crate) fn answers(&self) -> u64 {
        self.answers
    }

    /// What is in flight, in the shape the key-script pacer reads. `failed` is
    /// the app's first failed write, if any (only its message is kept, so the
    /// number and file are left empty).
    pub(crate) fn status(&self, failed: Option<&str>) -> ito_common::writes::Status {
        use ito_common::writes::{Failure, Pending, Status};
        Status {
            pending: self.kura.waiting.len() + self.common.waiting.len(),
            oldest: self.oldest().map(|(seq, since, file)| Pending { seq, file: file.to_owned(), since }),
            answers: self.answers,
            failed: failed.map(|why| Failure { seq: 0, file: String::new(), why: why.to_owned() }),
        }
    }

    fn oldest(&self) -> Option<(u64, Instant, &'static str)> {
        [self.kura.waiting.front(), self.common.waiting.front()].into_iter().flatten().min_by_key(|(_, at, _)| *at).copied()
    }
}

impl App {
    /// A config write that did not happen: said at once, and kept as the
    /// first failure so a `--keys` run can end on it.
    pub(crate) fn fail_config_write(&mut self, msg: String) {
        self.error(msg.clone());
        if self.config_write_failed.is_none() {
            self.config_write_failed = Some(msg);
        }
    }

    /// Whether a config write has not been answered yet.
    ///
    /// A writer thread that panics leaves its writes unanswered for good. A
    /// running key script catches that with `CONFIG_WAIT`; outside a script
    /// nothing reports it.
    pub(crate) fn config_writes_pending(&self) -> bool {
        self.config_writes.pending()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use std::time::Duration;

    #[test]
    fn a_kura_answer_is_newer_than_the_last_and_not_newer_than_the_last_sent() {
        let t = Instant::now();
        let mut w = ConfigWrites::default();
        for _ in 0..3 {
            w.sent(Writer::Kura, "config.toml", t);
        }
        // Three jobs merged into one write answer with the last seq.
        assert_eq!(w.answered(Writer::Kura, 3), Ok(()));
        assert!(!w.pending(), "all three dropped");
        assert_eq!(w.answers(), 1);
        // A duplicate, and one that was never sent.
        assert!(w.answered(Writer::Kura, 3).unwrap_err().contains("answer 3 came out of order (expected 4)"));
        assert!(w.answered(Writer::Kura, 9).is_err());
        assert_eq!(w.answers(), 1, "an answer refused counts for nothing");

        let mut w = ConfigWrites::default();
        w.sent(Writer::Kura, "keymap.toml", t);
        w.sent(Writer::Kura, "keymap.toml", t);
        assert_eq!(w.answered(Writer::Kura, 1), Ok(()));
        assert!(w.pending(), "the second is still out");
        assert_eq!(w.answered(Writer::Kura, 2), Ok(()));
        let err = w.answered(Writer::Kura, 1).unwrap_err();
        assert_eq!(err, "config.toml or keymap.toml: answer 1 came out of order (expected 3)");
    }

    #[test]
    fn a_common_answer_is_exactly_the_next_one() {
        let t = Instant::now();
        let mut w = ConfigWrites::default();
        w.sent(Writer::Common, "common.toml", t);
        w.sent(Writer::Common, "common.toml", t);
        assert_eq!(w.answered(Writer::Common, 2).unwrap_err(), "common.toml: answer 2 came out of order (expected 1)");
        assert_eq!(w.answered(Writer::Common, 1), Ok(()));
        assert!(w.answered(Writer::Common, 1).is_err(), "a duplicate");
        assert_eq!(w.answered(Writer::Common, 2), Ok(()));
        assert!(!w.pending());
        assert_eq!(w.answers(), 2);
    }

    #[test]
    fn the_status_names_the_oldest_unanswered_write() {
        let t = Instant::now();
        let mut w = ConfigWrites::default();
        let s = w.status(None);
        assert_eq!((s.pending, s.oldest, s.answers, s.failed), (0, None, 0, None));
        w.sent(Writer::Kura, "config.toml", t);
        w.sent(Writer::Common, "common.toml", t + Duration::from_secs(5));
        let s = w.status(Some("boom"));
        assert_eq!(s.pending, 2);
        let oldest = s.oldest.unwrap();
        assert_eq!((oldest.seq, oldest.file.as_str(), oldest.since), (1, "config.toml", t));
        assert_eq!(s.failed.unwrap().why, "boom");
        assert_eq!(w.answered(Writer::Kura, 1), Ok(()));
        let s = w.status(None);
        assert_eq!((s.pending, s.answers), (1, 1));
        let oldest = s.oldest.unwrap();
        assert_eq!((oldest.file.as_str(), oldest.since), ("common.toml", t + Duration::from_secs(5)));
    }

    fn job(seq: u64, path: &Path, key: &'static str) -> KuraJob {
        let change = KuraChange { table: "ui", key: key.into(), value: Some("1".into()) };
        KuraJob { seq, path: path.to_path_buf(), dirs: Vec::new(), changes: vec![change], keys: Vec::new(), webhooks: None, whole: None }
    }

    #[test]
    fn consecutive_jobs_for_one_file_merge_and_answer_with_the_last_seq() {
        let (a, b) = (Path::new("a.toml"), Path::new("b.toml"));
        let merged = merge_jobs([job(1, a, "x"), job(2, a, "y"), job(3, b, "x"), job(4, a, "x")]);
        assert_eq!(merged.iter().map(|j| j.seq).collect::<Vec<_>>(), [2, 3, 4], "only consecutive same-path jobs merge");
        assert_eq!(merged[0].changes.len(), 2);
    }

    #[test]
    fn merge_jobs_leaves_a_whole_job_alone() {
        let a = Path::new("a.toml");
        let mut reset = job(2, a, "x");
        reset.changes.clear();
        reset.whole = Some(Whole::Reset(Vec::new()));
        let merged = merge_jobs([job(1, a, "x"), reset, job(3, a, "y")]);
        assert_eq!(merged.iter().map(|j| j.seq).collect::<Vec<_>>(), [1, 2, 3], "kept in order, none merged");
        assert!(merged[1].whole.is_some());
    }

    #[test]
    fn the_kura_writer_answers_every_job_in_order_ending_with_the_last_seq() {
        let dir = crate::util::test_dir("kura-writer-seq");
        let path = dir.join("config.toml");
        let (done, answers) = crossbeam_channel::unbounded();
        let jobs = kura_writer(done, egui::Context::default()).expect("the writer started");
        jobs.send(job(1, &path, "x")).unwrap();
        jobs.send(job(2, &path, "y")).unwrap();
        let mut seen = Vec::new();
        while seen.last() != Some(&2) {
            seen.push(answers.recv_timeout(Duration::from_secs(10)).expect("the writer answered").seq);
        }
        assert!(seen == [2] || seen == [1, 2], "one merged answer or two in order: {seen:?}");
    }
}
