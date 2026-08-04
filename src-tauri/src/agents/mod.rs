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
#[derive(Clone, Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskDraft {
    pub title: String,
    pub issue_type: String,
    pub priority: u8,
    pub description: Option<String>,
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

    #[test]
    fn a_binary_is_found_by_walking_the_path() {
        // /bin/sh exists on every platform this app builds for.
        assert!(on_path("sh", Some("/nowhere:/bin")));
        assert!(!on_path("sh", Some("/nowhere")));
        assert!(!on_path("sh", None));
    }

    #[test]
    fn picking_falls_back_to_whatever_is_installed() {
        // "sh" is not an agent, so nothing is installed as far as pick is
        // concerned, and there is nothing to fall back to either.
        assert!(pick("claude", Some("/nowhere")).is_none());
        assert!(pick("nonsense", Some("/nowhere")).is_none());
    }
}
