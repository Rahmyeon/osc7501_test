//! Terminal-owned OSC 7501 records. Validated input comes from the parser;
//! GUI presentation and client acknowledgement are independent of this store.

use std::collections::{HashMap, VecDeque};
use wezterm_escape_parser::program_status::{Report, State};

pub const MAX_RECORDS: usize = 256;

/// An owned presentation snapshot. `app` preserves the report's explicit value;
/// `effective_app` resolves inheritance without changing the stored report.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ProgramStatusRecord {
    pub id: String,
    pub state: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub msg: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effective_app: Option<String>,
}

#[derive(Debug, Default)]
pub struct ProgramStatusStore {
    records: HashMap<String, Report>,
    // Oldest update first. Updating an identical report still refreshes recency.
    recency: VecDeque<String>,
    // The child waiter may signal exit before buffered PTY output is applied.
    // Late finished reports are useful, but transient reports must not revive
    // work after the attached process has already exited.
    attached_process_exited: bool,
}

impl ProgramStatusStore {
    /// Returns whether stored report content changed, separately from recency.
    pub fn apply(&mut self, report: Report) -> bool {
        if report.state == State::Clear {
            return self.clear(&report.id);
        }
        if self.attached_process_exited && matches!(report.state, State::Working | State::Blocked) {
            // Buffered output still replaces an earlier result for this id.
            // Apply the replacement and exit cleanup together, without making
            // the transient state visible or clearing finished descendants.
            self.recency.retain(|id| id != &report.id);
            return self.records.remove(&report.id).is_some();
        }
        let changed = self.records.get(&report.id) != Some(&report);
        self.recency.retain(|id| id != &report.id);
        if !self.records.contains_key(&report.id) && self.records.len() == MAX_RECORDS {
            if let Some(oldest) = self.recency.pop_front() {
                self.records.remove(&oldest);
            }
        }
        self.recency.push_back(report.id.clone());
        self.records.insert(report.id.clone(), report);
        changed
    }

    /// Clear this exact id and its descendants, respecting path boundaries.
    pub fn clear(&mut self, id: &str) -> bool {
        let before = self.records.len();
        self.records.retain(|key, _| !is_at_or_below(key, id));
        self.recency.retain(|key| !is_at_or_below(key, id));
        before != self.records.len()
    }

    /// Called for a new shell prompt or exit of the attached process.
    /// Idle records are retained, as allowed by the specification.
    pub fn finish_command(&mut self) -> bool {
        let before = self.records.len();
        self.records
            .retain(|_, record| !matches!(record.state, State::Working | State::Blocked));
        let records = &self.records;
        self.recency.retain(|id| records.contains_key(id));
        before != self.records.len()
    }

    pub fn process_exited(&mut self) -> bool {
        self.attached_process_exited = true;
        self.finish_command()
    }

    /// Raw records in stable id order. Missing app values remain missing here;
    /// consumers can resolve inheritance with `effective_app`.
    pub fn snapshot(&self) -> Vec<Report> {
        let mut records: Vec<_> = self.records.values().cloned().collect();
        records.sort_by(|a, b| a.id.cmp(&b.id));
        records
    }

    pub fn presentation_snapshot(&self) -> Vec<ProgramStatusRecord> {
        use wezterm_escape_parser::program_status::Kind;
        self.snapshot()
            .into_iter()
            .map(|record| ProgramStatusRecord {
                effective_app: self.effective_app(&record.id).map(str::to_owned),
                id: record.id,
                state: match record.state {
                    State::Idle => "idle",
                    State::Working => "working",
                    State::Done => "done",
                    State::Blocked => "blocked",
                    State::Error => "error",
                    State::Clear => unreachable!("clear is never stored"),
                }
                .to_owned(),
                kind: record.kind.map(|kind| {
                    match kind {
                        Kind::Permission => "permission",
                        Kind::Question => "question",
                        Kind::Auth => "auth",
                    }
                    .to_owned()
                }),
                progress: record.progress,
                app: record.app,
                title: record.title,
                msg: record.message,
            })
            .collect()
    }

    /// Resolve app dynamically so parent replacement and clearing take effect.
    pub fn effective_app(&self, id: &str) -> Option<&str> {
        self.records.get(id)?;
        let mut ancestor = id;
        loop {
            if let Some(app) = self.records.get(ancestor).and_then(|r| r.app.as_deref()) {
                return Some(app);
            }
            if ancestor.is_empty() {
                return None;
            }
            ancestor = ancestor
                .rsplit_once('/')
                .map(|(parent, _)| parent)
                .unwrap_or("");
        }
    }
}

fn is_at_or_below(candidate: &str, ancestor: &str) -> bool {
    ancestor.is_empty()
        || candidate == ancestor
        || candidate
            .strip_prefix(ancestor)
            .is_some_and(|suffix| suffix.starts_with('/'))
}

#[cfg(test)]
mod tests {
    use super::*;
    use wezterm_escape_parser::program_status::Command;

    fn report(body: &str) -> Report {
        match Command::parse_body(body.as_bytes()).unwrap() {
            Command::Report(report) => report,
            _ => panic!("expected a report"),
        }
    }

    #[test]
    fn program_status_replaces_the_complete_record() {
        let mut store = ProgramStatusStore::default();
        assert!(store.apply(report("state=working:app=cargo:progress=40")));
        assert!(store.apply(report("state=done")));
        let records = store.snapshot();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].state, State::Done);
        assert_eq!(records[0].app, None);
        assert_eq!(records[0].progress, None);
    }

    #[test]
    fn program_status_subtree_clearing_respects_boundaries() {
        let mut store = ProgramStatusStore::default();
        for id in ["", "build", "build/test", "builder/test"] {
            let mut r = report("state=working");
            r.id = id.to_owned();
            store.apply(r);
        }
        assert!(store.apply(report("state=clear:id=build")));
        assert_eq!(
            store
                .snapshot()
                .iter()
                .map(|r| r.id.as_str())
                .collect::<Vec<_>>(),
            ["", "builder/test"]
        );
        assert!(!store.clear("absent"));
        assert!(store.apply(report("state=clear")));
        assert!(store.snapshot().is_empty());
    }

    #[test]
    fn program_status_inherits_from_nearest_existing_ancestor() {
        let mut store = ProgramStatusStore::default();
        store.apply(report("state=working:app=root"));
        store.apply(report("state=working:id=build:app=cargo"));
        store.apply(report("state=blocked:id=build/missing/test"));
        assert_eq!(store.effective_app("build/missing/test"), Some("cargo"));
        store.apply(report("state=working:id=build"));
        assert_eq!(store.effective_app("build/missing/test"), Some("root"));
        store.apply(report("state=idle"));
        assert_eq!(store.effective_app("build/missing/test"), None);
        assert_eq!(store.effective_app("absent"), None);
    }

    #[test]
    fn program_status_prompt_and_exit_keep_finished_records() {
        let mut store = ProgramStatusStore::default();
        for (id, state) in [
            ("a", "working"),
            ("b", "blocked"),
            ("c", "done"),
            ("d", "error"),
            ("e", "idle"),
        ] {
            store.apply(report(&format!("state={state}:id={id}")));
        }
        assert!(store.finish_command());
        assert_eq!(
            store.snapshot().iter().map(|r| r.state).collect::<Vec<_>>(),
            [State::Done, State::Error, State::Idle]
        );
        assert!(!store.finish_command());
    }

    #[test]
    fn program_status_evicts_oldest_update_including_identical_refreshes() {
        let mut store = ProgramStatusStore::default();
        for n in 0..MAX_RECORDS {
            store.apply(report(&format!("state=working:id=task{n}")));
        }
        assert!(!store.apply(report("state=working:id=task0")));
        assert!(store.apply(report("state=working:id=new")));
        assert_eq!(store.snapshot().len(), MAX_RECORDS);
        assert!(store.records.contains_key("task0"));
        assert!(!store.records.contains_key("task1"));
        store.clear("task0");
        store.apply(report("state=done:id=after-clear"));
        assert_eq!(store.snapshot().len(), MAX_RECORDS);
    }

    #[test]
    fn program_status_presentation_snapshot_is_owned_and_resolves_inheritance() {
        let mut store = ProgramStatusStore::default();
        store.apply(report("state=working:app=cargo"));
        store.apply(report("state=blocked:id=build/test:kind=permission"));
        let mut snapshot = store.presentation_snapshot();
        assert_eq!(snapshot[1].id, "build/test");
        assert_eq!(snapshot[1].app, None);
        assert_eq!(snapshot[1].effective_app.as_deref(), Some("cargo"));
        assert_eq!(snapshot[1].state, "blocked");
        assert_eq!(snapshot[1].kind.as_deref(), Some("permission"));
        snapshot[1].state = "done".to_owned();
        snapshot.clear();
        assert_eq!(store.presentation_snapshot()[1].state, "blocked");
    }

    #[test]
    fn program_status_ancestor_eviction_updates_existing_child_inheritance() {
        let mut store = ProgramStatusStore::default();
        store.apply(report("state=idle:app=old-root"));
        store.apply(report("state=blocked:id=child"));
        for n in 0..MAX_RECORDS - 2 {
            store.apply(report(&format!("state=idle:id=task{n}")));
        }
        assert_eq!(store.effective_app("child"), Some("old-root"));
        // Root is the least recently updated record. Its eviction must also
        // change the derived app on surviving children without a child report.
        store.apply(report("state=idle:id=overflow"));
        assert_eq!(store.effective_app("child"), None);
        let snapshot = store.presentation_snapshot();
        let child = snapshot.iter().find(|record| record.id == "child").unwrap();
        assert_eq!(child.effective_app, None);
    }

    #[test]
    fn program_status_late_transients_replace_old_results_without_clearing_children() {
        for previous in ["done", "error", "idle"] {
            for transient in ["working", "blocked"] {
                let mut store = ProgramStatusStore::default();
                store.apply(report(&format!("state={previous}:id=build:app=cargo")));
                store.apply(report("state=done:id=build/tests"));
                assert!(!store.process_exited());

                assert!(store.apply(report(&format!("state={transient}:id=build"))));
                assert!(!store.records.contains_key("build"));
                assert_eq!(
                    store.recency.iter().map(String::as_str).collect::<Vec<_>>(),
                    ["build/tests"]
                );
                assert_eq!(store.snapshot()[0].state, State::Done);
                assert_eq!(store.effective_app("build/tests"), None);

                // There is no further visible change when the same late
                // transient report arrives again, or addresses a missing id.
                assert!(!store.apply(report(&format!("state={transient}:id=build"))));
                assert!(!store.apply(report(&format!("state={transient}:id=absent"))));
            }
        }
    }
}
