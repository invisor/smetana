use std::collections::BTreeMap;

use super::model::{ColumnDef, Delta, Issue, Snapshot};

/// Снимок трекера в памяти. Владеет им единственный воркер, поэтому
/// синхронизация здесь не нужна.
#[derive(Default)]
pub struct Store {
    issues: BTreeMap<String, Issue>,
    columns: Vec<ColumnDef>,
    generation: u64,
    last_seen: String,
}

impl Store {
    /// Поколение фронт читает из снимка и дельты, поэтому в самом приложении
    /// этот доступ не нужен — он остаётся ради утверждений в тестах ниже.
    #[allow(dead_code)]
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Метка времени для следующей инкрементальной догрузки.
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

    /// Возвращает true, если набор колонок действительно изменился.
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

    /// Полная сверка: всё, чего нет в выборке, считается удалённым.
    /// Инкремент так делать не может — он по определению видит не всё.
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

    /// Кладёт результат собственной записи, не дожидаясь watcher.
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
            title: format!("задача {id}"),
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
    fn новая_задача_попадает_в_дельту() {
        let mut store = Store::default();
        let delta = store.apply_incremental(vec![issue("a", "open", "2026-07-31T00:00:01Z")]);
        assert_eq!(delta.upserted.len(), 1);
        assert_eq!(delta.generation, 1);
    }

    #[test]
    fn неизменившаяся_задача_дельту_не_порождает() {
        let mut store = Store::default();
        store.apply_incremental(vec![issue("a", "open", "2026-07-31T00:00:01Z")]);
        let delta = store.apply_incremental(vec![issue("a", "open", "2026-07-31T00:00:01Z")]);
        assert!(delta.is_empty());
        assert_eq!(store.generation(), 1, "поколение растёт только при непустой дельте");
    }

    #[test]
    fn смена_статуса_попадает_в_дельту() {
        let mut store = Store::default();
        store.apply_incremental(vec![issue("a", "open", "2026-07-31T00:00:01Z")]);
        let delta = store.apply_incremental(vec![issue("a", "closed", "2026-07-31T00:00:02Z")]);
        assert_eq!(delta.upserted.len(), 1);
        assert_eq!(delta.upserted[0].status, "closed");
    }

    #[test]
    fn инкремент_не_удаляет_отсутствующее() {
        let mut store = Store::default();
        store.apply_incremental(vec![
            issue("a", "open", "2026-07-31T00:00:01Z"),
            issue("b", "open", "2026-07-31T00:00:01Z"),
        ]);
        let delta = store.apply_incremental(vec![issue("a", "closed", "2026-07-31T00:00:02Z")]);
        assert!(delta.removed.is_empty());
    }

    #[test]
    fn полная_сверка_убирает_пропавшее() {
        let mut store = Store::default();
        store.apply_incremental(vec![
            issue("a", "open", "2026-07-31T00:00:01Z"),
            issue("b", "open", "2026-07-31T00:00:01Z"),
        ]);
        let delta = store.apply_full(vec![issue("a", "open", "2026-07-31T00:00:01Z")]);
        assert_eq!(delta.removed, vec!["b".to_string()]);
    }

    #[test]
    fn запоминает_наибольшую_метку_времени() {
        let mut store = Store::default();
        store.apply_incremental(vec![
            issue("a", "open", "2026-07-31T00:00:05Z"),
            issue("b", "open", "2026-07-31T00:00:01Z"),
        ]);
        assert_eq!(store.last_seen(), "2026-07-31T00:00:05Z");
    }

    /// columns_delta корректна ровно тогда, когда её зовут после
    /// set_columns, вернувшего true, — поэтому проверяем связку целиком, а не
    /// один только флаг: и что дельта несёт новый набор, и что поколение
    /// сдвинулось на единицу, и что задач в ней нет.
    #[test]
    fn смена_набора_колонок_попадает_в_дельту() {
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

        assert!(!store.set_columns(columns), "тот же набор дельты не порождает");
        assert_eq!(store.generation(), 1);
    }
}
