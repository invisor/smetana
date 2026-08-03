use serde::{Deserialize, Serialize};

/// An edge of the dependency graph. bd gives an issue only its outgoing links:
/// issue_id depends on depends_on_id.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Dependency {
    pub issue_id: String,
    pub depends_on_id: String,
    /// "blocks", "parent-child", "related", "discovered-from"
    #[serde(rename = "type")]
    pub kind: String,
}

/// An issue in the shape bd hands it over. bd omits empty fields altogether,
/// so everything optional is either an Option or a collection with a default.
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

/// A board column. We take only the name and the category from bd: the glyph
/// and the colour belong to status.js, and bd's own icons are deliberately ignored.
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

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum HealthState {
    Ok,
    /// The project list is empty. This is not "there is no .beads here" —
    /// there is nothing to open yet, and that has to be said differently.
    NoProject,
    NotABeadsRepo,
    BdVersionMismatch,
    Error,
}

/// The comparison exists so the event fires only on a real change of state:
/// health on every successful tick is noise that hides the real trouble.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Health {
    pub state: HealthState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum TrackerError {
    #[error("bd exited with code {code}: {stderr}")]
    Command { code: i32, stderr: String },
    #[error("no JSON in bd's output")]
    NoJson,
    #[error("could not parse bd's output: {0}")]
    Parse(String),
    #[error("could not launch bd: {0}")]
    Spawn(String),
    #[error("bd returned an empty result")]
    Empty,
    #[error("no tracker in this folder: {0}")]
    NoTracker(String),
}

// Tauri requires a command's error to be serializable.
impl Serialize for TrackerError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}
