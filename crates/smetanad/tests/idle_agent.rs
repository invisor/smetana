//! An agent session that has gone quiet is waiting for a person, not working, so `server_restart`
//! `now` restarts over it without asking.
//!
//! Its own test binary, not a test in `server.rs`: the agent has to be a script this test provides
//! rather than whatever `claude` the machine has, and where a harness is looked for is process-wide
//! (the login shell's `PATH`, read once). Pointing `SHELL` at nothing makes that read fall back to
//! this process's `PATH`, which would also break the tests in `server.rs` that open a real shell.

mod common;

use std::time::Duration;

use common::bd;
use serde_json::json;
use smetanad::client::Client;
use smetanad::server::{Exit, Options, Server};

#[test]
fn restart_now_over_an_idle_agent_restarts_without_asking() {
    common::block_on(async {
        let bin = tempfile::tempdir().unwrap();
        let fake = bin.path().join("claude");
        std::fs::write(&fake, "#!/bin/sh\nexec sleep 600\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let path = std::env::var_os("PATH").unwrap_or_default();
        let mut paths = vec![bin.path().to_path_buf()];
        paths.extend(std::env::split_paths(&path));
        // Safe here: this binary has one test, and nothing has read the environment yet.
        std::env::set_var("PATH", std::env::join_paths(paths).unwrap());
        std::env::set_var("SHELL", "/nonexistent/smetana-test-shell");

        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("project");
        std::fs::create_dir_all(&project).unwrap();
        let mut options = Options::new(dir.path().join("data"));
        options.bd = Some(bd());
        options.restart_poll = Duration::from_millis(100);
        let server = Server::start(options).await.expect("the server starts");
        let client = Client::connect(&server.data_dir).await.unwrap();

        let project = project.to_string_lossy().into_owned();
        let created = client
            .call("terminal_create", json!({ "project": project, "intent": { "kind": "bare" } }))
            .await
            .expect("an agent session starts");
        let id = created["id"].as_u64().expect("a session id");

        let mut state = String::new();
        for _ in 0..200 {
            let sessions = client.call("terminal_list", json!({ "project": project })).await.unwrap();
            state = sessions
                .as_array()
                .unwrap()
                .iter()
                .find(|s| s["id"] == id)
                .and_then(|s| s["state"].as_str())
                .unwrap_or_default()
                .to_owned();
            if state == "idle" {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        assert_eq!(state, "idle", "the silent agent reaches idle");

        let reply = client.call("server_restart", json!({ "mode": "now", "confirmed": null })).await.unwrap();
        assert_eq!(reply["outcome"], "restarting", "{reply}");
        let exit = tokio::time::timeout(Duration::from_secs(5), server.wait()).await.expect("the server restarts");
        assert_eq!(exit, Exit::Restart);
        server.shutdown().await;
    });
}
