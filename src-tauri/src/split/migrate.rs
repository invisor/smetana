//! The first launch of the client build on a device that has an older single-file `settings.json`.
//!
//! That file holds both halves. The device half stays where it is. The server half is offered to the
//! server once (`settings_adopt`, which a server that has settings of its own refuses), and when the
//! server has answered, the device file is relieved of it (`settings::split::retire_server_half`: a
//! copy of the file as it was goes beside it first). It runs on every connection and is a read of one
//! small file when there is nothing to do; a failure leaves the file as it was, so the next
//! connection tries again.
//!
//! It runs before the link reports `connected`, so no read of the settings can be answered ahead of
//! it and show the server's defaults.

use std::path::Path;

use serde_json::json;
use smetana_core::settings::{file, split};
use smetanad::client::Client;

pub async fn run(client: &Client, path: &Path) {
    let owned = path.to_owned();
    let patch = match tokio::task::spawn_blocking(move || split::legacy_server_patch(&file::load(&owned).0)).await {
        Ok(Some(patch)) => patch,
        _ => return,
    };
    let adopted = match client.call("settings_adopt", json!({ "patch": patch })).await {
        Ok(value) => value.as_bool().unwrap_or(false),
        Err(err) => {
            log::warn!("split: the old settings file's server half was not handed over: {err}");
            return;
        }
    };
    if !adopted {
        log::info!("split: the server already has settings; this device's old server half is kept only in settings.pre-split.json");
    }
    let owned = path.to_owned();
    match tokio::task::spawn_blocking(move || split::retire_server_half(&owned)).await {
        Ok(Ok(_)) => {}
        Ok(Err(err)) => log::warn!("split: the device settings file was not relieved of the server's half: {err}"),
        Err(err) => log::warn!("split: the settings migration did not finish: {err}"),
    }
}
