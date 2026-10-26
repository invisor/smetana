//! The two commands the front end's connection store calls, answered in both builds.
//!
//! The build without the `split` feature has no connection to draw: its back end is in the same
//! process and cannot be away. It answers `connection_state` all the same, with `in-process`, so the
//! store reads one shape in either build and nothing in the front end branches on which it is.

#[cfg(not(feature = "split"))]
use serde_json::json;
use serde_json::Value;
use tauri::AppHandle;

/// What the connection is doing: see `split::state::View` for the `split` build's answer.
#[tauri::command]
pub fn connection_state(app: AppHandle) -> Value {
    #[cfg(feature = "split")]
    {
        crate::split::snapshot(&app)
    }
    #[cfg(not(feature = "split"))]
    {
        let _ = app;
        json!({ "mode": "in-process", "state": "connected", "epoch": 0 })
    }
}

/// Try to connect now rather than at the end of the wait. Nothing to do in-process.
#[tauri::command]
pub fn connection_retry(app: AppHandle) {
    #[cfg(feature = "split")]
    crate::split::retry(&app);
    #[cfg(not(feature = "split"))]
    let _ = app;
}
