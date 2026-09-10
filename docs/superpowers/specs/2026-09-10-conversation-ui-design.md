# Driving the agent instead of watching it — the conversation UI

Today an agent session is a terminal: the app spawns a harness under a PTY, the harness
paints a TUI for a person, and the app learns what is happening by reading that picture
with a `vt100` emulator. Everything the product knows about a session — is it running, is
it waiting on a human, what is it asking — is inferred from pixels somebody else drew.

This replaces that. The app becomes the harness's **client** over the structured protocol
each one already speaks, keeps an ordered journal of typed events, and draws the
conversation with its own components. What a session looks like stops depending on which
CLI it is.

Two things follow that are worth naming before the design: the app's knowledge of a
session becomes exact rather than inferred, and every feature a TUI gave a person for free
becomes something this product owns and must build.

## Why

- **A uniform look across agents.** Claude Code and Codex paint different terminals; a
  third agent would paint a third. Rendered from one vocabulary, all of them read the same.
- **Room for our own features.** A tool call, a permission, a turn's cost are objects, not
  scrollback. Anything the product wants to add — jump to a file a tool touched, re-run a
  command, fold a turn — becomes possible for the first time.
- **Detection stops being guesswork.** `needs-you` is an unanswered permission request, a
  fact off the wire, rather than a numbered list matched on a screen that a CLI release can
  reword.

## Scope of the first branch

One agent, one intent, the whole vertical: **Claude Code** and **`Intent::Bare`**, carried
from spawn to protocol to vocabulary to permissions to input to rendered panel to session
state. Every seam is exercised once. Nothing else moves.

`terminals.js` and `components/terminal/` are not touched on this branch. A `Bare` session
under Claude Code opens a tab of the new kind; every other intent keeps the terminal. The
two coexist until the last intent has moved.

Out of scope here, and each its own spec afterwards: the remaining interactive intents,
Codex, and the runs' batches.

## Architecture

A new subsystem, `src-tauri/src/session/`, beside `src-tauri/src/terminal/` rather than
inside it. The terminal subsystem rests on one invariant — a single byte stream and three
emulations of it that agree by construction — and a driven session has no screen, no
`vt100` grid and no bell. Folding one into the other would leave `service.rs` running two
different programs out of one `select!`, and `.claude/rules/terminal.md` describing two
incompatible worlds.

The endpoint is not coexistence. When the last intent has moved, `terminal/` is deleted;
the migration seam is therefore a union of two tab kinds, deliberately not an abstraction
over two backends, because a transitional abstraction is the kind that never leaves.

```
src-tauri/src/session/
  model.rs     Session, SessionState, Event, Journal — the vocabulary and its pure rules
  journal.rs   the ordered event log per session, seq numbering, trimming
  driver.rs    the Driver trait; owns a harness's codec and nothing else
  permission.rs the in-app MCP server: listener, per-session token, pending requests
  service.rs   the worker: one tokio task owning all mutable state
  commands.rs  thin #[tauri::command]s
```

`service.rs` follows the shape `tracker/service.rs` and `terminal/service.rs` already
have: one task, a request queue, child output arriving from per-session reader threads, a
flush tick, all meeting in one `select!`. Nothing shares state with it.

### The event vocabulary

A closed list, identical for every agent. A profile maps its protocol onto it; the front
end never learns an agent's name from an event.

| event | carries |
|---|---|
| `TurnStart` | whose turn: the person's or the agent's |
| `UserMessage` | the person's text, and the attachments that went with it |
| `Text` | a paragraph of the agent's prose, markdown as written |
| `Reasoning` | a thinking block, where the harness emits one (Codex does; Claude Code does not) |
| `ToolUse` | the tool's name and one line of what it is doing |
| `ToolResult` | how it ended — ok, error, how much output — never the output itself |
| `Permission` | an id, the tool, its arguments, and the answers on offer |
| `Result` | the turn's close: tokens, cost, duration |
| `Error` | anything that went wrong, API retries included |

`ToolUse`'s one line is `agents::claude::tool_detail`, which already exists and already
knows which field of which tool says what a call is doing. It moves rather than being
rewritten.

**An event type a driver does not recognise produces no event.** This is the same choice
`transcript_line` already made and for the same reason: a missing row costs a person
nothing they cannot get from the CLI's own logs, while a wall of raw protocol costs them
the panel.

### Session state is derived from the journal

- an unanswered `Permission` → `needs-you`
- a turn open, no pending permission → `running`
- a turn closed by `Result` → `ready`
- the child gone with an open turn → `failed`

There is no layer A and no layer B. The bell, the silence timer, `Quiet`'s screen
fingerprint and `Profile::question` have no counterpart here and are not reimplemented:
each existed to guess at what the wire now says outright.

### The journal

An ordered `Vec<Event>` per session with a monotonic `seq`, trimmed from the front on a
budget. `session_attach` hands back a snapshot plus the `seq` to continue from; the front
end re-attaches on an out-of-sequence event. This is `ring.rs`'s mechanic, by events
instead of bytes, and it is kept for the same reason: the window can close and reopen
while the session lives on. Events live in Rust, never only in the webview.

Unlike the terminal, events flow for **every** session rather than only the active one — an
event is a bounded object, not a repaint, and a background session's turn result is worth
having when its tab is opened.

## The Driver seam

`Driver` is a new trait beside `Profile`, not a widening of it. `Profile` answers
questions and owns nothing; a driver owns a conversation. One driver instance per live
session, built at spawn.

```rust
trait Driver: Send {
    fn start(&self, launch: &Launch) -> CommandBuilder;
    fn feed(&mut self, bytes: &[u8]) -> Vec<Event>;
    fn send(&mut self, input: Input) -> Vec<u8>;
    fn answer(&mut self, id: PermissionId, decision: Decision) -> Vec<u8>;
    fn interrupt(&mut self) -> Option<Vec<u8>>;
}
```

The worker owns the child process and the journal; the driver owns only the codec. Neither
knows Vue exists.

`feed` is what `terminal/transcript.rs` and `transcript_line` are today, with two
differences: typed events on the way out, and the partial-line buffering stays shared —
that part is about transport, not about any agent. `MAX_LINE` and its one-line note carry
over unchanged.

The protocols differ entirely and the difference stays inside the drivers. Claude Code is
`--input-format stream-json --output-format stream-json --verbose`: a JSONL event per line
out, a JSON message appended to stdin in. Codex is `codex app-server`: JSON-RPC over the
same pipes, `thread/start` and `turn/start` out, streamed items and server-to-client
requests back.

The process is spawned over **pipes rather than a PTY**. There is no terminal left to
emulate, and `terminal_resize` has no counterpart.

### Permissions

Claude Code, outside its own SDK, has one documented way to route a permission to a
client: `--permission-prompt-tool`, naming a tool on an MCP server the harness connects
to. The raw `control_request` channel the official SDK uses for `canUseTool` is an SDK
internal and is deliberately not built on — `.claude/rules/agents.md` records what
guessing at a CLI's vocabulary has already cost this project.

The server is **hosted inside the Tauri process**: an HTTP listener bound to `127.0.0.1`,
named to the child at spawn through `--mcp-config` as `{"type": "http", "url": …,
"headers": {"Authorization": "Bearer …"}}` — the transport and the header are both
documented, and `type` is required, since an entry with a `url` and no `type` is read as
stdio and silently skipped. Not a separate binary and not a stdio server the harness
spawns, because either would need a channel of its own back to the app anyway.

The token is not optional and not decoration. A loopback port is reachable by anything
running as this user, and an unauthenticated one would let another process answer, on a
person's behalf, a question about running `rm -rf`. One session, one token, carried in the
header; a request without the right one is refused at the listener and never reaches the
journal.

This listener is the design's **one new dependency**: `axum`, plus `tokio`'s `net` feature,
which the current build does not enable. Its transitive tree is not new — `hyper` 1.11,
`tower`, `tower-http` and `http-body-util` are already in `Cargo.lock`, pulled in by Tauri
itself, so what is added is a thin layer over crates this project already compiles. Writing
an HTTP server by hand to avoid it would be the worse trade: hand-rolled parsers on a
socket that answers permission questions is exactly where not to save a dependency.

A request arrives as a tool call, becomes a `Permission` event, and the person's answer
completes the still-open HTTP request. Codex needs none of this: `item/tool/requestUserInput`
arrives over the same connection and is answered there. The asymmetry lives in the drivers;
both surface the same `Permission`.

### Known losses

Named here rather than discovered later. Both are the price of leaving the TUI.

- **Interrupting a turn.** Codex has it in the protocol. Claude Code, outside the SDK, does
  not: the crude fallback is killing the child and resuming the conversation, which loses
  the turn in flight. `interrupt` returning `None` is the honest encoding of "this harness
  cannot".
- **Changing permission mode mid-turn** (`shift+tab`). No documented path outside the SDK.
  The mode is chosen when a turn starts.

## The front end

A new store, `src/stores/conversation.js`, beside `terminals.js`. It holds the journal per
session, the composer's draft, and the unanswered permission. It is the only new file in
`src/` that knows Tauri exists, and it joins the list in `CLAUDE.md`.

A new component group, `src/components/conversation/`, one component per event kind:

| component | notes |
|---|---|
| `ConversationView` | the journal; auto-scroll that detaches when a person scrolls up |
| `AgentMessage` | the agent's prose, markdown |
| `UserMessage` | the person's words and their attachments |
| `ToolCall` | name, the one line of detail, the result's state; a file path takes its icon from `catppuccinIcon.js` |
| `Reasoning` | folded, `data-attention="quiet"` |
| `PermissionRequest` | the only loud thing on the panel |
| `TurnResult` | tokens, cost, duration — mono |
| `Composer` | input, attachments, send, stop |

The panel's header carries the session's identity — `Profile::label()`, the model, the
folder, the state. The vocabulary is uniform; the session is not, and saying which agent is
talking is the header's job.

There is **no glyph for an agent brand**. `core/icons.js` is lucide and has no Claude or
Codex mark, and vendoring one would be a third exception to "no pictures" after the app
icon and the Catppuccin file icons. The label in mono is the answer, and it works for the
next agent on the day it is added.

`PermissionRequest` is the one component drawing at `loud`. That is within the system's
standing budget of one or two loud rows a screen — a session has at most one unanswered
question — and it is the reason the budget exists.

### Markdown

Parsed with `@lezer/markdown`, already in the tree under `@codemirror/lang-markdown`, into
a tree that Vue components render. **Never `v-html`.** The agent's prose routinely quotes
output it did not write, and injecting that as markup in a webview holding Tauri's IPC is a
real hole rather than a theoretical one. A parsed tree closes it by construction, and it
also obeys the system's own rule — no classes, inline style objects of token references.

Fenced code goes to CodeMirror read-only, which this project has already wired with a theme
and a language table.

The parser-to-tree step is a pure module under `src/components/conversation/`, so a test can
reach it. The rendering is the component's.

### Design source

The components are ports in the same sense as everything else in `src/components/`: values
are `var(--token)` references in computed style objects, no scoped CSS, no classes. All
eight go into `views/Gallery.vue` — this project has no component test runner, and
`?view=gallery` across the four theme × density combinations is the only check there is.

## Lifecycle

**The conversation id is still minted by the app.** `terminal::conversation::new_id` and
`--session-id` work in the headless form too, and that is settled by shipping code rather
than assumed: `claude::command` already puts `--session-id` on the line for a run's batch,
which is `-p --output-format stream-json` and nothing else. So the `.smetana/agents.json`
record, the offline session row and the way back into a conversation are unchanged. Resume
is `--resume <id>` alongside the same stream flags, and the existing rule that the two
flags never appear together holds unchanged.

The stored record gains one field: which transport the session ran under. Without it, a
restore after the migration would try to raise an old session by the new path.

**Restore itself is not built in the first two stages, and that is deliberate.** The
vertical those stages prove is spawn → talk → answer → draw; coming back to a session after
the app has been closed is a path of its own, with the restorable list, the offline row and
`--resume` in it, and it is worth its own stage rather than a corner of another. Until then
a driven session lives only as long as the app does, which is an honest limit for a branch
nobody ships from.

The sessions browser over `~/.claude/projects/*.jsonl` is untouched — those are somebody
else's files on disk, not our stream.

**The runs' batches are not moved on this branch, but the endpoint is designed for now:** a
batch is the same driven session with nobody watching. `agents::is_batch` eventually stops
meaning "is this stream structured" and starts meaning "is a panel drawn".

### What is deleted when the last intent has moved

`ring.rs`, `screen.rs`, `detect.rs`, `terminal/transcript.rs`, `terminal_resize`, layer B in
`agents/claude.rs` with its PTY fixtures, `components/terminal/` entire, `terminal/theme.js`,
and the xterm.js dependency. The second of the three exceptions to "every value is a token"
goes with it.

## Failure

**The first thing the branch must measure is how long Claude Code waits for a permission
answer.** `--permission-prompt-tool` is documented with `MCP_TIMEOUT` — 30 seconds — for the
server to *connect*; how long a tool *call* may take before the harness gives up is not
documented, and a person thinking about `rm -rf` takes longer than thirty seconds. If that
answer is bounded, the whole interactive model meets its ceiling there, and it has to be
known on day one rather than day five.

Everything else:

- the child dies mid-turn → an `Error` event and `failed`; the turn is never closed
- a line that does not parse → silence, as the vocabulary already decided
- a line past `MAX_LINE` → the same single note `Transcript` writes today
- the binary is not on `PATH` → the profile's existing check, unchanged
- a permission request with a bad or missing token → refused at the listener, never journalled

## Testing

- **Drivers** — `cargo test` over fixtures of real JSONL, exactly how `agents/claude.rs` is
  tested today. A fixture per event type, plus a chunk split mid-character and mid-line.
- **The journal and state rules** — `cargo test`; `model.rs` and `journal.rs` are pure.
- **The markdown tree** — vitest, as a pure module under its component directory.
- **The store** — `tests/stores/conversation.test.js` through `mockIPC`, like every other
  store, with the module graph rebuilt per test.
- **The components** — by eye in `?view=gallery`, four theme × density combinations. There
  is no component runner and none is invented for this.
- **`service.rs` and `permission.rs`** get no unit test: I/O and orchestration, the same
  standing the three existing workers have.

## Global constraints

- Comments, test names, `expect`/`panic` strings and log lines in **English**. Commit
  messages in Russian, subjects unchanged in form.
- One new dependency, named and argued above: `axum` for the permission listener, with
  `tokio`'s `net` feature turned on. Nothing else. `@lezer/markdown` is already in the tree
  under `@codemirror/lang-markdown`, and the JSON-RPC codec is `serde_json`, already a
  direct dependency.
- Every visual value is a `var(--token)` reference in a computed style object. No `<style>`
  block, no class, no hardcoded colour, radius, spacing or font.
- Sentence case in UI copy; identifiers in mono, prose in sans.
- No gradients, glass, blur or emoji.
