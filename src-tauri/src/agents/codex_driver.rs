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
}

impl CodexDriver {
    pub fn new(_permission: Option<crate::session::permission::PermissionTicket>) -> Self {
        Self { lines: LineBuffer::new(), next_id: 1, thread: None, opening: None, queued: Vec::new(), startup: None }
    }

    fn request(&mut self, method: &str, params: Value) -> Vec<u8> {
        let id = self.next_id;
        self.next_id += 1;
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
        self.request("turn/start", json!({"threadId":self.thread, "input":content}))
    }
}

impl Driver for CodexDriver {
    fn start(&self, _launch: &Launch) -> CommandBuilder {
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
            if let Some(error) = message.get("error") {
                let text = error.get("message").and_then(Value::as_str).unwrap_or("Codex app-server protocol error").to_owned();
                if self.thread.is_none() { self.startup = Some(Err(text.clone())); }
                events.push(EventKind::Error { text });
                continue;
            }
            if message.get("id").and_then(Value::as_u64) == Some(1) {
                let initialized = Self::notification("initialized", json!({}));
                let thread = self.request("thread/start", json!({}));
                self.queued.extend([initialized, thread]);
                continue;
            }
            if let Some(id) = message.pointer("/result/thread/id").and_then(Value::as_str) {
                self.thread = Some(id.to_owned());
                self.startup = Some(Ok(()));
                if let Some(opening) = self.opening.take() {
                    let turn = self.turn(opening);
                    self.queued.push(turn);
                }
                continue;
            }
            match message.get("method").and_then(Value::as_str) {
                Some("item/agentMessage/delta") => {
                    if let Some(text) = message.pointer("/params/delta").and_then(Value::as_str) { events.push(EventKind::TextDelta { text: text.to_owned() }); }
                }
                Some("item/completed") => if let Some(item) = message.get("params").and_then(|p| p.get("item")) {
                    match item.get("type").and_then(Value::as_str) {
                        Some("agentMessage") => if let Some(text) = item.get("text").and_then(Value::as_str) { events.push(EventKind::Text { text: text.to_owned() }); },
                        Some(kind) => events.push(EventKind::ToolUse { id: item.get("id").and_then(Value::as_str).unwrap_or(kind).to_owned(), name: kind.to_owned(), detail: String::new() }),
                        None => {}
                    }
                },
                Some("turn/completed") => events.push(EventKind::Result { tokens_in: 0, tokens_out: 0, cost_usd: None, ms: 0 }),
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

    fn answer(&mut self, _id: &str, _decision: Decision) -> Option<Vec<u8>> { None }

    fn interrupt(&mut self) -> Option<Vec<u8>> {
        let thread_id = self.thread.clone()?;
        Some(self.request("turn/interrupt", json!({"threadId":thread_id})))
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
        driver.feed(b"{\"jsonrpc\":\"2.0\",\"id\":3,\"result\":{\"thread\":{\"id\":\"t\"}}}\n");
        let turn = String::from_utf8(driver.outgoing().pop().unwrap()).unwrap();
        assert!(turn.contains("localImage"));
        assert!(turn.contains("/tmp/a, b.png"));
    }
}
