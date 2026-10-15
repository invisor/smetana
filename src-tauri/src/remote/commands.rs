//! The Tauri commands of the companion: thin, with the logic in `Remote`.

use std::str::FromStr;
use std::sync::Arc;

use remote_protocol::PublicKey;
use tauri::{AppHandle, Manager};

use super::devices::Device;
use super::{PairBegin, Remote};

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

/// Starts the endpoint if it is not running, issues a ticket and answers with the
/// string for the QR code and the second it stops being good.
#[tauri::command]
pub async fn remote_pair_begin(app: AppHandle) -> Result<PairBegin, RemoteError> {
    remote(&app)?.pair_begin().await.map_err(RemoteError::Failed)
}

/// Withdraws the ticket, and refuses a phone that is waiting to be confirmed.
#[tauri::command]
pub fn remote_pair_cancel(app: AppHandle) -> Result<(), RemoteError> {
    remote(&app)?.pairing.cancel();
    Ok(())
}

/// The person's answer to `remote:pair-request`.
#[tauri::command]
pub fn remote_pair_decide(app: AppHandle, accept: bool) -> Result<(), RemoteError> {
    remote(&app)?.pairing.decide(accept);
    Ok(())
}

#[tauri::command]
pub fn remote_devices(app: AppHandle) -> Result<Vec<Device>, RemoteError> {
    Ok(remote(&app)?.devices.all())
}

/// Removes the device and closes its connections.
#[tauri::command]
pub fn remote_device_revoke(app: AppHandle, key: String) -> Result<(), RemoteError> {
    let key = PublicKey::from_str(&key).map_err(|_| RemoteError::BadKey)?;
    remote(&app)?.revoke(&key).map_err(RemoteError::Failed)
}
