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
/// Сколько проектов можно держать открытыми. Предел не про вкус, а про то,
/// чтобы список в панели оставался списком, а файл — читаемым.
pub const MAX_OPEN: usize = 50;

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
    /// Состав и порядок списка на экране — порядок добавления, а не свежести:
    /// строки, прыгающие при каждом переключении, читать нельзя.
    pub open_projects: Vec<String>,
    pub projects: BTreeMap<String, ProjectState>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            version: CURRENT_VERSION,
            appearance: Appearance::default(),
            layout: Layout::default(),
            last_project: None,
            open_projects: Vec::new(),
            projects: BTreeMap::new(),
        }
    }
}

/// То, что видит фронт: общее (`appearance`, `layout`), состояние одного
/// проекта (`project`), состав списка открытых (`open_projects`) и то, какой
/// из них активен (`active_project`). Последние два — половина того, что
/// пересекает IPC: список на экране и подсветка строки берутся из них, и они
/// же приходят обратно в `settings_save`, потому что истина по составу списка
/// живёт во фронте. Карта остальных проектов за границу не выходит — фронт
/// про неё не знает.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ResolvedSettings {
    pub appearance: Appearance,
    pub layout: Layout,
    pub project: ProjectState,
    pub open_projects: Vec<String>,
    pub active_project: Option<String>,
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
        open_projects: section(&object, "openProjects"),
        projects: projects(&object),
    };
    adopt_last_project(&mut settings);
    settings.validate();
    Outcome::Ok(settings)
}

/// Файл, написанный до появления списка, знает только `lastProject`. Без этого
/// шага `validate` увидела бы активного вне пустого списка и обнулила его —
/// человек, обновивший приложение, встретил бы пустую панель вместо своего
/// проекта. Версия схемы здесь не помогает: те файлы тоже несут `version: 1`,
/// поэтому решаем по содержимому.
///
/// Только на чтении файла. Пустой список от фронта — это осознанное «я закрыл
/// последний проект», и воскрешать его нельзя: `ResolvedSettings::validate`
/// этой поблажки не знает.
fn adopt_last_project(settings: &mut Settings) {
    if settings.open_projects.is_empty() {
        if let Some(last) = settings.last_project.clone() {
            settings.open_projects.push(last);
        }
    }
}

/// Что отдаём фронту: общее, список открытых и состояние активного проекта.
/// `active` — это «покажи мне вот этот»: так фронт получает состояние другого
/// проекта при переключении, не перезапуская приложение. Без аргумента берём
/// активный из файла.
pub fn resolve(file: &Settings, active: Option<&str>) -> ResolvedSettings {
    let active = active.map(str::to_owned).or_else(|| file.last_project.clone());
    ResolvedSettings {
        appearance: file.appearance.clone(),
        layout: file.layout.clone(),
        project: active
            .as_deref()
            .and_then(|path| file.projects.get(path))
            .cloned()
            .unwrap_or_default(),
        open_projects: file.open_projects.clone(),
        active_project: active,
    }
}

/// Кладёт разрешённый вид обратно в файл. `now` приходит снаружи, чтобы
/// функция осталась чистой и проверяемой.
pub fn merge(file: &mut Settings, mut resolved: ResolvedSettings, now: String) {
    resolved.validate();
    file.version = CURRENT_VERSION;
    file.appearance = resolved.appearance;
    file.layout = resolved.layout;
    file.open_projects = resolved.open_projects;
    file.last_project = resolved.active_project.clone();

    // Закрыли последний проект — состояние писать некому, но список и
    // внешний вид сохранить всё равно надо.
    if let Some(active) = resolved.active_project {
        let mut state = resolved.project;
        state.used_at = Some(now);
        file.projects.insert(active.clone(), state);
        trim(&mut file.projects, Some(&active), &file.open_projects);
    } else {
        trim(&mut file.projects, None, &file.open_projects);
    }
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

/// Оставляем MAX_PROJECTS самых свежих среди тех, кого можно выселять.
/// Запись без `usedAt` считается самой старой: она из файла, написанного
/// руками, и ей нечем себя защитить.
///
/// `usedAt` сравнивается как момент времени, а не как строка: RFC 3339
/// допускает и `Z`, и `+00:00`, и любое смещение, и лексикографически они
/// выстраиваются не по возрасту. Непонятная метка приравнивается к
/// отсутствующей.
///
/// Не выселяются никогда двое: текущий проект и любой открытый. Из-за этого
/// MAX_PROJECTS перестаёт быть жёстким размером карты — она держит всех
/// открытых плюс закрытых, пока общее число не дойдёт до предела. Предел
/// ставился против роста от разовых заходов, а не против того, что человек
/// открыл сам.
fn trim(projects: &mut BTreeMap<String, ProjectState>, current: Option<&str>, open: &[String]) {
    if projects.len() <= MAX_PROJECTS {
        return;
    }
    let protected =
        |path: &str| current == Some(path) || open.iter().any(|p| p == path);

    let mut ordered: Vec<(String, Option<DateTime<FixedOffset>>)> = projects
        .iter()
        .filter(|(path, _)| !protected(path))
        .map(|(path, state)| {
            let stamp =
                state.used_at.as_deref().and_then(|text| DateTime::parse_from_rfc3339(text).ok());
            (path.clone(), stamp)
        })
        .collect();
    // None меньше любого Some, поэтому по убыванию свежие оказываются впереди.
    ordered.sort_by(|a, b| b.1.cmp(&a.1));

    // Места, занятые неприкосновенными, уже потрачены.
    let taken = projects.len() - ordered.len();
    let keep = MAX_PROJECTS.saturating_sub(taken);
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
        sane_paths(&mut self.open_projects, MAX_OPEN);
        active_in(&mut self.last_project, &self.open_projects);
    }
}

impl ResolvedSettings {
    pub fn validate(&mut self) {
        self.appearance.validate();
        self.project.validate();
        sane_paths(&mut self.open_projects, MAX_OPEN);
        active_in(&mut self.active_project, &self.open_projects);
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

        sane_paths(&mut self.expanded, MAX_EXPANDED);
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

/// Список путей — из файла или из фронта. Пустые строки и слишком длинные
/// пути мусор, дубли бессмысленны, а длина ограничена.
fn sane_paths(paths: &mut Vec<String>, max: usize) {
    let mut seen = HashSet::new();
    paths.retain(|path| !path.is_empty() && path.len() <= MAX_PATH_LEN && seen.insert(path.clone()));
    paths.truncate(max);
}

/// Активный проект обязан быть в списке открытых: иначе доска показывала бы
/// то, чего в списке нет, и ни одна строка не была бы подсвечена. Пустой
/// список — это законное «проектов нет», а не поломка.
fn active_in(active: &mut Option<String>, open: &[String]) {
    let known = active.as_deref().is_some_and(|path| open.iter().any(|p| p == path));
    if !known {
        *active = open.first().cloned();
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
            open_projects: vec!["/work/smetana".into()],
            active_project: Some("/work/smetana".into()),
        };

        merge(&mut file, resolved, "2026-08-01T09:12:00+00:00".into());

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
            open_projects: vec!["/p".into()],
            active_project: Some("/p".into()),
        };

        merge(&mut file, resolved, "2026-08-01T09:12:00+00:00".into());

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

        let resolved = ResolvedSettings {
            open_projects: vec!["/p-new".into()],
            active_project: Some("/p-new".into()),
            ..ResolvedSettings::default()
        };
        merge(&mut file, resolved, "2026-08-01T00:00:00+00:00".into());

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

        trim(&mut projects, Some("/current"), &[]);

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
        assert_eq!(resolve(&file, Some("/mine")).project, ProjectState::default());
    }

    #[test]
    fn open_projects_lose_blanks_duplicates_and_overlong_paths() {
        let long = "x".repeat(MAX_PATH_LEN + 1);
        let text = serde_json::json!({
            "version": 1,
            "openProjects": ["/a", "/a", "", long, "/b"],
            "lastProject": "/a"
        });

        let settings = settings_of(&text.to_string());

        assert_eq!(settings.open_projects, vec!["/a".to_string(), "/b".to_string()]);
    }

    #[test]
    fn the_active_project_has_to_be_in_the_list() {
        let settings = settings_of(r#"{"version":1,"openProjects":["/a","/b"],"lastProject":"/gone"}"#);
        assert_eq!(settings.last_project.as_deref(), Some("/a"), "чужой активный заменяется первым из списка");

        let settings = settings_of(r#"{"version":1,"openProjects":["/a","/b"]}"#);
        assert_eq!(settings.last_project.as_deref(), Some("/a"), "список есть, активного нет — берём первый");
    }

    #[test]
    fn an_empty_list_leaves_the_app_without_an_active_project() {
        // Так выглядит файл, записанный после закрытия последнего проекта:
        // список пуст и активного нет. Воскрешать нечего.
        let settings = settings_of(r#"{"version":1,"openProjects":[],"lastProject":null}"#);
        assert_eq!(settings.last_project, None);
        assert!(settings.open_projects.is_empty());
    }

    #[test]
    fn a_file_from_before_the_list_keeps_the_project_it_remembered() {
        // Файл, написанный до этой ветки: lastProject есть, openProjects нет.
        let settings = settings_of(r#"{"version":1,"lastProject":"/work/smetana"}"#);
        assert_eq!(settings.open_projects, vec!["/work/smetana".to_string()], "иначе панель пуста");
        assert_eq!(settings.last_project.as_deref(), Some("/work/smetana"));

        // Тот же случай, но список записан пустым — по содержимому он
        // неотличим, и версия схемы (тоже 1) здесь ничего не подсказывает.
        let settings = settings_of(r#"{"version":1,"openProjects":[],"lastProject":"/work/smetana"}"#);
        assert_eq!(settings.open_projects, vec!["/work/smetana".to_string()]);
        assert_eq!(settings.last_project.as_deref(), Some("/work/smetana"));
    }

    #[test]
    fn an_empty_list_from_the_front_end_is_not_resurrected() {
        // Поблажка старым файлам живёт только на чтении файла. Фронт, закрывший
        // последний проект, шлёт пустой список — и он обязан таким остаться,
        // иначе удаление последней строки отменялось бы само.
        let mut file = Settings::default();
        let resolved = ResolvedSettings {
            open_projects: Vec::new(),
            active_project: Some("/work/smetana".into()),
            ..ResolvedSettings::default()
        };

        merge(&mut file, resolved, "2026-08-01T09:12:00+00:00".into());

        assert!(file.open_projects.is_empty());
        assert_eq!(file.last_project, None, "активного вне списка не бывает");
        assert!(file.projects.is_empty(), "состояние писать некому");
    }

    #[test]
    fn merge_writes_the_list_and_the_active_project() {
        let mut file = Settings::default();
        let resolved = ResolvedSettings {
            open_projects: vec!["/one".into(), "/two".into()],
            active_project: Some("/two".into()),
            project: ProjectState { selected_task: Some("bd-a1b2".into()), ..ProjectState::default() },
            ..ResolvedSettings::default()
        };

        merge(&mut file, resolved, "2026-08-01T09:12:00+00:00".into());

        assert_eq!(file.open_projects, vec!["/one".to_string(), "/two".to_string()]);
        assert_eq!(file.last_project.as_deref(), Some("/two"));
        assert_eq!(file.projects["/two"].selected_task.as_deref(), Some("bd-a1b2"));
        assert!(!file.projects.contains_key("/one"), "состояние пишется только для активного проекта");
    }

    #[test]
    fn merge_without_an_active_project_writes_no_state() {
        let mut file = Settings::default();

        merge(&mut file, ResolvedSettings::default(), "2026-08-01T09:12:00+00:00".into());

        assert_eq!(file.last_project, None);
        assert!(file.projects.is_empty(), "закрыли последний проект — писать состояние некому");
    }

    #[test]
    fn trim_never_evicts_an_open_project() {
        let mut file = Settings::default();
        // Самая старая запись из всех — и при этом открытая.
        file.projects.insert(
            "/open-and-ancient".into(),
            ProjectState { used_at: Some("2000-01-01T00:00:00+00:00".into()), ..ProjectState::default() },
        );
        for i in 0..MAX_PROJECTS + 5 {
            file.projects.insert(
                format!("/p{i:02}"),
                ProjectState {
                    used_at: Some(format!("2026-01-{:02}T00:00:00+00:00", i + 1)),
                    ..ProjectState::default()
                },
            );
        }

        let resolved = ResolvedSettings {
            open_projects: vec!["/open-and-ancient".into(), "/current".into()],
            active_project: Some("/current".into()),
            ..ResolvedSettings::default()
        };
        merge(&mut file, resolved, "2026-08-01T00:00:00+00:00".into());

        assert!(file.projects.contains_key("/open-and-ancient"), "открытый проект не выселяется, как бы стар ни был");
        assert!(file.projects.contains_key("/current"));
        assert!(!file.projects.contains_key("/p00"), "закрытые проекты уходят от самого старого");
        assert!(file.projects.len() <= MAX_PROJECTS.max(2));
    }

    #[test]
    fn resolve_carries_the_list_and_the_state_of_the_asked_project() {
        let file = settings_of(
            r#"{"version":1,"openProjects":["/a","/b"],"lastProject":"/a",
                "projects":{"/b":{"sideTab":"agents"}}}"#,
        );

        let view = resolve(&file, Some("/b"));
        assert_eq!(view.active_project.as_deref(), Some("/b"));
        assert_eq!(view.project.side_tab, "agents");
        assert_eq!(view.open_projects, vec!["/a".to_string(), "/b".to_string()]);

        let view = resolve(&file, None);
        assert_eq!(view.active_project.as_deref(), Some("/a"), "без аргумента — активный из файла");
        assert_eq!(view.project, ProjectState::default());
    }
}
