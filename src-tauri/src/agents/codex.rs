//! Codex.
//!
//! Codex has a skills system of its own — `~/.codex/skills/`, `skills.config`,
//! `skills.bundled.enabled` — and `-c key=value` overrides any config value on
//! the command line. What it has no way to do is name an extra skills
//! directory: `SkillsConfig` carries no filesystem roots, and the only
//! mechanism that does, `skills/extraRoots/set`, belongs to the app-server, a
//! different process from the TUI this app runs in a PTY. Writing into
//! `~/.codex/skills/` or repointing `CODEX_HOME` would reach into the person's
//! own setup, so neither is done. The skill text rides in the prompt instead.
//!
//! Layer B is not implemented for Codex: reading its permission dialog off the
//! screen is tracked as smetana-603. Until then layer A — a bell, or three
//! seconds of silence — says that somebody is waiting, without the question.

use portable_pty::CommandBuilder;

use super::library::read_skill;
use super::{prompt, Brainstorm, Intent, Launch, Profile, SkillDelivery};

pub struct Codex;

impl Profile for Codex {
    fn id(&self) -> &'static str {
        "codex"
    }

    fn binary(&self) -> &'static str {
        "codex"
    }

    fn delivery(&self) -> SkillDelivery {
        SkillDelivery::Inline
    }

    fn command(&self, launch: &Launch) -> CommandBuilder {
        let mut cmd = CommandBuilder::new(self.binary());
        let filing_a_task = matches!(launch.intent, Intent::NewTask { .. });
        // Only the mode that actually uses the whole process pays for reading
        // it: `Auto` is handed the path and decides for itself.
        let discussing = matches!(
            launch.intent,
            Intent::NewTask { brainstorm: Brainstorm::On, .. }
        );
        let filing =
            filing_a_task.then(|| read_skill(&launch.skills.smetana, "filing-a-task")).flatten();
        let brainstorming_text =
            discussing.then(|| read_skill(&launch.skills.superpowers, "brainstorming")).flatten();
        let brainstorming = launch.skills.superpowers.join("skills/brainstorming");
        let text = prompt::SkillText {
            filing: filing.as_deref(),
            brainstorming: brainstorming_text.as_deref(),
        };
        if let Some(built) = prompt::build(&launch.intent, self.delivery(), &brainstorming, text) {
            cmd.arg(built);
        }
        cmd
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::library::Skills;
    use crate::agents::{Brainstorm, Intent, Launch, TaskDraft};
    use std::path::PathBuf;

    /// The real bundle resources, the way `claude.rs` reaches its own screen
    /// fixtures. A made-up path would leave `read_skill` answering `None` in
    /// every test here, and the gating in `command` — which decides that filing
    /// guidance reaches an `Inline` harness whatever the Brainstorming switch
    /// says — would never be observed at all.
    fn resources(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources").join(name)
    }

    fn launch(intent: Intent) -> Launch {
        Launch {
            profile: &Codex,
            cwd: PathBuf::from("/tmp/project"),
            intent,
            skills: Skills {
                smetana: resources("smetana"),
                superpowers: resources("superpowers"),
                superpowers_installed: false,
            },
        }
    }

    /// A line from the middle of each shipped `SKILL.md`, far enough in to be
    /// the body rather than the front matter: finding it in the prompt is the
    /// only proof that the file was read and pasted, not merely named.
    const FILING_BODY: &str = "The title says what needs doing";
    const BRAINSTORMING_BODY: &str = "ask questions one at a time";

    fn argv(launch: &Launch) -> Vec<String> {
        Codex
            .command(launch)
            .get_argv()
            .iter()
            .map(|s| s.to_string_lossy().into_owned())
            .collect()
    }

    fn new_task(brainstorm: Brainstorm) -> Intent {
        Intent::NewTask {
            brainstorm,
            draft: TaskDraft {
                title: "Swap the red for green".into(),
                issue_type: "bug".into(),
                priority: 2,
                description: None,
            },
        }
    }

    #[test]
    fn nothing_but_the_binary_and_the_prompt() {
        // Codex has no per-session flag for a skill library — verified against
        // 0.146.0 — so anything else on this command line would be a mistake.
        let args = argv(&launch(new_task(Brainstorm::Off)));
        assert_eq!(args.len(), 2);
        assert_eq!(args[0], "codex");
    }

    #[test]
    fn a_bare_session_is_just_the_binary() {
        assert_eq!(argv(&launch(Intent::Bare)), vec!["codex".to_string()]);
    }

    #[test]
    fn it_never_names_a_skill_registry_it_does_not_have() {
        let args = argv(&launch(new_task(Brainstorm::On)));
        assert!(!args.last().unwrap().contains("superpowers:"));
    }

    #[test]
    fn auto_points_at_the_file_instead_of_pasting_it() {
        let args = argv(&launch(new_task(Brainstorm::Auto)));
        let pointer = resources("superpowers").join("skills/brainstorming/SKILL.md");
        assert!(args.last().unwrap().contains(&pointer.display().to_string()));
    }

    #[test]
    fn the_filing_skill_is_pasted_in_whatever_the_switch_says() {
        // The branch's headline behaviour for a harness with no skill registry:
        // how this project wants a task worded is not part of the brainstorming
        // question, so the text travels in all three positions of the switch.
        // Reading it under `Brainstorm::On` alone would still satisfy every
        // other test in this file.
        for mode in [Brainstorm::Off, Brainstorm::Auto, Brainstorm::On] {
            let args = argv(&launch(new_task(mode)));
            assert!(
                args.last().unwrap().contains(FILING_BODY),
                "{mode:?}: the shipped filing skill never reached the prompt"
            );
        }
    }

    #[test]
    fn the_brainstorming_process_is_pasted_in_only_when_it_is_switched_on() {
        // 10 KB the agent may never use: `Off` has no business with it at all,
        // and `Auto` is handed the path instead so it pays only if it decides
        // the task warrants a conversation.
        for mode in [Brainstorm::Off, Brainstorm::Auto] {
            let args = argv(&launch(new_task(mode)));
            assert!(
                !args.last().unwrap().contains(BRAINSTORMING_BODY),
                "{mode:?}: the whole process was pasted in unasked"
            );
        }
        let args = argv(&launch(new_task(Brainstorm::On)));
        assert!(
            args.last().unwrap().contains(BRAINSTORMING_BODY),
            "on must carry the process itself, there being no registry to name it in"
        );
    }

    #[test]
    fn it_cannot_read_its_own_dialog_yet() {
        // Layer A still reports that somebody is waiting. smetana-603.
        assert!(Codex.question(&["│ Allow this command? │".to_string()]).is_none());
    }
}
