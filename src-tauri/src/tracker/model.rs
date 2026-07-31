use serde::{Deserialize, Serialize};

/// Ребро графа зависимостей. bd отдаёт у задачи только исходящие связи:
/// issue_id зависит от depends_on_id.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Dependency {
    pub issue_id: String,
    pub depends_on_id: String,
    /// "blocks", "parent-child", "related", "discovered-from"
    #[serde(rename = "type")]
    pub kind: String,
}

/// Задача в том виде, в каком её отдаёт bd. Пустые поля bd опускает целиком,
/// поэтому всё необязательное — Option или коллекция со значением по умолчанию.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Issue {
    pub id: String,
    pub title: String,
    pub status: String,
    pub updated_at: String,
    #[serde(default)]
    pub priority: Option<i64>,
    #[serde(default)]
    pub issue_type: Option<String>,
    #[serde(default)]
    pub assignee: Option<String>,
    #[serde(default)]
    pub parent: Option<String>,
    #[serde(default)]
    pub labels: Vec<String>,
    #[serde(default)]
    pub dependencies: Vec<Dependency>,
}

/// Колонка доски. Из bd берём только имя и категорию: глиф и цвет
/// принадлежат status.js, свои иконки bd мы намеренно игнорируем.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ColumnDef {
    pub name: String,
    /// "active" | "wip" | "frozen" | "done"
    pub category: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Snapshot {
    pub generation: u64,
    pub columns: Vec<ColumnDef>,
    pub issues: Vec<Issue>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Delta {
    pub generation: u64,
    pub upserted: Vec<Issue>,
    pub removed: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub columns: Option<Vec<ColumnDef>>,
}

impl Delta {
    pub fn is_empty(&self) -> bool {
        self.upserted.is_empty() && self.removed.is_empty() && self.columns.is_none()
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct NewIssue {
    pub title: String,
    pub issue_type: String,
    pub priority: i64,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct IssuePatch {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub issue_type: Option<String>,
    #[serde(default)]
    pub priority: Option<i64>,
    #[serde(default)]
    pub assignee: Option<String>,
    #[serde(default)]
    pub add_labels: Vec<String>,
    #[serde(default)]
    pub remove_labels: Vec<String>,
}

// Потребитель появится в задаче 7 (воркер эмитит tracker:health).
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum HealthState {
    Ok,
    NotABeadsRepo,
    BdVersionMismatch,
    Error,
}

// Потребитель появится в задаче 7 (воркер эмитит tracker:health).
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize)]
pub struct Health {
    pub state: HealthState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum TrackerError {
    #[error("bd завершился с кодом {code}: {stderr}")]
    Command { code: i32, stderr: String },
    #[error("в выводе bd нет JSON")]
    NoJson,
    #[error("не удалось разобрать вывод bd: {0}")]
    Parse(String),
    #[error("не удалось запустить bd: {0}")]
    Spawn(String),
    #[error("bd вернул пустой результат")]
    Empty,
}

// Tauri требует, чтобы ошибка команды умела сериализоваться.
impl Serialize for TrackerError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}
