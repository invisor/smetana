//! The thin layer over `config.rs`. There is no worker to queue behind and no
//! state to guard: reading one file costs milliseconds, the same reasoning
//! that keeps `files/` and `git.rs` out of a worker.

use std::path::Path;

use super::config::{self, ConfigState};

/// Every outcome is a state, so this cannot fail: a project with no config is
/// the ordinary case, and an unreadable one is `Broken` with what the OS said.
#[tauri::command]
pub fn project_config(project: String) -> ConfigState {
    config::load(Path::new(&project))
}
