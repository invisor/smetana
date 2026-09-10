//! The channel by which a harness asks this app for permission, and the only
//! source of `needs-you` in a driven session.
//!
//! Claude Code, outside its own SDK, has one documented way to route a
//! permission to a client: `--permission-prompt-tool`, naming a tool on an MCP
//! server it connects to. So the app hosts one. The raw `control_request`
//! channel the official SDK uses for `canUseTool` is an SDK internal and is
//! deliberately not built on — `.claude/rules/agents.md` records what guessing
//! at a CLI's vocabulary has already cost this project.
//!
//! The server lives **inside the Tauri process**: an HTTP listener on
//! `127.0.0.1`, port 0 so the operating system picks a free one — a fixed port
//! would collide with a second copy of the app. Not a separate binary and not a
//! stdio server the harness spawns, because either would need a channel of its
//! own back here anyway.
//!
//! **The token is not decoration.** A loopback port is reachable by anything
//! running as this user, and an unauthenticated one would let another process
//! answer, on a person's behalf, a question about running `rm -rf`. One session,
//! one token, carried in `Authorization`; a request without the right one is
//! refused here and never reaches a journal.
//!
//! ## Why a pending call is answered as a stream
//!
//! A question is held open until a person answers it, and holding it open is a
//! thing this file has to be able to do. smetana-dxzr measured the bound on
//! Claude Code 2.1.267: the harness abandons an unanswered tool call after
//! **1800 seconds**, cancels it and hands the model a `tool_use_error`, and the
//! session survives — one call dies, the turn goes on. The bound is an *idle*
//! one, and a `notifications/progress` against the `progressToken` that arrives
//! in `params._meta` restarts it.
//!
//! That renewal was measured over stdio, where a server writes a notification
//! whenever it likes. This is HTTP, where a POST is answered either with one
//! `application/json` body or with a `text/event-stream`, and there is no third
//! channel for a notification to take. So while a question is open the handler
//! answers with a **stream** — a progress notification every `HEARTBEAT`, then
//! the decision, then the end of the stream. Over HTTP that is an inference from
//! the stdio measurement rather than a measurement of its own, and the only way
//! to check it is this listener, once a session runs on it.
//!
//! There is deliberately **no countdown and no automatic deny**. A clock
//! counting down to a deadline a heartbeat keeps moving would be a lie on the
//! screen, and an automatic refusal would cut a person off at the thirty-minute
//! mark for no reason they could see.
//!
//! And one bound a heartbeat does **not** move: the `timeout` field of an
//! `mcpServers` entry. That one is hard, and this app writes the entry itself,
//! per session, with no operator anywhere in the path — so the entry
//! `PermissionTicket::mcp_config` produces carries `type`, `url` and `headers`
//! and must never carry `timeout`.

use std::collections::HashMap;
use std::convert::Infallible;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use std::time::Duration;

use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::sse::{Event as SseEvent, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use futures_core::Stream;
use serde_json::{json, Value};
use tokio::sync::{mpsc, oneshot};

use super::model::{Decision, SessionId};

/// The key this app's server takes in the child's `mcpServers`, and the one
/// tool standing on it. The harness composes the two itself, as
/// `mcp__<server>__<tool>`, and that composed name is what
/// `--permission-prompt-tool` is given — so both halves are written once, here,
/// and `permission_tool` is the only place they are put together.
pub const SERVER_NAME: &str = "smetana";
pub const TOOL_NAME: &str = "approve";

/// What `--permission-prompt-tool` is handed. See the two constants above.
pub fn permission_tool() -> String {
    format!("mcp__{SERVER_NAME}__{TOOL_NAME}")
}

/// The revision this server claims when a client does not say which it speaks.
/// A client that does say is echoed instead: everything implemented here — the
/// handshake, one tool, one call — is the same across every revision that has
/// the streamable HTTP transport, and echoing is what keeps a client newer than
/// this build from being told a version it has dropped.
const DEFAULT_PROTOCOL: &str = "2025-06-18";

/// How often a pending question emits `notifications/progress`.
///
/// Two orders of magnitude inside the 1800-second bound it exists to keep
/// resetting, which is margin for an operator who has lowered
/// `CLAUDE_CODE_MCP_TOOL_IDLE_TIMEOUT` as well as for a slow machine. The cost
/// is one short line on an idle socket every quarter of a minute, for as long
/// as somebody is looking at the question.
const HEARTBEAT: Duration = Duration::from_secs(15);

/// One question, on its way to the worker.
#[derive(Debug)]
pub struct Asked {
    pub session: SessionId,
    pub id: String,
    pub tool: String,
    /// Said by `agents::claude::tool_detail`, the same function the codec calls
    /// for a `ToolUse` event, so that the sentence in the question and the
    /// sentence in the conversation above it cannot drift apart.
    pub detail: String,
}

/// Where one session's child asks, and with what.
pub struct PermissionTicket {
    pub url: String,
    pub token: String,
}

// Written by hand rather than derived, and that is the whole point of it: the
// token is a bearer credential for a socket that answers questions about
// running `rm -rf`, and a derived `Debug` would put it in the first log line
// somebody adds while chasing something else.
impl std::fmt::Debug for PermissionTicket {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PermissionTicket").field("url", &self.url).field("token", &"…").finish()
    }
}

impl PermissionTicket {
    /// The `--mcp-config` document naming this listener to one child.
    ///
    /// `type` is required and its absence is silent: an entry with a `url` and
    /// no `type` is read as stdio and skipped, so the child would start, never
    /// connect, and simply never ask.
    ///
    /// **No `timeout`.** It is the one bound a heartbeat does not extend — the
    /// header of this file has the measurement — and this document is written
    /// by the app rather than by a person, so leaving the field out is the only
    /// way it stays out.
    pub fn mcp_config(&self) -> Value {
        json!({
            "mcpServers": {
                SERVER_NAME: {
                    "type": "http",
                    "url": self.url,
                    "headers": { "Authorization": format!("Bearer {}", self.token) },
                }
            }
        })
    }
}

struct Inner {
    /// Per session: the token that session's child must present. A session
    /// absent from here cannot be authenticated at all, which is what
    /// `register` falls back to when the machine will not give it randomness.
    tokens: HashMap<SessionId, String>,
    /// Questions asked and not yet answered, by the id we minted.
    waiting: HashMap<String, oneshot::Sender<Decision>>,
    next_question: u64,
}

impl Inner {
    fn new() -> Self {
        Self { tokens: HashMap::new(), waiting: HashMap::new(), next_question: 1 }
    }

    /// Whether a request presenting `presented` may speak for `session`.
    ///
    /// Absent is refused, and so is empty — the second explicitly rather than
    /// as a consequence, because `register` hands out an empty token when it
    /// could not mint a real one and this is the line that must hold even if
    /// somebody later makes that path quieter.
    fn authorized(&self, session: SessionId, presented: Option<&str>) -> bool {
        let Some(presented) = presented else {
            return false;
        };
        if presented.is_empty() {
            return false;
        }
        self.tokens.get(&session).is_some_and(|expected| expected == presented)
    }
}

pub struct PermissionServer {
    port: u16,
    inner: Arc<Mutex<Inner>>,
}

impl PermissionServer {
    /// Bind, and serve for as long as the process lives. Dropping the returned
    /// value stops nothing: the task holds the router and the listener, and
    /// this app wants exactly one of these for its whole run.
    pub async fn start(asked: mpsc::Sender<Asked>) -> std::io::Result<Self> {
        let inner = Arc::new(Mutex::new(Inner::new()));
        // Port 0: the operating system picks a free one. A fixed port would
        // collide with a second copy of the app, and with anything else on the
        // machine that had the same idea.
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let port = listener.local_addr()?.port();
        let app = Router::new()
            .route("/mcp/{session}", post(handle))
            .with_state(Shared { inner: inner.clone(), asked });
        tokio::spawn(async move {
            if let Err(error) = axum::serve(listener, app).await {
                log::error!("the permission listener stopped: {error}");
            }
        });
        Ok(Self { port, inner })
    }

    /// Where this session's child should ask, and with what token.
    ///
    /// A machine that will not give sixteen random bytes gets a ticket with an
    /// empty token and **no entry in `tokens`**, so every request naming this
    /// session is refused at the door. That is the safe end of the failure:
    /// the child starts, its MCP server never authenticates, and the harness
    /// declines the tool calls it would have asked about. The unsafe end would
    /// be a token an attacker can guess, and there is no third option — the
    /// whole of this design's security is that the child knows a secret nothing
    /// else on the machine does.
    pub fn register(&self, session: SessionId) -> PermissionTicket {
        let token = crate::terminal::conversation::new_id();
        match &token {
            Some(token) => {
                self.lock().tokens.insert(session, token.clone());
            }
            None => log::error!(
                "no randomness for a permission token, so session {session} gets no permission channel"
            ),
        }
        PermissionTicket {
            url: format!("http://127.0.0.1:{}/mcp/{session}", self.port),
            token: token.unwrap_or_default(),
        }
    }

    /// The session is over. Its questions need no sweeping here: each one is
    /// held by the request that is still streaming, and that request ends with
    /// the child — `Answering`'s `Drop` is what takes it out of `waiting`.
    pub fn forget(&self, session: SessionId) {
        self.lock().tokens.remove(&session);
    }

    /// A person's answer. `false` means there was no such question waiting —
    /// it was answered already, or the harness gave up and went.
    pub fn answer(&self, id: &str, decision: Decision) -> bool {
        let sender = self.lock().waiting.remove(id);
        sender.map(|tx| tx.send(decision).is_ok()).unwrap_or(false)
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().expect("the permission lock is never poisoned")
    }
}

#[derive(Clone)]
struct Shared {
    inner: Arc<Mutex<Inner>>,
    asked: mpsc::Sender<Asked>,
}

/// The token a request presents, or `None` when it presents none this server
/// could read. The scheme is compared without regard to case, as HTTP says it
/// is; the token itself is not touched beyond trimming the space around it.
fn presented_token(headers: &HeaderMap) -> Option<&str> {
    let value = headers.get("authorization")?.to_str().ok()?;
    let (scheme, token) = value.split_once(' ')?;
    scheme.eq_ignore_ascii_case("bearer").then(|| token.trim())
}

/// Everything this server can answer on its own, which is the whole protocol
/// except the one call that waits for a person.
///
/// Pure, and that is deliberate: it is what lets the handshake and the tool
/// list be checked by a test with no socket in it, which is the standing the
/// rest of this file's I/O does not have.
enum Immediate {
    Result(Value),
    Error { code: i64, message: String },
    /// A call of this server's one tool. Nothing here can answer it.
    Ask,
}

fn immediate(method: &str, request: &Value) -> Immediate {
    match method {
        "initialize" => {
            let protocol = request
                .pointer("/params/protocolVersion")
                .and_then(Value::as_str)
                .unwrap_or(DEFAULT_PROTOCOL);
            Immediate::Result(json!({
                "protocolVersion": protocol,
                "capabilities": { "tools": {} },
                "serverInfo": { "name": "smetana-permissions", "version": "1" },
            }))
        }
        "ping" => Immediate::Result(json!({})),
        "tools/list" => Immediate::Result(json!({ "tools": [{
            "name": TOOL_NAME,
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
        "tools/call" => match request.pointer("/params/name").and_then(Value::as_str) {
            Some(TOOL_NAME) => Immediate::Ask,
            other => Immediate::Error {
                code: -32602,
                message: format!("there is no tool {} on this server", other.unwrap_or("(unnamed)")),
            },
        },
        // A method this server does not have is said so in the protocol's own
        // words rather than answered with an empty result: `-32601` is what a
        // client reads as "not supported", and an empty result for, say,
        // `resources/list` would be a malformed answer instead of a refusal.
        _ => Immediate::Error {
            code: -32601,
            message: format!("this server does not implement {method}"),
        },
    }
}

/// What goes back to the harness once a person has decided. The shape
/// `--permission-prompt-tool` expects: one text block whose body is the
/// decision as JSON.
///
/// `AllowAlways` is an allow here and nothing more. Not asking again is the
/// worker's memory to keep, not this socket's — the harness asks afresh every
/// time and has nowhere to record that it should not.
fn decision_payload(decision: Decision, input: &Value) -> Value {
    match decision {
        Decision::Allow | Decision::AllowAlways => {
            json!({ "behavior": "allow", "updatedInput": input })
        }
        Decision::Deny => json!({
            "behavior": "deny",
            "message": "The person supervising this session declined.",
        }),
    }
}

fn rpc_result(id: &Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

fn rpc_error(id: &Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

async fn handle(
    Path(session): Path<SessionId>,
    State(shared): State<Shared>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    // Before the body is even looked at. A request that cannot prove it is this
    // session's child gets a status and nothing else: no `Asked`, no journal
    // line, and no log line either — a socket anything on this machine can
    // reach must not be able to write into what a person reads.
    {
        let inner = shared.inner.lock().expect("the permission lock is never poisoned");
        if !inner.authorized(session, presented_token(&headers)) {
            return StatusCode::UNAUTHORIZED.into_response();
        }
    }

    let Ok(request) = serde_json::from_slice::<Value>(&body) else {
        let body = rpc_error(&Value::Null, -32700, "the request body is not JSON");
        return (StatusCode::BAD_REQUEST, Json(body)).into_response();
    };
    let method = request.get("method").and_then(Value::as_str).unwrap_or_default();
    // A notification carries no id and wants no answer; `202` with an empty
    // body is what the transport asks for. `notifications/initialized` and
    // `notifications/cancelled` both arrive this way.
    let id = match request.get("id") {
        Some(id) if !id.is_null() => id.clone(),
        _ => return StatusCode::ACCEPTED.into_response(),
    };

    match immediate(method, &request) {
        Immediate::Result(result) => Json(rpc_result(&id, result)).into_response(),
        Immediate::Error { code, message } => Json(rpc_error(&id, code, &message)).into_response(),
        Immediate::Ask => ask(session, shared, request, id).await,
    }
}

/// Put the question to a person and hold the request open until it is answered.
async fn ask(session: SessionId, shared: Shared, request: Value, id: Value) -> Response {
    let arguments = request.pointer("/params/arguments").cloned().unwrap_or(Value::Null);
    let tool = arguments.get("tool_name").and_then(Value::as_str).unwrap_or("a tool").to_string();
    let input = arguments.get("input").cloned().unwrap_or(json!({}));
    let detail = crate::agents::claude::tool_detail(&tool, &input);

    let (tx, rx) = oneshot::channel();
    let question = {
        let mut inner = shared.inner.lock().expect("the permission lock is never poisoned");
        let question = format!("q{}", inner.next_question);
        inner.next_question += 1;
        inner.waiting.insert(question.clone(), tx);
        question
    };
    if shared
        .asked
        .send(Asked { session, id: question.clone(), tool, detail })
        .await
        .is_err()
    {
        // Nobody is left to put the question to, so the entry just made would
        // never be answered. Take it back out rather than leaving it to the
        // stream that is not going to exist.
        shared.inner.lock().expect("the permission lock is never poisoned").waiting.remove(&question);
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    }

    Sse::new(Answering {
        inner: shared.inner,
        question,
        id,
        input,
        progress_token: request.pointer("/params/_meta/progressToken").cloned(),
        // The first beat is a whole period away rather than immediate: a
        // heartbeat sent before anything has had time to go idle renews
        // nothing and only says the socket is open, which the response headers
        // have already said.
        beat: tokio::time::interval_at(tokio::time::Instant::now() + HEARTBEAT, HEARTBEAT),
        answer: rx,
        progress: 0,
        done: false,
    })
    .into_response()
}

/// One question's half of the response: progress until a person decides, then
/// the decision, then the end of the stream.
///
/// The stream is deliberately not primed with an event id and the transport's
/// `Last-Event-ID` resumption is not implemented: a question is a live thing
/// held in memory by the request that carries it, so there is nothing here to
/// resume onto, and offering an id would invite a client to try.
struct Answering {
    inner: Arc<Mutex<Inner>>,
    question: String,
    /// The JSON-RPC id of the call being answered.
    id: Value,
    /// What the harness proposed running, echoed back as `updatedInput`.
    input: Value,
    progress_token: Option<Value>,
    beat: tokio::time::Interval,
    answer: oneshot::Receiver<Decision>,
    progress: u64,
    done: bool,
}

impl Answering {
    fn heartbeat(&mut self) -> SseEvent {
        self.progress += 1;
        match &self.progress_token {
            Some(token) => {
                let notification = json!({
                    "jsonrpc": "2.0",
                    "method": "notifications/progress",
                    "params": { "progressToken": token, "progress": self.progress },
                });
                SseEvent::default().json_data(notification).unwrap_or_else(|error| {
                    log::error!("a progress notification would not serialise: {error}");
                    SseEvent::default().comment("progress")
                })
            }
            // No token, so there is nothing to renew the harness's idle bound
            // against and the 1800-second limit applies to this call after all.
            // A comment at least holds the socket open, and holding it open is
            // the half of this that is still ours to do.
            None => SseEvent::default().comment("waiting for the person supervising this session"),
        }
    }

    fn decided(&self, decision: Decision) -> SseEvent {
        let payload = decision_payload(decision, &self.input);
        let result = json!({ "content": [{ "type": "text", "text": payload.to_string() }] });
        SseEvent::default().json_data(rpc_result(&self.id, result)).unwrap_or_else(|error| {
            log::error!("a permission answer would not serialise: {error}");
            SseEvent::default().comment("the answer could not be encoded")
        })
    }
}

impl Stream for Answering {
    type Item = Result<SseEvent, Infallible>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        if this.done {
            return Poll::Ready(None);
        }
        match Pin::new(&mut this.answer).poll(cx) {
            Poll::Ready(answered) => {
                this.done = true;
                // A dropped sender means the question was taken out from under
                // this request — the app going down, and nothing else does it.
                // Deny is the only safe reading of "nobody will answer".
                Poll::Ready(Some(Ok(this.decided(answered.unwrap_or(Decision::Deny)))))
            }
            Poll::Pending => match this.beat.poll_tick(cx) {
                Poll::Ready(_) => Poll::Ready(Some(Ok(this.heartbeat()))),
                Poll::Pending => Poll::Pending,
            },
        }
    }
}

impl Drop for Answering {
    fn drop(&mut self) {
        // The harness gave up, or the connection went with the child. Either
        // way nobody is listening any more, and the entry would otherwise sit
        // in `waiting` for the life of the app — with `answer` reporting true
        // for a question whose answer reaches nothing.
        if let Ok(mut inner) = self.inner.lock() {
            inner.waiting.remove(&self.question);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A server with no socket under it. Everything tested here is the state
    /// and the protocol rather than the transport, which is the half that has
    /// no unit test by design — the same standing `service.rs` has.
    fn server() -> PermissionServer {
        PermissionServer { port: 4321, inner: Arc::new(Mutex::new(Inner::new())) }
    }

    fn bearer(token: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert("authorization", format!("Bearer {token}").parse().expect("a header value"));
        headers
    }

    #[test]
    fn the_tool_named_on_the_command_line_is_the_tool_the_server_declares() {
        assert_eq!(permission_tool(), "mcp__smetana__approve");
        let Immediate::Result(listed) = immediate("tools/list", &json!({})) else {
            panic!("tools/list is answered here and not by asking anybody");
        };
        let tools = listed["tools"].as_array().expect("a list of tools");
        assert_eq!(tools.len(), 1, "one tool and no more: {listed}");
        assert_eq!(tools[0]["name"], TOOL_NAME);
        assert_eq!(permission_tool(), format!("mcp__{SERVER_NAME}__{}", tools[0]["name"].as_str().expect("a name")));
    }

    #[test]
    fn the_handshake_answers_with_the_revision_the_client_asked_for() {
        let request = json!({ "params": { "protocolVersion": "2025-03-26" } });
        let Immediate::Result(hello) = immediate("initialize", &request) else {
            panic!("the handshake is answered here");
        };
        assert_eq!(hello["protocolVersion"], "2025-03-26");
        assert!(hello["capabilities"]["tools"].is_object(), "tools are declared: {hello}");
    }

    #[test]
    fn a_client_that_names_no_revision_is_told_this_builds() {
        let Immediate::Result(hello) = immediate("initialize", &json!({})) else {
            panic!("the handshake is answered here");
        };
        assert_eq!(hello["protocolVersion"], DEFAULT_PROTOCOL);
    }

    #[test]
    fn a_call_of_this_servers_one_tool_is_the_only_thing_that_asks_a_person() {
        let call = json!({ "params": { "name": TOOL_NAME } });
        assert!(matches!(immediate("tools/call", &call), Immediate::Ask));
    }

    #[test]
    fn a_call_of_a_tool_this_server_does_not_have_is_refused_rather_than_asked() {
        let call = json!({ "params": { "name": "something_else" } });
        let Immediate::Error { code, .. } = immediate("tools/call", &call) else {
            panic!("only `approve` reaches a person");
        };
        assert_eq!(code, -32602);
    }

    #[test]
    fn a_method_this_server_does_not_implement_is_an_error_and_not_an_empty_result() {
        // A client reads -32601 as "not supported"; an empty result would be a
        // malformed answer to `resources/list` rather than a refusal of it.
        let Immediate::Error { code, .. } = immediate("resources/list", &json!({})) else {
            panic!("this server has no resources");
        };
        assert_eq!(code, -32601);
    }

    #[test]
    fn a_request_with_the_sessions_own_token_is_the_only_one_that_speaks_for_it() {
        let server = server();
        let ticket = server.register(7);
        let inner = server.lock();
        assert!(inner.authorized(7, presented_token(&bearer(&ticket.token))));
        assert!(!inner.authorized(8, presented_token(&bearer(&ticket.token))), "another session");
        assert!(!inner.authorized(7, presented_token(&bearer("a-token-of-its-own"))), "a stranger");
    }

    #[test]
    fn a_request_with_no_header_at_all_or_an_empty_one_is_refused() {
        let server = server();
        server.register(7);
        let inner = server.lock();
        assert!(!inner.authorized(7, presented_token(&HeaderMap::new())), "no header");
        assert!(!inner.authorized(7, presented_token(&bearer(""))), "an empty token");
        let mut nonsense = HeaderMap::new();
        nonsense.insert("authorization", "Basic abc".parse().expect("a header value"));
        assert!(!inner.authorized(7, presented_token(&nonsense)), "another scheme");
    }

    #[test]
    fn a_session_nobody_registered_is_refused_whatever_it_presents() {
        let server = server();
        let inner = server.lock();
        assert!(!inner.authorized(7, presented_token(&bearer("anything"))));
    }

    #[test]
    fn a_forgotten_session_stops_being_able_to_ask() {
        let server = server();
        let ticket = server.register(7);
        server.forget(7);
        assert!(!server.lock().authorized(7, presented_token(&bearer(&ticket.token))));
    }

    #[test]
    fn a_ticket_says_where_to_ask_and_with_what() {
        let ticket = server().register(7);
        assert_eq!(ticket.url, "http://127.0.0.1:4321/mcp/7");
        assert!(!ticket.token.is_empty(), "a machine with /dev/urandom mints one");
    }

    #[test]
    fn the_config_written_for_a_child_carries_a_type_and_never_a_timeout() {
        // `type` is required and its absence is silent: an entry with a url and
        // no type is read as stdio and skipped. `timeout` is the one bound a
        // heartbeat does not extend, and this document has no operator in its
        // path, so it must not appear.
        let ticket = PermissionTicket { url: "http://127.0.0.1:4321/mcp/7".into(), token: "t".into() };
        let config = ticket.mcp_config();
        let entry = &config["mcpServers"][SERVER_NAME];
        assert_eq!(entry["type"], "http");
        assert_eq!(entry["url"], "http://127.0.0.1:4321/mcp/7");
        assert_eq!(entry["headers"]["Authorization"], "Bearer t");
        assert!(entry.get("timeout").is_none(), "no timeout in {entry}");
        let fields: Vec<&String> = entry.as_object().expect("an entry").keys().collect();
        assert_eq!(fields, vec!["headers", "type", "url"], "three fields and no fourth");
    }

    #[test]
    fn a_ticket_does_not_print_its_own_token() {
        let ticket = PermissionTicket { url: "http://127.0.0.1:4321/mcp/7".into(), token: "s3cret".into() };
        assert!(!format!("{ticket:?}").contains("s3cret"), "{ticket:?}");
    }

    #[test]
    fn an_allowed_call_goes_back_with_the_input_the_harness_proposed() {
        let input = json!({ "command": "cargo test" });
        let payload = decision_payload(Decision::Allow, &input);
        assert_eq!(payload["behavior"], "allow");
        assert_eq!(payload["updatedInput"], input);
    }

    #[test]
    fn allowing_for_the_rest_of_the_session_is_an_allow_on_this_socket() {
        // Not asking again is the worker's memory to keep; the harness asks
        // afresh every time and has nowhere to record that it should not.
        let payload = decision_payload(Decision::AllowAlways, &json!({}));
        assert_eq!(payload["behavior"], "allow");
    }

    #[test]
    fn a_denied_call_goes_back_with_a_message_and_no_input() {
        let payload = decision_payload(Decision::Deny, &json!({ "command": "rm -rf /" }));
        assert_eq!(payload["behavior"], "deny");
        assert!(payload["message"].is_string(), "{payload}");
        assert!(payload.get("updatedInput").is_none(), "{payload}");
    }

    #[test]
    fn answering_a_question_nobody_asked_is_false_rather_than_a_panic() {
        assert!(!server().answer("q404", Decision::Allow));
    }

    #[test]
    fn a_question_is_answered_once_and_the_second_answer_is_false() {
        let server = server();
        let (tx, rx) = oneshot::channel();
        server.lock().waiting.insert("q1".into(), tx);
        assert!(server.answer("q1", Decision::Allow), "the first answer reaches the request");
        assert!(!server.answer("q1", Decision::Deny), "and there is nothing left to answer");
        assert_eq!(rx.blocking_recv().expect("the decision arrived"), Decision::Allow);
    }

    #[test]
    fn an_answer_to_a_request_that_has_gone_is_false() {
        // The harness gave up, or the child died: the receiving half is gone,
        // and saying the answer landed would be saying something untrue.
        let server = server();
        let (tx, rx) = oneshot::channel();
        server.lock().waiting.insert("q1".into(), tx);
        drop(rx);
        assert!(!server.answer("q1", Decision::Allow));
    }
}
