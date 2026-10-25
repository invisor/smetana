use smetana_core::agents::commands as logic;

/// Every shipped harness and what it can do.
///
/// Read once at startup rather than per menu: the answer has to be in hand
/// while a row is being *drawn*, and a row greyed a round trip later is a row
/// somebody has already pressed.
#[tauri::command]
pub fn agents_catalog() -> Vec<smetana_core::agents::AgentRow> {
    smetana_core::agents::catalogue()
}

/// See `agents::commands::codex_models` in the core crate.
#[tauri::command]
pub async fn codex_models() -> Result<Vec<smetana_core::agents::AgentModel>, String> {
    logic::codex_models().await
}

/// See `agents::commands::agents_installed` in the core crate.
#[tauri::command]
pub fn agents_installed() -> Vec<String> {
    logic::agents_installed()
}
