//! Which config writes were sent and which have been answered.
//!
//! The settings screen and `--keys` scripts write config.toml, keymap.toml and
//! common.toml from writer threads. A script that reads the state straight
//! after a write would read the old one, so the player waits for every answer;
//! this is the bookkeeping for that. It is pure: every clock reading is passed
//! in.

use super::*;
use std::collections::VecDeque;
use std::time::Duration;

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
    pub(crate) fn answers(&self) -> u64 {
        self.answers
    }

    /// The oldest unanswered write that was sent `limit` or longer before `now`.
    pub(crate) fn overdue(&self, now: Instant, limit: Duration) -> Option<(u64, &'static str)> {
        self.oldest().filter(|(_, at, _)| *at + limit <= now).map(|(seq, _, file)| (seq, file))
    }

    /// The file of the oldest unanswered write.
    pub(crate) fn oldest_file(&self) -> Option<&'static str> {
        self.oldest().map(|(_, _, file)| file)
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
    fn the_oldest_unanswered_write_is_the_one_that_is_overdue() {
        let t = Instant::now();
        let ten = Duration::from_secs(10);
        let mut w = ConfigWrites::default();
        assert_eq!(w.overdue(t + ten, ten), None);
        w.sent(Writer::Kura, "config.toml", t);
        w.sent(Writer::Common, "common.toml", t + Duration::from_secs(5));
        assert_eq!(w.overdue(t + Duration::from_secs(9), ten), None);
        assert_eq!(w.overdue(t + ten, ten), Some((1, "config.toml")));
        assert_eq!(w.answered(Writer::Kura, 1), Ok(()));
        assert_eq!(w.overdue(t + ten, ten), None, "the other has waited 5 s");
        assert_eq!(w.overdue(t + Duration::from_secs(15), ten), Some((1, "common.toml")));
        assert_eq!(w.oldest_file(), Some("common.toml"));
    }

    fn job(seq: u64, path: &Path, key: &'static str) -> KuraJob {
        let change = KuraChange { table: "ui", key: key.into(), value: Some("1".into()) };
        KuraJob { seq, path: path.to_path_buf(), dirs: Vec::new(), changes: vec![change], keys: Vec::new(), webhooks: None }
    }

    #[test]
    fn consecutive_jobs_for_one_file_merge_and_answer_with_the_last_seq() {
        let (a, b) = (Path::new("a.toml"), Path::new("b.toml"));
        let merged = merge_jobs([job(1, a, "x"), job(2, a, "y"), job(3, b, "x"), job(4, a, "x")]);
        assert_eq!(merged.iter().map(|j| j.seq).collect::<Vec<_>>(), [2, 3, 4], "only consecutive same-path jobs merge");
        assert_eq!(merged[0].changes.len(), 2);
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
