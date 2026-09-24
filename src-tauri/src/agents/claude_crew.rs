//! Claude Code Agent Teams' structured runtime surface.
//!
//! The interactive team runtime is intentionally not decoded from its TUI.
//! Discovery comes from `~/.claude/teams/<team>/config.json`; per-agent output
//! comes from the documented subagent JSONL transcript paths. The mailbox
//! entry shape below is pinned by a captured interactive `SendMessage` run;
//! a transport still has to take the runtime's inter-process mailbox lock
//! before it performs the read/modify/write.

use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::collections::HashSet;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use serde_json::Value;

use super::crew::{ProviderNode, ProviderState};
use super::{claude::Claude, Launch};

pub const TEAM_ENV: &str = "CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS";
pub const TEAM_FLAG: &str = "--teammate-mode";
pub const TEAM_MODE: &str = "in-process";
pub const MIN_VERSION: (u32, u32, u32) = (2, 1, 281);

/// The only supported Claude Crew lead line. It is deliberately separate from
/// `ClaudeDriver`, whose `-p --input-format stream-json` protocol cannot make
/// native teammates. The team runtime must remain interactive while Smetana
/// observes its documented config/transcript/mailbox files.
/// Build the interactive runtime without a positional brief. Crew admission is
/// deliberately proved from the config, transcript and inboxes first; only
/// then does the session worker write this returned brief to the live PTY.
/// That keeps an unsupported runtime from receiving a Run prompt (and claiming
/// work) before Smetana can own its structured transport.
pub fn interactive_lead_command(launch: &Launch) -> (portable_pty::CommandBuilder, Option<String>) {
    let claude = Claude;
    let mut command = claude.command_without_prompt(launch);
    command.env(TEAM_ENV, "1");
    command.arg(TEAM_FLAG);
    command.arg(TEAM_MODE);
    (command, claude.prompt_text(launch))
}

pub fn supports_version(found: &str) -> bool {
    version_at_least(found, MIN_VERSION)
}

/// A message as Claude's interactive team runtime stores it in a teammate's
/// inbox. `recipient` does not travel in the file: the inbox filename is the
/// address. Keeping that distinction here prevents a caller from treating a
/// provider id as a Smetana node id.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MailboxMessage {
    pub from: String,
    pub text: String,
    pub summary: String,
    pub timestamp: String,
    #[serde(rename = "msgV")]
    pub version: u8,
    #[serde(rename = "msg_id")]
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub read: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum MailboxError {
    #[error("the Claude teammate inbox could not be read: {0}")]
    Read(String),
    #[error("the Claude teammate inbox is not a message array")]
    Corrupt,
    #[error("the Claude teammate inbox changed while the message was being addressed")]
    Changed,
    #[error("the Claude teammate inbox is busy")]
    Busy,
    #[error("the selected Claude teammate has already finished")]
    Unavailable,
    #[error("the Claude teammate inbox could not be written: {0}")]
    Write(String),
    #[error("the system could not create an addressed Claude message id")]
    NoMessageId,
}

/// The version/runtime facts a run checks before it can let a Crew lead claim
/// a task. A missing fact is a refusal, never a reason to fall back to the
/// provider TUI.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum PreflightError {
    #[error(
        "Claude Code {found} does not support native agent discovery; require {required} or newer"
    )]
    Version { found: String, required: String },
    #[error("Claude Code native agent discovery is unavailable: {0}")]
    Discovery(String),
    #[error("Claude Code separate agent journals are unavailable: {0}")]
    Journal(String),
    #[error("Claude Code addressed agent messages are unavailable: {0}")]
    AddressedMessages(String),
}

/// Verify the static part before a run leaves its queue and the dynamic part
/// immediately after the interactive lead establishes its own runtime files.
/// `team_dir` and `lead_session_dir` are provider-owned locations learned from
/// that runtime, never derived from the CLI session id.
pub fn preflight(
    version: &str,
    team_dir: &Path,
    lead_session_dir: &Path,
) -> Result<(), PreflightError> {
    if !version_at_least(version, MIN_VERSION) {
        return Err(PreflightError::Version {
            found: version.into(),
            required: format!("{}.{}.{}", MIN_VERSION.0, MIN_VERSION.1, MIN_VERSION.2),
        });
    }
    let config = team_dir.join("config.json");
    let config_bytes =
        fs::read(&config).map_err(|error| PreflightError::Discovery(error.to_string()))?;
    let config: Value = serde_json::from_slice(&config_bytes)
        .map_err(|_| PreflightError::Discovery("config.json is malformed".into()))?;
    if team_name(&config).is_none() || config.get("members").and_then(Value::as_array).is_none() {
        return Err(PreflightError::Discovery(
            "config.json has no team name or members".into(),
        ));
    }
    let subagents = lead_session_dir.join("subagents");
    if !subagents.is_dir() {
        return Err(PreflightError::Journal(format!(
            "{} is missing",
            subagents.display()
        )));
    }
    let inboxes = team_dir.join("inboxes");
    if !inboxes.is_dir() {
        return Err(PreflightError::AddressedMessages(format!(
            "{} is missing",
            inboxes.display()
        )));
    }
    // A configured member's actual inbox is the only writable contract. Do
    // not pre-create it: that would masquerade as a provider runtime surface.
    for member in config
        .get("members")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        // The lead is addressed through its own interactive text input. Only
        // teammates have provider-owned inbox files, so demanding a fictitious
        // `team-lead.json` would reject a healthy native runtime.
        if member.get("agentType").and_then(Value::as_str) == Some("team-lead") {
            continue;
        }
        if matches!(
            member.get("status").and_then(Value::as_str),
            Some("left" | "completed" | "failed")
        ) {
            continue;
        }
        let Some(name) = member.get("name").and_then(Value::as_str) else {
            continue;
        };
        let path = inbox(&team_dir, &name);
        OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .map_err(|error| {
                PreflightError::AddressedMessages(format!("{}: {error}", path.display()))
            })?;
    }
    Ok(())
}

fn version_at_least(found: &str, minimum: (u32, u32, u32)) -> bool {
    let mut numbers = found
        .split(|ch: char| !ch.is_ascii_digit())
        .filter(|part| !part.is_empty())
        .filter_map(|part| part.parse::<u32>().ok());
    let parsed = match (numbers.next(), numbers.next(), numbers.next()) {
        (Some(major), Some(minor), Some(patch)) => (major, minor, patch),
        _ => return false,
    };
    parsed >= minimum
}

/// The runtime owns team names. They are not derivable from the CLI session
/// id: Agent Teams creates `session-<team-id>` independently, so discovery
/// must use the actual `name` in its config or an Agent tool result.
pub fn team_name(config: &Value) -> Option<String> {
    config
        .get("name")
        .and_then(Value::as_str)
        .map(str::to_owned)
}

pub fn team_config(home: &Path, team: &str) -> PathBuf {
    home.join(".claude")
        .join("teams")
        .join(team)
        .join("config.json")
}

/// Locate a just-created team through the runtime's own config rather than by
/// deriving a name from a CLI session id. Multiple candidates are deliberately
/// returned: the session worker refuses to guess when two interactive leads
/// have the same project cwd.
pub fn teams_for_project(home: &Path, project: &Path) -> Vec<(PathBuf, Value)> {
    let root = home.join(".claude").join("teams");
    let Ok(entries) = fs::read_dir(root) else { return Vec::new() };
    entries
        .flatten()
        .filter_map(|entry| {
            let team_dir = entry.path();
            let config = fs::read(team_dir.join("config.json")).ok()?;
            let value: Value = serde_json::from_slice(&config).ok()?;
            let lead_here = value
                .get("members")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .any(|member| {
                    member.get("agentType").and_then(Value::as_str) == Some("team-lead")
                        && member.get("cwd").and_then(Value::as_str)
                            .is_some_and(|cwd| Path::new(cwd) == project)
                });
            lead_here.then_some((team_dir, value))
        })
        .collect()
}

/// Snapshot provider-owned team directories before spawning an interactive
/// lead. A later discovery is only eligible when its directory was created by
/// this launch window, which prevents an old same-cwd team from being adopted.
pub fn team_dirs(home: &Path) -> HashSet<PathBuf> {
    fs::read_dir(home.join(".claude").join("teams"))
        .map(|entries| entries.flatten().map(|entry| entry.path()).collect())
        .unwrap_or_default()
}

/// Narrow candidates to the exact lead Claude was told to create. The config's
/// `leadSessionId` is the runtime's structured binding between our
/// `--session-id` and its generated team name; cwd alone cannot distinguish
/// two concurrent Crew packages.
pub fn teams_for_lead(
    candidates: Vec<(PathBuf, Value)>,
    expected_session: &str,
    baseline: &HashSet<PathBuf>,
) -> Vec<(PathBuf, Value)> {
    candidates
        .into_iter()
        .filter(|(team, config)| {
            !baseline.contains(team)
                && config.get("leadSessionId").and_then(Value::as_str) == Some(expected_session)
        })
        .collect()
}

/// The child path is relative to the lead session directory. This deliberately
/// does not reconstruct Claude's encoded-project folder name: the existing
/// session reader already owns that provider-specific path mapping.
pub fn transcript(lead_session_dir: &Path, internal_agent_id: &str) -> PathBuf {
    lead_session_dir
        .join("subagents")
        .join(format!("agent-{internal_agent_id}.jsonl"))
}

/// Incremental reader for one native child transcript. The cursor advances
/// only past newline-terminated JSONL records, so a writer caught halfway
/// through an object is retried on the next poll rather than parsed as a
/// corrupt event or silently skipped. Each child owns one cursor, which keeps
/// repeated config polls from duplicating its journal.
#[derive(Default)]
pub struct TranscriptTail {
    offset: u64,
}

/// Lead-only lifecycle facts from Claude's structured JSONL.  The event
/// journal intentionally filters its `system/init` marker for ordinary
/// history, but a native Crew root needs that marker to leave `Starting`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LeadLifecycle {
    TurnStart,
    Ready,
    Failed,
}

fn lead_lifecycle(line: &str) -> Option<LeadLifecycle> {
    let value: Value = serde_json::from_str(line).ok()?;
    match (
        value.get("type").and_then(Value::as_str),
        value.get("subtype").and_then(Value::as_str),
    ) {
        (Some("system"), Some("init")) => Some(LeadLifecycle::TurnStart),
        (Some("result"), _) if value.get("is_error").and_then(Value::as_bool) == Some(true) => {
            Some(LeadLifecycle::Failed)
        }
        (Some("result"), _) => Some(LeadLifecycle::Ready),
        _ => None,
    }
}

/// The documented hook record is the bridge between Claude's two unrelated
/// identities: config/mailbox member `name` and child transcript `agentId`.
/// The ids must never be compared directly. `None` is an explicit absence a
/// runtime preflight can refuse, never an invitation to attach the first file.
pub fn subagent_start_identity(line: &str) -> Option<(String, String)> {
    let value: Value = serde_json::from_str(line).ok()?;
    let internal = value.get("agentId").and_then(Value::as_str)?.to_owned();
    let hook_name = find_hook_name(&value)?;
    let member = hook_name.strip_prefix("SubagentStart:")?.trim();
    (!member.is_empty()).then_some((internal, member.to_owned()))
}

pub fn subagent_start_from_file(path: &Path) -> std::io::Result<Option<(String, String)>> {
    let file = fs::File::open(path)?;
    let mut reader = BufReader::new(file);
    let mut line = String::new();
    while reader.read_line(&mut line)? != 0 {
        if let Some(identity) = subagent_start_identity(&line) {
            return Ok(Some(identity));
        }
        line.clear();
    }
    Ok(None)
}

/// Resolve the provider-owned lead session id in config to its actual
/// transcript directory. This searches only the documented project/session
/// layout; no TUI or guessed encoded project path is involved.
pub fn lead_subagents(home: &Path, config: &Value) -> Option<PathBuf> {
    let session = config.get("leadSessionId")?.as_str()?;
    let projects = home.join(".claude").join("projects");
    let entries = fs::read_dir(projects).ok()?;
    entries.flatten().find_map(|entry| {
        let path = entry.path().join(session).join("subagents");
        path.is_dir().then_some(path)
    })
}

/// The interactive lead's structured JSONL transcript. Claude keeps the lead
/// record beside (rather than inside) its `<session>/subagents` directory.
/// This is the same provider-owned `leadSessionId` used for child discovery;
/// no terminal byte is used as a substitute when the file is absent.
pub fn lead_transcript(home: &Path, config: &Value) -> Option<PathBuf> {
    let session = config.get("leadSessionId")?.as_str()?;
    let projects = home.join(".claude").join("projects");
    fs::read_dir(projects).ok()?.flatten().find_map(|entry| {
        let path = entry.path().join(format!("{session}.jsonl"));
        path.is_file().then_some(path)
    })
}

fn find_hook_name(value: &Value) -> Option<&str> {
    match value {
        Value::Object(object) => {
            if object.get("hookEvent").and_then(Value::as_str) == Some("SubagentStart") {
                if let Some(name) = object.get("hookName").and_then(Value::as_str) {
                    return Some(name);
                }
            }
            object.values().find_map(find_hook_name)
        }
        Value::Array(values) => values.iter().find_map(find_hook_name),
        _ => None,
    }
}

impl TranscriptTail {
    pub fn read_new(&mut self, path: &Path) -> std::io::Result<Vec<crate::session::history::Past>> {
        self.read_new_with_lifecycle(path).map(|(events, _)| events)
    }

    /// Like [`read_new`], retaining the lead's non-rendered lifecycle markers
    /// for the Crew root state machine. Child callers continue using the
    /// journal-only form above.
    pub fn read_new_with_lifecycle(
        &mut self,
        path: &Path,
    ) -> std::io::Result<(Vec<crate::session::history::Past>, Vec<LeadLifecycle>)> {
        let file = fs::File::open(path)?;
        let mut reader = BufReader::new(file);
        reader.seek(SeekFrom::Start(self.offset))?;
        let now = chrono::Utc::now().to_rfc3339();
        let mut events = Vec::new();
        let mut lifecycle = Vec::new();
        loop {
            let mut line = Vec::new();
            let bytes = reader.read_until(b'\n', &mut line)?;
            if bytes == 0 || !line.ends_with(b"\n") {
                break;
            }
            self.offset = self.offset.saturating_add(bytes as u64);
            let line = String::from_utf8_lossy(&line);
            if let Some(state) = lead_lifecycle(&line) {
                lifecycle.push(state);
            }
            events.extend(crate::session::history::events_of(&line, &now));
        }
        Ok((events, lifecycle))
    }
}

/// An addressed message is written to the *member name* inbox, not the
/// `name@team` member id and not the internal transcript id. The runtime owns
/// the containing team's lock discipline; callers must never replace this file
/// with a standalone atomic rename, which would discard a simultaneous native
/// teammate message.
pub fn inbox(team_dir: &Path, member_name: &str) -> PathBuf {
    team_dir.join("inboxes").join(format!("{member_name}.json"))
}

/// Append one addressed message through Smetana's own short-lived lock.
///
/// Claude consumes its inbox independently, so this cannot promise exclusion
/// against a native runtime writer whose lock protocol is undocumented. It
/// does promise that two Smetana sends do not overwrite one another, refuses a
/// malformed inbox, and rechecks the exact bytes after the lock and before the
/// atomic replacement. A changed inbox is retried a bounded number of times;
/// a persistent race is reported to the composer and is never redirected to a
/// lead or sibling.
pub fn append_message(
    team_dir: &Path,
    member_name: &str,
    from: &str,
    text: &str,
    summary: &str,
) -> Result<(), MailboxError> {
    const ATTEMPTS: u8 = 4;
    let path = inbox(team_dir, member_name);
    let id = crate::terminal::conversation::new_id().ok_or(MailboxError::NoMessageId)?;
    for attempt in 0..ATTEMPTS {
        if !member_can_message(team_dir, member_name)? {
            return Err(MailboxError::Unavailable);
        }
        let before = read_mailbox(&path)?;
        let Some(_lock) = InboxLock::acquire(&path)? else {
            if attempt + 1 < ATTEMPTS {
                std::thread::sleep(Duration::from_millis(20));
                continue;
            }
            return Err(MailboxError::Busy);
        };
        if !member_can_message(team_dir, member_name)? {
            return Err(MailboxError::Unavailable);
        }
        let locked = read_mailbox(&path)?;
        if locked != before {
            if attempt + 1 < ATTEMPTS {
                std::thread::sleep(Duration::from_millis(20));
                continue;
            }
            return Err(MailboxError::Changed);
        }
        let mut entries: Vec<MailboxMessage> =
            serde_json::from_slice(&locked).map_err(|_| MailboxError::Corrupt)?;
        entries.push(MailboxMessage {
            from: from.to_owned(),
            text: text.to_owned(),
            summary: summary.to_owned(),
            timestamp: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            version: 1,
            id: id.clone(),
            kind: "message".into(),
            read: false,
        });
        let replacement =
            serde_json::to_vec(&entries).map_err(|error| MailboxError::Write(error.to_string()))?;
        // The native consumer may have emptied the mailbox after our first
        // check. Refuse rather than write a stale array over that change.
        if read_mailbox(&path)? != locked {
            if attempt + 1 < ATTEMPTS {
                std::thread::sleep(Duration::from_millis(20));
                continue;
            }
            return Err(MailboxError::Changed);
        }
        replace_mailbox(&path, &replacement)?;
        return Ok(());
    }
    Err(MailboxError::Changed)
}

/// The config is the runtime's membership authority. It is intentionally read
/// both before and after acquiring the Smetana lock: selecting a node and
/// clicking Send can race a teammate leaving the team.
fn member_can_message(team_dir: &Path, member_name: &str) -> Result<bool, MailboxError> {
    let config = fs::read(team_dir.join("config.json"))
        .map_err(|error| MailboxError::Read(error.to_string()))?;
    let config: Value = serde_json::from_slice(&config).map_err(|_| MailboxError::Corrupt)?;
    Ok(config
        .get("members")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .any(|member| {
            member.get("name").and_then(Value::as_str) == Some(member_name)
                && member.get("agentType").and_then(Value::as_str) != Some("team-lead")
                && !matches!(
                    member.get("status").and_then(Value::as_str),
                    Some("left" | "completed" | "failed")
                )
        }))
}

fn read_mailbox(path: &Path) -> Result<Vec<u8>, MailboxError> {
    let bytes = fs::read(path).map_err(|error| MailboxError::Read(error.to_string()))?;
    if !matches!(serde_json::from_slice::<Value>(&bytes), Ok(Value::Array(_))) {
        return Err(MailboxError::Corrupt);
    }
    Ok(bytes)
}

fn replace_mailbox(path: &Path, bytes: &[u8]) -> Result<(), MailboxError> {
    static TEMP: AtomicU64 = AtomicU64::new(1);
    let parent = path
        .parent()
        .ok_or_else(|| MailboxError::Write("the inbox has no parent directory".into()))?;
    let temp = parent.join(format!(
        ".smetana-inbox-{}-{}.tmp",
        std::process::id(),
        TEMP.fetch_add(1, Ordering::Relaxed)
    ));
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
            .map_err(|error| MailboxError::Write(error.to_string()))?;
        file.write_all(bytes)
            .map_err(|error| MailboxError::Write(error.to_string()))?;
        file.sync_all()
            .map_err(|error| MailboxError::Write(error.to_string()))?;
        fs::rename(&temp, path).map_err(|error| MailboxError::Write(error.to_string()))
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

struct InboxLock(PathBuf);

impl InboxLock {
    fn acquire(mailbox: &Path) -> Result<Option<Self>, MailboxError> {
        let lock = mailbox.with_extension("json.smetana.lock");
        match OpenOptions::new().write(true).create_new(true).open(&lock) {
            Ok(_) => Ok(Some(Self(lock))),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => Ok(None),
            Err(error) => Err(MailboxError::Write(error.to_string())),
        }
    }
}

impl Drop for InboxLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

/// Reads `members[]` from a team config. Native Claude teams are one level
/// deep, so every teammate's parent is the Smetana root regardless of the
/// provider's own member ordering.
pub fn members(config: &Value) -> Vec<ProviderNode> {
    config
        .get("members")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|member| {
            let id = member.get("agentId").and_then(Value::as_str)?.to_owned();
            if member.get("agentType").and_then(Value::as_str) == Some("team-lead") {
                return None;
            }
            let name = member
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("Background agent");
            let state = match member.get("status").and_then(Value::as_str) {
                Some("idle") => ProviderState::Waiting,
                Some("left") | Some("completed") => ProviderState::Done,
                Some("failed") => ProviderState::Failed,
                Some("working") => ProviderState::Running,
                _ => ProviderState::Starting,
            };
            Some(ProviderNode {
                id,
                parent: None,
                state,
                label: Some(name.to_owned()),
                can_message: !matches!(state, ProviderState::Done | ProviderState::Failed),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::library::Skills;
    use crate::agents::{Intent, Launch};

    fn launch() -> Launch {
        Launch {
            profile: &Claude,
            cwd: PathBuf::from("/tmp/project"),
            intent: Intent::Bare,
            skills: Skills {
                smetana: PathBuf::from("/app/resources/smetana"),
                superpowers: PathBuf::from("/app/resources/superpowers"),
                superpowers_installed: false,
            },
            languages: crate::agents::Languages::default(),
            agent_prompt: String::new(),
            facts: None,
            session_id: Some("lead-session".into()),
            model: None,
            worker_model: None,
        }
    }

    #[test]
    fn interactive_lead_holds_its_prompt_until_structured_admission() {
        let launch = launch();
        let expected = Claude.prompt_text(&launch).expect("bare lead has a prompt");
        let (command, deferred) = interactive_lead_command(&launch);
        let argv: Vec<_> = command
            .get_argv()
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();
        assert_eq!(deferred.as_deref(), Some(expected.as_str()));
        assert!(!argv.iter().any(|arg| arg == &expected));
        assert!(argv.windows(2).any(|args| args == [TEAM_FLAG, TEAM_MODE]));
    }

    #[test]
    fn lead_lifecycle_comes_from_structured_transcript_records() {
        assert_eq!(
            lead_lifecycle(r#"{"type":"system","subtype":"init"}"#),
            Some(LeadLifecycle::TurnStart)
        );
        assert_eq!(
            lead_lifecycle(r#"{"type":"result","is_error":false}"#),
            Some(LeadLifecycle::Ready)
        );
        assert_eq!(
            lead_lifecycle(r#"{"type":"result","is_error":true}"#),
            Some(LeadLifecycle::Failed)
        );
    }

    #[test]
    fn config_members_are_structured_children_and_the_lead_is_not_duplicated() {
        let config: Value = serde_json::from_str(include_str!(
            "../../tests/fixtures/claude-2.1.281-team-config.json"
        ))
        .unwrap();
        assert_eq!(team_name(&config), Some("session-fixture-team".into()));
        assert_eq!(
            members(&config),
            vec![ProviderNode {
                id: "fixture-worker@session-fixture-team".into(),
                parent: None,
                state: ProviderState::Starting,
                label: Some("fixture-worker".into()),
                can_message: true,
            }]
        );
    }

    #[test]
    fn a_team_name_comes_from_the_structured_runtime_not_the_cli_session() {
        assert_eq!(
            team_name(&serde_json::json!({"name":"session-team-42"})),
            Some("session-team-42".into())
        );
        assert_eq!(
            team_name(&serde_json::json!({"leadSessionId":"different"})),
            None
        );
    }

    #[test]
    fn lead_session_selects_its_exact_new_config_among_stale_and_concurrent_teams() {
        let stale = PathBuf::from("/teams/stale");
        let ours = PathBuf::from("/teams/ours");
        let other = PathBuf::from("/teams/other");
        let baseline = HashSet::from([stale.clone()]);
        let candidates = vec![
            (stale, serde_json::json!({"leadSessionId":"lead-a"})),
            (ours.clone(), serde_json::json!({"leadSessionId":"lead-a"})),
            (other, serde_json::json!({"leadSessionId":"lead-b"})),
        ];
        let selected = teams_for_lead(candidates, "lead-a", &baseline);
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].0, ours);
    }

    #[test]
    fn lead_session_never_adopts_a_concurrent_config_with_another_id() {
        let candidates = vec![
            (PathBuf::from("/teams/a"), serde_json::json!({"leadSessionId":"lead-a"})),
            (PathBuf::from("/teams/b"), serde_json::json!({"leadSessionId":"lead-b"})),
        ];
        assert!(teams_for_lead(candidates, "lead-c", &HashSet::new()).is_empty());
    }

    #[test]
    fn config_and_child_transcript_paths_are_derived_without_a_tui() {
        assert_eq!(
            team_config(Path::new("/home/a"), "session-12345678"),
            PathBuf::from("/home/a/.claude/teams/session-12345678/config.json")
        );
        assert_eq!(
            transcript(
                Path::new("/home/a/.claude/projects/p/lead"),
                "areviewer-stable-hash"
            ),
            PathBuf::from(
                "/home/a/.claude/projects/p/lead/subagents/agent-areviewer-stable-hash.jsonl"
            )
        );
        assert_eq!(
            inbox(
                Path::new("/home/a/.claude/teams/session-12345678"),
                "reviewer"
            ),
            PathBuf::from("/home/a/.claude/teams/session-12345678/inboxes/reviewer.json")
        );
    }

    #[test]
    fn captured_send_message_mailbox_shape_has_no_recipient_field() {
        let captured = include_str!("../../tests/fixtures/claude-2.1.281-team-mailbox.json");
        let entries: Vec<MailboxMessage> = serde_json::from_str(captured).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].from, "team-lead");
        assert_eq!(entries[0].text, "CAPTURE-MARKER");
        assert_eq!(entries[0].summary, "Capture mailbox schema");
        assert_eq!(entries[0].version, 1);
        assert_eq!(entries[0].kind, "message");
        assert!(!entries[0].read);
    }

    #[test]
    fn addressed_send_preserves_the_existing_inbox_and_refuses_a_finished_member() {
        let root = std::env::temp_dir().join(format!(
            "smetana-claude-crew-mailbox-{}",
            std::process::id()
        ));
        let inboxes = root.join("inboxes");
        fs::create_dir_all(&inboxes).unwrap();
        fs::write(
            root.join("config.json"),
            r#"{"members":[{"name":"worker","agentId":"worker@team","agentType":"general-purpose"}]}"#,
        )
        .unwrap();
        let path = inbox(&root, "worker");
        fs::write(
            &path,
            r#"[{"from":"team-lead","text":"first","summary":"first","timestamp":"now","msgV":1,"msg_id":"first-id","type":"message","read":false}]"#,
        )
        .unwrap();
        append_message(
            &root,
            "worker",
            "team-lead",
            "only worker gets this",
            "specific target",
        )
        .unwrap();
        let entries: Vec<MailboxMessage> =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[1].text, "only worker gets this");
        fs::write(
            root.join("config.json"),
            r#"{"members":[{"name":"worker","agentId":"worker@team","agentType":"general-purpose","status":"left"}]}"#,
        )
        .unwrap();
        assert_eq!(
            append_message(&root, "worker", "team-lead", "must not redirect", "race"),
            Err(MailboxError::Unavailable)
        );
        let entries: Vec<MailboxMessage> =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(entries.len(), 2, "a rejected message must not be rerouted");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn preflight_names_the_missing_capability_and_required_version() {
        let root = std::env::temp_dir().join(format!(
            "smetana-claude-crew-preflight-{}",
            std::process::id()
        ));
        let team = root.join("team");
        let lead = root.join("lead");
        fs::create_dir_all(team.join("inboxes")).unwrap();
        fs::create_dir_all(lead.join("subagents")).unwrap();
        fs::write(
            team.join("config.json"),
            include_str!("../../tests/fixtures/claude-2.1.281-team-config.json"),
        )
        .unwrap();
        fs::write(inbox(&team, "fixture-worker"), "[]").unwrap();
        assert!(preflight("2.1.281", &team, &lead).is_ok());
        assert!(matches!(
            preflight("2.1.280", &team, &lead),
            Err(PreflightError::Version { .. })
        ));
        let _ = fs::remove_dir_all(lead.join("subagents"));
        assert!(matches!(
            preflight("2.1.281", &team, &lead),
            Err(PreflightError::Journal(_))
        ));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn transcript_tail_deduplicates_complete_lines_and_retries_a_partial_one() {
        let path = std::env::temp_dir().join(format!("smetana-team-tail-{}.jsonl", std::process::id()));
        fs::write(
            &path,
            "{\"type\":\"assistant\",\"message\":{\"content\":[{\"type\":\"text\",\"text\":\"first\"}]}}\n",
        )
        .unwrap();
        let mut tail = TranscriptTail::default();
        assert!(!tail.read_new(&path).unwrap().is_empty());
        assert!(tail.read_new(&path).unwrap().is_empty());
        let mut file = OpenOptions::new().append(true).open(&path).unwrap();
        file.write_all(b"{\"type\":\"assistant\"").unwrap();
        assert!(tail.read_new(&path).unwrap().is_empty());
        file.write_all(b",\"message\":{\"content\":[{\"type\":\"text\",\"text\":\"second\"}]}}\n")
            .unwrap();
        assert!(!tail.read_new(&path).unwrap().is_empty());
        let _ = fs::remove_file(path);
    }

    #[test]
    fn subagent_start_hook_maps_internal_transcript_identity_to_member_name() {
        let line = include_str!("../../tests/fixtures/claude-2.1.281-subagent-start.jsonl");
        assert_eq!(
            subagent_start_identity(line.trim()),
            Some((
                "afixture-worker-75e6a9dd8a6d5c7d".into(),
                "fixture-worker".into()
            ))
        );
    }
}
