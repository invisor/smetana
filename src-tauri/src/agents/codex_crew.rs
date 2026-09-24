//! Codex app-server Crew adapter.
//!
//! Codex multi-agent workers are ordinary app-server threads. The adapter
//! consumes `thread/*` and `collabAgentToolCall` JSON-RPC records, never a
//! rendered terminal. Provider thread ids remain private to `session::crew`.

use serde_json::Value;

use super::crew::{ProviderNode, ProviderState};

pub const MIN_VERSION: (u32, u32, u32) = (0, 155, 1);

/// A `thread/started` notification or `thread/list` response can carry one or
/// many thread records. Ignore a malformed record rather than inventing a row.
pub fn nodes(value: &Value) -> Vec<ProviderNode> {
    let records: Vec<&Value> = match value.get("method").and_then(Value::as_str) {
        Some("thread/started") => value.pointer("/params/thread").into_iter().collect(),
        _ => value
            .pointer("/result/data")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .collect(),
    };
    records.into_iter().filter_map(thread_node).collect()
}

/// A status change has only an id and status, so it updates a previously
/// discovered thread without replacing its label/parent details.
pub fn status_change(value: &Value) -> Option<(String, ProviderState, bool)> {
    (value.get("method").and_then(Value::as_str) == Some("thread/status/changed")).then_some(())?;
    let params = value.get("params")?;
    let id = params.get("threadId").and_then(Value::as_str)?.to_owned();
    let status = params.get("status").and_then(Value::as_str)?;
    Some((id, state(status), !matches!(status, "systemError")))
}

/// A collaboration tool call includes lifecycle states for all receiving
/// workers. These are terminal even if a subsequent thread/list snapshot has
/// not yet caught up.
pub fn collaboration_states(value: &Value) -> Vec<(String, ProviderState, bool)> {
    let item = value.pointer("/params/item").or_else(|| value.get("item"));
    let Some(item) = item else { return Vec::new() };
    if item.get("type").and_then(Value::as_str) != Some("collabAgentToolCall") {
        return Vec::new();
    }
    item.get("agentsStates")
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
        .filter_map(|(id, entry)| {
            let status = entry.get("status").and_then(Value::as_str)?;
            let state = match status {
                "pendingInit" => ProviderState::Starting,
                "running" => ProviderState::Running,
                "completed" | "shutdown" | "notFound" => ProviderState::Done,
                "interrupted" | "errored" => ProviderState::Failed,
                _ => return None,
            };
            Some((
                id.clone(),
                state,
                matches!(state, ProviderState::Starting | ProviderState::Running),
            ))
        })
        .collect()
}

/// A selected Codex child is addressed with a normal `turn/start` on exactly
/// its provider thread id. The run transport adds its standard model/sandbox
/// fields; this is the invariant part that must never switch to the lead.
pub fn addressed_turn(thread_id: &str, text: &str) -> Value {
    serde_json::json!({
        "method": "turn/start",
        "params": {"threadId": thread_id, "input": [{"type": "text", "text": text}]}
    })
}

fn thread_node(thread: &Value) -> Option<ProviderNode> {
    let id = thread.get("id").and_then(Value::as_str)?.to_owned();
    let raw_status = thread
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("notLoaded");
    let state = state(raw_status);
    let label = thread
        .get("agentNickname")
        .or_else(|| thread.get("agentRole"))
        .and_then(Value::as_str)
        .map(str::to_owned);
    Some(ProviderNode {
        id,
        parent: thread
            .get("parentThreadId")
            .and_then(Value::as_str)
            .map(str::to_owned),
        state,
        label,
        can_message: thread
            .get("canAcceptDirectInput")
            .and_then(Value::as_bool)
            .unwrap_or(false)
            && !matches!(raw_status, "systemError"),
    })
}

fn state(status: &str) -> ProviderState {
    match status {
        "notLoaded" => ProviderState::Starting,
        "idle" => ProviderState::Waiting,
        "systemError" => ProviderState::Failed,
        "active" => ProviderState::Running,
        _ => ProviderState::Starting,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thread_fixture_keeps_parent_and_direct_input_capability() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../tests/fixtures/codex-0.155.1-crew-threads.json"
        ))
        .unwrap();
        assert_eq!(
            nodes(&fixture),
            vec![ProviderNode {
                id: "child-thread".into(),
                parent: Some("lead-thread".into()),
                state: ProviderState::Running,
                label: Some("reviewer".into()),
                can_message: true,
            }]
        );
    }

    #[test]
    fn collaboration_fixture_marks_only_the_child_that_failed() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../tests/fixtures/codex-0.155.1-crew-collab.json"
        ))
        .unwrap();
        assert_eq!(
            collaboration_states(&fixture),
            vec![("child-thread".into(), ProviderState::Failed, false)]
        );
    }

    #[test]
    fn addressed_turn_never_targets_the_lead() {
        let turn = addressed_turn("child-thread", "UNIQUE-CHILD-MESSAGE");
        assert_eq!(
            turn.pointer("/params/threadId").and_then(Value::as_str),
            Some("child-thread")
        );
        assert_eq!(
            turn.pointer("/params/input/0/text").and_then(Value::as_str),
            Some("UNIQUE-CHILD-MESSAGE")
        );
    }
}
