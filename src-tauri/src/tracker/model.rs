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
///
/// The whole shape travels to the front end, not just what the board draws:
/// the task inspector shows every field bd knows, and a field left out here is
/// invisible there with nothing to say it went missing.
///
/// **`owner` and `assignee` are two different people and bd emits both**
/// (smetana-a5b). `owner` is whoever owns the issue; `assignee` is who holds it
/// right now, and it is what `bd update <id> --claim` sets — so a run's actor
/// (`smetana-run-<session-id>`) lands in `assignee` and never in `owner`. This
/// doc comment used to say bd never emits `assignee` at all, which was false,
/// and that one sentence is how four separate lookups came to filter on `owner`
/// and therefore match nothing, always. `IssuePatch` calls the write-side field
/// `assignee` too, because that is what `bd update -a` sets.
///
/// The counts are bd's own and are kept apart from `dependencies`: they count
/// the whole graph, while the edge list carries only the outgoing links.
/// `Default` is for the tests only — an issue with this many optional fields
/// is unreadable when spelled out in full, and every one of them spelled out is
/// noise around the two or three a test is actually about.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Issue {
    pub id: String,
    pub title: String,
    pub status: String,
    pub updated_at: String,
    #[serde(default)]
    pub description: Option<String>,
    /// The three prose fields beside the description (smetana-dbr): the spec's
    /// own sections, and `notes` is where a run writes why it parked a task —
    /// dropping that one made the reason readable only through `bd show`.
    #[serde(default)]
    pub acceptance_criteria: Option<String>,
    #[serde(default)]
    pub design: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub priority: Option<i64>,
    #[serde(default)]
    pub issue_type: Option<String>,
    #[serde(default)]
    pub owner: Option<String>,
    /// Who holds the issue right now — what `--claim` sets. See the note above:
    /// this is not `owner`, and reading one for the other is smetana-a5b.
    #[serde(default)]
    pub assignee: Option<String>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub created_by: Option<String>,
    #[serde(default)]
    pub started_at: Option<String>,
    #[serde(default)]
    pub closed_at: Option<String>,
    #[serde(default)]
    pub close_reason: Option<String>,
    #[serde(default)]
    pub comment_count: Option<i64>,
    #[serde(default)]
    pub dependency_count: Option<i64>,
    #[serde(default)]
    pub dependent_count: Option<i64>,
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
    /// Appended to the issue's notes with a newline, never replacing them —
    /// bd's `--append-notes`. This is how a run records why it parked a task:
    /// the note is the one sentence a person reads about a night's decision,
    /// and an overwrite would eat every earlier one.
    #[serde(default)]
    pub append_notes: Option<String>,
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
