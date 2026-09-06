//! What the app remembers between runs: `model.rs` is the schema and the pure
//! rules, `file.rs` is the disk, `commands.rs` is the two thin commands the
//! front end calls.
//!
//! The two functions here are for the rest of the app rather than for the front
//! end: a caller that wants one value out of the file, with no project to
//! resolve against and nobody to report a failure to.

pub mod commands;
pub mod file;
pub mod model;

use std::path::PathBuf;

use tauri::{AppHandle, Manager};

/// Where the file lives. `None` only when the platform will not name a config
/// directory at all, which costs the caller the same as a missing file does.
///
/// `commands::settings_path` builds the same path and keeps its own error type,
/// because a command has somebody to tell and these callers do not.
pub fn path(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_config_dir().ok().map(|dir| dir.join("settings.json"))
}

/// Which CLI agent the app is configured to start for one kind of call, and
/// which model it is to be asked for: a role that chose nothing inherits the
/// root pair, whole.
///
/// Read from the disk on each call rather than cached anywhere: there is no
/// settings worker, and one read costs milliseconds — the same reasoning that
/// keeps `files/` and `git.rs` out of a worker. A caller that needs the answer
/// to hold still for a while is the one that keeps a copy of it; `runs::service`
/// does exactly that, for the life of a run.
///
/// The rule itself is `Settings::role_pair`, which is pure; this is the half
/// that touches the file, and it is the head of the family below.
pub fn role_pair(app: &AppHandle, role: crate::agents::Role) -> (String, String) {
    path(app).map(|path| file::role_pair(&path, role)).unwrap_or_else(|| {
        let shipped = model::Settings::default();
        (shipped.agent, shipped.model)
    })
}

/// Which harness and which model this intent's role asks for — the one resolver,
/// called where `languages` is called, so that a person's session and a run's
/// batch get the same answer by construction rather than because two call sites
/// were kept in step.
///
/// `chosen` is a harness the caller already holds and will not give up: a run
/// snapshots its own when it starts and carries it for the whole of the run, so
/// that the allowance gate and the batches cannot land on two different
/// harnesses (smetana-3fi). `None` is a caller with no opinion — the front end,
/// which knows nothing about roles — and lets the file decide both halves.
///
/// **A harness the caller pinned arrives without a model.** The pair is
/// indivisible for the reason `AgentRole` records, and this is that rule at the
/// seam: a model chosen against one harness, handed to another, is a session
/// that dies at spawn.
///
/// `ResumeSession` is given no model at all, whatever the file says — see
/// `agents::Launch::model`.
pub fn role_model(
    app: &AppHandle,
    intent: &crate::agents::Intent,
    chosen: Option<&str>,
) -> (String, Option<String>) {
    let (mut agent, mut model) = role_pair(app, crate::agents::role_of(intent));
    if let Some(pinned) = chosen {
        if pinned != agent {
            agent = pinned.to_owned();
            model.clear();
        }
    }
    let resuming = matches!(intent, crate::agents::Intent::ResumeSession { .. });
    let model = (!model.is_empty() && !resuming).then_some(model);
    (agent, model)
}

/// The model a run's subagents are asked for: the `code` role's, resolved with
/// the same inheritance as everything else and read where every other session
/// fact is read.
///
/// Only the model, because only the model can be asked for: a subagent is
/// spawned inside the lead's own harness, so that role's harness half reaches
/// nothing here. `agents::prompt` turns this into one line of the run policy.
pub fn worker_model(app: &AppHandle) -> Option<String> {
    let (_, model) = role_pair(app, crate::agents::Role::Code);
    (!model.is_empty()).then_some(model)
}

/// The root pair, for the two callers that have no `Intent` to ask with: the
/// Git panel's commit-message button and the tracker's semantic search, both
/// one-shot calls with no session behind them. They take the `Default` role,
/// which is the root pair by definition — asked through `role_pair` all the
/// same, so that one place decides what "the default" means.
pub fn default_pair(app: &AppHandle) -> (String, Option<String>) {
    let (agent, model) = role_pair(app, crate::agents::Role::Default);
    (agent, (!model.is_empty()).then_some(model))
}

/// What a session started now should speak, and what it should write into bd.
///
/// Beside `role_pair` above and read the same way, from the disk on each call, and
/// it is deliberately read here rather than taken from `terminal_create`'s
/// arguments: `terminal::service` builds every session in the app, a person's
/// and a run's alike, so reading it once there is what keeps the two from
/// disagreeing. See the `Create` arm for the debounce this lives with.
pub fn languages(app: &AppHandle) -> crate::agents::Languages {
    path(app).map(|path| file::languages(&path)).unwrap_or_default()
}

/// What the person wants said in every session they are in.
///
/// Beside `languages` above, read the same way — from the disk on each call —
/// and by the same caller for the same reason: `terminal::service` builds every
/// session in the app, a person's and a run's alike, so reading it there once is
/// what keeps a second road into a session from existing at all. It lives with
/// the 400 ms debounce the languages already live with: a session started in the
/// same fraction of a second as an edit reads the previous text.
///
/// A platform that will not name a config directory answers with the empty
/// string, which is the shipped state and changes nothing.
pub fn agent_prompt(app: &AppHandle) -> String {
    path(app).map(|path| file::agent_prompt(&path)).unwrap_or_default()
}

/// The run gate's thresholds as `runs::usage` wants them. The schema's type and
/// the gate's are deliberately two types: one is a file people edit by hand and
/// has to tolerate anything, the other is a rule with no serde in it.
///
/// Beside `role_pair` above and read the same way, from the disk on each call — and
/// here, as with `updates_auto_check` below, that is the mechanism rather than
/// merely acceptable: the run loop asks at every gate check, which is what lets
/// a threshold moved while a run is paused take effect on *that* run.
pub fn subscription(app: &AppHandle) -> crate::runs::usage::Limits {
    subscription_at(path(app).as_deref())
}

/// The same read, for a caller that already holds the path — the run loop,
/// which is handed one when it starts and asks the file again at every gate
/// check. `None` is a platform that will not name a config directory, and it
/// answers with the shipped thresholds.
pub fn subscription_at(path: Option<&std::path::Path>) -> crate::runs::usage::Limits {
    let stored = path.map(file::subscription).unwrap_or_default();
    crate::runs::usage::Limits { pause_at: stored.pause_at, reduced_at: stored.reduced_at }
}

/// Whether a run may remove each task's worktree after it is merged and closed.
///
/// Beside `role_pair` above and read the same way, and by the same caller for the
/// same reason: `runs::service` reads it once when a run starts and carries it
/// for the whole of the run, so a night's batches all work to one answer rather
/// than to whatever the file said when each of them happened to spawn.
///
/// A platform that will not name a config directory answers `true`, the shipped
/// state — the same fallback `file::git_remove_worktrees` makes, and for the
/// reason written there.
pub fn git_remove_worktrees(app: &AppHandle) -> bool {
    path(app)
        .map(|path| file::git_remove_worktrees(&path))
        .unwrap_or_else(|| model::Settings::default().git.remove_worktrees)
}

/// Whether the update timer may ask the release feed by itself.
///
/// Beside `role_pair` above and read the same way, from the disk on each call — and
/// here that is not merely acceptable but the mechanism: `updates::schedule`
/// asks at every tick, which is what lets the switch stop and restart the
/// scheduled check without a restart of the app. The opposite of
/// `git_remove_worktrees`' caller, which reads once and carries the answer for
/// the whole of a run.
///
/// A platform that will not name a config directory answers `true`, the shipped
/// state — the same fallback `file::updates_auto_check` makes, and for the
/// reason written there.
pub fn updates_auto_check(app: &AppHandle) -> bool {
    path(app)
        .map(|path| file::updates_auto_check(&path))
        .unwrap_or_else(|| model::Settings::default().updates.auto_check)
}

/// Which branch this project's work is aimed at, as the run dialog was last
/// left here.
///
/// Beside `role_pair` above and read the same way, from the disk on each call, and
/// here for `updates_auto_check`'s reason rather than for convenience: the
/// caller is the tracker's sixty-second sweep, so a branch chosen a minute ago
/// takes effect on the next tick with no restart.
///
/// `None` where the file names none, and the caller falls back to the project's
/// own `[defaults] target_branch` — the two sources, in that order, that the run
/// dialog itself opens on.
pub fn target_branch(app: &AppHandle, project: &str) -> Option<String> {
    file::run_target_branch(&path(app)?, project)
}

/// How big one dialog window was left. `None` — including when there is no
/// settings path at all — asks for the height the content comes to.
pub fn dialog_size(app: &AppHandle, kind: &str) -> Option<model::DialogSize> {
    file::dialog_size(&path(app)?, kind)
}

/// Keeps how big one dialog window was left. A failure is a warning and nothing
/// more: the window on screen is the size the person made it either way, and
/// what is lost is that size at the next opening.
pub fn remember_dialog_size(app: &AppHandle, kind: &str, size: model::DialogSize) {
    let Some(path) = path(app) else {
        return;
    };
    if let Err(err) = file::remember_dialog_size(&path, kind, size) {
        log::warn!("settings: the dialog size was not kept: {err}");
    }
}
