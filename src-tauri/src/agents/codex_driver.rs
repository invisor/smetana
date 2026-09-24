//! Codex app-server JSON-RPC codec.

use portable_pty::CommandBuilder;
use serde_json::{json, Value};

use crate::agents::{codex::Codex, Intent, Launch};
use crate::session::driver::{Driver, Input, LineBuffer};
use crate::session::model::{Decision, EventKind};

pub struct CodexDriver {
    lines: LineBuffer,
    next_id: u64,
    thread: Option<String>,
    /// Turns waiting for a thread to exist — the app-server's `initialize`
    /// handshake is asynchronous, so a message this driver is handed before
    /// `self.thread` is set has nowhere to go yet. A queue and not a slot:
    /// nothing in `send`'s own contract may assume it is called at most once
    /// before the thread exists, so a second message arriving there is
    /// remembered rather than silently replacing the first, which was the
    /// defect a slot would reintroduce (finding 3, review pass 1 of
    /// smetana-gb7f.2). Drained one at a time, oldest first — `flush_opening`
    /// is the one place that does it — because two `turn/start` requests
    /// fired at once on the same thread before either has a `turnId` back is
    /// not a shape this protocol was asked to accept.
    opening: std::collections::VecDeque<Input>,
    queued: Vec<Vec<u8>>,
    startup: Option<Result<(), String>>,
    /// The invocation-local state `feed` needs once `initialize` answers: the
    /// cwd and model every thread carries, and — for a `ResumeSession` —
    /// which thread to reopen and whether to fork it. Set by `start` (the
    /// first two) and by `reopen` (the third); read back out whole rather
    /// than piecemeal, since `feed` needs all three at once to choose between
    /// `thread/start`, `thread/resume` and `thread/fork`.
    launch: std::sync::Mutex<(String, Option<String>, Option<(String, bool)>)>,
    /// Whether `initialize` has already been dispatched — by `reopen` at
    /// spawn, for a resume or a fork, or by the first `send` otherwise.
    /// Without it, a second call reaching `send` before the thread exists
    /// would fire a second `initialize` racing the first for the same reply.
    ///
    /// **Not reachable through the front end today, and the guard is kept
    /// anyway.** `awaits_startup` answers `true` unconditionally for this
    /// driver, so `session_start`'s own reply — the session id everything
    /// else needs — is withheld by `session::service` until the handshake
    /// settles; nothing can call `session_send` without one. What this
    /// actually protects is the driver's own contract against a caller that
    /// does not lean on that gate — a test exercising `send` directly, or a
    /// future change to `Request::Start`'s own wait — rather than a path a
    /// person can reach today.
    bootstrapped: bool,
    /// The thread id, the moment `thread/start`, `thread/resume` or
    /// `thread/fork` hands one back — taken once by `discovered_id`, which is
    /// what lets `session::service` write the `.smetana/agents.json` record
    /// the spawn could not, the way `terminal::service`'s own
    /// `Request::SessionIdFound` does for the PTY road. Set unconditionally
    /// in that arm rather than guarded, because the arm itself only ever
    /// runs once in a driver's life — one thread is started, resumed or
    /// forked per session, never several.
    discovered: Option<String>,
    active_turn: Option<String>,
    tickets: std::collections::BTreeMap<String, (Value, String)>,
    items: std::collections::BTreeMap<String, String>,
    reasoning: std::collections::BTreeMap<String, Vec<String>>,
    /// The latest per-turn app-server token breakdown, reported on completion.
    usage: (u64, u64),
    pending: std::collections::BTreeMap<u64, String>,
    interrupt_pending: bool,
    turn_start_pending: bool,
    crew_records: Vec<Value>,
}

impl CodexDriver {
    pub fn new(_permission: Option<crate::session::permission::PermissionTicket>) -> Self {
        Self { lines: LineBuffer::new(), next_id: 1, thread: None, opening: std::collections::VecDeque::new(), queued: Vec::new(), startup: None, launch: std::sync::Mutex::new((String::new(), None, None)), bootstrapped: false, discovered: None, active_turn: None, tickets: std::collections::BTreeMap::new(), items: std::collections::BTreeMap::new(), reasoning: std::collections::BTreeMap::new(), usage: (0, 0), pending: std::collections::BTreeMap::new(), interrupt_pending: false, turn_start_pending: false, crew_records: Vec::new() }
    }

    /// The oldest queued message, if any, sent as the next turn. The one
    /// place `opening` is drained, called once a thread exists and again
    /// every time a turn ends, so a second message that queued up behind the
    /// first is not dropped but sent as its own turn once the one ahead of
    /// it is out of the way.
    fn flush_opening(&mut self) {
        if let Some(opening) = self.opening.pop_front() {
            let turn = self.turn(opening);
            self.queued.push(turn);
        }
    }

    fn request(&mut self, method: &str, params: Value) -> Vec<u8> {
        let id = self.next_id;
        self.next_id += 1;
        self.pending.insert(id, method.to_owned());
        let mut bytes = serde_json::to_vec(&json!({"jsonrpc":"2.0", "id":id, "method":method, "params":params})).unwrap_or_default();
        bytes.push(b'\n');
        bytes
    }

    fn notification(method: &str, params: Value) -> Vec<u8> {
        let mut bytes = serde_json::to_vec(&json!({"jsonrpc":"2.0", "method":method, "params":params})).unwrap_or_default();
        bytes.push(b'\n');
        bytes
    }

    fn turn(&mut self, input: Input) -> Vec<u8> {
        let Input::Message { text, attachments } = input;
        let mut content = vec![json!({"type":"text", "text":text})];
        content.extend(attachments.into_iter().map(|path| json!({"type":"localImage", "path":path})));
        self.turn_start_pending = true;
        self.usage = (0, 0);
        self.request("turn/start", json!({"threadId":self.thread, "input":content}))
    }
}

impl Driver for CodexDriver {
    fn start(&self, launch: &Launch) -> CommandBuilder {
        if let Ok(mut state) = self.launch.lock() {
            state.0 = launch.cwd.to_string_lossy().into_owned();
            state.1 = launch.model.clone();
        }
        let mut command = CommandBuilder::new("codex");
        command.arg("app-server");
        command
    }

    fn opening(&self, launch: &Launch) -> Option<String> {
        if matches!(launch.intent, Intent::ResumeSession { .. }) { return None; }
        // `Codex::prompt_text` is the seam that answers this directly, the
        // same one `claude.rs` cuts for `ClaudeDriver`: asking for the text
        // rather than building a whole `CommandBuilder` and reading its last
        // argument back stops this from assuming the prompt is always the
        // final positional one, an assumption a flag landing after it on the
        // PTY road's own command line would break silently.
        Codex.prompt_text(launch)
    }

    fn feed(&mut self, bytes: &[u8]) -> Vec<EventKind> {
        let mut events = Vec::new();
        for line in self.lines.feed(bytes) {
            let Ok(message) = serde_json::from_str::<Value>(&line) else { continue };
            if matches!(
                message.get("method").and_then(Value::as_str),
                Some("thread/started" | "thread/status/changed")
            ) || message
                .pointer("/params/item/type")
                .and_then(Value::as_str)
                == Some("collabAgentToolCall")
            {
                self.crew_records.push(message.clone());
            }
            // A server request can use a numeric id that collides with ours.
            // It is a request because it has `method`, never a response.
            let response = if message.get("method").is_none() && (message.get("result").is_some() || message.get("error").is_some()) {
                message.get("id").and_then(Value::as_u64).and_then(|id| self.pending.remove(&id))
            } else { None };
            if let (Some(method), Some(error)) = (response.as_deref(), message.get("error")) {
                let text = error.get("message").and_then(Value::as_str).unwrap_or("Codex app-server protocol error").to_owned();
                // A resumed or forked thread that the app-server can no
                // longer find — an expired session, a deleted rollout —
                // answers here rather than at `thread/start`, and it settles
                // the startup promise the same way: an understandable error,
                // never a silent fall-through into a fresh conversation
                // wearing the old one's name.
                if matches!(method, "initialize" | "thread/start" | "thread/resume" | "thread/fork" | "thread/read") { self.startup = Some(Err(text.clone())); }
                if method == "turn/start" {
                    self.turn_start_pending = false;
                    self.interrupt_pending = false;
                    events.push(EventKind::TurnFailed { text });
                } else { events.push(EventKind::Error { text }); }
                continue;
            }
            if response.as_deref() == Some("initialize") {
                let initialized = Self::notification("initialized", json!({}));
                let (cwd, model, resume) = self.launch.lock().map(|state| state.clone()).unwrap_or_default();
                // Every intent this driver serves is an attended one — never
                // the `Auto` run that earns the wider bypass, since
                // `session::service::drivable` refuses `Intent::Run` outright
                // — so the workspace sandbox this thread starts under is
                // never left to whatever `~/.codex/config.toml` happens to
                // say. `codex.rs`'s own PTY road pins the same policy with
                // `every_non_auto_launch_explicitly_sandboxes_its_current_workspace`,
                // and it is pinned here on all three roads into a thread —
                // starting one, resuming one and forking one alike — rather
                // than on `thread/start` alone.
                let next = match resume {
                    Some((id, true)) => self.request(
                        "thread/fork",
                        json!({"threadId":id, "cwd":cwd, "model":model, "sandbox":"workspace-write"}),
                    ),
                    Some((id, false)) => self.request(
                        "thread/resume",
                        json!({"threadId":id, "cwd":cwd, "model":model, "sandbox":"workspace-write"}),
                    ),
                    None => self.request(
                        "thread/start",
                        json!({"cwd":cwd, "model":model, "sandbox":"workspace-write"}),
                    ),
                };
                self.queued.extend([initialized, next]);
                continue;
            }
            if matches!(response.as_deref(), Some("thread/start" | "thread/resume" | "thread/fork")) {
                if let Some(id) = message.pointer("/result/thread/id").and_then(Value::as_str) {
                    self.thread = Some(id.to_owned());
                    // Handed to `discovered_id` regardless of which of the
                    // three roads produced it: a fresh `thread/start` has
                    // never had an id recorded at all, and a `thread/fork`
                    // hands back one the caller's own `Intent::ResumeSession`
                    // never named — `session::service` guards on whether it
                    // already knows one, so a plain resume's redundant report
                    // of the id it was already given is a harmless no-op
                    // there rather than something to filter out here.
                    self.discovered = Some(id.to_owned());
                    if response.as_deref() == Some("thread/start") {
                        self.startup = Some(Ok(()));
                        self.flush_opening();
                    } else {
                        // A resume and a fork both reopen a conversation that
                        // already has words in it, and those words are worth
                        // showing before anything new goes out — `thread/read`
                        // with `includeTurns: true` is the app-server's own
                        // account of them, asked for on its own rather than
                        // trusted to whatever `result/thread/turns` this reply
                        // itself carries, so the translation lives in one
                        // place regardless of which of the two roads got
                        // here. The startup promise, and any turn a person
                        // already typed while this was connecting, both wait
                        // for that reply.
                        let read = self.request(
                            "thread/read",
                            json!({"threadId":id, "includeTurns":true}),
                        );
                        self.queued.push(read);
                    }
                    continue;
                }
            }
            if response.as_deref() == Some("thread/read") {
                if let Some(turns) = message.pointer("/result/thread/turns").and_then(Value::as_array) {
                    events.extend(translate_history(turns));
                }
                self.startup = Some(Ok(()));
                self.flush_opening();
                continue;
            }
            if response.as_deref() == Some("turn/start") {
                if let Some(id) = message.pointer("/result/turn/id").and_then(Value::as_str) {
                    self.active_turn = Some(id.to_owned());
                    self.turn_start_pending = false;
                    if self.interrupt_pending {
                        self.interrupt_pending = false;
                        let thread_id = self.thread.clone().unwrap_or_default();
                        let interrupt = self.request("turn/interrupt", json!({"threadId":thread_id, "turnId":id}));
                        self.queued.push(interrupt);
                    }
                    continue;
                }
            }
            match message.get("method").and_then(Value::as_str) {
                Some("item/started") => if let Some(item) = message.pointer("/params/item") {
                    let id = item.get("id").and_then(Value::as_str).unwrap_or("").to_owned();
                    let kind = item.get("type").and_then(Value::as_str).unwrap_or("").to_owned();
                    if !id.is_empty() { self.items.insert(id.clone(), kind.clone()); }
                    if !id.is_empty() {
                        if let Some((name, detail)) = tool_use_detail(&kind, item) {
                            events.push(EventKind::ToolUse { id, name: name.into(), detail });
                        }
                    }
                },
                Some(method @ ("item/commandExecution/requestApproval" | "item/fileChange/requestApproval" | "item/tool/requestUserInput")) => {
                    let Some(id) = message.get("id").cloned() else { continue };
                    let key = id.to_string();
                    let params = message.get("params").cloned().unwrap_or(Value::Null);
                    let (tool, detail, input) = match method {
                        "item/commandExecution/requestApproval" => ("command".to_owned(), params.pointer("/command").and_then(Value::as_str).unwrap_or("Command").to_owned(), Value::Null),
                        "item/fileChange/requestApproval" => ("file-change".to_owned(), params.get("reason").and_then(Value::as_str).unwrap_or("File change").to_owned(), Value::Null),
                        _ => ("AskUserQuestion".to_owned(), "Input requested".to_owned(), json!({"questions": params.get("questions").cloned().unwrap_or(Value::Array(vec![]))})),
                    };
                    self.tickets.insert(key.clone(), (id, method.to_owned()));
                    events.push(EventKind::Permission { id: key, tool, detail, options: vec![Decision::Allow, Decision::Deny], input });
                }
                Some("item/agentMessage/delta") => {
                    if let Some(text) = message.pointer("/params/delta").and_then(Value::as_str) { events.push(EventKind::TextDelta { text: text.to_owned() }); }
                }
                Some("item/reasoning/textDelta") | Some("item/reasoning/summaryTextDelta") => {
                    if let (Some(id), Some(text)) = (message.pointer("/params/itemId").and_then(Value::as_str), message.pointer("/params/delta").and_then(Value::as_str).filter(|text| !text.is_empty())) { self.reasoning.entry(id.to_owned()).or_default().push(text.to_owned()); }
                }
                // `item/commandExecution/outputDelta` and
                // `item/fileChange/outputDelta` used to turn every chunk of
                // stdout into its own `ToolResult`, which spends
                // `Journal::BUDGET` (4000) on a single noisy command — a
                // `cargo test` or an `npm install` inside a turn is hundreds
                // to thousands of them. `item/completed` below already
                // supplies the authoritative result, so nothing here forwards
                // a delta at all; live output needs its own event kind and its
                // own collapse, the way `TextDelta` got one, and that is a
                // later task. `item/fileChange/patchUpdated` went with them
                // rather than being repaired: it read `/params/delta` and
                // `/params/patch`, but `FileChangePatchUpdatedNotification`
                // carries neither — its shape is `{changes: [{diff, kind,
                // path}], itemId, threadId, turnId}` — so the arm had never
                // once produced an event.
                Some("item/completed") => if let Some(item) = message.get("params").and_then(|p| p.get("item")) {
                    match item.get("type").and_then(Value::as_str) {
                        Some("agentMessage") => if let Some(text) = item.get("text").and_then(Value::as_str) { events.push(EventKind::Text { text: text.to_owned() }); },
                        Some("reasoning") => {
                            let id = item.get("id").and_then(Value::as_str).unwrap_or("");
                            let text = item.get("summary").and_then(Value::as_array).into_iter().flatten().chain(item.get("content").and_then(Value::as_array).into_iter().flatten()).filter_map(Value::as_str).filter(|text| !text.is_empty()).map(str::to_owned).collect::<Vec<_>>();
                            let text = if text.is_empty() { self.reasoning.remove(id).unwrap_or_default().join("\n") } else { self.reasoning.remove(id); text.join("\n") };
                            if !text.is_empty() { events.push(EventKind::Reasoning { text }); }
                        },
                        Some(kind @ ("commandExecution" | "fileChange" | "mcpToolCall" | "dynamicToolCall" | "webSearch")) => {
                            let id = item.get("id").and_then(Value::as_str).unwrap_or(kind).to_owned();
                            self.items.remove(&id);
                            let (ok, summary) = tool_result_outcome(kind, item);
                            events.push(EventKind::ToolResult { id, ok, summary });
                        },
                        _ => {}
                    }
                },
                Some("thread/tokenUsage/updated") => if let Some(usage) = message.pointer("/params/tokenUsage/last") {
                    self.usage = (usage.get("inputTokens").and_then(Value::as_u64).unwrap_or(0), usage.get("outputTokens").and_then(Value::as_u64).unwrap_or(0));
                },
                // The app-server's own answer that a server request has been
                // settled — by the person, or by the app-server abandoning it
                // when the turn that asked it ends. Keyed exactly as `tickets`
                // is, on the id's own JSON rendering, so a ticket the panel is
                // still holding open is dropped here rather than pinning
                // `state_of` at `needs-you` for a card the app-server has
                // already moved past.
                Some("serverRequest/resolved") => {
                    let request_id = message.pointer("/params/requestId").cloned().unwrap_or(Value::Null);
                    let key = request_id.to_string();
                    if self.tickets.remove(&key).is_some() {
                        events.push(EventKind::PermissionAnswered { id: key, decision: Decision::Deny, answers: None });
                    }
                }
                Some("turn/completed") => {
                    self.active_turn = None;
                    self.turn_start_pending = false;
                    // Any ticket still standing belongs to a question this
                    // turn's own end has made moot — Stop rather than Deny or
                    // Allow, say — and `Journal::trim` never drops an
                    // unanswered `Permission`, so leaving it open here would
                    // pin the session at `needs-you` for good.
                    for id in std::mem::take(&mut self.tickets).into_keys() {
                        events.push(EventKind::PermissionAnswered { id, decision: Decision::Deny, answers: None });
                    }
                    let failed = message.pointer("/params/turn/status").and_then(Value::as_str) == Some("failed");
                    if let Some(error) = message.pointer("/params/turn/error/message").and_then(Value::as_str) {
                        events.push(EventKind::TurnFailed { text: error.to_owned() });
                    } else if failed {
                        events.push(EventKind::TurnFailed { text: "Codex turn failed".into() });
                    } else if !failed {
                        events.push(EventKind::Result { tokens_in: self.usage.0, tokens_out: self.usage.1, cost_usd: None, ms: message.pointer("/params/turn/durationMs").and_then(Value::as_u64).unwrap_or(0) });
                    }
                    // A second message queued up behind the one that just
                    // finished — reachable only through `send`'s own defensive
                    // branch (see `bootstrapped`'s header), never through the
                    // ordinary front end — is sent now rather than left
                    // waiting for a person to press anything again.
                    self.flush_opening();
                },
                Some("error") => if let Some(text) = message.pointer("/params/error/message").and_then(Value::as_str) { events.push(EventKind::Error { text: text.to_owned() }); },
                Some(method) if message.get("id").is_some() => {
                    // A JSON-RPC request this driver does not implement.
                    // `feed` used to fall through here silently, which left the
                    // app-server waiting on a reply that would never come —
                    // the turn stalls with nothing on screen to say why.
                    // `session::permission` answers the same way for its own
                    // unrecognised methods.
                    let id = message.get("id").cloned().unwrap_or(Value::Null);
                    let mut bytes = serde_json::to_vec(&json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "error": { "code": -32601, "message": format!("this driver does not implement {method}") },
                    })).unwrap_or_default();
                    bytes.push(b'\n');
                    self.queued.push(bytes);
                }
                _ => {}
            }
        }
        events
    }

    fn outgoing(&mut self) -> Vec<Vec<u8>> { std::mem::take(&mut self.queued) }

    fn crew_records(&mut self) -> Vec<Value> { std::mem::take(&mut self.crew_records) }

    fn crew_send(&mut self, provider_id: &str, text: String) -> Result<Vec<u8>, String> {
        if provider_id.is_empty() {
            return Err("the selected Codex thread has ended".into());
        }
        // This is exactly the generated app-server `turn/start` contract for
        // a child thread. It deliberately does not use `self.thread`: that is
        // the lead and using it here would redirect a completed child's draft.
        let request = crate::agents::codex_crew::addressed_turn(provider_id, &text);
        let method = request
            .get("method")
            .and_then(Value::as_str)
            .ok_or_else(|| "the Codex addressed-turn contract is malformed".to_string())?;
        let params = request
            .get("params")
            .cloned()
            .ok_or_else(|| "the Codex addressed-turn contract has no parameters".to_string())?;
        Ok(self.request(method, params))
    }

    fn startup(&mut self) -> Option<Result<(), String>> { self.startup.take() }
    fn awaits_startup(&self) -> bool { true }

    fn send(&mut self, input: Input) -> Vec<u8> {
        if self.thread.is_some() { return self.turn(input); }
        if self.bootstrapped {
            // Already connecting — a resume or a fork kicked `initialize` off
            // at spawn, or a person's own earlier message did. A second
            // `initialize` here would race the first for the same reply, so
            // this one is queued instead and becomes a turn of its own once
            // the connection is ready for it — appended rather than
            // overwriting whatever is already waiting, since two messages
            // queued here are two turns owed, never one replacing the other.
            self.opening.push_back(input);
            return Vec::new();
        }
        self.bootstrapped = true;
        self.opening.push_back(input);
        self.request("initialize", json!({"clientInfo":{"name":"smetana","version":"1"}, "capabilities":{}}))
    }

    fn reopen(&mut self, launch: &Launch) -> Option<Vec<u8>> {
        let Intent::ResumeSession { id, fork, .. } = &launch.intent else { return None };
        if self.bootstrapped { return None; }
        self.bootstrapped = true;
        if let Ok(mut state) = self.launch.lock() {
            state.2 = Some((id.clone(), *fork));
        }
        Some(self.request("initialize", json!({"clientInfo":{"name":"smetana","version":"1"}, "capabilities":{}})))
    }

    fn answer(&mut self, id: &str, decision: Decision, answers: Option<std::collections::BTreeMap<String, String>>) -> Option<Vec<u8>> {
        let (request_id, method) = self.tickets.remove(id)?;
        let response = match method.as_str() {
            "item/tool/requestUserInput" => json!({"answers": answers.unwrap_or_default().into_iter().map(|(key, value)| (key, json!({"answers":[value]}))).collect::<serde_json::Map<_, _>>() }),
            _ => json!({"decision": if matches!(decision, Decision::Deny) { "decline" } else { "accept" }}),
        };
        let mut bytes = serde_json::to_vec(&json!({"jsonrpc":"2.0", "id":request_id, "result":response})).ok()?;
        bytes.push(b'\n');
        Some(bytes)
    }

    fn interrupt(&mut self) -> Option<Vec<u8>> {
        let thread_id = self.thread.clone()?;
        let Some(turn_id) = self.active_turn.clone() else {
            if !self.turn_start_pending { return None; }
            self.interrupt_pending = true;
            return Some(Vec::new());
        };
        Some(self.request("turn/interrupt", json!({"threadId":thread_id, "turnId":turn_id})))
    }

    fn discovered_id(&mut self) -> Option<String> {
        self.discovered.take()
    }
}

/// The name and the one-line detail `ToolUse` draws for an executable
/// `ThreadItem`, read off the item's own fields rather than off the
/// notification that announced it. The schema carries `command`, `changes`,
/// `tool` and `query` on the item itself, never on `item/started`'s own
/// payload, so a completed item replayed out of `thread/read`'s history
/// (`translate_history`, below) answers exactly as it would have while it
/// was still arriving live — one function rather than a second copy of this
/// match for the history the live `item/started` arm above used to keep to
/// itself.
fn tool_use_detail(kind: &str, item: &Value) -> Option<(&'static str, String)> {
    match kind {
        "commandExecution" => Some(("commandExecution", item.get("command").and_then(Value::as_str).unwrap_or("Command").to_owned())),
        "fileChange" => Some(("fileChange", item.pointer("/changes/0/path").and_then(Value::as_str).unwrap_or("File change").to_owned())),
        "mcpToolCall" => Some(("mcpToolCall", item.get("tool").and_then(Value::as_str).unwrap_or("MCP tool").to_owned())),
        "dynamicToolCall" => Some(("dynamicToolCall", item.get("tool").and_then(Value::as_str).unwrap_or("Tool").to_owned())),
        "webSearch" => Some(("webSearch", item.get("query").and_then(Value::as_str).unwrap_or("Web search").to_owned())),
        _ => None,
    }
}

/// A resumed or forked thread's own account of itself, translated into this
/// driver's ordinary events — the same vocabulary `feed` produces live, so a
/// person reading a reopened conversation cannot tell the two halves apart.
/// `turns` is `thread/read`'s own `result/thread/turns`, each a `{id, items,
/// status}` whose `items` are the very `ThreadItem`s `item/completed`
/// already knows how to read — `tool_result_outcome` and `tool_use_detail`
/// above are shared rather than repeated for this half.
///
/// **No `TurnStart` is ever produced here, and that omission is the whole of
/// what keeps `state_of` reading a freshly reopened session as `ready`
/// rather than as a turn forever in flight.**
/// `session::history::read` keeps the identical rule for a resumed Claude
/// Code session and for the identical reason, recorded in its own header: a
/// `TurnStart` with nothing to close it is what that fold reads as a turn in
/// flight, and a resumed panel would open with the composer showing Stop and
/// no way back. Nothing here ever needs a `Result` or a `TurnFailed` to
/// close one either, because none is ever opened — a translated turn is
/// simply the agent's part of a conversation already finished, read back as
/// the same shape its live half would have produced, not replayed as the
/// turn it once was.
///
/// **Each turn's own `status` is deliberately never read, so a past turn
/// that failed replays exactly like one that succeeded — a decision rather
/// than an omission the review that added this function caught and left
/// standing.** The live path's own `turn/completed` arm turns a failed
/// status into `EventKind::TurnFailed`, but that event only means anything
/// paired with the `TurnStart` that opened the turn it closes, and this
/// function produces neither on purpose (the paragraph above). A
/// `TurnFailed` with no `TurnStart` around it would be exactly the
/// asymmetry this function otherwise takes care to avoid, drawn once for a
/// turn nobody watched fail rather than for one in flight. What a failed
/// past turn's own items still carry through untouched: each executable
/// item's own `ToolResult { ok, .. }`, off `tool_result_outcome`'s reading
/// of that item's own status, and any `agentMessage`/`reasoning` the turn
/// produced before it stopped — the parts of the account that are true
/// regardless of how the turn as a whole ended.
fn translate_history(turns: &[Value]) -> Vec<EventKind> {
    let mut events = Vec::new();
    for turn in turns {
        let Some(items) = turn.get("items").and_then(Value::as_array) else { continue };
        for item in items {
            let Some(kind) = item.get("type").and_then(Value::as_str) else { continue };
            match kind {
                "userMessage" => {
                    let content = item.get("content").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]);
                    let text = content
                        .iter()
                        .filter(|part| part.get("type").and_then(Value::as_str) == Some("text"))
                        .filter_map(|part| part.get("text").and_then(Value::as_str))
                        .collect::<Vec<_>>()
                        .join("\n");
                    let attachments = content
                        .iter()
                        .filter(|part| part.get("type").and_then(Value::as_str) == Some("localImage"))
                        .filter_map(|part| part.get("path").and_then(Value::as_str))
                        .map(str::to_owned)
                        .collect();
                    events.push(EventKind::UserMessage { text, attachments });
                }
                "agentMessage" => {
                    if let Some(text) = item.get("text").and_then(Value::as_str).filter(|text| !text.is_empty()) {
                        events.push(EventKind::Text { text: text.to_owned() });
                    }
                }
                "reasoning" => {
                    let text = item.get("summary").and_then(Value::as_array).into_iter().flatten()
                        .chain(item.get("content").and_then(Value::as_array).into_iter().flatten())
                        .filter_map(Value::as_str)
                        .filter(|text| !text.is_empty())
                        .collect::<Vec<_>>()
                        .join("\n");
                    if !text.is_empty() { events.push(EventKind::Reasoning { text }); }
                }
                kind @ ("commandExecution" | "fileChange" | "mcpToolCall" | "dynamicToolCall" | "webSearch") => {
                    let id = item.get("id").and_then(Value::as_str).unwrap_or(kind).to_owned();
                    if let Some((name, detail)) = tool_use_detail(kind, item) {
                        events.push(EventKind::ToolUse { id: id.clone(), name: name.into(), detail });
                    }
                    let (ok, summary) = tool_result_outcome(kind, item);
                    events.push(EventKind::ToolResult { id, ok, summary });
                }
                _ => {}
            }
        }
    }
    events
}

/// The outcome of a completed executable item: whether it succeeded, and the
/// one line `ToolCall.vue` draws beside the tick or the cross.
///
/// **Read off each kind's own fields, not a shape shared across the five.**
/// The five `ThreadItem` variants this driver treats as executable —
/// `commandExecution`, `fileChange`, `mcpToolCall`, `dynamicToolCall`,
/// `webSearch` — carry no common result field at all, measured against
/// `codex app-server generate-json-schema` at codex-cli 0.155.1 rather than
/// assumed: `aggregatedOutput` belongs to `commandExecution` alone, and
/// reading it (or a plain `output`, which none of the five schema variants
/// has) off any of the other four was this driver's own bug — every file
/// change, MCP call, dynamic-tool call and web search reached the panel as a
/// tick or a cross with nothing behind it, smetana-gb7f.4's review caught
/// it, and this function is the fix: one arm per kind's own shape rather
/// than one field guessed to be common to all of them.
fn tool_result_outcome(kind: &str, item: &Value) -> (bool, String) {
    match kind {
        "commandExecution" => {
            // `CommandExecutionStatus`: inProgress | completed | failed | declined.
            let ok = !matches!(item.get("status").and_then(Value::as_str), Some("failed" | "declined"));
            let summary = item.get("aggregatedOutput").and_then(Value::as_str).unwrap_or("").lines().next().unwrap_or("").to_owned();
            (ok, summary)
        }
        "fileChange" => {
            // `PatchApplyStatus`: inProgress | completed | failed | declined.
            // No `aggregatedOutput` and no `output` — the only account of what
            // happened is `changes[]`, each a `{diff, kind, path}` with no
            // summary field of its own either. The count is used whatever the
            // number of files — one, several or, on a completed item with an
            // empty list, none — so the row says the same *sort* of thing
            // regardless: a single file used to draw the diff's own first
            // line, a hunk header sitting right beside the path this row's
            // own `detail` already names, and an empty list drew nothing at
            // all, which read as a blank rather than as an answer of zero.
            let ok = !matches!(item.get("status").and_then(Value::as_str), Some("failed" | "declined"));
            let n = item.get("changes").and_then(Value::as_array).map(Vec::len).unwrap_or(0);
            let summary = format!("{n} file{} changed", if n == 1 { "" } else { "s" });
            (ok, summary)
        }
        "mcpToolCall" => {
            // `McpToolCallStatus` has no `declined` at all, and `status` is
            // the whole of `ok` — not `error`, which is independently
            // nullable and carries no stated relationship to `status` in the
            // schema, so a completed call that also happens to carry an
            // `error` (a warning inside a success, say) must not draw a
            // cross the protocol never asked for. The summary prefers
            // `error.message` only while `status` actually says `failed`:
            // a failed call's error is the whole account of what went
            // wrong and is shown ahead of whatever `result` might still
            // hold, while a completed call's own `result` is what it
            // returned and is shown ahead of an `error` riding along
            // beside it as a note — reading `error` first unconditionally
            // hid a tool's own answer behind a warning it also happened to
            // carry. `result.content` is an MCP `CallToolResult` list of
            // opaque items; its first entry's own `text` is read where one
            // exists, and a completed call with a note but no result text
            // still falls to that note rather than to nothing.
            //
            // **One MCP failure cannot be caught here, and it is not new to
            // this arm.** `McpToolCallResult` in this schema carries only
            // `content`, `structuredContent` and `_meta` — no `isError` —
            // so the ordinary MCP failure mode, a tool itself reporting
            // trouble, arrives as `status: "completed"` with the failure
            // text sitting inside `result.content[0].text` exactly like a
            // success would, and draws a tick. That signal is not on the
            // wire at this boundary, so there is nothing in this item for
            // any arm to read it off; it is the app-server's own shape,
            // not a gap in this one.
            let status = item.get("status").and_then(Value::as_str);
            let ok = status != Some("failed");
            let error_message = item.pointer("/error/message").and_then(Value::as_str);
            let result_text = item.pointer("/result/content/0/text").and_then(Value::as_str);
            let summary = if status == Some("failed") { error_message.or(result_text) } else { result_text.or(error_message) }
                .unwrap_or("")
                .lines()
                .next()
                .unwrap_or("")
                .to_owned();
            (ok, summary)
        }
        "dynamicToolCall" => {
            // `success` and `status` (inProgress | completed | failed, again
            // no `declined`) carry no documented relationship either, so
            // either one calling this a failure is enough: `success` is
            // this item's own verdict on itself and answers first, `status`
            // is asked whenever `success` has not already said no — which
            // includes the `null` a reply may leave it at, and also catches
            // a `status: "failed"` a stray `success: true` disagrees with.
            // `contentItems[]` is the closed `DynamicToolCallOutputContentItem`
            // union — `inputText`/`inputImage`/`inputAudio` — so only the
            // first is read and only its `text` where the variant has one.
            let success = item.get("success").and_then(Value::as_bool);
            let status = item.get("status").and_then(Value::as_str);
            let ok = success != Some(false) && status != Some("failed");
            let summary = item.pointer("/contentItems/0/text").and_then(Value::as_str).unwrap_or("").to_owned();
            (ok, summary)
        }
        "webSearch" => {
            // This `ThreadItem` variant carries no `status` and no `error`
            // field at all — an item this driver is told `item/completed`
            // for is the only signal there is, so `ok` cannot be derived
            // from a field the protocol never sends. `results` is opaque
            // JSON by the schema's own design ("new result fields and result
            // types can pass through without a Codex release"), so only its
            // length is read; the query stands in when there is nothing to
            // count.
            let query = item.get("query").and_then(Value::as_str).unwrap_or("");
            let summary = match item.get("results").and_then(Value::as_array) {
                Some(results) => format!("{} result{} for {query}", results.len(), if results.len() == 1 { "" } else { "s" }),
                None => query.to_owned(),
            };
            (true, summary)
        }
        _ => (true, String::new()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// A `Bare` launch, the shape `session::service::spawn_session` hands a
    /// driver for an ordinary session — the fixture `claude_driver.rs`'s own
    /// tests keep under the same name, for the same reason.
    fn launch() -> Launch {
        Launch {
            profile: &Codex,
            cwd: PathBuf::from("/tmp/project"),
            intent: Intent::Bare,
            skills: crate::agents::library::Skills {
                smetana: PathBuf::from("/app/resources/smetana"),
                superpowers: PathBuf::from("/app/resources/superpowers"),
                superpowers_installed: true,
            },
            facts: None,
            session_id: None,
            languages: crate::agents::Languages::default(),
            agent_prompt: String::new(),
            model: None,
            worker_model: None,
        }
    }

    /// A resume, never a fork: the same thread goes on being written into.
    fn resume_launch(id: &str) -> Launch {
        Launch {
            intent: Intent::ResumeSession { id: id.into(), cwd: "/tmp/project".into(), title: None, fork: false },
            ..launch()
        }
    }

    /// A fork: the same history, a new thread, the original left untouched.
    fn fork_launch(id: &str) -> Launch {
        Launch {
            intent: Intent::ResumeSession { id: id.into(), cwd: "/tmp/project".into(), title: None, fork: true },
            ..launch()
        }
    }

    #[test]
    fn fragmented_initialize_and_thread_start_keep_request_ids_separate() {
        let mut driver = CodexDriver::new(None);
        let initialize = String::from_utf8(driver.send(Input::Message { text: "task".into(), attachments: vec!["/tmp/a, b.png".into()] })).unwrap();
        assert!(initialize.contains("\"id\":1"));
        assert!(driver.feed(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{}}").is_empty());
        assert!(driver.feed(b"\n").is_empty());
        let ready = driver.outgoing();
        assert_eq!(ready.len(), 2);
        assert!(String::from_utf8_lossy(&ready[1]).contains("thread/start"));
        driver.feed(b"{\"jsonrpc\":\"2.0\",\"id\":3,\"result\":{\"thread\":{\"id\":\"wrong\"}}}\n");
        assert!(driver.outgoing().is_empty());
        driver.feed(b"{\"jsonrpc\":\"2.0\",\"id\":2,\"result\":{\"thread\":{\"id\":\"t\"}}}\n");
        let turn = String::from_utf8(driver.outgoing().pop().unwrap()).unwrap();
        assert!(turn.contains("localImage"));
        assert!(turn.contains("/tmp/a, b.png"));
    }

    // The regression matrix acceptance criterion 4 of smetana-gb7f.4 asks for
    // (successful start, refused start, fragmented input, several JSON-RPC
    // ids, text with several images, every supported result type, questions
    // to the user), for both New agent and New task. The last two words of
    // that pair are `Intent::Bare` and `Intent::NewTask`, and this driver
    // reads neither — it sees only the `Input` `terminal::service::spawn_session`
    // builds from whichever intent's `opening_words()` or `send` gave it, so
    // the two are one road here and `agents::codex`'s own
    // `prompt_text_answers_byte_for_byte_what_command_puts_on_the_line` and
    // `a_bare_session_is_the_binary_and_the_language_sentence` are where the
    // two intents' own prompts are pinned apart. What is this file's alone is
    // the JSON-RPC codec below it, so the matrix here is written once against
    // `Input::Message` rather than twice against two intents that would drive
    // it identically. `fragmented_initialize_and_thread_start_keep_request_ids_separate`
    // above already covers fragmented input and is not repeated.

    #[test]
    fn a_successful_start_settles_the_startup_promise_exactly_once() {
        let mut driver = CodexDriver::new(None);
        assert!(driver.startup().is_none(), "nothing to report before a reply arrives");
        driver.send(Input::Message { text: "task".into(), attachments: vec![] });
        assert!(driver.feed(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{}}\n").is_empty());
        assert!(driver.startup().is_none(), "initialize alone is not a usable conversation yet");
        driver.feed(b"{\"jsonrpc\":\"2.0\",\"id\":2,\"result\":{\"thread\":{\"id\":\"t\"}}}\n");
        assert_eq!(driver.startup(), Some(Ok(())));
        // `take`n rather than peeked: a second read must not repeat the same
        // answer to a caller that has already acted on it.
        assert!(driver.startup().is_none());
    }

    #[test]
    fn a_refused_start_settles_the_startup_promise_with_the_reported_text_and_stops_there() {
        // This is the tag `session::service::absorb` turns into
        // `SessionError::Spawn` — an attempt that was actually made — and
        // never into `SessionError::NotDriven`, which `startAgent` in
        // `views/DesktopApp.vue` reserves for a road that never tried
        // anything. A refusal here must reach a person as its own reason and
        // must never fall back to a PTY built from the same profile.
        let mut driver = CodexDriver::new(None);
        driver.send(Input::Message { text: "task".into(), attachments: vec![] });
        let events = driver.feed(
            br#"{"jsonrpc":"2.0","id":1,"error":{"message":"not logged in to ChatGPT"}}
"#,
        );
        assert_eq!(events, vec![EventKind::Error { text: "not logged in to ChatGPT".into() }]);
        assert_eq!(driver.startup(), Some(Err("not logged in to ChatGPT".into())));
        // Nothing left queued to send — a refused `initialize` has no
        // `thread/start` behind it to answer.
        assert!(driver.outgoing().is_empty());
    }

    #[test]
    fn a_refused_thread_start_reports_the_same_way_as_a_refused_initialize() {
        let mut driver = CodexDriver::new(None);
        driver.send(Input::Message { text: "task".into(), attachments: vec![] });
        driver.feed(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{}}\n");
        driver.outgoing();
        let events = driver.feed(
            br#"{"jsonrpc":"2.0","id":2,"error":{"message":"the workspace could not be sandboxed"}}
"#,
        );
        assert_eq!(events, vec![EventKind::Error { text: "the workspace could not be sandboxed".into() }]);
        assert_eq!(driver.startup(), Some(Err("the workspace could not be sandboxed".into())));
    }

    #[test]
    fn text_with_several_images_lists_every_one_as_its_own_local_image_entry() {
        let mut driver = CodexDriver::new(None);
        driver.send(Input::Message {
            text: "look at these".into(),
            attachments: vec!["/tmp/one.png".into(), "/tmp/two.png".into(), "/tmp/three.png".into()],
        });
        driver.feed(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{}}\n");
        driver.outgoing();
        driver.feed(b"{\"jsonrpc\":\"2.0\",\"id\":2,\"result\":{\"thread\":{\"id\":\"t\"}}}\n");
        let turn: Value = serde_json::from_slice(&driver.outgoing().pop().unwrap()).unwrap();
        let content = turn.pointer("/params/input").and_then(Value::as_array).unwrap();
        assert_eq!(content.len(), 4, "the text block plus the three images");
        assert_eq!(content[0], json!({"type": "text", "text": "look at these"}));
        assert_eq!(content[1], json!({"type": "localImage", "path": "/tmp/one.png"}));
        assert_eq!(content[2], json!({"type": "localImage", "path": "/tmp/two.png"}));
        assert_eq!(content[3], json!({"type": "localImage", "path": "/tmp/three.png"}));
    }

    /// One JSON-RPC line, `feed`'s own unit: `LineBuffer` only yields a line
    /// once it has seen the `\n` after it, so a message built with
    /// `serde_json::to_vec` alone sits in the buffer forever and `feed`
    /// answers with nothing — the empty `Vec` this once failed with reads
    /// exactly like a message this driver does not recognise.
    fn line(value: Value) -> Vec<u8> {
        let mut bytes = serde_json::to_vec(&value).unwrap();
        bytes.push(b'\n');
        bytes
    }

    fn item_completed(item: Value) -> Vec<u8> {
        line(json!({"jsonrpc": "2.0", "method": "item/completed", "params": {"item": item}}))
    }

    fn item_started(item: Value) -> Vec<u8> {
        line(json!({"jsonrpc": "2.0", "method": "item/started", "params": {"item": item}}))
    }

    // The five tests below are the acceptance-criterion-2 half of the
    // regression matrix, and every fixture in them is checked against the
    // real protocol rather than built by hand: `codex app-server
    // generate-json-schema` at codex-cli 0.155.1, the `ThreadItem` union.
    // The five executable variants share **no** result field at all past
    // `id`/`type` — the first version of this matrix invented an `"output"`
    // key on four kinds that carry no such field and drove their failure leg
    // with a `"declined"` status two of them cannot report and one of them
    // has no `status` to report anything on at all, which is exactly why it
    // passed against the bug smetana-gb7f.4's review found.
    //
    // Each test opens with the kind's own `item/started` and checks the
    // `ToolUse` detail it draws — `changes[0].path`, `tool`, `tool`, `query`
    // — before moving to `item/completed`: the rewrite that closed the bug
    // above fed only the second half, which left the detail extraction for
    // four of the five kinds pinned by no test at all. A fixture that cannot
    // occur is still worse than no fixture, so every field below — down to
    // `changes[].kind` being `{"type": "update"}` rather than the bare
    // string an earlier draft of this matrix wrote, `PatchChangeKind` being
    // an object union and not a string in the schema — is one the real
    // protocol can actually send.

    #[test]
    fn a_command_execution_completes_with_its_aggregated_output_and_its_status() {
        // `CommandExecutionThreadItem`: `aggregatedOutput: string | null`,
        // `status: CommandExecutionStatus` (inProgress | completed | failed
        // | declined). This is the one kind this driver always read
        // correctly; kept in the matrix so the five stay one list.
        let mut driver = CodexDriver::new(None);
        let started = driver.feed(&item_started(json!({
            "id": "cmd", "type": "commandExecution", "status": "inProgress",
            "command": "git status", "commandActions": [], "cwd": "/p"
        })));
        assert_eq!(started, vec![EventKind::ToolUse { id: "cmd".into(), name: "commandExecution".into(), detail: "git status".into() }]);

        let ok = driver.feed(&item_completed(json!({
            "id": "cmd", "type": "commandExecution", "status": "completed",
            "command": "git status", "commandActions": [], "cwd": "/p",
            "aggregatedOutput": "On branch main\nnothing to commit"
        })));
        assert_eq!(ok, vec![EventKind::ToolResult { id: "cmd".into(), ok: true, summary: "On branch main".into() }]);

        let declined = driver.feed(&item_completed(json!({
            "id": "cmd2", "type": "commandExecution", "status": "declined",
            "command": "rm -rf build", "commandActions": [], "cwd": "/p",
            "aggregatedOutput": null
        })));
        assert_eq!(declined, vec![EventKind::ToolResult { id: "cmd2".into(), ok: false, summary: String::new() }]);
    }

    #[test]
    fn a_file_change_completes_off_its_changes_list_never_off_an_output_field() {
        // `FileChangeThreadItem`: `changes: [{diff, kind, path}]`,
        // `status: PatchApplyStatus` (inProgress | completed | failed |
        // declined) — no `aggregatedOutput`, no `output`, nothing to read a
        // summary off but the changes themselves. `kind` is
        // `PatchChangeKind`, an object union (`{"type":"add"|"delete"|
        // "update", …}`), never a bare string.
        let mut driver = CodexDriver::new(None);
        let started = driver.feed(&item_started(json!({
            "id": "fc", "type": "fileChange", "status": "inProgress",
            "changes": [{"path": "src/main.rs", "kind": {"type": "update"}, "diff": "@@ -1 +1 @@\n-old\n+new"}]
        })));
        assert_eq!(started, vec![EventKind::ToolUse { id: "fc".into(), name: "fileChange".into(), detail: "src/main.rs".into() }]);

        // The count is used whatever the number of files — one, several, or
        // none on a completed item with an empty list — so the row says the
        // same *sort* of thing regardless: a single file used to draw the
        // diff's own first line, a hunk header sitting right beside the path
        // this row's own `detail` already names, and an empty list drew
        // nothing at all, which read as a blank rather than as an answer of
        // zero.
        let one_file = driver.feed(&item_completed(json!({
            "id": "fc", "type": "fileChange", "status": "completed",
            "changes": [{"path": "src/main.rs", "kind": {"type": "update"}, "diff": "@@ -1 +1 @@\n-old\n+new"}]
        })));
        assert_eq!(one_file, vec![EventKind::ToolResult { id: "fc".into(), ok: true, summary: "1 file changed".into() }]);

        let several_files = driver.feed(&item_completed(json!({
            "id": "fc2", "type": "fileChange", "status": "completed",
            "changes": [
                {"path": "a.rs", "kind": {"type": "update"}, "diff": "@@"},
                {"path": "b.rs", "kind": {"type": "add"}, "diff": "@@"}
            ]
        })));
        assert_eq!(several_files, vec![EventKind::ToolResult { id: "fc2".into(), ok: true, summary: "2 files changed".into() }]);

        let declined = driver.feed(&item_completed(json!({
            "id": "fc3", "type": "fileChange", "status": "declined",
            "changes": [{"path": "c.rs", "kind": {"type": "update"}, "diff": "@@"}]
        })));
        assert_eq!(declined, vec![EventKind::ToolResult { id: "fc3".into(), ok: false, summary: "1 file changed".into() }]);

        let empty = driver.feed(&item_completed(json!({
            "id": "fc4", "type": "fileChange", "status": "completed", "changes": []
        })));
        assert_eq!(empty, vec![EventKind::ToolResult { id: "fc4".into(), ok: true, summary: "0 files changed".into() }]);
    }

    #[test]
    fn an_mcp_tool_call_completes_off_its_result_or_its_error_never_a_bare_status() {
        // `McpToolCallThreadItem`: `status: McpToolCallStatus` (inProgress |
        // completed | failed — **no** `declined`), `result: McpToolCallResult
        // | null` (an MCP `CallToolResult`, `content: [...]` required),
        // `error: McpToolCallError | null` ({ message }). `error.message` is
        // the whole account of a failure and was discarded by the original
        // bug, which read `status` alone.
        let mut driver = CodexDriver::new(None);
        let started = driver.feed(&item_started(json!({
            "id": "mcp", "type": "mcpToolCall", "status": "inProgress",
            "server": "docs", "tool": "search_docs", "arguments": {"q": "vue"}
        })));
        assert_eq!(started, vec![EventKind::ToolUse { id: "mcp".into(), name: "mcpToolCall".into(), detail: "search_docs".into() }]);

        let ok = driver.feed(&item_completed(json!({
            "id": "mcp", "type": "mcpToolCall", "status": "completed",
            "server": "docs", "tool": "search_docs", "arguments": {"q": "vue"},
            "result": {"content": [{"type": "text", "text": "3 matches found"}]}
        })));
        assert_eq!(ok, vec![EventKind::ToolResult { id: "mcp".into(), ok: true, summary: "3 matches found".into() }]);

        let failed = driver.feed(&item_completed(json!({
            "id": "mcp2", "type": "mcpToolCall", "status": "failed",
            "server": "docs", "tool": "search_docs", "arguments": {"q": "vue"},
            "error": {"message": "the docs server is not responding"}
        })));
        assert_eq!(
            failed,
            vec![EventKind::ToolResult { id: "mcp2".into(), ok: false, summary: "the docs server is not responding".into() }]
        );

        // `status` alone is the verdict — the schema states no relationship
        // between it and `error`, which is independently nullable, so a
        // completed call that also happens to carry one (a warning inside a
        // success, say) must still draw a tick, with the error kept only as
        // the summary's own note rather than read as a reason to fail it.
        let completed_with_a_note = driver.feed(&item_completed(json!({
            "id": "mcp3", "type": "mcpToolCall", "status": "completed",
            "server": "docs", "tool": "search_docs", "arguments": {"q": "vue"},
            "error": {"message": "partial index, results may be stale"}
        })));
        assert_eq!(
            completed_with_a_note,
            vec![EventKind::ToolResult { id: "mcp3".into(), ok: true, summary: "partial index, results may be stale".into() }]
        );

        // A successful call that carries **both** — a result and a note —
        // shows what it found, not the warning: reading `error` first
        // unconditionally is the trap this case exists to close, since it
        // hides the more useful of two strings the row has in hand.
        let completed_with_a_result_and_a_note = driver.feed(&item_completed(json!({
            "id": "mcp4", "type": "mcpToolCall", "status": "completed",
            "server": "docs", "tool": "search_docs", "arguments": {"q": "vue"},
            "result": {"content": [{"type": "text", "text": "3 matches found"}]},
            "error": {"message": "partial index, results may be stale"}
        })));
        assert_eq!(
            completed_with_a_result_and_a_note,
            vec![EventKind::ToolResult { id: "mcp4".into(), ok: true, summary: "3 matches found".into() }]
        );
    }

    #[test]
    fn a_dynamic_tool_call_folds_its_own_success_flag_into_ok() {
        // `DynamicToolCallThreadItem`: `success: bool | null`,
        // `status: DynamicToolCallStatus` (inProgress | completed | failed —
        // again no `declined`), `contentItems: [InputText | InputImage |
        // InputAudio] | null`. Neither field is documented to agree with the
        // other, so either one calling this a failure is enough.
        let mut driver = CodexDriver::new(None);
        let started = driver.feed(&item_started(json!({
            "id": "dyn", "type": "dynamicToolCall", "status": "inProgress",
            "tool": "run_lints", "arguments": {}
        })));
        assert_eq!(started, vec![EventKind::ToolUse { id: "dyn".into(), name: "dynamicToolCall".into(), detail: "run_lints".into() }]);

        let ok = driver.feed(&item_completed(json!({
            "id": "dyn", "type": "dynamicToolCall", "status": "completed", "success": true,
            "tool": "run_lints", "arguments": {},
            "contentItems": [{"type": "inputText", "text": "no lint errors"}]
        })));
        assert_eq!(ok, vec![EventKind::ToolResult { id: "dyn".into(), ok: true, summary: "no lint errors".into() }]);

        let failed = driver.feed(&item_completed(json!({
            "id": "dyn2", "type": "dynamicToolCall", "status": "failed", "success": false,
            "tool": "run_lints", "arguments": {},
            "contentItems": [{"type": "inputText", "text": "3 lint errors"}]
        })));
        assert_eq!(failed, vec![EventKind::ToolResult { id: "dyn2".into(), ok: false, summary: "3 lint errors".into() }]);

        // `success: null` falls back to `status` rather than defaulting to
        // either outcome outright.
        let unknown_success = driver.feed(&item_completed(json!({
            "id": "dyn3", "type": "dynamicToolCall", "status": "failed", "success": null,
            "tool": "run_lints", "arguments": {}, "contentItems": null
        })));
        assert_eq!(unknown_success, vec![EventKind::ToolResult { id: "dyn3".into(), ok: false, summary: String::new() }]);

        // And a `status: "failed"` a stray `success: true` disagrees with
        // still draws a cross: taking `success` alone here is the trap this
        // test exists to close.
        let status_overrules_a_stray_success = driver.feed(&item_completed(json!({
            "id": "dyn4", "type": "dynamicToolCall", "status": "failed", "success": true,
            "tool": "run_lints", "arguments": {}, "contentItems": null
        })));
        assert_eq!(status_overrules_a_stray_success, vec![EventKind::ToolResult { id: "dyn4".into(), ok: false, summary: String::new() }]);
    }

    #[test]
    fn a_web_search_completes_as_ok_with_no_status_field_in_its_own_schema_at_all() {
        // `WebSearchThreadItem` has no `status` and no `error` — `id`,
        // `query`, an optional `action` and an opaque, nullable `results`
        // array are the whole of it. There is therefore no protocol-level
        // way for this item to report a failure of its own, and reading a
        // `status` that does not exist made the original `!matches!(…)` an
        // unconditional `true` rather than a check of anything: `ok` here is
        // a plain `true` on purpose, not a comparison that happens to answer
        // one.
        let mut driver = CodexDriver::new(None);
        let started = driver.feed(&item_started(json!({
            "id": "web", "type": "webSearch", "query": "rust async book"
        })));
        assert_eq!(started, vec![EventKind::ToolUse { id: "web".into(), name: "webSearch".into(), detail: "rust async book".into() }]);

        let with_results = driver.feed(&item_completed(json!({
            "id": "web", "type": "webSearch", "query": "rust async book",
            "results": [{"title": "Asynchronous Programming in Rust"}, {"title": "Tokio tutorial"}]
        })));
        assert_eq!(
            with_results,
            vec![EventKind::ToolResult { id: "web".into(), ok: true, summary: "2 results for rust async book".into() }]
        );

        let no_results = driver.feed(&item_completed(json!({
            "id": "web2", "type": "webSearch", "query": "an unanswerable query", "results": null
        })));
        assert_eq!(
            no_results,
            vec![EventKind::ToolResult { id: "web2".into(), ok: true, summary: "an unanswerable query".into() }]
        );
    }

    #[test]
    fn interleaved_string_request_ids_answer_each_original_request() {
        let mut driver = CodexDriver::new(None);
        let events = driver.feed(concat!(
            r#"{"jsonrpc":"2.0","id":"command-7","method":"item/commandExecution/requestApproval","params":{"command":"git status"}}"#, "\n",
            r#"{"jsonrpc":"2.0","id":8,"method":"item/fileChange/requestApproval","params":{"reason":"write file"}}"#, "\n",
            r#"{"jsonrpc":"2.0","id":"input","method":"item/tool/requestUserInput","params":{"questions":[{"id":"choice"}]}}"#, "\n"
        ).as_bytes());
        assert_eq!(events.len(), 3);
        let command = String::from_utf8(driver.answer("\"command-7\"", Decision::Allow, None).unwrap()).unwrap();
        let file = String::from_utf8(driver.answer("8", Decision::Deny, None).unwrap()).unwrap();
        let input = String::from_utf8(driver.answer("\"input\"", Decision::Allow, Some([(String::from("choice"), String::from("yes"))].into())).unwrap()).unwrap();
        assert!(command.contains("\"id\":\"command-7\""));
        assert!(file.contains("\"id\":8"));
        assert!(input.contains("\"answers\""));
    }

    #[test]
    fn turn_completion_clears_interrupt_for_the_next_turn() {
        let mut driver = CodexDriver::new(None);
        driver.thread = Some("thread".into());
        driver.active_turn = Some("turn-one".into());
        assert!(String::from_utf8(driver.interrupt().unwrap()).unwrap().contains("turn-one"));
        driver.feed(br#"{"jsonrpc":"2.0","method":"turn/completed","params":{"turn":{"durationMs":4}}}
"#);
        assert!(driver.interrupt().is_none());
        assert!(String::from_utf8(driver.send(Input::Message { text: "again".into(), attachments: vec![] })).unwrap().contains("turn/start"));
    }

    #[test]
    fn app_server_lifecycle_maps_executable_items_streams_and_usage() {
        let mut driver = CodexDriver::new(None);
        // Coalesced frames prove that userMessage, plan and unknown items do
        // not become tool calls while app-server stream notifications do.
        // `outputDelta` frames are included and produce nothing: `item/completed`
        // is the one authoritative `ToolResult`, never a second one per chunk.
        let events = driver.feed(concat!(
            r#"{"jsonrpc":"2.0","method":"item/started","params":{"item":{"id":"u","type":"userMessage"}}}"#, "\n",
            r#"{"jsonrpc":"2.0","method":"item/started","params":{"item":{"id":"plan","type":"plan"}}}"#, "\n",
            r#"{"jsonrpc":"2.0","method":"item/started","params":{"item":{"id":"cmd","type":"commandExecution","command":"git status"}}}"#, "\n",
            r#"{"jsonrpc":"2.0","method":"item/commandExecution/outputDelta","params":{"itemId":"cmd","delta":"On branch main\n"}}"#, "\n",
            r#"{"jsonrpc":"2.0","method":"item/reasoning/summaryTextDelta","params":{"itemId":"r","delta":"Checking"}}"#, "\n",
            r#"{"jsonrpc":"2.0","method":"item/completed","params":{"item":{"id":"r","type":"reasoning","summary":["Checking"]}}}"#, "\n",
            r#"{"jsonrpc":"2.0","method":"item/completed","params":{"item":{"id":"cmd","type":"commandExecution","status":"completed","aggregatedOutput":"On branch main"}}}"#, "\n",
            r#"{"jsonrpc":"2.0","method":"item/completed","params":{"item":{"id":"m","type":"agentMessage","text":"Done."}}}"#, "\n",
            r#"{"jsonrpc":"2.0","method":"thread/tokenUsage/updated","params":{"tokenUsage":{"last":{"inputTokens":12,"outputTokens":7,"reasoningOutputTokens":3}}}}"#, "\n",
            r#"{"jsonrpc":"2.0","method":"turn/completed","params":{"turn":{"status":"completed","durationMs":9}}}"#, "\n"
        ).as_bytes());
        assert_eq!(events, vec![
            EventKind::ToolUse { id: "cmd".into(), name: "commandExecution".into(), detail: "git status".into() },
            EventKind::Reasoning { text: "Checking".into() },
            EventKind::ToolResult { id: "cmd".into(), ok: true, summary: "On branch main".into() },
            EventKind::Text { text: "Done.".into() },
            EventKind::Result { tokens_in: 12, tokens_out: 7, cost_usd: None, ms: 9 },
        ]);
    }

    #[test]
    fn an_output_delta_produces_no_event_of_its_own() {
        // A `cargo test` or an `npm install` inside a turn streams hundreds to
        // thousands of these; one `ToolResult` per chunk would evict the
        // opening turn from `Journal::BUDGET` long before the command ends.
        // `item/completed` is the one authoritative result.
        let mut driver = CodexDriver::new(None);
        let events = driver.feed(concat!(
            r#"{"jsonrpc":"2.0","method":"item/started","params":{"item":{"id":"cmd","type":"commandExecution","command":"npm install"}}}"#, "\n",
            r#"{"jsonrpc":"2.0","method":"item/commandExecution/outputDelta","params":{"itemId":"cmd","delta":"added 1 package\n"}}"#, "\n",
            r#"{"jsonrpc":"2.0","method":"item/commandExecution/outputDelta","params":{"itemId":"cmd","delta":"added 2 packages\n"}}"#, "\n",
            r#"{"jsonrpc":"2.0","method":"item/fileChange/outputDelta","params":{"itemId":"fc","delta":"diff --git a b\n"}}"#, "\n",
            r#"{"jsonrpc":"2.0","method":"item/fileChange/patchUpdated","params":{"itemId":"fc","changes":[{"diff":"@@","kind":{"type":"update"},"path":"a"}],"threadId":"t","turnId":"turn"}}"#, "\n"
        ).as_bytes());
        assert_eq!(events, vec![EventKind::ToolUse { id: "cmd".into(), name: "commandExecution".into(), detail: "npm install".into() }]);
    }

    #[test]
    fn failed_turn_reports_its_error_without_a_result() {
        let mut driver = CodexDriver::new(None);
        assert_eq!(driver.feed(br#"{"jsonrpc":"2.0","method":"turn/completed","params":{"turn":{"status":"failed","error":{"message":"rate limited"}}}}
"#), vec![EventKind::TurnFailed { text: "rate limited".into() }]);
    }

    #[test]
    fn server_request_and_wrong_error_id_do_not_consume_startup_reply() {
        let mut driver = CodexDriver::new(None);
        driver.send(Input::Message { text: "task".into(), attachments: vec![] });
        // Same numeric id as initialize, but this is a server request.
        assert!(driver.feed(br#"{"jsonrpc":"2.0","id":1,"method":"item/tool/requestUserInput","params":{"questions":[]}}
"#).iter().any(|event| matches!(event, EventKind::Permission { .. })));
        assert!(driver.startup().is_none());
        // An unknown response error is not a startup failure either.
        assert!(driver.feed(br#"{"jsonrpc":"2.0","id":99,"error":{"message":"wrong"}}
"#).is_empty());
        assert!(driver.startup().is_none());
        assert!(driver.feed(br#"{"jsonrpc":"2.0","id":1,"result":{}}
"#).is_empty());
        assert_eq!(driver.outgoing().len(), 2);
    }

    #[test]
    fn thread_start_pins_the_workspace_sandbox_for_every_intent_this_driver_serves() {
        // `CodexDriver` serves only `Bare` and `NewTask` (`session::service`'s
        // `spawn` match) — an attended launch, never the `Auto` run that earns
        // the wider bypass — so the workspace sandbox this thread starts under
        // must never be left to whatever `~/.codex/config.toml` says. This
        // mirrors `codex.rs`'s own PTY-road pin,
        // `every_non_auto_launch_explicitly_sandboxes_its_current_workspace`.
        let mut driver = CodexDriver::new(None);
        driver.send(Input::Message { text: "task".into(), attachments: vec![] });
        driver.feed(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{}}\n");
        let thread_start = String::from_utf8(driver.outgoing().pop().unwrap()).unwrap();
        assert!(thread_start.contains("\"sandbox\":\"workspace-write\""), "{thread_start}");
    }

    #[test]
    fn a_ticket_still_open_when_its_turn_ends_is_answered_deny_rather_than_left_pinning_needs_you() {
        // Acceptance criterion 4: a card disappears once its turn ends, even
        // when nobody pressed Allow or Deny — Stop is the ordinary way that
        // happens. `Journal::trim` never drops an unanswered `Permission`, so
        // an event has to close it here or the session is `needs-you` for good.
        let mut driver = CodexDriver::new(None);
        let events = driver.feed(concat!(
            r#"{"jsonrpc":"2.0","id":"cmd-1","method":"item/commandExecution/requestApproval","params":{"command":"rm -rf build"}}"#, "\n",
            r#"{"jsonrpc":"2.0","method":"turn/completed","params":{"turn":{"status":"interrupted"}}}"#, "\n"
        ).as_bytes());
        assert_eq!(events[0], EventKind::Permission {
            id: "\"cmd-1\"".into(),
            tool: "command".into(),
            detail: "rm -rf build".into(),
            options: vec![Decision::Allow, Decision::Deny],
            input: Value::Null,
        });
        assert!(events.contains(&EventKind::PermissionAnswered {
            id: "\"cmd-1\"".into(),
            decision: Decision::Deny,
            answers: None,
        }));
        // The ticket is gone, so answering it a moment later — a stale click
        // on a card that should already have cleared — sends nothing.
        assert!(driver.answer("\"cmd-1\"", Decision::Allow, None).is_none());
    }

    #[test]
    fn server_request_resolved_clears_its_own_ticket_and_no_other() {
        // The protocol's own hook for a ticket the app-server has settled
        // some other way, keyed on `requestId` exactly as `tickets` is keyed
        // on the original request's own id.
        let mut driver = CodexDriver::new(None);
        let events = driver.feed(concat!(
            r#"{"jsonrpc":"2.0","id":"cmd-1","method":"item/commandExecution/requestApproval","params":{"command":"git status"}}"#, "\n",
            r#"{"jsonrpc":"2.0","id":8,"method":"item/fileChange/requestApproval","params":{"reason":"write file"}}"#, "\n",
            r#"{"jsonrpc":"2.0","method":"serverRequest/resolved","params":{"requestId":"cmd-1","threadId":"t"}}"#, "\n"
        ).as_bytes());
        assert_eq!(events[2], EventKind::PermissionAnswered { id: "\"cmd-1\"".into(), decision: Decision::Deny, answers: None });
        assert!(driver.answer("\"cmd-1\"", Decision::Allow, None).is_none(), "already resolved");
        // The other ticket is untouched by an id that is not its own.
        assert!(driver.answer("8", Decision::Deny, None).is_some(), "its own ticket is still open");
    }

    #[test]
    fn an_unhandled_server_request_is_answered_method_not_found_rather_than_left_hanging() {
        // A JSON-RPC request with no reply leaves the app-server blocked
        // indefinitely — the turn stalls with no timeout anywhere on this
        // path. `session::permission` answers its own unknown methods the
        // same way, `-32601`, "method not found".
        let mut driver = CodexDriver::new(None);
        assert!(driver.feed(br#"{"jsonrpc":"2.0","id":"x","method":"item/tool/somethingNew","params":{}}
"#).is_empty(), "not one of the handled shapes, so no card is drawn for it");
        let reply = String::from_utf8(driver.outgoing().pop().unwrap()).unwrap();
        assert!(reply.contains("\"id\":\"x\""), "{reply}");
        assert!(reply.contains("-32601"), "{reply}");
    }

    // The regression matrix below is smetana-gb7f.2's own: `ResumeSession`,
    // both plain and forked, going through app-server JSON-RPC rather than
    // the PTY road's `codex resume <id>` / `codex fork <id>` subcommands. A
    // driven resume has nothing of a person's own to open on — `opening`
    // answers `None` for it exactly as it always has — so `reopen` is what
    // this whole matrix is about: the one thing that has to kick the
    // handshake off with nothing for a person to have typed yet.

    #[test]
    fn opening_still_answers_none_for_a_resume_so_reopen_is_the_only_kickoff() {
        // Unchanged by this task: a reopened conversation already has
        // somebody's words in it, and `Driver::opening`'s own contract is
        // that nothing is composed for one. What has to change instead is
        // covered by the tests below.
        let driver = CodexDriver::new(None);
        assert!(driver.opening(&resume_launch("9f1c0a2e-0000-4000-8000-000000000000")).is_none());
        assert!(driver.opening(&fork_launch("9f1c0a2e-0000-4000-8000-000000000000")).is_none());
    }

    #[test]
    fn a_resume_requests_thread_resume_and_shows_its_history_before_anything_new() {
        let id = "9f1c0a2e-0000-4000-8000-000000000000";
        let mut driver = CodexDriver::new(None);
        let initialize = String::from_utf8(driver.reopen(&resume_launch(id)).unwrap()).unwrap();
        assert!(initialize.contains("\"method\":\"initialize\""));
        assert!(driver.startup().is_none(), "nothing to report before the handshake has even begun");

        assert!(driver.feed(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{}}\n").is_empty());
        let queued = driver.outgoing();
        assert_eq!(queued.len(), 2, "the `initialized` notification and the next request");
        let resume_request = String::from_utf8_lossy(&queued[1]).into_owned();
        assert!(resume_request.contains("\"method\":\"thread/resume\""), "{resume_request}");
        assert!(resume_request.contains(&format!("\"threadId\":\"{id}\"")), "{resume_request}");
        assert!(!resume_request.contains("thread/start"), "a resume must never fall back to starting a fresh thread");

        // The resume's own reply carries the thread back — same id, since
        // this is a resume and not a fork — and settles nothing yet: the
        // history is still to come.
        driver.feed(format!("{{\"jsonrpc\":\"2.0\",\"id\":2,\"result\":{{\"thread\":{{\"id\":\"{id}\",\"turns\":[]}}}}}}\n").as_bytes());
        assert!(driver.startup().is_none(), "the resume's own reply is not yet a usable conversation");
        let read_request = String::from_utf8(driver.outgoing().pop().unwrap()).unwrap();
        assert!(read_request.contains("\"method\":\"thread/read\""), "{read_request}");
        assert!(read_request.contains(&format!("\"threadId\":\"{id}\"")), "{read_request}");
        assert!(read_request.contains("\"includeTurns\":true"), "{read_request}");

        // `thread/read`'s own reply carries the conversation that already
        // happened, translated into the same events a live turn would have
        // produced — shown before the person has said a word since nothing
        // yet is queued to send.
        let events = driver.feed(concat!(
            r#"{"jsonrpc":"2.0","id":3,"result":{"thread":{"id":"9f1c0a2e-0000-4000-8000-000000000000","turns":[{"id":"t1","status":"completed","items":["#,
            r#"{"id":"u1","type":"userMessage","content":[{"type":"text","text":"Rename the worktree when the branch changes."}]},"#,
            r#"{"id":"cmd","type":"commandExecution","status":"completed","command":"git status","aggregatedOutput":"On branch main"},"#,
            r#"{"id":"a1","type":"agentMessage","text":"Renamed it already."}"#,
            r#"]}]}}}"#, "\n",
        ).as_bytes());
        assert_eq!(events, vec![
            EventKind::UserMessage { text: "Rename the worktree when the branch changes.".into(), attachments: Vec::new() },
            EventKind::ToolUse { id: "cmd".into(), name: "commandExecution".into(), detail: "git status".into() },
            EventKind::ToolResult { id: "cmd".into(), ok: true, summary: "On branch main".into() },
            EventKind::Text { text: "Renamed it already.".into() },
        ]);
        assert_eq!(driver.startup(), Some(Ok(())), "the history is what makes this conversation usable");
        assert!(driver.outgoing().is_empty(), "nothing typed yet, so nothing queued to send");

        // A message the person types afterwards goes on the very thread that
        // was resumed.
        let turn = String::from_utf8(driver.send(Input::Message { text: "and the test names too".into(), attachments: vec![] })).unwrap();
        assert!(turn.contains("\"method\":\"turn/start\""), "{turn}");
        assert!(turn.contains(&format!("\"threadId\":\"{id}\"")), "{turn}");
    }

    #[test]
    fn a_fork_requests_thread_fork_with_the_original_id_and_writes_new_turns_into_the_new_one() {
        let original = "9f1c0a2e-0000-4000-8000-000000000000";
        let forked = "aaaaaaaa-1111-4000-8000-000000000000";
        let mut driver = CodexDriver::new(None);
        driver.reopen(&fork_launch(original));
        driver.feed(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{}}\n");
        let fork_request = String::from_utf8_lossy(driver.outgoing().last().unwrap()).into_owned();
        assert!(fork_request.contains("\"method\":\"thread/fork\""), "{fork_request}");
        assert!(fork_request.contains(&format!("\"threadId\":\"{original}\"")), "a fork asks for the original thread's history, not a fresh one: {fork_request}");

        // The app-server hands back a *different* id: the original is left
        // exactly as it was, and this is a second, new thread.
        driver.feed(format!("{{\"jsonrpc\":\"2.0\",\"id\":2,\"result\":{{\"thread\":{{\"id\":\"{forked}\",\"turns\":[]}}}}}}\n").as_bytes());
        let read_request = String::from_utf8(driver.outgoing().pop().unwrap()).unwrap();
        assert!(read_request.contains(&format!("\"threadId\":\"{forked}\"")), "the copied history is read off the new thread: {read_request}");

        driver.feed(b"{\"jsonrpc\":\"2.0\",\"id\":3,\"result\":{\"thread\":{\"turns\":[]}}}\n");
        assert_eq!(driver.startup(), Some(Ok(())));

        // Whatever is said from here on lands in the new thread, never the
        // original one this fork was cut from.
        let turn = String::from_utf8(driver.send(Input::Message { text: "carry on from here".into(), attachments: vec![] })).unwrap();
        assert!(turn.contains(&format!("\"threadId\":\"{forked}\"")), "{turn}");
        assert!(!turn.contains(original), "the original thread is never written into by the fork: {turn}");
    }

    #[test]
    fn a_message_typed_while_reconnecting_is_queued_rather_than_restarting_the_handshake() {
        let id = "9f1c0a2e-0000-4000-8000-000000000000";
        let mut driver = CodexDriver::new(None);
        driver.reopen(&resume_launch(id));
        // A person's own message arrives before the handshake has finished —
        // it must not start a second `initialize` racing the first for the
        // same numeric reply.
        let bytes = driver.send(Input::Message { text: "any word from git status?".into(), attachments: vec![] });
        assert!(bytes.is_empty(), "remembered rather than sent a second time");

        driver.feed(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{}}\n");
        let outgoing = driver.outgoing();
        assert!(!String::from_utf8_lossy(outgoing.last().unwrap()).contains("initialize"), "no second handshake");
        driver.feed(format!("{{\"jsonrpc\":\"2.0\",\"id\":2,\"result\":{{\"thread\":{{\"id\":\"{id}\",\"turns\":[]}}}}}}\n").as_bytes());
        driver.outgoing();
        driver.feed(format!("{{\"jsonrpc\":\"2.0\",\"id\":3,\"result\":{{\"thread\":{{\"id\":\"{id}\",\"turns\":[]}}}}}}\n").as_bytes());
        // The history was empty, so the only thing left queued is the
        // message that was waiting to become the opening turn.
        let turn = String::from_utf8(driver.outgoing().pop().unwrap()).unwrap();
        assert!(turn.contains("\"method\":\"turn/start\""), "{turn}");
        assert!(turn.contains("any word from git status?"), "{turn}");
    }

    /// Finding 3, review pass 1 of smetana-gb7f.2: `opening` used to be a
    /// slot, so a second message arriving before the thread existed
    /// silently replaced the first — the composer's own `Ok` would have
    /// cleared both, and only the second was ever actually sent. This is
    /// the queue's own regression test: two messages, two turns, in order,
    /// the second withheld until the first has actually ended.
    #[test]
    fn a_second_message_queued_before_the_thread_exists_becomes_its_own_turn_once_the_first_ends() {
        let id = "9f1c0a2e-0000-4000-8000-000000000000";
        let mut driver = CodexDriver::new(None);
        driver.reopen(&resume_launch(id));
        assert!(driver.send(Input::Message { text: "first".into(), attachments: vec![] }).is_empty());
        assert!(driver.send(Input::Message { text: "second".into(), attachments: vec![] }).is_empty());

        driver.feed(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{}}\n");
        driver.outgoing();
        driver.feed(format!("{{\"jsonrpc\":\"2.0\",\"id\":2,\"result\":{{\"thread\":{{\"id\":\"{id}\",\"turns\":[]}}}}}}\n").as_bytes());
        driver.outgoing();
        driver.feed(format!("{{\"jsonrpc\":\"2.0\",\"id\":3,\"result\":{{\"thread\":{{\"id\":\"{id}\",\"turns\":[]}}}}}}\n").as_bytes());
        let turn_one = String::from_utf8(driver.outgoing().pop().unwrap()).unwrap();
        assert!(turn_one.contains("\"method\":\"turn/start\"") && turn_one.contains("first"), "{turn_one}");

        // Nothing left to send until the first turn actually ends — the
        // second message is still waiting rather than racing the first for
        // the same thread.
        assert!(driver.outgoing().is_empty());

        driver.feed(br#"{"jsonrpc":"2.0","method":"turn/completed","params":{"turn":{"status":"completed","durationMs":1}}}
"#);
        let turn_two = String::from_utf8(driver.outgoing().pop().unwrap()).unwrap();
        assert!(turn_two.contains("\"method\":\"turn/start\"") && turn_two.contains("second"), "{turn_two}");
        assert!(!turn_two.contains("first"), "{turn_two}");
    }

    #[test]
    fn a_missing_thread_refuses_with_the_reported_text_and_never_falls_back_to_a_fresh_thread_start() {
        // `session::service::absorb` turns this into `SessionError::Spawn` —
        // an attempt that was actually made — and it must never become a
        // `thread/start` in disguise: a vanished thread resumed as a fresh
        // conversation would be a new session wearing the old one's name.
        let id = "9f1c0a2e-0000-4000-8000-000000000000";
        let mut driver = CodexDriver::new(None);
        driver.reopen(&resume_launch(id));
        driver.feed(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{}}\n");
        driver.outgoing();
        let events = driver.feed(br#"{"jsonrpc":"2.0","id":2,"error":{"message":"no such thread"}}
"#);
        assert_eq!(events, vec![EventKind::Error { text: "no such thread".into() }]);
        assert_eq!(driver.startup(), Some(Err("no such thread".into())));
        assert!(driver.outgoing().is_empty(), "nothing left queued — no thread/start behind a refused resume");
    }

    #[test]
    fn a_missing_thread_on_a_fork_refuses_the_same_way() {
        let id = "9f1c0a2e-0000-4000-8000-000000000000";
        let mut driver = CodexDriver::new(None);
        driver.reopen(&fork_launch(id));
        driver.feed(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{}}\n");
        driver.outgoing();
        let events = driver.feed(br#"{"jsonrpc":"2.0","id":2,"error":{"message":"the source thread has been deleted"}}
"#);
        assert_eq!(events, vec![EventKind::Error { text: "the source thread has been deleted".into() }]);
        assert_eq!(driver.startup(), Some(Err("the source thread has been deleted".into())));
    }

    #[test]
    fn a_failed_thread_read_also_refuses_rather_than_opening_on_a_blank_history() {
        let id = "9f1c0a2e-0000-4000-8000-000000000000";
        let mut driver = CodexDriver::new(None);
        driver.reopen(&resume_launch(id));
        driver.feed(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{}}\n");
        driver.outgoing();
        driver.feed(format!("{{\"jsonrpc\":\"2.0\",\"id\":2,\"result\":{{\"thread\":{{\"id\":\"{id}\",\"turns\":[]}}}}}}\n").as_bytes());
        driver.outgoing();
        let events = driver.feed(br#"{"jsonrpc":"2.0","id":3,"error":{"message":"the rollout could not be read"}}
"#);
        assert_eq!(events, vec![EventKind::Error { text: "the rollout could not be read".into() }]);
        assert_eq!(driver.startup(), Some(Err("the rollout could not be read".into())));
    }

    #[test]
    fn resume_and_fork_both_pin_the_workspace_sandbox_too() {
        // The same pin `thread_start_pins_the_workspace_sandbox_for_every_intent_this_driver_serves`
        // holds for `thread/start`, extended to the two roads that reopen a
        // thread rather than starting one: every intent this driver serves
        // is attended, never the `Auto` run that earns the wider bypass.
        let id = "9f1c0a2e-0000-4000-8000-000000000000";
        let mut driver = CodexDriver::new(None);
        driver.reopen(&resume_launch(id));
        driver.feed(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{}}\n");
        let resume_request = String::from_utf8_lossy(driver.outgoing().last().unwrap()).into_owned();
        assert!(resume_request.contains("\"sandbox\":\"workspace-write\""), "{resume_request}");

        let mut driver = CodexDriver::new(None);
        driver.reopen(&fork_launch(id));
        driver.feed(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{}}\n");
        let fork_request = String::from_utf8_lossy(driver.outgoing().last().unwrap()).into_owned();
        assert!(fork_request.contains("\"sandbox\":\"workspace-write\""), "{fork_request}");
    }

    #[test]
    fn translate_history_never_produces_a_turn_start_so_a_reopened_session_reads_ready_not_running() {
        // `session::history::read` keeps the identical exclusion for a
        // resumed Claude Code session, and for the identical reason: a
        // `TurnStart` with nothing to close it is what `state_of` folds into
        // `running` for good.
        let turns: Vec<Value> = serde_json::from_str(concat!(
            r#"[{"id":"t1","status":"completed","items":["#,
            r#"{"id":"u1","type":"userMessage","content":[{"type":"text","text":"look at this"},{"type":"localImage","path":"/tmp/shot.png"}]},"#,
            r#"{"id":"r1","type":"reasoning","summary":["Checked the tests"]},"#,
            r#"{"id":"web","type":"webSearch","query":"rust async book","results":[{"title":"x"}]},"#,
            r#"{"id":"a1","type":"agentMessage","text":"Found it."}"#,
            r#"]}]"#,
        )).unwrap();
        let events = translate_history(&turns);
        assert_eq!(events, vec![
            EventKind::UserMessage { text: "look at this".into(), attachments: vec!["/tmp/shot.png".into()] },
            EventKind::Reasoning { text: "Checked the tests".into() },
            EventKind::ToolUse { id: "web".into(), name: "webSearch".into(), detail: "rust async book".into() },
            EventKind::ToolResult { id: "web".into(), ok: true, summary: "1 result for rust async book".into() },
            EventKind::Text { text: "Found it.".into() },
        ]);
        assert!(!events.iter().any(|event| matches!(event, EventKind::TurnStart { .. })), "{events:?}");
        assert_eq!(
            crate::session::model::state_of(
                &events.into_iter().enumerate().map(|(i, kind)| crate::session::model::Event {
                    seq: i as u64 + 1,
                    at: "2026-09-21T00:00:00Z".into(),
                    kind,
                }).collect::<Vec<_>>(),
                true,
            ),
            crate::session::model::SessionState::Ready,
            "a history with no open turn reads as ready, never as a turn forever in flight",
        );
    }
}
