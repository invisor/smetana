//! The server in this process: pairing through the local file and through a ticket, revoking a live
//! connection, the restart gateway and the health surface. `binary.rs` runs the real executable.

mod common;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use common::{bd, Device};
use remote_protocol::{BridgeError, DeviceKind, PairReply, CLOSE_REVOKED, CLOSE_UNKNOWN};
use remote_server::gateway::{Mode, Outcome, WorkKind};
use serde_json::json;
use smetanad::client::Client;
use smetanad::ops::{self, Paired};
use smetanad::server::{Exit, Options, Server};

async fn start() -> (tempfile::TempDir, Server) {
    let dir = tempfile::tempdir().unwrap();
    let mut options = Options::new(dir.path().join("data"));
    options.bd = Some(bd());
    options.restart_poll = Duration::from_millis(100);
    let server = Server::start(options).await.expect("the server starts");
    (dir, server)
}

/// A relay that cannot be reached: tickets name one, the test dials the loopback address instead.
const RELAY: &str = "https://relay.invalid";

/// `ops::pair` running against the server while a phone spends the ticket it prints.
async fn pair_a_phone(server: &Server, device: &Device, answer: bool) -> (Paired, PairReply) {
    let mut cli = Client::connect(&server.data_dir).await.expect("the local file pairs the cli");
    let output = Arc::new(Mutex::new(Vec::<u8>::new()));
    struct Out(Arc<Mutex<Vec<u8>>>);
    impl std::io::Write for Out {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let shown = output.clone();
    let terminal = tokio::spawn(async move {
        let mut out = Out(shown);
        ops::pair(&mut cli, DeviceKind::Phone, RELAY, move |_name| answer, &mut out).await.unwrap()
    });
    let ticket = {
        let mut found = None;
        for _ in 0..300 {
            let text = String::from_utf8_lossy(&output.lock().unwrap()).into_owned();
            found = text.lines().find(|line| line.starts_with("smetana1:")).map(str::to_owned);
            // The last line printed, so the QR code before it is complete.
            if found.is_some() && text.contains("Waiting for the device") {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        found.expect("the ticket is printed as text")
    };
    let text = String::from_utf8_lossy(&output.lock().unwrap()).into_owned();
    assert!(text.contains('█') || text.contains('▀') || text.contains('▄'), "and as a QR code: {text}");
    let reply = device.pair_with(&ticket, "Pixel 8").await;
    let paired = terminal.await.unwrap();
    // A ticket is single use: the same one presented again is refused.
    let again = Device::new(&server.data_dir).await.pair_with(&ticket, "Second").await;
    assert!(matches!(again, PairReply::Rejected { .. }), "reuse is rejected, got {again:?}");
    (paired, reply)
}

#[test]
fn the_local_file_pairs_a_client_without_a_person_and_it_is_a_desktop() {
    common::block_on(async {
        let (_dir, server) = start().await;
        let client = Client::connect(&server.data_dir).await.expect("pairs through the file");
        assert_eq!(server.remote.devices.all().len(), 1);
        let device = &server.remote.devices.all()[0];
        assert_eq!(device.kind, DeviceKind::Desktop);
        assert!(device.local);
        assert!(client.hello.capabilities.iter().any(|c| c == "terminal_write"));
        assert!(client.hello.capabilities.iter().any(|c| c == "server_restart"));
        client.call("tracker_snapshot", json!({})).await.expect("a desktop may call the tracker");

        // The next call goes straight to Hello: no second device.
        drop(client);
        Client::connect(&server.data_dir).await.unwrap();
        assert_eq!(server.remote.devices.all().len(), 1);

        // The relay endpoint stays down: nothing here can need it.
        assert!(!server.remote.devices.has_remote());
        server.shutdown().await;
    });
}

#[cfg(unix)]
#[test]
fn the_credential_file_is_private_and_goes_with_a_clean_stop() {
    common::block_on(async {
        use std::os::unix::fs::PermissionsExt;
        let (_dir, server) = start().await;
        let path = smetanad::local::path(&server.data_dir);
        assert_eq!(std::fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
        let data = server.data_dir.clone();
        server.shutdown().await;
        assert!(!smetanad::local::path(&data).exists());
    });
}

#[test]
fn pair_prints_a_ticket_a_loopback_phone_spends_once() {
    common::block_on(async {
        let (_dir, server) = start().await;
        let device = Device::new(&server.data_dir).await;
        let (paired, reply) = pair_a_phone(&server, &device, true).await;
        assert_eq!(reply, PairReply::Accepted);
        assert_eq!(paired, Paired::Accepted("Pixel 8".into()));
        let listed = server.remote.devices.all();
        let phone = listed.iter().find(|d| d.name == "Pixel 8").expect("the phone is on the list");
        assert_eq!(phone.kind, DeviceKind::Phone, "the ticket's kind, not the device's word");
        assert!(!phone.local);
        assert!(server.remote.devices.has_remote(), "a ticket-paired device wants the public endpoint at start");
        server.shutdown().await;
    });
}

#[test]
fn a_declined_pairing_saves_nothing() {
    common::block_on(async {
        let (_dir, server) = start().await;
        let device = Device::new(&server.data_dir).await;
        let (paired, reply) = pair_a_phone(&server, &device, false).await;
        assert!(matches!(reply, PairReply::Rejected { .. }));
        assert_eq!(paired, Paired::Declined);
        assert!(server.remote.devices.all().iter().all(|d| d.name != "Pixel 8"));
        server.shutdown().await;
    });
}

#[test]
fn revoke_closes_the_live_connection_of_the_device() {
    common::block_on(async {
        let (_dir, server) = start().await;
        let device = Device::new(&server.data_dir).await;
        let (_, reply) = pair_a_phone(&server, &device, true).await;
        assert_eq!(reply, PairReply::Accepted);

        // The phone is connected and working.
        let phone = Client::open(&device.endpoint, &device.addr).await.expect("a paired phone says hello");
        phone.call("tracker_health", json!({})).await.expect("an allowlisted call");
        // And it is a phone: the server's own commands are not its to call.
        for cmd in ["server_health", "server_restart", "remote_devices"] {
            assert_eq!(phone.call(cmd, json!({ "mode": "now" })).await, Err(BridgeError::Forbidden), "{cmd}");
        }

        let cli = Client::connect(&server.data_dir).await.unwrap();
        let mut out = Vec::new();
        let key = device.key.public().to_string();
        ops::revoke(&cli, &key[..12], &mut out).await.expect("revoke by a prefix of the key");
        assert!(String::from_utf8_lossy(&out).contains("Pixel 8"));

        let code = tokio::time::timeout(Duration::from_secs(10), phone.closed()).await.expect("closed at once");
        assert_eq!(code, Some(CLOSE_REVOKED));
        assert!(server.remote.devices.all().iter().all(|d| d.key != key));
        // And it does not get back in.
        let back = Client::open(&device.endpoint, &device.addr).await;
        assert!(back.is_err(), "a revoked key is closed before a byte is read");
        let _ = CLOSE_UNKNOWN;
        server.shutdown().await;
    });
}

#[test]
fn revoke_wants_a_prefix_that_names_exactly_one_device() {
    common::block_on(async {
        let (_dir, server) = start().await;
        let cli = Client::connect(&server.data_dir).await.unwrap();
        let mut out = Vec::new();
        assert!(ops::revoke(&cli, "ab", &mut out).await.is_err(), "too short");
        assert!(ops::revoke(&cli, "ffffffff", &mut out).await.unwrap_err().contains("no paired device"));
        server.shutdown().await;
    });
}

async fn a_shell(client: &Client) -> u64 {
    let dir = std::env::temp_dir().to_string_lossy().into_owned();
    let shell = client
        .call("terminal_shell", json!({ "project": dir, "cwd": null }))
        .await
        .expect("a shell starts");
    shell["id"].as_u64().expect("a session id")
}

#[test]
fn restart_now_with_a_live_session_lists_the_work_and_restarts_nothing() {
    common::block_on(async {
        let (_dir, server) = start().await;
        let client = Client::connect(&server.data_dir).await.unwrap();
        let id = a_shell(&client).await;

        let mut out = Vec::new();
        let outcome = ops::restart(&client, Mode::Now, false, |_| false, &mut out).await.unwrap();
        assert_eq!(outcome, Outcome::NeedsConfirmation);
        let text = String::from_utf8_lossy(&out).into_owned();
        assert!(text.contains("Shell") && text.contains(&format!("#{id}")), "the list names the shell: {text}");
        assert!(text.contains("Nothing was restarted"), "{text}");

        // Nothing happened: the server is still up and the shell is still there.
        assert!(tokio::time::timeout(Duration::from_millis(800), server.wait()).await.is_err(), "no restart");
        let reply = client.call("server_restart", json!({ "mode": "now", "confirm": false })).await.unwrap();
        assert_eq!(reply["outcome"], "needs-confirmation");
        assert_eq!(reply["work"][0]["kind"], serde_json::to_value(WorkKind::Shell).unwrap());
        let sessions = client.call("terminal_list", json!({ "project": std::env::temp_dir() })).await.unwrap();
        assert!(sessions.as_array().unwrap().iter().any(|s| s["id"] == id), "the shell survived the request");
        server.shutdown().await;
    });
}

#[test]
fn a_confirmed_restart_now_restarts_and_when_idle_waits_for_the_work() {
    common::block_on(async {
        let (_dir, server) = start().await;
        let client = Client::connect(&server.data_dir).await.unwrap();
        let id = a_shell(&client).await;

        // When idle: scheduled, and nothing is ended.
        let mut out = Vec::new();
        let outcome = ops::restart(&client, Mode::WhenIdle, false, |_| false, &mut out).await.unwrap();
        assert_eq!(outcome, Outcome::Scheduled);
        assert!(tokio::time::timeout(Duration::from_millis(600), server.wait()).await.is_err());
        // The person finishes the work; the restart follows without anybody asking again.
        client.call("terminal_remove", json!({ "id": id })).await.unwrap();
        assert_eq!(tokio::time::timeout(Duration::from_secs(10), server.wait()).await.unwrap(), Exit::Restart);

        // Confirmed now, with work alive.
        let (_dir2, other) = start().await;
        let client = Client::connect(&other.data_dir).await.unwrap();
        a_shell(&client).await;
        let mut out = Vec::new();
        let outcome = ops::restart(&client, Mode::Now, false, |_| true, &mut out).await.unwrap();
        assert_eq!(outcome, Outcome::Restarting);
        assert_eq!(tokio::time::timeout(Duration::from_secs(10), other.wait()).await.unwrap(), Exit::Restart);
        server.shutdown().await;
        other.shutdown().await;
    });
}

#[test]
fn a_scheduled_restart_can_be_cancelled_from_the_command_line() {
    common::block_on(async {
        let (_dir, server) = start().await;
        let client = Client::connect(&server.data_dir).await.unwrap();
        a_shell(&client).await;
        let mut out = Vec::new();
        assert_eq!(ops::restart(&client, Mode::WhenIdle, false, |_| false, &mut out).await.unwrap(), Outcome::Scheduled);
        assert_eq!(ops::restart(&client, Mode::Cancel, false, |_| false, &mut out).await.unwrap(), Outcome::Cancelled);
        assert_eq!(ops::restart(&client, Mode::Cancel, false, |_| false, &mut out).await.unwrap(), Outcome::NothingScheduled);
        server.shutdown().await;
    });
}

#[test]
fn the_health_surface_reports_bd_and_git_to_a_client() {
    common::block_on(async {
        let (_dir, server) = start().await;
        let client = Client::connect(&server.data_dir).await.unwrap();
        let report = client.call("server_health", json!({})).await.expect("the report");
        assert_eq!(report["bd"]["found"], true, "{report}");
        assert_eq!(report["bd"]["answers"], true, "{report}");
        assert_eq!(report["bdExpected"], "1.1.2");
        assert!(report["git"]["found"].is_boolean());
        let agents = report["agents"].as_array().unwrap();
        assert!(agents.iter().any(|a| a["id"] == "claude") && agents.iter().any(|a| a["id"] == "codex"));
        server.shutdown().await;
    });
}

#[test]
fn a_confirmation_that_does_not_cover_the_work_alive_is_refused_with_the_fresh_list() {
    common::block_on(async {
        let (_dir, server) = start().await;
        let client = Client::connect(&server.data_dir).await.unwrap();
        // The person confirmed an empty list; a shell has started since.
        let id = a_shell(&client).await;
        let reply = client.call("server_restart", json!({ "mode": "now", "confirmed": [] })).await.unwrap();
        assert_eq!(reply["outcome"], "needs-confirmation");
        assert_eq!(reply["work"][0]["id"], id, "the fresh list is returned");
        assert!(tokio::time::timeout(Duration::from_millis(800), server.wait()).await.is_err(), "no restart");
        // Confirming what the server just listed does restart.
        let confirmed = reply["work"].clone();
        let reply = client.call("server_restart", json!({ "mode": "now", "confirmed": confirmed })).await.unwrap();
        assert_eq!(reply["outcome"], "restarting");
        assert_eq!(tokio::time::timeout(Duration::from_secs(10), server.wait()).await.unwrap(), Exit::Restart);
        server.shutdown().await;
    });
}

#[test]
fn yes_prints_the_work_it_confirms() {
    common::block_on(async {
        let (_dir, server) = start().await;
        let client = Client::connect(&server.data_dir).await.unwrap();
        a_shell(&client).await;
        let mut out = Vec::new();
        let outcome = ops::restart(&client, Mode::Now, true, |_| panic!("--yes does not ask"), &mut out).await.unwrap();
        assert_eq!(outcome, Outcome::Restarting);
        let text = String::from_utf8_lossy(&out).into_owned();
        assert!(text.contains("Shell") && text.contains("--yes: confirming exactly the work listed"), "{text}");
        server.shutdown().await;
    });
}

#[test]
fn revoking_a_same_machine_client_says_it_will_pair_again() {
    common::block_on(async {
        let (_dir, server) = start().await;
        let cli = Client::connect(&server.data_dir).await.unwrap();
        let mut out = Vec::new();
        let key = server.remote.devices.all()[0].key.clone();
        ops::revoke(&cli, &key[..12], &mut out).await.unwrap();
        let text = String::from_utf8_lossy(&out).into_owned();
        assert!(text.contains("pair itself again"), "{text}");
        // And it does: the credential file is still good.
        Client::connect(&server.data_dir).await.expect("re-pairs through the file");
        server.shutdown().await;
    });
}

#[test]
fn status_asks_the_running_server_and_says_so() {
    common::block_on(async {
        let (_dir, server) = start().await;
        let (report, source) = ops::status(&server.data_dir, None).await;
        assert_eq!(source, ops::Source::Server);
        assert!(report.bd.found, "the server's own bd, though no --bd was given here");
        server.shutdown().await;
    });
}
