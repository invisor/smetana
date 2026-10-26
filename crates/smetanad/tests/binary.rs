//! The real executable: `smetanad run` in a temporary directory, a client that pairs through the file
//! it writes, the board it serves, and the command line's own subcommands against it.

mod common;

use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use common::{bd, block_on, wait_for};
use serde_json::json;
use smetanad::client::Client;

const EXE: &str = env!("CARGO_BIN_EXE_smetanad");

/// A running `smetanad run`, ended when the guard goes, whatever the test did.
struct Running {
    child: Child,
}

impl Running {
    fn start(data: &Path) -> Running {
        let child = Command::new(EXE)
            .args(["run", "--data-dir"])
            .arg(data)
            .arg("--bd")
            .arg(bd())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("smetanad starts");
        Running { child }
    }

    /// A clean stop, the way a service manager does it.
    #[cfg(unix)]
    fn terminate(&mut self) -> std::process::ExitStatus {
        // SAFETY: signalling a child this test started and still owns.
        unsafe { libc::kill(self.child.id() as i32, libc::SIGTERM) };
        self.child.wait().expect("it exits")
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn git_init(dir: &Path) {
    std::fs::create_dir_all(dir).unwrap();
    let status = Command::new("git").args(["init", "-q"]).current_dir(dir).status().expect("git runs");
    assert!(status.success());
}

#[test]
fn run_serves_a_client_paired_through_the_file_and_streams_tracker_deltas() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().join("data");
    let project = dir.path().join("project");
    git_init(&project);
    let mut server = Running::start(&data);

    block_on(async {
        wait_for("the credential file", || smetanad::local::path(&data).exists()).await;
        // Paired with no person, through the file alone.
        let mut client = Client::connect(&data).await.expect("pairs through the local file");
        assert!(client.hello.capabilities.iter().any(|c| c == "tracker_snapshot"));

        let snapshot = client.call("tracker_snapshot", json!({})).await.expect("a snapshot");
        assert!(snapshot["issues"].is_array(), "{snapshot}");

        client.listen("tracker:delta").await.unwrap();
        client
            .call("tracker_set_project", json!({ "path": project.to_string_lossy() }))
            .await
            .expect("the project opens");
        client.call("tracker_init", json!({})).await.expect("bd initialises a tracker there");
        let (name, payload) = client
            .event_within(Duration::from_secs(60))
            .await
            .expect("a tracker:delta reaches the client");
        assert_eq!(name, "tracker:delta");
        assert!(payload["generation"].is_u64(), "{payload}");
    });

    #[cfg(unix)]
    {
        let status = server.terminate();
        assert!(status.success(), "a SIGTERM is a clean stop: {status:?}");
        assert!(!smetanad::local::path(&data).exists(), "the credential goes with it");
    }
    drop(server);
}

#[test]
fn status_reports_bd_and_git_and_each_agent_and_exits_nonzero_when_something_is_missing() {
    let dir = tempfile::tempdir().unwrap();
    let output = Command::new(EXE)
        .args(["status", "--json", "--data-dir"])
        .arg(dir.path())
        .arg("--bd")
        .arg(bd())
        .output()
        .expect("status runs");
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).expect("JSON on stdout");
    assert_eq!(report["source"], "none", "no server is running for this directory, and the output says so");
    assert_eq!(report["bd"]["answers"], true, "{report}");
    assert_eq!(report["bd"]["version"].as_str().unwrap_or("").contains("1.1.2"), true, "{report}");
    assert_eq!(report["git"]["answers"], true, "{report}");
    assert_eq!(report["agents"].as_array().unwrap().len(), 2);
    assert_eq!(output.status.success(), report["ok"].as_bool().unwrap());

    // With no bd at all, it is a line in the report and a failing exit code.
    let output = Command::new(EXE)
        .args(["status", "--data-dir"])
        .arg(dir.path())
        .arg("--bd")
        .arg(dir.path().join("no-such-bd"))
        .output()
        .expect("status runs");
    assert!(!output.status.success());
    let text = String::from_utf8_lossy(&output.stdout).into_owned();
    assert!(text.contains("`bd` was not found"));
    assert!(text.contains("no server is running"), "the fallback is announced: {text}");
}

#[test]
fn the_subcommands_that_need_a_server_say_so_when_there_is_none() {
    let dir = tempfile::tempdir().unwrap();
    let output = Command::new(EXE).args(["devices", "--data-dir"]).arg(dir.path()).output().unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("smetanad run"));
}

#[test]
fn devices_lists_the_cli_itself_and_a_second_server_on_the_directory_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().join("data");
    let _server = Running::start(&data);
    block_on(async {
        wait_for("the credential file", || smetanad::local::path(&data).exists()).await;
    });
    let output = Command::new(EXE).args(["devices", "--data-dir"]).arg(&data).output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = String::from_utf8_lossy(&output.stdout).into_owned();
    assert!(text.contains("desktop") && text.contains("smetanad (this machine)"), "{text}");

    let second = Command::new(EXE)
        .args(["run", "--data-dir"])
        .arg(&data)
        .arg("--bd")
        .arg(bd())
        .output()
        .unwrap();
    assert!(!second.status.success());
    assert!(String::from_utf8_lossy(&second.stderr).contains("already owns"));
}
