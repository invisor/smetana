//! What the app knows about the CLI agents it runs. One file per agent, and
//! everything harness-specific lives in it: how to spawn it, whether it can be
//! handed a skill library and how, and how to tell that it is waiting on a
//! person.
//!
//! The split that makes this worth a module: an *intent* — file a task, edit
//! one, just start — is the same for every agent and is where the product
//! decision lives. *Delivery* is not. Claude Code takes a directory on the
//! command line; Codex has no per-session mechanism at all, so its skills ride
//! in the prompt. Neither harness gets to leak into the code that decides what
//! we want done.

pub mod claude;
pub mod codex;
pub mod library;
pub mod prompt;

use std::path::PathBuf;

use portable_pty::CommandBuilder;

use crate::terminal::model::Question;

/// How a harness accepts a skill library.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SkillDelivery {
    /// A directory named on the command line; the agent reads a skill only
    /// when it invokes it, so the cost until then is one line in an index.
    PluginDir,
    /// The text of the skill, carried in the prompt. Works anywhere, because
    /// a positional prompt is the one thing every harness has.
    Inline,
}

/// Whether the agent must talk the task through before filing it. `Auto`
/// leaves the judgement to the agent on purpose: nothing in the app has read
/// the text, and a heuristic on title length would misfire in both directions.
#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Brainstorm {
    Auto,
    On,
    Off,
}

/// What the new-task dialog collected. Not an issue: nothing here is written
/// to bd by this app any more — the agent files it.
///
/// No `rename_all = "camelCase"` here, unlike `Intent` below: `issue_type` is
/// bd's own field name, spelled the same way in the modal, in the tracker's
/// `Issue`/`IssuePatch` (`tracker/model.rs`) and by bd itself, and snake_case
/// is the convention for it throughout this codebase. Renaming it here would
/// only have broken the one place it needs to match.
#[derive(Clone, Debug, serde::Deserialize)]
pub struct TaskDraft {
    /// What the person wrote, in one piece. bd wants a title as well, and
    /// writing one is the agent's job: it has read this text and the app has
    /// not, and the filing skill is where the wording rules live.
    pub text: String,
    /// `None` is the dialog's Auto: the agent decides from the text. Auto
    /// arrives as absence rather than as a word, so a value that reaches here
    /// at all is one bd knows.
    pub issue_type: Option<String>,
    pub priority: Option<u8>,
}

/// Why a session is being started. The front end sends this; every profile
/// turns the same value into its own command line.
#[derive(Clone, Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum Intent {
    /// The "+ New agent" row: a person with their own reason, and nothing to
    /// impose on them.
    Bare,
    NewTask {
        brainstorm: Brainstorm,
        draft: TaskDraft,
    },
    EditTask {
        id: String,
        title: String,
    },
}

/// Everything a spawn needs before any agent has looked at it.
pub struct Launch {
    pub profile: &'static dyn Profile,
    pub cwd: PathBuf,
    pub intent: Intent,
    pub skills: library::Skills,
}

pub trait Profile: Sync {
    fn id(&self) -> &'static str;
    /// What to exec. Also what we look for on `PATH`.
    fn binary(&self) -> &'static str;
    fn delivery(&self) -> SkillDelivery;
    /// The whole command line, prompt included. `cwd` and the environment are
    /// added by `terminal::pty::build_command`, which owns those for every agent.
    fn command(&self, launch: &Launch) -> CommandBuilder;
    /// Layer B of detection: this agent's own question, read off the screen.
    /// The default is "this profile cannot read its agent's dialog", which is
    /// an ordinary state — layer A still says that somebody is waiting.
    fn question(&self, _screen: &[String]) -> Option<Question> {
        None
    }
}

/// The closed list of agent ids, and the only copy of it. `settings/model.rs`
/// validates against this rather than repeating it: the side-tab set is
/// already written out twice in this codebase and the cost is recorded in
/// CLAUDE.md — a value that survives the session and silently comes back as
/// something else.
pub const IDS: [&str; 2] = ["claude", "codex"];

pub fn resolve(id: &str) -> Option<&'static dyn Profile> {
    match id {
        "claude" => Some(&claude::Claude),
        "codex" => Some(&codex::Codex),
        _ => None,
    }
}

/// Is this binary reachable? Pure in the argument that matters, so it can be
/// tested without a fixture directory: `path_var` is `PATH` as the process
/// sees it.
pub fn on_path(binary: &str, path_var: Option<&str>) -> bool {
    let Some(paths) = path_var else { return false };
    std::env::split_paths(paths)
        .filter(|dir| !dir.as_os_str().is_empty())
        .any(|dir| dir.join(binary).is_file())
}

/// The profile to actually run: the configured one when it is installed,
/// otherwise the first one that is. Returning something other than what was
/// asked for is not silent — `Session.agent` carries the name of whatever ran,
/// so the row in the panel says which agent this is.
pub fn pick(id: &str, path_var: Option<&str>) -> Option<&'static dyn Profile> {
    let installed = |p: &'static dyn Profile| on_path(p.binary(), path_var);
    if let Some(profile) = resolve(id).filter(|p| installed(*p)) {
        return Some(profile);
    }
    IDS.iter().filter_map(|id| resolve(id)).find(|p| installed(*p))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_id_resolves_to_a_profile_that_agrees_about_its_own_name() {
        for id in IDS {
            let profile = resolve(id).expect("a listed id must resolve");
            assert_eq!(profile.id(), id);
        }
    }

    #[test]
    fn an_unknown_id_resolves_to_nothing() {
        assert!(resolve("cursor").is_none());
    }

    #[test]
    fn the_two_harnesses_take_skills_differently() {
        assert_eq!(resolve("claude").unwrap().delivery(), SkillDelivery::PluginDir);
        assert_eq!(resolve("codex").unwrap().delivery(), SkillDelivery::Inline);
    }

    // `:` is a path separator on Unix and an ordinary character on Windows
    // (where `std::env::split_paths` splits on `;`), so this literal only
    // proves the fact it claims to on Unix; /bin/sh is itself a Unix fact.
    #[cfg(unix)]
    #[test]
    fn a_binary_is_found_by_walking_the_path() {
        // /bin/sh exists on every platform this app builds for.
        assert!(on_path("sh", Some("/nowhere:/bin")));
        assert!(!on_path("sh", Some("/nowhere")));
        assert!(!on_path("sh", None));
    }

    // The JSON below is written by hand, not built from an `Intent` and
    // serialized back: a round trip through `Serialize` would only agree with
    // itself and prove nothing about the wire format the front end actually
    // sends. Each string is copied from what `createSession` in
    // `src/stores/terminals.js` hands to `invoke('terminal_create', ...)` for
    // the intent literals built in `src/views/DesktopApp.vue` (`newAgent`,
    // `submitNewTask`, `askAgentToEdit`) — this is the one place in either
    // suite that crosses the IPC boundary instead of mocking it away.

    #[test]
    fn a_bare_intent_deserializes_from_the_front_ends_json() {
        let intent: Intent = serde_json::from_str(r#"{"kind":"bare"}"#).expect("deserializes");
        assert!(matches!(intent, Intent::Bare));
    }

    #[test]
    fn a_new_task_intent_deserializes_from_the_front_ends_json() {
        // All three positions of the switch, because all three are literals the
        // modal writes and nothing but a test reads back: a rename on either
        // side of the boundary would otherwise surface as a session that simply
        // refuses to start, with the switch position as the only clue.
        for (literal, expected) in
            [("auto", Brainstorm::Auto), ("on", Brainstorm::On), ("off", Brainstorm::Off)]
        {
            let json = format!(
                r#"{{
                    "kind": "newTask",
                    "brainstorm": "{literal}",
                    "draft": {{
                        "text": "Fix the thing",
                        "issue_type": "bug",
                        "priority": 2
                    }}
                }}"#
            );
            let intent: Intent = serde_json::from_str(&json).expect("deserializes");
            match intent {
                Intent::NewTask { brainstorm, draft } => {
                    assert_eq!(brainstorm, expected, "{literal}");
                    assert_eq!(draft.text, "Fix the thing");
                    assert_eq!(draft.issue_type.as_deref(), Some("bug"));
                    assert_eq!(draft.priority, Some(2));
                }
                other => panic!("expected NewTask, got {other:?}"),
            }
        }
    }

    #[test]
    fn auto_arrives_as_null_from_the_front_ends_json() {
        // The dialog's Auto positions. `null` rather than a missing key,
        // because that is literally what `NewTaskModal.vue` sends — and if it
        // ever sends the word "auto" instead, this is where that shows up as a
        // session refusing to start rather than as a type bd would reject.
        let json = r#"{
            "kind": "newTask",
            "brainstorm": "auto",
            "draft": { "text": "Fix the thing", "issue_type": null, "priority": null }
        }"#;
        let intent: Intent = serde_json::from_str(json).expect("deserializes");
        match intent {
            Intent::NewTask { draft, .. } => {
                assert!(draft.issue_type.is_none());
                assert!(draft.priority.is_none());
            }
            other => panic!("expected NewTask, got {other:?}"),
        }
    }

    #[test]
    fn an_edit_task_intent_deserializes_from_the_front_ends_json() {
        let json = r#"{"kind":"editTask","id":"bd-1","title":"Some title"}"#;
        let intent: Intent = serde_json::from_str(json).expect("deserializes");
        match intent {
            Intent::EditTask { id, title } => {
                assert_eq!(id, "bd-1");
                assert_eq!(title, "Some title");
            }
            other => panic!("expected EditTask, got {other:?}"),
        }
    }

    #[test]
    fn picking_falls_back_to_whatever_is_installed() {
        // "sh" is not an agent, so nothing is installed as far as pick is
        // concerned, and there is nothing to fall back to either.
        assert!(pick("claude", Some("/nowhere")).is_none());
        assert!(pick("nonsense", Some("/nowhere")).is_none());

        // Claude is absent and codex is present in this directory, so only
        // the fallback branch of `pick` can produce the codex profile here.
        let dir = std::env::temp_dir().join(format!(
            "smetana-agents-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock is after the Unix epoch")
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).expect("create temp dir for the fake install");
        std::fs::File::create(dir.join("codex")).expect("create fake codex binary");

        let path_var = dir.to_str().expect("temp dir path is valid UTF-8");
        assert_eq!(pick("claude", Some(path_var)).map(|p| p.id()), Some("codex"));

        std::fs::remove_dir_all(&dir).expect("remove temp dir for the fake install");
    }
}
