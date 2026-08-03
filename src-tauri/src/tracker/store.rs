use std::collections::BTreeMap;

use super::model::{ColumnDef, Delta, Issue, Snapshot};

/// The tracker's in-memory snapshot. A single worker owns it, so no
/// synchronization is needed here.
#[derive(Default)]
pub struct Store {
    issues: BTreeMap<String, Issue>,
    columns: Vec<ColumnDef>,
    generation: u64,
    last_seen: String,
}

impl Store {
    /// The front end reads the generation from the snapshot and the delta, so
    /// the app itself does not need this accessor — it stays for the assertions
    /// in the tests below.
    #[allow(dead_code)]
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// The timestamp for the next incremental catch-up.
    pub fn last_seen(&self) -> &str {
        &self.last_seen
    }

    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            generation: self.generation,
            columns: self.columns.clone(),
            issues: self.issues.values().cloned().collect(),
        }
    }

    /// A project switch: the snapshot belonged to the previous folder and is
    /// wrong in its entirety. The delta carries the disappearance of every
    /// former issue and column, and the generation grows with it — otherwise a
    /// listener that did not manage to mute events for the duration of the
    /// switch would mix the new project's issues into the old project's without
    /// noticing a gap in the numbering. The generation still never rolls back:
    /// if there is nothing to reset (the snapshot is already empty), the delta
    /// is empty and the generation stays as it was — growth with no delta sent
    /// would read to a listener as a missed event.
    pub fn reset(&mut self) -> Delta {
        let removed: Vec<String> = self.issues.keys().cloned().collect();
        let had_columns = !self.columns.is_empty();

        self.issues.clear();
        self.columns.clear();
        self.last_seen.clear();

        let mut delta = Delta {
            removed,
            columns: if had_columns { Some(Vec::new()) } else { None },
            ..Default::default()
        };

        if !delta.is_empty() {
            self.generation += 1;
            delta.generation = self.generation;
        }
        delta
    }

    /// Returns true if the set of columns really did change.
    pub fn set_columns(&mut self, columns: Vec<ColumnDef>) -> bool {
        if self.columns == columns {
            return false;
        }
        self.columns = columns;
        true
    }

    pub fn apply_incremental(&mut self, fetched: Vec<Issue>) -> Delta {
        self.apply(fetched, false)
    }

    /// A full sweep: anything absent from the batch counts as deleted. An
    /// incremental one cannot do that — by definition it does not see everything.
    pub fn apply_full(&mut self, fetched: Vec<Issue>) -> Delta {
        self.apply(fetched, true)
    }

    fn apply(&mut self, fetched: Vec<Issue>, full: bool) -> Delta {
        let mut delta = Delta::default();

        if full {
            let seen: std::collections::BTreeSet<&String> = fetched.iter().map(|i| &i.id).collect();
            delta.removed = self
                .issues
                .keys()
                .filter(|id| !seen.contains(id))
                .cloned()
                .collect();
            for id in &delta.removed {
                self.issues.remove(id);
            }
        }

        for issue in fetched {
            if issue.updated_at > self.last_seen {
                self.last_seen = issue.updated_at.clone();
            }
            if self.issues.get(&issue.id) != Some(&issue) {
                self.issues.insert(issue.id.clone(), issue.clone());
                delta.upserted.push(issue);
            }
        }

        if !delta.is_empty() {
            self.generation += 1;
            delta.generation = self.generation;
        }
        delta
    }

    /// Puts the result of our own write in, without waiting for the watcher.
    pub fn upsert_one(&mut self, issue: Issue) -> Delta {
        self.apply_incremental(vec![issue])
    }

    pub fn columns_delta(&mut self) -> Delta {
        self.generation += 1;
        Delta {
            generation: self.generation,
            columns: Some(self.columns.clone()),
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tracker::model::Issue;

    fn issue(id: &str, status: &str, updated: &str) -> Issue {
        Issue {
            id: id.into(),
            title: format!("issue {id}"),
            status: status.into(),
            updated_at: updated.into(),
            priority: None,
            issue_type: None,
            assignee: None,
            parent: None,
            labels: vec![],
            dependencies: vec![],
        }
    }

    #[test]
    fn a_new_issue_lands_in_the_delta() {
        let mut store = Store::default();
        let delta = store.apply_incremental(vec![issue("a", "open", "2026-07-31T00:00:01Z")]);
        assert_eq!(delta.upserted.len(), 1);
        assert_eq!(delta.generation, 1);
    }

    #[test]
    fn an_unchanged_issue_produces_no_delta() {
        let mut store = Store::default();
        store.apply_incremental(vec![issue("a", "open", "2026-07-31T00:00:01Z")]);
        let delta = store.apply_incremental(vec![issue("a", "open", "2026-07-31T00:00:01Z")]);
        assert!(delta.is_empty());
        assert_eq!(store.generation(), 1, "the generation grows only on a non-empty delta");
    }

    #[test]
    fn a_status_change_lands_in_the_delta() {
        let mut store = Store::default();
        store.apply_incremental(vec![issue("a", "open", "2026-07-31T00:00:01Z")]);
        let delta = store.apply_incremental(vec![issue("a", "closed", "2026-07-31T00:00:02Z")]);
        assert_eq!(delta.upserted.len(), 1);
        assert_eq!(delta.upserted[0].status, "closed");
    }

    #[test]
    fn an_incremental_sweep_deletes_nothing_that_is_absent() {
        let mut store = Store::default();
        store.apply_incremental(vec![
            issue("a", "open", "2026-07-31T00:00:01Z"),
            issue("b", "open", "2026-07-31T00:00:01Z"),
        ]);
        let delta = store.apply_incremental(vec![issue("a", "closed", "2026-07-31T00:00:02Z")]);
        assert!(delta.removed.is_empty());
    }

    #[test]
    fn a_full_sweep_removes_what_vanished() {
        let mut store = Store::default();
        store.apply_incremental(vec![
            issue("a", "open", "2026-07-31T00:00:01Z"),
            issue("b", "open", "2026-07-31T00:00:01Z"),
        ]);
        let delta = store.apply_full(vec![issue("a", "open", "2026-07-31T00:00:01Z")]);
        assert_eq!(delta.removed, vec!["b".to_string()]);
    }

    #[test]
    fn remembers_the_largest_timestamp() {
        let mut store = Store::default();
        store.apply_incremental(vec![
            issue("a", "open", "2026-07-31T00:00:05Z"),
            issue("b", "open", "2026-07-31T00:00:01Z"),
        ]);
        assert_eq!(store.last_seen(), "2026-07-31T00:00:05Z");
    }

    /// columns_delta is correct exactly when it is called after a set_columns
    /// that returned true — so we check the pair as a whole rather than the flag
    /// alone: that the delta carries the new set, that the generation moved by
    /// one, and that there are no issues in it.
    #[test]
    fn a_change_to_the_column_set_lands_in_the_delta() {
        let columns = vec![
            ColumnDef { name: "open".into(), category: "active".into() },
            ColumnDef { name: "closed".into(), category: "done".into() },
        ];
        let mut store = Store::default();

        assert!(store.set_columns(columns.clone()));
        let delta = store.columns_delta();
        assert_eq!(delta.columns.as_deref(), Some(&columns[..]));
        assert_eq!(delta.generation, 1);
        assert!(delta.upserted.is_empty() && delta.removed.is_empty());
        assert!(!delta.is_empty());

        assert!(!store.set_columns(columns), "the same set produces no delta");
        assert_eq!(store.generation(), 1);
    }

    #[test]
    fn a_reset_sends_a_delta_with_a_gap_and_moves_the_generation() {
        let mut store = Store::default();
        store.set_columns(vec![ColumnDef { name: "open".into(), category: "active".into() }]);
        store.apply_full(vec![
            issue("a", "open", "2026-07-31T00:00:01Z"),
            issue("b", "open", "2026-07-31T00:00:01Z"),
        ]);
        let before = store.generation();

        let delta = store.reset();

        let mut removed = delta.removed.clone();
        removed.sort();
        assert_eq!(
            removed,
            vec!["a".to_string(), "b".to_string()],
            "the front end has to learn that every issue of the previous project is gone"
        );
        assert_eq!(
            delta.columns.as_deref(),
            Some(&[][..]),
            "the previous tracker's columns stopped existing too"
        );
        assert_eq!(delta.generation, before + 1, "the delta carries the new generation");
        assert_eq!(
            store.generation(),
            before + 1,
            "the generation grew by exactly one — neither skipped nor doubled"
        );

        let snapshot = store.snapshot();
        assert!(snapshot.issues.is_empty(), "the issues belonged to the previous project");
        assert!(snapshot.columns.is_empty(), "the columns too: another tracker has its own");
        assert_eq!(store.last_seen(), "", "otherwise the new project's first catch-up would ask only for recent changes");
    }

    #[test]
    fn resetting_an_already_empty_snapshot_does_not_move_the_generation() {
        let mut store = Store::default();
        let before = store.generation();

        let delta = store.reset();

        assert!(delta.is_empty(), "there was nothing to reset — the delta is empty and must not be sent");
        assert_eq!(
            store.generation(), before,
            "a generation that grew with no delta sent would read to a listener as a miss"
        );
    }
}
