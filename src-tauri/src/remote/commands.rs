//! The Tauri commands of the companion: thin, with the logic in `remote_server::commands::plain`.

use std::sync::Arc;

use remote_protocol::DeviceKind;
use remote_server::commands::{plain, RemoteError};
use remote_server::{DeviceView, PairBegin, Remote};
use tauri::{AppHandle, Manager};

fn remote(app: &AppHandle) -> Result<Arc<Remote>, RemoteError> {
    app.try_state::<Arc<Remote>>().map(|state| state.inner().clone()).ok_or(RemoteError::Unavailable)
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
