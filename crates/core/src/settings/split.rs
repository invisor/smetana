//! What of `settings.json` is the server's and what is the device's.
//!
//! Step 6 of `docs/superpowers/specs/2026-10-25-headless-server-design.md`. The desktop is a client of
//! `smetanad` and keeps the device's half; the server keeps the other. Only a browser, with no shell
//! and no server, still reads and writes one document whole (the mock back end).
//!
//! **The rule has two clauses.** A field is the server's if a worker on the server reads it — an
//! agent's profile, a run's policy, the relay it pairs through, the project list it starts from — or
//! if a paired phone draws it: the theme, the density, the board's rules and every project's column
//! order. Every client sees the same value of those. Everything else is about a screen or a person and
//! stays on the device: the font size, the layout, which task is open. One server, one look.
//! Both halves keep the **same shape as the file**, so a half is a `Settings` document with the other
//! half's keys missing, and nothing here needs a second schema to keep in step with `model.rs`.
//!
//! The server half is written by a **patch**, never by a whole document. A client sends only the
//! fields it changed (a JSON merge patch, RFC 7386: an object merges key by key, anything else
//! replaces, `null` removes), the server applies it to the file it holds *under one lock* and
//! broadcasts the result as `settings:changed`, so two clients that changed different fields both
//! keep their change. A patch can only touch the server's keys: whatever else it carries is dropped,
//! which is what stops a client's idea of the device half from landing on the server.
//!
//! The device half is saved where it always was, whole, by the device itself ([`merge_device`]).
//!
//! **A field that is in neither list is the device's.** The lists below name the server's, and a test
//! pins them to the letter, so a field added to the schema is a decision somebody had to make rather
//! than a default nobody noticed.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde_json::{Map, Value};

use super::commands::SettingsError;
use super::file;
use super::model::{self, parse, Outcome, ResolvedSettings, Settings, CURRENT_VERSION};

/// The event every client follows: the server half, whole, after a patch was applied.
pub const SETTINGS_CHANGED: &str = "settings:changed";

/// Keys at the root of the file that the server reads, whole. `agent`, the languages, `agentPrompt`,
/// `model` and `agentRoles` are what an agent session is started with; `conversationPanel` decides
/// how a run's sessions are driven; `subscription` and `runFailover` are the run gate; `remote` holds
/// the relay a phone is paired through; `openProjects` and `lastProject` are what the server starts
/// its workers on.
const SERVER_ROOT: &[&str] = &[
    "agent",
    "agentLanguage",
    "taskLanguage",
    "commitLanguage",
    "reportLanguage",
    "agentPrompt",
    "model",
    "agentRoles",
    "conversationPanel",
    "subscription",
    "runFailover",
    "remote",
    "openProjects",
    "lastProject",
    "kanban",
];

/// The server's fields of `git`: `removeWorktrees` is a line of the run prompt, while `autoFetch` is
/// the window asking for a fetch when it gains focus, which the server only ever obeys.
const SERVER_GIT: &[&str] = &["removeWorktrees"];

/// The server's fields of `appearance`: what a phone draws. `uiFontSize` stays on the device, because
/// a phone's type is its own and the person chose not to copy the desktop's.
const SERVER_APPEARANCE: &[&str] = &["theme", "density"];

/// The server's fields of one project: the run dialog's last settings (the target branch is read when
/// a run starts), the project's own agent table, and the stamp the least recently used project is
/// dropped by. `usedAt` is also kept on the device, for the device file's own limit.
const SERVER_PROJECT: &[&str] = &["runSettings", "agents", "usedAt", "columnOrder"];

/// The server's fields, in the order a person reads them, as paths. Held to the schema by a test.
#[cfg(test)]
pub(crate) fn server_paths() -> Vec<String> {
    let mut out: Vec<String> = SERVER_ROOT.iter().map(|key| (*key).to_owned()).collect();
    out.extend(SERVER_GIT.iter().map(|key| format!("git.{key}")));
    out.extend(SERVER_APPEARANCE.iter().map(|key| format!("appearance.{key}")));
    out.extend(SERVER_PROJECT.iter().map(|key| format!("projects.*.{key}")));
    out
}

/// What of a document the server keeps: a copy of `value` with the device's keys removed.
fn keep_server(value: &Value) -> Value {
    let Some(object) = value.as_object() else {
        return Value::Object(Map::new());
    };
    let mut out = Map::new();
    for (key, inner) in object {
        if SERVER_ROOT.contains(&key.as_str()) {
            out.insert(key.clone(), inner.clone());
        } else if key == "git" {
            if let Some(git) = inner.as_object() {
                let kept: Map<String, Value> =
                    git.iter().filter(|(name, _)| SERVER_GIT.contains(&name.as_str())).map(|(k, v)| (k.clone(), v.clone())).collect();
                out.insert(key.clone(), Value::Object(kept));
            }
        } else if key == "appearance" {
            if let Some(appearance) = inner.as_object() {
                let kept: Map<String, Value> = appearance
                    .iter()
                    .filter(|(name, _)| SERVER_APPEARANCE.contains(&name.as_str()))
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect();
                out.insert(key.clone(), Value::Object(kept));
            }
        } else if key == "projects" {
            if let Some(projects) = inner.as_object() {
                let mut kept = Map::new();
                for (path, state) in projects {
                    match state {
                        // A project removed.
                        Value::Null => {
                            kept.insert(path.clone(), Value::Null);
                        }
                        Value::Object(fields) => {
                            let fields: Map<String, Value> = fields
                                .iter()
                                .filter(|(name, _)| SERVER_PROJECT.contains(&name.as_str()))
                                .map(|(k, v)| (k.clone(), v.clone()))
                                .collect();
                            kept.insert(path.clone(), Value::Object(fields));
                        }
                        _ => {}
                    }
                }
                out.insert(key.clone(), Value::Object(kept));
            }
        }
    }
    Value::Object(out)
}

/// What of a document the device keeps: everything the server does not read.
fn keep_device(value: &Value) -> Value {
    let Some(object) = value.as_object() else {
        return Value::Object(Map::new());
    };
    let mut out = Map::new();
    for (key, inner) in object {
        if SERVER_ROOT.contains(&key.as_str()) {
            continue;
        }
        if key == "git" {
            if let Some(git) = inner.as_object() {
                let kept: Map<String, Value> =
                    git.iter().filter(|(name, _)| !SERVER_GIT.contains(&name.as_str())).map(|(k, v)| (k.clone(), v.clone())).collect();
                out.insert(key.clone(), Value::Object(kept));
            }
        } else if key == "appearance" {
            if let Some(appearance) = inner.as_object() {
                let kept: Map<String, Value> = appearance
                    .iter()
                    .filter(|(name, _)| !SERVER_APPEARANCE.contains(&name.as_str()))
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect();
                out.insert(key.clone(), Value::Object(kept));
            }
        } else if key == "projects" {
            if let Some(projects) = inner.as_object() {
                let mut kept = Map::new();
                for (path, state) in projects {
                    if let Some(fields) = state.as_object() {
                        let fields: Map<String, Value> = fields
                            .iter()
                            // `usedAt` stays on both sides, each for its own file's limit.
                            .filter(|(name, _)| name.as_str() == "usedAt" || !SERVER_PROJECT.contains(&name.as_str()))
                            .map(|(k, v)| (k.clone(), v.clone()))
                            .collect();
                        kept.insert(path.clone(), Value::Object(fields));
                    }
                }
                out.insert(key.clone(), Value::Object(kept));
            }
        } else {
            out.insert(key.clone(), inner.clone());
        }
    }
    Value::Object(out)
}

fn document(settings: &Settings) -> Value {
    serde_json::to_value(settings).unwrap_or(Value::Null)
}

/// The server's half of a settings document, in the file's shape. This is what `settings:changed`
/// carries.
pub fn server_half(settings: &Settings) -> Value {
    keep_server(&document(settings))
}

/// The device's half of a settings document, in the file's shape.
pub fn device_half(settings: &Settings) -> Value {
    keep_device(&document(settings))
}

/// RFC 7386: an object patch merges into the target key by key, `null` removes a key, and any other
/// value replaces what is there, an array included.
pub fn merge_patch(target: &mut Value, patch: &Value) {
    let Value::Object(changes) = patch else {
        *target = patch.clone();
        return;
    };
    if !target.is_object() {
        *target = Value::Object(Map::new());
    }
    let Value::Object(into) = target else { return };
    for (key, change) in changes {
        if change.is_null() {
            into.remove(key);
        } else {
            merge_patch(into.entry(key.clone()).or_insert(Value::Null), change);
        }
    }
}

/// One write of the server half at a time. The patch is read-modify-write on a file, so two
/// patches that overlapped would each start from the file as it was and the second rename would
/// undo the first: this lock is what makes "both are kept" true. It is held across the broadcast
/// too, so that the order clients hear the states in is the order they were written in.
pub(super) static WRITE: Mutex<()> = Mutex::new(());

/// How many states of the server half this process has written, counted under [`WRITE`]. It rides
/// the answer to a patch and the event that announces it as `rev`, so a client can tell an older state
/// arriving late from a newer one. It starts again at 1 when the server does, which a client meets as
/// a new connection.
static REV: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Applies a patch of the server's fields to the file at `path`, validates the result the way a read
/// does, writes it, and hands the server half now in force to `announce` before the next patch can
/// start. Whatever the patch carries beyond the server's fields is dropped.
///
/// With `only_if_fresh` the patch is applied only to a server that has no settings file yet — the
/// migration's road, which must never overwrite what a server already holds — and answers `None`
/// when it did not apply.
pub fn apply_patch(
    path: &Path,
    patch: &Value,
    now: &str,
    only_if_fresh: bool,
    announce: &dyn Fn(&Value),
) -> Result<Option<Value>, SettingsError> {
    if !patch.is_object() {
        return Err(SettingsError::Patch("a patch is an object of the fields that changed".to_owned()));
    }
    let _guard = WRITE.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    if only_if_fresh && path.exists() {
        return Ok(None);
    }
    let (stored, problem) = file::load(path);
    // The same asymmetry `settings_save` documents: a broken file has gone to `.bak`, an unreadable
    // one has not and must not be written over.
    if matches!(problem, Some(file::Problem::Unreadable)) {
        return Err(SettingsError::Write(format!(
            "{}: the existing file could not be read, so it was not overwritten",
            path.display()
        )));
    }

    let wanted = keep_server(patch);
    if &wanted != patch {
        log::warn!("settings: a patch named fields that are not the server's; they were dropped");
    }
    let mut doc = document(&stored);
    merge_patch(&mut doc, &wanted);
    let Outcome::Ok(mut next) = parse(&doc.to_string()) else {
        return Err(SettingsError::Patch("the patched settings did not read back".to_owned()));
    };
    if !only_if_fresh {
        if let Some(projects) = wanted.get("projects").and_then(Value::as_object) {
            for (project, state) in projects {
                if state.is_object() {
                    if let Some(entry) = next.projects.get_mut(project) {
                        entry.used_at = Some(now.to_owned());
                    }
                }
            }
        }
    }
    let open = next.open_projects.clone();
    let last = next.last_project.clone();
    model::trim(&mut next.projects, last.as_deref(), &open);
    next.version = CURRENT_VERSION;
    file::save(path, &next).map_err(SettingsError::Write)?;
    super::notify_run_settings_changed();
    let mut half = server_half(&next);
    half["rev"] = Value::from(REV.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1);
    announce(&half);
    Ok(Some(half))
}

/// Saves the device's half into the device's own file, whole, as the single-file build saves all of
/// it. Fields the server owns are left exactly as the file has them: after a migration they are the
/// defaults, and before one they are the old file's, which is why nothing is lost.
pub fn merge_device(file: &mut Settings, resolved: ResolvedSettings, now: String) {
    let mut whole = file.clone();
    model::merge(&mut whole, resolved, now);
    let mut doc = document(file);
    merge_patch(&mut doc, &device_half(&whole));
    // A project `merge` let go of for the limit goes from the file too, which a merge patch cannot say.
    if let Some(Value::Object(projects)) = doc.get_mut("projects") {
        projects.retain(|path, _| whole.projects.contains_key(path));
    }
    if let Outcome::Ok(next) = parse(&doc.to_string()) {
        *file = next;
    }
}

/// The device's half of the server's own file, for a client whose earlier build kept the whole
/// document there (step 5 forwarded `settings_save`): `None` when the server has no file. Read
/// under the write lock, so it is a state and never half of a patch.
pub fn device_half_of_file(path: &Path) -> Result<Option<Value>, SettingsError> {
    let _guard = WRITE.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    if !path.exists() {
        return Ok(None);
    }
    let (stored, problem) = file::load(path);
    if matches!(problem, Some(file::Problem::Unreadable)) {
        return Err(SettingsError::Write(format!("{}: the file could not be read", path.display())));
    }
    Ok(Some(device_half(&stored)))
}

/// Settles the device file's device half, once. A file already marked (`deviceSeeded`) is left alone.
/// Otherwise the device half the server held (`from_server`, if it held any) becomes this file's,
/// with the dialog sizes this machine remembered laid over it - they were never the server's - and
/// the mark is set in the same write. With `None` the device's own half is the one that stands, and
/// only the mark is set. A copy of the file as it was is kept first (`keep_copy`, `settings.pre-seed`).
/// Answers whether it wrote anything.
pub fn seed_device(path: &Path, from_server: Option<&Value>) -> Result<bool, String> {
    let (local, problem) = file::load(path);
    if problem.is_some() {
        return Err(format!("{}: the device settings could not be read", path.display()));
    }
    if local.device_seeded {
        return Ok(false);
    }
    let seeded = match from_server {
        Some(half) => {
            if path.exists() {
                keep_copy(path, "settings.pre-seed")?;
            }
            // The server's device keys, then this device's own file over them where it has an
            // opinion the server could not: its dialog sizes, and anything the server lacks.
            let mut doc = keep_device(half);
            let own = keep_device(&document(&local));
            if let Some(dialogs) = own.get("dialogs") {
                let mut merged = doc.get("dialogs").cloned().unwrap_or(Value::Null);
                merge_patch(&mut merged, dialogs);
                doc["dialogs"] = merged;
            }
            doc["deviceSeeded"] = Value::Bool(true);
            // The server-owned keys this file may still hold go back to the defaults: the migration
            // has offered them already and the device keeps none.
            let fresh = server_half(&Settings::default());
            merge_patch(&mut doc, &fresh);
            match parse(&doc.to_string()) {
                Outcome::Ok(next) => next,
                _ => return Err("the seeded settings did not read back".to_owned()),
            }
        }
        None => Settings { device_seeded: true, ..local },
    };
    file::save(path, &seeded)?;
    Ok(true)
}

/// The server half of a document with the entries that say nothing taken out: a project with no
/// run settings and no agent table of its own is only the device's state, `usedAt` aside. The look
/// (theme, density, the board's rules) is left out too: a split device holds it in its own file until
/// `look_patch` has sent it, and it must not read as an old file's server half, which would retire it
/// before it was sent.
fn substance(settings: &Settings) -> Value {
    let mut half = server_half(settings);
    if let Some(root) = half.as_object_mut() {
        root.remove("appearance");
        root.remove("kanban");
    }
    if let Some(Value::Object(projects)) = half.get_mut("projects") {
        projects.retain(|_, state| {
            state.get("runSettings").is_some_and(|v| !v.is_null()) || state.get("agents").is_some_and(|v| !v.is_null())
        });
    }
    half
}

/// What an old single-file `settings.json` holds that now belongs to the server, as a patch — or
/// `None` when it holds none, which is a file already migrated and a first run alike.
pub fn legacy_server_patch(settings: &Settings) -> Option<Value> {
    (substance(settings) != substance(&Settings::default())).then(|| server_half(settings))
}

/// Ends the migration on the device: the server's fields in the device file go back to the defaults,
/// after a copy of the file as it was is kept beside it (`keep_copy`, `settings.pre-split`). Answers
/// whether anything was changed.
///
/// It can run more than once on one device: an older release reinstalled and used for a while writes a
/// server half into the file again, and a server that already has settings refuses to adopt it. That
/// copy is then the only place the half survives, which is why a second retire never skips its copy
/// because a first one left a file of the same name.
pub fn retire_server_half(path: &Path) -> Result<bool, String> {
    let (mut settings, problem) = file::load(path);
    if problem.is_some() || legacy_server_patch(&settings).is_none() {
        return Ok(false);
    }
    keep_copy(path, "settings.pre-split")?;
    let fresh = Settings::default();
    settings.git.remove_worktrees = fresh.git.remove_worktrees;
    settings.remote = fresh.remote;
    settings.subscription = fresh.subscription;
    settings.run_failover = fresh.run_failover;
    settings.agent = fresh.agent;
    settings.agent_language = fresh.agent_language;
    settings.task_language = fresh.task_language;
    settings.commit_language = fresh.commit_language;
    settings.report_language = fresh.report_language;
    settings.agent_prompt = fresh.agent_prompt;
    settings.model = fresh.model;
    settings.agent_roles = fresh.agent_roles;
    settings.conversation_panel = fresh.conversation_panel;
    settings.open_projects = fresh.open_projects;
    settings.last_project = fresh.last_project;
    // The look went with the adopted half; the device keeps only the font size.
    settings.appearance.theme = fresh.appearance.theme;
    settings.appearance.density = fresh.appearance.density;
    settings.kanban = fresh.kanban;
    for state in settings.projects.values_mut() {
        state.run_settings = None;
        state.agents = None;
        state.column_order = Vec::new();
    }
    // The same write marks the device half settled. Were the mark a second write, a quit between the two
    // would leave a retired but unmarked file, and the next launch would seed the device half from the
    // server over the device's own.
    settings.device_seeded = true;
    file::save(path, &settings)?;
    Ok(true)
}

/// The look this device holds in its own file, as a patch for the server: the theme and density when
/// they are not the defaults, the board's rules whole when they are not, and the column order of every
/// project that has one. `None` when nothing differs, which is a file that never chose a look and a file
/// `retire_server_half` has already emptied of it.
pub fn look_patch(settings: &Settings) -> Option<Value> {
    let fresh = Settings::default();
    let mut patch = Map::new();
    let mut appearance = Map::new();
    if settings.appearance.theme != fresh.appearance.theme {
        appearance.insert("theme".to_owned(), Value::from(settings.appearance.theme.clone()));
    }
    if settings.appearance.density != fresh.appearance.density {
        appearance.insert("density".to_owned(), Value::from(settings.appearance.density.clone()));
    }
    if !appearance.is_empty() {
        patch.insert("appearance".to_owned(), Value::Object(appearance));
    }
    if settings.kanban != fresh.kanban {
        patch.insert("kanban".to_owned(), serde_json::to_value(&settings.kanban).unwrap_or(Value::Null));
    }
    let orders: Map<String, Value> = settings
        .projects
        .iter()
        .filter(|(_, state)| !state.column_order.is_empty())
        .map(|(path, state)| (path.clone(), serde_json::json!({ "columnOrder": state.column_order })))
        .collect();
    if !orders.is_empty() {
        patch.insert("projects".to_owned(), Value::Object(orders));
    }
    (!patch.is_empty()).then_some(Value::Object(patch))
}

/// Sets the `lookShared` mark in the device file: the look has been offered to the server. A plain
/// read-modify-write with no copy of the file, because the write loses nothing. Answers whether it
/// wrote; a file already marked answers `Ok(false)`.
pub fn mark_look_shared(path: &Path) -> Result<bool, String> {
    let (mut settings, problem) = file::load(path);
    if problem.is_some() {
        return Err(format!("{}: the device settings could not be read", path.display()));
    }
    if settings.look_shared {
        return Ok(false);
    }
    settings.look_shared = true;
    file::save(path, &settings)?;
    Ok(true)
}

/// Keeps a copy of `path` as it is now beside it: `<stem>.json`, or, when a copy of that name already
/// holds something else, the first free `<stem>.<n>.json` from 2 up. An existing copy is never
/// overwritten, so the oldest state survives, and a later one is never dropped because an older one is
/// there. A copy that already holds exactly these bytes is enough, and nothing is written.
fn keep_copy(path: &Path, stem: &str) -> Result<PathBuf, String> {
    let now = std::fs::read(path).map_err(|err| format!("{}: {err}", path.display()))?;
    let mut n = 1u32;
    loop {
        let name = if n == 1 { format!("{stem}.json") } else { format!("{stem}.{n}.json") };
        let backup = path.with_file_name(name);
        match std::fs::read(&backup) {
            Ok(held) if held == now => return Ok(backup),
            Ok(_) => n += 1,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                std::fs::write(&backup, &now).map_err(|err| format!("{}: {err}", backup.display()))?;
                return Ok(backup);
            }
            Err(err) => return Err(format!("{}: {err}", backup.display())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::{Arc, Mutex as StdMutex};

    /// A settings document with every kind of field set to something that is not the default, read
    /// the way a file is read, so that the values are the ones the app would hold.
    fn old_file() -> String {
        r#"{
            "version": 1,
            "appearance": {"theme": "light", "density": "compact", "uiFontSize": 15},
            "layout": {"leftCollapsed": true, "rightCollapsed": false, "railOpen": false, "leftWidth": 300, "rightWidth": 420,
                       "branchSide": "origin",
                       "gitSections": {"reposRows": 5, "branchRows": 6, "commitRows": 4, "reposOpen": false, "changesOpen": true, "branchesOpen": false}},
            "editor": {"fontSize": 14, "wordWrap": true, "confirmCloseTerminals": false},
            "kanban": {"columns": "all", "interval": "all", "alwaysShow": ["blocked"], "unlimited": ["done"]},
            "reports": {"perPage": 50, "order": "oldest"},
            "git": {"autoFetch": false, "removeWorktrees": false},
            "remote": {"relayUrl": "https://relay.example.org"},
            "subscription": {"pauseAt": 80, "reducedAt": 70},
            "runFailover": {"enabled": true, "returnToPrimary": false, "waitMinutes": 10, "priority": ["codex", "claude"]},
            "window": {"restoreGeometry": false},
            "updates": {"autoCheck": false},
            "notifications": {"runFinished": "off", "needsAttention": "sound-1", "onlyWhenUnfocused": false, "showReport": false},
            "agent": "codex",
            "agentLanguage": "ru",
            "taskLanguage": "de",
            "commitLanguage": "fr",
            "reportLanguage": "ja",
            "agentPrompt": "be brief",
            "model": "",
            "agentRoles": {"tasks": {"agent": "codex", "model": ""}, "code": {"agent": "", "model": ""},
                           "runLead": {"agent": "claude", "model": ""}, "reviewBranch": {"agent": "", "model": ""}},
            "conversationPanel": false,
            "lastProject": "/work/a",
            "openProjects": ["/work/a", "/work/b"],
            "projects": {
                "/work/a": {
                    "sideTab": "git", "rightTab": "sessions", "activeTab": "kanban", "selectedTask": "a-1",
                    "recentTasks": ["a-1", "a-2"], "selectedPath": "/work/a/src", "selectedRepo": "/work/a",
                    "expanded": ["/work/a/src"], "branchFolders": ["feature"], "remoteBranchFolders": ["feature"],
                    "branchTab": "origin", "favoriteBranches": ["main"], "openTabs": ["/work/a/x.rs"], "previewTab": "/work/a/y.rs",
                    "columnOrder": ["open", "done"], "tabOrder": ["/work/a/x.rs"], "agentOrder": ["c1"], "pinnedAgents": ["c1"],
                    "agentNames": {"c1": "reviewer"},
                    "runSettings": {"mode": "supervised", "targetBranch": "develop", "minPriority": 2, "batchPauseMin": 1, "batchPauseMax": 2,
                                    "liveCheck": false, "fileFindings": false},
                    "storageWarnedMib": 50, "usedAt": "2026-10-01T10:00:00+00:00",
                    "agents": {"agent": "codex", "model": "", "agentRoles": {"tasks": {"agent": "", "model": ""}, "code": {"agent": "", "model": ""},
                                                                          "runLead": {"agent": "", "model": ""}, "reviewBranch": {"agent": "", "model": ""}}}
                },
                "/work/b": {"sideTab": "files", "selectedTask": "b-9", "usedAt": "2026-09-01T10:00:00+00:00"}
            },
            "dialogs": {"run": {"width": 600, "height": 500}}
        }"#
        .to_owned()
    }

    fn old_settings() -> Settings {
        match parse(&old_file()) {
            Outcome::Ok(settings) => settings,
            other => panic!("the fixture did not read: {other:?}"),
        }
    }

    fn dir() -> tempfile::TempDir {
        tempfile::tempdir().expect("a directory of the test's own")
    }

    fn nothing(_: &Value) {}

    /// The leaf paths of a document, `*` standing for a project path, so that two documents can be
    /// compared by which fields they hold rather than by what is in them.
    fn leaves(value: &Value, prefix: &str, out: &mut Vec<String>) {
        match value {
            Value::Object(map) if !map.is_empty() => {
                for (key, inner) in map {
                    let key = if prefix == "projects" { "*" } else { key.as_str() };
                    let path = if prefix.is_empty() { key.to_owned() } else { format!("{prefix}.{key}") };
                    leaves(inner, &path, out);
                }
            }
            _ => out.push(prefix.to_owned()),
        }
    }

    fn paths_of(value: &Value) -> std::collections::BTreeSet<String> {
        let mut out = Vec::new();
        leaves(value, "", &mut out);
        out.into_iter().collect()
    }

    #[test]
    fn the_servers_fields_are_exactly_these_and_a_new_field_is_the_devices_until_someone_says_otherwise() {
        assert_eq!(
            server_paths(),
            [
                "agent", "agentLanguage", "taskLanguage", "commitLanguage", "reportLanguage", "agentPrompt", "model", "agentRoles",
                "conversationPanel", "subscription", "runFailover", "remote", "openProjects", "lastProject", "kanban",
                "git.removeWorktrees", "appearance.theme", "appearance.density", "projects.*.runSettings", "projects.*.agents",
                "projects.*.usedAt", "projects.*.columnOrder",
            ]
        );
        // Every root key of the schema is classified, and the device's are the ones below. A key
        // added to `Settings` shows up here and fails until somebody decides which half it is.
        let whole = document(&old_settings());
        let device: Vec<String> = keep_device(&whole).as_object().unwrap().keys().cloned().collect();
        let mut device_sorted = device.clone();
        device_sorted.sort();
        assert_eq!(
            device_sorted,
            ["appearance", "deviceSeeded", "dialogs", "editor", "git", "layout", "lookShared", "notifications", "projects", "reports", "updates", "version", "window"]
        );
        let server: Vec<String> = keep_server(&whole).as_object().unwrap().keys().cloned().collect();
        for key in whole.as_object().unwrap().keys() {
            let in_server = server.contains(key);
            let in_device = device.contains(key);
            assert!(in_server || in_device, "`{key}` belongs to no half");
            assert!(!(in_server && in_device) || key == "git" || key == "appearance" || key == "projects", "`{key}` belongs to both halves");
        }
    }

    #[test]
    fn the_two_halves_are_disjoint_and_together_are_the_whole_document() {
        let settings = old_settings();
        let (server, device) = (server_half(&settings), device_half(&settings));
        let (mut a, mut b) = (paths_of(&server), paths_of(&device));
        // `usedAt` is the one field both files keep, each for its own limit.
        a.remove("projects.*.usedAt");
        b.remove("projects.*.usedAt");
        assert!(a.is_disjoint(&b), "in both halves: {:?}", a.intersection(&b).collect::<Vec<_>>());
        let mut together = device.clone();
        merge_patch(&mut together, &server);
        // A null and a missing key are the same thing to a read, so the comparison is made on what
        // the app would hold, not on the text.
        assert_eq!(parse(&together.to_string()), Outcome::Ok(settings), "a field fell between the halves");
    }

    #[test]
    fn a_patch_changes_the_fields_it_names_and_leaves_every_other_alone() {
        let tmp = dir();
        let path = tmp.path().join("settings.json");
        file::save(&path, &old_settings()).unwrap();
        let half = apply_patch(&path, &json!({"agent": "claude", "subscription": {"pauseAt": 95}}), "now", false, &nothing)
            .unwrap()
            .unwrap();
        let after = file::load(&path).0;
        assert_eq!(after.agent, "claude");
        assert_eq!(after.subscription.pause_at, 95);
        assert_eq!(after.subscription.reduced_at, 70, "a field the patch did not name is untouched");
        assert_eq!(after.agent_prompt, "be brief");
        assert_eq!(after.run_failover, old_settings().run_failover);
        let mut half = half;
        half.as_object_mut().unwrap().remove("rev");
        assert_eq!(half, server_half(&after), "the answer is the half now in force");
    }

    #[test]
    fn patches_from_two_clients_that_changed_different_fields_are_both_kept() {
        let tmp = dir();
        let path = Arc::new(tmp.path().join("settings.json"));
        file::save(&path, &Settings::default()).unwrap();
        let heard: Arc<StdMutex<Vec<Value>>> = Arc::default();
        let mut threads = Vec::new();
        for n in 0..8 {
            let (path, heard) = (path.clone(), heard.clone());
            threads.push(std::thread::spawn(move || {
                // Each client changes a field of its own, from a view that knows nothing of the rest.
                let patch = match n {
                    0 => json!({"agent": "codex"}),
                    1 => json!({"agentLanguage": "ru"}),
                    2 => json!({"agentPrompt": "be brief"}),
                    3 => json!({"conversationPanel": false}),
                    4 => json!({"subscription": {"pauseAt": 80}}),
                    5 => json!({"runFailover": {"enabled": true}}),
                    6 => json!({"git": {"removeWorktrees": false}}),
                    _ => json!({"remote": {"relayUrl": "https://relay.example.org"}}),
                };
                let announce = |half: &Value| heard.lock().unwrap().push(half.clone());
                apply_patch(&path, &patch, "now", false, &announce).unwrap();
            }));
        }
        for thread in threads {
            thread.join().unwrap();
        }
        let after = file::load(&path).0;
        assert_eq!(after.agent, "codex");
        assert_eq!(after.agent_language, "ru");
        assert_eq!(after.agent_prompt, "be brief");
        assert!(!after.conversation_panel);
        assert_eq!(after.subscription.pause_at, 80);
        assert!(after.run_failover.enabled);
        assert!(!after.git.remove_worktrees);
        assert_eq!(after.remote.relay_url, "https://relay.example.org");
        let heard = heard.lock().unwrap();
        assert_eq!(heard.len(), 8, "every applied patch is announced once");
        let mut last = heard.last().unwrap().clone();
        let revs: Vec<u64> = heard.iter().map(|half| half["rev"].as_u64().unwrap()).collect();
        assert!(revs.windows(2).all(|pair| pair[0] < pair[1]), "the states are numbered in the order they were written: {revs:?}");
        last.as_object_mut().unwrap().remove("rev");
        assert_eq!(last, server_half(&after), "the last state announced is the state on disk");
        // Each announcement holds everything before it: the states are heard in the order written.
        let held = |half: &Value| ["agent", "agentLanguage", "agentPrompt", "conversationPanel"].iter().filter(|key| half[**key] != server_half(&Settings::default())[**key]).count()
            + usize::from(half["subscription"]["pauseAt"] != 90)
            + usize::from(half["runFailover"]["enabled"] == true)
            + usize::from(half["git"]["removeWorktrees"] == false)
            + usize::from(half["remote"]["relayUrl"] != "https://relay.smetana.io");
        let counts: Vec<usize> = heard.iter().map(held).collect();
        assert_eq!(counts, [1, 2, 3, 4, 5, 6, 7, 8]);
    }

    #[test]
    fn a_patch_cannot_write_the_devices_fields() {
        let tmp = dir();
        let path = tmp.path().join("settings.json");
        let patch = json!({
            "appearance": {"uiFontSize": 20}, "layout": {"leftWidth": 999}, "editor": {"fontSize": 20},
            "window": {"restoreGeometry": false}, "git": {"autoFetch": false, "removeWorktrees": false},
            "dialogs": {"run": {"width": 1, "height": 1}},
            "projects": {"/work/a": {"selectedTask": "a-1", "sideTab": "git", "runSettings": {"mode": "solo"}}}
        });
        apply_patch(&path, &patch, "now", false, &nothing).unwrap();
        let after = file::load(&path).0;
        let fresh = Settings::default();
        assert_eq!(after.appearance, fresh.appearance);
        assert_eq!(after.layout, fresh.layout);
        assert_eq!(after.editor, fresh.editor);
        assert_eq!(after.window, fresh.window);
        assert_eq!(after.dialogs, fresh.dialogs);
        assert!(after.git.auto_fetch, "`git.autoFetch` is the device's");
        assert!(!after.git.remove_worktrees, "`git.removeWorktrees` is the server's");
        let project = &after.projects["/work/a"];
        assert_eq!(project.selected_task, None);
        assert_eq!(project.side_tab, "files");
        assert_eq!(project.run_settings.as_ref().unwrap().mode, "solo");
    }

    #[test]
    fn a_patch_is_validated_as_a_read_is_and_stamps_the_project_it_touched() {
        let tmp = dir();
        let path = tmp.path().join("settings.json");
        let patch = json!({"agent": "no-such-agent", "projects": {"/work/a": {"runSettings": {"mode": "solo"}}}});
        apply_patch(&path, &patch, "2026-10-26T00:00:00+00:00", false, &nothing).unwrap();
        let after = file::load(&path).0;
        assert_eq!(after.agent, "claude", "an agent nobody ships is forgotten, as in a hand-edited file");
        assert_eq!(after.projects["/work/a"].used_at.as_deref(), Some("2026-10-26T00:00:00+00:00"));
    }

    #[test]
    fn null_removes_a_field_and_a_patch_that_is_not_an_object_is_refused() {
        let tmp = dir();
        let path = tmp.path().join("settings.json");
        file::save(&path, &old_settings()).unwrap();
        let patch = json!({"projects": {"/work/a": {"runSettings": null, "agents": null}}, "lastProject": null});
        apply_patch(&path, &patch, "now", false, &nothing).unwrap();
        let after = file::load(&path).0;
        assert_eq!(after.projects["/work/a"].run_settings, None);
        assert_eq!(after.projects["/work/a"].agents, None);
        assert_eq!(after.projects["/work/a"].selected_task.as_deref(), Some("a-1"), "the file keeps what it holds of the device");
        // `validate` keeps the active project among the open ones, so a removed one is the first open.
        assert_eq!(after.last_project.as_deref(), Some("/work/a"));
        assert!(matches!(apply_patch(&path, &json!([1]), "now", false, &nothing), Err(SettingsError::Patch(_))));
    }

    #[test]
    fn a_refused_patch_announces_nothing() {
        let tmp = dir();
        let path = tmp.path().join("settings.json");
        let heard = StdMutex::new(0);
        let announce = |_: &Value| *heard.lock().unwrap() += 1;
        assert!(apply_patch(&path, &json!("nope"), "now", false, &announce).is_err());
        assert_eq!(*heard.lock().unwrap(), 0);
        assert!(!path.exists());
    }

    #[test]
    fn the_device_save_leaves_the_servers_fields_as_the_file_has_them() {
        let mut file = old_settings();
        let mut resolved = model::resolve(&file, Some("/work/a"));
        resolved.appearance.ui_font_size = 17;
        resolved.agent = "claude".into(); // a server field: not the device save's to write
        resolved.project.selected_task = Some("a-2".into());
        resolved.project.agents = None;
        merge_device(&mut file, resolved, "now".into());
        assert_eq!(file.appearance.ui_font_size, 17);
        assert_eq!(file.appearance.theme, "light", "the theme is the server's: the device save leaves it");
        assert_eq!(file.projects["/work/a"].selected_task.as_deref(), Some("a-2"));
        assert_eq!(file.agent, "codex");
        assert!(file.projects["/work/a"].agents.is_some());
        assert_eq!(file.projects["/work/a"].used_at.as_deref(), Some("now"));
    }

    #[test]
    fn a_migration_puts_every_field_of_an_old_file_in_its_half_and_loses_nothing() {
        let tmp = dir();
        let device = tmp.path().join("device").join("settings.json");
        let server = tmp.path().join("server").join("settings.json");
        std::fs::create_dir_all(device.parent().unwrap()).unwrap();
        std::fs::write(&device, old_file()).unwrap();
        let original = file::load(&device).0;

        // The device reads its old file, finds the server's half in it and offers it.
        let offered = legacy_server_patch(&file::load(&device).0).expect("an old file holds the server's fields");
        let adopted = apply_patch(&server, &offered, "now", true, &nothing).unwrap();
        assert!(adopted.is_some());
        assert!(retire_server_half(&device).unwrap());

        let server_file = file::load(&server).0;
        let device_file = file::load(&device).0;
        // Every field is in its half, with its value.
        assert_eq!(server_half(&server_file), server_half(&original));
        let mut marked = device_half(&original);
        marked["deviceSeeded"] = json!(true);
        assert_eq!(device_half(&device_file), marked);
        assert_eq!(server_file.agent, "codex");
        assert_eq!(server_file.agent_roles, original.agent_roles);
        assert_eq!(server_file.projects["/work/a"].run_settings, original.projects["/work/a"].run_settings);
        assert_eq!(server_file.projects["/work/a"].agents, original.projects["/work/a"].agents);
        assert_eq!(server_file.open_projects, ["/work/a", "/work/b"]);
        assert_eq!(server_file.last_project.as_deref(), Some("/work/a"));
        assert_eq!(device_file.appearance.ui_font_size, original.appearance.ui_font_size);
        assert_eq!(device_file.appearance.theme, Settings::default().appearance.theme, "the look went with the adopted half");
        assert_eq!(device_file.kanban, Settings::default().kanban);
        assert_eq!(server_file.appearance.theme, "light");
        assert_eq!(server_file.appearance.density, "compact");
        assert_eq!(server_file.kanban, original.kanban);
        assert_eq!(server_file.projects["/work/a"].column_order, ["open", "done"]);
        assert!(look_patch(&device_file).is_none(), "a retired file has no look left to send");
        assert_eq!(device_file.layout, original.layout);
        assert_eq!(device_file.projects["/work/a"].selected_task.as_deref(), Some("a-1"));
        assert_eq!(device_file.projects["/work/a"].agent_names, original.projects["/work/a"].agent_names);
        assert_eq!(device_file.dialogs, original.dialogs);
        assert!(!device_file.git.auto_fetch);
        assert!(!server_file.git.remove_worktrees);
        // Nothing is lost: the two halves put together are the old file.
        let mut together = device_half(&device_file);
        merge_patch(&mut together, &server_half(&server_file));
        // The only difference from the old file is the mark the retiring write set.
        let mut expected = original.clone();
        expected.device_seeded = true;
        assert_eq!(parse(&together.to_string()), Outcome::Ok(expected));
        // The device no longer carries the server's fields, and the old file is kept beside it.
        assert_eq!(device_file.agent, Settings::default().agent);
        assert!(device_file.open_projects.is_empty());
        assert!(device_file.projects["/work/a"].run_settings.is_none());
        assert_eq!(file::load(&device.with_file_name("settings.pre-split.json")).0, original);
        // And it is done once: the device has nothing left to offer.
        assert!(legacy_server_patch(&device_file).is_none());
        assert!(!retire_server_half(&device).unwrap());
    }

    #[test]
    fn a_second_retire_keeps_a_copy_of_its_own_and_never_loses_what_was_set_in_between() {
        let tmp = dir();
        let device = tmp.path().join("settings.json");
        std::fs::write(&device, old_file()).unwrap();
        assert!(retire_server_half(&device).unwrap());
        let first = std::fs::read(device.with_file_name("settings.pre-split.json")).unwrap();

        // An older release, reinstalled, writes a server half into the file again — a different one.
        let mut again = file::load(&device).0;
        again.agent_prompt = "set in between".into();
        again.open_projects = vec!["/work/later".into()];
        file::save(&device, &again).unwrap();
        let before = std::fs::read(&device).unwrap();
        assert!(retire_server_half(&device).unwrap());

        assert_eq!(std::fs::read(device.with_file_name("settings.pre-split.json")).unwrap(), first, "the first copy stands");
        let second = device.with_file_name("settings.pre-split.2.json");
        assert_eq!(std::fs::read(&second).unwrap(), before, "the second retire kept the file as it then was");
        assert_eq!(file::load(&second).0.agent_prompt, "set in between");

        // A copy that already holds the same bytes is enough: nothing piles up.
        assert_eq!(keep_copy(&device, "settings.pre-split").unwrap(), keep_copy(&device, "settings.pre-split").unwrap());
        assert!(!device.with_file_name("settings.pre-split.4.json").exists());
    }

    #[test]
    fn a_server_that_already_has_settings_is_not_overwritten_by_a_migration() {
        let tmp = dir();
        let server = tmp.path().join("settings.json");
        apply_patch(&server, &json!({"agent": "claude", "agentPrompt": "mine"}), "now", false, &nothing).unwrap();
        let heard = StdMutex::new(0);
        let announce = |_: &Value| *heard.lock().unwrap() += 1;
        let taken = apply_patch(&server, &server_half(&old_settings()), "now", true, &announce).unwrap();
        assert!(taken.is_none());
        assert_eq!(*heard.lock().unwrap(), 0);
        assert_eq!(file::load(&server).0.agent_prompt, "mine");
    }

    #[test]
    fn a_first_run_has_nothing_to_migrate() {
        assert!(legacy_server_patch(&Settings::default()).is_none());
        let tmp = dir();
        assert_eq!(retire_server_half(&tmp.path().join("settings.json")), Ok(false));
        // A device that has only ever kept its own state offers nothing either.
        let mut device = Settings::default();
        device.projects.insert("/work/a".into(), model::ProjectState { selected_task: Some("a-1".into()), ..Default::default() });
        assert!(legacy_server_patch(&device).is_none());
    }

    /// What step 5 left: the server's `settings.json` holds the whole document, and the device's file
    /// holds only what this machine remembered itself.
    #[test]
    fn a_step_5_server_file_seeds_every_device_field_onto_the_device_and_loses_nothing() {
        let tmp = dir();
        let server = tmp.path().join("server").join("settings.json");
        let device = tmp.path().join("settings.json");
        std::fs::create_dir_all(server.parent().unwrap()).unwrap();
        std::fs::write(&server, old_file()).unwrap();
        std::fs::write(&device, r#"{"version":1,"dialogs":{"commit":{"width":500,"height":400}}}"#).unwrap();
        let original = old_settings();

        let half = device_half_of_file(&server).unwrap().expect("the server has a file");
        assert!(seed_device(&device, Some(&half)).unwrap());
        let seeded = file::load(&device).0;

        assert!(seeded.device_seeded);
        let mut expected = device_half(&original);
        merge_patch(&mut expected, &json!({"dialogs": {"commit": {"width": 500, "height": 400}}, "deviceSeeded": true}));
        assert_eq!(device_half(&seeded), expected, "every device field is on the device");
        assert_eq!(seeded.appearance.ui_font_size, original.appearance.ui_font_size);
        assert_eq!(seeded.appearance.theme, Settings::default().appearance.theme, "the theme is the server's, not seeded");
        assert_eq!(seeded.kanban, Settings::default().kanban);
        assert_eq!(seeded.layout, original.layout);
        assert_eq!(seeded.notifications, original.notifications);
        assert_eq!(seeded.projects["/work/a"].selected_task.as_deref(), Some("a-1"));
        assert_eq!(seeded.projects["/work/a"].agent_names, original.projects["/work/a"].agent_names);
        assert_eq!(seeded.dialogs.len(), 2, "the dialog sizes of both files are kept");
        assert_eq!(seeded.agent, Settings::default().agent, "the server's half is not copied to the device");
        // Nothing is lost: the server still holds the whole document, and the device's old file is kept.
        assert_eq!(file::load(&server).0, original);
        assert!(device.with_file_name("settings.pre-seed.json").exists());
    }

    #[test]
    fn seeding_happens_once_and_a_marked_file_is_left_alone() {
        let tmp = dir();
        let device = tmp.path().join("settings.json");
        let half = device_half(&old_settings());
        assert!(seed_device(&device, Some(&half)).unwrap());
        // Edited on the device since; a second launch must not bring the server's copy back.
        let mut mine = file::load(&device).0;
        mine.appearance.theme = "dark".into();
        file::save(&device, &mine).unwrap();
        assert!(!seed_device(&device, Some(&half)).unwrap());
        assert!(!seed_device(&device, None).unwrap());
        assert_eq!(file::load(&device).0.appearance.theme, "dark");
    }

    #[test]
    fn a_server_with_no_file_leaves_the_devices_own_half_and_only_marks_it() {
        let tmp = dir();
        assert_eq!(device_half_of_file(&tmp.path().join("none.json")).unwrap(), None);
        let device = tmp.path().join("settings.json");
        let mut mine = Settings::default();
        mine.appearance.theme = "light".into();
        file::save(&device, &mine).unwrap();
        assert!(seed_device(&device, None).unwrap());
        let after = file::load(&device).0;
        assert!(after.device_seeded);
        assert_eq!(after.appearance.theme, "light");
        assert!(!device.with_file_name("settings.pre-seed.json").exists());
    }

    /// Source one succeeded and the process ended (or the seeding call failed) before anything else was
    /// written: the next run must find the device half settled and leave it alone.
    #[test]
    fn a_retired_file_is_marked_in_the_same_write_so_a_later_seeding_never_overwrites_it() {
        let tmp = dir();
        let device = tmp.path().join("settings.json");
        std::fs::write(&device, old_file()).unwrap();
        assert!(retire_server_half(&device).unwrap());
        let retired = file::load(&device).0;
        assert!(retired.device_seeded, "the retiring write carries the mark");
        // The next connection: the server's copy of the device half (here a stale, different one).
        let mut stale = device_half(&Settings::default());
        stale["appearance"] = json!({"theme": "dark", "density": "comfortable", "uiFontSize": 13});
        assert!(!seed_device(&device, Some(&stale)).unwrap());
        assert_eq!(file::load(&device).0, retired);
        assert_eq!(file::load(&device).0.appearance.ui_font_size, 15);
    }

    #[test]
    fn a_patch_sets_the_theme_and_the_column_order_and_leaves_the_font_size_to_the_device() {
        let tmp = dir();
        let path = tmp.path().join("settings.json");
        let patch = json!({
            "appearance": {"theme": "light", "uiFontSize": 20},
            "kanban": {"columns": "non-empty"},
            "projects": {"/work/a": {"columnOrder": ["done", "open"]}}
        });
        apply_patch(&path, &patch, "now", false, &nothing).unwrap();
        let after = file::load(&path).0;
        let server = server_half(&after);
        assert_eq!(server["appearance"]["theme"], "light");
        assert_eq!(server["kanban"]["columns"], "non-empty");
        assert_eq!(server["projects"]["/work/a"]["columnOrder"], json!(["done", "open"]));
        assert_eq!(after.appearance.ui_font_size, Settings::default().appearance.ui_font_size, "the font size is the device's");
        let device = device_half(&after);
        assert_eq!(device["appearance"]["uiFontSize"], 13);
        assert!(device["appearance"].get("theme").is_none());
        assert!(server["appearance"].get("uiFontSize").is_none());
        assert!(device.get("kanban").is_none());
    }

    #[test]
    fn the_look_patch_carries_the_devices_theme_density_kanban_and_every_column_order() {
        let mut settings = Settings::default();
        settings.appearance.theme = "light".into();
        settings.appearance.density = "compact".into();
        settings.kanban.columns = "non-empty".into();
        settings.projects.insert("/work/a".into(), model::ProjectState { column_order: vec!["done".into(), "open".into()], ..Default::default() });
        settings.projects.insert("/work/b".into(), model::ProjectState::default());
        let kanban = serde_json::to_value(&settings.kanban).unwrap();
        assert_eq!(
            look_patch(&settings),
            Some(json!({
                "appearance": {"theme": "light", "density": "compact"},
                "kanban": kanban,
                "projects": {"/work/a": {"columnOrder": ["done", "open"]}}
            }))
        );
    }

    #[test]
    fn the_look_patch_of_a_file_holding_only_defaults_is_none() {
        assert_eq!(look_patch(&Settings::default()), None);
        let tmp = dir();
        let device = tmp.path().join("settings.json");
        std::fs::write(&device, old_file()).unwrap();
        assert!(look_patch(&file::load(&device).0).is_some());
        assert!(retire_server_half(&device).unwrap());
        assert_eq!(look_patch(&file::load(&device).0), None);
    }

    #[test]
    fn a_split_devices_look_is_not_taken_for_an_old_files_server_half() {
        let mut settings = Settings { device_seeded: true, ..Settings::default() };
        settings.appearance.theme = "light".into();
        settings.kanban.columns = "non-empty".into();
        assert!(legacy_server_patch(&settings).is_none(), "retiring it would lose the look before it is sent");
    }

    #[test]
    fn marking_the_look_shared_writes_the_mark_once_and_keeps_the_rest() {
        let tmp = dir();
        let device = tmp.path().join("settings.json");
        std::fs::write(&device, old_file()).unwrap();
        let before = file::load(&device).0;
        assert!(!before.look_shared);
        assert_eq!(mark_look_shared(&device), Ok(true));
        let after = file::load(&device).0;
        assert!(after.look_shared);
        assert_eq!(after.dialogs, before.dialogs);
        assert_eq!(mark_look_shared(&device), Ok(false));
    }
}
