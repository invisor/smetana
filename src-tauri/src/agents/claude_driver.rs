//! Claude Code's protocol, as this app's vocabulary.
//!
//! A port of what `claude::transcript_line` already does for a terminal pane,
//! with one difference that runs through the whole file: this produces typed
//! events for components to draw, not strings for a terminal to print. So a
//! paragraph keeps its newlines — the front end parses markdown, and flattening
//! a list here would destroy it — while control bytes are still stripped, for
//! the reason `claude::one_line` records: the agent routinely quotes somebody
//! else's output, CSI sequences and a bell arrive inside it, and a bell would
//! turn the session's row `needs-you`.
//!
//! What a tool's call is doing is not decided again in here.
//! `claude::tool_detail` is the reference formatter's table of which field of
//! which tool says so, and it is borrowed rather than copied.
//!
//! **Streaming text deltas (smetana-6we6).** `--include-partial-messages` is
//! documented on the installed CLI (2.1.269) as "Include partial message
//! chunks as they arrive (only works with --print and
//! --output-format=stream-json)" — both of which this driver already passes —
//! and was verified rather than trusted: `claude -p --output-format
//! stream-json --input-format stream-json --include-partial-messages` against
//! a live model wraps the ordinary Messages API SSE shape one line per event,
//! `{"type":"stream_event","event":{...}}`, with no `data:` prefix. The one
//! piece this driver reads out of it is `content_block_delta` whose `delta`
//! carries `{"type":"text_delta","text":"..."}` — `EventKind::TextDelta`
//! below. Everything else the flag adds — `message_start`,
//! `content_block_start`/`content_block_stop` for every block kind,
//! `thinking_delta`, `signature_delta`, `input_json_delta` for a tool call's
//! arguments streaming in, `message_delta`, `message_stop` — produces nothing,
//! on the same rule as an event kind this file has never heard of, and on
//! purpose: reasoning and tool-call streaming are out of scope for this task,
//! and the whole consolidated `assistant` message this driver already turned
//! into `Text` still arrives once a block completes, unchanged. That whole
//! message is what actually closes a streamed reply — there is no separate
//! "stream finished" event to wait for, and none is needed.
//!
//! **The flag has no version probe on this line, and that absence was
//! decided rather than overlooked.** This driver's own command line already
//! puts `--input-format stream-json`, `--mcp-config` and
//! `--append-system-prompt` on every driven session unconditionally, none of
//! them behind a check either, and Claude Code is the person's own install
//! rather than a pinned sidecar (`.claude/rules/agents.md`; unlike `bd`,
//! CLAUDE.md's own section on it) — so an install too old for any one of
//! those already refuses the whole session at spawn, before this task. What
//! was established rather than guessed: `~/.claude/cache/changelog.md`
//! records `--include-partial-messages` as landing at **1.0.109**
//! ("SDK: Added partial message streaming support via
//! `--include-partial-messages` CLI flag"), while every other fact this file
//! and `claude.rs` already carry about the installed CLI — the permission
//! dialog's frame, the `--session-id` flag, the model aliases `--help`
//! documents — was read off builds in the 2.1.1xx-2.1.269 range, over a
//! thousand releases later. An install new enough to run a driven session at
//! all, on the evidence already written into this file before this task
//! touched it, is new enough by a wide margin for this one flag; there is no
//! version this project already assumes that lacks it. A probe would be
//! answering a question this file's own history already settles.
//!
//! **What was actually verified, against what was inferred from it, said
//! precisely because the two were nearly written down as one thing.** The
//! live run above settles the SSE envelope's shape — that
//! `--include-partial-messages` wraps `content_block_delta`/`text_delta`
//! exactly as documented, one JSON object per line — and that much is
//! measured. What this codec's design *rests on*, and what that one capture
//! only ever showed rather than proved, is that Claude Code emits one
//! consolidated `assistant` event per content block, as that block completes,
//! before the next block's deltas begin: the captured trace had exactly two
//! blocks, `thinking` then `text`, in that order, with no tool call and no
//! second text block to say whether the guarantee holds generally or was
//! this reply's coincidence. `content_block_delta` carries the block's own
//! `index`, and this driver reads only `delta.text` off it — the index is
//! decoded and thrown away, and neither this driver nor `journal.js`'s fold
//! knows which block a delta belongs to. If the premise is ever false —
//! two text blocks whose deltas interleave, or a consolidated event arriving
//! late relative to the next block's own deltas — the visible failure is a
//! stitched row reading the two blocks concatenated ("onetwo"), which the
//! closing `Text` then shrinks back down to one block's own content while
//! pushing a second row for the other: a reflow at exactly the moment the
//! acceptance criteria forbid one. A `[thinking, text]` turn misordered the
//! same way would draw the `Reasoning` row *below* the reply it was supposed
//! to precede. Nothing in this file or in `journal.js` guards against that;
//! the guard, if the premise ever needs one, is carrying `index` on
//! `TextDelta` and starting a fresh stitched row whenever it changes.
//!
//! **Stop, and what it costs the child (smetana-y7mv).** Claude Code's headless
//! docs say "To end the turn instead, send SIGINT" — and that was measured
//! against the installed CLI (2.1.270) rather than trusted, with
//! `claude -p --input-format stream-json --output-format stream-json --verbose
//! --include-partial-messages` run three ways against a turn in flight.
//!
//! SIGINT to the process: the CLI writes one `result` event,
//! `{"type":"result","subtype":"error_during_execution","is_error":true}`,
//! and **exits** (code 0). There is no process left for a second message to
//! reach, so this is not what `interrupt` below sends.
//!
//! A `control_request` of `subtype: "interrupt"` written to stdin — what the
//! official Agent SDK's own `interrupt()` sends, and not documented for a bare
//! CLI session — was measured next: the CLI answers a `control_response`,
//! closes the turn on the same `error_during_execution` result SIGINT
//! produces, and **stays alive**, answering the next message normally in the
//! same process. Captured verbatim:
//!
//! ```text
//! → {"type":"control_request","request_id":"req-1","request":{"subtype":"interrupt"}}
//! ← {"type":"control_response","response":{"subtype":"success","request_id":"req-1","response":{"still_queued":[]}}}
//! ← {"type":"user","message":{"role":"user","content":[{"type":"text","text":"[Request interrupted by user]"}]},"parent_tool_use_id":null,"session_id":"…","uuid":"…","timestamp":"…"}
//! ← {"type":"result","subtype":"error_during_execution","is_error":true,"num_turns":2,…}
//! → {"type":"user","message":{"role":"user","content":"Reply with exactly the word PONG."}}
//! ← {"type":"system","subtype":"init",…}
//! ← {"type":"assistant",…"PONG"…}
//! ← {"type":"result","subtype":"success","is_error":false,"result":"PONG","num_turns":1,…}
//! ```
//!
//! A third, uninterrupted control run showed `system`/`init` printing on
//! **every** turn, not only the first — so the second `init` above is this
//! harness's ordinary behaviour, not an artefact of interrupting.
//!
//! `interrupt` below writes exactly that one `control_request` line, save for
//! `request_id`: the capture's `"req-1"` was this measurement's own, and the
//! driver writes `smetana-interrupt-<n>` instead, unique per call rather than
//! per line captured. Nothing else in this file had to change for it:
//! `"result"` already becomes `EventKind::Result` in `one_event` regardless of
//! `subtype` or `is_error`, so `error_during_execution` closes the turn the
//! same way `success` does, and `control_response` is a `type` this codec has
//! never been told about, which already produces nothing. The interrupted turn's own
//! `"[Request interrupted by user]"` text arrives as a `user` message, and this
//! codec already reads a `user` message only for its `tool_result` blocks — the
//! sentence is discarded on purpose and is not drawn anywhere.
//!
//! **Not measured, but predictable from what the rest of this tree already
//! settles: Stop pressed while the session sits on a permission card.**
//! Nobody ran that scenario for this task — it wants the desktop app itself
//! and a live approval dialog on screen, not the stdin/stdout harness the
//! three runs above used — so what follows is a prediction to be confirmed
//! or refuted, never a fourth measurement. The composer draws Stop over a
//! permission card in the first place, since `BUSY = ['running',
//! 'needs-you']` (`journal.js`) never blocks it; `state_of`
//! (`session::model`) clears a card's `pending` entry only on
//! `EventKind::PermissionAnswered`, which the `Answer` request path in
//! `service.rs` appends before it ever touches the listener; and
//! `permission.rs` drops an unmatched `notifications/cancelled` with a bare
//! `202` rather than acting on it. Put together, the likely shape is that
//! the card and its `needs-you` state survive an interrupt untouched, and
//! the person still has to press the card's own Allow or Deny; if so, the
//! one visible cost is that answering the now-stale card may still reach
//! `session_answer` and draw one spurious error toast, since the turn it was
//! asked on has already closed underneath it. Try it by hand in
//! `npm run tauri dev` before trusting any of this.

use std::io::Write;
use std::path::{Path, PathBuf};

use serde_json::Value;

use super::claude::{clip, one_line, tool_detail, Claude, MAX_DETAIL};
use crate::agents::{Intent, Launch};
use crate::session::driver::{Driver, Input, LineBuffer};
use crate::session::model::{Actor, Decision, EventKind};
use crate::session::permission::{permission_tool, PermissionTicket};

pub struct ClaudeDriver {
    lines: LineBuffer,
    /// The `--mcp-config` file naming this session's permission listener, or
    /// `None` for a session that has no ticket — a codec test, and a session
    /// started on a machine where the file could not be written.
    permission: Option<McpConfig>,
    /// How many times `interrupt` has written a `control_request` for this
    /// driver, which is folded into that line's own `request_id` so two
    /// interrupts in the same session never repeat one. Uniqueness within
    /// this process is all the CLI was measured to need.
    interrupts: u64,
}

/// The config file handed to the child, alive for as long as the driver is.
///
/// A temporary deleted on the way out of `start` would be gone before the child
/// had opened it, and the failure is silent: the harness would simply never
/// connect to the server and never ask about anything.
struct McpConfig {
    path: PathBuf,
}

impl Drop for McpConfig {
    fn drop(&mut self) {
        // The session is over, so the child that was reading this is too. The
        // file holds the session's bearer token, and leaving it in a shared
        // temporary directory for the life of the machine is the one thing
        // worth doing something about here.
        let _ = std::fs::remove_file(&self.path);
    }
}

/// A path nothing else is going to pick: this process, a counter within it, and
/// the clock. The same shape the rest of this tree uses for scratch state
/// (`smetana-git-…`, `smetana-shell-…`), with the clock added because a config
/// file outlives nothing and a recycled pid must not meet a stale one.
fn config_path() -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(1);
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|since| since.as_nanos())
        .unwrap_or(0);
    std::env::temp_dir().join(format!("smetana-mcp-{}-{n}-{nanos}.json", std::process::id()))
}

/// Create the file readable by nobody but this user, and refuse a path that is
/// already there.
///
/// Both halves are about the same thing and neither is ceremony: what goes in
/// here is the bearer token for a socket that answers questions about running
/// `rm -rf`, and the directory it goes in is shared with every other user of the
/// machine. `create_new` is what stops another process pre-planting a symlink at
/// the path; the mode is what stops it simply reading the file afterwards.
fn create_private(path: &Path) -> std::io::Result<std::fs::File> {
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)
}

fn write_mcp_config(ticket: &PermissionTicket) -> Option<McpConfig> {
    let path = config_path();
    let body = serde_json::to_vec(&ticket.mcp_config()).ok()?;
    match create_private(&path).and_then(|mut file| file.write_all(&body)) {
        Ok(()) => Some(McpConfig { path }),
        Err(error) => {
            // Loud, and then the session goes on without a permission channel:
            // the harness declines what it would have asked about, which is a
            // session that gets less done rather than one that does something
            // nobody agreed to.
            log::error!("the permission config could not be written to {}: {error}", path.display());
            None
        }
    }
}

impl ClaudeDriver {
    /// `None` for the ticket is a driver with no permission channel: the codec
    /// tests, which exercise the stream and never spawn anything.
    pub fn new(permission: Option<PermissionTicket>) -> Self {
        Self {
            lines: LineBuffer::new(),
            permission: permission.as_ref().and_then(write_mcp_config),
            interrupts: 0,
        }
    }
}

impl Default for ClaudeDriver {
    fn default() -> Self {
        Self::new(None)
    }
}

/// Control bytes go; everything else, newlines included, stays. The one place
/// this file differs from `claude::one_line`, and the difference is the whole
/// reason it is a separate function: a pane row is a row, a paragraph is not.
fn prose(text: &str) -> String {
    text.chars().filter(|c| !c.is_control() || *c == '\n').collect()
}

fn str_at<'a>(value: &'a Value, key: &str) -> &'a str {
    value.get(key).and_then(Value::as_str).unwrap_or("")
}

fn u64_at(value: &Value, pointer: &str) -> u64 {
    value.pointer(pointer).and_then(Value::as_u64).unwrap_or(0)
}

/// How many lines of prose, as the one thing a result event says.
fn lines_summary(lines: usize) -> String {
    match lines {
        0 => "no output".to_string(),
        1 => "1 line".to_string(),
        many => format!("{many} lines"),
    }
}

/// How a tool's result went, and never what it said. A result routinely carries
/// a whole file, and that wall of output is what this design exists to remove.
///
/// The shape being read is `string | ContentBlock[]`, and the array form is
/// ordinary rather than exotic — a `Read` of an image produces one, and it is
/// roughly a fifth of the tool results in a real transcript. A version reading
/// only the string form calls every one of them "no output", which is worse
/// than a missing row: the summary is the whole of what this event carries,
/// since the content is deliberately thrown away, so there is nothing beside it
/// to correct the impression.
///
/// A block that is not prose has no lines to count, so it is said in its own
/// word. Folding it into the line count would be wrong about both halves.
fn result_summary(content: Option<&Value>) -> String {
    let Some(content) = content else {
        return lines_summary(0);
    };
    if let Some(text) = content.as_str() {
        return lines_summary(text.lines().count());
    }
    let Some(blocks) = content.as_array() else {
        return lines_summary(0);
    };
    let mut text = String::new();
    let mut others: Vec<(&str, usize)> = Vec::new();
    for block in blocks {
        match str_at(block, "type") {
            "text" => {
                if !text.is_empty() {
                    text.push('\n');
                }
                text.push_str(str_at(block, "text"));
            }
            kind => {
                // Its own type is the word, so a block this build has never
                // heard of is still counted and named rather than dropped.
                let kind = if kind.is_empty() { "block" } else { kind };
                match others.iter_mut().find(|(name, _)| *name == kind) {
                    Some((_, count)) => *count += 1,
                    None => others.push((kind, 1)),
                }
            }
        }
    }
    let mut parts = Vec::new();
    if !text.is_empty() {
        parts.push(lines_summary(text.lines().count()));
    }
    for (kind, count) in others {
        parts.push(format!("{count} {kind}{}", if count == 1 { "" } else { "s" }));
    }
    if parts.is_empty() {
        lines_summary(0)
    } else {
        parts.join(", ")
    }
}

/// One decoded line of the stream, as events.
///
/// Zero events is an ordinary answer and the commonest one: hook chatter, a
/// content block this build has never heard of, and every event type outside
/// the match below. That is the choice `claude::transcript_line` already made,
/// for the reason it records — a missing row costs a person nothing the CLI's
/// own logs do not still hold, while a wall of raw protocol costs them the
/// panel.
///
/// **`pub(crate)` for one caller outside the codec: `session::history`.** A
/// record in a `.jsonl` transcript and a line of this harness's stream-json
/// output are the same object — that is what makes `--resume` work over a
/// session recorded either way — so the history a person scrolls back through
/// is decoded by this very function rather than by a second reading of the same
/// format. Two readings would drift, and the drift would show as rows quietly
/// ceasing to appear.
pub(crate) fn one_event(event: &Value) -> Vec<EventKind> {
    match str_at(event, "type") {
        "system" => match str_at(event, "subtype") {
            "init" => vec![EventKind::TurnStart { by: Actor::Agent }],
            "api_retry" => {
                let error = clip(&one_line(str_at(event, "error")), MAX_DETAIL);
                let error = if error.is_empty() { "error".to_string() } else { error };
                let attempt = event.get("attempt").and_then(Value::as_i64).unwrap_or(0);
                vec![EventKind::Error { text: format!("api retry ({error}), attempt {attempt}") }]
            }
            _ => Vec::new(),
        },
        "assistant" => event
            .pointer("/message/content")
            .and_then(Value::as_array)
            .map(|blocks| {
                blocks
                    .iter()
                    .filter_map(|block| match str_at(block, "type") {
                        "text" => {
                            let text = prose(str_at(block, "text"));
                            (!text.trim().is_empty()).then_some(EventKind::Text { text })
                        }
                        "thinking" => {
                            // The emptiness guard is load-bearing rather than
                            // tidiness, and a refactor that drops it will not
                            // fail anywhere else: an extended-thinking block
                            // routinely arrives as `{"thinking":"","signature":
                            // "..."}`, where the signature is the whole of the
                            // payload and there is no reasoning to show. Without
                            // it every one of those draws an empty row.
                            let text = prose(str_at(block, "thinking"));
                            (!text.trim().is_empty()).then_some(EventKind::Reasoning { text })
                        }
                        "tool_use" => Some(EventKind::ToolUse {
                            id: str_at(block, "id").to_string(),
                            name: str_at(block, "name").to_string(),
                            detail: tool_detail(
                                str_at(block, "name"),
                                block.get("input").unwrap_or(&Value::Null),
                            ),
                        }),
                        _ => None,
                    })
                    .collect()
            })
            .unwrap_or_default(),
        // A tool's result arrives as a user message, which is the protocol's
        // shape rather than ours: nobody typed it.
        "user" => event
            .pointer("/message/content")
            .and_then(Value::as_array)
            .map(|blocks| {
                blocks
                    .iter()
                    .filter(|block| str_at(block, "type") == "tool_result")
                    .map(|block| EventKind::ToolResult {
                        id: str_at(block, "tool_use_id").to_string(),
                        ok: !block.get("is_error").and_then(Value::as_bool).unwrap_or(false),
                        summary: result_summary(block.get("content")),
                    })
                    .collect()
            })
            .unwrap_or_default(),
        "result" => vec![EventKind::Result {
            // Every input token the turn was billed for, cached ones included,
            // and this **deliberately diverges from `claude::transcript_line`**,
            // which reads `input_tokens` alone. That convention was set for a
            // cosmetic pane row, where being low is a cosmetic problem; this is
            // structured data the front end renders as what the turn cost. A
            // real turn reports 19 uncached input tokens beside 43 336 read from
            // the cache and 10 000 written to it, so the unsummed figure is
            // three orders of magnitude out — and a number that wrong reads as a
            // working feature rather than as a stale one.
            tokens_in: u64_at(event, "/usage/input_tokens")
                + u64_at(event, "/usage/cache_read_input_tokens")
                + u64_at(event, "/usage/cache_creation_input_tokens"),
            tokens_out: u64_at(event, "/usage/output_tokens"),
            cost_usd: event.get("total_cost_usd").and_then(Value::as_f64),
            ms: event.get("duration_ms").and_then(Value::as_u64).unwrap_or(0),
        }],
        // `--include-partial-messages`'s own wrapper — see this file's header
        // for where the shape was read. Only a text delta produces anything;
        // every other nested `event.type` (`message_start`,
        // `content_block_start`/`content_block_stop` whichever block they
        // name, `thinking_delta`, `signature_delta`, `input_json_delta`,
        // `message_delta`, `message_stop`) is out of scope for this task or
        // carries nothing the front end does not already get from the
        // consolidated `assistant` event above, and produces nothing, on this
        // function's own standing rule for a shape it has not been told about.
        "stream_event" => match event.pointer("/event/type").and_then(Value::as_str) {
            Some("content_block_delta") => {
                match event.pointer("/event/delta/type").and_then(Value::as_str) {
                    Some("text_delta") => {
                        let raw =
                            event.pointer("/event/delta/text").and_then(Value::as_str).unwrap_or("");
                        let text = prose(raw);
                        // Never `text.trim().is_empty()`, the guard `Text`
                        // itself uses above: a delta that is a lone space or
                        // newline between two words is real content, and
                        // trimming it away would glue two words together on
                        // the wire with nothing to say so.
                        if text.is_empty() { Vec::new() } else { vec![EventKind::TextDelta { text }] }
                    }
                    _ => Vec::new(),
                }
            }
            _ => Vec::new(),
        },
        _ => Vec::new(),
    }
}

impl Driver for ClaudeDriver {
    fn start(&self, launch: &Launch) -> portable_pty::CommandBuilder {
        // The profile still builds the line: the plugins, the skills directory,
        // the model flag, `--session-id` and the rule that it never stands
        // beside `--resume` are all its knowledge, and none of it is written
        // twice here. What this adds is the shape of the conversation, which is
        // the driver's own business.
        //
        // `command_without_prompt` rather than `command`, and that is the whole
        // reason `claude.rs` has a seam in it. Under `--input-format
        // stream-json` this harness reads the turn off stdin and **throws the
        // positional argument away** — measured against 2.1.267, where a
        // positional and a stdin turn saying different things produced only the
        // stdin answer. `command`'s positional is the prompt, so a driven
        // session built on it opened with the conversation-language paragraph
        // reaching nothing at all.
        let mut cmd = Claude.command_without_prompt(launch);
        // Both halves of the stream are required together: `--input-format
        // stream-json` is refused without `--output-format stream-json`, and
        // `--verbose` is what makes the output one event per line rather than
        // one blob at the end. `--include-partial-messages` is the fourth —
        // documented on the installed CLI as needing `--print` and
        // `--output-format stream-json`, both already here — and is what
        // this file's own header describes turning into `EventKind::TextDelta`.
        //
        // Appended rather than put in front the way `batch_args` is.
        // `CommandBuilder` can only be pushed to, so leading the line would mean
        // rebuilding one out of `get_argv` and carrying `cwd` and the extra
        // environment across by hand — a real risk of dropping something, to buy
        // tidiness the parser does not care about: the same probe showed both
        // orderings parse identically.
        for arg in [
            "-p",
            "--verbose",
            "--output-format",
            "stream-json",
            "--input-format",
            "stream-json",
            "--include-partial-messages",
        ] {
            cmd.arg(arg);
        }
        // Where this session asks about permissions. Both flags or neither:
        // `--permission-prompt-tool` names a tool on a server that only
        // `--mcp-config` puts there, so a line carrying one of them alone is a
        // harness that fails every tool call it would have asked about.
        //
        // The file is **not** deleted here. It is held by `McpConfig` for the
        // driver's whole life, because the child opens it some way into its own
        // start and a temporary released at the end of this function would be
        // gone by then.
        if let Some(config) = &self.permission {
            cmd.arg("--mcp-config");
            cmd.arg(&config.path);
            cmd.arg("--permission-prompt-tool");
            cmd.arg(permission_tool());
        }
        // The standing instruction, on the one channel this mode leaves open
        // for one — and **only for a bare session**. A bare launch's whole
        // prompt is the conversation-language sentence, which is exactly what
        // a system-prompt clause is for. Every other intent carries a brief,
        // and a brief is a turn: it goes over stdin through `opening` below,
        // so that the model reads it as what somebody asked rather than as an
        // appended rule. The probe that established this flag found that its
        // text reaches the model; it did not establish that it outranks
        // anything, and nothing here relies on that.
        //
        // This decides the question the file's header used to leave open —
        // "whoever first drives a session on another intent should stop at
        // this line and decide it": `Bare` keeps the system prompt,
        // everything else opens on `opening`, and a resume gets neither,
        // since `prompt::build` refuses that intent a prompt at all and an
        // empty `--append-system-prompt` would be this app talking over
        // somebody's words in a quieter voice rather than not talking over
        // them.
        if matches!(launch.intent, Intent::Bare) {
            if let Some(text) = Claude.prompt_text(launch) {
                cmd.arg("--append-system-prompt");
                cmd.arg(text);
            }
        }
        cmd
    }

    fn opening(&self, launch: &Launch) -> Option<String> {
        // Everything but the two that open on nothing over stdin: a bare
        // session's prompt is on the system prompt above, and a resume has
        // none. `prompt_text` already answers `None` for the resume; the bare
        // case is the one decided here.
        if matches!(launch.intent, Intent::Bare) {
            return None;
        }
        Claude.prompt_text(launch)
    }

    fn feed(&mut self, bytes: &[u8]) -> Vec<EventKind> {
        // A line that does not parse as JSON produces nothing rather than an
        // error row: a harness printing a warning to stdout must not put a red
        // row in somebody's conversation.
        self.lines
            .feed(bytes)
            .iter()
            .filter_map(|line| serde_json::from_str::<Value>(line).ok())
            .flat_map(|event| one_event(&event))
            .collect()
    }

    fn send(&mut self, input: Input) -> Vec<u8> {
        let Input::Message { text, attachments } = input;
        // A path named in the prose is the one channel every harness has, and
        // it is what `ImageDelivery::InPrompt` — the delivery this profile
        // keeps — already means elsewhere.
        let body = if attachments.is_empty() {
            text
        } else {
            format!("{text}\n\n{}", attachments.join("\n"))
        };
        let message = serde_json::json!({
            "type": "user",
            "message": { "role": "user", "content": body },
        });
        let mut bytes = serde_json::to_vec(&message).unwrap_or_default();
        bytes.push(b'\n');
        bytes
    }

    fn answer(&mut self, _id: &str, _decision: Decision) -> Option<Vec<u8>> {
        // This harness answers through the permission listener, not over stdin,
        // so there are no bytes for the worker to write here.
        None
    }

    fn interrupt(&mut self) -> Option<Vec<u8>> {
        // Measured against the installed CLI (2.1.270) rather than guessed —
        // see this file's own header for the run. A `control_request` of
        // `subtype: "interrupt"` closes the open turn and leaves the child
        // alive to answer the next message; `request_id` only has to be
        // unique within this driver, which the counter gives it.
        self.interrupts += 1;
        let message = serde_json::json!({
            "type": "control_request",
            "request_id": format!("smetana-interrupt-{}", self.interrupts),
            "request": { "subtype": "interrupt" },
        });
        let mut bytes = serde_json::to_vec(&message)
            .expect("a literal control_request of two string fields always serializes");
        bytes.push(b'\n');
        Some(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::{library::Skills, Intent, Languages};
    use std::path::PathBuf;

    /// Lines as Claude Code 2.1 writes them under `--output-format stream-json`.
    /// Captured rather than invented: an invented fixture tests the fixture.
    const INIT: &str =
        r#"{"type":"system","subtype":"init","model":"claude-opus-5","session_id":"abc"}"#;
    const TEXT: &str = r#"{"type":"assistant","message":{"content":[{"type":"text","text":"Reading the file now."}]}}"#;
    /// One `content_block_delta` under `--include-partial-messages`, captured
    /// against the installed CLI (2.1.269) rather than invented — see this
    /// file's own header for the run that produced it.
    const TEXT_DELTA: &str =
        r#"{"type":"stream_event","event":{"type":"content_block_delta","index":1,"delta":{"type":"text_delta","text":"hi"}}}"#;
    /// The neighbouring `thinking_delta`, out of scope for this task — only
    /// the reply's own text streams, never the agent's reasoning.
    const THINKING_DELTA: &str =
        r#"{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":"weighing it"}}}"#;
    /// A tool call's arguments streaming in, also out of scope.
    const INPUT_JSON_DELTA: &str =
        r#"{"type":"stream_event","event":{"type":"content_block_delta","index":2,"delta":{"type":"input_json_delta","partial_json":"{\"path\""}}}"#;
    const MESSAGE_START: &str =
        r#"{"type":"stream_event","event":{"type":"message_start","message":{"id":"msg_1"}}}"#;
    const CONTENT_BLOCK_START_TEXT: &str = r#"{"type":"stream_event","event":{"type":"content_block_start","index":1,"content_block":{"type":"text","text":""}}}"#;
    const CONTENT_BLOCK_STOP: &str =
        r#"{"type":"stream_event","event":{"type":"content_block_stop","index":1}}"#;
    const MESSAGE_STOP: &str = r#"{"type":"stream_event","event":{"type":"message_stop"}}"#;
    const TOOL: &str = r#"{"type":"assistant","message":{"content":[{"type":"tool_use","id":"t1","name":"Bash","input":{"command":"cargo test"}}]}}"#;
    const TOOL_RESULT: &str = r#"{"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"t1","is_error":false,"content":"ok\nok\nok"}]}}"#;
    const RESULT: &str = r#"{"type":"result","subtype":"success","duration_ms":4200,"total_cost_usd":0.031,"usage":{"input_tokens":120,"output_tokens":40}}"#;
    const RETRY: &str = r#"{"type":"system","subtype":"api_retry","error":"overloaded","attempt":2}"#;
    /// What Stop actually closes a turn with (smetana-y7mv) — captured against
    /// the installed CLI (2.1.270) rather than invented, see this file's own
    /// header for the run. `error_during_execution` is not `Error`: it is
    /// still a `result` event, and the turn closes exactly as a successful one
    /// does.
    const RESULT_INTERRUPTED: &str = r#"{"type":"result","subtype":"error_during_execution","is_error":true,"duration_ms":900,"usage":{"input_tokens":5,"output_tokens":1}}"#;
    /// The CLI's own acknowledgement of the `control_request` `interrupt`
    /// sends. A shape this codec has never been told about, so it produces
    /// nothing — the same rule as any other unrecognised `type`.
    const CONTROL_RESPONSE: &str = r#"{"type":"control_response","response":{"subtype":"success","request_id":"req-1","response":{"still_queued":[]}}}"#;
    /// The other half of `content`'s `string | ContentBlock[]`, which the string
    /// fixture above does not reach: a list of blocks, with and without prose
    /// in it. The image block is a `Read` of a PNG, as that tool really answers.
    const TOOL_RESULT_BLOCKS: &str = r#"{"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"t2","is_error":false,"content":[{"type":"text","text":"one\ntwo"}]}]}}"#;
    const TOOL_RESULT_IMAGE: &str = r#"{"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"t3","is_error":false,"content":[{"type":"image","source":{"type":"base64","media_type":"image/png","data":"iVBORw0KGgo="}}]}]}}"#;
    const TOOL_RESULT_MIXED: &str = r#"{"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"t4","is_error":false,"content":[{"type":"text","text":"one\ntwo"},{"type":"image","source":{"type":"base64","media_type":"image/png","data":"iVBORw0KGgo="}}]}]}}"#;
    const THINKING: &str = r#"{"type":"assistant","message":{"content":[{"type":"thinking","thinking":"The file is read before it is written.","signature":"EqQBCkYIBRgC"}]}}"#;
    /// An extended-thinking block whose whole payload is its signature. Roughly
    /// every turn produces one.
    const THINKING_SIGNED_ONLY: &str = r#"{"type":"assistant","message":{"content":[{"type":"thinking","thinking":"","signature":"EqQBCkYIBRgCKkBd"}]}}"#;
    /// A turn that mostly hit the cache, which is the ordinary case rather than
    /// the exception once a session has any length to it.
    const RESULT_CACHED: &str = r#"{"type":"result","subtype":"success","duration_ms":4200,"total_cost_usd":0.031,"usage":{"input_tokens":19,"cache_read_input_tokens":43336,"cache_creation_input_tokens":10000,"output_tokens":40}}"#;

    const CHOSEN: &str = "11111111-2222-3333-4444-555555555555";

    fn events(driver: &mut ClaudeDriver, line: &str) -> Vec<EventKind> {
        driver.feed(format!("{line}\n").as_bytes())
    }

    /// A `Bare` launch carrying the conversation id this app chose, which is
    /// what the worker hands a driver for an ordinary session.
    fn launch() -> Launch {
        Launch {
            profile: &Claude,
            cwd: PathBuf::from("/tmp/project"),
            intent: Intent::Bare,
            skills: Skills {
                smetana: PathBuf::from("/app/resources/smetana"),
                superpowers: PathBuf::from("/app/resources/superpowers"),
                superpowers_installed: true,
            },
            facts: None,
            session_id: Some(CHOSEN.to_owned()),
            languages: Languages::default(),
            agent_prompt: String::new(),
            model: None,
            worker_model: None,
        }
    }

    /// The one intent that opens on no prompt at all.
    fn resume_launch() -> Launch {
        Launch {
            intent: Intent::ResumeSession {
                id: "9f1c0a2e-0000-4000-8000-000000000000".into(),
                cwd: "/tmp/project".into(),
                title: None,
                fork: false,
            },
            // A resume is never told a chosen id either: `--resume` already
            // carries the conversation's own.
            session_id: None,
            ..launch()
        }
    }

    /// A launch that carries a brief rather than a standing instruction.
    fn new_task_launch() -> Launch {
        Launch {
            intent: Intent::NewTask {
                brainstorm: crate::agents::Stage::Off,
                spec: crate::agents::Stage::Off,
                plan: crate::agents::Stage::Off,
                draft: crate::agents::TaskDraft {
                    text: "Make the bell ring once".into(),
                    issue_type: None,
                    priority: None,
                    parent: None,
                    images: vec!["/tmp/shot.png".into()],
                },
            },
            ..launch()
        }
    }

    fn argv(builder: &portable_pty::CommandBuilder) -> Vec<String> {
        builder.get_argv().iter().map(|arg| arg.to_string_lossy().to_string()).collect()
    }

    #[test]
    fn only_a_bare_launch_puts_its_prompt_on_the_system_prompt() {
        let bare = argv(&ClaudeDriver::new(None).start(&launch()));
        assert!(bare.iter().any(|arg| arg == "--append-system-prompt"), "{bare:?}");

        let brief = argv(&ClaudeDriver::new(None).start(&new_task_launch()));
        assert!(!brief.iter().any(|arg| arg == "--append-system-prompt"), "{brief:?}");
    }

    #[test]
    fn a_brief_opens_the_session_over_stdin_and_a_bare_launch_does_not() {
        let driver = ClaudeDriver::new(None);
        assert_eq!(driver.opening(&launch()), None);
        assert_eq!(driver.opening(&resume_launch()), None);

        let opening = driver.opening(&new_task_launch()).expect("a brief opens on its prompt");
        // Byte for byte what the PTY road would have handed over positionally.
        assert_eq!(Some(opening.clone()), Claude.prompt_text(&new_task_launch()));
        assert!(opening.contains("Make the bell ring once"), "{opening}");
    }

    #[test]
    fn an_init_event_opens_the_agents_turn() {
        let mut driver = ClaudeDriver::new(None);
        assert_eq!(events(&mut driver, INIT), vec![EventKind::TurnStart { by: Actor::Agent }]);
    }

    #[test]
    fn a_text_block_becomes_one_paragraph_with_its_markdown_intact() {
        // Unlike the terminal's renderer, newlines are kept: the front end
        // parses markdown, and flattening here would destroy every list.
        let mut driver = ClaudeDriver::new(None);
        assert_eq!(
            events(&mut driver, TEXT),
            vec![EventKind::Text { text: "Reading the file now.".into() }]
        );
    }

    #[test]
    fn a_text_delta_carries_only_its_own_incremental_piece() {
        // Never accumulated here — `journal.js` is what stitches a run of
        // these together, and `TextDelta`'s own doc comment says why.
        let mut driver = ClaudeDriver::new(None);
        assert_eq!(events(&mut driver, TEXT_DELTA), vec![EventKind::TextDelta { text: "hi".into() }]);
    }

    #[test]
    fn a_delta_of_a_lone_space_is_kept_rather_than_trimmed_away() {
        // The guard this pins is deliberately `text.is_empty()` and not
        // `text.trim().is_empty()`: a chunk that is only a space or a newline
        // between two words is real content on the wire, and the `Text` guard
        // one arm up would silently glue two words together if it were reused
        // here unchanged.
        let mut driver = ClaudeDriver::new(None);
        let line = r#"{"type":"stream_event","event":{"type":"content_block_delta","index":1,"delta":{"type":"text_delta","text":" "}}}"#;
        assert_eq!(events(&mut driver, line), vec![EventKind::TextDelta { text: " ".into() }]);
    }

    #[test]
    fn an_empty_text_delta_draws_no_row() {
        let mut driver = ClaudeDriver::new(None);
        let line = r#"{"type":"stream_event","event":{"type":"content_block_delta","index":1,"delta":{"type":"text_delta","text":""}}}"#;
        assert!(events(&mut driver, line).is_empty());
    }

    #[test]
    fn reasoning_and_tool_input_streaming_in_are_out_of_scope_and_draw_nothing() {
        // Only the reply's own text streams — smetana-6we6's own boundary.
        // Reasoning and a tool call's arguments still arrive whole, from the
        // consolidated `assistant` event, exactly as before this task.
        let mut driver = ClaudeDriver::new(None);
        assert!(events(&mut driver, THINKING_DELTA).is_empty());
        assert!(events(&mut driver, INPUT_JSON_DELTA).is_empty());
    }

    #[test]
    fn the_rest_of_the_partial_message_envelope_draws_nothing() {
        // `message_start`, a block opening or closing, and `message_stop` are
        // structure this driver does not need: the block's own consolidated
        // `assistant` event is what closes a streamed reply, not any of these.
        let mut driver = ClaudeDriver::new(None);
        for line in [MESSAGE_START, CONTENT_BLOCK_START_TEXT, CONTENT_BLOCK_STOP, MESSAGE_STOP] {
            assert!(events(&mut driver, line).is_empty(), "{line} produced an event");
        }
    }

    #[test]
    fn a_streamed_reply_still_ends_on_the_same_whole_text_event_as_before() {
        // The wire decision this task made: deltas are additive, and the
        // existing consolidated `assistant` event — unchanged — is still what
        // a reader relies on for the authoritative, final copy of the reply.
        let mut driver = ClaudeDriver::new(None);
        let mut all = events(&mut driver, TEXT_DELTA);
        all.extend(events(&mut driver, TEXT));
        assert_eq!(
            all,
            vec![
                EventKind::TextDelta { text: "hi".into() },
                EventKind::Text { text: "Reading the file now.".into() }
            ]
        );
    }

    #[test]
    fn a_tool_call_carries_the_one_line_the_reference_formatter_shows() {
        let mut driver = ClaudeDriver::new(None);
        assert_eq!(
            events(&mut driver, TOOL),
            vec![EventKind::ToolUse {
                id: "t1".into(),
                name: "Bash".into(),
                detail: "cargo test".into()
            }]
        );
    }

    #[test]
    fn a_tool_result_says_how_it_went_and_never_what_it_said() {
        // A result routinely carries a whole file. The panel wants the outcome;
        // the content would be the wall of output this design exists to remove.
        let mut driver = ClaudeDriver::new(None);
        assert_eq!(
            events(&mut driver, TOOL_RESULT),
            vec![EventKind::ToolResult { id: "t1".into(), ok: true, summary: "3 lines".into() }]
        );
    }

    #[test]
    fn a_tool_result_whose_content_is_a_list_of_blocks_counts_its_prose() {
        // `content` is `string | ContentBlock[]` and the list form is ordinary:
        // reading only the string form reported "no output" for about a fifth
        // of the tool results in a real transcript.
        let mut driver = ClaudeDriver::new(None);
        assert_eq!(
            events(&mut driver, TOOL_RESULT_BLOCKS),
            vec![EventKind::ToolResult { id: "t2".into(), ok: true, summary: "2 lines".into() }]
        );
    }

    #[test]
    fn a_result_carrying_no_prose_is_named_rather_than_called_empty() {
        // A `Read` of a PNG answers with one image block and no text at all.
        // "no output" would be false, and the summary is the whole of what this
        // event says.
        let mut driver = ClaudeDriver::new(None);
        assert_eq!(
            events(&mut driver, TOOL_RESULT_IMAGE),
            vec![EventKind::ToolResult { id: "t3".into(), ok: true, summary: "1 image".into() }]
        );
    }

    #[test]
    fn prose_and_a_picture_in_one_result_are_counted_apart() {
        // A block that is not prose has no lines, so folding it into the line
        // count would be wrong about both halves.
        let mut driver = ClaudeDriver::new(None);
        assert_eq!(
            events(&mut driver, TOOL_RESULT_MIXED),
            vec![EventKind::ToolResult {
                id: "t4".into(),
                ok: true,
                summary: "2 lines, 1 image".into()
            }]
        );
    }

    #[test]
    fn a_thinking_block_is_the_agents_reasoning() {
        let mut driver = ClaudeDriver::new(None);
        assert_eq!(
            events(&mut driver, THINKING),
            vec![EventKind::Reasoning { text: "The file is read before it is written.".into() }]
        );
    }

    #[test]
    fn a_thinking_block_carrying_only_its_signature_draws_no_row() {
        // The guard in the `thinking` arm is what this pins, and nothing else
        // in the suite would notice its removal: such a block arrives on nearly
        // every turn, and each one would draw an empty reasoning row.
        let mut driver = ClaudeDriver::new(None);
        assert!(events(&mut driver, THINKING_SIGNED_ONLY).is_empty());
    }

    #[test]
    fn a_result_closes_the_turn_with_its_tokens_and_its_clock() {
        let mut driver = ClaudeDriver::new(None);
        assert_eq!(
            events(&mut driver, RESULT),
            vec![EventKind::Result {
                tokens_in: 120,
                tokens_out: 40,
                cost_usd: Some(0.031),
                ms: 4200
            }]
        );
    }

    #[test]
    fn the_tokens_a_turn_cost_include_the_ones_it_read_from_the_cache() {
        // Deliberately unlike `claude::transcript_line`, which reads
        // `input_tokens` alone: on a cached turn that field is 19 against the
        // 53 355 the turn actually consumed, and this event is rendered as what
        // the turn cost rather than printed into a pane.
        let mut driver = ClaudeDriver::new(None);
        assert_eq!(
            events(&mut driver, RESULT_CACHED),
            vec![EventKind::Result {
                tokens_in: 53_355,
                tokens_out: 40,
                cost_usd: Some(0.031),
                ms: 4200
            }]
        );
    }

    #[test]
    fn an_api_retry_is_an_error_a_person_should_see() {
        let mut driver = ClaudeDriver::new(None);
        assert_eq!(
            events(&mut driver, RETRY),
            vec![EventKind::Error { text: "api retry (overloaded), attempt 2".into() }]
        );
    }

    #[test]
    fn an_event_type_this_build_has_never_heard_of_produces_nothing() {
        let mut driver = ClaudeDriver::new(None);
        assert!(events(&mut driver, r#"{"type":"something_new","payload":42}"#).is_empty());
    }

    #[test]
    fn a_line_that_is_not_json_produces_nothing_rather_than_an_error_row() {
        // A harness that prints a warning to stdout must not put a red row in
        // somebody's conversation.
        let mut driver = ClaudeDriver::new(None);
        assert!(events(&mut driver, "warning: something").is_empty());
    }

    #[test]
    fn control_bytes_in_the_agents_own_prose_do_not_survive() {
        // Such a string routinely carries the JSON escapes for ESC and BEL, and
        // serde_json decodes those into live bytes. A bell would turn a row
        // needs-you; colour would be escape codes on a panel that is no longer
        // a terminal.
        let mut driver = ClaudeDriver::new(None);
        let line = r#"{"type":"assistant","message":{"content":[{"type":"text","text":"red \u001b[31mhere\u0007"}]}}"#;
        let EventKind::Text { text } = &events(&mut driver, line)[0] else {
            panic!("a text block is a Text event");
        };
        assert!(!text.contains('\u{1b}') && !text.contains('\u{7}'), "{text:?}");
    }

    #[test]
    fn a_newline_inside_a_paragraph_is_kept_because_markdown_needs_it() {
        let mut driver = ClaudeDriver::new(None);
        let line =
            r#"{"type":"assistant","message":{"content":[{"type":"text","text":"- one\n- two"}]}}"#;
        assert_eq!(
            events(&mut driver, line),
            vec![EventKind::Text { text: "- one\n- two".into() }]
        );
    }

    #[test]
    fn the_command_line_asks_for_a_two_way_stream() {
        // Both halves are required together: --input-format stream-json is
        // refused without --output-format stream-json, and --verbose is what
        // makes the output one event per line rather than one blob at the end.
        let args = argv(&ClaudeDriver::new(None).start(&launch()));
        for flag in ["-p", "--verbose"] {
            assert!(args.iter().any(|arg| arg == flag), "{flag} is missing from {args:?}");
        }
        // Each format flag against its own value, rather than both words
        // somewhere on the line: the pairing is the load-bearing half, and a
        // test looking only for the words would pass a line reading
        // `--input-format --output-format stream-json`.
        for flag in ["--input-format", "--output-format"] {
            let at = args.iter().position(|arg| arg == flag).expect("the flag is on the line");
            assert_eq!(args.get(at + 1).map(String::as_str), Some("stream-json"), "{args:?}");
        }
    }

    #[test]
    fn the_command_line_asks_for_partial_message_chunks() {
        // Without this flag Claude Code buffers a whole reply and this file
        // never sees a `content_block_delta` at all — smetana-6we6's whole
        // premise, verified against the installed CLI rather than assumed.
        let args = argv(&ClaudeDriver::new(None).start(&launch()));
        assert!(
            args.iter().any(|arg| arg == "--include-partial-messages"),
            "--include-partial-messages is missing from {args:?}"
        );
    }

    #[test]
    fn the_conversation_id_this_app_chose_is_on_the_line() {
        let args = argv(&ClaudeDriver::new(None).start(&launch()));
        let at =
            args.iter().position(|arg| arg == "--session-id").expect("the flag is on the line");
        assert_eq!(args[at + 1], CHOSEN);
    }

    #[test]
    fn the_opening_prompt_reaches_the_agent_as_an_appended_system_prompt() {
        // The channel, and the exact text: under `--input-format stream-json`
        // the positional argument this text used to ride on is discarded by the
        // harness, so asserting merely that something non-empty is on the line
        // would have passed throughout the defect.
        let launch = launch();
        let text = Claude.prompt_text(&launch).expect("a bare launch opens on a prompt");
        let args = argv(&ClaudeDriver::new(None).start(&launch));
        let at = args
            .iter()
            .position(|arg| arg == "--append-system-prompt")
            .expect("the flag is on the line");
        assert_eq!(args.get(at + 1), Some(&text), "{args:?}");
        // Once on the whole line, and there. Unlike the positional walk in the
        // test below, this statement has no false-negative window, and it is
        // what catches the shape nobody has had to think about yet: the text
        // behind the flag *and* still trailing as a positional, which is what a
        // careless merge of this seam with `command`'s old body produces.
        let carried: Vec<usize> = args
            .iter()
            .enumerate()
            .filter(|(_, arg)| *arg == &text)
            .map(|(at, _)| at)
            .collect();
        assert_eq!(carried, vec![at + 1], "{args:?}");
    }

    #[test]
    fn no_argument_on_the_line_is_a_positional() {
        // The regression test for the whole defect. Every argument after the
        // program is either a flag or the value directly behind one, so a
        // prompt appended as a positional — which is what `Profile::command`
        // produces and what this harness throws away — puts a bare argument on
        // the line and fails here.
        let args = argv(&ClaudeDriver::new(None).start(&launch()));
        for (at, arg) in args.iter().enumerate().skip(1) {
            if arg.starts_with('-') {
                continue;
            }
            let flag = &args[at - 1];
            assert!(
                flag.starts_with('-'),
                "{arg:?} follows {flag:?} rather than a flag, so nothing carries it: {args:?}"
            );
        }
    }

    #[test]
    fn a_resumed_conversation_is_told_nothing_at_all() {
        // `prompt::build` refuses `Intent::ResumeSession` a prompt, and the
        // refusal is load-bearing: somebody's words are already in that
        // conversation and the app must not talk over them. An empty flag would
        // be doing it quietly rather than not doing it.
        let args = argv(&ClaudeDriver::new(None).start(&resume_launch()));
        assert!(
            !args.iter().any(|arg| arg == "--append-system-prompt"),
            "a resume carries no opening prompt: {args:?}"
        );
    }

    #[test]
    fn a_persons_message_goes_out_as_one_json_line_the_protocol_understands() {
        let mut driver = ClaudeDriver::new(None);
        let bytes = driver.send(Input::Message { text: "hello".into(), attachments: vec![] });
        let text = String::from_utf8(bytes).expect("the codec writes utf-8");
        assert!(text.ends_with('\n'), "a line the child can read ends: {text:?}");
        let sent: serde_json::Value =
            serde_json::from_str(text.trim_end()).expect("one JSON object");
        assert_eq!(sent["type"], "user");
        assert_eq!(sent["message"]["role"], "user");
        assert_eq!(sent["message"]["content"], "hello");
    }

    #[test]
    fn an_attachment_reaches_the_agent_as_a_path_in_the_message() {
        // The default every profile has for images: a path named in the prompt
        // is the one channel every harness has. See `Profile::images`.
        let mut driver = ClaudeDriver::new(None);
        let bytes = driver.send(Input::Message {
            text: "look at this".into(),
            attachments: vec!["/tmp/shot.png".into()],
        });
        let text = String::from_utf8(bytes).expect("the codec writes utf-8");
        assert!(text.contains("/tmp/shot.png"), "{text:?}");
    }

    fn ticket() -> PermissionTicket {
        PermissionTicket { url: "http://127.0.0.1:4321/mcp/7".into(), token: "a-token".into() }
    }

    /// The path `--mcp-config` was given, or a panic naming what was there
    /// instead.
    fn config_arg(args: &[String]) -> PathBuf {
        let at = args.iter().position(|arg| arg == "--mcp-config").expect(&format!("{args:?}"));
        PathBuf::from(&args[at + 1])
    }

    #[test]
    fn a_session_with_a_ticket_is_told_where_to_ask_and_what_to_ask_with() {
        let driver = ClaudeDriver::new(Some(ticket()));
        let args = argv(&driver.start(&launch()));
        let config = config_arg(&args);
        // The whole point of holding the file on the driver: the child opens it
        // some way into its own start, so it has to still be there afterwards.
        assert!(config.exists(), "the config outlives the start: {}", config.display());
        let written: Value =
            serde_json::from_slice(&std::fs::read(&config).expect("the config reads back"))
                .expect("the config is JSON");
        let entry = &written["mcpServers"]["smetana"];
        assert_eq!(entry["type"], "http", "{written}");
        assert_eq!(entry["url"], "http://127.0.0.1:4321/mcp/7");
        assert_eq!(entry["headers"]["Authorization"], "Bearer a-token");
        assert!(entry.get("timeout").is_none(), "no timeout, ever: {entry}");

        let at = args
            .iter()
            .position(|arg| arg == "--permission-prompt-tool")
            .expect("the tool flag stands beside the config one");
        assert_eq!(args[at + 1], "mcp__smetana__approve");
    }

    #[cfg(unix)]
    #[test]
    fn the_config_is_readable_by_nobody_but_this_user() {
        // It holds the session's bearer token, and the directory it sits in is
        // shared with every other user of the machine.
        use std::os::unix::fs::PermissionsExt;
        let driver = ClaudeDriver::new(Some(ticket()));
        let config = config_arg(&argv(&driver.start(&launch())));
        let mode = std::fs::metadata(&config).expect("the config is there").permissions().mode();
        assert_eq!(mode & 0o077, 0, "mode {mode:o}");
    }

    #[test]
    fn the_config_goes_when_the_session_does() {
        let config = {
            let driver = ClaudeDriver::new(Some(ticket()));
            config_arg(&argv(&driver.start(&launch())))
        };
        assert!(!config.exists(), "left behind: {}", config.display());
    }

    #[test]
    fn a_session_with_no_ticket_carries_neither_flag() {
        // A codec test, and any session started where the config could not be
        // written: one flag without the other would be a harness failing every
        // tool call it would have asked about.
        let args = argv(&ClaudeDriver::new(None).start(&launch()));
        assert!(!args.iter().any(|arg| arg == "--mcp-config"), "{args:?}");
        assert!(!args.iter().any(|arg| arg == "--permission-prompt-tool"), "{args:?}");
    }

    #[test]
    fn interrupt_writes_one_control_request_line_with_a_fresh_request_id_each_time() {
        // smetana-y7mv: Stop closes the turn over stdin now, rather than
        // killing the child — measured against the installed CLI (2.1.270),
        // see this file's own header.
        let mut driver = ClaudeDriver::new(None);
        let first = driver.interrupt().expect("Claude Code can be asked to stop a turn");
        let text = String::from_utf8(first).expect("the codec writes utf-8");
        assert!(text.ends_with('\n'), "a line the child can read ends: {text:?}");
        let sent: Value = serde_json::from_str(text.trim_end()).expect("one JSON object");
        assert_eq!(sent["type"], "control_request");
        assert_eq!(sent["request"]["subtype"], "interrupt");
        let first_id = sent["request_id"].as_str().expect("a request_id string").to_string();
        assert!(!first_id.is_empty(), "{sent}");

        let second = driver.interrupt().expect("a second interrupt is answered the same way");
        let second_text = String::from_utf8(second).expect("the codec writes utf-8");
        let second_sent: Value =
            serde_json::from_str(second_text.trim_end()).expect("one JSON object");
        assert_ne!(
            second_sent["request_id"].as_str(),
            Some(first_id.as_str()),
            "two interrupts must not repeat a request_id: {sent} / {second_sent}"
        );
    }

    #[test]
    fn a_control_response_to_the_interrupt_draws_no_row() {
        // The CLI's own acknowledgement of the control_request `interrupt`
        // sends — a `type` this codec has never been told about, on the same
        // rule as any other one.
        let mut driver = ClaudeDriver::new(None);
        assert!(events(&mut driver, CONTROL_RESPONSE).is_empty());
    }

    #[test]
    fn an_interrupted_turn_still_closes_on_its_own_result_event() {
        // The whole reason nothing else in this file had to change for Stop:
        // "result" becomes EventKind::Result regardless of subtype or
        // is_error, so error_during_execution closes the turn exactly as
        // success does — `state_of` reads that as `ready`, not `failed`.
        let mut driver = ClaudeDriver::new(None);
        assert_eq!(
            events(&mut driver, RESULT_INTERRUPTED),
            vec![EventKind::Result { tokens_in: 5, tokens_out: 1, cost_usd: None, ms: 900 }]
        );
    }
}
