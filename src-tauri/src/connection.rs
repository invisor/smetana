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

/// Which server this device uses: `{ target: { kind: "local" } }`, or a remote one with where it is.
/// The build without `split` has no choice to make and says so with `in-process`.
#[tauri::command]
pub fn server_target(app: AppHandle) -> Value {
    #[cfg(feature = "split")]
    {
        crate::split::target_view(&app)
    }
    #[cfg(not(feature = "split"))]
    {
        let _ = app;
        json!({ "target": { "kind": "in-process" } })
    }
}

/// Back to the server on this machine.
#[tauri::command]
pub fn server_use_local(app: AppHandle) -> Result<Value, String> {
    #[cfg(feature = "split")]
    {
        crate::split::use_local(&app)
    }
    #[cfg(not(feature = "split"))]
    {
        let _ = app;
        Err("This build has no server to choose: its back end is in this process.".to_owned())
    }
}

/// To the server a desktop pairing ticket names. See `split::use_remote`.
#[tauri::command]
pub async fn server_use_remote(app: AppHandle, ticket: String, stop_local: bool) -> Result<Value, String> {
    #[cfg(feature = "split")]
    {
        crate::split::use_remote(&app, ticket, stop_local).await
    }
    #[cfg(not(feature = "split"))]
    {
        let _ = (app, ticket, stop_local);
        Err("This build has no server to choose: its back end is in this process.".to_owned())
    }
}

/// What the server on this machine is doing that a switch away from it would leave running.
#[tauri::command]
pub async fn server_local_work(app: AppHandle) -> Value {
    #[cfg(feature = "split")]
    {
        crate::split::local_work(&app).await
    }
    #[cfg(not(feature = "split"))]
    {
        let _ = app;
        json!([])
    }
}
