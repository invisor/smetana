//! The one command, and the reason it is `async` for work that never awaits.

use std::path::PathBuf;

use super::model::SessionSummary;
use super::read;

/// The sessions of a project, newest first.
///
/// **The work runs on the blocking pool**, and the rule is `vcs/commands.rs`'s,
/// which `files/commands.rs` follows for the same reason: every IPC call in this
/// app — the board, the file tree, the editor, the terminals — is polled on one
/// shared runtime, so work parked in the body of an `async fn` takes a worker
/// out of everything else on screen with nothing saying why. This one opens
/// every transcript of a project, seeks each at both ends and streams the whole
/// of the ones that belong: on the machine this was written against that is 276
/// files and 291 MB of reading for one project, which is not a thing to do on a
/// runtime worker.
///
/// No `Result`. A machine with no Claude Code on it, a folder that cannot be
/// read, a transcript in a shape nobody here has seen — all of them mean the
/// list is shorter, and none of them is something to tell a person about. See
/// this module's header.
#[tauri::command]
pub async fn sessions_list(project: String) -> Vec<SessionSummary> {
    tokio::task::spawn_blocking(move || read::list(&PathBuf::from(project)))
        .await
        .unwrap_or_default()
}
