//! Настройки приложения: типы, умолчания, разбор файла и слияние.
//!
//! Здесь нет ввода-вывода: всё, что зависит от диска, живёт в `file.rs`.
//! Поэтому именно этот файл покрыт тестами — как `tracker/store.rs`.

use std::collections::{BTreeMap, HashSet};

use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// Версия схемы файла. Растёт тогда, когда старый файл нельзя прочитать как есть.
pub const CURRENT_VERSION: u32 = 1;
/// Сколько проектов помним: карта не должна расти вечно от разовых заходов.
pub const MAX_PROJECTS: usize = 20;

const THEMES: [&str; 2] = ["dark", "light"];
const DENSITIES: [&str; 2] = ["comfortable", "compact"];
/// Закрытый список — и он продублирован по ту сторону IPC: те же три вкладки
/// перечислены в `src/views/DesktopApp.vue` (константа `SIDE_TABS`). Меняя
/// один список, меняйте и второй: значение, которого здесь нет, по дороге на
/// диск молча станет "files", и после перезапуска человек увидит не то, что
/// оставил.
const SIDE_TABS: [&str; 3] = ["files", "git", "agents"];
/// Закрытого списка вкладок у центра нет и не будет: вкладки файлов приходят
/// из проекта. Поэтому проверяем не принадлежность списку, а вменяемость.
const MAX_ID_LEN: usize = 200;
const MAX_PATH_LEN: usize = 4096;
const MAX_EXPANDED: usize = 500;

/// Внешний вид — про человека и его экран, поэтому общий для всех проектов.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Appearance {
    pub theme: String,
    pub density: String,
}

impl Default for Appearance {
    fn default() -> Self {
        Self { theme: "dark".into(), density: "comfortable".into() }
    }
}

/// Свёрнутость боковых панелей — тоже про экран, а не про содержимое.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Layout {
    pub left_collapsed: bool,
    pub right_collapsed: bool,
}

/// Всё, что относится к содержимому, — под путём проекта. Мультипроекта ещё
/// нет, и запись всегда одна, но форма уже та, в которую он ляжет.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ProjectState {
    pub side_tab: String,
    pub active_tab: String,
    pub selected_task: Option<String>,
    pub selected_path: Option<String>,
    pub expanded: Vec<String>,
    /// RFC 3339, проставляется при записи. Нужен только для обрезки карты.
    pub used_at: Option<String>,
}

impl Default for ProjectState {
    fn default() -> Self {
        Self {
            side_tab: "files".into(),
            active_tab: "kanban".into(),
            selected_task: None,
            selected_path: None,
            expanded: Vec::new(),
            used_at: None,
        }
    }
}

/// Файл целиком.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub version: u32,
    pub appearance: Appearance,
    pub layout: Layout,
    pub last_project: Option<String>,
    pub projects: BTreeMap<String, ProjectState>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            version: CURRENT_VERSION,
            appearance: Appearance::default(),
            layout: Layout::default(),
            last_project: None,
            projects: BTreeMap::new(),
        }
    }
}

/// То, что видит фронт: общее плюс запись текущего проекта. Карты остальных
/// проектов за границу не выходит — фронт про неё не знает.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ResolvedSettings {
    pub appearance: Appearance,
    pub layout: Layout,
    pub project: ProjectState,
}

/// Что вышло из файла.
#[derive(Debug, PartialEq)]
pub enum Outcome {
    Ok(Settings),
    /// Не JSON или не объект — читать нечего.
    Broken,
    /// Файл новее этой сборки: молча уронить чужие поля нельзя.
    TooNew,
}

pub fn parse(text: &str) -> Outcome {
    let Ok(value) = serde_json::from_str::<Value>(text) else {
        return Outcome::Broken;
    };
    let Some(object) = value.as_object() else {
        return Outcome::Broken;
    };

    let version = object.get("version").and_then(Value::as_u64).unwrap_or(0);
    if version > CURRENT_VERSION as u64 {
        return Outcome::TooNew;
    }
    let object = migrate(object.clone(), version);

    let mut settings = Settings {
        version: CURRENT_VERSION,
        appearance: section(&object, "appearance"),
        layout: section(&object, "layout"),
        last_project: object.get("lastProject").and_then(Value::as_str).map(str::to_owned),
        projects: projects(&object),
    };
    settings.validate();
    Outcome::Ok(settings)
}

/// Что отдаём фронту: общее плюс запись текущего проекта или умолчания.
pub fn resolve(file: &Settings, project: &str) -> ResolvedSettings {
    ResolvedSettings {
        appearance: file.appearance.clone(),
        layout: file.layout.clone(),
        project: file.projects.get(project).cloned().unwrap_or_default(),
    }
}

/// Кладёт разрешённый вид обратно в файл. `now` приходит снаружи, чтобы
/// функция осталась чистой и проверяемой.
pub fn merge(file: &mut Settings, mut resolved: ResolvedSettings, project: &str, now: String) {
    resolved.validate();
    file.version = CURRENT_VERSION;
    file.appearance = resolved.appearance;
    file.layout = resolved.layout;
    file.last_project = Some(project.to_owned());

    let mut state = resolved.project;
    state.used_at = Some(now);
    file.projects.insert(project.to_owned(), state);
    trim(&mut file.projects, project);
}

/// Секция читается отдельно от соседей: сломанный тип в одной не должен
/// уносить весь файл.
fn section<T: Default + serde::de::DeserializeOwned>(object: &Map<String, Value>, key: &str) -> T {
    object
        .get(key)
        .cloned()
        .and_then(|value| serde_json::from_value(value).ok())
        .unwrap_or_default()
}

fn projects(object: &Map<String, Value>) -> BTreeMap<String, ProjectState> {
    let Some(map) = object.get("projects").and_then(Value::as_object) else {
        return BTreeMap::new();
    };
    // И каждая запись — тоже сама по себе.
    map.iter()
        .filter_map(|(path, value)| {
            serde_json::from_value::<ProjectState>(value.clone())
                .ok()
                .map(|state| (path.clone(), state))
        })
        .collect()
}

/// Шов для миграций, а не миграция.
///
/// `parse` читает `version`, зовёт эту функцию и сам проставляет
/// `CURRENT_VERSION`. Сегодня обе ветки возвращают объект как есть: файл без
/// версии — это и есть первая схема, полей в нём столько же, и он читается
/// без переделки. Когда схема разойдётся со старой, переписывание полей
/// появится здесь; цепочки шагов пока нет, и делать вид, что она есть, незачем.
fn migrate(object: Map<String, Value>, from: u64) -> Map<String, Value> {
    match from {
        0 | 1 => object,
        // Версии новее текущей `parse` сюда не пускает.
        _ => object,
    }
}

/// Оставляем MAX_PROJECTS самых свежих. Запись без `usedAt` считается самой
/// старой: она из файла, написанного руками, и ей нечем себя защитить.
///
/// `usedAt` сравнивается как момент времени, а не как строка: RFC 3339
/// допускает и `Z`, и `+00:00`, и любое смещение, и лексикографически они
/// выстраиваются не по возрасту ('Z' больше '.', `+03:00` вообще не на месте).
/// Непонятная метка приравнивается к отсутствующей.
///
/// Текущий проект не выселяется никогда — не потому, что его метка самая
/// свежая (это совпадение), а потому, что он исключён из отбора.
fn trim(projects: &mut BTreeMap<String, ProjectState>, current: &str) {
    if projects.len() <= MAX_PROJECTS {
        return;
    }
    let mut ordered: Vec<(String, Option<DateTime<FixedOffset>>)> = projects
        .iter()
        .filter(|(path, _)| path.as_str() != current)
        .map(|(path, state)| {
            let stamp =
                state.used_at.as_deref().and_then(|text| DateTime::parse_from_rfc3339(text).ok());
            (path.clone(), stamp)
        })
        .collect();
    // None меньше любого Some, поэтому по убыванию свежие оказываются впереди.
    ordered.sort_by(|a, b| b.1.cmp(&a.1));
    // Место под текущий проект уже занято, если он в карте есть.
    let keep = if projects.contains_key(current) {
        MAX_PROJECTS.saturating_sub(1)
    } else {
        MAX_PROJECTS
    };
    for (path, _) in ordered.into_iter().skip(keep) {
        projects.remove(&path);
    }
}

impl Settings {
    /// Значение вне допустимого множества — это не повод выбросить файл:
    /// теряет только само поле.
    pub fn validate(&mut self) {
        self.appearance.validate();
        for state in self.projects.values_mut() {
            state.validate();
        }
        if self.last_project.as_deref() == Some("") {
            self.last_project = None;
        }
    }
}

impl ResolvedSettings {
    pub fn validate(&mut self) {
        self.appearance.validate();
        self.project.validate();
    }
}

impl Appearance {
    fn validate(&mut self) {
        one_of(&mut self.theme, &THEMES, "dark");
        one_of(&mut self.density, &DENSITIES, "comfortable");
    }
}

impl ProjectState {
    fn validate(&mut self) {
        one_of(&mut self.side_tab, &SIDE_TABS, "files");
        if self.active_tab.is_empty() || self.active_tab.len() > MAX_ID_LEN {
            self.active_tab = "kanban".into();
        }
        forget_if_junk(&mut self.selected_task, MAX_ID_LEN);
        forget_if_junk(&mut self.selected_path, MAX_PATH_LEN);

        let mut seen = HashSet::new();
        self.expanded
            .retain(|path| !path.is_empty() && path.len() <= MAX_PATH_LEN && seen.insert(path.clone()));
        self.expanded.truncate(MAX_EXPANDED);
    }
}

fn one_of(value: &mut String, allowed: &[&str], fallback: &str) {
    if !allowed.contains(&value.as_str()) {
        *value = fallback.to_owned();
    }
}

/// Пустая строка приходит из фронта как «ничего не выбрано», слишком длинная —
/// как мусор. И то и другое лучше забыть, чем хранить.
fn forget_if_junk(value: &mut Option<String>, max: usize) {
    if let Some(text) = value {
        if text.is_empty() || text.len() > max {
            *value = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings_of(text: &str) -> Settings {
        match parse(text) {
            Outcome::Ok(settings) => settings,
            other => panic!("ожидали Ok, получили {other:?}"),
        }
    }

    #[test]
    fn missing_version_reads_as_the_first_schema() {
        let settings = settings_of(r#"{"appearance":{"theme":"light","density":"compact"}}"#);
        assert_eq!(settings.version, CURRENT_VERSION);
        assert_eq!(settings.appearance.theme, "light");
        assert_eq!(settings.appearance.density, "compact");
    }

    #[test]
    fn newer_version_is_not_read() {
        assert_eq!(parse(r#"{"version":99,"appearance":{"theme":"light"}}"#), Outcome::TooNew);
    }

    #[test]
    fn broken_json_is_not_settings() {
        assert_eq!(parse("{не json"), Outcome::Broken);
        assert_eq!(parse("[1,2,3]"), Outcome::Broken);
    }

    #[test]
    fn unknown_value_falls_back_field_by_field() {
        let settings = settings_of(r#"{"version":1,"appearance":{"theme":"neon","density":"compact"}}"#);
        assert_eq!(settings.appearance.theme, "dark");
        assert_eq!(settings.appearance.density, "compact", "соседнее поле должно уцелеть");
    }

    #[test]
    fn broken_section_does_not_take_the_rest_of_the_file() {
        let settings = settings_of(r#"{"version":1,"appearance":5,"layout":{"leftCollapsed":true}}"#);
        assert_eq!(settings.appearance, Appearance::default());
        assert!(settings.layout.left_collapsed);
    }

    #[test]
    fn unknown_side_tab_falls_back_to_files() {
        let settings = settings_of(r#"{"version":1,"projects":{"/p":{"sideTab":"tarot","activeTab":"chat"}}}"#);
        let state = &settings.projects["/p"];
        assert_eq!(state.side_tab, "files");
        assert_eq!(state.active_tab, "chat", "своя вкладка не из закрытого списка остаётся");
    }

    #[test]
    fn a_broken_project_entry_does_not_take_its_neighbours() {
        let settings = settings_of(r#"{"version":1,"projects":{"/bad":7,"/good":{"sideTab":"agents"}}}"#);
        assert!(!settings.projects.contains_key("/bad"));
        assert_eq!(settings.projects["/good"].side_tab, "agents");
    }

    #[test]
    fn merge_writes_into_the_current_project_and_stamps_it() {
        let mut file = Settings::default();
        let resolved = ResolvedSettings {
            appearance: Appearance { theme: "light".into(), density: "comfortable".into() },
            layout: Layout { left_collapsed: true, right_collapsed: false },
            project: ProjectState {
                selected_task: Some("bd-a1b2".into()),
                ..ProjectState::default()
            },
        };

        merge(&mut file, resolved, "/work/smetana", "2026-08-01T09:12:00+00:00".into());

        assert_eq!(file.version, CURRENT_VERSION);
        assert_eq!(file.appearance.theme, "light");
        assert!(file.layout.left_collapsed);
        assert_eq!(file.last_project.as_deref(), Some("/work/smetana"));
        let state = &file.projects["/work/smetana"];
        assert_eq!(state.selected_task.as_deref(), Some("bd-a1b2"));
        assert_eq!(state.used_at.as_deref(), Some("2026-08-01T09:12:00+00:00"));
    }

    #[test]
    fn a_value_the_front_end_should_not_have_sent_does_not_reach_the_file() {
        let mut file = Settings::default();
        let resolved = ResolvedSettings {
            appearance: Appearance { theme: "neon".into(), density: "comfortable".into() },
            layout: Layout::default(),
            project: ProjectState { side_tab: "tarot".into(), ..ProjectState::default() },
        };

        merge(&mut file, resolved, "/p", "2026-08-01T09:12:00+00:00".into());

        assert_eq!(file.appearance.theme, "dark", "проверка на входе, а не только на выходе");
        assert_eq!(file.projects["/p"].side_tab, "files");
    }

    #[test]
    fn the_expanded_list_loses_blanks_duplicates_and_everything_past_the_limit() {
        let mut paths = vec![String::from("/a"), String::from("/a"), String::new(), "x".repeat(MAX_PATH_LEN + 1)];
        for i in 0..MAX_EXPANDED {
            paths.push(format!("/dir{i:04}"));
        }
        let text = serde_json::json!({"version": 1, "projects": {"/p": {"expanded": paths}}});

        let settings = settings_of(&text.to_string());

        let expanded = &settings.projects["/p"].expanded;
        let last_kept = format!("/dir{:04}", MAX_EXPANDED - 2);
        assert_eq!(expanded.len(), MAX_EXPANDED, "длиннее предела список не хранится");
        assert_eq!(expanded[0], "/a");
        assert_eq!(expanded[1], "/dir0000", "дубль, пустая строка и слишком длинный путь выпали");
        assert_eq!(expanded.last(), Some(&last_kept), "обрезается хвост, а не начало");
    }

    #[test]
    fn merge_keeps_only_the_newest_projects() {
        let mut file = Settings::default();
        for i in 0..MAX_PROJECTS + 5 {
            file.projects.insert(
                format!("/p{i:02}"),
                ProjectState {
                    used_at: Some(format!("2026-01-{:02}T00:00:00+00:00", i + 1)),
                    ..ProjectState::default()
                },
            );
        }

        merge(&mut file, ResolvedSettings::default(), "/p-new", "2026-08-01T00:00:00+00:00".into());

        assert_eq!(file.projects.len(), MAX_PROJECTS);
        assert!(file.projects.contains_key("/p-new"), "текущий проект остаётся всегда");
        assert!(!file.projects.contains_key("/p00"), "самый старый уходит первым");
    }

    #[test]
    fn trim_compares_instants_not_strings_and_never_evicts_the_current_project() {
        let mut projects: BTreeMap<String, ProjectState> = BTreeMap::new();

        // Текущий проект несёт самую старую метку из всех — и всё равно
        // обязан остаться: он исключён из отбора структурно, а не потому,
        // что его дата оказалась наибольшей (как было в старом тесте).
        projects.insert(
            "/current".into(),
            ProjectState { used_at: Some("2000-01-01T00:00:00+00:00".into()), ..ProjectState::default() },
        );

        // Наполнитель — заведомо самые свежие записи, отбору не подлежат
        // при любом способе сравнения; занимают все места, кроме одного.
        let filler_count = MAX_PROJECTS - 2;
        for i in 0..filler_count {
            projects.insert(
                format!("/filler{i:02}"),
                ProjectState {
                    used_at: Some(format!("2026-07-{:02}T00:00:00+00:00", i + 1)),
                    ..ProjectState::default()
                },
            );
        }

        // Momент "/newer-instant" на самом деле позже "/older-instant" —
        // 23:00Z против 22:00Z того же дня по UTC (01:00+03:00 — это и есть
        // 22:00Z). Но как *строка* "2026-05-01..." больше "2026-04-30..." —
        // сравниваются цифры дня, "05" > "04" — и лексикографический
        // порядок расставил бы их наоборот.
        projects.insert(
            "/older-instant".into(),
            ProjectState { used_at: Some("2026-05-01T01:00:00+03:00".into()), ..ProjectState::default() },
        );
        projects.insert(
            "/newer-instant".into(),
            ProjectState { used_at: Some("2026-04-30T23:00:00Z".into()), ..ProjectState::default() },
        );

        assert_eq!(projects.len(), MAX_PROJECTS + 1, "проверка на входе: обрезка должна случиться");

        trim(&mut projects, "/current");

        assert_eq!(projects.len(), MAX_PROJECTS);
        assert!(projects.contains_key("/current"), "текущий проект не выселяется никогда");
        assert!(
            projects.contains_key("/newer-instant"),
            "более поздний момент остаётся, даже когда его строка лексикографически меньше"
        );
        assert!(
            !projects.contains_key("/older-instant"),
            "более ранний момент уходит, даже когда его строка лексикографически больше"
        );
        for i in 0..filler_count {
            let path = format!("/filler{i:02}");
            assert!(projects.contains_key(&path), "заведомо свежие записи не должны были попасть под обрезку");
        }
    }

    #[test]
    fn resolve_gives_defaults_for_an_unknown_project() {
        let file = settings_of(r#"{"version":1,"projects":{"/other":{"sideTab":"agents"}}}"#);
        assert_eq!(resolve(&file, "/mine").project, ProjectState::default());
    }
}
