//! Codex app-server JSON-RPC codec.

use portable_pty::CommandBuilder;
use serde_json::{json, Value};

use crate::agents::{codex::Codex, Intent, Launch, Profile};
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
    pending: std::collections::BTreeMap<u64, String>,
    interrupt_pending: bool,
    turn_start_pending: bool,
}

impl CodexDriver {
    pub fn new(_permission: Option<crate::session::permission::PermissionTicket>) -> Self {
        Self { lines: LineBuffer::new(), next_id: 1, thread: None, opening: None, queued: Vec::new(), startup: None, launch: std::sync::Mutex::new((String::new(), None)), active_turn: None, tickets: std::collections::BTreeMap::new(), items: std::collections::BTreeMap::new(), pending: std::collections::BTreeMap::new(), interrupt_pending: false, turn_start_pending: false }
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
        // Codex's existing profile owns construction of the complete brief.
        Codex.command(launch).get_argv().last().map(|arg| arg.to_string_lossy().into_owned())
    }

    fn feed(&mut self, bytes: &[u8]) -> Vec<EventKind> {
        let mut events = Vec::new();
        for line in self.lines.feed(bytes) {
            let Ok(message) = serde_json::from_str::<Value>(&line) else { continue };
            let response = message.get("id").and_then(Value::as_u64).and_then(|id| self.pending.remove(&id));
            if let Some(error) = message.get("error") {
                let text = error.get("message").and_then(Value::as_str).unwrap_or("Codex app-server protocol error").to_owned();
                if self.thread.is_none() { self.startup = Some(Err(text.clone())); }
                events.push(EventKind::Error { text });
                continue;
            }
            if response.as_deref() == Some("initialize") {
                let initialized = Self::notification("initialized", json!({}));
                let (cwd, model) = self.launch.lock().map(|state| state.clone()).unwrap_or_default();
                let thread = self.request("thread/start", json!({"cwd":cwd, "model":model}));
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
                    let kind = item.get("type").and_then(Value::as_str).unwrap_or("tool").to_owned();
                    if !id.is_empty() { self.items.insert(id.clone(), kind.clone()); }
                    if kind != "agentMessage" && kind != "reasoning" {
                        events.push(EventKind::ToolUse { id, name: kind, detail: item.get("command").and_then(Value::as_str).unwrap_or("").to_owned() });
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
                Some("item/completed") => if let Some(item) = message.get("params").and_then(|p| p.get("item")) {
                    match item.get("type").and_then(Value::as_str) {
                        Some("agentMessage") => if let Some(text) = item.get("text").and_then(Value::as_str) { events.push(EventKind::Text { text: text.to_owned() }); },
                        Some("reasoning") => if let Some(text) = item.get("text").and_then(Value::as_str) { events.push(EventKind::Reasoning { text: text.to_owned() }); },
                        Some(kind) => {
                            let id = item.get("id").and_then(Value::as_str).unwrap_or(kind).to_owned();
                            self.items.remove(&id);
                            events.push(EventKind::ToolResult { id, ok: item.get("status").and_then(Value::as_str) != Some("failed"), summary: item.get("output").and_then(Value::as_str).unwrap_or("").lines().next().unwrap_or("").to_owned() });
                        },
                        None => {}
                    }
                },
                Some("turn/completed") => {
                    self.active_turn = None;
                    self.turn_start_pending = false;
                    let failed = message.pointer("/params/turn/status").and_then(Value::as_str) == Some("failed");
                    if let Some(error) = message.pointer("/params/turn/error/message").and_then(Value::as_str) {
                        events.push(EventKind::Error { text: error.to_owned() });
                    } else if failed {
                        events.push(EventKind::Error { text: "Codex turn failed".into() });
                    } else if !failed {
                        events.push(EventKind::Result { tokens_in: 0, tokens_out: 0, cost_usd: None, ms: message.pointer("/params/turn/durationMs").and_then(Value::as_u64).unwrap_or(0) });
                    }
                },
                Some("error") => if let Some(text) = message.pointer("/params/error/message").and_then(Value::as_str) { events.push(EventKind::Error { text: text.to_owned() }); },
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
}
