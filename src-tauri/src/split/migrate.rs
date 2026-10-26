//! The first launch of the client build on a device that has an older single-file `settings.json`.
//!
//! That file holds both halves. The device half stays where it is. The server half is offered to the
//! server once (`settings_adopt`, which a server that has settings of its own refuses), and when the
//! server has answered, the device file is relieved of it (`settings::split::retire_server_half`: a
//! copy of the file as it was goes beside it first). It runs on every connection and is a read of one
//! small file when there is nothing to do; a failure leaves the file as it was, so the next
//! connection tries again.
//!
//! A second source is the file an earlier build of the client left on the *server*: step 5 forwarded the
//! whole document there, so a Smetana Next install has its theme, layout and per-project tabs in the
//! server's `settings.json` and only `dialogs` on the device. That device half is fetched once
//! (`settings_device_half`) and written into the device file together with the `deviceSeeded` mark.
//!
//! It runs before the link reports `connected`, so no read of the settings can be answered ahead of
//! it and show the server's defaults.

use std::path::Path;

use serde_json::json;
use smetana_core::settings::{file, split};
use smetanad::client::Client;

pub async fn run(client: &Client, path: &Path) {
    // Source one: the device's own old file holds the whole document (what a release install has, once
    // the client build is the only one). Its server half goes to the server; its device half is already
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
    if seeded {
        return;
    }
    let from_server = if handed_over {
        None
    } else {
        match client.call("settings_device_half", json!({})).await {
            Ok(half) if half.is_null() => None,
            Ok(half) => Some(half),
            Err(err) => {
                log::warn!("split: the server's copy of this device's settings was not fetched: {err}");
                return;
            }
        }
    };
    let owned = path.to_owned();
    match tokio::task::spawn_blocking(move || split::seed_device(&owned, from_server.as_ref())).await {
        Ok(Ok(_)) => {}
        Ok(Err(err)) => log::warn!("split: the device settings were not seeded: {err}"),
        Err(err) => log::warn!("split: the settings seeding did not finish: {err}"),
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
            log::warn!("split: the old settings file's server half was not handed over: {err}");
            return Handover::Failed;
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
    Handover::Done
}
