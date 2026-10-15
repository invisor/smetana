//! The command bridge: what a paired phone says after the handshake, what the
//! desktop answers, and the one list that decides which commands and events a
//! phone may reach.
//!
//! The messages travel as the same length-prefixed postcard frames as the rest
//! of the protocol, on the one bi-directional stream the handshake opened.
//!
//! **JSON inside postcard.** `serde_json::Value` is self-describing: it
//! deserializes through `deserialize_any`, which postcard cannot do. So every
//! `Value` on the wire travels as the text of its JSON (`json_text` below) — the
//! type in Rust, and the shape of the messages, stay exactly what the protocol
//! says; only the bytes of that one field are a string. Under a human-readable
//! format such as `serde_json` itself the field is the value, unwrapped.
//!
//! **The shape of every allowlisted command and event is a public contract.**
//! `contract/` holds an example of each argument object, each reply and each
//! event payload, and `src-tauri/src/remote/contract.rs` compares the desktop's
//! real values with them. See the repository's `.claude/rules/remote.md`.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

/// A phone's request. Frames are read from the phone with a size limit of the
/// desktop's choosing, and every field is checked before it is acted on.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ClientMsg {
    /// Call a command. `id` is the phone's own counter: the reply carries it back,
    /// and replies may arrive in any order, because a call can take seconds.
    Invoke {
        id: u64,
        cmd: String,
        #[serde(with = "json_text")]
        args: Value,
    },
    /// Start receiving an event. Listening twice is the same as once.
    Listen { event: String },
    Unlisten { event: String },
}

/// The desktop's message.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ServerMsg {
    Reply {
        id: u64,
        #[serde(with = "json_result")]
        result: Result<Value, BridgeError>,
    },
    Event {
        name: String,
        #[serde(with = "json_text")]
        payload: Value,
    },
}

/// Why a call did not return a value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum BridgeError {
    /// The command exists on the desktop and is not on the allowlist (or its
    /// arguments ask for something a phone may not do). The handler did not run.
    Forbidden,
    /// No command of that name.
    UnknownCommand,
    /// The handler ran and failed. The value is what the desktop's own front end
    /// receives as the rejection of `invoke`, in the same form.
    Handler(#[serde(with = "json_text")] Value),
    /// The bridge itself could not do it: a reply too large for a frame, the
    /// handlers not being ready, a worker gone.
    Internal(String),
}

impl std::fmt::Display for BridgeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BridgeError::Forbidden => write!(f, "forbidden"),
            BridgeError::UnknownCommand => write!(f, "unknown command"),
            BridgeError::Handler(value) => write!(f, "{value}"),
            BridgeError::Internal(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for BridgeError {}

/// The commands a phone may call, in the first version. Everything else on the
/// desktop answers `Forbidden`. Adding a name here is a minor change of the
/// protocol and needs its `contract/commands/<name>.json`; removing or renaming
/// one is a major change.
///
/// There is deliberately no `terminal_write`, `terminal_resize`, `terminal_shell`
/// or `terminal_create`: a terminal is read-only on a phone. `session_send` is
/// allowlisted, but its `attachments` must be empty (the desktop refuses a
/// phone's paths — see `remote::bridge`).
pub const ALLOWED_COMMANDS: &[&str] = &[
    "tracker_health",
    "tracker_snapshot",
    "tracker_set_project",
    "settings_load",
    "agents_catalog",
    "agents_installed",
    "project_config",
    "target_branches",
    "run_state",
    "run_reports",
    "run_start",
    "run_stop",
    "session_start",
    "session_attach",
    "session_since",
    "session_send",
    "session_answer",
    "session_stop",
    "crew_tree",
    "crew_attach",
    "crew_send",
    "crew_stop",
    "terminal_list",
    "terminal_attach",
];

/// The events a phone may listen to. Every name contains a colon and no command
/// name does, which is how `capabilities` can carry both in one list.
pub const ALLOWED_EVENTS: &[&str] = &[
    "tracker:delta",
    "tracker:health",
    "run:state",
    "session:events",
    "crew:events",
    "terminal:state",
    "terminal:removed",
];

pub fn is_allowed_command(cmd: &str) -> bool {
    ALLOWED_COMMANDS.contains(&cmd)
}

pub fn is_allowed_event(event: &str) -> bool {
    ALLOWED_EVENTS.contains(&event)
}

/// What a `HelloReply` advertises: every allowed command, then every allowed
/// event. The phone hides whatever it does not find here.
pub fn capabilities() -> Vec<String> {
    ALLOWED_COMMANDS.iter().chain(ALLOWED_EVENTS).map(|name| (*name).to_owned()).collect()
}

/// Where a command's contract example lives, relative to `contract/`.
pub fn command_contract_path(cmd: &str) -> String {
    format!("commands/{cmd}.json")
}

/// Where an event's contract example lives, relative to `contract/`. The colon
/// becomes a dot: it is not a character every file system accepts.
pub fn event_contract_path(event: &str) -> String {
    format!("events/{}.json", event.replace(':', "."))
}

/// A `Value` as the text of its JSON when the format is not human-readable.
mod json_text {
    use super::*;

    pub fn serialize<S: Serializer>(value: &Value, serializer: S) -> Result<S::Ok, S::Error> {
        if serializer.is_human_readable() {
            value.serialize(serializer)
        } else {
            let text = serde_json::to_string(value).map_err(serde::ser::Error::custom)?;
            serializer.serialize_str(&text)
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Value, D::Error> {
        if deserializer.is_human_readable() {
            Value::deserialize(deserializer)
        } else {
            let text = String::deserialize(deserializer)?;
            serde_json::from_str(&text).map_err(serde::de::Error::custom)
        }
    }
}

/// `Result<Value, BridgeError>` as an enum of its own, so that the `Value` in
/// the `Ok` arm takes the same road as every other.
mod json_result {
    use super::*;

    #[derive(Serialize)]
    enum WireRef<'a> {
        Ok(#[serde(with = "json_text")] &'a Value),
        Err(&'a BridgeError),
    }

    #[derive(Deserialize)]
    enum Wire {
        Ok(#[serde(with = "json_text")] Value),
        Err(BridgeError),
    }

    pub fn serialize<S: Serializer>(
        result: &Result<Value, BridgeError>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        match result {
            Ok(value) => WireRef::Ok(value).serialize(serializer),
            Err(err) => WireRef::Err(err).serialize(serializer),
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Result<Value, BridgeError>, D::Error> {
        Ok(match Wire::deserialize(deserializer)? {
            Wire::Ok(value) => Ok(value),
            Wire::Err(err) => Err(err),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{decode_frame, encode_frame};
    use serde_json::json;

    fn round_trip<T: Serialize + serde::de::DeserializeOwned + PartialEq + std::fmt::Debug>(value: T) {
        let frame = encode_frame(&value).unwrap();
        let (decoded, rest): (T, _) = decode_frame(&frame).unwrap();
        assert_eq!(decoded, value);
        assert!(rest.is_empty());
    }

    #[test]
    fn json_values_survive_a_postcard_frame() {
        round_trip(ClientMsg::Invoke {
            id: 7,
            cmd: "run_stop".into(),
            args: json!({ "token": 3, "nested": [1, null, { "a": "b" }], "f": 1.5 }),
        });
        round_trip(ClientMsg::Listen { event: "run:state".into() });
        round_trip(ClientMsg::Unlisten { event: "run:state".into() });
        round_trip(ServerMsg::Event { name: "tracker:delta".into(), payload: json!({ "generation": 2 }) });
        round_trip(ServerMsg::Reply { id: 1, result: Ok(json!(null)) });
        round_trip(ServerMsg::Reply { id: 2, result: Ok(json!({ "a": [true] })) });
        for err in [
            BridgeError::Forbidden,
            BridgeError::UnknownCommand,
            BridgeError::Handler(json!({ "kind": "noSuchSession", "message": 4 })),
            BridgeError::Handler(json!("a plain string")),
            BridgeError::Internal("no".into()),
        ] {
            round_trip(ServerMsg::Reply { id: 3, result: Err(err) });
        }
    }

    #[test]
    fn under_json_a_value_is_the_value_and_not_its_text() {
        let msg = ClientMsg::Invoke { id: 1, cmd: "x".into(), args: json!({ "a": 1 }) };
        let text = serde_json::to_value(&msg).unwrap();
        assert_eq!(text, json!({ "Invoke": { "id": 1, "cmd": "x", "args": { "a": 1 } } }));
    }

    #[test]
    fn a_frame_whose_json_text_is_not_json_is_an_error_and_not_a_panic() {
        // Variant 0 (Invoke), id 1, cmd "x", args "{" — a string that is not JSON.
        let body = [0u8, 1, 1, b'x', 1, b'{'];
        let mut frame = (body.len() as u32).to_be_bytes().to_vec();
        frame.extend_from_slice(&body);
        assert!(decode_frame::<ClientMsg>(&frame).is_err());
    }

    #[test]
    fn the_allowlist_has_no_duplicates_and_no_writes_to_a_terminal() {
        for list in [ALLOWED_COMMANDS, ALLOWED_EVENTS] {
            let mut seen = std::collections::HashSet::new();
            for name in list {
                assert!(seen.insert(*name), "{name} is listed twice");
            }
        }
        for name in ["terminal_write", "terminal_resize", "terminal_shell", "terminal_create"] {
            assert!(!is_allowed_command(name), "{name} must stay off the phone");
        }
        assert!(ALLOWED_COMMANDS.iter().all(|name| !name.contains(':')));
        assert!(ALLOWED_EVENTS.iter().all(|name| name.contains(':')));
    }

    #[test]
    fn capabilities_are_the_commands_then_the_events() {
        let caps = capabilities();
        assert_eq!(caps.len(), ALLOWED_COMMANDS.len() + ALLOWED_EVENTS.len());
        assert_eq!(caps.first().map(String::as_str), Some("tracker_health"));
        assert_eq!(caps.last().map(String::as_str), Some("terminal:removed"));
    }

    /// Every allowlisted name has its contract file, and every file there is
    /// for an allowlisted name: adding to the list without pinning a shape, or
    /// leaving a shape behind a removed command, fails here.
    #[test]
    fn the_contract_directory_matches_the_allowlist_exactly() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("contract");
        let mut expected: Vec<String> = ALLOWED_COMMANDS.iter().map(|c| command_contract_path(c)).collect();
        expected.extend(ALLOWED_EVENTS.iter().map(|e| event_contract_path(e)));
        let mut found = Vec::new();
        for dir in ["commands", "events"] {
            for entry in std::fs::read_dir(root.join(dir)).unwrap() {
                found.push(format!("{dir}/{}", entry.unwrap().file_name().to_string_lossy()));
            }
        }
        expected.sort();
        found.sort();
        assert_eq!(found, expected);
        for path in &expected {
            let text = std::fs::read_to_string(root.join(path)).unwrap();
            serde_json::from_str::<Value>(&text).unwrap_or_else(|err| panic!("{path} is not JSON: {err}"));
        }
    }
}
