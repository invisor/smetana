# Stage 1 — the session worker

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A `Bare` Claude Code session can be spawned, talked to, and answered over IPC as a stream of typed events, with permissions routed through an in-app MCP listener — no UI at all.

**Architecture:** A new subsystem `src-tauri/src/session/` beside `terminal/`. The child runs over pipes rather than a PTY. A per-profile `Driver` owns the codec; the worker owns the process and an ordered journal of typed events; session state is derived from that journal. `terminal/` is not touched.

**Tech Stack:** Rust, tokio, axum (new), serde_json, Tauri 2. Reuses `agents::Profile` and `agents::claude` unchanged for command construction.

**Spec:** `docs/superpowers/specs/2026-09-10-conversation-ui-design.md`

## Global Constraints

- Comments, test names, `expect`/`panic` strings and log lines in **English**, without exception. Commit messages in Russian; what sits before the colon in a subject stays as it is.
- One new direct dependency: `axum`. Its transitive tree (`hyper` 1.11, `tower`, `tower-http`, `http-body-util`) is already in `Cargo.lock` via Tauri. Nothing else is added.
- `tokio`'s features must grow from `["sync", "time", "rt", "macros"]` to include `"net"` (the listener), `"process"` (spawning over pipes) and `"io-util"` (reading the child). The spec names `net`; the other two follow from spawning without a PTY.
- `service.rs` follows the shape `tracker/service.rs` and `terminal/service.rs` already have: one tokio task owning all mutable state, a request queue, a `select!`. Nothing shares state with it.
- Pure logic carries the tests: `model.rs`, `journal.rs`, and each driver's `feed`. `service.rs` and `permission.rs` get no unit test — I/O and orchestration, the same standing the three existing workers have.
- Do not touch `src-tauri/src/terminal/`, `src/stores/terminals.js` or `src/components/terminal/` anywhere in this plan.
- `cp`, `mv` and `rm` may be aliased to `-i`; every such call needs `-f`/`-rf`.

---

## File structure

| file | responsibility |
|---|---|
| `src-tauri/src/session/mod.rs` | the module tree and nothing else |
| `src-tauri/src/session/model.rs` | `Event`, `EventKind`, `Decision`, `SessionState`, `SessionError`, and the pure rule that derives state from a journal |
| `src-tauri/src/session/journal.rs` | the ordered log: `seq` numbering, append, snapshot-from, trimming |
| `src-tauri/src/session/driver.rs` | the `Driver` trait and `LineBuffer`, the transport-level line cutting shared by every driver |
| `src-tauri/src/agents/claude_driver.rs` | Claude Code's codec: JSONL in, `Event`s out; messages and decisions out |
| `src-tauri/src/session/permission.rs` | the in-app MCP listener: bind, token check, pending requests |
| `src-tauri/src/session/service.rs` | the worker |
| `src-tauri/src/session/commands.rs` | thin `#[tauri::command]`s |

---

## Task 1: Measure how long a permission call may take

Throwaway. Nothing from this task is kept; its output is a number written into the spec. It is first because every later task assumes the answer is "long enough for a person".

**Files:**
- Create: `/tmp/perm-probe/` (outside the repo — a stray file in the project root breaks a later merge's clean-checkout precondition)

- [ ] **Step 1: Write a stdio MCP server that stalls**

A stdio server is used here — not the HTTP one the product will ship — because the question is about the harness's patience, not about our transport, and stdio needs no crate.

```js
// /tmp/perm-probe/server.mjs — answers initialize and tools/list at once,
// then sits on tools/call for as long as it takes, logging when it started.
import { createInterface } from 'node:readline'

const TOOL = {
  name: 'approve',
  description: 'Approve or deny a tool call',
  inputSchema: {
    type: 'object',
    properties: { tool_name: { type: 'string' }, input: { type: 'object' } },
    required: ['tool_name', 'input']
  }
}

const send = (msg) => process.stdout.write(JSON.stringify(msg) + '\n')
const note = (text) => process.stderr.write(`[probe ${new Date().toISOString()}] ${text}\n`)

createInterface({ input: process.stdin }).on('line', (line) => {
  let req
  try { req = JSON.parse(line) } catch { return }
  if (req.method === 'initialize') {
    send({ jsonrpc: '2.0', id: req.id, result: {
      protocolVersion: '2024-11-05',
      capabilities: { tools: {} },
      serverInfo: { name: 'probe', version: '0.0.1' }
    }})
  } else if (req.method === 'tools/list') {
    send({ jsonrpc: '2.0', id: req.id, result: { tools: [TOOL] } })
  } else if (req.method === 'tools/call') {
    note(`tools/call arrived, stalling: ${JSON.stringify(req.params?.arguments ?? {})}`)
    // Deliberately never answered. We are timing the harness's patience.
  }
})
note('ready')
```

- [ ] **Step 2: Point Claude Code at it and make it want a tool**

```bash
mkdir -p /tmp/perm-probe && cd /tmp/perm-probe
cat > mcp.json <<'JSON'
{"mcpServers":{"probe":{"type":"stdio","command":"node","args":["/tmp/perm-probe/server.mjs"]}}}
JSON
cd /tmp/perm-probe && time claude -p --verbose --output-format stream-json \
  --mcp-config /tmp/perm-probe/mcp.json \
  --permission-prompt-tool mcp__probe__approve \
  'Run the shell command `echo hello` and tell me what it printed.' 2>probe.err | tee probe.out
```

- [ ] **Step 3: Read the answer off the clock**

Expected, if the harness waits indefinitely: the command hangs until you interrupt it, and `probe.err` shows `tools/call arrived, stalling` with nothing after. That is the good outcome.

If instead the run ends on its own, note the wall time `time` reports and find the event in `probe.out` that says why:

```bash
grep -o '"subtype":"[^"]*"' /tmp/perm-probe/probe.out | sort -u
tail -3 /tmp/perm-probe/probe.out | python3 -m json.tool
```

- [ ] **Step 4: Write the finding into the spec and commit**

Replace the paragraph under `## Failure` beginning "**The first thing the branch must measure**" with what was measured: the harness's behaviour, the wall time if bounded, and — if bounded — the consequence, which is that `PermissionRequest` needs a visible countdown and an automatic deny, and that this plan gains a task for it before Task 7.

```bash
cd /Users/flexo/Desktop/Projects/smetana
git add -f docs/superpowers/specs/2026-09-10-conversation-ui-design.md
git commit -m "docs: измерено, сколько Claude Code ждёт ответа на разрешение"
rm -rf /tmp/perm-probe
```

**STOP.** If the wait turned out to be bounded and short, do not start Task 2 — bring the number back and let the design be revisited.

---

## Task 2: The event vocabulary and the state rule

**Files:**
- Create: `src-tauri/src/session/mod.rs`, `src-tauri/src/session/model.rs`
- Modify: `src-tauri/src/lib.rs` (add `mod session;` beside `mod terminal;`)

**Interfaces:**
- Produces: `SessionId = u64`; `EventKind`, `Event { seq: u64, at: String, kind: EventKind }`, `Decision`, `SessionState`, `SessionError`, and `state_of(events: &[Event], child_alive: bool) -> SessionState`.

- [ ] **Step 1: Write the failing test**

Create `src-tauri/src/session/model.rs` containing only this test module for now.

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn ev(seq: u64, kind: EventKind) -> Event {
        Event { seq, at: "2026-09-10T12:00:00Z".into(), kind }
    }

    fn permission(id: &str) -> EventKind {
        EventKind::Permission {
            id: id.into(),
            tool: "Bash".into(),
            detail: "rm -rf /tmp/x".into(),
            options: vec![Decision::Allow, Decision::Deny],
        }
    }

    #[test]
    fn an_unanswered_permission_is_the_loudest_thing_a_journal_can_say() {
        // It outranks a turn in flight: the agent is not working, it is waiting.
        let events = vec![
            ev(1, EventKind::TurnStart { by: Actor::Agent }),
            ev(2, permission("p1")),
        ];
        assert_eq!(state_of(&events, true), SessionState::NeedsYou);
    }

    #[test]
    fn a_permission_that_was_answered_is_not_a_question_any_more() {
        let events = vec![
            ev(1, EventKind::TurnStart { by: Actor::Agent }),
            ev(2, permission("p1")),
            ev(3, EventKind::PermissionAnswered { id: "p1".into(), decision: Decision::Allow }),
        ];
        assert_eq!(state_of(&events, true), SessionState::Running);
    }

    #[test]
    fn two_questions_are_settled_one_at_a_time() {
        // Answering the first leaves the second standing. Matching by id rather
        // than by count is what makes an out-of-order answer harmless.
        let events = vec![
            ev(1, permission("p1")),
            ev(2, permission("p2")),
            ev(3, EventKind::PermissionAnswered { id: "p2".into(), decision: Decision::Deny }),
        ];
        assert_eq!(state_of(&events, true), SessionState::NeedsYou);
    }

    #[test]
    fn a_turn_closed_by_its_result_leaves_the_session_ready() {
        let events = vec![
            ev(1, EventKind::TurnStart { by: Actor::Agent }),
            ev(2, EventKind::Text { text: "done".into() }),
            ev(3, EventKind::Result { tokens_in: 10, tokens_out: 20, cost_usd: None, ms: 400 }),
        ];
        assert_eq!(state_of(&events, true), SessionState::Ready);
    }

    #[test]
    fn a_child_that_died_with_a_turn_open_failed_rather_than_finished() {
        let events = vec![ev(1, EventKind::TurnStart { by: Actor::Agent })];
        assert_eq!(state_of(&events, false), SessionState::Failed);
    }

    #[test]
    fn a_child_that_died_between_turns_simply_ended() {
        let events = vec![
            ev(1, EventKind::TurnStart { by: Actor::Agent }),
            ev(2, EventKind::Result { tokens_in: 1, tokens_out: 1, cost_usd: None, ms: 1 }),
        ];
        assert_eq!(state_of(&events, false), SessionState::Exited);
    }

    #[test]
    fn a_session_that_has_produced_nothing_yet_is_starting() {
        assert_eq!(state_of(&[], true), SessionState::Starting);
    }
}
```

- [ ] **Step 2: Run it and watch it fail**

Run: `cd src-tauri && cargo test session::model`
Expected: FAIL — `cannot find type Event in this scope`, and the rest.

- [ ] **Step 3: Write the vocabulary above the test module**

```rust
//! The vocabulary of a driven session and the pure rules over it. No I/O here:
//! the process lives in `service.rs`, the codec in a driver.
//!
//! Every type here is what the front end sees, so the serde shape is part of
//! the contract rather than an implementation detail.

pub type SessionId = u64;

/// Whose turn produced this.
#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Actor {
    Person,
    Agent,
}

/// What a person may answer a permission request with. The list an agent
/// actually offers travels on the request, because not every harness offers
/// all three.
#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Decision {
    Allow,
    /// Allow this tool for the rest of the session without asking again.
    AllowAlways,
    Deny,
}

/// The closed list. A driver maps its harness's protocol onto exactly this and
/// the front end never learns which harness it was talking to.
///
/// **An event type a driver does not recognise produces no event.** That is the
/// choice `agents::claude::transcript_line` already made, for the reason it
/// records: a missing row costs a person nothing the CLI's own logs do not
/// still hold, while a wall of raw protocol costs them the panel.
#[derive(Clone, PartialEq, Debug, serde::Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum EventKind {
    TurnStart { by: Actor },
    UserMessage { text: String, attachments: Vec<String> },
    /// Markdown as the agent wrote it. Rendering is the front end's business.
    Text { text: String },
    Reasoning { text: String },
    ToolUse { id: String, name: String, detail: String },
    ToolResult { id: String, ok: bool, summary: String },
    Permission { id: String, tool: String, detail: String, options: Vec<Decision> },
    PermissionAnswered { id: String, decision: Decision },
    Result { tokens_in: u64, tokens_out: u64, cost_usd: Option<f64>, ms: u64 },
    Error { text: String },
}

#[derive(Clone, PartialEq, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Event {
    pub seq: u64,
    /// RFC 3339, minted by the worker when the event is appended.
    pub at: String,
    #[serde(flatten)]
    pub kind: EventKind,
}

/// Translated by the store into the design system's statuses, the same way
/// `terminal::model::SessionState` is.
#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SessionState {
    Starting,
    Running,
    Ready,
    NeedsYou,
    Exited,
    Failed,
}

#[derive(Clone, Debug, thiserror::Error, serde::Serialize)]
#[serde(rename_all = "camelCase", tag = "kind", content = "message")]
pub enum SessionError {
    #[error("the session could not be started: {0}")]
    Spawn(String),
    #[error("there is no session {0}")]
    NoSuchSession(SessionId),
    #[error("there is no question {0} waiting for an answer")]
    NoSuchQuestion(String),
}

/// The whole of what a session's state is: a fold over its journal.
///
/// There is no bell here and no silence timer. Both existed to guess at what
/// the wire now says outright, and neither is reimplemented.
pub fn state_of(events: &[Event], child_alive: bool) -> SessionState {
    let mut open_turn = false;
    let mut pending: Vec<&str> = Vec::new();
    let mut seen_anything = false;
    for event in events {
        seen_anything = true;
        match &event.kind {
            EventKind::TurnStart { .. } => open_turn = true,
            EventKind::Result { .. } => open_turn = false,
            EventKind::Permission { id, .. } => pending.push(id),
            EventKind::PermissionAnswered { id, .. } => pending.retain(|open| open != id),
            _ => {}
        }
    }
    if !child_alive {
        // A turn still open when the process went is the honest reading of a
        // crash; a turn closed is an ordinary end.
        return if open_turn { SessionState::Failed } else { SessionState::Exited };
    }
    // A question outranks a turn in flight: the agent is not working, it waits.
    if !pending.is_empty() {
        return SessionState::NeedsYou;
    }
    if open_turn {
        return SessionState::Running;
    }
    if seen_anything { SessionState::Ready } else { SessionState::Starting }
}
```

And `src-tauri/src/session/mod.rs`:

```rust
//! Driving an agent over its own protocol, rather than reading the terminal it
//! paints. See `docs/superpowers/specs/2026-09-10-conversation-ui-design.md`.

pub mod model;
```

Add `mod session;` to `src-tauri/src/lib.rs` beside `mod terminal;`.

- [ ] **Step 4: Run the tests**

Run: `cd src-tauri && cargo test session::model`
Expected: PASS, 7 tests.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/session/ src-tauri/src/lib.rs
git commit -m "feat(session): словарь событий и правило состояния сессии"
```

---

## Task 3: The journal

**Files:**
- Create: `src-tauri/src/session/journal.rs`
- Modify: `src-tauri/src/session/mod.rs` (add `pub mod journal;`)

**Interfaces:**
- Consumes: `model::{Event, EventKind}` from Task 2.
- Produces: `Journal::new()`, `Journal::append(&mut self, kind: EventKind, at: String) -> Event`, `Journal::since(&self, seq: u64) -> Option<Vec<Event>>`, `Journal::snapshot(&self) -> (Vec<Event>, u64)`, `Journal::events(&self) -> &[Event]`, and `pub const BUDGET: usize`.

- [ ] **Step 1: Write the failing test**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::model::{Actor, EventKind};

    fn text(body: &str) -> EventKind {
        EventKind::Text { text: body.into() }
    }

    fn at() -> String {
        "2026-09-10T12:00:00Z".into()
    }

    #[test]
    fn seq_starts_at_one_so_that_zero_can_mean_nothing_seen_yet() {
        // A front end that has never attached asks from 0, and must be given
        // everything rather than everything-but-the-first.
        let mut journal = Journal::new();
        assert_eq!(journal.append(text("a"), at()).seq, 1);
        assert_eq!(journal.since(0).expect("nothing was trimmed").len(), 1);
    }

    #[test]
    fn since_hands_back_only_what_came_after_the_number_it_was_given() {
        let mut journal = Journal::new();
        journal.append(text("a"), at());
        journal.append(text("b"), at());
        journal.append(text("c"), at());
        let tail = journal.since(1).expect("nothing was trimmed");
        assert_eq!(tail.len(), 2);
        assert_eq!(tail[0].seq, 2);
    }

    #[test]
    fn a_seq_that_was_trimmed_away_answers_none_rather_than_a_hole() {
        // None is what makes the front end re-attach. Handing back a partial
        // tail would leave it drawing a conversation with a silent gap in it.
        let mut journal = Journal::new();
        for n in 0..BUDGET + 10 {
            journal.append(text(&n.to_string()), at());
        }
        assert!(journal.since(1).is_none(), "seq 1 is long gone");
        assert!(journal.since(journal.snapshot().1).is_some(), "the newest is always reachable");
    }

    #[test]
    fn trimming_never_takes_a_question_still_waiting_for_an_answer() {
        // Losing it would leave the session stuck in needs-you with nothing on
        // screen to answer, since `state_of` folds over what the journal holds.
        let mut journal = Journal::new();
        journal.append(
            EventKind::Permission {
                id: "p1".into(),
                tool: "Bash".into(),
                detail: "ls".into(),
                options: vec![],
            },
            at(),
        );
        for n in 0..BUDGET + 10 {
            journal.append(text(&n.to_string()), at());
        }
        assert!(
            journal.events().iter().any(|e| matches!(&e.kind, EventKind::Permission { id, .. } if id == "p1")),
            "the unanswered question survived the trim"
        );
    }

    #[test]
    fn a_question_that_has_been_answered_is_ordinary_and_may_be_trimmed() {
        let mut journal = Journal::new();
        journal.append(
            EventKind::Permission { id: "p1".into(), tool: "Bash".into(), detail: "ls".into(), options: vec![] },
            at(),
        );
        journal.append(
            EventKind::PermissionAnswered { id: "p1".into(), decision: crate::session::model::Decision::Allow },
            at(),
        );
        for n in 0..BUDGET + 10 {
            journal.append(text(&n.to_string()), at());
        }
        assert!(journal.events().len() <= BUDGET + 2, "answered questions do not accumulate");
    }

    #[test]
    fn the_snapshot_says_which_seq_to_continue_from() {
        let mut journal = Journal::new();
        journal.append(text("a"), at());
        journal.append(text("b"), at());
        let (events, seq) = journal.snapshot();
        assert_eq!(seq, 2);
        assert_eq!(events.last().expect("two were appended").seq, 2);
        assert!(journal.since(seq).expect("nothing trimmed").is_empty());
    }

    #[test]
    fn an_empty_journal_snapshots_to_nothing_at_zero() {
        let (events, seq) = Journal::new().snapshot();
        assert!(events.is_empty());
        assert_eq!(seq, 0);
    }

    #[test]
    fn the_turn_start_of_an_agent_is_kept_the_way_a_person_message_is() {
        // Both are structure rather than chatter, and a journal trimmed to
        // paragraphs alone would draw prose with no turn boundaries in it.
        let mut journal = Journal::new();
        journal.append(EventKind::TurnStart { by: Actor::Person }, at());
        assert_eq!(journal.events().len(), 1);
    }
}
```

- [ ] **Step 2: Run it and watch it fail**

Run: `cd src-tauri && cargo test session::journal`
Expected: FAIL — `cannot find type Journal in this scope`.

- [ ] **Step 3: Write the journal above the test module**

```rust
//! The ordered event log of one session. This is what `terminal::ring` is for a
//! terminal — the thing a window that has just opened is caught up from — with
//! events in place of bytes.

use super::model::{Event, EventKind};

/// How many events a session keeps. A long night of tool calls is thousands of
/// them and each is small; the ceiling is here so that a session left running
/// for a week cannot grow without bound.
pub const BUDGET: usize = 4000;

pub struct Journal {
    events: Vec<Event>,
    next: u64,
}

impl Journal {
    pub fn new() -> Self {
        Self { events: Vec::new(), next: 1 }
    }

    pub fn append(&mut self, kind: EventKind, at: String) -> Event {
        let event = Event { seq: self.next, at, kind };
        self.next += 1;
        self.events.push(event.clone());
        self.trim();
        event
    }

    /// Everything after `seq`, or `None` when `seq` is older than what is still
    /// held. `None` is not an error: it is what tells the front end to take a
    /// fresh snapshot instead of drawing a conversation with a hole in it.
    pub fn since(&self, seq: u64) -> Option<Vec<Event>> {
        let oldest = self.events.first().map_or(self.next, |event| event.seq);
        // `seq + 1` is the first event wanted; it must still be here, and the
        // caller asking for exactly what it already has is always answerable.
        if seq + 1 < oldest && seq != 0 || (seq == 0 && oldest > 1) {
            return None;
        }
        Some(self.events.iter().filter(|event| event.seq > seq).cloned().collect())
    }

    pub fn snapshot(&self) -> (Vec<Event>, u64) {
        (self.events.clone(), self.next - 1)
    }

    pub fn events(&self) -> &[Event] {
        &self.events
    }

    /// Drop from the front, and never a question nobody has answered.
    ///
    /// `state_of` folds over whatever the journal holds, so trimming an open
    /// `Permission` away would leave the session `needs-you` with nothing on
    /// screen to answer and no way back to `running`. An answered one carries
    /// no such weight and goes with the rest.
    fn trim(&mut self) {
        if self.events.len() <= BUDGET {
            return;
        }
        let answered: Vec<String> = self
            .events
            .iter()
            .filter_map(|event| match &event.kind {
                EventKind::PermissionAnswered { id, .. } => Some(id.clone()),
                _ => None,
            })
            .collect();
        let mut over = self.events.len() - BUDGET;
        self.events.retain(|event| {
            if over == 0 {
                return true;
            }
            if let EventKind::Permission { id, .. } = &event.kind {
                if !answered.contains(id) {
                    return true;
                }
            }
            over -= 1;
            false
        });
    }
}

impl Default for Journal {
    fn default() -> Self {
        Self::new()
    }
}
```

Add `pub mod journal;` to `src-tauri/src/session/mod.rs`.

- [ ] **Step 4: Run the tests**

Run: `cd src-tauri && cargo test session::journal`
Expected: PASS, 8 tests.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/session/
git commit -m "feat(session): журнал событий с seq и обрезкой, щадящей открытый вопрос"
```

---

## Task 4: The Driver seam and shared line cutting

**Files:**
- Create: `src-tauri/src/session/driver.rs`
- Modify: `src-tauri/src/session/mod.rs`

**Interfaces:**
- Consumes: `model::{Decision, EventKind}`.
- Produces: `trait Driver`, `enum Input`, `struct LineBuffer` with `LineBuffer::new()`, `feed(&mut self, bytes: &[u8]) -> Vec<String>`, and `pub const MAX_LINE: usize = 1 << 20`.

- [ ] **Step 1: Write the failing test**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_line_is_only_handed_over_once_it_has_ended() {
        let mut buffer = LineBuffer::new();
        assert!(buffer.feed(b"{\"a\":1}").is_empty(), "no newline yet, nothing to parse");
        assert_eq!(buffer.feed(b"\n"), vec!["{\"a\":1}".to_string()]);
    }

    #[test]
    fn a_character_split_across_two_reads_is_decoded_whole() {
        // Decoding each chunk as it arrives would put a replacement character
        // in the middle of every word unlucky enough to straddle a read.
        let mut buffer = LineBuffer::new();
        let text = "привет".as_bytes();
        let (head, tail) = text.split_at(3);
        assert!(buffer.feed(head).is_empty());
        let mut rest = tail.to_vec();
        rest.push(b'\n');
        assert_eq!(buffer.feed(&rest), vec!["привет".to_string()]);
    }

    #[test]
    fn several_lines_in_one_read_all_come_out() {
        let mut buffer = LineBuffer::new();
        assert_eq!(buffer.feed(b"one\ntwo\nthree\n"), vec!["one", "two", "three"]);
    }

    #[test]
    fn a_trailing_carriage_return_is_not_part_of_the_line() {
        let mut buffer = LineBuffer::new();
        assert_eq!(buffer.feed(b"one\r\n"), vec!["one".to_string()]);
    }

    #[test]
    fn a_line_past_the_ceiling_is_dropped_along_with_the_rest_of_itself() {
        // A tool result carrying a large file is one enormous line, and this
        // runs for the length of a night. Rendering the tail of it would be
        // rendering a fragment, so everything up to the next newline goes too.
        let mut buffer = LineBuffer::new();
        let huge = vec![b'x'; MAX_LINE + 1];
        assert!(buffer.feed(&huge).is_empty());
        assert!(buffer.feed(b"still the same line\n").is_empty());
        assert_eq!(buffer.feed(b"a fresh one\n"), vec!["a fresh one".to_string()]);
    }
}
```

- [ ] **Step 2: Run it and watch it fail**

Run: `cd src-tauri && cargo test session::driver`
Expected: FAIL — `cannot find type LineBuffer in this scope`.

- [ ] **Step 3: Write the seam above the test module**

```rust
//! What every harness has in common, and the seam that hides what it does not.
//!
//! `LineBuffer` is transport rather than vocabulary: cutting a pipe's bytes
//! into lines is the same job whichever protocol those lines carry, and it is
//! `terminal::transcript::Transcript`'s buffering half, kept and reused.

use portable_pty::CommandBuilder;

use super::model::{Decision, EventKind};
use crate::agents::Launch;

/// The ceiling on a line that has not ended yet.
pub const MAX_LINE: usize = 1 << 20;

/// What a person can put into a session.
#[derive(Clone, Debug)]
pub enum Input {
    Message { text: String, attachments: Vec<String> },
}

/// One live conversation's codec. The worker owns the process and the journal;
/// a driver owns only the translation, and knows nothing of either.
pub trait Driver: Send {
    /// What to spawn. Built by the profile, so nothing about a command line is
    /// written twice: `service.rs` turns this into a `tokio::process::Command`
    /// with `get_argv`, `get_cwd` and `iter_extra_env_as_str`.
    fn start(&self, launch: &Launch) -> CommandBuilder;

    /// Bytes off the child's stdout, as events. Zero events is the commonest
    /// answer and an ordinary one.
    fn feed(&mut self, bytes: &[u8]) -> Vec<EventKind>;

    /// A person's message, as bytes for the child's stdin.
    fn send(&mut self, input: Input) -> Vec<u8>;

    /// A person's answer to a question. Some harnesses answer over stdin, some
    /// over a channel of their own; `None` means this one needs no bytes here
    /// and the worker should look to the driver's own side channel.
    fn answer(&mut self, id: &str, decision: Decision) -> Option<Vec<u8>>;

    /// Stop the turn in flight. `None` means this harness has no way to be
    /// asked, and the worker's only recourse is killing the child — see the
    /// spec's "Known losses".
    fn interrupt(&mut self) -> Option<Vec<u8>>;
}

/// Bytes rather than a `String`, so that a multi-byte character split across
/// two reads is decoded once, whole, when its line ends.
pub struct LineBuffer {
    buf: Vec<u8>,
    /// A line was dropped for length: everything up to the next newline is the
    /// rest of it, and handing that on would be handing on a fragment.
    dropped: bool,
}

impl LineBuffer {
    pub fn new() -> Self {
        Self { buf: Vec::new(), dropped: false }
    }

    pub fn feed(&mut self, bytes: &[u8]) -> Vec<String> {
        self.buf.extend_from_slice(bytes);
        let mut lines = Vec::new();
        while let Some(end) = self.buf.iter().position(|byte| *byte == b'\n') {
            let line: Vec<u8> = self.buf.drain(..=end).collect();
            if self.dropped {
                self.dropped = false;
                continue;
            }
            let line = String::from_utf8_lossy(&line);
            let line = line.trim_end_matches(['\n', '\r']);
            if !line.is_empty() {
                lines.push(line.to_string());
            }
        }
        if self.buf.len() > MAX_LINE {
            self.buf.clear();
            self.dropped = true;
        }
        lines
    }
}

impl Default for LineBuffer {
    fn default() -> Self {
        Self::new()
    }
}
```

Add `pub mod driver;` to `src-tauri/src/session/mod.rs`.

- [ ] **Step 4: Run the tests**

Run: `cd src-tauri && cargo test session::driver`
Expected: PASS, 5 tests.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/session/
git commit -m "feat(session): трейт Driver и общая нарезка строк транспорта"
```

---

## Task 5: Claude Code's codec — the stream in

**Files:**
- Create: `src-tauri/src/agents/claude_driver.rs`
- Modify: `src-tauri/src/agents/mod.rs` (add `pub mod claude_driver;`), `src-tauri/src/agents/claude.rs` (make `tool_detail`, `one_line`, `clip` and `MAX_DETAIL` `pub(crate)`)

**Interfaces:**
- Consumes: `session::driver::{Driver, Input, LineBuffer}`, `session::model::{Actor, Decision, EventKind}`, `agents::claude::tool_detail`.
- Produces: `ClaudeDriver::new(permission: PermissionChannel) -> Self` — for this task the constructor takes nothing; Task 7 adds the parameter.

Reuse rather than rewrite: `tool_detail` already knows which field of which tool says what a call is doing, and it is the reference formatter's table. It moves visibility, not code.

- [ ] **Step 1: Write the failing test**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::model::{Actor, EventKind};

    /// Lines as Claude Code 2.1 writes them under `--output-format stream-json`.
    /// Captured rather than invented: an invented fixture tests the fixture.
    const INIT: &str = r#"{"type":"system","subtype":"init","model":"claude-opus-5","session_id":"abc"}"#;
    const TEXT: &str = r#"{"type":"assistant","message":{"content":[{"type":"text","text":"Reading the file now."}]}}"#;
    const TOOL: &str = r#"{"type":"assistant","message":{"content":[{"type":"tool_use","id":"t1","name":"Bash","input":{"command":"cargo test"}}]}}"#;
    const TOOL_RESULT: &str = r#"{"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"t1","is_error":false,"content":"ok\nok\nok"}]}}"#;
    const RESULT: &str = r#"{"type":"result","subtype":"success","duration_ms":4200,"total_cost_usd":0.031,"usage":{"input_tokens":120,"output_tokens":40}}"#;
    const RETRY: &str = r#"{"type":"system","subtype":"api_retry","error":"overloaded","attempt":2}"#;

    fn events(driver: &mut ClaudeDriver, line: &str) -> Vec<EventKind> {
        driver.feed(format!("{line}\n").as_bytes())
    }

    #[test]
    fn an_init_event_opens_the_agents_turn() {
        let mut driver = ClaudeDriver::new();
        assert_eq!(events(&mut driver, INIT), vec![EventKind::TurnStart { by: Actor::Agent }]);
    }

    #[test]
    fn a_text_block_becomes_one_paragraph_with_its_markdown_intact() {
        // Unlike the terminal's renderer, newlines are kept: the front end
        // parses markdown, and flattening here would destroy every list.
        let mut driver = ClaudeDriver::new();
        assert_eq!(
            events(&mut driver, TEXT),
            vec![EventKind::Text { text: "Reading the file now.".into() }]
        );
    }

    #[test]
    fn a_tool_call_carries_the_one_line_the_reference_formatter_shows() {
        let mut driver = ClaudeDriver::new();
        assert_eq!(
            events(&mut driver, TOOL),
            vec![EventKind::ToolUse { id: "t1".into(), name: "Bash".into(), detail: "cargo test".into() }]
        );
    }

    #[test]
    fn a_tool_result_says_how_it_went_and_never_what_it_said() {
        // A result routinely carries a whole file. The panel wants the outcome;
        // the content would be the wall of output this design exists to remove.
        let mut driver = ClaudeDriver::new();
        assert_eq!(
            events(&mut driver, TOOL_RESULT),
            vec![EventKind::ToolResult { id: "t1".into(), ok: true, summary: "3 lines".into() }]
        );
    }

    #[test]
    fn a_result_closes_the_turn_with_its_tokens_and_its_clock() {
        let mut driver = ClaudeDriver::new();
        assert_eq!(
            events(&mut driver, RESULT),
            vec![EventKind::Result { tokens_in: 120, tokens_out: 40, cost_usd: Some(0.031), ms: 4200 }]
        );
    }

    #[test]
    fn an_api_retry_is_an_error_a_person_should_see() {
        let mut driver = ClaudeDriver::new();
        assert_eq!(
            events(&mut driver, RETRY),
            vec![EventKind::Error { text: "api retry (overloaded), attempt 2".into() }]
        );
    }

    #[test]
    fn an_event_type_this_build_has_never_heard_of_produces_nothing() {
        let mut driver = ClaudeDriver::new();
        assert!(events(&mut driver, r#"{"type":"something_new","payload":42}"#).is_empty());
    }

    #[test]
    fn a_line_that_is_not_json_produces_nothing_rather_than_an_error_row() {
        // A harness that prints a warning to stdout must not put a red row in
        // somebody's conversation.
        let mut driver = ClaudeDriver::new();
        assert!(events(&mut driver, "warning: something").is_empty());
    }

    #[test]
    fn control_bytes_in_the_agents_own_prose_do_not_survive() {
        // Such a string routinely carries [31m and  in the JSON.
        // A bell would turn a row needs-you; colour would be escape codes on a
        // panel that is no longer a terminal.
        let mut driver = ClaudeDriver::new();
        let line = r#"{"type":"assistant","message":{"content":[{"type":"text","text":"red [31mhere"}]}}"#;
        let EventKind::Text { text } = &events(&mut driver, line)[0] else {
            panic!("a text block is a Text event");
        };
        assert!(!text.contains('\u{1b}') && !text.contains('\u{7}'), "{text:?}");
    }

    #[test]
    fn a_newline_inside_a_paragraph_is_kept_because_markdown_needs_it() {
        let mut driver = ClaudeDriver::new();
        let line = r#"{"type":"assistant","message":{"content":[{"type":"text","text":"- one\n- two"}]}}"#;
        assert_eq!(events(&mut driver, line), vec![EventKind::Text { text: "- one\n- two".into() }]);
    }
}
```

- [ ] **Step 2: Run it and watch it fail**

Run: `cd src-tauri && cargo test agents::claude_driver`
Expected: FAIL — `cannot find type ClaudeDriver in this scope`.

- [ ] **Step 3: Widen the three helpers in `claude.rs`**

Change `fn one_line`, `fn clip`, `fn tool_detail` and `const MAX_DETAIL` in `src-tauri/src/agents/claude.rs` to `pub(crate)`. Nothing else in that file changes; `transcript_line` keeps its callers.

- [ ] **Step 4: Write the codec above the test module**

```rust
//! Claude Code's protocol, as this app's vocabulary.
//!
//! A port of what `claude::transcript_line` already does for a terminal pane,
//! with one difference that runs through the whole file: this produces typed
//! events for components to draw, not strings for a terminal to print. So a
//! paragraph keeps its newlines — the front end parses markdown — while control
//! bytes are still stripped, for the reason `claude::one_line` records.

use serde_json::Value;

use super::claude::{clip, one_line, tool_detail, MAX_DETAIL};
use crate::agents::Launch;
use crate::session::driver::{Driver, Input, LineBuffer};
use crate::session::model::{Actor, Decision, EventKind};

pub struct ClaudeDriver {
    lines: LineBuffer,
}

impl ClaudeDriver {
    pub fn new() -> Self {
        Self { lines: LineBuffer::new() }
    }
}

impl Default for ClaudeDriver {
    fn default() -> Self {
        Self::new()
    }
}

/// Control bytes go; everything else, newlines included, stays.
fn prose(text: &str) -> String {
    text.chars().filter(|c| !c.is_control() || *c == '\n').collect()
}

fn str_at<'a>(value: &'a Value, key: &str) -> &'a str {
    value.get(key).and_then(Value::as_str).unwrap_or("")
}

fn u64_at(value: &Value, pointer: &str) -> u64 {
    value.pointer(pointer).and_then(Value::as_u64).unwrap_or(0)
}

fn one_event(event: &Value) -> Vec<EventKind> {
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
                            (!text.trim().is_empty()).then(|| EventKind::Text { text })
                        }
                        "thinking" => {
                            let text = prose(str_at(block, "thinking"));
                            (!text.trim().is_empty()).then(|| EventKind::Reasoning { text })
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
                    .map(|block| {
                        let body = block.get("content").map_or(String::new(), |content| {
                            content.as_str().map(str::to_string).unwrap_or_default()
                        });
                        let lines = body.lines().count();
                        EventKind::ToolResult {
                            id: str_at(block, "tool_use_id").to_string(),
                            ok: !block.get("is_error").and_then(Value::as_bool).unwrap_or(false),
                            summary: match lines {
                                0 => "no output".to_string(),
                                1 => "1 line".to_string(),
                                many => format!("{many} lines"),
                            },
                        }
                    })
                    .collect()
            })
            .unwrap_or_default(),
        "result" => vec![EventKind::Result {
            tokens_in: u64_at(event, "/usage/input_tokens"),
            tokens_out: u64_at(event, "/usage/output_tokens"),
            cost_usd: event.get("total_cost_usd").and_then(Value::as_f64),
            ms: event.get("duration_ms").and_then(Value::as_u64).unwrap_or(0),
        }],
        _ => Vec::new(),
    }
}

impl Driver for ClaudeDriver {
    fn start(&self, launch: &Launch) -> portable_pty::CommandBuilder {
        // Task 6 replaces this with the streaming flags.
        crate::agents::for_id(launch.agent.as_str()).command(launch)
    }

    fn feed(&mut self, bytes: &[u8]) -> Vec<EventKind> {
        self.lines
            .feed(bytes)
            .iter()
            .filter_map(|line| serde_json::from_str::<Value>(line).ok())
            .flat_map(|event| one_event(&event))
            .collect()
    }

    fn send(&mut self, _input: Input) -> Vec<u8> {
        // Task 6.
        Vec::new()
    }

    fn answer(&mut self, _id: &str, _decision: Decision) -> Option<Vec<u8>> {
        // Task 7: this harness answers over the permission listener, not stdin.
        None
    }

    fn interrupt(&mut self) -> Option<Vec<u8>> {
        // Named in the spec as a known loss: outside its own SDK, Claude Code
        // has no documented way of being asked to stop a turn.
        None
    }
}
```

Add `pub mod claude_driver;` to `src-tauri/src/agents/mod.rs`.

- [ ] **Step 5: Run the tests**

Run: `cd src-tauri && cargo test agents::claude_driver`
Expected: PASS, 10 tests.

- [ ] **Step 6: Check nothing else broke and commit**

```bash
cd src-tauri && cargo test
git add src-tauri/src/agents/
git commit -m "feat(agents): кодек Claude Code — поток JSONL в события словаря"
```

---

## Task 6: Claude Code's codec — the command line and the input

**Files:**
- Modify: `src-tauri/src/agents/claude_driver.rs`

**Interfaces:**
- Produces: `ClaudeDriver::start` returning a streaming command line; `ClaudeDriver::send` producing a stdin message.

- [ ] **Step 1: Write the failing test**

Add to the existing `mod tests`:

```rust
    use crate::agents::{Intent, Launch};
    use crate::session::driver::Input;

    fn launch() -> Launch {
        Launch { session_id: Some("11111111-2222-3333-4444-555555555555".into()), ..Launch::bare("/tmp/p") }
    }

    fn argv(builder: &portable_pty::CommandBuilder) -> Vec<String> {
        builder.get_argv().iter().map(|arg| arg.to_string_lossy().to_string()).collect()
    }

    #[test]
    fn the_command_line_asks_for_a_two_way_stream() {
        // Both halves are required together: --input-format stream-json is
        // refused without --output-format stream-json, and --verbose is what
        // makes the output one event per line rather than one blob at the end.
        let args = argv(&ClaudeDriver::new().start(&launch()));
        for flag in ["--input-format", "stream-json", "--output-format", "--verbose", "-p"] {
            assert!(args.iter().any(|arg| arg == flag), "{flag} is missing from {args:?}");
        }
    }

    #[test]
    fn the_conversation_id_this_app_chose_is_on_the_line() {
        let args = argv(&ClaudeDriver::new().start(&launch()));
        let at = args.iter().position(|arg| arg == "--session-id").expect("the flag is on the line");
        assert_eq!(args[at + 1], "11111111-2222-3333-4444-555555555555");
    }

    #[test]
    fn a_persons_message_goes_out_as_one_json_line_the_protocol_understands() {
        let mut driver = ClaudeDriver::new();
        let bytes = driver.send(Input::Message { text: "hello".into(), attachments: vec![] });
        let text = String::from_utf8(bytes).expect("the codec writes utf-8");
        assert!(text.ends_with('\n'), "a line the child can read ends: {text:?}");
        let sent: serde_json::Value = serde_json::from_str(text.trim_end()).expect("one JSON object");
        assert_eq!(sent["type"], "user");
        assert_eq!(sent["message"]["role"], "user");
        assert_eq!(sent["message"]["content"], "hello");
    }

    #[test]
    fn an_attachment_reaches_the_agent_as_a_path_in_the_message() {
        // The default every profile has for images: a path named in the prompt
        // is the one channel every harness has. See `Profile::images`.
        let mut driver = ClaudeDriver::new();
        let bytes = driver.send(Input::Message {
            text: "look at this".into(),
            attachments: vec!["/tmp/shot.png".into()],
        });
        let text = String::from_utf8(bytes).expect("the codec writes utf-8");
        assert!(text.contains("/tmp/shot.png"), "{text:?}");
    }
```

- [ ] **Step 2: Run it and watch it fail**

Run: `cd src-tauri && cargo test agents::claude_driver`
Expected: FAIL — the flag assertions, and `sent["type"]` on an empty string.

- [ ] **Step 3: Implement `start` and `send`**

Replace the two stub methods:

```rust
    fn start(&self, launch: &Launch) -> portable_pty::CommandBuilder {
        // The profile still builds the line: the plugins, the skills directory,
        // the model flag, `--session-id` and the resume rule are all its
        // knowledge and none of it is written twice here. What this adds is the
        // shape of the conversation, which is the driver's own business.
        let mut cmd = crate::agents::claude::Claude.command(launch);
        for arg in ["-p", "--verbose", "--output-format", "stream-json", "--input-format", "stream-json"] {
            cmd.arg(arg);
        }
        cmd
    }

    fn send(&mut self, input: Input) -> Vec<u8> {
        let Input::Message { text, attachments } = input;
        // A path named in the prose is the one channel every harness has, and
        // it is what `ImageDelivery::InPrompt` already means elsewhere.
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
```

- [ ] **Step 4: Run the tests**

Run: `cd src-tauri && cargo test agents::claude_driver`
Expected: PASS, 14 tests.

If `Claude` is not reachable as `crate::agents::claude::Claude`, use whatever `agents::mod.rs` already exports for it — `for_id("claude")` returns the same profile behind a trait object and `command` is on the trait.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/agents/
git commit -m "feat(agents): командная строка двустороннего потока и ввод человека"
```

---

## Task 7: The permission listener

**Files:**
- Create: `src-tauri/src/session/permission.rs`
- Modify: `src-tauri/src/session/mod.rs`, `src-tauri/Cargo.toml`

**Interfaces:**
- Consumes: `model::{Decision, SessionId}`.
- Produces: `PermissionServer::start() -> Result<PermissionServer, std::io::Error>` (binds and spawns), `PermissionServer::register(&self, session: SessionId) -> PermissionTicket { url: String, token: String }`, `PermissionServer::forget(&self, session: SessionId)`, `PermissionServer::answer(&self, id: &str, decision: Decision) -> bool`, and a channel of `Asked { session: SessionId, id: String, tool: String, detail: String }` the worker selects on.

No unit test: this is I/O, the same standing `service.rs` has. It is proven by Task 9's manual check.

- [ ] **Step 1: Add the dependency**

In `src-tauri/Cargo.toml`:

```toml
axum = { version = "0.8", default-features = false, features = ["http1", "json", "tokio"] }
```

and widen tokio:

```toml
tokio = { version = "1", features = ["sync", "time", "rt", "macros", "net", "process", "io-util"] }
```

Run: `cd src-tauri && cargo build`
Expected: builds. Confirm nothing large was pulled in:

```bash
cd src-tauri && cargo tree -i hyper --depth 0 && git diff --stat Cargo.lock
```

- [ ] **Step 2: Write the listener**

```rust
//! The channel by which a harness asks this app for permission.
//!
//! Claude Code, outside its own SDK, has one documented way to route a
//! permission to a client: `--permission-prompt-tool`, naming a tool on an MCP
//! server it connects to. So the app hosts one. Not a separate binary and not a
//! stdio server the harness spawns, because either would need a channel of its
//! own back here anyway.
//!
//! **The token is not decoration.** A loopback port is reachable by anything
//! running as this user, and an unauthenticated one would let another process
//! answer, on a person's behalf, a question about running `rm -rf`. One session,
//! one token, carried in `Authorization`; a request without the right one is
//! refused here and never reaches a journal.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::routing::post;
use axum::{Json, Router};
use serde_json::{json, Value};
use tokio::sync::{mpsc, oneshot};

use super::model::{Decision, SessionId};

/// One question, on its way to the worker.
pub struct Asked {
    pub session: SessionId,
    pub id: String,
    pub tool: String,
    pub detail: String,
}

pub struct PermissionTicket {
    pub url: String,
    pub token: String,
}

struct Inner {
    /// Per session: the token that session's child must present.
    tokens: HashMap<SessionId, String>,
    /// Questions asked and not yet answered, by the id we minted.
    waiting: HashMap<String, oneshot::Sender<Decision>>,
    next_question: u64,
}

pub struct PermissionServer {
    port: u16,
    inner: Arc<Mutex<Inner>>,
}

impl PermissionServer {
    pub async fn start(asked: mpsc::Sender<Asked>) -> std::io::Result<Self> {
        let inner = Arc::new(Mutex::new(Inner {
            tokens: HashMap::new(),
            waiting: HashMap::new(),
            next_question: 1,
        }));
        // Port 0: the OS picks a free one. A fixed port would collide with a
        // second copy of the app, and with anything else on the machine.
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let port = listener.local_addr()?.port();
        let app = Router::new()
            .route("/mcp/{session}", post(handle))
            .with_state((inner.clone(), asked));
        tokio::spawn(async move {
            if let Err(error) = axum::serve(listener, app).await {
                log::error!("the permission listener stopped: {error}");
            }
        });
        Ok(Self { port, inner })
    }

    pub fn register(&self, session: SessionId) -> PermissionTicket {
        let token = crate::terminal::conversation::new_id();
        self.inner.lock().expect("the permission lock is never poisoned").tokens.insert(session, token.clone());
        PermissionTicket { url: format!("http://127.0.0.1:{}/mcp/{session}", self.port), token }
    }

    pub fn forget(&self, session: SessionId) {
        let mut inner = self.inner.lock().expect("the permission lock is never poisoned");
        inner.tokens.remove(&session);
    }

    /// A person's answer. `false` means there was no such question waiting —
    /// the child gave up, or it was answered twice.
    pub fn answer(&self, id: &str, decision: Decision) -> bool {
        let sender = self
            .inner
            .lock()
            .expect("the permission lock is never poisoned")
            .waiting
            .remove(id);
        sender.map(|tx| tx.send(decision).is_ok()).unwrap_or(false)
    }
}

type Shared = (Arc<Mutex<Inner>>, mpsc::Sender<Asked>);

async fn handle(
    Path(session): Path<SessionId>,
    State((inner, asked)): State<Shared>,
    headers: HeaderMap,
    Json(request): Json<Value>,
) -> Result<Json<Value>, StatusCode> {
    let presented = headers
        .get("authorization")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .unwrap_or_default()
        .to_string();
    {
        let inner = inner.lock().expect("the permission lock is never poisoned");
        match inner.tokens.get(&session) {
            Some(expected) if *expected == presented => {}
            _ => return Err(StatusCode::UNAUTHORIZED),
        }
    }
    let id = request.get("id").cloned().unwrap_or(Value::Null);
    let reply = |result: Value| Ok(Json(json!({ "jsonrpc": "2.0", "id": id, "result": result })));

    match request.get("method").and_then(Value::as_str).unwrap_or_default() {
        "initialize" => reply(json!({
            "protocolVersion": "2024-11-05",
            "capabilities": { "tools": {} },
            "serverInfo": { "name": "smetana-permissions", "version": "1" },
        })),
        "tools/list" => reply(json!({ "tools": [{
            "name": "approve",
            "description": "Ask the person supervising this session whether to run a tool",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "tool_name": { "type": "string" },
                    "input": { "type": "object" },
                },
                "required": ["tool_name", "input"],
            },
        }]})),
        "tools/call" => {
            let arguments = request.pointer("/params/arguments").cloned().unwrap_or(Value::Null);
            let tool = arguments.get("tool_name").and_then(Value::as_str).unwrap_or("a tool").to_string();
            let detail = crate::agents::claude::tool_detail(
                &tool,
                arguments.get("input").unwrap_or(&Value::Null),
            );
            let (tx, rx) = oneshot::channel();
            let question = {
                let mut inner = inner.lock().expect("the permission lock is never poisoned");
                let question = format!("q{}", inner.next_question);
                inner.next_question += 1;
                inner.waiting.insert(question.clone(), tx);
                question
            };
            if asked
                .send(Asked { session, id: question.clone(), tool: tool.clone(), detail })
                .await
                .is_err()
            {
                return Err(StatusCode::SERVICE_UNAVAILABLE);
            }
            // Held open until a person answers. The spec's Task 1 measured how
            // long the harness will wait for this.
            let decision = rx.await.unwrap_or(Decision::Deny);
            let behavior = match decision {
                Decision::Allow | Decision::AllowAlways => "allow",
                Decision::Deny => "deny",
            };
            // The shape `--permission-prompt-tool` expects back: one text
            // block whose body is the decision as JSON.
            let payload = if behavior == "allow" {
                json!({ "behavior": "allow", "updatedInput": arguments.get("input").cloned().unwrap_or(json!({})) })
            } else {
                json!({ "behavior": "deny", "message": "The person supervising this session declined." })
            };
            reply(json!({ "content": [{ "type": "text", "text": payload.to_string() }] }))
        }
        // Notifications carry no id and want no answer.
        _ => reply(json!({})),
    }
}
```

- [ ] **Step 3: Wire the flags into the driver**

`ClaudeDriver::new` gains the ticket, and `start` writes the config. In `claude_driver.rs`:

```rust
pub struct ClaudeDriver {
    lines: LineBuffer,
    /// Where this session's child asks about permissions, and with what token.
    /// `None` in tests that only exercise the codec.
    permission: Option<PermissionTicket>,
    /// The config file handed to the child. Kept alive for the session's
    /// length: a temporary deleted on drop would be gone before the child read
    /// it, and the MCP server would simply never connect.
    config: Option<std::path::PathBuf>,
}
```

`start` writes it beside the session's other scratch state and adds the flags:

```rust
        if let Some(ticket) = &self.permission {
            let config = json!({ "mcpServers": { "smetana": {
                // `type` is required: an entry with a url and no type is read
                // as stdio and silently skipped.
                "type": "http",
                "url": ticket.url,
                "headers": { "Authorization": format!("Bearer {}", ticket.token) },
            }}});
            // written to `config` path, then:
            cmd.arg("--mcp-config");
            cmd.arg(path);
            cmd.arg("--permission-prompt-tool");
            cmd.arg("mcp__smetana__approve");
        }
```

- [ ] **Step 4: Build and commit**

Run: `cd src-tauri && cargo test`
Expected: PASS — the codec tests still pass, built with `ClaudeDriver::new()` taking `None` for the ticket.

```bash
git add src-tauri/
git commit -m "feat(session): MCP-слушатель разрешений внутри приложения, с токеном на сессию"
```

---

## Task 8: The worker and the commands

**Files:**
- Create: `src-tauri/src/session/service.rs`, `src-tauri/src/session/commands.rs`
- Modify: `src-tauri/src/session/mod.rs`, `src-tauri/src/lib.rs` (register the handle and the commands)

**Interfaces:**
- Produces the IPC surface Stage 2 consumes. Stage 2's store wraps `session_attach`
  in a function it calls `attach(id)`; the command's name is the one below.


| command | arguments | answers |
|---|---|---|
| `session_start` | `project: String, intent: Intent` | `SessionId` |
| `session_attach` | `id: SessionId` | `{ events: Vec<Event>, seq: u64, state: SessionState }` |
| `session_since` | `id: SessionId, seq: u64` | `Option<Vec<Event>>` — `None` means take a fresh snapshot |
| `session_send` | `id: SessionId, text: String, attachments: Vec<String>` | `()` |
| `session_answer` | `id: SessionId, question: String, decision: Decision` | `()` |
| `session_stop` | `id: SessionId` | `()` |

- Events out: `session:events` `{ id, events }` for every session, active or not; `session:state` `{ id, state }`.

- [ ] **Step 1: Write the worker**

Follow `terminal/service.rs`: one tokio task, a `select!` over the request queue, child output from per-session reader tasks, and the `Asked` channel from Task 7. Spawn with `tokio::process::Command`, translated from the driver's `CommandBuilder`:

```rust
/// The profile built the line; this only moves it onto a type that can be
/// spawned over pipes. Nothing about a command line is decided here.
fn spawnable(builder: portable_pty::CommandBuilder) -> tokio::process::Command {
    let argv = builder.get_argv();
    let mut cmd = tokio::process::Command::new(&argv[0]);
    cmd.args(&argv[1..]);
    if let Some(cwd) = builder.get_cwd() {
        cmd.current_dir(cwd);
    }
    for (key, value) in builder.iter_extra_env_as_str() {
        cmd.env(key, value);
    }
    cmd.stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    cmd
}
```

Rules the worker holds, each one load-bearing:

- **Events flow for every session, not only the active one.** Unlike the terminal, an event is a bounded object rather than a repaint, and a background session's turn result is worth having when its tab is opened. There is no equivalent of `flush()` dropping a background session's bytes.
- **A person's message appends `TurnStart { by: Person }` and `UserMessage` to the journal before the bytes go to stdin.** The harness does not echo it back, and a conversation that shows only the agent's half is not one.
- **An `Asked` from the listener appends a `Permission` event**; `session_answer` appends `PermissionAnswered` and then calls `PermissionServer::answer`. In that order: the journal is what `state_of` reads, and a child that dies between the two must not leave a question standing.
- **`stderr` is drained and logged, never journalled.** It is the harness's own diagnostics, not the conversation.
- **On the child exiting**, mark it dead, recompute the state and emit it. `state_of`'s `child_alive` is that flag.
- **`session_stop`** tries `Driver::interrupt` first and kills the child when it answers `None`.

- [ ] **Step 2: Write the commands**

Copy the shape of `terminal/commands.rs` exactly — `ask` and `tell` helpers over a `oneshot`, the outer `Result` about delivery to the worker and the inner one about the operation.

- [ ] **Step 3: Register in `lib.rs`**

Manage the `SessionHandle` alongside `TerminalHandle`, and add the six commands to `invoke_handler`.

- [ ] **Step 4: Prove it end to end by hand**

There is no automated check for this task — it is I/O and orchestration. From `npm run tauri dev`, in the webview console:

```js
const id = await window.__TAURI__.core.invoke('session_start', { project: '/tmp/probe', intent: { kind: 'bare' } })
await window.__TAURI__.event.listen('session:events', (e) => console.log(JSON.stringify(e.payload)))
await window.__TAURI__.core.invoke('session_send', { id, text: 'Run `echo hello` and tell me what it printed.', attachments: [] })
```

Expected in order: `turn-start`, some `text`, a `permission` for `Bash`, then nothing until answered. Then:

```js
await window.__TAURI__.core.invoke('session_answer', { id, question: 'q1', decision: 'allow' })
```

Expected: `permission-answered`, `tool-use`, `tool-result`, `text`, `result`.

- [ ] **Step 5: Commit**

```bash
cd src-tauri && cargo test && cd ..
git add src-tauri/
git commit -m "feat(session): воркер управляемых сессий и его команды"
```

---

## Self-review notes for the executor

- Nothing in this plan touches `terminal/`. If a task seems to need to, stop and say so — that is the sign the migration seam has been misread.
- `Intent::Bare` is the only intent this stage starts. Others reaching `session_start` should be refused with `SessionError::Spawn`, not half-supported.
- **Restore is out of scope and no task here writes `.smetana/agents.json`.** A driven
  session lives as long as the app does. The spec says why, under Lifecycle; a third stage
  covers the restorable list, the offline row and `--resume`. Do not add the record's
  transport field here — a field written by nothing that reads it is worse than none.
- The one number this plan does not know is Task 1's. If it comes back bounded and short, Task 7's design changes and so does the front end's.
