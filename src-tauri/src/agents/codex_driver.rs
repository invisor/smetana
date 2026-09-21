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
    opening: Option<Input>,
    queued: Vec<Vec<u8>>,
    startup: Option<Result<(), String>>,
    launch: std::sync::Mutex<(String, Option<String>)>,
    active_turn: Option<String>,
    tickets: std::collections::BTreeMap<String, (Value, String)>,
    items: std::collections::BTreeMap<String, String>,
    reasoning: std::collections::BTreeMap<String, Vec<String>>,
    /// The latest per-turn app-server token breakdown, reported on completion.
    usage: (u64, u64),
    pending: std::collections::BTreeMap<u64, String>,
    interrupt_pending: bool,
    turn_start_pending: bool,
}

impl CodexDriver {
    pub fn new(_permission: Option<crate::session::permission::PermissionTicket>) -> Self {
        Self { lines: LineBuffer::new(), next_id: 1, thread: None, opening: None, queued: Vec::new(), startup: None, launch: std::sync::Mutex::new((String::new(), None)), active_turn: None, tickets: std::collections::BTreeMap::new(), items: std::collections::BTreeMap::new(), reasoning: std::collections::BTreeMap::new(), usage: (0, 0), pending: std::collections::BTreeMap::new(), interrupt_pending: false, turn_start_pending: false }
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
            *state = (launch.cwd.to_string_lossy().into_owned(), launch.model.clone());
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
            // A server request can use a numeric id that collides with ours.
            // It is a request because it has `method`, never a response.
            let response = if message.get("method").is_none() && (message.get("result").is_some() || message.get("error").is_some()) {
                message.get("id").and_then(Value::as_u64).and_then(|id| self.pending.remove(&id))
            } else { None };
            if let (Some(method), Some(error)) = (response.as_deref(), message.get("error")) {
                let text = error.get("message").and_then(Value::as_str).unwrap_or("Codex app-server protocol error").to_owned();
                if matches!(method, "initialize" | "thread/start") { self.startup = Some(Err(text.clone())); }
                if method == "turn/start" {
                    self.turn_start_pending = false;
                    self.interrupt_pending = false;
                    events.push(EventKind::TurnFailed { text });
                } else { events.push(EventKind::Error { text }); }
                continue;
            }
            if response.as_deref() == Some("initialize") {
                let initialized = Self::notification("initialized", json!({}));
                let (cwd, model) = self.launch.lock().map(|state| state.clone()).unwrap_or_default();
                // Every intent this driver serves is an attended one — `Bare`
                // and `NewTask`, never the `Auto` run that earns the wider
                // bypass — so the workspace sandbox this thread starts under
                // is never left to whatever `~/.codex/config.toml` happens to
                // say. `codex.rs`'s own PTY road pins the same policy with
                // `every_non_auto_launch_explicitly_sandboxes_its_current_workspace`.
                let thread = self.request(
                    "thread/start",
                    json!({"cwd":cwd, "model":model, "sandbox":"workspace-write"}),
                );
                self.queued.extend([initialized, thread]);
                continue;
            }
            if response.as_deref() == Some("thread/start") {
                if let Some(id) = message.pointer("/result/thread/id").and_then(Value::as_str) {
                    self.thread = Some(id.to_owned());
                    self.startup = Some(Ok(()));
                    if let Some(opening) = self.opening.take() {
                        let turn = self.turn(opening);
                        self.queued.push(turn);
                    }
                    continue;
                }
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
                    let (name, detail) = match kind.as_str() {
                        "commandExecution" => ("commandExecution", item.get("command").and_then(Value::as_str).unwrap_or("Command")),
                        "fileChange" => ("fileChange", item.pointer("/changes/0/path").and_then(Value::as_str).unwrap_or("File change")),
                        "mcpToolCall" => ("mcpToolCall", item.get("tool").and_then(Value::as_str).unwrap_or("MCP tool")),
                        "dynamicToolCall" => ("dynamicToolCall", item.get("tool").and_then(Value::as_str).unwrap_or("Tool")),
                        "webSearch" => ("webSearch", item.get("query").and_then(Value::as_str).unwrap_or("Web search")),
                        _ => continue,
                    };
                    if !id.is_empty() {
                        events.push(EventKind::ToolUse { id, name: name.into(), detail: detail.into() });
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
                            let output = item.get("aggregatedOutput").or_else(|| item.get("output")).and_then(Value::as_str).unwrap_or("");
                            let ok = !matches!(item.get("status").and_then(Value::as_str), Some("failed" | "declined"));
                            events.push(EventKind::ToolResult { id, ok, summary: output.lines().next().unwrap_or("").to_owned() });
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

    fn startup(&mut self) -> Option<Result<(), String>> { self.startup.take() }
    fn awaits_startup(&self) -> bool { true }

    fn send(&mut self, input: Input) -> Vec<u8> {
        if self.thread.is_some() { return self.turn(input); }
        self.opening = Some(input);
        self.request("initialize", json!({"clientInfo":{"name":"smetana","version":"1"}, "capabilities":{}}))
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
}

#[cfg(test)]
mod tests {
    use super::*;

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
            r#"{"jsonrpc":"2.0","method":"item/fileChange/patchUpdated","params":{"itemId":"fc","changes":[{"diff":"@@","kind":"update","path":"a"}],"threadId":"t","turnId":"turn"}}"#, "\n"
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
}
