use std::path::PathBuf;

use tauri::AppHandle;
use tauri_plugin_shell::ShellExt;

use super::model::{ColumnDef, Issue, IssuePatch, NewIssue, TrackerError};

/// Порядок колонок задают категории bd: сначала доступное, потом в работе,
/// потом отложенное, потом завершённое.
fn category_rank(category: &str) -> u8 {
    match category {
        "active" => 0,
        "wip" => 1,
        "frozen" => 2,
        "done" => 3,
        _ => 4,
    }
}

/// Предупреждения bd уходят в stderr, но полагаться на это целиком не стоит:
/// отрезаем всё до первой скобки.
fn slice_json(stdout: &str) -> Result<&str, TrackerError> {
    stdout
        .find(['[', '{'])
        .map(|i| &stdout[i..])
        .ok_or(TrackerError::NoJson)
}

/// bd create отдаёт объект, а update и close — массив, потому что принимают
/// несколько идентификаторов. Приводим обе формы к вектору.
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
        _ => Err(TrackerError::Parse("ожидался объект или массив".into())),
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

/// Заголовок уезжает во флаг `--title`, а не в позиционный аргумент.
/// Оболочки здесь нет, инъекция невозможна, но задача с именем «-n 5» —
/// вполне достижимая: bd проверяет позиционный заголовок на ведущий дефис и
/// отказывается его создавать, причём даже после `--` (проверено на bd 1.1.2:
/// `title "-n 5" looks like a flag`). Значение флага такой проверки не
/// проходит и берётся как есть.
pub fn create_args(new: &NewIssue) -> Vec<String> {
    let mut args = vec![
        "create".to_string(),
        "--json".to_string(),
        "--title".to_string(),
        new.title.clone(),
        "-t".to_string(),
        new.issue_type.clone(),
        "-p".to_string(),
        new.priority.to_string(),
    ];
    if let Some(description) = &new.description {
        args.push("-d".into());
        args.push(description.clone());
    }
    args
}

/// `--` перед идентификатором: всё, что после него, bd разбирает как
/// позиционный аргумент, а не как флаг.
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

/// `bd init` в каталоге проекта.
///
/// `--non-interactive` обязателен: терминала у нас нет, а мастер, ждущий
/// ответа на вопрос про роль, повесил бы вызов навсегда. Префикс задач не
/// передаём — bd берёт имя каталога, и это ровно то, что человек и ожидает.
pub fn init_args() -> Vec<String> {
    vec!["init".to_string(), "--non-interactive".to_string()]
}

/// Обёртка над вшитым бинарником bd. Единственное место, которое знает,
/// как выглядят аргументы CLI.
#[derive(Clone)]
pub struct Bd {
    app: AppHandle,
    cwd: PathBuf,
}

impl Bd {
    pub fn new(app: AppHandle, cwd: PathBuf) -> Self {
        Self { app, cwd }
    }

    /// Ошибкой считается только ненулевой код возврата. Предупреждения bd
    /// ("dolt auto-push failed", "beads.role not configured") идут в stderr
    /// постоянно и ошибкой не являются.
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

    /// -n 0 обязателен: по умолчанию bd list отдаёт только 50 записей.
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

    pub async fn create(&self, new: &NewIssue) -> Result<Issue, TrackerError> {
        self.one(create_args(new)).await
    }

    /// Заводит трекер в каталоге. Вывод не разбираем: важен только код
    /// возврата — дальше воркер всё равно перечитывает каталог с нуля.
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
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Так выглядит выдача bd list --json: пустые поля отсутствуют целиком.
    const LIST: &str = r#"[
      {"id":"smetana-29j","title":"Живая синхронизация","status":"open","priority":1,
       "issue_type":"feature","created_at":"2026-07-30T21:31:27Z","updated_at":"2026-07-30T21:31:27Z",
       "dependency_count":0,"dependent_count":0,"comment_count":0},
      {"id":"smetana-3km","title":"проверка контракта","status":"open","priority":2,
       "issue_type":"task","assignee":"flexo","labels":["alpha"],"parent":"smetana-29j",
       "updated_at":"2026-07-31T00:58:55Z",
       "dependencies":[
         {"issue_id":"smetana-3km","depends_on_id":"smetana-1or","type":"blocks",
          "created_at":"2026-07-31T00:58:55Z","created_by":"flexo","metadata":"{}"},
         {"issue_id":"smetana-3km","depends_on_id":"smetana-29j","type":"parent-child",
          "created_at":"2026-07-31T00:58:55Z","created_by":"flexo","metadata":"{}"}]}
    ]"#;

    /// bd create отдаёт объект, а не массив.
    const CREATED: &str = r#"{"id":"smetana-3km","title":"проверка контракта","status":"open",
      "priority":2,"issue_type":"task","updated_at":"2026-07-30T21:57:07Z"}"#;

    const STATUSES: &str = r#"{"built_in_statuses":[
        {"category":"active","description":"Available to work","icon":"○","name":"open"},
        {"category":"done","description":"Completed","icon":"✓","name":"closed"},
        {"category":"wip","description":"Actively being worked on","icon":"◐","name":"in_progress"}],
      "custom_statuses":[{"category":"wip","name":"awaiting-review"}],
      "schema_version":1}"#;

    #[test]
    fn разбирает_массив_задач() {
        let issues = parse_issues(LIST).unwrap();
        assert_eq!(issues.len(), 2);
        assert_eq!(issues[0].id, "smetana-29j");
        assert_eq!(issues[0].assignee, None);
        assert!(issues[0].labels.is_empty());
    }

    #[test]
    fn разбирает_одиночный_объект() {
        let issues = parse_issues(CREATED).unwrap();
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].id, "smetana-3km");
    }

    #[test]
    fn сохраняет_тип_зависимости() {
        let issues = parse_issues(LIST).unwrap();
        let kinds: Vec<&str> = issues[1].dependencies.iter().map(|d| d.kind.as_str()).collect();
        assert_eq!(kinds, vec!["blocks", "parent-child"]);
    }

    #[test]
    fn пропускает_баннер_перед_json() {
        let issues = parse_issues("warning: beads.role not configured\n[]").unwrap();
        assert!(issues.is_empty());
    }

    #[test]
    fn колонки_идут_встроенные_и_кастомные_в_порядке_категорий() {
        let cols = parse_columns(STATUSES).unwrap();
        let names: Vec<&str> = cols.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, vec!["open", "in_progress", "awaiting-review", "closed"]);
    }

    #[test]
    fn достаёт_версию() {
        assert_eq!(parse_version("bd version 1.1.2 (20e493e5)").as_deref(), Some("1.1.2"));
        assert_eq!(parse_version("чепуха"), None);
    }

    #[test]
    fn аргументы_обновления_содержат_только_заданные_поля() {
        let patch = IssuePatch { status: Some("in_progress".into()), title: Some("новое".into()),
            ..Default::default() };
        assert_eq!(update_args("smetana-1", &patch),
            vec!["update", "--json", "-s", "in_progress", "--title", "новое", "--", "smetana-1"]);
    }

    /// Заголовок с ведущим дефисом bd обязан принять как заголовок, а не
    /// как флаг: позиционным аргументом он этого не умеет даже после `--`.
    #[test]
    fn заголовок_уходит_флагом_а_не_позиционным_аргументом() {
        let new = NewIssue { title: "-n 5".into(), issue_type: "task".into(), priority: 2,
            description: None };
        assert_eq!(create_args(&new),
            vec!["create", "--json", "--title", "-n 5", "-t", "task", "-p", "2"]);
    }

    #[test]
    fn инициализация_идёт_без_вопросов() {
        assert_eq!(init_args(), vec!["init".to_string(), "--non-interactive".to_string()]);
    }
}
