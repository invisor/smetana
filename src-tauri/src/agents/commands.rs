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
