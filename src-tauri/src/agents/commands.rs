//! What the front end may ask about the harnesses themselves.

/// Every shipped harness and what it can do.
///
/// Read once at startup rather than per menu: the answer has to be in hand
/// while a row is being *drawn*, and a row greyed a round trip later is a row
/// somebody has already pressed.
#[tauri::command]
pub fn agents_catalog() -> Vec<super::AgentRow> {
    super::catalogue()
}

#[tauri::command]
pub async fn codex_models() -> Result<Vec<super::AgentModel>, String> {
    super::codex::listed_models().await.map(|models| models.into_iter().map(|(id, label)| super::AgentModel { id, label }).collect())
}

/// Which harnesses are on the login shell's `PATH` right now. Asked when the
/// review window opens rather than once at startup: a build's harnesses cannot
/// change while the app runs, but what is installed can.
#[tauri::command]
pub fn agents_installed() -> Vec<String> {
    super::installed(crate::shell_env::path()).into_iter().map(str::to_owned).collect()
}
