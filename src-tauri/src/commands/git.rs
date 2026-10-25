use smetana_core::git::commands as logic;
use smetana_core::git::Head;

/// See `git::commands::git_head` in the core crate.
#[tauri::command]
pub fn git_head(project: String) -> Head {
    logic::git_head(project)
}
