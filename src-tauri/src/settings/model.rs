//! App settings: types, defaults, file parsing and merging.
//!
//! No I/O here: everything that depends on the disk lives in `file.rs`.
//! That is why this file is the one carrying the tests — same as `tracker/store.rs`.

use std::collections::{BTreeMap, HashSet};

use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// The file's schema version. It grows when an old file can no longer be read as is.
pub const CURRENT_VERSION: u32 = 1;
/// How many projects we remember: the map must not grow forever from one-off visits.
pub const MAX_PROJECTS: usize = 20;
/// How many projects may stay open. The limit is not about taste but about
/// keeping the list in the panel a list, and the file readable.
pub const MAX_OPEN: usize = 50;

const THEMES: [&str; 2] = ["dark", "light"];
const DENSITIES: [&str; 2] = ["comfortable", "compact"];
/// A closed list — and it is duplicated on the other side of the IPC: the same
/// three tabs are listed in `src/views/DesktopApp.vue` (the `SIDE_TABS`
/// constant). Change one list and you must change the other: a value missing
/// here silently becomes "files" on its way to disk, and after a restart a
/// person sees something other than what they left.
const SIDE_TABS: [&str; 3] = ["files", "git", "agents"];
/// The centre has no closed list of tabs and never will: file tabs come from
/// the project. So we check sanity rather than membership.
const MAX_ID_LEN: usize = 200;
const MAX_PATH_LEN: usize = 4096;
const MAX_EXPANDED: usize = 500;
/// How many file tabs we remember. The limit is not about taste but about
/// keeping the tab row a row, and the settings file readable.
pub const MAX_OPEN_TABS: usize = 50;
/// How many columns an order may name. bd ships eleven statuses and a project
/// adds custom ones; the cap is generous by that measure and only there to stop
/// a garbage list from growing without bound.
const MAX_COLUMNS: usize = 60;

/// Appearance is about the person and their screen, hence shared by all projects.
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

/// Collapsed state and width of the side panels — also about the screen, not
/// about content.
///
/// The stored width is the one a person dragged to, not the one that fitted the
/// window: fitting it to the current window is the front end's job
/// (`src/views/panelWidths.js`), and a narrow window must not rewrite a
/// preference. Hence the generous ceiling in the check below: it catches
/// garbage, not tightness.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Layout {
    pub left_collapsed: bool,
    pub right_collapsed: bool,
    pub left_width: u32,
    pub right_width: u32,
}

/// The defaults are repeated in the front end (`LEFT_DEFAULT`, `RIGHT_DEFAULT`
/// in `panelWidths.js`): with no back end the app must still open looking the same.
pub const LEFT_WIDTH_DEFAULT: u32 = 252;
pub const RIGHT_WIDTH_DEFAULT: u32 = 340;
/// Sanity bounds, not layout ones. A zero would come from a panel collapsed
/// into a rail, and after a restart it would expand into nothing; the upper
/// bound cuts off accidental garbage without getting in a wide monitor's way.
const MIN_PANEL_WIDTH: u32 = 120;
const MAX_PANEL_WIDTH: u32 = 4000;

impl Default for Layout {
    fn default() -> Self {
        Self {
            left_collapsed: false,
            right_collapsed: false,
            left_width: LEFT_WIDTH_DEFAULT,
            right_width: RIGHT_WIDTH_DEFAULT,
        }
    }
}

/// Everything to do with content sits under the project's path. There is no
/// multi-project yet and the entry is always one, but the shape is already the
/// one it will take.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ProjectState {
    pub side_tab: String,
    pub active_tab: String,
    pub selected_task: Option<String>,
    pub selected_path: Option<String>,
    pub expanded: Vec<String>,
    /// Open files in tab order. Paths are relative to the project root: the
    /// project's key in the map is already absolute, and duplicating it in
    /// every tab is pointless — it also means a moved folder does not turn the
    /// list into rubbish.
    pub open_tabs: Vec<String>,
    /// Which of them is temporary. There is always exactly one, and it is never
    /// dirty: an edit drops the temporary flag at the same moment it adds the dot.
    pub preview_tab: Option<String>,
    /// The board's columns in the order they were dragged into, named by the
    /// front end's status vocabulary. Per project because the set of statuses
    /// is: bd carries custom ones, and one repository's status has no meaning
    /// in another one's order.
    ///
    /// A hint rather than a truth — bd still owns which columns exist. A name
    /// here that bd no longer has costs nothing and is not pruned, so a status
    /// that comes back finds its place again.
    pub column_order: Vec<String>,
    /// What the run dialog was last set to here. `None` until somebody opens
    /// it, which is every settings file written before this existed.
    ///
    /// Per project, for the same reason `column_order` is: a branch name has no
    /// meaning in another repository. And **without the scope** — that comes
    /// from whichever play button was pressed, and remembering it would open
    /// the dialog claiming to run something nobody clicked.
    pub run_settings: Option<RunDefaults>,
    /// RFC 3339, stamped on write. Needed only for trimming the map.
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
            open_tabs: Vec::new(),
            preview_tab: None,
            column_order: Vec::new(),
            run_settings: None,
            used_at: None,
        }
    }
}

/// What the run dialog opens on next time. A mirror of `runs::model::RunSettings`
/// minus the scope, and deliberately its own type rather than a reuse: this one
/// lives in a file people edit by hand and has to tolerate anything, while the
/// other crosses the IPC boundary and must not.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct RunDefaults {
    pub mode: String,
    pub target_branch: Option<String>,
    /// `None` is "no floor was ever chosen here", and it is a different thing
    /// from any number — an `Option` for exactly the reason `RunSettings`'
    /// own field is one, which is what makes this a mirror rather than a
    /// near-mirror. A run aimed at a task or an epic sends no floor at all;
    /// stored as a 2 it would come back as a choice nobody made, and the queue
    /// dialog would open on it instead of on the project's own
    /// `[defaults] min_priority` — quietly overriding the file with a number
    /// that only ever came from this field's default.
    ///
    /// Left out of the file entirely when there is none, which is the one place
    /// in this schema where that matters. Every change to it so far has been
    /// additive, and an unknown key is ignored — but this field changed *type*,
    /// and a build older than this one reads `u8` here. A present `null` is not
    /// a missing field, so `#[serde(default)]` would not rescue it: the whole
    /// `RunDefaults` would fail, taking `ProjectState` with it, and `projects()`
    /// drops an entry it cannot parse — that project's side tab, open tabs,
    /// expanded folders and selection, gone without a word. The version is
    /// deliberately not bumped for this, which would cost every project's
    /// entry rather than one. Absent, an older build simply takes its own
    /// default, and nothing in the current front end can tell absent from null.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_priority: Option<u8>,
    pub live_check: bool,
    pub file_findings: bool,
}

impl Default for RunDefaults {
    fn default() -> Self {
        Self {
            mode: "auto".into(),
            target_branch: None,
            min_priority: None,
            live_check: true,
            file_findings: true,
        }
    }
}

/// The three the dialog offers. Written out here as well as in
/// `runs::model::RunMode` — the one place this codebase accepts that
/// duplication, and for the reason recorded on `IDS`: what crosses the IPC
/// boundary must refuse an unknown value, while what comes off somebody's disk
/// must survive one. `solo` is in the list; whether it is allowed for the scope
/// in front of you is `RunSettings::validate`'s answer, not this file's.
const RUN_MODES: [&str; 3] = ["auto", "supervised", "solo"];

/// bd's priority scale. Anything outside it would silently take everything or
/// nothing.
const MAX_PRIORITY: u8 = 4;

/// The whole file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub version: u32,
    pub appearance: Appearance,
    pub layout: Layout,
    /// Which CLI agent the app starts. A habit of the person's, not a property
    /// of the repository, so it sits at the root rather than under a project.
    ///
    /// The set of legal values is `agents::IDS` and is not repeated here: the
    /// side-tab list is already written out twice — in this file and in
    /// `src/views/DesktopApp.vue` — and a value missing from one of them comes
    /// back after a restart as something the person did not choose.
    pub agent: String,
    pub last_project: Option<String>,
    /// The contents and order of the on-screen list — the order things were
    /// added, not how recent they are: rows that jump on every switch are
    /// unreadable.
    pub open_projects: Vec<String>,
    pub projects: BTreeMap<String, ProjectState>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            version: CURRENT_VERSION,
            appearance: Appearance::default(),
            layout: Layout::default(),
            agent: "claude".into(),
            last_project: None,
            open_projects: Vec::new(),
            projects: BTreeMap::new(),
        }
    }
}

/// What the front end sees: the shared parts (`appearance`, `layout`), one
/// project's state (`project`), the contents of the open list (`open_projects`)
/// and which of them is active (`active_project`). The last two are half of
/// what crosses the IPC: the on-screen list and the highlighted row come from
/// them, and they come back through `settings_save`, because the truth about
/// the list's contents lives in the front end. The map of the remaining
/// projects never crosses the boundary — the front end knows nothing about it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ResolvedSettings {
    pub appearance: Appearance,
    pub layout: Layout,
    /// Which CLI agent the app starts. See `Settings::agent`.
    pub agent: String,
    pub project: ProjectState,
    pub open_projects: Vec<String>,
    pub active_project: Option<String>,
}

/// Written out rather than derived for the sake of one field: the derived
/// `Default` gave `agent` the empty string, which is not an agent and
/// contradicts the `"claude"` that `Settings` beside it defaults to. Nothing
/// reachable read that value — `resolve` always copies the file's and `merge`
/// validates before writing — but a default that disagrees with its own file
/// is a trap for the next person to lean on it.
impl Default for ResolvedSettings {
    fn default() -> Self {
        Self {
            appearance: Appearance::default(),
            layout: Layout::default(),
            agent: "claude".into(),
            project: ProjectState::default(),
            open_projects: Vec::new(),
            active_project: None,
        }
    }
}

/// What came out of the file.
#[derive(Debug, PartialEq)]
pub enum Outcome {
    Ok(Settings),
    /// Not JSON, or not an object — there is nothing to read.
    Broken,
    /// The file is newer than this build: silently dropping someone else's
    /// fields is not allowed.
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
        agent: object.get("agent").and_then(Value::as_str).map(str::to_owned).unwrap_or_else(|| "claude".into()),
        last_project: object.get("lastProject").and_then(Value::as_str).map(str::to_owned),
        open_projects: section(&object, "openProjects"),
        projects: projects(&object),
    };
    adopt_last_project(&mut settings);
    settings.validate();
    Outcome::Ok(settings)
}

/// A file written before the list existed knows only `lastProject`. Without
/// this step `validate` would see an active project outside an empty list and
/// clear it — someone who just updated the app would meet an empty panel
/// instead of their project. The schema version does not help here: those files
/// carry `version: 1` too, so the decision goes by content.
///
/// On file reads only. An empty list from the front end is a deliberate "I
/// closed the last project", and it must not be resurrected:
/// `ResolvedSettings::validate` knows nothing of this leniency.
fn adopt_last_project(settings: &mut Settings) {
    if settings.open_projects.is_empty() {
        if let Some(last) = settings.last_project.clone() {
            settings.open_projects.push(last);
        }
    }
}

/// What we hand the front end: the shared parts, the open list and the active
/// project's state. `active` means "show me this one": that is how the front
/// end gets another project's state when switching, without restarting the app.
/// With no argument we take the active project from the file.
pub fn resolve(file: &Settings, active: Option<&str>) -> ResolvedSettings {
    let active = active.map(str::to_owned).or_else(|| file.last_project.clone());
    ResolvedSettings {
        appearance: file.appearance.clone(),
        layout: file.layout.clone(),
        agent: file.agent.clone(),
        project: active
            .as_deref()
            .and_then(|path| file.projects.get(path))
            .cloned()
            .unwrap_or_default(),
        open_projects: file.open_projects.clone(),
        active_project: active,
    }
}

/// Puts the resolved view back into the file. `now` comes from outside so the
/// function stays pure and testable.
pub fn merge(file: &mut Settings, mut resolved: ResolvedSettings, now: String) {
    resolved.validate();
    file.version = CURRENT_VERSION;
    file.appearance = resolved.appearance;
    file.layout = resolved.layout;
    file.agent = resolved.agent;
    file.open_projects = resolved.open_projects;
    file.last_project = resolved.active_project.clone();

    // The last project was closed — there is nobody to write state for, but the
    // list and the appearance still have to be saved.
    if let Some(active) = resolved.active_project {
        let mut state = resolved.project;
        state.used_at = Some(now);
        file.projects.insert(active.clone(), state);
        trim(&mut file.projects, Some(&active), &file.open_projects);
    } else {
        trim(&mut file.projects, None, &file.open_projects);
    }
}

/// A section is read independently of its neighbours: a broken type in one must
/// not take the whole file down.
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
    // And every entry stands on its own too.
    map.iter()
        .filter_map(|(path, value)| {
            serde_json::from_value::<ProjectState>(value.clone())
                .ok()
                .map(|state| (path.clone(), state))
        })
        .collect()
}

/// A seam for migrations, not a migration.
///
/// `parse` reads `version`, calls this function and stamps `CURRENT_VERSION`
/// itself. Today both arms return the object as is: a file with no version is
/// the first schema, it has the same fields, and it reads without rework. When
/// the schema does diverge from the old one, the field rewriting will appear
/// here; there is no chain of steps yet, and pretending there is would be idle.
fn migrate(object: Map<String, Value>, from: u64) -> Map<String, Value> {
    match from {
        0 | 1 => object,
        // `parse` never lets versions newer than the current one reach here.
        _ => object,
    }
}

/// Keeps the MAX_PROJECTS most recent among those that may be evicted. An entry
/// with no `usedAt` counts as the oldest: it comes from a hand-written file and
/// has nothing to defend itself with.
///
/// `usedAt` is compared as an instant, not as a string: RFC 3339 allows `Z`,
/// `+00:00` and any offset, and lexicographically they do not line up by age.
/// A mark that cannot be parsed is treated as missing.
///
/// Two kinds are never evicted: the current project and any open one. Because
/// of that MAX_PROJECTS stops being a hard size for the map — it holds every
/// open project plus closed ones until the total reaches the limit. The limit
/// was put there against growth from one-off visits, not against what a person
/// opened themselves.
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
    // None is less than any Some, so sorting descending puts the recent ones first.
    ordered.sort_by(|a, b| b.1.cmp(&a.1));

    // The slots taken by the untouchable entries are already spent.
    let taken = projects.len() - ordered.len();
    let keep = MAX_PROJECTS.saturating_sub(taken);
    for (path, _) in ordered.into_iter().skip(keep) {
        projects.remove(&path);
    }
}

impl Settings {
    /// A value outside the allowed set is no reason to throw the file away:
    /// only the field itself is lost.
    pub fn validate(&mut self) {
        one_of(&mut self.agent, &crate::agents::IDS, "claude");
        self.appearance.validate();
        self.layout.validate();
        for state in self.projects.values_mut() {
            state.validate();
        }
        sane_list(&mut self.open_projects, MAX_OPEN, MAX_PATH_LEN);
        active_in(&mut self.last_project, &self.open_projects);
    }
}

impl ResolvedSettings {
    pub fn validate(&mut self) {
        one_of(&mut self.agent, &crate::agents::IDS, "claude");
        self.appearance.validate();
        self.layout.validate();
        self.project.validate();
        sane_list(&mut self.open_projects, MAX_OPEN, MAX_PATH_LEN);
        active_in(&mut self.active_project, &self.open_projects);
    }
}

impl RunDefaults {
    fn validate(&mut self) {
        one_of(&mut self.mode, &RUN_MODES, "auto");
        forget_if_junk(&mut self.target_branch, MAX_PATH_LEN);
        // Out of range is forgotten rather than clamped: a 9 in this field is
        // not somebody meaning "the lowest priority", it is a file that has
        // been edited wrongly, and guessing which way they meant it is how a
        // run silently takes work nobody wanted taken. Forgotten rather than
        // replaced with a number of ours, too — with no floor remembered the
        // dialog falls back to the project's own configured default, which is
        // a real answer, where a 2 from here would be an invention.
        if self.min_priority.is_some_and(|floor| floor > MAX_PRIORITY) {
            self.min_priority = None;
        }
    }
}

impl Appearance {
    fn validate(&mut self) {
        one_of(&mut self.theme, &THEMES, "dark");
        one_of(&mut self.density, &DENSITIES, "comfortable");
    }
}

impl Layout {
    fn validate(&mut self) {
        in_range(&mut self.left_width, LEFT_WIDTH_DEFAULT);
        in_range(&mut self.right_width, RIGHT_WIDTH_DEFAULT);
    }
}

/// A value out of range loses the field and takes the default — the same rule
/// as `one_of`: one field is damaged, not the whole section.
fn in_range(value: &mut u32, fallback: u32) {
    if !(MIN_PANEL_WIDTH..=MAX_PANEL_WIDTH).contains(value) {
        *value = fallback;
    }
}

impl ProjectState {
    fn validate(&mut self) {
        one_of(&mut self.side_tab, &SIDE_TABS, "files");
        forget_if_junk(&mut self.selected_task, MAX_ID_LEN);
        forget_if_junk(&mut self.selected_path, MAX_PATH_LEN);
        sane_list(&mut self.expanded, MAX_EXPANDED, MAX_PATH_LEN);
        sane_list(&mut self.open_tabs, MAX_OPEN_TABS, MAX_PATH_LEN);
        // A status name, not a path — hence the identifier ceiling. Membership
        // is deliberately not checked: bd's set of statuses is not known here,
        // and a name that matches nothing is passed over by the board anyway.
        sane_list(&mut self.column_order, MAX_COLUMNS, MAX_ID_LEN);
        if let Some(run) = self.run_settings.as_mut() {
            run.validate();
        }

        // A preview tab that is not among the open ones cannot exist: it would
        // be drawn in italics over nothing, or replaced in a slot that is not there.
        if self.preview_tab.as_deref().is_some_and(|p| !self.open_tabs.iter().any(|t| t == p)) {
            self.preview_tab = None;
        }

        // There is no closed list of tabs in the centre: `terminal` and `kanban`
        // always exist, everything else is an open file. Hence the limit is on
        // path length, not identifier length — a file tab with a long path used
        // to silently become the board on every restart.
        //
        // `chat` was this tab's name before it grew a terminal. Files with that
        // name already sit on people's disks, and without the substitution the
        // tab would fail the check below and silently become the board.
        //
        // Only when nothing open is called `chat`, though: a project with a
        // file of that name at its root has an active tab that means exactly
        // what it says, and migrating it would take a person off their own
        // file. An open tab is evidence; the old name is only a guess.
        if self.active_tab == "chat" && !self.open_tabs.iter().any(|t| t == "chat") {
            self.active_tab = "terminal".into();
        }
        let known = self.active_tab == "terminal"
            || self.active_tab == "kanban"
            || self.open_tabs.iter().any(|t| *t == self.active_tab);
        if !known || self.active_tab.len() > MAX_PATH_LEN {
            self.active_tab = "kanban".into();
        }
    }
}

fn one_of(value: &mut String, allowed: &[&str], fallback: &str) {
    if !allowed.contains(&value.as_str()) {
        *value = fallback.to_owned();
    }
}

/// An empty string arrives from the front end as "nothing is selected", an
/// overlong one as garbage. Both are better forgotten than kept.
fn forget_if_junk(value: &mut Option<String>, max: usize) {
    if let Some(text) = value {
        if text.is_empty() || text.len() > max {
            *value = None;
        }
    }
}

/// A list of names — paths, statuses — from the file or from the front end.
/// Empty strings and overlong ones are garbage, duplicates are pointless, and
/// the length is capped. `max_item` differs by what the list holds: a path may
/// legitimately be thousands of characters, a status name may not.
fn sane_list(items: &mut Vec<String>, max: usize, max_item: usize) {
    let mut seen = HashSet::new();
    items.retain(|item| !item.is_empty() && item.len() <= max_item && seen.insert(item.clone()));
    items.truncate(max);
}

/// The active project must be in the open list: otherwise the board would show
/// something the list does not contain, and no row would be highlighted. An
/// empty list is a legitimate "there are no projects", not a breakage.
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
            other => panic!("expected Ok, got {other:?}"),
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
        assert_eq!(parse("{not json"), Outcome::Broken);
        assert_eq!(parse("[1,2,3]"), Outcome::Broken);
    }

    #[test]
    fn unknown_value_falls_back_field_by_field() {
        let settings = settings_of(r#"{"version":1,"appearance":{"theme":"neon","density":"compact"}}"#);
        assert_eq!(settings.appearance.theme, "dark");
        assert_eq!(settings.appearance.density, "compact", "the neighbouring field must survive");
    }

    #[test]
    fn broken_section_does_not_take_the_rest_of_the_file() {
        let settings = settings_of(r#"{"version":1,"appearance":5,"layout":{"leftCollapsed":true}}"#);
        assert_eq!(settings.appearance, Appearance::default());
        assert!(settings.layout.left_collapsed);
    }

    #[test]
    fn a_file_without_widths_opens_at_the_shipped_ones() {
        // A file written before the panels learned to be dragged.
        let settings = settings_of(r#"{"version":1,"layout":{"leftCollapsed":true}}"#);
        assert!(settings.layout.left_collapsed);
        assert_eq!(settings.layout.left_width, LEFT_WIDTH_DEFAULT);
        assert_eq!(settings.layout.right_width, RIGHT_WIDTH_DEFAULT);
    }

    #[test]
    fn a_width_out_of_range_loses_only_itself() {
        let settings = settings_of(r#"{"version":1,"layout":{"leftWidth":0,"rightWidth":300}}"#);
        assert_eq!(settings.layout.left_width, LEFT_WIDTH_DEFAULT);
        assert_eq!(settings.layout.right_width, 300, "the neighbouring field must survive");

        let huge = settings_of(r#"{"version":1,"layout":{"leftWidth":999999}}"#);
        assert_eq!(huge.layout.left_width, LEFT_WIDTH_DEFAULT);
    }

    #[test]
    fn a_width_wider_than_any_window_survives_the_trip() {
        // The check catches garbage, not tightness: the front end fits the width
        // to the window, and a number that does not fit today must survive to
        // reach a wide monitor.
        let settings = settings_of(r#"{"version":1,"layout":{"leftWidth":1200}}"#);
        assert_eq!(settings.layout.left_width, 1200);
    }

    #[test]
    fn unknown_side_tab_falls_back_to_files() {
        let settings = settings_of(r#"{"version":1,"projects":{"/p":{"sideTab":"tarot","activeTab":"terminal"}}}"#);
        let state = &settings.projects["/p"];
        assert_eq!(state.side_tab, "files");
        assert_eq!(state.active_tab, "terminal", "a tab outside the closed list stays");
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
            layout: Layout { left_collapsed: true, left_width: 420, ..Layout::default() },
            agent: "claude".into(),
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
        assert_eq!(file.layout.left_width, 420);
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
            agent: "claude".into(),
            project: ProjectState { side_tab: "tarot".into(), ..ProjectState::default() },
            open_projects: vec!["/p".into()],
            active_project: Some("/p".into()),
        };

        merge(&mut file, resolved, "2026-08-01T09:12:00+00:00".into());

        assert_eq!(file.appearance.theme, "dark", "validated on the way in, not only on the way out");
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
        assert_eq!(expanded.len(), MAX_EXPANDED, "the list is not stored past the limit");
        assert_eq!(expanded[0], "/a");
        assert_eq!(expanded[1], "/dir0000", "the duplicate, the empty string and the overlong path fell out");
        assert_eq!(expanded.last(), Some(&last_kept), "the tail is trimmed, not the head");
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
        assert!(file.projects.contains_key("/p-new"), "the current project always stays");
        assert!(!file.projects.contains_key("/p00"), "the oldest goes first");
    }

    #[test]
    fn trim_compares_instants_not_strings_and_never_evicts_the_current_project() {
        let mut projects: BTreeMap<String, ProjectState> = BTreeMap::new();

        // The current project carries the oldest mark of all — and still has to
        // stay: it is excluded from the selection structurally, not because its
        // date happened to be the largest (as it was in the old test).
        projects.insert(
            "/current".into(),
            ProjectState { used_at: Some("2000-01-01T00:00:00+00:00".into()), ..ProjectState::default() },
        );

        // Filler — deliberately the most recent entries, not eligible for
        // eviction under any comparison; they take every slot but one.
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

        // The instant "/newer-instant" really is later than "/older-instant" —
        // 23:00Z against 22:00Z of the same UTC day (01:00+03:00 is exactly
        // 22:00Z). But as a *string* "2026-05-01..." is greater than
        // "2026-04-30..." — the day digits compare, "05" > "04" — and
        // lexicographic order would line them up the other way round.
        projects.insert(
            "/older-instant".into(),
            ProjectState { used_at: Some("2026-05-01T01:00:00+03:00".into()), ..ProjectState::default() },
        );
        projects.insert(
            "/newer-instant".into(),
            ProjectState { used_at: Some("2026-04-30T23:00:00Z".into()), ..ProjectState::default() },
        );

        assert_eq!(projects.len(), MAX_PROJECTS + 1, "check on the way in: trimming must happen");

        trim(&mut projects, Some("/current"), &[]);

        assert_eq!(projects.len(), MAX_PROJECTS);
        assert!(projects.contains_key("/current"), "the current project is never evicted");
        assert!(
            projects.contains_key("/newer-instant"),
            "the later instant stays even when its string is lexicographically smaller"
        );
        assert!(
            !projects.contains_key("/older-instant"),
            "the earlier instant goes even when its string is lexicographically larger"
        );
        for i in 0..filler_count {
            let path = format!("/filler{i:02}");
            assert!(projects.contains_key(&path), "deliberately recent entries should not have been trimmed");
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
        assert_eq!(settings.last_project.as_deref(), Some("/a"), "an active project outside the list is replaced by the first one");

        let settings = settings_of(r#"{"version":1,"openProjects":["/a","/b"]}"#);
        assert_eq!(settings.last_project.as_deref(), Some("/a"), "a list with no active project — take the first");
    }

    #[test]
    fn an_empty_list_leaves_the_app_without_an_active_project() {
        // This is what a file written after the last project was closed looks
        // like: the list is empty and there is no active project. Nothing to resurrect.
        let settings = settings_of(r#"{"version":1,"openProjects":[],"lastProject":null}"#);
        assert_eq!(settings.last_project, None);
        assert!(settings.open_projects.is_empty());
    }

    #[test]
    fn a_file_from_before_the_list_keeps_the_project_it_remembered() {
        // A file written before this branch: lastProject is there, openProjects is not.
        let settings = settings_of(r#"{"version":1,"lastProject":"/work/smetana"}"#);
        assert_eq!(settings.open_projects, vec!["/work/smetana".to_string()], "otherwise the panel is empty");
        assert_eq!(settings.last_project.as_deref(), Some("/work/smetana"));

        // The same case, but the list was written empty — by content it is
        // indistinguishable, and the schema version (also 1) says nothing here.
        let settings = settings_of(r#"{"version":1,"openProjects":[],"lastProject":"/work/smetana"}"#);
        assert_eq!(settings.open_projects, vec!["/work/smetana".to_string()]);
        assert_eq!(settings.last_project.as_deref(), Some("/work/smetana"));
    }

    #[test]
    fn an_empty_list_from_the_front_end_is_not_resurrected() {
        // The leniency towards old files lives on the file-reading side only. A
        // front end that closed the last project sends an empty list — and it
        // has to stay that way, otherwise removing the last row would undo itself.
        let mut file = Settings::default();
        let resolved = ResolvedSettings {
            open_projects: Vec::new(),
            active_project: Some("/work/smetana".into()),
            ..ResolvedSettings::default()
        };

        merge(&mut file, resolved, "2026-08-01T09:12:00+00:00".into());

        assert!(file.open_projects.is_empty());
        assert_eq!(file.last_project, None, "there is no active project outside the list");
        assert!(file.projects.is_empty(), "there is nobody to write state for");
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
        assert!(!file.projects.contains_key("/one"), "state is written only for the active project");
    }

    #[test]
    fn merge_without_an_active_project_writes_no_state() {
        let mut file = Settings::default();

        merge(&mut file, ResolvedSettings::default(), "2026-08-01T09:12:00+00:00".into());

        assert_eq!(file.last_project, None);
        assert!(file.projects.is_empty(), "the last project was closed — there is nobody to write state for");
    }

    #[test]
    fn trim_never_evicts_an_open_project() {
        let mut file = Settings::default();
        // The oldest entry of all — and an open one at that.
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

        assert!(file.projects.contains_key("/open-and-ancient"), "an open project is never evicted, however old");
        assert!(file.projects.contains_key("/current"));
        assert!(!file.projects.contains_key("/p00"), "closed projects go oldest first");
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
        assert_eq!(view.active_project.as_deref(), Some("/a"), "with no argument — the active project from the file");
        assert_eq!(view.project, ProjectState::default());
    }

    #[test]
    fn tabs_are_read_and_written() {
        let settings = settings_of(
            r#"{"version":1,"projects":{"/p":{
                "openTabs":["src/App.vue","README.md"],
                "previewTab":"README.md",
                "activeTab":"src/App.vue"}}}"#,
        );
        let state = &settings.projects["/p"];
        assert_eq!(state.open_tabs, vec!["src/App.vue".to_string(), "README.md".to_string()]);
        assert_eq!(state.preview_tab.as_deref(), Some("README.md"));
        assert_eq!(state.active_tab, "src/App.vue");
    }

    #[test]
    fn the_preview_tab_has_to_be_among_the_open_ones() {
        let settings = settings_of(
            r#"{"version":1,"projects":{"/p":{"openTabs":["a.txt"],"previewTab":"b.txt"}}}"#,
        );
        assert_eq!(settings.projects["/p"].preview_tab, None);
    }

    #[test]
    fn the_active_tab_is_terminal_kanban_or_one_of_the_open_files() {
        let settings = settings_of(
            r#"{"version":1,"projects":{
                "/gone":{"openTabs":["a.txt"],"activeTab":"b.txt"},
                "/chat":{"activeTab":"chat"},
                "/file":{"openTabs":["a.txt"],"activeTab":"a.txt"}}}"#,
        );
        assert_eq!(settings.projects["/gone"].active_tab, "kanban", "no such tab — nothing to be active");
        assert_eq!(settings.projects["/chat"].active_tab, "terminal", "the old tab name migrates");
        assert_eq!(settings.projects["/file"].active_tab, "a.txt");
    }

    #[test]
    fn a_chat_tab_from_an_old_file_becomes_terminal() {
        let settings = settings_of(r#"{"version":1,"projects":{"/p":{"activeTab":"chat"}}}"#);
        assert_eq!(settings.projects["/p"].active_tab, "terminal");
    }

    #[test]
    fn an_open_file_named_chat_does_not_migrate_to_terminal() {
        // A file with that name at the project root is an ordinary thing, and it
        // predates the tab's rename: the migration must not take a person off
        // their own file.
        let settings =
            settings_of(r#"{"version":1,"projects":{"/p":{"openTabs":["chat"],"activeTab":"chat"}}}"#);
        assert_eq!(settings.projects["/p"].active_tab, "chat");
    }

    #[test]
    fn the_terminal_tab_passes_validation() {
        let settings = settings_of(r#"{"version":1,"projects":{"/p":{"activeTab":"terminal"}}}"#);
        assert_eq!(settings.projects["/p"].active_tab, "terminal");
    }

    #[test]
    fn a_long_path_can_be_the_active_tab() {
        // active_tab used to be cut at MAX_ID_LEN (200); a path can be longer,
        // and the tab would silently become the board on every restart.
        let long = format!("src/{}/App.vue", "very-long-directory-name".repeat(12));
        assert!(long.len() > MAX_ID_LEN && long.len() < MAX_PATH_LEN);
        let text = serde_json::json!({
            "version": 1,
            "projects": {"/p": {"openTabs": [long.clone()], "activeTab": long.clone()}}
        });

        let settings = settings_of(&text.to_string());

        assert_eq!(settings.projects["/p"].active_tab, long);
    }

    #[test]
    fn the_tab_list_loses_junk_and_is_trimmed() {
        let mut tabs = vec![String::from("a.txt"), String::from("a.txt"), String::new()];
        for i in 0..MAX_OPEN_TABS {
            tabs.push(format!("f{i:03}.txt"));
        }
        let text = serde_json::json!({"version": 1, "projects": {"/p": {"openTabs": tabs}}});

        let settings = settings_of(&text.to_string());

        let open = &settings.projects["/p"].open_tabs;
        assert_eq!(open.len(), MAX_OPEN_TABS);
        assert_eq!(open[0], "a.txt");
        assert_eq!(open[1], "f000.txt", "the duplicate and the empty string fell out");
    }

    #[test]
    fn a_file_written_before_tabs_reads_without_them() {
        let settings = settings_of(r#"{"version":1,"projects":{"/p":{"sideTab":"agents"}}}"#);
        let state = &settings.projects["/p"];
        assert!(state.open_tabs.is_empty());
        assert_eq!(state.preview_tab, None);
        assert_eq!(state.active_tab, "kanban");
    }

    #[test]
    fn the_column_order_is_read_and_written() {
        let settings =
            settings_of(r#"{"version":1,"projects":{"/p":{"columnOrder":["done","ready"]}}}"#);
        assert_eq!(
            settings.projects["/p"].column_order,
            vec!["done".to_string(), "ready".to_string()]
        );

        let mut file = Settings::default();
        let resolved = ResolvedSettings {
            project: ProjectState {
                column_order: vec!["running".into(), "ready".into()],
                ..ProjectState::default()
            },
            open_projects: vec!["/p".into()],
            active_project: Some("/p".into()),
            ..ResolvedSettings::default()
        };
        merge(&mut file, resolved, "2026-08-01T09:12:00+00:00".into());
        assert_eq!(
            file.projects["/p"].column_order,
            vec!["running".to_string(), "ready".to_string()]
        );
    }

    #[test]
    fn a_file_written_before_the_column_order_reads_without_it() {
        let settings = settings_of(r#"{"version":1,"projects":{"/p":{"sideTab":"agents"}}}"#);
        assert!(settings.projects["/p"].column_order.is_empty(), "no order is bd's own order");
    }

    #[test]
    fn the_column_order_loses_blanks_duplicates_and_everything_past_the_limit() {
        let mut order = vec![
            String::from("done"),
            String::from("done"),
            String::new(),
            "x".repeat(MAX_ID_LEN + 1),
        ];
        for i in 0..MAX_COLUMNS {
            order.push(format!("gen{i:03}"));
        }
        let text = serde_json::json!({"version": 1, "projects": {"/p": {"columnOrder": order}}});

        let stored = &settings_of(&text.to_string()).projects["/p"].column_order;

        assert_eq!(stored.len(), MAX_COLUMNS, "the order is not stored past the limit");
        assert_eq!(stored[0], "done");
        assert_eq!(stored[1], "gen000", "the duplicate, the empty name and the overlong one fell out");
        assert_eq!(
            stored.last(),
            Some(&format!("gen{:03}", MAX_COLUMNS - 2)),
            "the tail is trimmed, not the head"
        );
    }

    /// bd's set of statuses is unknown on this side of the IPC, so an unknown
    /// name is kept rather than pruned — the board passes it over, and a status
    /// that comes back finds the place it was left in.
    #[test]
    fn the_column_order_keeps_a_status_nothing_matches() {
        let settings =
            settings_of(r#"{"version":1,"projects":{"/p":{"columnOrder":["done","gone","ready"]}}}"#);
        assert_eq!(
            settings.projects["/p"].column_order,
            vec!["done".to_string(), "gone".to_string(), "ready".to_string()]
        );
    }

    #[test]
    fn a_fresh_file_runs_claude_code() {
        assert_eq!(Settings::default().agent, "claude");
        // The resolved view has its own hand-written Default, and the two have
        // to name the same agent: an empty string there is not one.
        assert_eq!(ResolvedSettings::default().agent, "claude");
    }

    #[test]
    fn an_agent_nobody_ships_loses_the_field_and_not_the_section() {
        let mut settings = Settings { agent: "cursor".into(), ..Settings::default() };
        settings.validate();
        assert_eq!(settings.agent, "claude");
        assert_eq!(settings.appearance.theme, "dark", "the section around it survives");
    }

    #[test]
    fn every_agent_the_app_knows_is_a_legal_setting() {
        for id in crate::agents::IDS {
            let mut settings = Settings { agent: id.to_string(), ..Settings::default() };
            settings.validate();
            assert_eq!(settings.agent, id);
        }
    }

    /// A field added to `Settings` and `ResolvedSettings` but not wired into
    /// `parse`, `resolve` and `merge` reads as `"claude"` forever no matter
    /// what the file says — the struct-alone tests above cannot see that,
    /// since they never call any of the three. This one walks the real path a
    /// hand-written file takes: disk, to the front end, and back to disk.
    #[test]
    fn a_chosen_agent_does_not_quietly_become_claude_again() {
        let file = settings_of(r#"{"version":1,"agent":"codex"}"#);
        assert_eq!(file.agent, "codex", "parse must read it off the disk");

        let resolved = resolve(&file, None);
        assert_eq!(resolved.agent, "codex", "resolve must carry it to the front end");

        let mut written = Settings::default();
        merge(&mut written, resolved, "2026-08-01T00:00:00+00:00".into());
        assert_eq!(written.agent, "codex", "merge must carry it back into the file");
    }
    #[test]
    fn a_settings_file_written_before_the_run_dialog_existed_still_loads() {
        // Every file on a person's disk right now is this file.
        let state: ProjectState =
            serde_json::from_str(r#"{"sideTab":"files"}"#).expect("deserializes");
        assert_eq!(state.run_settings, None);
    }

    #[test]
    fn an_unknown_run_mode_loses_the_field_and_not_the_section() {
        // The same leniency every other single value gets: a bad value costs
        // that value, a bad type costs the section it is in.
        let mut state = ProjectState {
            run_settings: Some(RunDefaults { mode: "yolo".into(), ..RunDefaults::default() }),
            ..ProjectState::default()
        };
        state.validate();
        let run = state.run_settings.expect("the section survives");
        assert_eq!(run.mode, "auto");
        assert!(run.live_check, "nothing else in the section was touched");
    }

    #[test]
    fn a_priority_outside_bds_scale_is_forgotten() {
        // Clamping would guess: a 9 here is a wrongly edited file, and reading
        // it as "the lowest priority" makes a run take work nobody wanted taken.
        // No floor sends the dialog back to the project's own configured one.
        let mut state = ProjectState {
            run_settings: Some(RunDefaults { min_priority: Some(9), ..RunDefaults::default() }),
            ..ProjectState::default()
        };
        state.validate();
        assert_eq!(state.run_settings.expect("kept").min_priority, None);
    }

    #[test]
    fn a_run_with_no_floor_is_remembered_as_having_none() {
        // Every run aimed at a task or an epic writes this shape: a floor only
        // means something for the queue, so the dialog sends none. Read back as
        // a number it would open the next queue run on a choice nobody made,
        // over the top of `[defaults] min_priority` in the project's own config.
        //
        // The written shape is asserted as well as the read one, and the key
        // has to be genuinely absent rather than a null: a build older than
        // this one reads `u8` here, and a null it cannot parse costs that whole
        // project entry. See the field's own comment.
        let state = ProjectState {
            run_settings: Some(RunDefaults::default()),
            ..ProjectState::default()
        };
        let json = serde_json::to_string(&state).expect("serializes");
        assert!(!json.contains("minPriority"), "no floor is no key at all: {json}");
        let back: ProjectState = serde_json::from_str(&json).expect("deserializes");
        assert_eq!(back.run_settings.expect("kept").min_priority, None);

        // And the same file read by this build once more, written by hand the
        // way every file on disk before this change looks.
        let old: ProjectState = serde_json::from_str(
            r#"{"runSettings":{"mode":"auto","liveCheck":true,"fileFindings":true}}"#,
        )
        .expect("deserializes");
        assert_eq!(old.run_settings.expect("kept").min_priority, None);
    }

    #[test]
    fn a_floor_inside_the_scale_survives_the_round_trip() {
        // The other half of the pair above: what the queue dialog did choose
        // has to come back, and come back as the same number.
        let mut state = ProjectState {
            run_settings: Some(RunDefaults { min_priority: Some(1), ..RunDefaults::default() }),
            ..ProjectState::default()
        };
        state.validate();
        let json = serde_json::to_string(&state).expect("serializes");
        let back: ProjectState = serde_json::from_str(&json).expect("deserializes");
        assert_eq!(back.run_settings.expect("kept").min_priority, Some(1));
    }

    #[test]
    fn solo_is_a_mode_this_file_accepts_even_though_a_queue_would_refuse_it() {
        // Whether solo fits the scope is RunSettings::validate's answer. This
        // file only knows the three names, and dropping solo here would make
        // the dialog forget a legitimate choice on every restart.
        let mut state = ProjectState {
            run_settings: Some(RunDefaults { mode: "solo".into(), ..RunDefaults::default() }),
            ..ProjectState::default()
        };
        state.validate();
        assert_eq!(state.run_settings.expect("kept").mode, "solo");
    }
}
