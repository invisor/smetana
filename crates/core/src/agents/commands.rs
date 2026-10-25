//! What the front end may ask about the harnesses themselves.


pub async fn codex_models() -> Result<Vec<super::AgentModel>, String> {
    super::codex::listed_models().await.map(|models| models.into_iter().map(|(id, label)| super::AgentModel { id, label }).collect())
}

/// Which harnesses are on the login shell's `PATH` right now. Asked when the
/// review window opens rather than once at startup: a build's harnesses cannot
/// change while the app runs, but what is installed can.
pub fn agents_installed() -> Vec<String> {
    super::installed(crate::shell_env::path()).into_iter().map(str::to_owned).collect()
}
