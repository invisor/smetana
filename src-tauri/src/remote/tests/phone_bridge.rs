//! The command bridge end to end: a paired phone on a real iroh connection
//! (loopback, no relay process — the same harness as the pairing tests) calling
//! the allowlisted commands and listening to events, with recording stand-ins
//! behind the dispatcher where a test must prove a handler was or was not
//! reached.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use iroh::endpoint::{Connection, RecvStream, SendStream};
use remote_protocol::{
    read_frame, write_frame, BridgeError, ClientMsg, HelloReply, ServerMsg, ALLOWED_COMMANDS,
    MAX_FRAME,
};
use serde_json::{json, Value};
use tokio::sync::mpsc;

use super::{harness, hello, pair_accepted, phone, wait_until, Harness, Phone};
use crate::remote::bridge::{invoke, BoxFuture, Dispatch, DESKTOP_COMMANDS};
use crate::remote::dispatch::Services;
use crate::remote::PROTOCOL_MAJOR;
use crate::runs::service::RunHandle;
use crate::session::model::SessionError;
use crate::session::service::{Request as SessionRequest, SessionHandle};
use crate::terminal::service::TerminalHandle;
use crate::tracker::model::{ColumnDef, Issue, Snapshot};
use crate::tracker::service::{Request as TrackerRequest, TrackerHandle};

/// A dispatcher that records what reached it and answers `null`, after an
/// optional delay per command.
#[derive(Default)]
struct Recorder {
    calls: Mutex<Vec<String>>,
    slow: Mutex<Vec<(&'static str, Duration)>>,
}

impl Dispatch for Recorder {
    fn call<'a>(&'a self, cmd: &'a str, _args: Value) -> BoxFuture<'a, Result<Value, BridgeError>> {
        Box::pin(async move {
            self.calls.lock().unwrap().push(cmd.to_owned());
            let delay = self.slow.lock().unwrap().iter().find(|(c, _)| *c == cmd).map(|(_, d)| *d);
            if let Some(delay) = delay {
                tokio::time::sleep(delay).await;
            }
            Ok(json!({ "ran": cmd }))
        })
    }
}

#[tokio::test]
async fn remote_bridge_refuses_every_command_off_the_allowlist_and_runs_no_handler() {
    let recorder = Arc::new(Recorder::default());
    let dispatch: Arc<dyn Dispatch> = recorder.clone();
    let named = [
        "files_write",
        "vcs_push",
        "settings_save",
        "terminal_write",
        "terminal_shell",
        "remote_device_revoke",
    ];
    for cmd in named {
        let answer = invoke(Some(&dispatch), cmd, json!({})).await;
        assert_eq!(answer, Err(BridgeError::Forbidden), "{cmd}");
    }
    // Not only the named ones: every other command the desktop registers.
    for cmd in DESKTOP_COMMANDS.iter().filter(|c| !ALLOWED_COMMANDS.contains(c)) {
        let answer = invoke(Some(&dispatch), cmd, json!({})).await;
        assert_eq!(answer, Err(BridgeError::Forbidden), "{cmd}");
    }
    assert!(recorder.calls.lock().unwrap().is_empty(), "a handler was reached by a refused command");

    // And an allowed one does reach it, so the recorder is not simply deaf.
    assert_eq!(invoke(Some(&dispatch), "tracker_health", json!({})).await, Ok(json!({ "ran": "tracker_health" })));
    assert_eq!(recorder.calls.lock().unwrap().as_slice(), ["tracker_health"]);
}

#[tokio::test]
async fn remote_bridge_answers_unknown_command_for_a_name_the_desktop_does_not_have() {
    let recorder = Arc::new(Recorder::default());
    let dispatch: Arc<dyn Dispatch> = recorder.clone();
    for cmd in ["no_such_command", "", "TRACKER_SNAPSHOT", "tracker_snapshot ", "terminal_write2"] {
        let answer = invoke(Some(&dispatch), cmd, json!({})).await;
        assert_eq!(answer, Err(BridgeError::UnknownCommand), "{cmd:?}");
    }
    assert!(recorder.calls.lock().unwrap().is_empty());
}

#[tokio::test]
async fn remote_bridge_without_a_dispatcher_answers_internal_for_an_allowed_command() {
    let answer = invoke(None, "tracker_snapshot", json!({})).await;
    assert!(matches!(answer, Err(BridgeError::Internal(_))));
    assert_eq!(invoke(None, "files_write", json!({})).await, Err(BridgeError::Forbidden));
}

/// `DESKTOP_COMMANDS` is what tells `Forbidden` from `UnknownCommand`, so it has
/// to be what `lib.rs` registers.
#[test]
fn remote_bridge_desktop_commands_are_those_lib_rs_registers() {
    let lib = include_str!("../../lib.rs");
    let body = &lib[lib.find("generate_handler![").expect("lib.rs registers commands")..];
    let body = &body[..body.find("])").unwrap()];
    let mut registered: Vec<&str> = body
        .lines()
        .skip(1)
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with("//"))
        .map(|line| line.trim_end_matches(',').rsplit("::").next().unwrap())
        .collect();
    let mut listed: Vec<&str> = DESKTOP_COMMANDS.to_vec();
    registered.sort_unstable();
    listed.sort_unstable();
    assert_eq!(listed, registered, "DESKTOP_COMMANDS and lib.rs's generate_handler! have drifted");
    for cmd in ALLOWED_COMMANDS {
        assert!(registered.contains(cmd), "{cmd} is allowlisted but the desktop has no such command");
    }
}

/// The phone's end of the bridge.
struct Client {
    conn: Connection,
    send: SendStream,
    recv: RecvStream,
    next: u64,
    events: Vec<(String, Value)>,
    hello: HelloReply,
}

impl Client {
    async fn connect(h: &Harness, phone: &Phone) -> Client {
        let conn = phone.connect(h).await;
        let (mut send, mut recv) = conn.open_bi().await.unwrap();
        write_frame(&mut send, &hello(PROTOCOL_MAJOR)).await.unwrap();
        let reply = read_frame::<HelloReply>(&mut recv, MAX_FRAME).await.unwrap();
        Client { conn, send, recv, next: 1, events: Vec::new(), hello: reply }
    }

    async fn send_msg(&mut self, msg: ClientMsg) {
        write_frame(&mut self.send, &msg).await.unwrap();
    }

    async fn read(&mut self) -> ServerMsg {
        tokio::time::timeout(Duration::from_secs(10), read_frame::<ServerMsg>(&mut self.recv, MAX_FRAME))
            .await
            .expect("the desktop answered in time")
            .expect("a frame")
    }

    /// Sends a call and returns its id without waiting.
    async fn start(&mut self, cmd: &str, args: Value) -> u64 {
        let id = self.next;
        self.next += 1;
        self.send_msg(ClientMsg::Invoke { id, cmd: cmd.into(), args }).await;
        id
    }

    /// Reads until the reply to `id` arrives, keeping the events met on the way.
    async fn reply(&mut self, id: u64) -> Result<Value, BridgeError> {
        loop {
            match self.read().await {
                ServerMsg::Reply { id: got, result } if got == id => return result,
                ServerMsg::Reply { .. } => {}
                ServerMsg::Event { name, payload } => self.events.push((name, payload)),
            }
        }
    }

    async fn call(&mut self, cmd: &str, args: Value) -> Result<Value, BridgeError> {
        let id = self.start(cmd, args).await;
        self.reply(id).await
    }

    /// The next event, from those already read or the stream.
    async fn event(&mut self, within: Duration) -> Option<(String, Value)> {
        if !self.events.is_empty() {
            return Some(self.events.remove(0));
        }
        let read = async {
            loop {
                if let ServerMsg::Event { name, payload } = self.read().await {
                    return (name, payload);
                }
            }
        };
        tokio::time::timeout(within, read).await.ok()
    }
}

async fn paired(h: &Harness) -> (Phone, Client) {
    let phone = phone().await;
    pair_accepted(h, &phone).await;
    let client = Client::connect(h, &phone).await;
    (phone, client)
}

/// Workers that answer from a script: the tracker's snapshot, and a session
/// worker for which no session exists.
fn services() -> (Services, Snapshot) {
    let snapshot = Snapshot {
        generation: 4,
        columns: vec![ColumnDef { name: "open".into(), category: "active".into() }],
        issues: vec![Issue {
            id: "smetana-1".into(),
            title: "A task".into(),
            status: "open".into(),
            updated_at: "2026-10-15T10:00:00Z".into(),
            labels: vec!["x".into()],
            ..Issue::default()
        }],
    };
    let (tracker_tx, mut tracker_rx) = mpsc::channel(8);
    let answer = snapshot.clone();
    tokio::spawn(async move {
        while let Some(request) = tracker_rx.recv().await {
            match request {
                TrackerRequest::Snapshot(tx) => {
                    let _ = tx.send(answer.clone());
                }
                TrackerRequest::SetProject(_, tx) => {
                    let _ = tx.send(answer.clone());
                }
                _ => {}
            }
        }
    });
    let (session_tx, mut session_rx) = mpsc::channel(8);
    tokio::spawn(async move {
        while let Some(request) = session_rx.recv().await {
            match request {
                SessionRequest::Attach(id, tx) => {
                    let _ = tx.send(Err(SessionError::NoSuchSession(id)));
                }
                SessionRequest::Send(..) => panic!("a send with attachments must never reach the worker"),
                _ => {}
            }
        }
    });
    // The other two have no worker behind them: a call to either would hang, and
    // none of these tests makes one.
    let (runs_tx, runs_rx) = mpsc::channel(8);
    let (terminal_tx, terminal_rx) = mpsc::channel(8);
    std::mem::forget((runs_rx, terminal_rx));
    let services = Services {
        tracker: TrackerHandle(tracker_tx),
        runs: RunHandle(runs_tx),
        sessions: SessionHandle(session_tx),
        terminal: TerminalHandle(terminal_tx),
        settings_path: Err("no settings in tests".into()),
        project_switched: None,
    };
    (services, snapshot)
}

#[tokio::test]
async fn remote_bridge_hello_reply_lists_the_allowed_commands_and_events() {
    let h = harness().await;
    let (_phone, client) = paired(&h).await;
    assert_eq!(client.hello.capabilities, remote_protocol::capabilities());
    for cmd in ALLOWED_COMMANDS {
        assert!(client.hello.capabilities.iter().any(|c| c == cmd));
    }
    assert!(client.hello.capabilities.iter().any(|c| c == "run:state"));
    assert!(!client.hello.capabilities.iter().any(|c| c == "terminal_write"));
}

#[tokio::test]
async fn remote_bridge_a_phone_gets_the_json_the_tauri_command_returns() {
    let h = harness().await;
    let (services, snapshot) = services();
    // What the `tracker_snapshot` wrapper returns: the plain function it calls.
    let direct = crate::tracker::commands::snapshot(&services.tracker).await.unwrap();
    assert_eq!(direct.generation, snapshot.generation);
    let expected = serde_json::to_value(&direct).unwrap();
    h.remote.hub.set_dispatch(Arc::new(services));

    let (_phone, mut client) = paired(&h).await;
    let got = client.call("tracker_snapshot", json!({})).await.unwrap();
    assert_eq!(got, expected);
    assert_eq!(got["issues"][0]["id"], "smetana-1");
}

#[tokio::test]
async fn remote_bridge_off_the_list_is_forbidden_over_the_wire_and_the_unknown_is_unknown() {
    let h = harness().await;
    let recorder = Arc::new(Recorder::default());
    h.remote.hub.set_dispatch(recorder.clone());
    let (_phone, mut client) = paired(&h).await;

    for cmd in ["files_write", "vcs_push", "settings_save", "terminal_write", "terminal_shell", "remote_device_revoke"] {
        assert_eq!(client.call(cmd, json!({})).await, Err(BridgeError::Forbidden), "{cmd}");
    }
    assert_eq!(client.call("no_such_command", json!({})).await, Err(BridgeError::UnknownCommand));
    assert!(recorder.calls.lock().unwrap().is_empty());
    assert!(h.remote.devices.contains(&_phone.key.public()), "a refusal does not unpair anyone");
}

#[tokio::test]
async fn remote_bridge_a_handler_error_has_the_form_the_desktop_front_end_receives() {
    let h = harness().await;
    let (services, _) = services();
    h.remote.hub.set_dispatch(Arc::new(services));
    let (_phone, mut client) = paired(&h).await;

    let got = client.call("session_attach", json!({ "id": 9 })).await;
    // Tauri rejects `invoke` with the command's error run through serde.
    let desktop = serde_json::to_value(SessionError::NoSuchSession(9)).unwrap();
    assert_eq!(desktop, json!({ "kind": "noSuchSession", "message": 9 }));
    assert_eq!(got, Err(BridgeError::Handler(desktop)));

    // Arguments that do not decode are a rejection too, in words, as Tauri's.
    match client.call("session_attach", json!({ "id": "nine" })).await {
        Err(BridgeError::Handler(Value::String(message))) => assert!(message.contains("session_attach")),
        other => panic!("expected a handler rejection, got {other:?}"),
    }
}

#[tokio::test]
async fn remote_bridge_a_message_with_attachments_never_reaches_the_session() {
    let h = harness().await;
    let (services, _) = services();
    h.remote.hub.set_dispatch(Arc::new(services));
    let (_phone, mut client) = paired(&h).await;
    let args = json!({ "id": 1, "text": "hi", "attachments": ["/etc/passwd"] });
    assert_eq!(client.call("session_send", args).await, Err(BridgeError::Forbidden));
}

#[tokio::test]
async fn remote_bridge_listen_delivers_an_emitted_event_and_unlisten_stops_it() {
    let h = harness().await;
    let (_phone, mut client) = paired(&h).await;
    let payload = json!({ "token": 1, "project": "/p" });

    // Before listening, nothing is delivered.
    h.remote.hub.publish("run:state", &payload);

    client.send_msg(ClientMsg::Listen { event: "run:state".into() }).await;
    wait_until("the listener", || h.remote.hub.listeners("run:state") == 1).await;
    h.remote.hub.publish("run:state", &payload);
    h.remote.hub.publish("tracker:health", &json!({ "state": "ok" }));
    let (name, got) = client.event(Duration::from_secs(5)).await.expect("the event arrives");
    assert_eq!((name.as_str(), &got), ("run:state", &payload));
    assert!(
        client.event(Duration::from_millis(300)).await.is_none(),
        "neither the earlier emit nor an event not listened to"
    );

    client.send_msg(ClientMsg::Unlisten { event: "run:state".into() }).await;
    wait_until("the unlisten", || h.remote.hub.listeners("run:state") == 0).await;
    h.remote.hub.publish("run:state", &payload);
    assert!(client.event(Duration::from_millis(300)).await.is_none(), "delivery stopped");
}

#[tokio::test]
async fn remote_bridge_an_event_off_the_list_cannot_be_listened_to() {
    let h = harness().await;
    h.remote.hub.set_dispatch(Arc::new(Recorder::default()));
    let (_phone, mut client) = paired(&h).await;
    for event in ["terminal:output", "remote:devices", "session:state"] {
        client.send_msg(ClientMsg::Listen { event: event.into() }).await;
    }
    // Messages are handled in order: once this answer is back, the Listens were seen.
    client.call("tracker_health", json!({})).await.unwrap();
    for event in ["terminal:output", "remote:devices", "session:state"] {
        assert_eq!(h.remote.hub.listeners(event), 0, "{event}");
        h.remote.hub.publish(event, &json!({}));
    }
    assert!(client.event(Duration::from_millis(300)).await.is_none());
}

#[tokio::test]
async fn remote_bridge_a_slow_call_does_not_hold_up_a_fast_one() {
    let h = harness().await;
    let recorder = Arc::new(Recorder::default());
    recorder.slow.lock().unwrap().push(("run_state", Duration::from_millis(600)));
    h.remote.hub.set_dispatch(recorder.clone());
    let (_phone, mut client) = paired(&h).await;

    let slow = client.start("run_state", json!({ "project": "/p" })).await;
    let fast = client.start("tracker_health", json!({})).await;
    let started = std::time::Instant::now();
    assert_eq!(client.reply(fast).await, Ok(json!({ "ran": "tracker_health" })));
    assert!(started.elapsed() < Duration::from_millis(500), "the fast call waited for the slow one");
    assert_eq!(client.reply(slow).await, Ok(json!({ "ran": "run_state" })));
}

#[tokio::test]
async fn remote_bridge_a_revoked_phone_is_closed_and_its_listeners_go() {
    let h = harness().await;
    h.remote.hub.set_dispatch(Arc::new(Recorder::default()));
    let (phone, mut client) = paired(&h).await;
    client.send_msg(ClientMsg::Listen { event: "run:state".into() }).await;
    wait_until("the listener", || h.remote.hub.listeners("run:state") == 1).await;

    h.remote.revoke(&phone.key.public()).unwrap();
    let closed = tokio::time::timeout(Duration::from_secs(5), client.conn.closed()).await.unwrap();
    assert!(matches!(
        closed,
        iroh::endpoint::ConnectionError::ApplicationClosed(close)
            if close.error_code.into_inner() == remote_protocol::CLOSE_REVOKED as u64
    ));
    wait_until("the listener to go", || h.remote.hub.listeners("run:state") == 0).await;
    h.remote.hub.publish("run:state", &json!({}));
    assert!(write_frame(&mut client.send, &ClientMsg::Listen { event: "run:state".into() }).await.is_err());
}

#[tokio::test]
async fn remote_bridge_a_project_switch_from_a_phone_is_announced_to_the_window() {
    let h = harness().await;
    let (mut services, _) = services();
    let heard: Arc<Mutex<Vec<Option<String>>>> = Arc::default();
    let sink = heard.clone();
    services.project_switched = Some(Arc::new(move |path| sink.lock().unwrap().push(path)));
    h.remote.hub.set_dispatch(Arc::new(services));
    let (_phone, mut client) = paired(&h).await;

    client.call("tracker_set_project", json!({ "path": "/work/b" })).await.unwrap();
    client.call("tracker_set_project", json!({ "path": null })).await.unwrap();
    // A call that is refused or does not decode announces nothing.
    assert!(client.call("tracker_set_project", json!({ "path": 5 })).await.is_err());
    assert_eq!(heard.lock().unwrap().as_slice(), [Some("/work/b".to_string()), None]);
}
