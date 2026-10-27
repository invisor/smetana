//! The first launch of the client on a device that has an older single-file `settings.json`: what every
//! release before the switch-over wrote, since it held its workers in-process.
//!
//! That file holds both halves. The device half stays where it is. The server half is offered to the
//! server once (`settings_adopt`, which a server that has settings of its own refuses), and when the
//! server has answered, the device file is relieved of it (`settings::split::retire_server_half`: a
//! copy of the file as it was goes beside it first). It runs on every connection and is a read of one
//! small file when there is nothing to do; a failure leaves the file as it was, so the next
//! connection tries again.
//!
//! A second source is the file an earlier build of the client left on the *server*: the separate
//! "Smetana Next" build of step 5 forwarded the whole document there, so such an install has its
//! theme, layout and per-project tabs in the server's `settings.json` and only `dialogs` on the device. That device half is fetched once
//! (`settings_device_half`) and written into the device file together with the `deviceSeeded` mark.
//!
//! A third source is the look of a desktop that was already split: its theme, density, the board's
//! rules and every project's column order used to be the device's, and are the server's now, because a
//! paired phone draws them. A file not yet marked `lookShared` has them sent once as a patch
//! (`split::look_patch`) and is marked when the server has answered; a failure leaves the file as it
//! was and the next connection tries again. It goes last on purpose: source one hands the old file's
//! server half over with `settings_adopt`, which takes only a server with no settings, and a look
//! patch sent first would give it some. When the server refuses the adoption the look is sent with
//! `settings_patch` before the retiring write, which clears it. A file source one has retired, or source
//! two has seeded, holds no look and is marked `lookShared` in the same write, so source three only
//! skips it. The patch is always the whole look, defaults included: a person who went back to dark chose it. A desktop that meets a server with a look of its own
//! overwrites it with its own: the last one to migrate wins.
//!
//! It runs before the link reports `connected`, so no read of the settings can be answered ahead of
//! it and show the server's defaults.

use std::path::Path;

use serde_json::json;
use smetana_core::settings::{file, split};
use smetanad::client::Client;

pub async fn run(client: &Client, path: &Path) {
    // Source one: the device's own old file holds the whole document (what a release install has on its
    // first start after the switch-over). Its server half goes to the server; its device half is already
    // where it belongs.
    let handed_over = match hand_over_server_half(client, path).await {
        Handover::Failed => return,
        Handover::Done => true,
        Handover::Nothing => false,
    };
    // Source two: the device file has not been settled yet (`deviceSeeded`), and an earlier build of the
    // client kept the whole document on the *server* (step 5 forwarded `settings_save`). Its device half
    // is fetched and becomes the device file's, once. When source one fired the device's own half is the
    // one that stands, and only the mark is set.
    let owned = path.to_owned();
    let seeded = tokio::task::spawn_blocking(move || file::load(&owned).0.device_seeded).await.unwrap_or(true);
    if !seeded {
        if !seed(client, path, handed_over).await {
            return;
        }
    }
    // Source three: the device's look goes up once.
    share_look(client, path).await;
}

/// Source two. Answers whether the run may go on to source three.
async fn seed(client: &Client, path: &Path, handed_over: bool) -> bool {
    let from_server = if handed_over {
        None
    } else {
        match client.call("settings_device_half", json!({})).await {
            Ok(half) if half.is_null() => None,
            Ok(half) => Some(half),
            Err(err) => {
                log::warn!("client: the server's copy of this device's settings was not fetched: {err}");
                return false;
            }
        }
    };
    let owned = path.to_owned();
    match tokio::task::spawn_blocking(move || split::seed_device(&owned, from_server.as_ref())).await {
        Ok(Ok(_)) => {}
        Ok(Err(err)) => log::warn!("client: the device settings were not seeded: {err}"),
        Err(err) => log::warn!("client: the settings seeding did not finish: {err}"),
    }
    true
}

/// Source three: sends the look the device file holds, if it has not been sent, and marks the file.
async fn share_look(client: &Client, path: &Path) {
    let owned = path.to_owned();
    let patch = match tokio::task::spawn_blocking(move || {
        let (settings, problem) = file::load(&owned);
        (problem.is_none() && !settings.look_shared).then(|| split::look_patch(&settings))
    })
    .await
    {
        Ok(Some(patch)) => patch,
        _ => return,
    };
    if let Err(err) = client.call("settings_patch", json!({ "patch": patch })).await {
        log::warn!("client: the device's look was not shared with the server: {err}");
        return;
    }
    let owned = path.to_owned();
    match tokio::task::spawn_blocking(move || split::mark_look_shared(&owned)).await {
        Ok(Ok(_)) => {}
        Ok(Err(err)) => log::warn!("client: the look was shared but the device file was not marked: {err}"),
        Err(err) => log::warn!("client: the look sharing did not finish: {err}"),
    }
}

enum Handover {
    /// The file holds no server half.
    Nothing,
    /// It did, and the server has answered.
    Done,
    /// It did, and the server could not be asked: nothing else is done this time.
    Failed,
}

/// Offers the old file's server half to the server and relieves the file of it.
async fn hand_over_server_half(client: &Client, path: &Path) -> Handover {
    let owned = path.to_owned();
    let patch = match tokio::task::spawn_blocking(move || split::legacy_server_patch(&file::load(&owned).0)).await {
        Ok(Some(patch)) => patch,
        _ => return Handover::Nothing,
    };
    let adopted = match client.call("settings_adopt", json!({ "patch": patch })).await {
        Ok(value) => value.as_bool().unwrap_or(false),
        Err(err) => {
            log::warn!("client: the old settings file's server half was not handed over: {err}");
            return Handover::Failed;
        }
    };
    if !adopted {
        // The server kept its own settings, so the look in this file did not travel inside the half.
        // It is sent now, before the retiring write clears it, or it would be lost.
        let owned = path.to_owned();
        let look = tokio::task::spawn_blocking(move || {
            let (settings, problem) = file::load(&owned);
            (problem.is_none() && !settings.look_shared).then(|| split::look_patch(&settings))
        })
        .await
        .unwrap_or(None);
        if let Some(patch) = look {
            if let Err(err) = client.call("settings_patch", json!({ "patch": patch })).await {
                log::warn!("client: the old settings file's look was not handed over: {err}");
                return Handover::Failed;
            }
        }
        log::info!("client: the server already has settings; this device's old server half is kept only in settings.pre-split.json");
    }
    let owned = path.to_owned();
    match tokio::task::spawn_blocking(move || split::retire_server_half(&owned)).await {
        Ok(Ok(_)) => {}
        Ok(Err(err)) => log::warn!("client: the device settings file was not relieved of the server's half: {err}"),
        Err(err) => log::warn!("client: the settings migration did not finish: {err}"),
    }
    Handover::Done
}

#[cfg(test)]
mod tests {
    use super::*;
    use smetanad::server::{Options, Server};
    use std::time::Duration;

    fn bd() -> std::path::PathBuf {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("binaries");
        std::fs::read_dir(&dir)
            .ok()
            .and_then(|entries| {
                entries
                    .flatten()
                    .map(|entry| entry.path())
                    .find(|path| path.file_name().is_some_and(|name| name.to_string_lossy().starts_with("bd-")))
            })
            .unwrap_or_else(|| panic!("no bd sidecar in {}: run `npm run fetch-bd`", dir.display()))
    }

    #[test]
    fn a_split_devices_look_is_sent_once_and_the_file_is_marked() {
        let runtime = tokio::runtime::Builder::new_multi_thread().enable_all().build().expect("a runtime");
        runtime.block_on(async {
            let dir = tempfile::tempdir().unwrap();
            let server_dir = dir.path().join("server");
            let mut options = Options::new(server_dir.clone());
            options.bd = Some(bd());
            let server = Server::start(options).await.expect("the server starts");
            let device = dir.path().join("device").join("settings.json");
            std::fs::create_dir_all(device.parent().unwrap()).unwrap();
            std::fs::write(&device, r#"{"version":1,"deviceSeeded":true,"appearance":{"theme":"light"},"kanban":{"columns":"non-empty"}}"#).unwrap();

            let client = tokio::time::timeout(Duration::from_secs(30), Client::connect_as(&server_dir, dir.path()))
                .await
                .expect("in time")
                .expect("connected");
            let before = client.call("settings_load", json!({ "project": null })).await.unwrap();
            assert_eq!(before["appearance"]["theme"], "dark");

            run(&client, &device).await;
            let after = client.call("settings_load", json!({ "project": null })).await.unwrap();
            assert_eq!(after["appearance"]["theme"], "light");
            assert_eq!(after["kanban"]["columns"], "non-empty");
            assert!(file::load(&device).0.look_shared);

            // A second run sends nothing: another client's change in between is not overwritten.
            client.call("settings_patch", json!({ "patch": { "appearance": { "theme": "dark" } } })).await.unwrap();
            run(&client, &device).await;
            let again = client.call("settings_load", json!({ "project": null })).await.unwrap();
            assert_eq!(again["appearance"]["theme"], "dark", "a marked file sends no settings_patch");

            client.close();
            server.shutdown().await;
        });
    }

    /// A server and a client connected to it, with a device file holding `device` and a server that
    /// has been patched with `held` first.
    async fn meet(device_text: &str, held: serde_json::Value) -> (Client, std::path::PathBuf, Server, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let server_dir = dir.path().join("server");
        let mut options = Options::new(server_dir.clone());
        options.bd = Some(bd());
        let server = Server::start(options).await.expect("the server starts");
        let device = dir.path().join("device").join("settings.json");
        std::fs::create_dir_all(device.parent().unwrap()).unwrap();
        std::fs::write(&device, device_text).unwrap();
        let client = tokio::time::timeout(Duration::from_secs(30), Client::connect_as(&server_dir, dir.path()))
            .await
            .expect("in time")
            .expect("connected");
        client.call("settings_patch", json!({ "patch": held })).await.unwrap();
        (client, device, server, dir)
    }

    #[test]
    fn an_old_files_look_survives_a_server_that_refuses_to_adopt_its_half() {
        let runtime = tokio::runtime::Builder::new_multi_thread().enable_all().build().expect("a runtime");
        runtime.block_on(async {
            let (client, device, server, _dir) = meet(
                r#"{"version":1,"appearance":{"theme":"light","density":"compact"},"kanban":{"columns":"non-empty"},
                    "agent":"codex","agentPrompt":"mine","projects":{"/work/a":{"columnOrder":["done"]}}}"#,
                json!({ "agentPrompt": "the servers" }),
            )
            .await;
            run(&client, &device).await;
            let after = client.call("settings_load", json!({ "project": "/work/a" })).await.unwrap();
            assert_eq!(after["agentPrompt"], "the servers", "the server kept its own settings");
            assert_eq!(after["appearance"]["theme"], "light");
            assert_eq!(after["appearance"]["density"], "compact");
            assert_eq!(after["kanban"]["columns"], "non-empty");
            assert_eq!(after["project"]["columnOrder"], json!(["done"]));
            let kept = file::load(&device).0;
            assert!(kept.look_shared && kept.device_seeded);
            client.close();
            server.shutdown().await;
        });
    }

    #[test]
    fn a_look_gone_back_to_the_defaults_overwrites_a_stale_copy_on_the_server() {
        let runtime = tokio::runtime::Builder::new_multi_thread().enable_all().build().expect("a runtime");
        runtime.block_on(async {
            let (client, device, server, _dir) = meet(
                r#"{"version":1,"deviceSeeded":true}"#,
                json!({ "appearance": { "theme": "light" }, "kanban": { "columns": "non-empty" } }),
            )
            .await;
            run(&client, &device).await;
            let after = client.call("settings_load", json!({ "project": null })).await.unwrap();
            assert_eq!(after["appearance"]["theme"], "dark");
            assert_eq!(after["kanban"]["columns"], "all");
            assert!(file::load(&device).0.look_shared);
            client.close();
            server.shutdown().await;
        });
    }
}
