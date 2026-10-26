//! The Tauri commands of the companion: thin, with the logic in `Remote`.

use std::str::FromStr;
use std::sync::Arc;

use remote_protocol::{DeviceKind, PublicKey};
use tauri::{AppHandle, Manager};

use super::{device_views, DeviceView, PairBegin, Remote};

#[derive(Debug, thiserror::Error)]
pub enum RemoteError {
    #[error("the companion is not available on this machine")]
    Unavailable,
    #[error("{0}")]
    Failed(String),
    #[error("not a device key")]
    BadKey,
}

impl serde::Serialize for RemoteError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

fn remote(app: &AppHandle) -> Result<Arc<Remote>, RemoteError> {
    app.try_state::<Arc<Remote>>().map(|state| state.inner().clone()).ok_or(RemoteError::Unavailable)
}

/// The bodies of the commands, over the service rather than Tauri's `AppHandle`: the Tauri
/// wrappers below and the bridge's dispatcher (`dispatch.rs`) both call these, so a desktop
/// device managing a server's devices does exactly what the Devices tab does.
pub mod plain {
    use super::*;

    pub async fn pair_begin(
        remote: &Arc<Remote>,
        relay_url: &str,
        kind: DeviceKind,
    ) -> Result<PairBegin, RemoteError> {
        remote.pair_begin(relay_url, kind).await.map_err(RemoteError::Failed)
    }

    pub fn pair_cancel(remote: &Arc<Remote>) -> Result<(), RemoteError> {
        remote.pairing.cancel();
        Ok(())
    }

    pub fn pair_decide(remote: &Arc<Remote>, accept: bool) -> Result<(), RemoteError> {
        if remote.pairing.decide(accept) {
            Ok(())
        } else {
            Err(RemoteError::Failed("There is no pairing request to answer.".into()))
        }
    }

    pub fn devices(remote: &Arc<Remote>) -> Result<Vec<DeviceView>, RemoteError> {
        Ok(device_views(&remote.devices.all()))
    }

    pub fn revoke(remote: &Arc<Remote>, key: &str) -> Result<(), RemoteError> {
        let key = PublicKey::from_str(key).map_err(|_| RemoteError::BadKey)?;
        remote.revoke(&key).map_err(RemoteError::Failed)
    }
}

/// Starts the endpoint if it is not running on `relay_url`, issues a ticket and
/// answers with the string for the QR code and the second it stops being good.
/// The address is the one in the field, passed in so that it cannot lag the
/// settings file by a debounce. `kind` is what the device that spends the ticket will be
/// allowed to do; absent, a phone.
#[tauri::command]
pub async fn remote_pair_begin(
    app: AppHandle,
    relay_url: String,
    kind: Option<DeviceKind>,
) -> Result<PairBegin, RemoteError> {
    plain::pair_begin(&remote(&app)?, &relay_url, kind.unwrap_or_default()).await
}

/// Withdraws the ticket, and refuses a phone that is waiting to be confirmed.
#[tauri::command]
pub fn remote_pair_cancel(app: AppHandle) -> Result<(), RemoteError> {
    plain::pair_cancel(&remote(&app)?)
}

/// The person's answer to `remote:pair-request`. An error when there is nothing
/// to answer — the request timed out, or the phone left, a moment ago — so the
/// screen does not report a success that did not happen.
#[tauri::command]
pub fn remote_pair_decide(app: AppHandle, accept: bool) -> Result<(), RemoteError> {
    plain::pair_decide(&remote(&app)?, accept)
}

#[tauri::command]
pub fn remote_devices(app: AppHandle) -> Result<Vec<DeviceView>, RemoteError> {
    plain::devices(&remote(&app)?)
}

/// Removes the device and closes its connections.
#[tauri::command]
pub fn remote_device_revoke(app: AppHandle, key: String) -> Result<(), RemoteError> {
    plain::revoke(&remote(&app)?, &key)
}
