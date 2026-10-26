//! Every server command, called without Tauri.
//!
//! Each arm calls the plain function the `#[tauri::command]` wrapper of the same
//! name calls (`tracker::commands::snapshot`, `session::commands::plain::send`, …),
//! so a remote device reaches exactly the code the desktop front end does, with the same
//! argument validation and the same errors. Which commands a device may call at all is
//! decided before this layer, by its kind (`remote_protocol::is_command_allowed_for`); what
//! this layer adds is the few places where the kind changes what a command does. A handler's failure is serialized
//! with serde as Tauri serializes it for the webview, and travels as
//! `BridgeError::Handler` in that form.
//!
//! Arguments are decoded here into one struct per command, named the way the
//! front end names them (camelCase), because Tauri does that renaming for a
//! command's parameters and this layer has to do the same by hand. Unknown keys
//! are ignored as Tauri ignores them — a newer phone may send more than an older
//! desktop reads — except in tests, where they are an error, so that the contract
//! example of a command's arguments cannot name a parameter nothing reads.

use std::collections::BTreeMap;
use std::path::PathBuf;

use remote_protocol::{BridgeError, DeviceKind};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use smetana_core::tracker::model::IssuePatch;
use smetana_core::vcs::model::OpKind;

use super::bridge::{BoxFuture, Caller, Dispatch};
use crate::agents::Intent;
use crate::context::Context;
use crate::runs::config;
use crate::runs::model::RunSettings;
use crate::runs::service::RunHandle;
use crate::session::model::Decision;
use crate::session::service::SessionHandle;
use crate::terminal::service::TerminalHandle;
use crate::tracker::service::TrackerHandle;

/// The workers and the one path the allowlisted commands need.
#[derive(Clone)]
pub struct Services {
    pub tracker: TrackerHandle,
    pub runs: RunHandle,
    pub sessions: SessionHandle,
    pub terminal: TerminalHandle,
    /// Where `settings.json` is, or why that is not known.
    pub settings_path: Result<PathBuf, String>,
    /// The attachment store, or why that is not known. A phone's pictures are
    /// written under it, and the paths a phone may name in a new task are
    /// confined to it.
    pub attachments_root: Result<PathBuf, String>,
    /// Told the folder after a project switch made over the bridge succeeds. The
    /// tracker worker holds the one active project, and a phone's switch moves
    /// only that; this is how the window is told to follow (see
    /// `projects.js::followRemoteProject`). Production emits the window event.
    pub project_switched: Option<ProjectSwitched>,
    /// What the commands that need more than a worker take: the data directory, the settings
    /// file, the agents' one-shot runs. `None` in a test that calls none of them.
    pub ctx: Option<Context>,
    /// The device service, for the commands about devices. Weak: the service owns the hub that
    /// owns this.
    pub remote: Option<std::sync::Weak<super::Remote>>,
    /// The restart gateway, on a server that can be restarted from a client. `None` in a test harness
    /// or any host nobody restarts from a client; then no restart control is advertised.
    pub gateway: Option<std::sync::Arc<super::gateway::Gateway>>,
}

pub type ProjectSwitched = std::sync::Arc<dyn Fn(Option<String>) + Send + Sync>;

/// The window event a remote device's project switch is announced with.
pub const PROJECT_SWITCHED_EVENT: &str = "remote:project-switched";

/// The payload of [`PROJECT_SWITCHED_EVENT`].
pub fn project_switched_payload(path: Option<String>) -> Value {
    json!({ "path": path })
}

impl Services {
    /// The [`ProjectSwitched`] a host wants: the window event, emitted through the context's sink.
    pub fn announce_project_switch(ctx: Context) -> ProjectSwitched {
        std::sync::Arc::new(move |path| ctx.emit_json(PROJECT_SWITCHED_EVENT, project_switched_payload(path)))
    }
}

/// The rejection Tauri gives a command whose arguments do not decode.
fn invalid(cmd: &str, err: serde_json::Error) -> BridgeError {
    BridgeError::Handler(json!(format!("invalid args for command `{cmd}`: {err}")))
}

fn decode<T: DeserializeOwned>(cmd: &str, args: Value) -> Result<T, BridgeError> {
    // No arguments at all is an empty object, as a call of a command with only
    // optional parameters from the front end is.
    let args = if args.is_null() { json!({}) } else { args };
    serde_json::from_value(args).map_err(|err| invalid(cmd, err))
}

/// What a phone may name as a new task's pictures and files: paths in the
/// attachment store, and nothing else. The agent that files the task runs on the desktop
/// and opens whatever path it is given, so a path outside the store would let a
/// phone point it at any file of the desktop's. A store that cannot be located
/// refuses every attachment (nothing observable reads as "no").
fn confine_new_task(root: &Result<PathBuf, String>, intent: &Intent) -> Result<(), BridgeError> {
    let Intent::NewTask { draft, .. } = intent else {
        return Ok(());
    };
    if draft.images.is_empty() && draft.files.is_empty() {
        return Ok(());
    }
    let Ok(root) = root else {
        return Err(BridgeError::Forbidden);
    };
    if draft
        .images
        .iter()
        .chain(draft.files.iter())
        .all(|path| crate::attachments::cleanup::in_store(root, std::path::Path::new(path)))
    {
        Ok(())
    } else {
        Err(BridgeError::Forbidden)
    }
}

/// A handler's answer as the front end would receive it.
fn answer<T: Serialize, E: Serialize>(result: Result<T, E>) -> Result<Value, BridgeError> {
    match result {
        Ok(value) => serde_json::to_value(value).map_err(|err| BridgeError::Internal(err.to_string())),
        Err(err) => Err(match serde_json::to_value(err) {
            Ok(value) => BridgeError::Handler(value),
            Err(err) => BridgeError::Internal(err.to_string()),
        }),
    }
}

/// An answer that cannot be a failure.
fn plain<T: Serialize>(value: T) -> Result<Value, BridgeError> {
    answer(Ok::<_, ()>(value))
}

/// Runs a synchronous command off the async workers, where Tauri would run it.
async fn blocking<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static) -> Result<T, BridgeError> {
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|err| BridgeError::Internal(format!("the command did not finish: {err}")))
}

/// Declares every command in one place: its argument struct (named the way the front end names
/// the parameters, camelCase — Tauri does that renaming for a command and this layer has to do
/// it by hand), the variant of [`Call`] that carries it, and the arm that runs it. A command's
/// name appears once, so the list a test holds against `SERVER_COMMANDS` cannot drift from the
/// arms.
macro_rules! calls {
    ($(
        $variant:ident = $cmd:literal { $($field:ident : $ty:ty),* $(,)? }
            => |$svc:ident, $caller:ident, $a:ident| $body:expr;
    )*) => {
        $(
            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase")]
            #[cfg_attr(test, serde(deny_unknown_fields))]
            #[allow(dead_code)]
            pub(crate) struct $variant { $(pub(crate) $field: $ty),* }
        )*

        /// A command and its decoded arguments. Decoding is a step of its own so that a
        /// test can hold the contract's example arguments to it without running anything.
        pub(crate) enum Call { $($variant($variant)),* }

        /// Every command this layer runs, in declaration order.
        impl Call {
            /// `cmd` must be a server command the device's kind may call; the bridge checks
            /// that first. A name that is not one answers `Forbidden` here too, so this cannot
            /// be the place a gap opens.
            pub(crate) fn parse(cmd: &str, args: Value) -> Result<Call, BridgeError> {
                Ok(match cmd {
                    $($cmd => Call::$variant(decode(cmd, args)?),)*
                    _ => return Err(BridgeError::Forbidden),
                })
            }
        }

        impl Services {
            #[allow(unused_variables)]
            pub(crate) async fn run(&self, caller: &Caller, call: Call) -> Result<Value, BridgeError> {
                match call {
                    $(Call::$variant($a) => {
                        let $svc = self;
                        let $caller = caller;
                        $body
                    })*
                }
            }
        }
    };
}

impl Services {
    fn ctx(&self) -> Result<&Context, BridgeError> {
        self.ctx.as_ref().ok_or_else(|| BridgeError::Internal("the server has no context".into()))
    }

    fn gateway(&self) -> Result<std::sync::Arc<super::gateway::Gateway>, BridgeError> {
        self.gateway
            .clone()
            .ok_or_else(|| BridgeError::Handler(json!("this server cannot be restarted from a client")))
    }

    /// Where `server_file_read` may read: the attachment store and the report folder of every
    /// project the server's settings know. Read afresh at each call, so a project added a moment ago
    /// counts and one removed does not.
    fn artifact_roots(&self) -> crate::placement::ArtifactRoots {
        let mut projects: Vec<PathBuf> = Vec::new();
        if let Ok(path) = &self.settings_path {
            let (settings, _) = crate::settings::file::load(path);
            for project in settings.open_projects.iter().chain(settings.projects.keys()) {
                let project = PathBuf::from(project);
                if !projects.contains(&project) {
                    projects.push(project);
                }
            }
        }
        crate::placement::ArtifactRoots { attachments: self.attachments_root.clone().ok(), projects }
    }

    fn remote(&self) -> Result<std::sync::Arc<super::Remote>, BridgeError> {
        self.remote
            .as_ref()
            .and_then(std::sync::Weak::upgrade)
            .ok_or_else(|| BridgeError::Handler(json!("the companion is not available on this machine")))
    }
}

calls! {
    // --- the tracker
    TrackerHealth = "tracker_health" {} => |s, c, a| answer(crate::tracker::commands::health(&s.tracker).await);
    TrackerSnapshot = "tracker_snapshot" {} => |s, c, a| answer(crate::tracker::commands::snapshot(&s.tracker).await);
    TrackerResync = "tracker_resync" {} => |s, c, a| answer(crate::tracker::commands::tracker_resync(&s.tracker).await);
    TrackerUpdate = "tracker_update" { id: String, patch: IssuePatch }
        => |s, c, a| answer(crate::tracker::commands::tracker_update(&s.tracker, a.id, a.patch).await);
    TrackerClose = "tracker_close" { id: String, reason: Option<String> }
        => |s, c, a| answer(crate::tracker::commands::tracker_close(&s.tracker, a.id, a.reason).await);
    TrackerReopen = "tracker_reopen" { id: String }
        => |s, c, a| answer(crate::tracker::commands::tracker_reopen(&s.tracker, a.id).await);
    TrackerDelete = "tracker_delete" { id: String }
        => |s, c, a| answer(crate::tracker::commands::tracker_delete(&s.tracker, a.id).await);
    TrackerSetProject = "tracker_set_project" { path: Option<String> } => |s, c, a| {
        let path = a.path.clone();
        let result = crate::tracker::commands::set_project(&s.tracker, a.path).await;
        if result.is_ok() {
            if let Some(announce) = &s.project_switched {
                announce(path);
            }
        }
        answer(result)
    };
    TrackerInit = "tracker_init" {} => |s, c, a| answer(crate::tracker::commands::tracker_init(&s.tracker).await);
    TrackerRepair = "tracker_repair" {} => |s, c, a| answer(crate::tracker::commands::tracker_repair(&s.tracker).await);
    TrackerFailure = "tracker_failure" {} => |s, c, a| answer(crate::tracker::commands::tracker_failure(&s.tracker).await);
    TrackerAccessRepair = "tracker_access_repair" {}
        => |s, c, a| answer(crate::tracker::commands::tracker_access_repair(&s.tracker).await);
    TrackerProbe = "tracker_probe" { paths: Vec<String> }
        => |s, c, a| plain(crate::tracker::commands::tracker_probe(a.paths).await);
    TrackerSearchSemantic = "tracker_search_semantic" { query: String, project: Option<String> }
        => |s, c, a| answer(crate::tracker::commands::tracker_search_semantic(s.ctx()?, &s.tracker, a.query, a.project).await);
    ProjectRoot = "project_root" { path: String }
        => |s, c, a| plain(crate::tracker::commands::project_root(a.path).await);
    HomeDir = "home_dir" {}
        => |s, c, a| plain(crate::tracker::access::home().map(|home| home.to_string_lossy().into_owned()));

    // --- settings, agents, projects
    SettingsLoad = "settings_load" { project: Option<String> } => |s, c, a| match &s.settings_path {
        Ok(path) => answer(crate::settings::commands::load_from(path, a.project)),
        Err(message) => answer(Err::<(), _>(crate::settings::commands::SettingsError::Dir(message.clone()))),
    };
    SettingsSave = "settings_save" { settings: crate::settings::model::ResolvedSettings }
        => |s, c, a| answer(crate::settings::commands::settings_save(s.ctx()?, a.settings).await);
    SettingsPatch = "settings_patch" { patch: Value }
        => |s, c, a| answer(crate::settings::commands::settings_patch(s.ctx()?, a.patch).await);
    SettingsAdopt = "settings_adopt" { patch: Value }
        => |s, c, a| answer(crate::settings::commands::settings_adopt(s.ctx()?, a.patch).await);
    SettingsDeviceHalf = "settings_device_half" {}
        => |s, c, a| answer(crate::settings::commands::settings_device_half(s.ctx()?).await);
    AgentsCatalog = "agents_catalog" {} => |s, c, a| plain(blocking(crate::agents::catalogue).await?);
    AgentsInstalled = "agents_installed" {}
        => |s, c, a| plain(blocking(crate::agents::commands::agents_installed).await?);
    CodexModels = "codex_models" {} => |s, c, a| answer(crate::agents::commands::codex_models().await);
    ProjectConfig = "project_config" { project: String }
        => |s, c, a| plain(blocking(move || crate::runs::commands::project_config(a.project)).await?);
    ProjectConfigSaveDefaults = "project_config_save_defaults" { project: String, defaults: config::Defaults }
        => |s, c, a| answer(blocking(move || crate::runs::commands::project_config_save_defaults(a.project, a.defaults)).await?);
    TargetBranches = "target_branches" { project: String }
        => |s, c, a| plain(blocking(move || crate::runs::commands::target_branches(a.project)).await?);
    GitHead = "git_head" { project: String }
        => |s, c, a| plain(blocking(move || smetana_core::git::commands::git_head(a.project)).await?);

    // --- runs
    RunState = "run_state" { project: String } => |s, c, a| answer(crate::runs::commands::state(&s.runs, a.project).await);
    RunReports = "run_reports" { project: String } => |s, c, a| plain(crate::runs::commands::run_reports(a.project).await);
    RunStart = "run_start" { project: String, settings: RunSettings }
        => |s, c, a| answer(crate::runs::commands::start(&s.runs, a.project, a.settings).await);
    RunStop = "run_stop" { token: u64 } => |s, c, a| answer(crate::runs::commands::stop(&s.runs, a.token).await);
    RunRelease = "run_release" {} => |s, c, a| answer(crate::runs::commands::run_release(&s.runs).await);
    BrowserTools = "browser_tools" { project: String }
        => |s, c, a| plain(crate::runs::commands::browser_tools(Some(&s.runs), a.project).await);
    AgentUsage = "agent_usage" { agent: Option<String>, project: Option<String> }
        => |s, c, a| plain(crate::runs::commands::agent_usage(s.ctx()?.clone(), a.agent, a.project).await);

    // --- driven sessions and crews
    SessionStart = "session_start" { project: String, intent: Intent } => |s, c, a| {
        // A phone's new task may name only what is already in the attachment store; a desktop
        // is the owner's own machine and names what it likes.
        if c.kind == DeviceKind::Phone {
            confine_new_task(&s.attachments_root, &a.intent)?;
        }
        answer(crate::session::commands::plain::start(&s.sessions, a.project, a.intent).await)
    };
    SessionAttach = "session_attach" { id: u64 }
        => |s, c, a| answer(crate::session::commands::plain::attach(&s.sessions, a.id).await);
    SessionSince = "session_since" { id: u64, seq: u64 }
        => |s, c, a| answer(crate::session::commands::plain::since(&s.sessions, a.id, a.seq).await);
    SessionSend = "session_send" { id: u64, text: String, attachments: Vec<String> } => |s, c, a| {
        // The spec keeps "attachments from disk" off the phone: these are paths on the
        // desktop, and a phone has no business naming them.
        if c.kind == DeviceKind::Phone && !a.attachments.is_empty() {
            return Err(BridgeError::Forbidden);
        }
        answer(crate::session::commands::plain::send(&s.sessions, a.id, a.text, a.attachments).await)
    };
    SessionAnswer = "session_answer" {
        id: u64, question: String, decision: Decision, answers: Option<BTreeMap<String, String>>
    } => |s, c, a| answer(
        crate::session::commands::plain::answer(&s.sessions, a.id, a.question, a.decision, a.answers).await
    );
    SessionStop = "session_stop" { id: u64 }
        => |s, c, a| answer(crate::session::commands::plain::stop(&s.sessions, a.id).await);
    SessionClose = "session_close" { id: u64 }
        => |s, c, a| answer(crate::session::commands::session_close(&s.sessions, a.id).await);
    CrewTree = "crew_tree" { root: u64 }
        => |s, c, a| answer(crate::session::commands::plain::crew_tree(&s.sessions, a.root).await);
    CrewAttach = "crew_attach" { root: u64, node: u64 }
        => |s, c, a| answer(crate::session::commands::plain::crew_attach(&s.sessions, a.root, a.node).await);
    CrewSend = "crew_send" { root: u64, node: u64, text: String }
        => |s, c, a| answer(crate::session::commands::plain::crew_send(&s.sessions, a.root, a.node, a.text).await);
    CrewStop = "crew_stop" { root: u64 }
        => |s, c, a| answer(crate::session::commands::plain::crew_stop(&s.sessions, a.root).await);
    CrewClear = "crew_clear" { root: u64 }
        => |s, c, a| answer(crate::session::commands::crew_clear(&s.sessions, a.root).await);
    SessionsList = "sessions_list" { project: String }
        => |s, c, a| plain(smetana_core::sessions::commands::sessions_list(a.project).await);
    SessionsDelete = "sessions_delete" { path: String }
        => |s, c, a| answer(smetana_core::sessions::commands::sessions_delete(a.path).await);

    // --- terminals
    TerminalList = "terminal_list" { project: String }
        => |s, c, a| answer(crate::terminal::commands::plain::list(&s.terminal, a.project).await);
    // A peek and not an attach: a client over a connection must not take the window's seat. The
    // bridge registers the client for the session's output around this call.
    TerminalAttach = "terminal_attach" { id: u64, after: Option<u64> }
        => |s, c, a| answer(crate::terminal::commands::plain::peek(&s.terminal, a.id, a.after).await);
    TerminalDetach = "terminal_detach" { id: u64 } => |s, c, a| {
        // The bridge has already stopped sending this client the session's output; the
        // worker's own detach is about the window's seat, which is not this client's.
        plain(())
    };
    TerminalMarks = "terminal_marks" {} => |s, c, a| answer(crate::terminal::commands::terminal_marks(&s.terminal).await);
    TerminalCreate = "terminal_create" { project: String, intent: Intent }
        => |s, c, a| answer(crate::terminal::commands::terminal_create(&s.terminal, a.project, a.intent).await);
    TerminalShell = "terminal_shell" { project: String, cwd: Option<String> }
        => |s, c, a| answer(crate::terminal::commands::terminal_shell(&s.terminal, a.project, a.cwd).await);
    TerminalRemove = "terminal_remove" { id: u64 }
        => |s, c, a| answer(crate::terminal::commands::terminal_remove(&s.terminal, a.id).await);
    TerminalResize = "terminal_resize" { id: u64, cols: u16, rows: u16 }
        => |s, c, a| answer(crate::terminal::commands::plain::resize_as(&s.terminal, c.id, a.id, a.cols, a.rows).await);
    TerminalWrite = "terminal_write" { id: u64, data: String }
        => |s, c, a| answer(crate::terminal::commands::plain::write_as(&s.terminal, c.id, a.id, a.data).await);
    TerminalClear = "terminal_clear" { id: u64 }
        => |s, c, a| answer(crate::terminal::commands::terminal_clear(&s.terminal, a.id).await);
    TerminalRunCapture = "terminal_run_capture" { id: u64, input: String, settle_ms: u64, timeout_ms: u64 }
        => |s, c, a| answer(crate::terminal::commands::terminal_run_capture(&s.terminal, a.id, a.input, a.settle_ms, a.timeout_ms).await);
    TerminalRestorable = "terminal_restorable" { project: String }
        => |s, c, a| answer(crate::terminal::commands::terminal_restorable(a.project).await);
    TerminalForget = "terminal_forget" { project: String, session_id: String }
        => |s, c, a| answer(crate::terminal::commands::terminal_forget(a.project, a.session_id).await);

    // --- files
    FilesList = "files_list" { root: String, dir: String }
        => |s, c, a| answer(smetana_core::files::commands::files_list(a.root, a.dir).await);
    FilesRead = "files_read" { root: String, path: String }
        => |s, c, a| answer(smetana_core::files::commands::files_read(a.root, a.path).await);
    FilesWrite = "files_write" { root: String, path: String, text: String, expected_mtime: i64 }
        => |s, c, a| answer(smetana_core::files::commands::files_write(a.root, a.path, a.text, a.expected_mtime).await);
    FilesStat = "files_stat" { root: String, paths: Vec<String> }
        => |s, c, a| plain(smetana_core::files::commands::files_stat(a.root, a.paths).await);
    FilesCreate = "files_create" { root: String, dir: String, name: String }
        => |s, c, a| answer(smetana_core::files::commands::files_create(a.root, a.dir, a.name).await);
    FilesMkdir = "files_mkdir" { root: String, dir: String, name: String }
        => |s, c, a| answer(smetana_core::files::commands::files_mkdir(a.root, a.dir, a.name).await);
    FilesTrash = "files_trash" { root: String, path: String }
        => |s, c, a| answer(smetana_core::files::commands::files_trash(a.root, a.path).await);
    FilesCopy = "files_copy" { root: String, src: String, dst_dir: String }
        => |s, c, a| answer(smetana_core::files::commands::files_copy(a.root, a.src, a.dst_dir).await);
    FilesCopyExternal = "files_copy_external" { root: String, src: String, dst_dir: String }
        => |s, c, a| answer(smetana_core::files::commands::files_copy_external(a.root, a.src, a.dst_dir).await);
    FilesMove = "files_move" { root: String, src: String, dst_dir: String }
        => |s, c, a| answer(smetana_core::files::commands::files_move(a.root, a.src, a.dst_dir).await);
    FilesRename = "files_rename" { root: String, path: String, name: String }
        => |s, c, a| answer(smetana_core::files::commands::files_rename(a.root, a.path, a.name).await);

    // --- attachments
    AttachmentWrite = "attachment_write" { name: Option<String>, data: String } => |s, c, a| {
        let root = s.attachments_root.clone().map_err(|_| BridgeError::Forbidden)?;
        let project = crate::attachments::current_project(&s.tracker).await;
        answer(crate::attachments::plain::write(&root, project.as_deref(), a.name, a.data))
    };
    AttachmentImport = "attachment_import" { path: String }
        => |s, c, a| answer(crate::attachments::import_file(s.ctx()?, Some(&s.tracker), a.path).await);
    AttachmentReopen = "attachment_reopen" { path: String }
        => |s, c, a| answer(crate::attachments::reopen_stored(s.ctx()?, a.path).await);
    AttachmentsSurvey = "attachments_survey" {}
        => |s, c, a| answer(crate::attachments::survey_store(s.ctx()?, Some(&s.tracker)).await);
    AttachmentsClean = "attachments_clean" {}
        => |s, c, a| answer(crate::attachments::clean_store(s.ctx()?, Some(&s.tracker)).await);
    ImageRead = "image_read" { base: String, src: String }
        => |s, c, a| answer(crate::attachments::figure::image_read(a.base, a.src).await);

    // --- git
    VcsRepos = "vcs_repos" { project: String } => |s, c, a| plain(smetana_core::vcs::commands::vcs_repos(a.project).await);
    VcsStatus = "vcs_status" { repo: String } => |s, c, a| answer(smetana_core::vcs::commands::vcs_status(a.repo).await);
    VcsInProgress = "vcs_in_progress" { repo: String }
        => |s, c, a| answer(smetana_core::vcs::commands::vcs_in_progress(a.repo).await);
    VcsBranches = "vcs_branches" { repo: String } => |s, c, a| plain(smetana_core::vcs::commands::vcs_branches(a.repo).await);
    VcsRemoteBranches = "vcs_remote_branches" { repo: String }
        => |s, c, a| plain(smetana_core::vcs::commands::vcs_remote_branches(a.repo).await);
    VcsLastFetch = "vcs_last_fetch" { repo: String }
        => |s, c, a| plain(smetana_core::vcs::commands::vcs_last_fetch(a.repo).await);
    VcsTracking = "vcs_tracking" { repo: String } => |s, c, a| plain(smetana_core::vcs::commands::vcs_tracking(a.repo).await);
    VcsFetch = "vcs_fetch" { repo: String } => |s, c, a| answer(smetana_core::vcs::commands::vcs_fetch(a.repo).await);
    VcsPull = "vcs_pull" { repo: String } => |s, c, a| answer(smetana_core::vcs::commands::vcs_pull(a.repo).await);
    VcsPush = "vcs_push" { repo: String, set_upstream: bool }
        => |s, c, a| answer(smetana_core::vcs::commands::vcs_push(a.repo, a.set_upstream).await);
    VcsCheckout = "vcs_checkout" { repo: String, branch: String }
        => |s, c, a| answer(smetana_core::vcs::commands::vcs_checkout(a.repo, a.branch).await);
    VcsCheckoutRemote = "vcs_checkout_remote" { repo: String, branch: String }
        => |s, c, a| answer(smetana_core::vcs::commands::vcs_checkout_remote(a.repo, a.branch).await);
    VcsCreateBranch = "vcs_create_branch" { repo: String, name: String, start: String, switch: bool }
        => |s, c, a| answer(smetana_core::vcs::commands::vcs_create_branch(a.repo, a.name, a.start, a.switch).await);
    VcsRenameBranch = "vcs_rename_branch" { repo: String, from: String, to: String }
        => |s, c, a| answer(smetana_core::vcs::commands::vcs_rename_branch(a.repo, a.from, a.to).await);
    VcsDeleteBranch = "vcs_delete_branch" { repo: String, branch: String, force: bool }
        => |s, c, a| answer(smetana_core::vcs::commands::vcs_delete_branch(a.repo, a.branch, a.force).await);
    VcsMerge = "vcs_merge" { repo: String, branch: String }
        => |s, c, a| answer(smetana_core::vcs::commands::vcs_merge(a.repo, a.branch).await);
    VcsRebase = "vcs_rebase" { repo: String, onto: String }
        => |s, c, a| answer(smetana_core::vcs::commands::vcs_rebase(a.repo, a.onto).await);
    VcsAbort = "vcs_abort" { repo: String, op: OpKind }
        => |s, c, a| answer(smetana_core::vcs::commands::vcs_abort(a.repo, a.op).await);
    VcsCommit = "vcs_commit" { repo: String, message: String }
        => |s, c, a| answer(smetana_core::vcs::commands::vcs_commit(a.repo, a.message).await);
    VcsDiscard = "vcs_discard" { repo: String, path: String, orig_path: Option<String> }
        => |s, c, a| answer(smetana_core::vcs::commands::vcs_discard(a.repo, a.path, a.orig_path).await);
    VcsSuggestMessage = "vcs_suggest_message" { repo: String, project: Option<String> }
        => |s, c, a| answer(smetana_core::vcs::commands::vcs_suggest_message(s.ctx()?, a.repo, a.project).await);
    VcsFileAtHead = "vcs_file_at_head" { repo: String, path: String }
        => |s, c, a| answer(smetana_core::vcs::commands::vcs_file_at_head(a.repo, a.path).await);
    VcsFileAtRev = "vcs_file_at_rev" { repo: String, rev: String, path: String }
        => |s, c, a| answer(smetana_core::vcs::commands::vcs_file_at_rev(a.repo, a.rev, a.path).await);
    VcsCompare = "vcs_compare" { repo: String, branch: String, mode: String }
        => |s, c, a| answer(smetana_core::vcs::commands::vcs_compare(a.repo, a.branch, a.mode).await);

    // --- the server's own devices
    RemotePairBegin = "remote_pair_begin" { relay_url: String, kind: Option<DeviceKind> }
        => |s, c, a| answer(super::commands::plain::pair_begin(&s.remote()?, &a.relay_url, a.kind.unwrap_or_default()).await);
    RemotePairCancel = "remote_pair_cancel" {} => |s, c, a| answer(super::commands::plain::pair_cancel(&s.remote()?));
    RemotePairDecide = "remote_pair_decide" { accept: bool }
        => |s, c, a| answer(super::commands::plain::pair_decide(&s.remote()?, a.accept));
    RemoteDevices = "remote_devices" {} => |s, c, a| answer(super::commands::plain::devices(&s.remote()?));
    RemoteDeviceRevoke = "remote_device_revoke" { key: String }
        => |s, c, a| answer(super::commands::plain::revoke(&s.remote()?, &a.key));

    // --- the machine the server runs on
    ServerHealth = "server_health" {} => |s, c, a| plain(smetana_core::health::report(s.ctx()?).await);
    ServerRestart = "server_restart" { mode: super::gateway::Mode, confirmed: Option<Vec<super::gateway::LiveWork>> }
        => |s, c, a| plain(s.gateway()?.request(a.mode, a.confirmed).await);
    ServerWork = "server_work" {} => |s, c, a| plain(s.gateway()?.live_work().await);

    // --- a client that is not on this machine (protocol 1.11, `smetana_core::placement`)
    FoldersList = "folders_list" { path: Option<String> } => |s, c, a| {
        let root = crate::placement::browse_root()
            .ok_or_else(|| BridgeError::Handler(json!("the server user has no home folder to browse from")))?;
        answer(blocking(move || crate::placement::folders_list(&root, a.path.as_deref())).await?)
    };
    ProjectClone = "project_clone" { url: String, parent: String, name: Option<String> } => |s, c, a| {
        let root = crate::placement::browse_root()
            .ok_or_else(|| BridgeError::Handler(json!("the server user has no home folder to clone into")))?;
        answer(crate::placement::project_clone(&root, a.url, a.parent, a.name).await)
    };
    ServerFileRead = "server_file_read" { path: String } => |s, c, a| {
        let roots = s.artifact_roots();
        answer(blocking(move || crate::placement::file_fetch(&roots, &a.path)).await?)
    };
}

impl Dispatch for Services {
    fn serves(&self, cmd: &str) -> bool {
        match cmd {
            "server_restart" | "server_work" | "server:restart-deferred" => self.gateway.is_some(),
            "server_health" => self.ctx.is_some(),
            _ => true,
        }
    }

    fn call<'a>(
        &'a self,
        caller: &'a Caller,
        cmd: &'a str,
        args: Value,
    ) -> BoxFuture<'a, Result<Value, BridgeError>> {
        Box::pin(async move {
            // A restart is committed: nothing new may start, or it would be ended unlisted.
            if super::gateway::WORK_COMMANDS.contains(&cmd) && self.gateway.as_ref().is_some_and(|g| g.frozen()) {
                return Err(BridgeError::Handler(json!("the server is restarting; try again once it is back")));
            }
            self.run(caller, Call::parse(cmd, args)?).await
        })
    }

    fn terminal_ring<'a>(&'a self, session: u64) -> BoxFuture<'a, Option<(u64, String)>> {
        Box::pin(async move {
            let attached = crate::terminal::commands::plain::peek(&self.terminal, session, None).await.ok()?;
            Some((attached.seq, attached.data))
        })
    }

    fn client_gone<'a>(&'a self, caller: &'a Caller) -> BoxFuture<'a, ()> {
        Box::pin(async move {
            let _ = crate::terminal::commands::plain::forget_client(&self.terminal, caller.id).await;
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn new_task(images: &[&str]) -> Intent {
        new_task_with(images, &[])
    }

    fn new_task_with(images: &[&str], files: &[&str]) -> Intent {
        serde_json::from_value(json!({
            "kind": "newTask",
            "brainstorm": "auto",
            "spec": "auto",
            "plan": "auto",
            "draft": { "text": "x", "issue_type": null, "priority": null, "images": images, "files": files, "parent": null }
        }))
        .expect("a new task intent decodes")
    }

    #[test]
    fn a_phones_new_task_may_name_only_pictures_in_the_store() {
        let root = Ok(PathBuf::from("/data/attachments"));
        let inside = "/data/attachments/proj-abc/20261017-x.png";
        assert!(confine_new_task(&root, &new_task(&[inside])).is_ok());
        assert!(confine_new_task(&root, &new_task(&[])).is_ok());
        assert!(matches!(
            confine_new_task(&root, &new_task(&["/etc/passwd"])),
            Err(BridgeError::Forbidden)
        ));
        assert!(matches!(
            confine_new_task(&root, &new_task(&[inside, "/etc/passwd"])),
            Err(BridgeError::Forbidden)
        ));
        assert!(matches!(
            confine_new_task(&root, &new_task(&["/data/attachments/../secret.png"])),
            Err(BridgeError::Forbidden)
        ));
    }

    #[test]
    fn a_file_outside_the_store_is_forbidden_even_when_the_pictures_are_inside() {
        let root = Ok(PathBuf::from("/data/attachments"));
        let intent = new_task_with(&["/data/attachments/p/a.png"], &["/Users/you/secret.txt"]);
        assert!(matches!(confine_new_task(&root, &intent), Err(BridgeError::Forbidden)));
        let alone = new_task_with(&[], &["/Users/you/secret.txt"]);
        assert!(matches!(confine_new_task(&root, &alone), Err(BridgeError::Forbidden)));
    }

    #[test]
    fn files_inside_the_store_pass() {
        let root = Ok(PathBuf::from("/data/attachments"));
        let intent = new_task_with(
            &["/data/attachments/p/a.png"],
            &["/data/attachments/p/20261021-brief.pdf", "/data/attachments/p/20261021-notes.md"],
        );
        assert!(confine_new_task(&root, &intent).is_ok());
    }

    #[test]
    fn an_unlocatable_store_refuses_every_picture_but_not_a_task_without_any() {
        let root: Result<PathBuf, String> = Err("no app data".into());
        assert!(confine_new_task(&root, &new_task(&[])).is_ok());
        assert!(matches!(
            confine_new_task(&root, &new_task(&["/data/attachments/p/x.png"])),
            Err(BridgeError::Forbidden)
        ));
        assert!(matches!(
            confine_new_task(&root, &new_task_with(&[], &["/data/attachments/p/x.pdf"])),
            Err(BridgeError::Forbidden)
        ));
    }

    #[test]
    fn attachment_write_is_decoded_with_a_null_name() {
        let call = Call::parse("attachment_write", json!({ "name": null, "data": "aGk=" }))
            .unwrap_or_else(|_| panic!("the call decodes"));
        match call {
            Call::AttachmentWrite(a) => {
                assert_eq!(a.name, None);
                assert_eq!(a.data, "aGk=");
            }
            _ => panic!("expected an attachment_write call"),
        }
    }
}
