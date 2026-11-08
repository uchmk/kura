//! The ledger of config writes: which were sent and which have been answered.
//!
//! The settings screen and `--keys` scripts write config.toml, keymap.toml and
//! common.toml from writer threads. A script that reads the state straight
//! after a write would read the old one, so the player waits for every answer;
//! `ito_common::writes::Ledger` keeps that bookkeeping.

use super::*;

impl App {
    /// A config write that did not happen: said at once, and kept as the
    /// first failure so a `--keys` run can end on it.
    pub(crate) fn fail_config_write(&mut self, msg: String) {
        self.error(msg.clone());
        self.config_writes.sent("config").failed(msg);
    }

    /// Whether a config write has not been answered yet.
    ///
    /// A writer thread that panics drops its job's tickets, which the ledger
    /// records as a failure, so a running key script ends at once.
    pub(crate) fn config_writes_pending(&self) -> bool {
        self.config_writes.status().pending > 0
    }

    /// The first config write that failed, as the ledger recorded it.
    pub(crate) fn config_write_failed(&self) -> Option<String> {
        self.config_writes.status().failed.map(|f| f.why)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use std::time::Duration;

    fn job(path: &Path, key: &'static str, tickets: Vec<ito_common::writes::Ticket>) -> KuraJob {
        let change = KuraChange { table: "ui", key: key.into(), value: Some("1".into()) };
        KuraJob {
            tickets,
            path: path.to_path_buf(),
            dirs: Vec::new(),
            changes: vec![change],
            keys: Vec::new(),
            webhooks: None,
            whole: None,
        }
    }

    #[test]
    fn a_merged_job_carries_every_ticket() {
        let ledger = ito_common::writes::Ledger::new();
        let (a, b) = (Path::new("a.toml"), Path::new("b.toml"));
        let t = |n: u64| {
            let t = ledger.sent("x");
            assert_eq!(t.seq(), Some(n));
            vec![t]
        };
        let merged = merge_jobs([job(a, "x", t(1)), job(a, "y", t(2)), job(b, "x", t(3)), job(a, "x", t(4))]);
        let seqs = |j: &KuraJob| j.tickets.iter().map(|t| t.seq().unwrap()).collect::<Vec<_>>();
        assert_eq!(merged.iter().map(seqs).collect::<Vec<_>>(), [vec![1, 2], vec![3], vec![4]], "only consecutive same-path jobs merge");
        assert_eq!(merged[0].changes.len(), 2);
        assert_eq!(ledger.status().pending, 4, "no ticket was dropped by the merge");
        for j in merged {
            for t in j.tickets {
                t.applied();
            }
        }
        let s = ledger.status();
        assert_eq!((s.pending, s.answers, s.failed), (0, 4, None));
    }

    #[test]
    fn merge_jobs_leaves_a_whole_job_alone() {
        let a = Path::new("a.toml");
        let mut reset = job(a, "x", Vec::new());
        reset.changes.clear();
        reset.whole = Some(Whole::Reset(Vec::new()));
        let merged = merge_jobs([job(a, "x", Vec::new()), reset, job(a, "y", Vec::new())]);
        assert_eq!(merged.len(), 3, "kept in order, none merged");
        assert!(merged[1].whole.is_some());
    }

    #[test]
    fn a_writer_that_drops_a_job_is_a_failure_and_not_a_pending_write() {
        let ledger = ito_common::writes::Ledger::new();
        let j = job(Path::new("a.toml"), "x", vec![ledger.sent("config.toml")]);
        assert_eq!(ledger.status().pending, 1);
        drop(j);
        let s = ledger.status();
        assert_eq!(s.pending, 0);
        assert!(s.failed.unwrap().why.contains("dropped without an answer"));
    }

    #[test]
    fn the_kura_writer_answers_every_job_with_its_tickets() {
        let dir = crate::util::test_dir("kura-writer-seq");
        let path = dir.join("config.toml");
        let ledger = ito_common::writes::Ledger::new();
        let (done, answers) = crossbeam_channel::unbounded();
        let jobs = kura_writer(done, egui::Context::default()).expect("the writer started");
        jobs.send(job(&path, "x", vec![ledger.sent("config.toml")])).unwrap();
        jobs.send(job(&path, "y", vec![ledger.sent("config.toml")])).unwrap();
        let mut seen = Vec::new();
        while seen.last() != Some(&2) {
            let w = answers.recv_timeout(Duration::from_secs(10)).expect("the writer answered");
            for t in w.tickets {
                seen.push(t.seq().unwrap());
                t.applied();
            }
        }
        assert_eq!(seen, [1, 2], "one merged answer or two in order");
        let s = ledger.status();
        assert_eq!((s.pending, s.failed), (0, None));
    }
}
