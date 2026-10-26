//! What of `settings.json` is the server's and what is the device's.
//!
//! Step 6 of `docs/superpowers/specs/2026-10-25-headless-server-design.md`, and only for the client
//! build (`split`): the release goes on writing one file whole, exactly as before.
//!
//! **The rule is one question: does the server read it?** If a worker on the server reads the field —
//! an agent's profile, a run's policy, the relay it pairs through, the project list it starts from —
//! it is the server's, and every client sees the same value. Everything else is about a screen or a
//! person and stays on the device: the look, the layout, which task is open, how the board is drawn.
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

use std::path::Path;
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
];

/// The server's fields of `git`: `removeWorktrees` is a line of the run prompt, while `autoFetch` is
/// the window asking for a fetch when it gains focus, which the server only ever obeys.
const SERVER_GIT: &[&str] = &["removeWorktrees"];

/// The server's fields of one project: the run dialog's last settings (the target branch is read when
/// a run starts), the project's own agent table, and the stamp the least recently used project is
/// dropped by. `usedAt` is also kept on the device, for the device file's own limit.
const SERVER_PROJECT: &[&str] = &["runSettings", "agents", "usedAt"];

/// The server's fields, in the order a person reads them, as paths. Held to the schema by a test.
#[cfg(test)]
pub(crate) fn server_paths() -> Vec<String> {
    let mut out: Vec<String> = SERVER_ROOT.iter().map(|key| (*key).to_owned()).collect();
    out.extend(SERVER_GIT.iter().map(|key| format!("git.{key}")));
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
static WRITE: Mutex<()> = Mutex::new(());

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
    let half = server_half(&next);
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

/// The server half of a document with the entries that say nothing taken out: a project with no
/// run settings and no agent table of its own is only the device's state, `usedAt` aside.
fn substance(settings: &Settings) -> Value {
    let mut half = server_half(settings);
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
/// after a copy of the file as it was is put beside it (`settings.pre-split.json`, never overwritten,
/// so the first one is the one that survives). Answers whether anything was changed.
pub fn retire_server_half(path: &Path) -> Result<bool, String> {
    let (mut settings, problem) = file::load(path);
    if problem.is_some() || legacy_server_patch(&settings).is_none() {
        return Ok(false);
    }
    let backup = path.with_file_name("settings.pre-split.json");
    if !backup.exists() {
        std::fs::copy(path, &backup).map_err(|err| format!("{}: {err}", backup.display()))?;
    }
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
    for state in settings.projects.values_mut() {
        state.run_settings = None;
        state.agents = None;
    }
    file::save(path, &settings)?;
    Ok(true)
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
                "conversationPanel", "subscription", "runFailover", "remote", "openProjects", "lastProject", "git.removeWorktrees",
                "projects.*.runSettings", "projects.*.agents", "projects.*.usedAt",
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
            ["appearance", "dialogs", "editor", "git", "kanban", "layout", "notifications", "projects", "reports", "updates", "version", "window"]
        );
        let server: Vec<String> = keep_server(&whole).as_object().unwrap().keys().cloned().collect();
        for key in whole.as_object().unwrap().keys() {
            let in_server = server.contains(key);
            let in_device = device.contains(key);
            assert!(in_server || in_device, "`{key}` belongs to no half");
            assert!(!(in_server && in_device) || key == "git" || key == "projects", "`{key}` belongs to both halves");
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
        assert_eq!(heard.last().unwrap(), &server_half(&after), "the last state announced is the state on disk");
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
            "appearance": {"theme": "light"}, "layout": {"leftWidth": 999}, "editor": {"fontSize": 20},
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
        resolved.appearance.theme = "dark".into();
        resolved.agent = "claude".into(); // a server field: not the device save's to write
        resolved.project.selected_task = Some("a-2".into());
        resolved.project.agents = None;
        merge_device(&mut file, resolved, "now".into());
        assert_eq!(file.appearance.theme, "dark");
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
        assert_eq!(device_half(&device_file), device_half(&original));
        assert_eq!(server_file.agent, "codex");
        assert_eq!(server_file.agent_roles, original.agent_roles);
        assert_eq!(server_file.projects["/work/a"].run_settings, original.projects["/work/a"].run_settings);
        assert_eq!(server_file.projects["/work/a"].agents, original.projects["/work/a"].agents);
        assert_eq!(server_file.open_projects, ["/work/a", "/work/b"]);
        assert_eq!(server_file.last_project.as_deref(), Some("/work/a"));
        assert_eq!(device_file.appearance, original.appearance);
        assert_eq!(device_file.layout, original.layout);
        assert_eq!(device_file.projects["/work/a"].selected_task.as_deref(), Some("a-1"));
        assert_eq!(device_file.projects["/work/a"].agent_names, original.projects["/work/a"].agent_names);
        assert_eq!(device_file.dialogs, original.dialogs);
        assert!(!device_file.git.auto_fetch);
        assert!(!server_file.git.remove_worktrees);
        // Nothing is lost: the two halves put together are the old file.
        let mut together = device_half(&device_file);
        merge_patch(&mut together, &server_half(&server_file));
        assert_eq!(parse(&together.to_string()), Outcome::Ok(original.clone()));
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
}
