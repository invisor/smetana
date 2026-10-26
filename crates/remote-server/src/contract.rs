//! The contract test: the shape of everything a device may call or hear is pinned
//! in `crates/remote-protocol/contract/` — the phone's allowlist and, beyond it, every command
//! and event of the server — and this compares it with what the desktop really produces.
//!
//! **Replies and event payloads** are real values. Each is built here from the
//! types the handlers return (and, for events, the same payload functions the
//! emitters use), serialized by serde exactly as Tauri or the bridge does, and
//! compared to the pinned file field by field. One value of every variant of
//! every enum on the wire is built, because a renamed variant is as breaking as
//! a renamed field. A mismatch fails naming the command or event and the path of
//! the field:
//!
//! - a field the file has and the desktop no longer produces, or a value or type
//!   that changed, is a removal or a rename: **`PROTOCOL_MAJOR`**;
//! - a field the desktop produces and the file does not have is an addition: add
//!   it to the file and raise **`PROTOCOL_MINOR`**.
//!
//! **Arguments** are the other direction: the files are the source, and the test
//! feeds every example to the decoder the bridge uses (strictly: an example may
//! not carry a key nothing reads), so a renamed or newly required parameter
//! breaks the examples a phone is built on.
//!
//! The files are written by `SMETANA_UPDATE_CONTRACT=1 cargo test remote_contract`
//! (replies and payloads; arguments are edited by hand). That never touches
//! `versions.json`, which pins the digest of the contract at each protocol
//! version and is edited by a person together with the version bump: regenerating
//! after a rename fails `remote_contract_changes_only_with_a_protocol_version`.

use std::collections::BTreeMap;
use std::path::PathBuf;

use remote_protocol::{command_contract_path, event_contract_path, SERVER_COMMANDS, SERVER_EVENTS};
use serde::Serialize;
use serde_json::{json, Value};

use super::dispatch::Call;
use super::{PROTOCOL_MAJOR, PROTOCOL_MINOR};
use crate::attachments::Attachment;
use crate::agents::{AgentModel, AgentRow, Capabilities};
use crate::runs::commands::ProjectConfigReply;
use crate::runs::config::{self, ConfigState};
use crate::runs::model::{
    Run, RunMode, RunScope, RunSettings, RunState, StopReason,
};
use crate::runs::reports::{Head, ReportEntry};
use crate::runs::summary::{RunSummary, TaskLine, Tasks};
use crate::session::crew::{CrewNode, CrewState};
use crate::session::model::{
    Actor, Decision, Event, EventKind, SessionState as DrivenState,
};
use crate::session::service::{events_payload, Attached as DrivenAttached, CrewAttached, CrewEventsChange};
use crate::settings::model::ResolvedSettings;
use crate::terminal::model::{
    LogEntry, LogKind, Question, QuestionOption, Session, SessionState as TerminalState, SessionWork,
};
use crate::terminal::service::{removed_payload, Attached as TerminalAttached};
use crate::tracker::model::{ColumnDef, Delta, Dependency, Failure, Health, HealthState, Issue, Repair, Snapshot};
use smetana_core::attachments::cleanup::Tally;
use smetana_core::attachments::{Cleaned, Survey};
use smetana_core::files::model::{Entry, EntryKind, FileText, Listing, Stat};
use smetana_core::runs::browser::BrowserTools;
use smetana_core::runs::usage::{AgentUsage, Band, Unavailable, Usage};
use smetana_core::session::model::StateChange;
use smetana_core::session::service::CrewTreeChange;
use smetana_core::sessions::model::SessionSummary;
use smetana_core::terminal::model::{SessionMark, WorkKind};
use smetana_core::terminal::restore::Restorable;
use smetana_core::tracker::access::AccessRepair;
use smetana_core::tracker::commands::ProjectProbe;
use smetana_core::vcs::model::{
    Branch, Change, ChangeKind, CompareChange, Comparison, InProgress, Landed, MergeOutcome, OpKind,
    ProjectRepos, Repo, Tracking, WorkingTree,
};
use remote_protocol::{DeviceKind, Platform};
use super::devices::Device;
use super::{device_views, PairBegin, PairRequestPayload};

fn json_of<T: Serialize>(value: &T) -> Value {
    serde_json::to_value(value).expect("a contract value serializes")
}

fn all<T: Serialize>(values: &[T]) -> Vec<Value> {
    values.iter().map(json_of).collect()
}

// ---------------------------------------------------------------------------
// Real values, one of every variant.
// ---------------------------------------------------------------------------

fn issue(full: bool) -> Issue {
    let mut issue = Issue {
        id: "smetana-1".into(),
        title: "A task".into(),
        status: "open".into(),
        updated_at: "2026-10-15T10:00:00Z".into(),
        ..Issue::default()
    };
    if full {
        issue.description = Some("Why".into());
        issue.acceptance_criteria = Some("Done when".into());
        issue.design = Some("How".into());
        issue.notes = Some("Parked because".into());
        issue.priority = Some(1);
        issue.issue_type = Some("bug".into());
        issue.owner = Some("owner".into());
        issue.assignee = Some("smetana-run-1".into());
        issue.created_at = Some("2026-10-14T10:00:00Z".into());
        issue.created_by = Some("someone".into());
        issue.started_at = Some("2026-10-15T09:00:00Z".into());
        issue.closed_at = Some("2026-10-15T11:00:00Z".into());
        issue.close_reason = Some("Done".into());
        issue.comment_count = Some(2);
        issue.dependency_count = Some(1);
        issue.dependent_count = Some(3);
        issue.parent = Some("smetana-0".into());
        issue.labels = vec!["frontend".into()];
        issue.dependencies = vec![Dependency {
            issue_id: "smetana-1".into(),
            depends_on_id: "smetana-0".into(),
            kind: "parent-child".into(),
        }];
    }
    issue
}

fn columns() -> Vec<ColumnDef> {
    vec![
        ColumnDef { name: "open".into(), category: "active".into() },
        ColumnDef { name: "in_progress".into(), category: "wip".into() },
        ColumnDef { name: "closed".into(), category: "done".into() },
    ]
}

fn snapshots() -> Vec<Snapshot> {
    vec![
        Snapshot { generation: 3, columns: columns(), issues: vec![issue(true), issue(false)] },
        Snapshot { generation: 1, columns: Vec::new(), issues: Vec::new() },
    ]
}

fn healths() -> Vec<Health> {
    let states = [
        HealthState::Ok,
        HealthState::NoProject,
        HealthState::NotABeadsRepo,
        HealthState::BdVersionMismatch,
        HealthState::FolderRefused,
        HealthState::Error,
    ];
    let mut out: Vec<Health> = states
        .into_iter()
        .map(|state| Health { state, message: Some("bd said no".into()) })
        .collect();
    out.push(Health { state: HealthState::Ok, message: None });
    out
}

fn deltas() -> Vec<Delta> {
    vec![
        Delta {
            generation: 4,
            upserted: vec![issue(true), issue(false)],
            removed: vec!["smetana-9".into()],
            columns: Some(columns()),
        },
        Delta { generation: 5, upserted: Vec::new(), removed: vec!["smetana-8".into()], columns: None },
    ]
}

fn run_settings(scope: RunScope, mode: RunMode, full: bool) -> RunSettings {
    RunSettings {
        scope,
        mode,
        target_branch: "main".into(),
        create_target: full,
        min_priority: full.then_some(2),
        max_parallel_tasks: full.then_some(3),
        batch_pause_min: full.then_some(1),
        batch_pause_max: full.then_some(5),
        live_check: full,
        file_findings: full,
        instructions: full.then(|| "Be careful".into()),
    }
}

fn run(token: u64, state: RunState, full: bool) -> Run {
    Run {
        token,
        project: "/work/smetana".into(),
        settings: run_settings(RunScope::Queue, RunMode::Auto, full),
        primary_agent: "claude".into(),
        agent: "claude".into(),
        attempt: 1,
        state,
        transport: crate::runs::session::Transport::Driven,
        session: full.then_some(7),
        last_session: full.then_some(6),
        batches: 2,
        stopping: full,
        reduced: full.then_some(50),
        summary: full.then(|| RunSummary {
            seconds: 3600,
            tasks: Some(Tasks {
                closed: vec![TaskLine { id: "smetana-1".into(), title: "A task".into() }],
                parked: vec![TaskLine { id: "smetana-2".into(), title: "Another".into() }],
            }),
            report: Some("/work/smetana/.smetana/reports/run.html".into()),
        }),
    }
}

/// The run with a field of every kind filled in, and the one with none.
fn runs_basic() -> Vec<Run> {
    vec![
        run(1, RunState::Working { iteration: 2 }, true),
        run(2, RunState::Preflight, false),
    ]
}

/// A run for every state, and for every reason a run stops, and every kind of
/// scope, mode and transport.
fn runs_sweep() -> Vec<Run> {
    let states = vec![
        RunState::Preflight,
        RunState::Working { iteration: 1 },
        RunState::Deciding,
        RunState::Paused { pct: 90, resets: Some("2026-10-15T15:00:00Z".into()), spent: false },
        RunState::Paused { pct: 100, resets: None, spent: true },
        RunState::Resting { until: "2026-10-15T15:00:00Z".into(), minutes: 5 },
        RunState::WaitingForAgent { agent: "codex".into(), pct: 99, resets: Some("soon".into()) },
        RunState::WaitingForAnyAgent,
        RunState::SwitchingAgent { from: "claude".into(), to: "codex".into() },
    ];
    let reasons = vec![
        StopReason::QueueEmpty,
        StopReason::BatchDone,
        StopReason::NoProgress,
        StopReason::MaxIterations,
        StopReason::Crashed { attempts: 3 },
        StopReason::Unreadable,
        StopReason::Cancelled,
        StopReason::SessionRemoved,
        StopReason::NeedsAnswer { question: "Which one?".into() },
        StopReason::NothingDone { batches: 2 },
        StopReason::Preflight { detail: "no bd".into() },
    ];
    let mut out = Vec::new();
    for (i, state) in states.into_iter().enumerate() {
        out.push(run(i as u64 + 1, state, i % 2 == 0));
    }
    for reason in reasons {
        let token = out.len() as u64 + 1;
        out.push(run(token, RunState::Stopped { reason }, true));
    }
    let mut task = run(100, RunState::Deciding, false);
    task.settings = run_settings(RunScope::Task { id: "smetana-1".into() }, RunMode::Supervised, true);
    task.transport = crate::runs::session::Transport::Pty;
    let mut epic = run(101, RunState::Deciding, false);
    epic.settings = run_settings(RunScope::Epic { id: "smetana-2".into() }, RunMode::Solo, false);
    epic.transport = crate::runs::session::Transport::DrivenCrew;
    out.push(task);
    out.push(epic);
    out
}

fn agent_rows() -> Vec<AgentRow> {
    vec![AgentRow {
        id: "claude".into(),
        label: "Claude Code".into(),
        capabilities: Capabilities {
            resume: true,
            fork: true,
            clear: true,
            usage: true,
            batch: true,
            oneshot: true,
            sign_in: vec!["browser", "deviceCode"],
        },
        models: vec![AgentModel { id: "opus".into(), label: "Opus".into() }],
    }]
}

const CONFIG_TOML: &str = r#"
[project]
repos = ["."]

[defaults]
target_branch = "main"
min_priority = 2
max_parallel_tasks = 3
review_passes = 5

[repo."."]
setup = "npm install"
gates = ["npm test"]
env_files = [".env"]

[preflight]
commands = ["docker compose up -d"]
health = [{ url = "http://localhost:5173" }, { tcp = 5432 }]

[merge]
hazards = "Lockfile conflicts"
regenerate = [{ paths = ["package-lock.json"], command = "npm install" }]

[live_check]
mode = "browser"
command = "npm run dev"
notes = "Open the board"
"#;

fn config_replies() -> Vec<ProjectConfigReply> {
    let config = config::parse(CONFIG_TOML).expect("the contract's own config parses");
    vec![
        ProjectConfigReply { state: ConfigState::Ok { config: Box::new(config) }, empty: false },
        ProjectConfigReply { state: ConfigState::Missing, empty: true },
        ProjectConfigReply { state: ConfigState::Broken { message: "bad toml".into() }, empty: false },
    ]
}

fn report_entries() -> Vec<ReportEntry> {
    vec![
        ReportEntry {
            path: "/work/smetana/.smetana/reports/run.html".into(),
            file: "run.html".into(),
            stamp: "2026-10-15T11:00:00Z".into(),
            head: Head {
                title: Some("Run report".into()),
                scope: Some("the queue".into()),
                finished: Some("2026-10-15 11:00".into()),
                closed: Some(3),
                parked: Some(1),
                batches: Some(2),
                total: Some("1h".into()),
                summary: Some("Three tasks".into()),
                seconds: Some(3600),
            },
        },
        ReportEntry {
            path: "/work/smetana/.smetana/reports/old.html".into(),
            file: "old.html".into(),
            stamp: "2026-10-14T11:00:00Z".into(),
            head: Head::default(),
        },
    ]
}

fn branch_options() -> Vec<smetana_core::git::BranchOption> {
    vec![
        smetana_core::git::BranchOption { name: "main".into(), missing_in: vec!["web".into()], touched_at: Some(1_760_000_000) },
        smetana_core::git::BranchOption { name: "old".into(), missing_in: Vec::new(), touched_at: None },
    ]
}

fn ev(seq: u64, kind: EventKind) -> Event {
    Event { seq, at: "2026-10-15T10:00:00Z".into(), kind }
}

/// One event of every kind, and the variants of the kinds that carry an option.
fn conversation() -> Vec<Event> {
    let mut answers = BTreeMap::new();
    answers.insert("Which approach?".to_string(), "Rewrite".to_string());
    let kinds = vec![
        EventKind::TurnStart { by: Actor::Person },
        EventKind::TurnStart { by: Actor::Agent },
        EventKind::UserMessage { text: "Hello".into(), attachments: vec!["a.png".into()] },
        EventKind::Opening { text: Some("Start".into()), attachments: vec!["b.png".into()] },
        EventKind::Opening { text: None, attachments: Vec::new() },
        EventKind::Text { text: "Hi".into() },
        EventKind::TextQuestion,
        EventKind::TextDelta { text: "H".into() },
        EventKind::Reasoning { text: "Thinking".into() },
        EventKind::ToolUse { id: "t1".into(), name: "Bash".into(), detail: "ls".into() },
        EventKind::ToolResult { id: "t1".into(), ok: true, summary: "3 files".into(), denial: None },
        EventKind::Permission {
            id: "p1".into(),
            tool: "Bash".into(),
            detail: "rm x".into(),
            options: vec![Decision::Allow, Decision::AllowAlways, Decision::Deny],
            input: json!({ "command": "rm x" }),
        },
        EventKind::PermissionAnswered { id: "p1".into(), decision: Decision::Allow, answers: None },
        EventKind::PermissionAnswered { id: "p2".into(), decision: Decision::Deny, answers: Some(answers) },
        EventKind::Result { tokens_in: 10, tokens_out: 20, cost_usd: Some(0.5), ms: 400 },
        EventKind::Result { tokens_in: 1, tokens_out: 2, cost_usd: None, ms: 4 },
        EventKind::TurnFailed { text: "failed".into() },
        EventKind::Error { text: "oops".into() },
    ];
    kinds.into_iter().enumerate().map(|(i, kind)| ev(i as u64 + 1, kind)).collect()
}

fn driven_attached() -> Vec<DrivenAttached> {
    let states = [
        DrivenState::Starting,
        DrivenState::Running,
        DrivenState::Ready,
        DrivenState::NeedsYou,
        DrivenState::Exited,
        DrivenState::Failed,
    ];
    states
        .into_iter()
        .enumerate()
        .map(|(i, state)| DrivenAttached {
            events: if i == 0 { conversation() } else { Vec::new() },
            seq: 18,
            state,
            conversation: (i == 0).then(|| "conv-1".to_string()),
            cwd: "/work/smetana".into(),
            title: (i == 0).then(|| "Fix the board".to_string()),
            agent: "claude".into(),
            // Pinned on the first element only: the key is absent when `None`
            // (protocol 1.5), so the others keep the shape older phones know.
            model: (i == 0).then(|| "opus".to_string()),
        })
        .collect()
}

fn crew_nodes() -> Vec<CrewNode> {
    let states = [CrewState::Starting, CrewState::Running, CrewState::Waiting, CrewState::Done, CrewState::Failed];
    states
        .into_iter()
        .enumerate()
        .map(|(i, state)| CrewNode {
            id: i as u64 + 1,
            parent: (i > 0).then_some(1),
            root: 1,
            state,
            label: "worker".into(),
            can_message: i % 2 == 0,
        })
        .collect()
}

fn crew_attached() -> Vec<CrewAttached> {
    let states = [CrewState::Starting, CrewState::Running, CrewState::Waiting, CrewState::Done, CrewState::Failed];
    states
        .into_iter()
        .enumerate()
        .map(|(i, state)| CrewAttached {
            events: if i == 0 { conversation() } else { Vec::new() },
            seq: 18,
            state,
            cwd: "/work/smetana".into(),
        })
        .collect()
}

fn terminal_sessions() -> Vec<Session> {
    let works = vec![
        SessionWork::Bare,
        SessionWork::NewTask {
            text: "Add a thing".into(),
            issue_type: Some("feature".into()),
            priority: Some(2),
            parent: Some("smetana-0".into()),
        },
        SessionWork::NewTask { text: "Add".into(), issue_type: None, priority: None, parent: None },
        SessionWork::EditTask { id: "smetana-1".into() },
        SessionWork::ResolveTask { id: "smetana-1".into() },
        SessionWork::FixTask { id: "smetana-1".into() },
        SessionWork::ResolveConflict { repo: ".".into(), theirs: "main".into() },
        SessionWork::RepairTracker,
        SessionWork::ResumeSession { title: Some("Before".into()) },
        SessionWork::ResumeSession { title: None },
        SessionWork::Setup,
        SessionWork::Bootstrap,
        SessionWork::SignIn,
        SessionWork::ReviewBranch { report: "/r.html".into() },
        SessionWork::Run,
        SessionWork::Shell,
    ];
    let states = [
        TerminalState::Starting,
        TerminalState::Running,
        TerminalState::Idle,
        TerminalState::NeedsYou,
        TerminalState::Exited,
    ];
    works
        .into_iter()
        .enumerate()
        .map(|(i, work)| {
            let full = i == 0;
            Session {
                id: i as u64 + 1,
                agent: "claude".into(),
                cwd: "/work/smetana".into(),
                project: "/work/smetana".into(),
                state: states[i % states.len()],
                question: full.then(|| Question {
                    text: "Proceed?".into(),
                    options: vec![QuestionOption { label: "Yes".into(), send: "1".into() }],
                    selected: Some(0),
                }),
                started_at: "2026-10-15T10:00:00Z".into(),
                exit_code: full.then_some(0),
                work,
                conversation: full.then(|| "conv-1".to_string()),
                timeline: full,
            }
        })
        .collect()
}

/// One entry of every kind a batch's structured log can carry, a tool call both
/// with a `cwd` and without, a result both ways.
fn log_entries() -> Vec<LogEntry> {
    let at = || "2026-10-19T10:00:00+03:00".to_string();
    [
        LogKind::Note { text: "session start (model opus)".into() },
        LogKind::Say { text: "Reading `merging`.\n\nThen the diff.".into() },
        LogKind::Tool { id: "t1".into(), tool: "Bash".into(), command: "git status".into(), cwd: Some("/work/smetana".into()) },
        LogKind::Result { id: "t1".into(), ok: true, summary: "4 lines".into() },
        LogKind::Tool { id: "t2".into(), tool: "Read".into(), command: "/work/smetana/a.txt".into(), cwd: None },
        LogKind::Result { id: "t2".into(), ok: false, summary: "1 line".into() },
    ]
    .into_iter()
    .enumerate()
    .map(|(i, kind)| LogEntry { seq: i as u64 + 4, at: at(), kind })
    .collect()
}

// ---------------------------------------------------------------------------
// The rest of the server's surface (protocol 1.8): what only a desktop device calls.
//
// One example of each shape is pinned, and of every variant of an enum a reply carries; the
// breadth of the phone's own commands above is not repeated for commands whose replies are a
// single flat record.
// ---------------------------------------------------------------------------

fn attachment(picture: bool) -> Attachment {
    Attachment {
        path: "/data/attachments/proj-abc/20261017-x.png".into(),
        name: "20261017-x.png".into(),
        bytes: 2,
        mime: if picture { "image/png" } else { "application/octet-stream" }.into(),
        data: picture.then(|| "aGk=".to_string()),
    }
}

fn survey() -> Survey {
    Survey {
        store: Tally { files: 4, bytes: 4000 },
        project: Some("/work/smetana".into()),
        board: HealthState::Ok,
        kept: Tally { files: 3, bytes: 3000 },
        removable: Tally { files: 1, bytes: 1000 },
    }
}

fn changes() -> Vec<Change> {
    let kinds = [
        ChangeKind::Modified,
        ChangeKind::Added,
        ChangeKind::Deleted,
        ChangeKind::Renamed,
        ChangeKind::Copied,
        ChangeKind::TypeChanged,
        ChangeKind::Untracked,
        ChangeKind::Conflicted,
    ];
    kinds
        .into_iter()
        .enumerate()
        .map(|(i, kind)| Change {
            path: format!("src/file{i}.js"),
            orig_path: (kind == ChangeKind::Renamed).then(|| "src/old.js".to_string()),
            kind,
            staged: i % 2 == 0,
            unstaged: i % 3 == 0,
        })
        .collect()
}

fn landed(full: bool) -> Landed {
    if full {
        Landed { commits: Some(2), files: Some(3), insertions: Some(40), deletions: Some(5) }
    } else {
        Landed::default()
    }
}

fn session_marks() -> Vec<SessionMark> {
    let kinds = [
        WorkKind::Bare,
        WorkKind::NewTask,
        WorkKind::EditTask,
        WorkKind::ResolveTask,
        WorkKind::FixTask,
        WorkKind::ResolveConflict,
        WorkKind::RepairTracker,
        WorkKind::ResumeSession,
        WorkKind::Setup,
        WorkKind::Bootstrap,
        WorkKind::SignIn,
        WorkKind::ReviewBranch,
        WorkKind::Run,
        WorkKind::Shell,
    ];
    let states = [
        TerminalState::Starting,
        TerminalState::Running,
        TerminalState::Idle,
        TerminalState::NeedsYou,
        TerminalState::Exited,
    ];
    kinds
        .into_iter()
        .enumerate()
        .map(|(i, kind)| SessionMark {
            id: i as u64 + 1,
            project: "/work/smetana".into(),
            state: states[i % states.len()],
            kind,
        })
        .collect()
}

fn restorable() -> Vec<Restorable> {
    vec![Restorable {
        session_id: "conv-1".into(),
        agent: "claude".into(),
        cwd: "/work/smetana".into(),
        project: "/work/smetana".into(),
        work: SessionWork::NewTask { text: "Add a thing".into(), issue_type: None, priority: None, parent: None },
        started_at: "2026-10-15T10:00:00Z".into(),
        title: Some("Add a thing".into()),
    }]
}

fn agent_usages() -> Vec<AgentUsage> {
    vec![
        AgentUsage::Unsupported { agent: Some("codex".into()) },
        AgentUsage::Unsupported { agent: None },
        AgentUsage::Unreadable { agent: "claude".into(), reason: Unavailable::NotSignedIn },
        AgentUsage::Unreadable { agent: "claude".into(), reason: Unavailable::UnsupportedAccount },
        AgentUsage::Unreadable { agent: "claude".into(), reason: Unavailable::TimedOut },
        AgentUsage::Unreadable { agent: "claude".into(), reason: Unavailable::InvalidResponse },
        AgentUsage::Read {
            agent: "claude".into(),
            usage: Usage {
                session_pct: Some(40),
                session_reset: Some("in 2h".into()),
                session_label: Some("Current session".into()),
                week_pct: Some(10),
                week_reset: Some("Mon".into()),
                week_label: Some("Current week".into()),
                ..Usage::default()
            },
            band: Band::Normal,
            enumerates_windows: true,
        },
        AgentUsage::Read { agent: "claude".into(), usage: Usage::default(), band: Band::Reduced, enumerates_windows: false },
        AgentUsage::Read { agent: "claude".into(), usage: Usage::default(), band: Band::Pause, enumerates_windows: false },
    ]
}

fn devices() -> Vec<Device> {
    let device = |key: &str, platform, kind, push: bool| Device {
        key: key.repeat(32),
        name: "A device".into(),
        platform,
        push_token: push.then(|| "token".to_string()),
        push_key: "00".repeat(32),
        paired_at: 1_760_000_000,
        last_contact: push.then_some(1_760_000_100),
        kind,
        local: false,
    };
    vec![
        device("ab", Platform::Ios, DeviceKind::Phone, true),
        device("cd", Platform::Android, DeviceKind::Phone, false),
        device("ef", Platform::Desktop, DeviceKind::Desktop, false),
    ]
}

/// Every command beyond the phone's, with the examples of what it answers.
fn server_replies(map: &mut BTreeMap<&'static str, Vec<Value>>) {
    let nothing = || vec![Value::Null];
    let one_path = || vec![json!("src/new.js")];
    let clean = MergeOutcome::Clean { landed: landed(true) };
    let conflict = MergeOutcome::Conflict { files: vec!["src/a.js".into()] };

    map.insert("tracker_resync", all(&snapshots()));
    for name in ["tracker_update", "tracker_close", "tracker_reopen"] {
        map.insert(name, vec![json_of(&issue(true)), json_of(&issue(false))]);
    }
    map.insert("tracker_delete", nothing());
    map.insert("tracker_init", all(&snapshots()));
    map.insert(
        "tracker_repair",
        vec![json_of(&Repair {
            backup: "/work/smetana/.beads.bak".into(),
            output: "repaired".into(),
            snapshot: snapshots().remove(0),
        })],
    );
    map.insert(
        "tracker_failure",
        vec![json_of(&Failure {
            dir: "/work/smetana".into(),
            bd_version: "1.0.3".into(),
            command: "bd list --json".into(),
            stderr: "boom".into(),
        })],
    );
    map.insert(
        "tracker_access_repair",
        all(&[AccessRepair::Reset, AccessRepair::FullDiskAccess, AccessRepair::Unavailable]),
    );
    map.insert(
        "tracker_probe",
        vec![
            json_of(&vec![
                ProjectProbe { path: "/work/smetana".into(), tracked: true },
                ProjectProbe { path: "/work/other".into(), tracked: false },
            ]),
            json!([]),
        ],
    );
    map.insert("tracker_search_semantic", vec![json!(["smetana-1", "smetana-2"]), json!([])]);
    map.insert("project_root", vec![json!("/work/smetana")]);
    map.insert("home_dir", vec![json!("/Users/you"), Value::Null]);

    map.insert(
        "files_list",
        vec![json_of(&Listing {
            dir: "src".into(),
            entries: vec![
                Entry { name: "views".into(), path: "src/views".into(), kind: EntryKind::Dir, ignored: false },
                Entry { name: "main.js".into(), path: "src/main.js".into(), kind: EntryKind::File, ignored: false },
                Entry { name: "out.log".into(), path: "src/out.log".into(), kind: EntryKind::File, ignored: true },
            ],
            truncated: 0,
        })],
    );
    map.insert(
        "files_read",
        vec![json_of(&FileText { path: "src/main.js".into(), text: "export {}\n".into(), mtime: 1_760_000_000_000 })],
    );
    map.insert("files_write", vec![json!(1_760_000_000_001i64)]);
    map.insert(
        "files_stat",
        vec![json_of(&vec![
            Stat { path: "src/main.js".into(), mtime: Some(1_760_000_000_000) },
            Stat { path: "src/gone.js".into(), mtime: None },
        ])],
    );
    for name in ["files_create", "files_mkdir", "files_copy", "files_copy_external", "files_move", "files_rename"] {
        map.insert(name, one_path());
    }
    map.insert("files_trash", nothing());

    map.insert("attachment_import", vec![json_of(&attachment(true)), json_of(&attachment(false))]);
    map.insert("attachment_reopen", vec![json_of(&attachment(true))]);
    map.insert("image_read", vec![json_of(&attachment(true))]);
    map.insert("attachments_survey", vec![json_of(&survey())]);
    map.insert(
        "attachments_clean",
        vec![json_of(&Cleaned { removed: Tally { files: 1, bytes: 1000 }, failed: 0, survey: survey() })],
    );

    map.insert(
        "git_head",
        vec![
            json_of(&smetana_core::git::Head { branch: Some("main".into()), detached: None }),
            json_of(&smetana_core::git::Head { branch: None, detached: Some("abc1234".into()) }),
        ],
    );
    map.insert(
        "vcs_repos",
        vec![json_of(&ProjectRepos {
            repos: vec![
                Repo { name: ".".into(), path: "/work/smetana".into(), branch: Some("main".into()), detached: None },
                Repo { name: "web".into(), path: "/work/smetana/web".into(), branch: None, detached: Some("abc1234".into()) },
            ],
            unlisted: vec!["/work/smetana/extra".into()],
        })],
    );
    map.insert(
        "vcs_status",
        vec![
            json_of(&WorkingTree { branch: Some("main".into()), detached: None, changes: changes() }),
            json_of(&WorkingTree::default()),
        ],
    );
    map.insert(
        "vcs_in_progress",
        vec![
            json_of(&Some(InProgress { op: OpKind::Merge, ours: Some("main".into()), theirs: Some("topic".into()) })),
            json_of(&Some(InProgress { op: OpKind::Rebase, ours: None, theirs: None })),
            Value::Null,
        ],
    );
    map.insert(
        "vcs_branches",
        vec![json_of(&vec![
            Branch { name: "main".into(), current: true },
            Branch { name: "topic".into(), current: false },
        ])],
    );
    map.insert(
        "vcs_remote_branches",
        vec![json_of(&vec![
            smetana_core::git::RemoteBranch { name: "origin/main".into(), touched_at: Some(1_760_000_000) },
            smetana_core::git::RemoteBranch { name: "origin/old".into(), touched_at: None },
        ])],
    );
    map.insert("vcs_last_fetch", vec![json!(1_760_000_000i64), Value::Null]);
    map.insert(
        "vcs_tracking",
        vec![json_of(&vec![
            Tracking { branch: "main".into(), upstream: Some("origin/main".into()), ahead: 1, behind: 2, gone: false },
            Tracking { branch: "topic".into(), upstream: None, ahead: 0, behind: 0, gone: true },
        ])],
    );
    for name in [
        "vcs_fetch",
        "vcs_checkout",
        "vcs_checkout_remote",
        "vcs_create_branch",
        "vcs_rename_branch",
        "vcs_delete_branch",
        "vcs_abort",
        "vcs_commit",
        "vcs_discard",
    ] {
        map.insert(name, nothing());
    }
    for name in ["vcs_pull", "vcs_merge", "vcs_rebase"] {
        map.insert(name, vec![json_of(&clean), json_of(&conflict)]);
    }
    map.insert("vcs_push", vec![json_of(&landed(true)), json_of(&landed(false))]);
    map.insert("vcs_suggest_message", vec![json!("feat: a thing")]);
    map.insert("vcs_file_at_head", vec![json!("export {}\n"), Value::Null]);
    map.insert("vcs_file_at_rev", vec![json!("export {}\n"), Value::Null]);
    map.insert(
        "vcs_compare",
        vec![json_of(&Comparison {
            left: "main".into(),
            right: "topic".into(),
            files: vec![
                CompareChange { path: "src/a.js".into(), orig_path: None, kind: ChangeKind::Modified },
                CompareChange { path: "src/b.js".into(), orig_path: Some("src/c.js".into()), kind: ChangeKind::Renamed },
            ],
        })],
    );

    map.insert("project_config_save_defaults", nothing());
    map.insert(
        "browser_tools",
        vec![
            json_of(&BrowserTools {
                playwright_mcp: true,
                playwright_browsers: true,
                extension: false,
                busy_project: Some("/work/other".into()),
            }),
            json_of(&BrowserTools::default()),
        ],
    );
    map.insert("run_release", nothing());
    map.insert("agent_usage", all(&agent_usages()));
    map.insert("codex_models", vec![json_of(&vec![AgentModel { id: "gpt-5".into(), label: "GPT-5".into() }])]);
    map.insert(
        "sessions_list",
        vec![json_of(&vec![SessionSummary {
            id: "abc".into(),
            path: "/home/you/.claude/projects/-work-smetana/abc.jsonl".into(),
            cwd: "/work/smetana".into(),
            branch: Some("main".into()),
            title: Some("Fix the board".into()),
            first_prompt: Some("Fix it".into()),
            last_role: Some("assistant".into()),
            last_text: Some("Done".into()),
            messages: 12,
            subagents: 1,
            model: Some("opus".into()),
            modified_at: "2026-10-15T10:00:00Z".into(),
            cwd_exists: true,
            size: 2048,
        }])],
    );
    map.insert("sessions_delete", nothing());
    map.insert("settings_save", nothing());

    map.insert("terminal_marks", vec![json_of(&session_marks()), json!([])]);
    let sessions = terminal_sessions();
    map.insert("terminal_create", vec![json_of(&sessions[1]), json_of(&sessions[0])]);
    map.insert("terminal_shell", vec![json_of(&sessions[15])]);
    for name in ["terminal_remove", "terminal_detach", "terminal_resize", "terminal_write", "terminal_clear", "terminal_forget"] {
        map.insert(name, nothing());
    }
    map.insert("terminal_run_capture", vec![json!(["$ ls", "a.txt"]), json!([])]);
    map.insert("terminal_restorable", vec![json_of(&restorable()), json!([])]);
    map.insert("crew_clear", nothing());
    map.insert("session_close", nothing());

    map.insert(
        "remote_pair_begin",
        vec![json_of(&PairBegin { ticket: "smetana2:abcd".into(), expires_at: 1_760_000_120 })],
    );
    map.insert("remote_pair_cancel", nothing());
    map.insert("remote_pair_decide", nothing());
    map.insert("remote_devices", vec![json_of(&device_views(&devices())), json!([])]);
    map.insert("remote_device_revoke", nothing());

    map.insert("server_health", vec![json_of(&health_ok()), json_of(&health_with_problems())]);
    map.insert("server_restart", restart_replies());
}

fn tool(name: &str, version: Option<&str>) -> smetana_core::health::Tool {
    smetana_core::health::Tool {
        name: name.into(),
        found: version.is_some(),
        path: version.map(|_| format!("/usr/local/bin/{name}")),
        answers: version.is_some(),
        version: version.map(str::to_owned),
        problem: if version.is_some() { None } else { Some(format!("`{name}` was not found")) },
    }
}

fn health_ok() -> smetana_core::health::Report {
    use smetana_core::health::{AgentCheck, Report};
    Report {
        ok: true,
        problems: Vec::new(),
        bd: tool("bd", Some("bd version 1.1.2")),
        bd_expected: "1.1.2".into(),
        git: tool("git", Some("git version 2.50.1")),
        agents: vec![
            AgentCheck { id: "claude".into(), label: "Claude Code".into(), binary: tool("claude", Some("2.1.0 (Claude Code)")) },
            AgentCheck { id: "codex".into(), label: "Codex".into(), binary: tool("codex", Some("codex-cli 0.80.0")) },
        ],
        path: Some("/usr/local/bin:/usr/bin".into()),
    }
}

fn health_with_problems() -> smetana_core::health::Report {
    let mut report = health_ok();
    report.ok = false;
    report.problems = vec!["`codex` was not found".into()];
    report.agents[1].binary = tool("codex", None);
    report.path = None;
    report
}

fn restart_work() -> Vec<crate::gateway::LiveWork> {
    use crate::gateway::{LiveWork, WorkKind};
    vec![
        LiveWork { kind: WorkKind::Run, id: Some(1), project: "/work/smetana".into(), label: "run".into(), owner: None },
        LiveWork { kind: WorkKind::Agent, id: Some(4), project: "/work/smetana".into(), label: "claude".into(), owner: None },
        LiveWork { kind: WorkKind::Session, id: Some(2), project: "/work/smetana".into(), label: "codex".into(), owner: Some(1) },
        LiveWork { kind: WorkKind::Shell, id: Some(7), project: "/work/smetana".into(), label: "shell".into(), owner: None },
    ]
}

fn restart_replies() -> Vec<Value> {
    use crate::gateway::{LiveWork, Outcome, Reply};
    let work = restart_work();
    let reply = |outcome, scheduled, work: &[LiveWork]| json_of(&Reply { outcome, scheduled, work: work.to_vec() });
    vec![
        reply(Outcome::NeedsConfirmation, false, &work),
        reply(Outcome::Scheduled, true, &work[..1]),
        reply(Outcome::Restarting, false, &work),
        reply(Outcome::Cancelled, false, &[]),
        reply(Outcome::NothingScheduled, false, &[]),
    ]
}

/// Every event beyond the phone's.
fn server_payloads(map: &mut BTreeMap<&'static str, Vec<Value>>) {
    use crate::bridge::output_payload;
    map.insert(
        "terminal:output",
        vec![
            // One chunk, and a run of three coalesced (`from..=seq`).
            output_payload(3, 8, 8, "aGk=", false),
            output_payload(3, 9, 11, "aGk=", false),
            // The ring afresh, for a device that fell behind: the data replaces the terminal.
            output_payload(3, 12, 12, "aGk=", true),
        ],
    );
    map.insert("server:restart-deferred", vec![crate::gateway::deferred_payload(&restart_work())]);
    map.insert(
        "terminal:log",
        vec![smetana_core::terminal::service::log_payload(3, &json_of(&log_entries()))],
    );
    let states = [
        DrivenState::Starting,
        DrivenState::Running,
        DrivenState::Ready,
        DrivenState::NeedsYou,
        DrivenState::Exited,
        DrivenState::Failed,
    ];
    map.insert(
        "session:state",
        states
            .into_iter()
            .enumerate()
            .map(|(i, state)| {
                json_of(&StateChange {
                    id: i as u64 + 1,
                    state,
                    conversation: (i == 0).then(|| "conv-1".to_string()),
                    title: (i == 0).then(|| "Fix the board".to_string()),
                })
            })
            .collect(),
    );
    map.insert(
        "crew:tree",
        vec![
            json_of(&CrewTreeChange { project: "/work/smetana".into(), root: 1, nodes: crew_nodes() }),
            json_of(&CrewTreeChange { project: "/work/smetana".into(), root: 2, nodes: Vec::new() }),
        ],
    );
    map.insert(
        "remote:project-switched",
        vec![
            super::dispatch::project_switched_payload(Some("/work/smetana".into())),
            super::dispatch::project_switched_payload(None),
        ],
    );
    map.insert(
        "remote:pair-request",
        vec![json_of(&PairRequestPayload { device_name: "Pixel 8".into(), platform: Platform::Android })],
    );
    map.insert("remote:devices", vec![json_of(&device_views(&devices())), json!([])]);
    map.insert("remote:pair-ended", vec![Value::Null]);
}

// ---------------------------------------------------------------------------
// What the contract says each command answers and each event carries.
// ---------------------------------------------------------------------------

/// Every allowlisted command's reply examples, by name.
fn replies() -> BTreeMap<&'static str, Vec<Value>> {
    let mut map: BTreeMap<&'static str, Vec<Value>> = BTreeMap::new();
    map.insert("tracker_health", all(&healths()));
    map.insert("tracker_snapshot", all(&snapshots()));
    map.insert("tracker_set_project", all(&snapshots()));
    map.insert("settings_load", vec![json_of(&ResolvedSettings::default())]);
    map.insert("agents_catalog", vec![json_of(&agent_rows())]);
    map.insert("agents_installed", vec![json!(["claude", "codex"])]);
    map.insert("project_config", all(&config_replies()));
    map.insert("target_branches", vec![json_of(&branch_options())]);
    map.insert("run_state", vec![json_of(&runs_basic()), json!([])]);
    map.insert("run_reports", vec![json_of(&report_entries()), json!([])]);
    map.insert("run_start", all(&runs_basic()));
    map.insert("run_stop", vec![json_of(&runs_basic()[0]), Value::Null]);
    map.insert("session_start", vec![json!(7)]);
    map.insert("session_attach", all(&driven_attached()));
    map.insert("session_since", vec![json_of(&conversation()), Value::Null]);
    map.insert(
        "attachment_write",
        vec![
            json_of(&Attachment {
                path: "/data/attachments/proj-abc/20261017-x.png".into(),
                name: "20261017-x.png".into(),
                bytes: 2,
                mime: "image/png".into(),
                data: Some("aGk=".into()),
            }),
            // A file that is not a picture comes back with no bytes.
            json_of(&Attachment {
                path: "/data/attachments/proj-abc/20261021-brief.pdf".into(),
                name: "20261021-brief.pdf".into(),
                bytes: 2,
                mime: "application/octet-stream".into(),
                data: None,
            }),
        ],
    );
    map.insert("session_send", vec![Value::Null]);
    map.insert("session_answer", vec![Value::Null]);
    map.insert("session_stop", vec![Value::Null]);
    map.insert("crew_tree", vec![json_of(&crew_nodes()), Value::Null]);
    map.insert("crew_attach", all(&crew_attached()));
    map.insert("crew_send", vec![Value::Null]);
    map.insert("crew_stop", vec![Value::Null]);
    map.insert("terminal_list", vec![json_of(&terminal_sessions()), json!([])]);
    server_replies(&mut map);
    map.insert("terminal_attach", vec![json_of(&TerminalAttached { data: "aGk=".into(), seq: 12, log: log_entries(), log_trimmed: 3, resumed_after: None }),
        // A reply to a call that named the number it holds: the bytes that followed it, which the
        // caller writes onto the terminal it already has (protocol 1.8). Absent in the first
        // example, so the shape a phone reads is the one it always was.
        json_of(&TerminalAttached { data: "aGk=".into(), seq: 14, log: Vec::new(), log_trimmed: 0, resumed_after: Some(12) })]);
    map
}

/// Every allowlisted event's payload examples, by name.
fn payloads() -> BTreeMap<&'static str, Vec<Value>> {
    let mut map: BTreeMap<&'static str, Vec<Value>> = BTreeMap::new();
    map.insert("tracker:delta", all(&deltas()));
    map.insert("tracker:health", all(&healths()));
    map.insert("run:state", all(&runs_sweep()));
    map.insert("session:events", vec![events_payload(7, &conversation()), events_payload(8, &[])]);
    map.insert(
        "crew:events",
        vec![json_of(&CrewEventsChange { root: 1, node: 2, events: conversation() })],
    );
    map.insert("terminal:state", all(&terminal_sessions()));
    map.insert("terminal:removed", vec![removed_payload(3)]);
    server_payloads(&mut map);
    map
}

// ---------------------------------------------------------------------------
// The files.
// ---------------------------------------------------------------------------

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../remote-protocol/contract")
}

fn updating() -> bool {
    std::env::var_os("SMETANA_UPDATE_CONTRACT").is_some()
}

/// One example per line, so that a change to one reads as a change to one line.
fn render(key: &str, examples: &[Value], before: &[(&str, &Value)]) -> String {
    let mut out = String::from("{\n");
    for (name, value) in before {
        out.push_str(&format!("  \"{name}\": {},\n", serde_json::to_string(value).unwrap()));
    }
    out.push_str(&format!("  \"{key}\": [\n"));
    let lines: Vec<String> = examples
        .iter()
        .map(|example| format!("    {}", serde_json::to_string(example).unwrap()))
        .collect();
    out.push_str(&lines.join(",\n"));
    out.push_str("\n  ]\n}\n");
    out
}

fn read(path: &str) -> Value {
    let text = std::fs::read_to_string(root().join(path))
        .unwrap_or_else(|err| panic!("contract file {path} is missing ({err}); the allowlist and the contract must agree"));
    serde_json::from_str(&text).unwrap_or_else(|err| panic!("contract file {path} is not JSON: {err}"))
}

/// Where two values differ, as sentences that say what kind of change it is.
fn diff(path: &str, pinned: &Value, actual: &Value, out: &mut Vec<String>) {
    match (pinned, actual) {
        (Value::Object(p), Value::Object(a)) => {
            for key in p.keys().filter(|k| !a.contains_key(*k)) {
                out.push(format!(
                    "field `{path}.{key}` is pinned but no longer produced (removed or renamed: raise PROTOCOL_MAJOR)"
                ));
            }
            for key in a.keys().filter(|k| !p.contains_key(*k)) {
                out.push(format!(
                    "field `{path}.{key}` is produced but not pinned (added: pin it in the contract and raise PROTOCOL_MINOR)"
                ));
            }
            for (key, value) in p {
                if let Some(other) = a.get(key) {
                    diff(&format!("{path}.{key}"), value, other, out);
                }
            }
        }
        (Value::Array(p), Value::Array(a)) => {
            if p.len() != a.len() {
                out.push(format!(
                    "`{path}` holds {} examples in the contract and the desktop produced {}",
                    p.len(),
                    a.len()
                ));
            }
            for (i, (pinned, actual)) in p.iter().zip(a).enumerate() {
                diff(&format!("{path}[{i}]"), pinned, actual, out);
            }
        }
        (p, a) if p == a => {}
        (p, a) => out.push(format!(
            "`{path}` was {p} in the contract and is now {a} (a changed value or type: raise PROTOCOL_MAJOR)"
        )),
    }
}

/// `$[3].state[0]` as `$[].state[]`.
fn without_indexes(path: &str) -> String {
    let mut out = String::new();
    let mut skipping = false;
    for c in path.chars() {
        match c {
            '[' => {
                out.push('[');
                skipping = true;
            }
            ']' => {
                out.push(']');
                skipping = false;
            }
            _ if skipping => {}
            _ => out.push(c),
        }
    }
    out
}

/// What differs between a pinned file and the desktop's examples, as one report.
fn check(kind: &str, name: &str, file: &str, key: &str, actual: &[Value]) -> Option<String> {
    let pinned_file = read(file);
    let pinned = pinned_file[key].clone();
    let mut problems = Vec::new();
    diff("$", &pinned, &Value::Array(actual.to_vec()), &mut problems);
    // The same field wrong in twenty examples is one problem.
    let mut seen = std::collections::HashSet::new();
    problems.retain(|problem| seen.insert(without_indexes(problem)));
    (!problems.is_empty()).then(|| {
        format!(
            "the {key} of {kind} `{name}` no longer matches contract/{file}:\n  {}",
            problems.join("\n  ")
        )
    })
}

fn fail_on(reports: Vec<String>) {
    assert!(
        reports.is_empty(),
        "{}\n\nAdditions are free but need the contract file updated and PROTOCOL_MINOR raised; removals \
         and renames need PROTOCOL_MAJOR. `SMETANA_UPDATE_CONTRACT=1 cargo test remote_contract` rewrites \
         the files from the desktop's values.",
        reports.join("\n")
    );
}

#[test]
fn remote_contract_replies_of_every_allowed_command_match_the_pinned_files() {
    let replies = replies();
    assert_eq!(replies.keys().copied().collect::<Vec<_>>(), {
        let mut names = SERVER_COMMANDS.to_vec();
        names.sort_unstable();
        names
    }, "a reply example is needed for exactly the allowlisted commands");
    let mut reports = Vec::new();
    for (name, examples) in &replies {
        let file = command_contract_path(name);
        if updating() {
            // Arguments are the files' own: keep what is there.
            let args = read_or_default(&file, "args");
            let text = render("reply", examples, &[("args", &args)]);
            std::fs::write(root().join(&file), text).unwrap();
            continue;
        }
        reports.extend(check("command", name, &file, "reply", examples));
    }
    fail_on(reports);
}

#[test]
fn remote_contract_payloads_of_every_allowed_event_match_the_pinned_files() {
    let payloads = payloads();
    assert_eq!(payloads.keys().copied().collect::<Vec<_>>(), {
        let mut names = SERVER_EVENTS.to_vec();
        names.sort_unstable();
        names
    }, "a payload example is needed for exactly the allowlisted events");
    let mut reports = Vec::new();
    for (name, examples) in &payloads {
        let file = event_contract_path(name);
        if updating() {
            std::fs::write(root().join(&file), render("payload", examples, &[])).unwrap();
            continue;
        }
        reports.extend(check("event", name, &file, "payload", examples));
    }
    fail_on(reports);
}

fn read_or_default(file: &str, key: &str) -> Value {
    std::fs::read_to_string(root().join(file))
        .ok()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        .map(|value| value[key].clone())
        .filter(|value| !value.is_null())
        .unwrap_or_else(|| json!([{}]))
}

/// FNV-1a over every contract file, in path order, each as its canonical JSON
/// (so line endings and layout do not matter). Not a security hash: it is a
/// fingerprint, and a changed byte of any file changes it.
fn contract_digest() -> String {
    let mut paths: Vec<String> = SERVER_COMMANDS.iter().map(|c| command_contract_path(c)).collect();
    paths.extend(SERVER_EVENTS.iter().map(|e| event_contract_path(e)));
    paths.sort();
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    let mut feed = |bytes: &[u8]| {
        for byte in bytes {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x0100_0000_01b3);
        }
    };
    for path in &paths {
        feed(path.as_bytes());
        feed(&[0]);
        feed(read(path).to_string().as_bytes());
        feed(&[0]);
    }
    format!("{hash:016x}")
}

fn version_key(major: u16, minor: u16) -> String {
    format!("{major}.{minor}")
}

fn parse_version(key: &str) -> (u16, u16) {
    let (major, minor) = key.split_once('.').unwrap_or_else(|| panic!("versions.json key {key:?} is not MAJOR.MINOR"));
    (major.parse().expect("a numeric major"), minor.parse().expect("a numeric minor"))
}

/// `versions.json` records the digest of the contract at every protocol version,
/// and **nothing writes it but a person**: `SMETANA_UPDATE_CONTRACT` rewrites the
/// examples and never this file. So a regenerated contract cannot pass by
/// itself. The files of a version that has an entry are frozen, and a different
/// digest under the same version is a change that needed a bump; the only way
/// forward is a new entry under a higher `PROTOCOL_MAJOR`/`PROTOCOL_MINOR`, which
/// is a decision in the diff.
#[test]
fn remote_contract_changes_only_with_a_protocol_version() {
    let versions = read("versions.json");
    let entries = versions.as_object().expect("versions.json is an object of MAJOR.MINOR to digest");
    let current = version_key(PROTOCOL_MAJOR, PROTOCOL_MINOR);
    let digest = contract_digest();

    let newest = entries.keys().map(|key| parse_version(key)).max().expect("versions.json is empty");
    assert!(
        newest <= (PROTOCOL_MAJOR, PROTOCOL_MINOR),
        "versions.json records {}.{}, newer than PROTOCOL_MAJOR/PROTOCOL_MINOR ({current}): the constants were lowered",
        newest.0,
        newest.1
    );

    match entries.get(&current).and_then(Value::as_str) {
        Some(pinned) if pinned == digest => {}
        Some(pinned) => panic!(
            "the contract changed but the protocol version did not: version {current} is pinned to digest {pinned} \
             and the files now hash to {digest}.\n\
             - if you only ADDED (a field, a command, an event): raise PROTOCOL_MINOR in remote-server/src/lib.rs to {}.{} \
             and add \"{}.{}\": \"{digest}\" to contract/versions.json;\n\
             - if you REMOVED or RENAMED anything: raise PROTOCOL_MAJOR to {} and add \"{}.0\": \"{digest}\".\n\
             Do not edit the digest of an existing version: phones built against it hold that contract.",
            PROTOCOL_MAJOR,
            PROTOCOL_MINOR + 1,
            PROTOCOL_MAJOR,
            PROTOCOL_MINOR + 1,
            PROTOCOL_MAJOR + 1,
            PROTOCOL_MAJOR + 1,
        ),
        None => panic!(
            "PROTOCOL_MAJOR/PROTOCOL_MINOR is {current} and versions.json has no entry for it: \
             add \"{current}\": \"{digest}\""
        ),
    }
}

/// The argument examples are the contract's input side: each must decode the way
/// the bridge decodes a call, with no key left over.
#[test]
fn remote_contract_argument_examples_of_every_allowed_command_decode() {
    for cmd in SERVER_COMMANDS {
        let file = command_contract_path(cmd);
        let pinned = read(&file);
        let examples = pinned["args"].as_array().unwrap_or_else(|| panic!("{file} has no `args` list"));
        assert!(!examples.is_empty(), "{cmd} has no argument example");
        for example in examples {
            // `session_send` with attachments is a refusal, not a decode failure,
            // and is not an example of what a phone sends.
            if let Err(err) = Call::parse(cmd, example.clone()) {
                panic!("the argument example of `{cmd}` no longer decodes: {err}\n  example: {example}");
            }
        }
    }
}

/// An argument that is a whole type of its own is held to a round trip as well,
/// because serde ignores a key such a type does not have.
#[test]
fn remote_contract_run_settings_example_round_trips_without_losing_a_key() {
    let pinned = read(&command_contract_path("run_start"));
    for example in pinned["args"].as_array().unwrap() {
        let settings = example["settings"].clone();
        let parsed: RunSettings = serde_json::from_value(settings.clone()).unwrap();
        let mut problems = Vec::new();
        diff("$.settings", &settings, &json_of(&parsed), &mut problems);
        assert!(problems.is_empty(), "run_start `settings` argument: {}", problems.join("; "));
    }
}

#[test]
fn remote_contract_diff_names_the_field_that_changed() {
    let pinned = json!({ "token": 1, "state": { "kind": "working", "iteration": 2 } });
    let mut problems = Vec::new();
    diff("$", &pinned, &json!({ "token": 1, "state": { "kind": "working", "round": 2 } }), &mut problems);
    assert_eq!(problems.len(), 2);
    assert!(problems.iter().any(|p| p.contains("$.state.iteration") && p.contains("PROTOCOL_MAJOR")));
    assert!(problems.iter().any(|p| p.contains("$.state.round") && p.contains("PROTOCOL_MINOR")));
}

// ---------------------------------------------------------------------------
// The server's surface is the front end's surface.
// ---------------------------------------------------------------------------

/// Every command the desktop front end can call is a server command with a pinned example, or is
/// declared the client's own. A command added to `lib.rs` without either fails here, which is the
/// point: whether a new command belongs to the server is a decision, and the contract is where it
/// is written down.
#[test]
fn remote_contract_every_front_end_command_is_pinned_or_the_clients() {
    use super::bridge::{CLIENT_COMMANDS, DESKTOP_COMMANDS};
    for cmd in DESKTOP_COMMANDS {
        let server = SERVER_COMMANDS.contains(cmd);
        let client = CLIENT_COMMANDS.contains(cmd);
        assert!(
            server != client,
            "`{cmd}` must be exactly one of a server command (SERVER_COMMANDS) and a client one (CLIENT_COMMANDS)"
        );
        if server {
            let file = command_contract_path(cmd);
            let pinned = read(&file);
            for key in ["args", "reply"] {
                let examples = pinned[key].as_array().unwrap_or_else(|| panic!("{file} has no `{key}` list"));
                assert!(!examples.is_empty(), "server command `{cmd}` has no {key} example in contract/{file}");
            }
        }
    }
    for cmd in SERVER_COMMANDS {
        assert!(
            DESKTOP_COMMANDS.contains(cmd) || remote_protocol::SERVER_ONLY_COMMANDS.contains(cmd),
            "`{cmd}` is a server command the desktop does not have and that is not declared server-only"
        );
    }
    for cmd in remote_protocol::SERVER_ONLY_COMMANDS {
        assert!(SERVER_COMMANDS.contains(cmd), "`{cmd}` is declared server-only and is not a server command");
        assert!(!DESKTOP_COMMANDS.contains(cmd), "`{cmd}` is declared server-only and the desktop has it");
    }
    for cmd in CLIENT_COMMANDS {
        assert!(DESKTOP_COMMANDS.contains(cmd), "`{cmd}` is declared the client's and the desktop has no such command");
    }
}

/// Every file under `dir` with the extension `ext`, as text, except the mock backend: it answers
/// for a browser and names events of the windows' own.
fn sources(dir: &std::path::Path, ext: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(path) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&path) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == ext)
                && path.file_name().is_some_and(|name| name != "mockBackend.js")
            {
                out.push(std::fs::read_to_string(&path).unwrap_or_default());
            }
        }
    }
    out
}

/// An event name: lower-case words around one colon.
fn is_event_name(text: &str) -> bool {
    text.split_once(':').is_some_and(|(namespace, name)| {
        !namespace.is_empty()
            && !name.is_empty()
            && namespace.bytes().all(|b| b.is_ascii_lowercase())
            && name.bytes().all(|b| b.is_ascii_lowercase() || b == b'-')
    })
}

/// The literals quoted with `quote` that follow `call` (and any whitespace) in `text`, when they
/// are event names.
fn event_names_after(text: &str, call: &str, quote: char) -> Vec<String> {
    let mut out = Vec::new();
    for (at, _) in text.match_indices(call) {
        let rest = text[at + call.len()..].trim_start();
        let Some(rest) = rest.strip_prefix(quote) else { continue };
        if let Some(end) = rest.find(quote) {
            if is_event_name(&rest[..end]) {
                out.push(rest[..end].to_owned());
            }
        }
    }
    out
}

/// Every event the front end listens to by name, and every event a worker emits, is pinned as a
/// server event. The scans read source text, which is crude and is the point: a new
/// `listen('x:y')` or `ctx.emit("x:y", ...)` that nobody added to the contract fails here.
#[test]
fn remote_contract_every_front_end_and_worker_event_is_pinned() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let stores = sources(&manifest.join("../../src/stores"), "js");
    let workers = sources(&manifest.join("../core/src"), "rs");
    let listened: Vec<String> = stores.iter().flat_map(|text| event_names_after(text, "listen(", '\'')).collect();
    // The companion's own events are named by constants in `stores/remote.js`.
    let devices: Vec<String> = stores
        .iter()
        .flat_map(|text| event_names_after(text, "_EVENT =", '\''))
        .filter(|name| name.starts_with("remote:"))
        .collect();
    let emitted: Vec<String> = workers
        .iter()
        .flat_map(|text| {
            let mut names = event_names_after(text, "emit(", '"');
            names.extend(event_names_after(text, "emit_json(", '"'));
            names
        })
        .collect();
    assert!(listened.len() >= 10, "the scan of the stores found only {listened:?}");
    assert!(devices.len() >= 3, "the scan of remote.js found only {devices:?}");
    assert!(emitted.len() >= 10, "the scan of the workers found only {emitted:?}");
    for (what, names) in [("listens to", listened), ("remote.js names", devices), ("a worker emits", emitted)] {
        for name in names {
            assert!(
                SERVER_EVENTS.contains(&name.as_str()),
                "the front end {what} `{name}`, which is not in SERVER_EVENTS: add it with its contract/events file"
            );
            let file = event_contract_path(&name);
            let pinned = read(&file);
            assert!(
                pinned["payload"].as_array().is_some_and(|examples| !examples.is_empty()),
                "event `{name}` has no payload example in contract/{file}"
            );
        }
    }
    for name in SERVER_EVENTS {
        assert!(is_event_name(name), "{name}");
    }
}

#[test]
fn remote_contract_event_name_scan_reads_what_it_should() {
    let text = "await listen('run:state', (e) => 1)\nlisten(\n  'crew:tree', x)\nlisten(CONST, y)\nlisten(`dialog:hello:${k}`, z)";
    assert_eq!(event_names_after(text, "listen(", '\''), ["run:state", "crew:tree"]);
    let rust = "ctx.emit(\n    \"crew:events\", &x);\nctx.emit_json(\"terminal:log\", y);\nctx.emit(name, z);";
    assert_eq!(event_names_after(rust, "emit(", '"'), ["crew:events"]);
    assert_eq!(event_names_after(rust, "emit_json(", '"'), ["terminal:log"]);
}
