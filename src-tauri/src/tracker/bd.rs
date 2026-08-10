use std::path::PathBuf;

use tauri::AppHandle;
use tauri_plugin_shell::ShellExt;

use super::model::{ColumnDef, Issue, IssuePatch, TrackerError};

/// The column order comes from bd's categories: available first, then in
/// progress, then frozen, then done.
fn category_rank(category: &str) -> u8 {
    match category {
        "active" => 0,
        "wip" => 1,
        "frozen" => 2,
        "done" => 3,
        _ => 4,
    }
}

/// bd's warnings go to stderr, but relying on that entirely is unwise: we cut
/// off everything before the first bracket.
fn slice_json(stdout: &str) -> Result<&str, TrackerError> {
    stdout
        .find(['[', '{'])
        .map(|i| &stdout[i..])
        .ok_or(TrackerError::NoJson)
}

/// bd create returns an object, while update and close return an array because
/// they take several identifiers. Both forms are reduced to a vector.
pub fn parse_issues(stdout: &str) -> Result<Vec<Issue>, TrackerError> {
    let value: serde_json::Value =
        serde_json::from_str(slice_json(stdout)?).map_err(|e| TrackerError::Parse(e.to_string()))?;
    match value {
        serde_json::Value::Array(_) | serde_json::Value::Object(_) => {
            let wrapped = if value.is_array() {
                value
            } else {
                serde_json::Value::Array(vec![value])
            };
            serde_json::from_value(wrapped).map_err(|e| TrackerError::Parse(e.to_string()))
        }
        _ => Err(TrackerError::Parse("expected an object or an array".into())),
    }
}

pub fn parse_columns(stdout: &str) -> Result<Vec<ColumnDef>, TrackerError> {
    #[derive(serde::Deserialize)]
    struct Out {
        #[serde(default)]
        built_in_statuses: Vec<ColumnDef>,
        #[serde(default)]
        custom_statuses: Vec<ColumnDef>,
    }
    let out: Out = serde_json::from_str(slice_json(stdout)?)
        .map_err(|e| TrackerError::Parse(e.to_string()))?;
    let mut columns = out.built_in_statuses;
    columns.extend(out.custom_statuses);
    columns.sort_by_key(|c| category_rank(&c.category));
    Ok(columns)
}

pub fn parse_version(stdout: &str) -> Option<String> {
    stdout
        .split_whitespace()
        .skip_while(|w| *w != "version")
        .nth(1)
        .map(str::to_string)
}

/// `--` before the identifier: bd parses everything after it as a positional
/// argument rather than a flag.
pub fn update_args(id: &str, patch: &IssuePatch) -> Vec<String> {
    let mut args = vec!["update".to_string(), "--json".to_string()];
    let mut push = |flag: &str, value: String| {
        args.push(flag.to_string());
        args.push(value);
    };
    if let Some(v) = &patch.status {
        push("-s", v.clone());
    }
    if let Some(v) = &patch.title {
        push("--title", v.clone());
    }
    if let Some(v) = &patch.description {
        push("-d", v.clone());
    }
    if let Some(v) = &patch.issue_type {
        push("-t", v.clone());
    }
    if let Some(v) = patch.priority {
        push("-p", v.to_string());
    }
    if let Some(v) = &patch.assignee {
        push("-a", v.clone());
    }
    if let Some(v) = &patch.append_notes {
        push("--append-notes", v.clone());
    }
    for label in &patch.add_labels {
        push("--add-label", label.clone());
    }
    for label in &patch.remove_labels {
        push("--remove-label", label.clone());
    }
    args.push("--".into());
    args.push(id.to_string());
    args
}

/// `-f` is not optional and is not about skipping a prompt: without it
/// `bd delete` prints a preview and deletes nothing, so the call would report
/// success while the issue stayed on the board. It also decides what happens to
/// the issue's dependents — plain `bd delete` refuses outright when there are
/// any, which would make the button useless on every parent task, while `-f`
/// deletes and orphans them. The confirmation the person sees says so.
pub fn delete_args(id: &str) -> Vec<String> {
    vec!["delete".to_string(), "-f".to_string(), "--".to_string(), id.to_string()]
}

/// `bd init` in the project's directory.
///
/// `--non-interactive` is mandatory: we have no terminal, and a wizard waiting
/// for an answer about the role would hang the call forever. We do not pass an
/// issue prefix — bd takes the directory name, and that is exactly what a
/// person expects.
pub fn init_args() -> Vec<String> {
    vec!["init".to_string(), "--non-interactive".to_string()]
}

/// A wrapper around the bundled bd binary. The only place that knows what the
/// CLI arguments look like.
#[derive(Clone)]
pub struct Bd {
    app: AppHandle,
    cwd: PathBuf,
}

impl Bd {
    pub fn new(app: AppHandle, cwd: PathBuf) -> Self {
        Self { app, cwd }
    }

    /// Only a non-zero exit code counts as an error. bd's warnings
    /// ("dolt auto-push failed", "beads.role not configured") go to stderr all
    /// the time and are not errors.
    async fn run(&self, args: Vec<String>) -> Result<String, TrackerError> {
        let output = self
            .app
            .shell()
            .sidecar("bd")
            .map_err(|e| TrackerError::Spawn(e.to_string()))?
            .current_dir(self.cwd.clone())
            .args(args)
            .output()
            .await
            .map_err(|e| TrackerError::Spawn(e.to_string()))?;

        if !output.status.success() {
            return Err(TrackerError::Command {
                code: output.status.code().unwrap_or(-1),
                stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
            });
        }
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    async fn one(&self, args: Vec<String>) -> Result<Issue, TrackerError> {
        parse_issues(&self.run(args).await?)?
            .into_iter()
            .next()
            .ok_or(TrackerError::Empty)
    }

    pub async fn version(&self) -> Result<Option<String>, TrackerError> {
        Ok(parse_version(&self.run(vec!["version".into()]).await?))
    }

    pub async fn columns(&self) -> Result<Vec<ColumnDef>, TrackerError> {
        parse_columns(&self.run(vec!["statuses".into(), "--json".into()]).await?)
    }

    /// -n 0 is mandatory: by default bd list returns only 50 entries.
    pub async fn list_all(&self) -> Result<Vec<Issue>, TrackerError> {
        parse_issues(
            &self
                .run(vec![
                    "list".into(),
                    "--all".into(),
                    "-n".into(),
                    "0".into(),
                    "--json".into(),
                ])
                .await?,
        )
    }

    pub async fn list_updated_after(&self, since: &str) -> Result<Vec<Issue>, TrackerError> {
        parse_issues(
            &self
                .run(vec![
                    "list".into(),
                    "--all".into(),
                    "-n".into(),
                    "0".into(),
                    "--updated-after".into(),
                    since.to_string(),
                    "--json".into(),
                ])
                .await?,
        )
    }

    /// Sets up a tracker in the directory. We do not parse the output: only
    /// the exit code matters — the worker re-reads the folder from scratch anyway.
    pub async fn init(&self) -> Result<(), TrackerError> {
        self.run(init_args()).await.map(|_| ())
    }

    pub async fn update(&self, id: &str, patch: &IssuePatch) -> Result<Issue, TrackerError> {
        self.one(update_args(id, patch)).await
    }

    pub async fn close(&self, id: &str, reason: Option<&str>) -> Result<Issue, TrackerError> {
        let mut args = vec!["close".to_string(), "--json".to_string()];
        if let Some(reason) = reason {
            args.push("-r".into());
            args.push(reason.to_string());
        }
        args.push("--".into());
        args.push(id.to_string());
        self.one(args).await
    }

    pub async fn reopen(&self, id: &str) -> Result<Issue, TrackerError> {
        self.one(vec!["reopen".into(), "--json".into(), "--".into(), id.to_string()])
            .await
    }

    /// Deletion is irreversible and gives nothing back to parse — an issue that
    /// no longer exists has no shape to return. Only the exit code matters, the
    /// same as `init`.
    pub async fn delete(&self, id: &str) -> Result<(), TrackerError> {
        self.run(delete_args(id)).await.map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// This is what bd list --json output looks like: empty fields are absent
    /// altogether.
    const LIST: &str = r#"[
      {"id":"smetana-29j","title":"Live synchronization","status":"open","priority":1,
       "issue_type":"feature","created_at":"2026-07-30T21:31:27Z","updated_at":"2026-07-30T21:31:27Z",
       "dependency_count":0,"dependent_count":0,"comment_count":0},
      {"id":"smetana-3km","title":"contract check","status":"open","priority":2,
       "issue_type":"task","owner":"flexo","labels":["alpha"],"parent":"smetana-29j",
       "description":"the shape bd hands over","created_by":"flexo",
       "acceptance_criteria":"AC body","design":"Design body",
       "notes":"parked: needs a decision\nparked: still waiting",
       "created_at":"2026-07-31T00:58:55Z","updated_at":"2026-07-31T00:58:55Z",
       "dependencies":[
         {"issue_id":"smetana-3km","depends_on_id":"smetana-1or","type":"blocks",
          "created_at":"2026-07-31T00:58:55Z","created_by":"flexo","metadata":"{}"},
         {"issue_id":"smetana-3km","depends_on_id":"smetana-29j","type":"parent-child",
          "created_at":"2026-07-31T00:58:55Z","created_by":"flexo","metadata":"{}"}]}
    ]"#;

    /// bd create returns an object, not an array.
    const CREATED: &str = r#"{"id":"smetana-3km","title":"contract check","status":"open",
      "priority":2,"issue_type":"task","updated_at":"2026-07-30T21:57:07Z"}"#;

    const STATUSES: &str = r#"{"built_in_statuses":[
        {"category":"active","description":"Available to work","icon":"○","name":"open"},
        {"category":"done","description":"Completed","icon":"✓","name":"closed"},
        {"category":"wip","description":"Actively being worked on","icon":"◐","name":"in_progress"}],
      "custom_statuses":[{"category":"wip","name":"awaiting-review"}],
      "schema_version":1}"#;

    #[test]
    fn parses_an_array_of_issues() {
        let issues = parse_issues(LIST).unwrap();
        assert_eq!(issues.len(), 2);
        assert_eq!(issues[0].id, "smetana-29j");
        assert_eq!(issues[0].owner, None);
        assert!(issues[0].labels.is_empty());
    }

    /// bd calls it `owner`; the struct called it `assignee` for a while, which
    /// meant it silently stayed None on every issue bd ever returned. The name
    /// is the whole test.
    #[test]
    fn the_owner_arrives_under_bds_own_name() {
        let issues = parse_issues(LIST).unwrap();
        assert_eq!(issues[1].owner.as_deref(), Some("flexo"));
    }

    /// Everything the task inspector shows has to survive deserialization —
    /// a field missing from the struct is invisible in the panel with nothing
    /// to say it went missing.
    #[test]
    fn the_inspectors_fields_survive_the_parse() {
        let issues = parse_issues(LIST).unwrap();
        assert_eq!(issues[1].description.as_deref(), Some("the shape bd hands over"));
        assert_eq!(issues[1].acceptance_criteria.as_deref(), Some("AC body"));
        assert_eq!(issues[1].design.as_deref(), Some("Design body"));
        // The whole note, appended lines included — a truncated note would
        // silently drop the latest "parked:" line, the one a person reads.
        assert_eq!(
            issues[1].notes.as_deref(),
            Some("parked: needs a decision\nparked: still waiting")
        );
        assert_eq!(issues[0].notes, None);
        assert_eq!(issues[1].created_by.as_deref(), Some("flexo"));
        assert_eq!(issues[1].created_at.as_deref(), Some("2026-07-31T00:58:55Z"));
        assert_eq!(issues[0].comment_count, Some(0));
        assert_eq!(issues[0].dependent_count, Some(0));
    }

    #[test]
    fn parses_a_single_object() {
        let issues = parse_issues(CREATED).unwrap();
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].id, "smetana-3km");
    }

    #[test]
    fn keeps_the_dependency_kind() {
        let issues = parse_issues(LIST).unwrap();
        let kinds: Vec<&str> = issues[1].dependencies.iter().map(|d| d.kind.as_str()).collect();
        assert_eq!(kinds, vec!["blocks", "parent-child"]);
    }

    #[test]
    fn skips_the_banner_before_the_json() {
        let issues = parse_issues("warning: beads.role not configured\n[]").unwrap();
        assert!(issues.is_empty());
    }

    #[test]
    fn columns_come_built_in_and_custom_in_category_order() {
        let cols = parse_columns(STATUSES).unwrap();
        let names: Vec<&str> = cols.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, vec!["open", "in_progress", "awaiting-review", "closed"]);
    }

    #[test]
    fn extracts_the_version() {
        assert_eq!(parse_version("bd version 1.1.2 (20e493e5)").as_deref(), Some("1.1.2"));
        assert_eq!(parse_version("nonsense"), None);
    }

    #[test]
    fn the_update_arguments_carry_only_the_fields_that_were_set() {
        let patch = IssuePatch { status: Some("in_progress".into()), title: Some("new one".into()),
            ..Default::default() };
        assert_eq!(update_args("smetana-1", &patch),
            vec!["update", "--json", "-s", "in_progress", "--title", "new one", "--", "smetana-1"]);
    }

    /// The park a run performs is one update, not two calls: `--append-notes`
    /// rides beside `-s`, and appends rather than replaces — `bd note` itself
    /// is shorthand for exactly this flag, so an earlier park's note survives.
    #[test]
    fn parking_carries_the_status_and_the_appended_note_in_one_update() {
        let patch = IssuePatch {
            status: Some("parked".into()),
            append_notes: Some("parked: Do you trust this directory?".into()),
            ..Default::default()
        };
        assert_eq!(
            update_args("smetana-1", &patch),
            vec![
                "update",
                "--json",
                "-s",
                "parked",
                "--append-notes",
                "parked: Do you trust this directory?",
                "--",
                "smetana-1"
            ]
        );
    }

    /// `-f` turns a preview into an actual deletion; without it bd would print
    /// what it would have removed, exit zero, and leave the issue in place.
    #[test]
    fn deleting_asks_for_the_deletion_and_not_for_a_preview() {
        assert_eq!(delete_args("smetana-1"), vec!["delete", "-f", "--", "smetana-1"]);
    }

    #[test]
    fn initialization_asks_no_questions() {
        assert_eq!(init_args(), vec!["init".to_string(), "--non-interactive".to_string()]);
    }
}
