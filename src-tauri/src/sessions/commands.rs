//! The one command, and the reason it is `async` for work that never awaits.

use std::path::PathBuf;

use super::model::SessionSummary;
use super::{act, read};

/// A machine with no `HOME` at all, which is the one state in which neither
/// verb below can decide anything. Said in words rather than answered with
/// "that is not a transcript", which would be a claim about the path.
const NO_ROOT: &str = "There is no home directory to find Claude Code's transcripts under.";

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
///
/// **The log line is the one exception to that, and it is what keeps the
/// promise from being a quiet lie** — the argument is `vcs/commands.rs`'s
/// `off_the_runtime_or_empty`, made here for one caller rather than borrowed,
/// since lifting that helper would mean disturbing a module this task has no
/// business in and wrapping a single call site buys nothing but a layer. A
/// blocking task that panicked, or a runtime shutting down under it, is the one
/// outcome here that is genuinely wrong: nothing looked at the disk, and yet
/// the empty list this hands back is drawn as "no sessions yet", which is a
/// statement about a person's history and not a shrug. The shape of the answer
/// has no room for a refusal, so it goes where a developer will find it.
#[tauri::command]
pub async fn sessions_list(project: String) -> Vec<SessionSummary> {
    tokio::task::spawn_blocking(move || read::list(&PathBuf::from(project)))
        .await
        .unwrap_or_else(|err| {
            log::error!(
                "sessions: the read of ~/.claude/projects gave way and is being reported as \
                 no sessions at all: {err}"
            );
            Vec::new()
        })
}

/// A session's transcript, or the directory it ran in, handed to the desktop.
///
/// A `Result` where the command above has none, and the difference is the whole
/// of `act.rs`'s header: this is a verb somebody pressed, and a press that does
/// nothing and says nothing is the one outcome the Sessions tab may not have.
/// The message is written for a person, since it is put on screen as it stands.
///
/// On the blocking pool for the same reason as the read: `open_path` stats the
/// file and then spawns whatever the system has registered, and neither of
/// those belongs on a runtime worker shared with the board and the terminals.
#[tauri::command]
pub async fn sessions_open(path: String) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        let root = read::projects_root().ok_or_else(|| NO_ROOT.to_owned())?;
        act::open(&PathBuf::from(path), &root)
    })
    .await
    .unwrap_or_else(|err| Err(format!("The open did not run: {err}")))
}

/// One transcript, deleted.
///
/// The confirmation is the front end's — `views/dialogRegistry.js`'s
/// `delete-session` window, which names the id, the path and the size — and
/// this command is deliberately not the place that asks: a dialog raised from
/// Rust would be a second vocabulary for the same question and could not draw
/// in this app's own tokens.
///
/// What stands here instead is the guard, and it is not a formality: the path
/// arrives from the webview, so `sessions::model::is_transcript` is the whole
/// of what keeps this command from being "delete any file on the machine".
#[tauri::command]
pub async fn sessions_delete(path: String) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        let root = read::projects_root().ok_or_else(|| NO_ROOT.to_owned())?;
        act::delete(&PathBuf::from(path), &root)
    })
    .await
    .unwrap_or_else(|err| Err(format!("The delete did not run: {err}")))
}
