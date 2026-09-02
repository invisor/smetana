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

    /// The other half of that, for the one write whose result is an absence.
    /// An id that is not here produces an empty delta rather than a phantom
    /// removal: the full sweep would have taken it out already, and telling the
    /// front end to remove what it never had would spend a generation on
    /// nothing.
    pub fn remove_one(&mut self, id: &str) -> Delta {
        if self.issues.remove(id).is_none() {
            return Delta::default();
        }
        self.generation += 1;
        Delta {
            generation: self.generation,
            removed: vec![id.to_string()],
            ..Default::default()
        }
    }

    /// The ids of everything holding one status, in id order.
    ///
    /// One caller: the sweep that closes a task whose branch somebody merged by
    /// hand (`service::close_merged`), which asks for `ready_to_merge` and for
    /// nothing else — the sweep's whole safety rests on that narrowness, since a
    /// branch carrying a task's slug may be half merged or belong to other work
    /// while the task is still open or in progress, and a false closure costs
    /// more than a missed one.
    ///
    /// Here rather than a filter over `snapshot()`: the snapshot clones every
    /// issue in the project to hand back a handful of ids, once a minute.
    pub fn ids_with_status(&self, status: &str) -> Vec<String> {
        self.issues
            .values()
            .filter(|issue| issue.status == status)
            .map(|issue| issue.id.clone())
            .collect()
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
            ..Issue::default()
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
    fn a_deletion_leaves_the_snapshot_and_lands_in_the_delta() {
        let mut store = Store::default();
        store.apply_incremental(vec![
            issue("a", "open", "2026-07-31T00:00:01Z"),
            issue("b", "open", "2026-07-31T00:00:01Z"),
        ]);
        let delta = store.remove_one("a");
        assert_eq!(delta.removed, vec!["a".to_string()]);
        assert_eq!(delta.generation, 2);
        let ids: Vec<String> = store.snapshot().issues.into_iter().map(|i| i.id).collect();
        assert_eq!(ids, vec!["b".to_string()]);
    }

    /// The full sweep may have taken it out already. Announcing a removal the
    /// front end has no issue for would spend a generation on nothing.
    #[test]
    fn deleting_what_is_not_there_produces_no_delta() {
        let mut store = Store::default();
        store.apply_incremental(vec![issue("a", "open", "2026-07-31T00:00:01Z")]);
        let delta = store.remove_one("b");
        assert!(delta.is_empty());
        assert_eq!(store.generation(), 1);
    }

    /// The sweep that closes merged work asks for one status and gets exactly
    /// it. The board holds tasks in flight and tasks nobody has started, and
    /// both may have a branch carrying the same slug — closing either on the
    /// strength of that branch is the failure the narrowness exists to prevent.
    #[test]
    fn only_the_status_asked_for_comes_back() {
        let mut store = Store::default();
        store.apply_incremental(vec![
            issue("a", "ready_to_merge", "2026-07-31T00:00:01Z"),
            issue("b", "in_progress", "2026-07-31T00:00:01Z"),
            issue("c", "open", "2026-07-31T00:00:01Z"),
            issue("d", "ready_to_merge", "2026-07-31T00:00:01Z"),
            issue("e", "closed", "2026-07-31T00:00:01Z"),
        ]);
        assert_eq!(
            store.ids_with_status("ready_to_merge"),
            vec!["a".to_string(), "d".to_string()]
        );
        assert!(store.ids_with_status("parked").is_empty());
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
