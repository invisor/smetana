//! The bodies of the companion's commands, over the service rather than over a window's handle:
//! the desktop shell's `#[tauri::command]` wrappers and the bridge's dispatcher (`dispatch.rs`) both
//! call these, so a desktop device managing a server's devices does exactly what the Devices tab does.

use std::str::FromStr;
use std::sync::Arc;

use remote_protocol::{DeviceKind, PublicKey};

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
