//! The bridge for a desktop device, end to end on loopback: what a device's kind decides, and the
//! terminal stream — resuming after a drop with nothing missing and nothing twice, and a client
//! that stops reading costing the server a bounded buffer and nothing else.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use remote_protocol::{BridgeError, DeviceKind, Platform};
use serde_json::{json, Value};

use super::phone_bridge::{Client, Recorder};
use super::{harness, pair_accepted, pair_accepted_as, phone, wait_until, Harness, Phone};
use crate::bridge::{BoxFuture, Caller, Dispatch};
use crate::terminal::stream::History;

const RING: usize = 256 * 1024;

/// One terminal session, kept the way the worker keeps it: numbered chunks in a history, and a
/// ring. Its answers are the worker's `peek` in miniature, built on the same `History`.
struct FakeTerminal {
    state: Mutex<Fake>,
}

struct Fake {
    seq: u64,
    history: History,
    ring: Vec<u8>,
}

impl FakeTerminal {
    fn new(history_cap: usize) -> Arc<Self> {
        Arc::new(Self { state: Mutex::new(Fake { seq: 0, history: History::new(history_cap), ring: Vec::new() }) })
    }

    /// The worker committing a chunk: numbered, kept, and offered to the hub.
    fn commit(&self, h: &Harness, bytes: &[u8]) {
        let seq = {
            let mut fake = self.state.lock().unwrap();
            fake.seq += 1;
            let seq = fake.seq;
            fake.history.push(seq, bytes.to_vec());
            fake.ring.extend_from_slice(bytes);
            // A ring is bounded, and a frame is too: the worker's holds a megabyte.
            if fake.ring.len() > RING {
                let excess = fake.ring.len() - RING;
                fake.ring.drain(..excess);
            }
            seq
        };
        h.remote.hub.publish_terminal_chunk(1, seq, bytes);
    }
}

impl Dispatch for FakeTerminal {
    fn call<'a>(&'a self, _caller: &'a Caller, cmd: &'a str, args: Value) -> BoxFuture<'a, Result<Value, BridgeError>> {
        Box::pin(async move {
            if cmd != "terminal_attach" {
                return Ok(Value::Null);
            }
            let fake = self.state.lock().unwrap();
            let after = args["after"].as_u64();
            let resumed = after.and_then(|after| fake.history.after(after, fake.seq).map(|bytes| (after, bytes)));
            let mut reply = json!({ "seq": fake.seq, "log": [], "logTrimmed": 0 });
            match resumed {
                Some((after, bytes)) => {
                    reply["data"] = json!(data_encoding::BASE64.encode(&bytes));
                    reply["resumedAfter"] = json!(after);
                }
                None => reply["data"] = json!(data_encoding::BASE64.encode(&fake.ring)),
            }
            Ok(reply)
        })
    }

    fn terminal_ring<'a>(&'a self, _session: u64) -> BoxFuture<'a, Option<(u64, String)>> {
        Box::pin(async move {
            let fake = self.state.lock().unwrap();
            Some((fake.seq, data_encoding::BASE64.encode(&fake.ring)))
        })
    }
}

async fn desktop(h: &Harness) -> (Phone, Client) {
    let phone = phone().await;
    pair_accepted_as(h, &phone, DeviceKind::Desktop).await;
    let client = Client::connect(h, &phone).await;
    (phone, client)
}

async fn listening(h: &Harness, client: &mut Client, expected: usize) {
    client.send_msg(remote_protocol::ClientMsg::Listen { event: "terminal:output".into() }).await;
    wait_until("the listener", || h.remote.hub.listeners("terminal:output") == expected).await;
}

fn decode(data: &Value) -> Vec<u8> {
    data_encoding::BASE64.decode(data.as_str().unwrap().as_bytes()).unwrap()
}

/// Reads output events until one reaches `seq`; returns the bytes heard and the last number.
async fn hear_until(client: &mut Client, seq: u64) -> (Vec<u8>, u64) {
    hear_after(client, 0, seq).await
}

/// [`hear_until`] for a client whose last chunk is `last`: every event must continue exactly from it.
async fn hear_after(client: &mut Client, mut last: u64, seq: u64) -> (Vec<u8>, u64) {
    let mut heard = Vec::new();
    while last < seq {
        let (name, payload) = client.event(Duration::from_secs(10)).await.expect("an output event");
        assert_eq!(name, "terminal:output");
        assert_eq!(payload["from"].as_u64(), Some(last + 1), "an event that does not continue from {last}: {payload}");
        heard.extend(decode(&payload["data"]));
        last = payload["seq"].as_u64().unwrap();
    }
    (heard, last)
}

#[tokio::test]
async fn remote_desktop_a_phone_that_calls_itself_a_desktop_gets_the_phone_set_and_no_terminal_write() {
    let h = harness().await;
    let recorder = Arc::new(Recorder::default());
    h.remote.hub.set_dispatch(recorder.clone());
    let phone = phone().await;
    pair_accepted(&h, &phone).await; // a ticket for a phone
    let mut client = Client::connect_saying(&h, &phone, Some(Platform::Desktop)).await;

    assert_eq!(client.hello.capabilities, remote_protocol::capabilities());
    assert!(!client.hello.capabilities.iter().any(|c| c == "terminal_write" || c == "terminal:output"));
    assert_eq!(h.remote.devices.kind_of(&phone.key.public()), Some(DeviceKind::Phone));

    for cmd in ["terminal_write", "terminal_resize", "files_write", "settings_save"] {
        assert_eq!(client.call(cmd, json!({})).await, Err(BridgeError::Forbidden), "{cmd}");
    }
    client.send_msg(remote_protocol::ClientMsg::Listen { event: "terminal:output".into() }).await;
    client.call("tracker_health", json!({})).await.unwrap();
    assert_eq!(h.remote.hub.listeners("terminal:output"), 0, "a phone cannot listen to terminal output");
    assert_eq!(recorder.calls.lock().unwrap().as_slice(), ["tracker_health"]);
}

#[tokio::test]
async fn remote_desktop_a_device_paired_as_a_desktop_gets_every_server_command() {
    let h = harness().await;
    let recorder = Arc::new(Recorder::default());
    h.remote.hub.set_dispatch(recorder.clone());
    let (_phone, mut client) = desktop(&h).await;

    assert_eq!(client.hello.capabilities, remote_protocol::capabilities_for(DeviceKind::Desktop));
    assert!(client.hello.capabilities.iter().any(|c| c == "terminal_write"));
    client.call("terminal_write", json!({ "id": 1, "data": "ls" })).await.unwrap();
    // The client's own commands stay off the connection.
    assert_eq!(client.call("window_show_ready", json!({})).await, Err(BridgeError::Forbidden));
    assert_eq!(client.call("no_such_command", json!({})).await, Err(BridgeError::UnknownCommand));
    assert_eq!(recorder.calls.lock().unwrap().as_slice(), ["terminal_write"]);
}

#[tokio::test]
async fn remote_desktop_capabilities_leave_out_what_the_host_cannot_honour() {
    use crate::gateway::Gateway;
    // A `Remote` with no gateway (a host nobody restarts from a client): no restart control is advertised.
    let h = harness().await;
    h.remote.hub.set_dispatch(Arc::new(super::phone_bridge::services().0));
    let (_phone, client) = desktop(&h).await;
    assert!(!client.hello.capabilities.iter().any(|c| c == "server_restart"), "{:?}", client.hello.capabilities);
    assert!(!client.hello.capabilities.iter().any(|c| c == "server:restart-deferred"), "nor the event it announces");
    assert!(client.hello.capabilities.iter().any(|c| c == "terminal_write"), "the rest is untouched");

    // A server with one is advertised it.
    let h = harness().await;
    let mut services = super::phone_bridge::services().0;
    services.gateway = Some(Gateway::new(
        Box::new(|| Box::pin(async { Vec::new() })),
        Arc::new(|| {}),
        Arc::new(|| {}),
        Arc::new(|_| {}),
        Duration::from_secs(1),
    ));
    h.remote.hub.set_dispatch(Arc::new(services));
    let (_phone, client) = desktop(&h).await;
    assert!(client.hello.capabilities.iter().any(|c| c == "server_restart"));
    assert!(client.hello.capabilities.iter().any(|c| c == "server:restart-deferred"));
}

#[tokio::test]
async fn remote_desktop_a_client_that_drops_resumes_with_no_gap_and_no_repeat() {
    let h = harness().await;
    let fake = FakeTerminal::new(1 << 20);
    h.remote.hub.set_dispatch(fake.clone());
    let (phone, mut client) = desktop(&h).await;
    listening(&h, &mut client, 1).await;
    let attached = client.call("terminal_attach", json!({ "id": 1, "after": null })).await.unwrap();
    assert_eq!((attached["seq"].as_u64(), decode(&attached["data"])), (Some(0), Vec::new()));

    fake.commit(&h, b"alpha ");
    fake.commit(&h, b"beta ");
    let (mut heard, last) = hear_until(&mut client, 2).await;
    assert_eq!((heard.as_slice(), last), (b"alpha beta ".as_slice(), 2));

    // The connection goes in the middle of the stream, and the output goes on without it.
    client.conn.close(0u32.into(), b"gone");
    wait_until("the listener to go", || h.remote.hub.listeners("terminal:output") == 0).await;
    fake.commit(&h, b"gamma ");
    fake.commit(&h, b"delta ");

    let mut client = Client::connect(&h, &phone).await;
    listening(&h, &mut client, 1).await;
    let resumed = client.call("terminal_attach", json!({ "id": 1, "after": last })).await.unwrap();
    assert_eq!(resumed["resumedAfter"], json!(last));
    assert_eq!(resumed["seq"], json!(4));
    heard.extend(decode(&resumed["data"]));
    fake.commit(&h, b"epsilon");
    let (more, newest) = hear_after(&mut client, 4, 5).await;
    assert_eq!(newest, 5);
    heard.extend(more);
    assert_eq!(String::from_utf8(heard).unwrap(), "alpha beta gamma delta epsilon");
}

#[tokio::test]
async fn remote_desktop_a_history_that_no_longer_reaches_back_answers_with_the_whole_ring() {
    let h = harness().await;
    let fake = FakeTerminal::new(8);
    h.remote.hub.set_dispatch(fake.clone());
    for chunk in ["aaaa", "bbbb", "cccc", "dddd"] {
        fake.commit(&h, chunk.as_bytes());
    }
    let (_phone, mut client) = desktop(&h).await;
    listening(&h, &mut client, 1).await;
    let reply = client.call("terminal_attach", json!({ "id": 1, "after": 1 })).await.unwrap();
    assert!(reply.get("resumedAfter").is_none(), "trimmed: the ring, not a continuation");
    assert_eq!(decode(&reply["data"]), b"aaaabbbbccccdddd");
    assert_eq!(reply["seq"], json!(4));
}

#[tokio::test]
async fn remote_desktop_a_client_that_stops_reading_costs_a_bounded_buffer_and_gets_a_fresh_ring() {
    let h = harness().await;
    let fake = FakeTerminal::new(1 << 20);
    h.remote.hub.set_dispatch(fake.clone());
    const CAP: usize = 64 * 1024;
    h.remote.hub.set_outbox_cap(CAP);
    let (_phone, mut client) = desktop(&h).await;
    listening(&h, &mut client, 1).await;
    client.call("terminal_attach", json!({ "id": 1, "after": null })).await.unwrap();

    // The client reads nothing from here on. The worker's side is a synchronous call that must
    // not wait for it, however much it has to say.
    let chunk = vec![b'x'; 32 * 1024];
    let started = Instant::now();
    let mut peak = 0;
    for _ in 0..800 {
        fake.commit(&h, &chunk);
        peak = peak.max(h.remote.hub.buffered_bytes());
    }
    assert!(started.elapsed() < Duration::from_secs(5), "publishing waited for the client");
    assert!(peak <= CAP, "the buffer reached {peak} bytes against a ceiling of {CAP}");

    // It reads again: whatever it was sent, then the ring afresh, then the stream goes on.
    let mut screen: Vec<u8> = Vec::new();
    let mut resets = 0;
    loop {
        let (name, payload) = client.event(Duration::from_secs(10)).await.expect("an output event");
        assert_eq!(name, "terminal:output");
        let data = decode(&payload["data"]);
        if payload["reset"] == json!(true) {
            resets += 1;
            assert_eq!(payload["from"], payload["seq"], "a reset is the ring as of one number");
            screen = data;
        } else {
            screen.extend(data);
        }
        if payload["seq"].as_u64() == Some(800) {
            break;
        }
    }
    assert!(resets >= 1, "a client that fell behind is sent the ring again");
    assert!(screen.len() >= RING && screen.iter().all(|b| *b == b'x'), "and what it shows is the ring and what followed");
    fake.commit(&h, b"tail");
    let (tail, seq) = hear_after(&mut client, 800, 801).await;
    assert_eq!((tail.as_slice(), seq), (b"tail".as_slice(), 801));
}

#[tokio::test]
async fn remote_desktop_a_coalesced_event_names_exactly_the_chunks_it_joins() {
    let h = harness().await;
    let fake = FakeTerminal::new(1 << 20);
    h.remote.hub.set_dispatch(fake.clone());
    let (_phone, mut client) = desktop(&h).await;
    // No Listen: following a session is attaching to it.
    client.call("terminal_attach", json!({ "id": 1, "after": null })).await.unwrap();
    // Nothing yields between these, so the writer finds all five waiting and sends one event.
    for chunk in ["a", "b", "c", "d", "e"] {
        fake.commit(&h, chunk.as_bytes());
    }
    let (_, payload) = client.event(Duration::from_secs(10)).await.expect("an event");
    assert_eq!((payload["from"].as_u64(), payload["seq"].as_u64()), (Some(1), Some(5)));
    assert_eq!(decode(&payload["data"]), b"abcde");
    assert!(payload.get("reset").is_none());
}

#[tokio::test]
async fn remote_desktop_attaching_again_on_a_live_connection_starts_the_follow_afresh() {
    let h = harness().await;
    let fake = FakeTerminal::new(1 << 20);
    h.remote.hub.set_dispatch(fake.clone());
    let (_phone, mut client) = desktop(&h).await;
    client.call("terminal_attach", json!({ "id": 1, "after": null })).await.unwrap();
    fake.commit(&h, b"one ");
    let (heard, last) = hear_after(&mut client, 0, 1).await;
    assert_eq!(heard, b"one ");
    let again = client.call("terminal_attach", json!({ "id": 1, "after": last })).await.unwrap();
    assert_eq!((again["resumedAfter"].as_u64(), again["seq"].as_u64()), (Some(1), Some(1)));
    fake.commit(&h, b"two");
    let (heard, _) = hear_after(&mut client, 1, 2).await;
    assert_eq!(heard, b"two");
}

#[tokio::test]
async fn remote_desktop_a_phone_that_attaches_to_a_terminal_gets_no_stream() {
    let h = harness().await;
    let fake = FakeTerminal::new(1 << 20);
    h.remote.hub.set_dispatch(fake.clone());
    let phone = phone().await;
    pair_accepted(&h, &phone).await;
    let mut client = Client::connect(&h, &phone).await;
    // The phone's attach is a read of the ring, as ever.
    let reply = client.call("terminal_attach", json!({ "id": 1 })).await.unwrap();
    assert_eq!(decode(&reply["data"]), Vec::<u8>::new());

    fake.commit(&h, b"raw pty bytes");
    h.remote.hub.publish_terminal_log(1, &json!([]));
    assert_eq!(h.remote.hub.buffered_bytes(), 0, "nothing is held for a phone");
    assert!(client.event(Duration::from_millis(300)).await.is_none(), "and nothing is sent to it");
}

#[tokio::test]
async fn remote_desktop_output_in_flight_when_a_client_attaches_again_is_not_applied_twice() {
    let h = harness().await;
    let fake = FakeTerminal::new(1 << 20);
    h.remote.hub.set_dispatch(fake.clone());
    let (_phone, mut client) = desktop(&h).await;
    client.call("terminal_attach", json!({ "id": 1, "after": null })).await.unwrap();
    fake.commit(&h, b"a");
    let (mut screen, last) = hear_after(&mut client, 0, 1).await;

    // Two more chunks are written to the connection before the client asks again.
    fake.commit(&h, b"b");
    fake.commit(&h, b"c");
    tokio::time::sleep(Duration::from_millis(100)).await;
    let reply = client.call("terminal_attach", json!({ "id": 1, "after": last })).await.unwrap();
    // The documented rule: output that arrived while the attach was outstanding is discarded, and
    // the reply alone sets `last`.
    let stale = client.events.len();
    client.events.clear();
    assert!(stale >= 1, "the chunks really were in flight, or this test proves nothing");
    assert_eq!(reply["resumedAfter"], json!(last));
    screen.extend(decode(&reply["data"]));
    let last = reply["seq"].as_u64().unwrap();

    fake.commit(&h, b"d");
    let (more, _) = hear_after(&mut client, last, 4).await;
    screen.extend(more);
    assert_eq!(String::from_utf8(screen).unwrap(), "abcd");
}

#[tokio::test]
async fn remote_desktop_reading_a_file_from_the_server_is_confined_and_a_phone_cannot_ask() {
    let tmp = tempfile::tempdir().unwrap();
    let base = std::fs::canonicalize(tmp.path()).unwrap();
    let project = base.join("work/app");
    std::fs::create_dir_all(project.join(".smetana/reports")).unwrap();
    std::fs::write(project.join(".smetana/reports/run-1.html"), "<html>ok</html>").unwrap();
    std::fs::write(project.join("secret.txt"), "secret").unwrap();
    let store = base.join("data/attachments");
    std::fs::create_dir_all(store.join("p-1")).unwrap();
    std::fs::write(store.join("p-1/pic.png"), "png").unwrap();
    // The server's settings name the project, as `settings.json` does once it has been added.
    let settings = base.join("settings.json");
    std::fs::write(&settings, json!({ "openProjects": [project.to_string_lossy()] }).to_string()).unwrap();

    let h = harness().await;
    let mut services = super::phone_bridge::services().0;
    services.attachments_root = Ok(store.clone());
    services.settings_path = Ok(settings);
    h.remote.hub.set_dispatch(Arc::new(services));
    let (_desktop, mut client) = desktop(&h).await;

    let read = |path: String| json!({ "path": path });
    let report = client.call("server_file_read", read(project.join(".smetana/reports/run-1.html").to_string_lossy().into_owned())).await.unwrap();
    assert_eq!(report["name"], "run-1.html");
    assert_eq!(decode(&report["data"]), b"<html>ok</html>");
    let picture = client.call("server_file_read", read(store.join("p-1/pic.png").to_string_lossy().into_owned())).await.unwrap();
    assert_eq!(decode(&picture["data"]), b"png");

    // The rest of the project, a path that climbs out of the store, and a file nowhere near either.
    let sneaky = format!("{}/p-1/../../../work/app/secret.txt", store.display());
    for path in [project.join("secret.txt").to_string_lossy().into_owned(), sneaky, "/etc/passwd".to_owned()] {
        match client.call("server_file_read", read(path.clone())).await {
            Err(BridgeError::Handler(value)) => assert_eq!(value["kind"], "outside", "{path}: {value}"),
            other => panic!("{path} was not refused: {other:?}"),
        }
    }

    // A phone is not a desktop: none of the four exists for it. (Its own server, because a pairing
    // request in the first one's event log would be taken for the second's.)
    let h = harness().await;
    h.remote.hub.set_dispatch(Arc::new(super::phone_bridge::services().0));
    let phone_device = phone().await;
    pair_accepted(&h, &phone_device).await;
    let mut phone_client = Client::connect(&h, &phone_device).await;
    for cmd in ["server_file_read", "folders_list", "project_clone", "server_work"] {
        assert_eq!(phone_client.call(cmd, json!({ "path": "/etc/passwd" })).await, Err(BridgeError::Forbidden), "{cmd}");
    }
    assert!(!phone_client.hello.capabilities.iter().any(|c| c == "server_file_read" || c == "folders_list"));
}

#[tokio::test]
async fn remote_desktop_the_folder_browser_refuses_a_path_above_the_servers_home() {
    let h = harness().await;
    h.remote.hub.set_dispatch(Arc::new(super::phone_bridge::services().0));
    let (_desktop, mut client) = desktop(&h).await;
    // Whatever the home folder is, the filesystem root is above it (or the same, on a machine whose
    // home is the root, which nobody runs a server as).
    match client.call("folders_list", json!({ "path": "/" })).await {
        Err(BridgeError::Handler(value)) => assert_eq!(value["kind"], "outside", "{value}"),
        Ok(listing) => assert_eq!(listing["parent"], Value::Null, "only the root itself may be listed: {listing}"),
        other => panic!("{other:?}"),
    }
    let home = client.call("folders_list", json!({ "path": null })).await.unwrap();
    assert_eq!(home["parent"], Value::Null);
}
