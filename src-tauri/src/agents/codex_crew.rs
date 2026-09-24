//! Codex app-server Crew adapter.
//!
//! Codex multi-agent workers are ordinary app-server threads. The adapter
//! consumes `thread/*` and `collabAgentToolCall` JSON-RPC records, never a
//! rendered terminal. Provider thread ids remain private to `session::crew`.

use serde_json::Value;

use super::crew::{ProviderNode, ProviderState};
use crate::session::model::{Actor, EventKind};

pub const MIN_VERSION: (u32, u32, u32) = (0, 155, 1);

pub fn supports_version(found: &str) -> bool {
    let mut values = found
        .split(|character: char| !character.is_ascii_digit())
        .filter(|part| !part.is_empty())
        .filter_map(|part| part.parse::<u32>().ok());
    matches!(
        (values.next(), values.next(), values.next()),
        (Some(major), Some(minor), Some(patch)) if (major, minor, patch) >= MIN_VERSION
    )
}

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

/// Translate one non-lead app-server notification into the selected child's
/// journal. The root driver deliberately hands these records over before it
/// folds them, so interleaved child output can never enter the lead journal.
pub fn journal(value: &Value) -> Option<(String, Vec<EventKind>)> {
    let thread = value
        .pointer("/params/threadId")
        .or_else(|| value.pointer("/params/item/threadId"))
        .and_then(Value::as_str)?
        .to_owned();
    let method = value.get("method").and_then(Value::as_str)?;
    let events = match method {
        "turn/started" => vec![EventKind::TurnStart { by: Actor::Agent }],
        "item/agentMessage/delta" => value
            .pointer("/params/delta")
            .and_then(Value::as_str)
            .filter(|text| !text.is_empty())
            .map(|text| {
                vec![EventKind::TextDelta {
                    text: text.to_owned(),
                }]
            })
            .unwrap_or_default(),
        "item/completed" => completed(value.pointer("/params/item")?),
        "turn/completed" => {
            let turn = value.pointer("/params/turn")?;
            if let Some(error) = turn.pointer("/error/message").and_then(Value::as_str) {
                vec![EventKind::TurnFailed {
                    text: error.to_owned(),
                }]
            } else if turn.get("status").and_then(Value::as_str) == Some("failed") {
                vec![EventKind::TurnFailed {
                    text: "Codex turn failed".into(),
                }]
            } else {
                vec![EventKind::Result {
                    tokens_in: 0,
                    tokens_out: 0,
                    cost_usd: None,
                    ms: turn.get("durationMs").and_then(Value::as_u64).unwrap_or(0),
                }]
            }
        }
        "error" => value
            .pointer("/params/error/message")
            .and_then(Value::as_str)
            .map(|text| {
                vec![EventKind::Error {
                    text: text.to_owned(),
                }]
            })
            .unwrap_or_default(),
        _ => Vec::new(),
    };
    Some((thread, events))
}

/// A `thread/read(includeTurns:true)` response tagged by `CodexDriver` with
/// its requested child thread id. It is history, so the shared translator
/// deliberately produces no open `TurnStart` event.
pub fn hydrated_journal(value: &Value) -> Option<(String, Vec<EventKind>)> {
    let thread = value
        .get("crewThreadId")
        .and_then(Value::as_str)?
        .to_owned();
    let turns = value
        .pointer("/result/thread/turns")
        .and_then(Value::as_array)?;
    let mut events = crate::agents::codex_driver::translate_history(turns);
    // A terminal snapshot turn is the one terminal lifecycle fact already
    // present before its buffered `turn/completed` arrives. Unlike normal
    // conversation-history reopen, this is an active Crew child's live
    // journal, so retain that marker exactly once.
    for turn in turns {
        match turn.get("status").and_then(Value::as_str) {
            Some("completed") => events.push(EventKind::Result {
                tokens_in: 0,
                tokens_out: 0,
                cost_usd: None,
                ms: turn.get("durationMs").and_then(Value::as_u64).unwrap_or(0),
            }),
            Some("failed") => events.push(EventKind::TurnFailed {
                text: turn
                    .pointer("/error/message")
                    .and_then(Value::as_str)
                    .unwrap_or("Codex turn failed")
                    .to_owned(),
            }),
            _ => {}
        }
    }
    Some((thread, events))
}

/// Snapshot coverage is intentionally distinct from live-event identity. It
/// may suppress a buffered `item/completed` that was already in `thread/read`,
/// but can never suppress a later turn completion or a text delta.
pub fn hydrated_journal_keys(value: &Value) -> Option<(String, Vec<String>)> {
    let thread = value.get("crewThreadId")?.as_str()?.to_owned();
    let turns = value.pointer("/result/thread/turns")?.as_array()?;
    let mut keys = Vec::new();
    for turn in turns {
        if matches!(
            turn.get("status").and_then(Value::as_str),
            Some("completed" | "failed")
        ) {
            if let Some(id) = turn.get("id").and_then(Value::as_str) {
                keys.push(format!("snapshot:turn:{id}"));
            }
        }
        for item in turn.get("items").and_then(Value::as_array).into_iter().flatten() {
            if let Some(id) = item.get("id").and_then(Value::as_str) {
                keys.push(format!("snapshot:item:{id}"));
            }
        }
    }
    Some((thread, keys))
}

/// A live event's provider-local identity. A turn start and completion are
/// separate facts even with the same turn id. Deltas return no de-duplication
/// key: `params.itemId` identifies their stream, but every occurrence is
/// meaningful and must remain appendable.
pub fn journal_key(value: &Value) -> Option<(String, String)> {
    let thread = value.pointer("/params/threadId").or_else(|| value.pointer("/params/item/threadId")).and_then(Value::as_str)?.to_owned();
    let method = value.get("method")?.as_str()?;
    if method.ends_with("/delta") {
        // `itemId` is deliberately read (rather than pretending the message
        // text is identity), but no key is returned: identical consecutive
        // deltas are two real stream occurrences.
        let _ = value.pointer("/params/itemId").and_then(Value::as_str);
        return None;
    }
    let key = match method {
        "turn/started" => value
            .pointer("/params/turn/id")
            .and_then(Value::as_str)
            .map(|id| format!("live:turn-start:{id}")),
        "turn/completed" => value
            .pointer("/params/turn/id")
            .and_then(Value::as_str)
            .map(|id| format!("live:turn-complete:{id}")),
        "item/completed" => value
            .pointer("/params/item/id")
            .and_then(Value::as_str)
            .map(|id| format!("live:item-complete:{id}")),
        // A wire notification without an item/turn id has no stable provider
        // identity. Do not collapse two errors merely because their method is
        // both `error`; the worker appends every such occurrence.
        _ => None,
    }?;
    Some((thread, key))
}

/// The one buffered live fact a snapshot is allowed to cover. Lifecycle and
/// delta records deliberately answer `None` here.
pub fn buffered_snapshot_coverage(value: &Value) -> Option<(String, String)> {
    let thread = value.pointer("/params/threadId").and_then(Value::as_str)?.to_owned();
    match value.get("method").and_then(Value::as_str)? {
        "item/completed" => value
            .pointer("/params/item/id")
            .and_then(Value::as_str)
            .map(|item| (thread, format!("snapshot:item:{item}"))),
        "turn/completed" => value
            .pointer("/params/turn/id")
            .and_then(Value::as_str)
            .map(|turn| (thread, format!("snapshot:turn:{turn}"))),
        _ => None,
    }
}

fn completed(item: &Value) -> Vec<EventKind> {
    match item.get("type").and_then(Value::as_str) {
        Some("agentMessage") => item
            .get("text")
            .and_then(Value::as_str)
            .filter(|text| !text.is_empty())
            .map(|text| {
                vec![EventKind::Text {
                    text: text.to_owned(),
                }]
            })
            .unwrap_or_default(),
        Some("reasoning") => {
            let text = item
                .get("summary")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .chain(
                    item.get("content")
                        .and_then(Value::as_array)
                        .into_iter()
                        .flatten(),
                )
                .filter_map(Value::as_str)
                .filter(|text| !text.is_empty())
                .collect::<Vec<_>>()
                .join("\n");
            (!text.is_empty())
                .then_some(EventKind::Reasoning { text })
                .into_iter()
                .collect()
        }
        Some(
            kind @ ("commandExecution" | "fileChange" | "mcpToolCall" | "dynamicToolCall"
            | "webSearch"),
        ) => {
            let id = item
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or(kind)
                .to_owned();
            let ok = !matches!(
                item.get("status").and_then(Value::as_str),
                Some("failed" | "error")
            );
            vec![EventKind::ToolResult {
                id,
                ok,
                summary: kind.into(),
            }]
        }
        _ => Vec::new(),
    }
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

    #[test]
    fn interleaved_child_items_keep_their_provider_thread_identity() {
        let first = serde_json::json!({
            "method": "item/agentMessage/delta",
            "params": {"threadId": "child-one", "delta": "ONE"}
        });
        let second = serde_json::json!({
            "method": "item/agentMessage/delta",
            "params": {"threadId": "child-two", "delta": "TWO"}
        });
        assert_eq!(
            journal(&first),
            Some((
                "child-one".into(),
                vec![EventKind::TextDelta { text: "ONE".into() }]
            ))
        );
        assert_eq!(
            journal(&second),
            Some((
                "child-two".into(),
                vec![EventKind::TextDelta { text: "TWO".into() }]
            ))
        );
    }

    #[test]
    fn hydration_translates_only_the_requested_child_history() {
        let record = serde_json::json!({
            "crewThreadId": "child-two",
            "result": {"thread": {"turns": [{"items": [
                {"type": "agentMessage", "text": "only child two"}
            ]}]}}
        });
        assert_eq!(
            hydrated_journal(&record),
            Some((
                "child-two".into(),
                vec![EventKind::Text {
                    text: "only child two".into()
                }]
            ))
        );
    }

    #[test]
    fn snapshot_coverage_is_not_a_live_completion_identity() {
        let snapshot = serde_json::json!({
            "crewThreadId": "child-two",
            "result": {"thread": {"turns": [{
                "id": "turn-7",
                "status": "completed",
                "items": [{"id": "item-9", "type": "agentMessage", "text": "snapshot"}]
            }]}}
        });
        let live = serde_json::json!({
            "method": "item/completed",
            "params": {"threadId": "child-two", "item": {
                "id": "item-9", "type": "agentMessage", "text": "live duplicate"
            }}
        });
        let (_, keys) = hydrated_journal_keys(&snapshot).expect("snapshot keys");
        assert!(keys.contains(&"snapshot:turn:turn-7".into()));
        assert!(keys.contains(&"snapshot:item:item-9".into()));
        assert_eq!(
            journal_key(&live),
            Some(("child-two".into(), "live:item-complete:item-9".into()))
        );
        assert_eq!(
            buffered_snapshot_coverage(&live),
            Some(("child-two".into(), "snapshot:item:item-9".into()))
        );
    }

    #[test]
    fn snapshot_covers_only_a_buffered_turn_completion() {
        let started = serde_json::json!({
            "method": "turn/started",
            "params": {"threadId": "child", "turn": {"id": "turn-1"}}
        });
        let completed = serde_json::json!({
            "method": "turn/completed",
            "params": {"threadId": "child", "turn": {"id": "turn-1"}}
        });
        assert_eq!(journal_key(&started), Some(("child".into(), "live:turn-start:turn-1".into())));
        assert_eq!(journal_key(&completed), Some(("child".into(), "live:turn-complete:turn-1".into())));
        assert_eq!(buffered_snapshot_coverage(&started), None);
        assert_eq!(
            buffered_snapshot_coverage(&completed),
            Some(("child".into(), "snapshot:turn:turn-1".into()))
        );
    }

    #[test]
    fn failed_snapshot_materializes_and_covers_its_terminal_failure() {
        let snapshot = serde_json::json!({
            "crewThreadId": "child",
            "result": {"thread": {"turns": [{
                "id": "turn-failed",
                "status": "failed",
                "error": {"message": "rate limited"},
                "items": []
            }]}}
        });
        assert_eq!(
            hydrated_journal(&snapshot),
            Some((
                "child".into(),
                vec![EventKind::TurnFailed {
                    text: "rate limited".into()
                }]
            ))
        );
        assert_eq!(
            hydrated_journal_keys(&snapshot),
            Some(("child".into(), vec!["snapshot:turn:turn-failed".into()]))
        );
    }

    #[test]
    fn nonterminal_snapshot_never_covers_a_later_terminal_completion() {
        let snapshot = serde_json::json!({
            "crewThreadId": "child",
            "result": {"thread": {"turns": [{
                "id": "turn-live",
                "status": "inProgress",
                "items": []
            }]}}
        });
        assert_eq!(hydrated_journal(&snapshot), Some(("child".into(), Vec::new())));
        assert_eq!(hydrated_journal_keys(&snapshot), Some(("child".into(), Vec::new())));
    }

    #[test]
    fn deltas_with_an_item_id_are_occurrences_not_deduplication_keys() {
        let delta = serde_json::json!({
            "method": "item/agentMessage/delta",
            "params": {"threadId": "child", "itemId": "item-1", "delta": "same"}
        });
        assert_eq!(journal_key(&delta), None);
        assert_eq!(buffered_snapshot_coverage(&delta), None);
    }

    #[test]
    fn unkeyed_errors_are_independent_wire_occurrences() {
        let error = serde_json::json!({
            "method": "error",
            "params": {"threadId": "child", "error": {"message": "same error"}}
        });
        assert_eq!(journal_key(&error), None);
        assert_eq!(
            journal(&error),
            Some((
                "child".into(),
                vec![EventKind::Error {
                    text: "same error".into()
                }]
            ))
        );
    }
}
