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
pub const BOOTSTRAP_MEMBER_NAME: &str = "smetana-bootstrap";

/// Claude creates its native team config the moment the interactive runtime
/// starts — holding only the team-lead — before any prompt has been sent; two
/// live probes (2026-09-27, Claude Code 2.1.281) caught `config.json` on disk
/// in the same second as the process launch. What actually needs an
/// interactive turn is the *teammate*: the model has to read one before
/// `smetana-bootstrap` (or any other member), its inbox, and the lead's own
/// `subagents/`/transcript files exist. This deliberately inert first turn
/// asks for exactly that and nothing else, allowing the real Run brief to
/// stay behind capability admission. It is not a rendered-TUI command or a
/// navigation sequence.
pub const BOOTSTRAP_PROMPT: &str = "Initialize a native Claude Agent Team runtime now: create one inert teammate named smetana-bootstrap and tell it only to reply READY. Reply READY once the structured team config, lead transcript, and teammate inboxes exist. You and that teammate MUST NOT read the board, claim tasks, create worktrees, inspect or modify project files, run project commands, review code, merge, or perform any task work. This is transport initialization only.";

/// No line terminator of any kind. A live probe (2026-09-28, Claude Code
/// 2.1.283) measured the interactive TUI's own input contract: LF is read as
/// a newline inside the composer rather than as Enter, and a `\r` written
/// before the runtime has switched the terminal to raw mode is turned into
/// LF by the pty line discipline (ICRNL) — so neither terminator sends this
/// turn at the moment it is written. The text still goes on the PTY right
/// after spawn, since it was measured to sit buffered in the composer until
/// the TUI is ready for it; what actually submits it is the repeated bare
/// `\r` `refresh_claude_crews` writes on each tick — see `needs_enter_nudge`
/// below and `.claude/rules/agents.md`'s "Native Crew transports".
pub fn bootstrap_input() -> Vec<u8> {
    BOOTSTRAP_PROMPT.as_bytes().to_vec()
}

/// Whether one admission tick should write another bare `\r` into the Claude
/// Crew lead's PTY. The interactive TUI submits a buffered turn only once it
/// has switched the terminal to raw mode — a fact this app cannot observe
/// directly (`.claude/rules/terminal.md` forbids decoding the screen) — so
/// instead of a fixed delay tuned to one machine, admission retries Enter on
/// every tick until the lead's own structured transcript proves the turn
/// already began. That makes the retry self-correcting rather than a second
/// timing constant: a `\r` arriving before raw mode is turned into a harmless
/// extra newline in the still-empty composer by the pty line discipline
/// (ICRNL); one landing on an empty raw-mode field does nothing; the first to
/// land once raw mode is active is the one that actually sends it. Once
/// `lead_transcript` answers `Some` the turn has started and nothing is
/// written again, and once admission itself has completed
/// (`admission_pending` false) nothing is written either, whatever the
/// transcript says — a completed admission has already delivered the real
/// Run brief and a stray `\r` at that point would be typed into it instead.
pub fn needs_enter_nudge(admission_pending: bool, lead_transcript_exists: bool) -> bool {
    admission_pending && !lead_transcript_exists
}

/// The only supported Claude Crew lead line. It is deliberately separate from
/// `ClaudeDriver`, whose `-p --input-format stream-json` protocol cannot make
/// native teammates. The team runtime must remain interactive while Smetana
/// observes its documented config/transcript/mailbox files.
/// Build the interactive runtime without a positional Run brief. The session
/// worker first writes [`BOOTSTRAP_PROMPT`], proves the structured runtime, and
/// only then writes this returned real brief to the live PTY.
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

/// The six things `session::service::refresh_claude_crews` waits on before a
/// Claude Crew lead is admitted, in the order a fresh launch actually clears
/// them (2026-09-27 measurements: the config with the lone lead lands inside
/// the same second as the spawn; the teammate, its inbox, `subagents/` and the
/// lead's own transcript follow only once the model has read the bootstrap
/// turn). None of these being absent is a failure on its own — the admission
/// loop simply waits for the next tick — but the *last* one still missing when
/// the 90-second timeout fires is what [`admission_timeout_message`] names.
/// `WAIT_NO_BOOTSTRAP_INBOX` is the narrowest of the six: `inboxes/` itself
/// can exist a tick or more before Claude has written the bootstrap
/// teammate's own `<name>.json` inside it, and `preflight`'s `OpenOptions`
/// read cannot tell that ordinary race from a real one — so it is checked for
/// ahead of `preflight`, the same way `WAIT_NO_INBOXES_DIR` already is.
pub const WAIT_NO_TEAM_CONFIG: &str = "the team config never appeared";
pub const WAIT_NO_BOOTSTRAP: &str = "the bootstrap teammate never joined";
pub const WAIT_NO_SUBAGENTS_DIR: &str = "no subagents directory";
pub const WAIT_NO_INBOXES_DIR: &str = "no inboxes directory";
pub const WAIT_NO_BOOTSTRAP_INBOX: &str = "the bootstrap teammate has no inbox yet";
pub const WAIT_NO_LEAD_TRANSCRIPT: &str = "no lead transcript";

/// The sentence a refused `CrewStart` reply carries when admission times out.
/// `reason` is whichever of the five constants above the admission loop last
/// recorded, so a person reads what was actually still missing rather than
/// one fixed phrase for every cause.
pub fn admission_timeout_message(reason: &str) -> String {
    format!(
        "the session could not be started: Claude Crew runtime did not establish an exact team config, transcript, and inbox contract within 90 seconds ({reason})"
    )
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

/// The first non-lead, non-finished member in `config` whose own inbox file
/// does not exist yet, or `None` when every relevant member already has one.
/// Mirrors `preflight`'s own member loop, but asks only whether the path
/// **exists** rather than opening it — existence is what distinguishes the
/// ordinary "Claude has not written this file yet" race from a real problem.
/// A path that exists but is the wrong kind, or cannot be opened for
/// writing, answers `None` here and is left for `preflight` to refuse as the
/// contradiction it actually is; masking that behind another tick of waiting
/// would turn a real fault into a timeout with no message worth reading.
fn member_inbox_missing(team_dir: &Path, config: &Value) -> Option<String> {
    config
        .get("members")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .find_map(|member| {
            if member.get("agentType").and_then(Value::as_str) == Some("team-lead") {
                return None;
            }
            if matches!(
                member.get("status").and_then(Value::as_str),
                Some("left" | "completed" | "failed")
            ) {
                return None;
            }
            let name = member.get("name").and_then(Value::as_str)?;
            (!inbox(team_dir, name).exists()).then(|| name.to_owned())
        })
}

/// What one admission tick decides about a Claude Crew lead, once
/// `refresh_claude_crews` has already found the team directory and config
/// for *this* launch. Finding that directory in the first place, and
/// confirming its config still names it, stay that caller's own: both run
/// identically for a starting package and an already-admitted one, so moving
/// them in here would only give the two a second copy to drift against.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AdmissionStep {
    /// Missing exactly one of the `WAIT_NO_*` things above. Not a failure —
    /// the caller is meant to try again on the next tick.
    Waiting(&'static str),
    /// Every structured file this app can address by our own `--session-id`
    /// is present and open. The three fields are what admission still has to
    /// hand off before the Run brief can be written.
    Ready {
        bootstrap: String,
        subagents: PathBuf,
        lead_transcript: PathBuf,
    },
    /// A contradiction rather than ordinary startup timing: every file
    /// `preflight` and the checks ahead of it look for was already confirmed
    /// present by the point it ran, so an error here means something is
    /// genuinely wrong rather than merely not yet written.
    Failed(String),
}

/// The whole of one admission tick's readiness decision, pure so a test can
/// drive it tick by tick without a worker, a `AppHandle` or a real Claude
/// process behind it. `refresh_claude_crews` calls this only while a package
/// is still in `starting`; an already-admitted package keeps its own,
/// unchanged health checks, which is deliberate — see this type's own
/// comment for why they do not share this function.
pub fn admission_step(
    home: &Path,
    team_dir: &Path,
    config: &Value,
    expected_session: &str,
) -> AdmissionStep {
    let Some(bootstrap) = bootstrap_member_id(config) else {
        return AdmissionStep::Waiting(WAIT_NO_BOOTSTRAP);
    };
    let Some(subagents) = lead_subagents(home, expected_session) else {
        return AdmissionStep::Waiting(WAIT_NO_SUBAGENTS_DIR);
    };
    let Some(lead_session_dir) = subagents.parent() else {
        return AdmissionStep::Failed("Claude Crew lead transcript directory is invalid".into());
    };
    if !team_dir.join("inboxes").is_dir() {
        return AdmissionStep::Waiting(WAIT_NO_INBOXES_DIR);
    }
    if member_inbox_missing(team_dir, config).is_some() {
        return AdmissionStep::Waiting(WAIT_NO_BOOTSTRAP_INBOX);
    }
    // Version selection happened before the run entered the board loop; this
    // is the runtime half, and by this point subagents/, inboxes/ and every
    // relevant member's own inbox file have all been confirmed present, so an
    // error here is the contradiction this type's own doc comment describes.
    if let Err(error) = preflight("2.1.281", team_dir, lead_session_dir) {
        return AdmissionStep::Failed(error.to_string());
    }
    let Some(lead_transcript) = lead_transcript(home, expected_session) else {
        return AdmissionStep::Waiting(WAIT_NO_LEAD_TRANSCRIPT);
    };
    AdmissionStep::Ready { bootstrap, subagents, lead_transcript }
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

/// Narrow candidates to the ones the runtime created for *this* launch.
/// `leadSessionId` cannot bind a candidate to our own `--session-id`: two live
/// probes (2026-09-27, Claude Code 2.1.281) showed Claude mints that field
/// itself, independently of the id this app passed on `--session-id` — the
/// two never matched in either probe. `baseline` is what stands in for that
/// binding instead: every team directory that already existed before this
/// launch's interactive lead was spawned, so anything new since is a
/// candidate. cwd alone still cannot distinguish two concurrent Crew packages
/// in the same project; a caller seeing more than one new candidate here is
/// meant to wait for one to resolve rather than guess between them (see
/// `refresh_claude_crews` in `session::service`).
pub fn teams_for_lead(
    candidates: Vec<(PathBuf, Value)>,
    baseline: &HashSet<PathBuf>,
) -> Vec<(PathBuf, Value)> {
    candidates
        .into_iter()
        .filter(|(team, _config)| !baseline.contains(team))
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

/// The lead's own subagents directory, addressed by *our* `--session-id`
/// rather than by the runtime's `leadSessionId`. The two never agree
/// (see [`teams_for_lead`]'s own comment for the measurement): the lead's
/// transcript directory is written under the id this app actually passed on
/// `--session-id`, not under whatever Claude generated for the config. This
/// searches only the documented project/session layout; no TUI or guessed
/// encoded project path is involved.
pub fn lead_subagents(home: &Path, session: &str) -> Option<PathBuf> {
    let projects = home.join(".claude").join("projects");
    let entries = fs::read_dir(projects).ok()?;
    entries.flatten().find_map(|entry| {
        let path = entry.path().join(session).join("subagents");
        path.is_dir().then_some(path)
    })
}

/// The interactive lead's structured JSONL transcript. Claude keeps the lead
/// record beside (rather than inside) its `<session>/subagents` directory.
/// This is addressed by the same `--session-id` [`lead_subagents`] above
/// uses, never by the config's own `leadSessionId` — see that function's
/// comment for why the two do not name the same session; no terminal byte is
/// used as a substitute when the file is absent.
pub fn lead_transcript(home: &Path, session: &str) -> Option<PathBuf> {
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
/// Capture the provider identity of *our* bootstrap helper at admission. Its
/// name is only used at that controlled handshake; every later exclusion is by
/// this exact provider id, so a real teammate with a similar label is never
/// hidden.
pub fn bootstrap_member_id(config: &Value) -> Option<String> {
    config
        .get("members")
        .and_then(Value::as_array)?
        .iter()
        .find(|member| member.get("name").and_then(Value::as_str) == Some(BOOTSTRAP_MEMBER_NAME))?
        .get("agentId")
        .and_then(Value::as_str)
        .map(str::to_owned)
}

pub fn member_id(config: &Value, name: &str) -> Option<String> {
    config
        .get("members")
        .and_then(Value::as_array)?
        .iter()
        .find(|member| member.get("name").and_then(Value::as_str) == Some(name))?
        .get("agentId")
        .and_then(Value::as_str)
        .map(str::to_owned)
}

/// The bootstrap helper is private only while it remains the inert admission
/// handshake. If Claude later changes this exact member to active work, it is
/// a normal native teammate and must regain a public node and transcript.
pub fn bootstrap_is_working(config: &Value, provider: &str) -> bool {
    config
        .get("members")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .find(|member| member.get("agentId").and_then(Value::as_str) == Some(provider))
        .and_then(|member| member.get("status").and_then(Value::as_str))
        .is_some_and(|status| matches!(status, "working" | "active"))
}

pub fn bootstrap_is_internal(config: &Value, provider: &str, admission_pending: bool) -> bool {
    admission_pending || !bootstrap_is_working(config, provider)
}

pub fn members_excluding(config: &Value, excluded_provider: Option<&str>) -> Vec<ProviderNode> {
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
            if excluded_provider == Some(id.as_str()) {
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
                Some("working" | "active") => ProviderState::Running,
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
    fn bootstrap_input_carries_no_line_terminator() {
        let input = bootstrap_input();
        assert!(!input.ends_with(b"\n"));
        assert!(!input.ends_with(b"\r"));
        assert_eq!(input, BOOTSTRAP_PROMPT.as_bytes());
    }

    #[test]
    fn enter_nudge_writes_until_the_transcript_proves_the_turn_began() {
        // No transcript yet: keep retrying Enter.
        assert!(needs_enter_nudge(true, false));
        // The transcript exists: the turn already began, stop.
        assert!(!needs_enter_nudge(true, true));
        // Admission itself is over, whatever the transcript says: the real
        // Run brief has already been delivered, and nothing should be typed
        // into it after the fact.
        assert!(!needs_enter_nudge(false, false));
        assert!(!needs_enter_nudge(false, true));
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
            members_excluding(&config, None),
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
    fn bootstrap_member_is_excluded_by_exact_provider_identity() {
        let config: Value = serde_json::from_str(include_str!(
            "../../tests/fixtures/claude-2.1.281-bootstrap-team-config.json"
        ))
        .unwrap();
        let bootstrap = bootstrap_member_id(&config).expect("captured bootstrap id");
        assert_eq!(bootstrap, "smetana-bootstrap@session-bootstrap-fixture");
        assert_eq!(
            config.pointer("/members/1/status").and_then(Value::as_str),
            Some("idle"),
            "the captured native config retains the bootstrap lifecycle state"
        );
        assert_eq!(
            members_excluding(&config, Some(&bootstrap)),
            vec![ProviderNode {
                id: "real-worker@session-bootstrap-fixture".into(),
                parent: None,
                state: ProviderState::Running,
                label: Some("bootstrap-helper".into()),
                can_message: true,
            }]
        );
    }

    #[test]
    fn an_admitted_bootstrap_that_starts_working_becomes_a_real_member() {
        let mut config: Value = serde_json::from_str(include_str!(
            "../../tests/fixtures/claude-2.1.281-bootstrap-team-config.json"
        ))
        .unwrap();
        let bootstrap = bootstrap_member_id(&config).expect("captured bootstrap id");
        assert!(!bootstrap_is_working(&config, &bootstrap));
        assert!(bootstrap_is_internal(&config, &bootstrap, true));
        assert!(bootstrap_is_internal(&config, &bootstrap, false));
        config["members"][1]["status"] = Value::String("working".into());
        assert!(bootstrap_is_working(&config, &bootstrap));
        assert!(!bootstrap_is_internal(&config, &bootstrap, false));
        assert_eq!(
            member_id(&config, BOOTSTRAP_MEMBER_NAME).as_deref(),
            Some(bootstrap.as_str()),
            "post-admission transcript routing stays bound to the exact id"
        );
        assert!(members_excluding(&config, None)
            .iter()
            .any(|node| node.id == bootstrap && node.state == ProviderState::Running));
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
    fn lead_session_excludes_only_directories_that_existed_before_the_launch() {
        // leadSessionId plays no part in the selection any more (see
        // `teams_for_lead`'s own comment): both the stale and the new
        // directory below name unrelated, Claude-generated ids, and the
        // baseline is what tells them apart instead.
        let stale = PathBuf::from("/teams/stale");
        let ours = PathBuf::from("/teams/ours");
        let baseline = HashSet::from([stale.clone()]);
        let candidates = vec![
            (stale, serde_json::json!({"leadSessionId":"unrelated-a"})),
            (ours.clone(), serde_json::json!({"leadSessionId":"unrelated-b"})),
        ];
        let selected = teams_for_lead(candidates, &baseline);
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].0, ours);
    }

    #[test]
    fn two_directories_new_since_the_baseline_are_both_returned_to_wait_on() {
        // Ambiguity between two concurrent same-cwd launches is the caller's
        // to resolve by waiting — `teams_for_lead` itself only excludes what
        // was already there, so it hands back both rather than guessing.
        let candidates = vec![
            (PathBuf::from("/teams/a"), serde_json::json!({"leadSessionId":"lead-a"})),
            (PathBuf::from("/teams/b"), serde_json::json!({"leadSessionId":"lead-b"})),
        ];
        assert_eq!(teams_for_lead(candidates, &HashSet::new()).len(), 2);
    }

    #[test]
    fn lead_subagents_and_transcript_are_found_by_our_own_session_id_not_leadsessionid() {
        let root = std::env::temp_dir().join(format!(
            "smetana-claude-crew-lead-paths-{}",
            std::process::id()
        ));
        let home = root.join("home");
        let session = "smetana-own-session-id";
        let lead_dir = home
            .join(".claude/projects/encoded-project")
            .join(session);
        fs::create_dir_all(lead_dir.join("subagents")).unwrap();
        fs::write(
            lead_dir.parent().unwrap().join(format!("{session}.jsonl")),
            "{}",
        )
        .unwrap();
        assert_eq!(lead_subagents(&home, session), Some(lead_dir.join("subagents")));
        assert_eq!(
            lead_transcript(&home, session),
            Some(lead_dir.parent().unwrap().join(format!("{session}.jsonl")))
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn admission_timeout_message_names_the_specific_missing_element() {
        let message = admission_timeout_message(WAIT_NO_INBOXES_DIR);
        assert!(message.contains("90 seconds"));
        assert!(message.contains(WAIT_NO_INBOXES_DIR));
    }

    #[test]
    fn admission_step_waits_through_each_missing_piece_then_admits() {
        let root = std::env::temp_dir().join(format!(
            "smetana-claude-crew-admission-step-{}",
            std::process::id()
        ));
        let home = root.join("home");
        let team = home.join(".claude/teams/session-new");
        let session = "exact-lead-session";
        let lead_dir = home.join(".claude/projects/encoded-project").join(session);
        fs::create_dir_all(&team).unwrap();

        // Pass 1: the config Claude writes the instant its runtime starts —
        // the team-lead alone. The model has not read the bootstrap turn yet.
        let lead_only: Value = serde_json::from_str(
            r#"{"name":"session-new","leadSessionId":"unrelated","members":[{"name":"team-lead","agentType":"team-lead","cwd":"/project"}]}"#,
        )
        .unwrap();
        assert_eq!(
            admission_step(&home, &team, &lead_only, session),
            AdmissionStep::Waiting(WAIT_NO_BOOTSTRAP)
        );

        // Pass 2: the teammate is in config, `subagents/` and `inboxes/`
        // exist, but Claude has not yet written the teammate's own inbox
        // file inside `inboxes/` — a missing FILE, not the directory. This is
        // the exact race review pass 1 found failing admission outright.
        // `preflight` re-reads `config.json` off disk on its own, so the
        // fixture has to be written there too, not only held in memory.
        fs::create_dir_all(team.join("inboxes")).unwrap();
        fs::create_dir_all(lead_dir.join("subagents")).unwrap();
        let with_teammate_json = r#"{"name":"session-new","leadSessionId":"unrelated","members":[{"name":"team-lead","agentType":"team-lead","cwd":"/project"},{"name":"smetana-bootstrap","agentType":"general-purpose","agentId":"smetana-bootstrap@session-new"}]}"#;
        fs::write(team.join("config.json"), with_teammate_json).unwrap();
        let with_teammate: Value = serde_json::from_str(with_teammate_json).unwrap();
        assert_eq!(
            admission_step(&home, &team, &with_teammate, session),
            AdmissionStep::Waiting(WAIT_NO_BOOTSTRAP_INBOX)
        );

        // Pass 3: the inbox file and the lead's own transcript exist too —
        // the full set, and admission succeeds.
        fs::write(team.join("inboxes/smetana-bootstrap.json"), "[]").unwrap();
        fs::write(
            lead_dir.parent().unwrap().join(format!("{session}.jsonl")),
            r#"{"type":"system","subtype":"init"}"#,
        )
        .unwrap();
        assert_eq!(
            admission_step(&home, &team, &with_teammate, session),
            AdmissionStep::Ready {
                bootstrap: "smetana-bootstrap@session-new".into(),
                subagents: lead_dir.join("subagents"),
                lead_transcript: lead_dir.parent().unwrap().join(format!("{session}.jsonl")),
            }
        );

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn admission_step_fails_on_an_inbox_that_exists_but_is_the_wrong_kind() {
        let root = std::env::temp_dir().join(format!(
            "smetana-claude-crew-admission-step-failed-{}",
            std::process::id()
        ));
        let home = root.join("home");
        let team = home.join(".claude/teams/session-new");
        let session = "exact-lead-session";
        let lead_dir = home.join(".claude/projects/encoded-project").join(session);
        fs::create_dir_all(team.join("inboxes")).unwrap();
        fs::create_dir_all(lead_dir.join("subagents")).unwrap();
        let config: Value = serde_json::from_str(
            r#"{"name":"session-new","leadSessionId":"unrelated","members":[{"name":"team-lead","agentType":"team-lead","cwd":"/project"},{"name":"smetana-bootstrap","agentType":"general-purpose","agentId":"smetana-bootstrap@session-new"}]}"#,
        )
        .unwrap();
        // The inbox path exists, so `member_inbox_missing`'s plain `exists()`
        // reads it as present and this reaches `preflight` — whose own
        // `OpenOptions::write` refuses a directory. Present but broken is a
        // genuine contradiction, never another tick of waiting.
        fs::create_dir_all(inbox(&team, "smetana-bootstrap")).unwrap();
        assert!(matches!(
            admission_step(&home, &team, &config, session),
            AdmissionStep::Failed(_)
        ));
        let _ = fs::remove_dir_all(&root);
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
