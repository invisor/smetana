//! The allowlisted commands, called without Tauri.
//!
//! Each arm calls the plain function the `#[tauri::command]` wrapper of the same
//! name calls (`tracker::commands::snapshot`, `session::commands::plain::send`, …),
//! so a phone reaches exactly the code the desktop front end does, with the same
//! argument validation and the same errors. A handler's failure is serialized
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

use remote_protocol::BridgeError;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

use super::bridge::{BoxFuture, Dispatch};
use crate::agents::Intent;
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
}

impl Services {
    /// The managed state of a running app. `None` while any of it is missing,
    /// which at the end of `setup` it never is.
    pub fn from_app(app: &AppHandle) -> Option<Self> {
        Some(Self {
            tracker: app.try_state::<TrackerHandle>()?.inner().clone(),
            runs: app.try_state::<RunHandle>()?.inner().clone(),
            sessions: app.try_state::<SessionHandle>()?.inner().clone(),
            terminal: app.try_state::<TerminalHandle>()?.inner().clone(),
            settings_path: crate::settings::commands::settings_path(app)
                .map_err(|err| err.to_string()),
        })
    }
}

#[derive(Deserialize)]
#[cfg_attr(test, serde(deny_unknown_fields))]
pub(crate) struct Project {
    project: String,
}

#[derive(Deserialize)]
#[cfg_attr(test, serde(deny_unknown_fields))]
pub(crate) struct SetProject {
    path: Option<String>,
}

#[derive(Deserialize)]
#[cfg_attr(test, serde(deny_unknown_fields))]
pub(crate) struct SettingsLoad {
    project: Option<String>,
}

#[derive(Deserialize)]
#[cfg_attr(test, serde(deny_unknown_fields))]
pub(crate) struct RunStart {
    project: String,
    settings: RunSettings,
}

#[derive(Deserialize)]
#[cfg_attr(test, serde(deny_unknown_fields))]
pub(crate) struct RunStop {
    token: u64,
}

#[derive(Deserialize)]
#[cfg_attr(test, serde(deny_unknown_fields))]
pub(crate) struct SessionStart {
    project: String,
    intent: Intent,
}

#[derive(Deserialize)]
#[cfg_attr(test, serde(deny_unknown_fields))]
pub(crate) struct SessionId {
    id: u64,
}

#[derive(Deserialize)]
#[cfg_attr(test, serde(deny_unknown_fields))]
pub(crate) struct SessionSince {
    id: u64,
    seq: u64,
}

#[derive(Deserialize)]
#[cfg_attr(test, serde(deny_unknown_fields))]
pub(crate) struct SessionSend {
    id: u64,
    text: String,
    attachments: Vec<String>,
}

#[derive(Deserialize)]
#[cfg_attr(test, serde(deny_unknown_fields))]
pub(crate) struct SessionAnswer {
    id: u64,
    question: String,
    decision: Decision,
    answers: Option<BTreeMap<String, String>>,
}

#[derive(Deserialize)]
#[cfg_attr(test, serde(deny_unknown_fields))]
pub(crate) struct CrewRoot {
    root: u64,
}

#[derive(Deserialize)]
#[cfg_attr(test, serde(deny_unknown_fields))]
pub(crate) struct CrewAt {
    root: u64,
    node: u64,
}

#[derive(Deserialize)]
#[cfg_attr(test, serde(deny_unknown_fields))]
pub(crate) struct CrewSend {
    root: u64,
    node: u64,
    text: String,
}

/// A command and its decoded arguments. Decoding is a step of its own so that a
/// test can hold the contract's example arguments to it without running anything.
pub(crate) enum Call {
    TrackerHealth,
    TrackerSnapshot,
    TrackerSetProject(SetProject),
    SettingsLoad(SettingsLoad),
    AgentsCatalog,
    AgentsInstalled,
    ProjectConfig(Project),
    TargetBranches(Project),
    RunState(Project),
    RunReports(Project),
    RunStart(RunStart),
    RunStop(RunStop),
    SessionStart(SessionStart),
    SessionAttach(SessionId),
    SessionSince(SessionSince),
    SessionSend(SessionSend),
    SessionAnswer(SessionAnswer),
    SessionStop(SessionId),
    CrewTree(CrewRoot),
    CrewAttach(CrewAt),
    CrewSend(CrewSend),
    CrewStop(CrewRoot),
    TerminalList(Project),
    TerminalAttach(SessionId),
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

impl Call {
    /// `cmd` must be allowlisted; the bridge checks that first. A name that is
    /// not answers `Forbidden` here too, so this cannot be the place a gap opens.
    pub(crate) fn parse(cmd: &str, args: Value) -> Result<Call, BridgeError> {
        Ok(match cmd {
            "tracker_health" => Call::TrackerHealth,
            "tracker_snapshot" => Call::TrackerSnapshot,
            "tracker_set_project" => Call::TrackerSetProject(decode(cmd, args)?),
            "settings_load" => Call::SettingsLoad(decode(cmd, args)?),
            "agents_catalog" => Call::AgentsCatalog,
            "agents_installed" => Call::AgentsInstalled,
            "project_config" => Call::ProjectConfig(decode(cmd, args)?),
            "target_branches" => Call::TargetBranches(decode(cmd, args)?),
            "run_state" => Call::RunState(decode(cmd, args)?),
            "run_reports" => Call::RunReports(decode(cmd, args)?),
            "run_start" => Call::RunStart(decode(cmd, args)?),
            "run_stop" => Call::RunStop(decode(cmd, args)?),
            "session_start" => Call::SessionStart(decode(cmd, args)?),
            "session_attach" => Call::SessionAttach(decode(cmd, args)?),
            "session_since" => Call::SessionSince(decode(cmd, args)?),
            "session_send" => {
                let send: SessionSend = decode(cmd, args)?;
                // The spec keeps "attachments from disk" off the phone: these are
                // paths on the desktop, and a phone has no business naming them.
                if !send.attachments.is_empty() {
                    return Err(BridgeError::Forbidden);
                }
                Call::SessionSend(send)
            }
            "session_answer" => Call::SessionAnswer(decode(cmd, args)?),
            "session_stop" => Call::SessionStop(decode(cmd, args)?),
            "crew_tree" => Call::CrewTree(decode(cmd, args)?),
            "crew_attach" => Call::CrewAttach(decode(cmd, args)?),
            "crew_send" => Call::CrewSend(decode(cmd, args)?),
            "crew_stop" => Call::CrewStop(decode(cmd, args)?),
            "terminal_list" => Call::TerminalList(decode(cmd, args)?),
            "terminal_attach" => Call::TerminalAttach(decode(cmd, args)?),
            _ => return Err(BridgeError::Forbidden),
        })
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

impl Services {
    pub(crate) async fn run(&self, call: Call) -> Result<Value, BridgeError> {
        use crate::{runs, session, terminal, tracker};
        match call {
            Call::TrackerHealth => answer(tracker::commands::health(&self.tracker).await),
            Call::TrackerSnapshot => answer(tracker::commands::snapshot(&self.tracker).await),
            Call::TrackerSetProject(a) => {
                answer(tracker::commands::set_project(&self.tracker, a.path).await)
            }
            Call::SettingsLoad(a) => match &self.settings_path {
                Ok(path) => answer(crate::settings::commands::load_from(path, a.project)),
                Err(message) => answer(Err::<(), _>(crate::settings::commands::SettingsError::Dir(
                    message.clone(),
                ))),
            },
            Call::AgentsCatalog => plain(blocking(crate::agents::commands::agents_catalog).await?),
            Call::AgentsInstalled => plain(blocking(crate::agents::commands::agents_installed).await?),
            Call::ProjectConfig(a) => {
                plain(blocking(move || runs::commands::project_config(a.project)).await?)
            }
            Call::TargetBranches(a) => {
                plain(blocking(move || runs::commands::target_branches(a.project)).await?)
            }
            Call::RunState(a) => answer(runs::commands::state(&self.runs, a.project).await),
            Call::RunReports(a) => plain(runs::commands::run_reports(a.project).await),
            Call::RunStart(a) => answer(runs::commands::start(&self.runs, a.project, a.settings).await),
            Call::RunStop(a) => answer(runs::commands::stop(&self.runs, a.token).await),
            Call::SessionStart(a) => {
                answer(session::commands::plain::start(&self.sessions, a.project, a.intent).await)
            }
            Call::SessionAttach(a) => answer(session::commands::plain::attach(&self.sessions, a.id).await),
            Call::SessionSince(a) => {
                answer(session::commands::plain::since(&self.sessions, a.id, a.seq).await)
            }
            Call::SessionSend(a) => answer(
                session::commands::plain::send(&self.sessions, a.id, a.text, a.attachments).await,
            ),
            Call::SessionAnswer(a) => answer(
                session::commands::plain::answer(&self.sessions, a.id, a.question, a.decision, a.answers)
                    .await,
            ),
            Call::SessionStop(a) => answer(session::commands::plain::stop(&self.sessions, a.id).await),
            Call::CrewTree(a) => answer(session::commands::plain::crew_tree(&self.sessions, a.root).await),
            Call::CrewAttach(a) => {
                answer(session::commands::plain::crew_attach(&self.sessions, a.root, a.node).await)
            }
            Call::CrewSend(a) => answer(
                session::commands::plain::crew_send(&self.sessions, a.root, a.node, a.text).await,
            ),
            Call::CrewStop(a) => answer(session::commands::plain::crew_stop(&self.sessions, a.root).await),
            Call::TerminalList(a) => answer(terminal::commands::plain::list(&self.terminal, a.project).await),
            Call::TerminalAttach(a) => answer(terminal::commands::plain::attach(&self.terminal, a.id).await),
        }
    }
}

impl Dispatch for Services {
    fn call<'a>(&'a self, cmd: &'a str, args: Value) -> BoxFuture<'a, Result<Value, BridgeError>> {
        Box::pin(async move { self.run(Call::parse(cmd, args)?).await })
    }
}
