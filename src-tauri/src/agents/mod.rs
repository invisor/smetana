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

/// How a harness is handed the images attached to a task.
///
/// The same split as `SkillDelivery`, for the same reason and with the same
/// division of labour: *that* the agent has images and what it owes us for them
/// is the product's decision and is written once, in `prompt.rs`; *how the
/// pixels reach this particular CLI* is the harness's business and lives in its
/// own file. Codex takes `-i/--image`; Claude Code has no such flag and reads
/// an image when the prompt names its path.
///
/// The paths are named in the prompt either way — the agent has to copy them
/// into the issue description, and the description is what an implementer opens
/// the picture by. The delivery only decides what is said about them.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ImageDelivery {
    /// One flag per file on the command line, in front of the prompt.
    Flag(&'static str),
    /// Nothing on the command line: the harness opens a path it is told about.
    InPrompt,
}

/// One position of one stage of the work a filing session does before the
/// task exists: talking it through, writing down the design that discussion
/// produced, writing the implementation plan. All three switches in the
/// new-task dialog offer these same three, and `Auto` means the same thing in
/// each — the agent's judgement, because nothing in the app has read the text,
/// and a heuristic on title length would misfire in both directions.
///
/// One type for all three deliberately, matching `STAGES` on the front end,
/// which is likewise one list for the three dropdowns: while the discussion
/// had a copy of this enum to itself, a fourth position added to one of them
/// compiled perfectly and left the other two a position short.
#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Stage {
    Auto,
    On,
    Off,
}

/// The cascade the dialog draws (`src/components/kanban/taskStages.js`),
/// applied again on this side of the wire, and it is not a duplicate to be
/// tidied away: what arrives here is a payload, and a payload can carry a spec
/// that was chosen under a Brainstorming that has since been turned off. A
/// stage under a parent that is not `On` is settled by that parent — there is
/// nothing for a design document to record when no discussion happened, and
/// nothing for a plan to plan when no design was written.
///
/// Returns the spec and the plan as they will actually be carried out, which
/// is what every reader downstream — the prose in `prompt.rs`, the skill Codex
/// reads off disk — has to work from.
pub fn cascade(brainstorm: Stage, spec: Stage, plan: Stage) -> (Stage, Stage) {
    let under = |parent: Stage, chosen: Stage| if parent == Stage::On { chosen } else { parent };
    let spec = under(brainstorm, spec);
    (spec, under(spec, plan))
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
    /// Absolute paths of the images attached in the dialog, already copied into
    /// the app's own data directory by `attachments.rs`. `default` because a
    /// dialog that attached nothing sends nothing, and because a payload
    /// written before this field existed must still start a session rather than
    /// fail to deserialize.
    #[serde(default)]
    pub images: Vec<String>,
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
        brainstorm: Stage,
        /// Whether the design the discussion produced is written to a file.
        /// Meaningful only under `Stage::On`, and `cascade` is what says
        /// so — never read either of these two raw.
        spec: Stage,
        /// Whether an implementation plan is written. Meaningful only under a
        /// spec that is itself `On`.
        plan: Stage,
        draft: TaskDraft,
    },
    EditTask {
        id: String,
        title: String,
    },
    /// Work out what this project is made of and write
    /// `.smetana/project.toml`. Started from the dialog a person gets when
    /// they add a project, and from the project row afterwards.
    Setup,
    /// One batch of a run. Started by `runs::service`, never by a person
    /// directly — which is why it carries the whole of what the run was asked
    /// to do rather than a reference to it: the session may outlive a settings
    /// change, and a batch that quietly retargets halfway is worse than one
    /// that is wrong from the start and says so.
    Run {
        settings: crate::runs::model::RunSettings,
    },
}

impl Intent {
    /// What this reduces to for the two panels that draw a session. It lives
    /// here rather than in `terminal::model` because it is knowledge about
    /// `Intent` — which of its payload is drawn and which is only a briefing
    /// for the agent — and the answer moves whenever a variant does.
    ///
    /// A draft's three fields come along and its `images` do not: the right
    /// panel draws the prose, the type and the priority, and the paths of the
    /// attachments are for the agent to open and to copy into the issue. So
    /// are `brainstorm`, `spec` and `plan`: they are instructions about how to
    /// work rather than anything about the task, and nothing on screen would
    /// draw them.
    pub fn work(&self) -> crate::terminal::model::SessionWork {
        use crate::terminal::model::SessionWork as W;
        match self {
            Intent::Bare => W::Bare,
            Intent::NewTask { draft, .. } => W::NewTask {
                text: draft.text.clone(),
                issue_type: draft.issue_type.clone(),
                priority: draft.priority,
            },
            Intent::EditTask { id, .. } => W::EditTask { id: id.clone() },
            Intent::Setup => W::Setup,
            Intent::Run { .. } => W::Run,
        }
    }
}

/// Everything a spawn needs before any agent has looked at it.
pub struct Launch {
    pub profile: &'static dyn Profile,
    pub cwd: PathBuf,
    pub intent: Intent,
    pub skills: library::Skills,
    /// What a survey of the project found, already rendered. Only a `Setup`
    /// intent has any, and it is read by the caller for the same reason skill
    /// text is: `prompt.rs` stays pure and the disk stays outside it.
    pub facts: Option<String>,
}

pub trait Profile: Sync {
    fn id(&self) -> &'static str;
    /// What to exec. Also what we look for on `PATH`.
    fn binary(&self) -> &'static str;
    fn delivery(&self) -> SkillDelivery;
    /// How images reach this harness. The default is the answer for any CLI
    /// that has no flag for them, which is most of them: a path named in the
    /// prompt is the one channel every harness has.
    fn images(&self) -> ImageDelivery {
        ImageDelivery::InPrompt
    }
    /// The whole command line, prompt included. `cwd` and the environment are
    /// added by `terminal::pty::build_command`, which owns those for every agent.
    fn command(&self, launch: &Launch) -> CommandBuilder;
    /// Layer B of detection: this agent's own question, read off the screen.
    /// The default is "this profile cannot read its agent's dialog", which is
    /// an ordinary state — layer A still says that somebody is waiting.
    fn question(&self, _screen: &[String]) -> Option<Question> {
        None
    }

    /// How this harness is asked what is left of the subscription's allowance,
    /// as arguments after `binary()`, and how its answer reads. A pair: the
    /// command is worth nothing without something able to read what it prints,
    /// and a profile that answers one and not the other simply reads as
    /// unaskable, which `runs::usage::decide` treats as no reason to hold a run
    /// up.
    ///
    /// The default is that pair of absences, and it is a working answer rather
    /// than a gap — the same shape `question` keeps. A harness with no way to
    /// report its allowance runs at full size and finds out by failing, which
    /// is where every harness was before this existed.
    fn usage_command(&self) -> Option<&'static [&'static str]> {
        None
    }

    fn parse_usage(&self, _output: &str) -> Option<crate::runs::usage::Usage> {
        None
    }

    /// Extra arguments for working without a person, and the environment that
    /// goes with them.
    ///
    /// The default is nothing, and that is a working answer rather than a gap:
    /// a harness with no such switch stops at its first permission prompt, the
    /// session turns `needs-you`, and the run waits — which is exactly what
    /// `Supervised` is. A harness that cannot be autonomous is a fact about
    /// that harness, and the app says so by behaving like the supervised mode
    /// instead of pretending.
    fn autonomy(&self, _mode: crate::runs::model::RunMode) -> Autonomy {
        Autonomy::default()
    }
}

/// What a profile needs added to run a batch.
#[derive(Default, Debug, PartialEq, Eq)]
pub struct Autonomy {
    pub args: Vec<&'static str>,
    pub env: Vec<(&'static str, &'static str)>,
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
            [("auto", Stage::Auto), ("on", Stage::On), ("off", Stage::Off)]
        {
            let json = format!(
                r#"{{
                    "kind": "newTask",
                    "brainstorm": "{literal}",
                    "spec": "on",
                    "plan": "off",
                    "draft": {{
                        "text": "Fix the thing",
                        "issue_type": "bug",
                        "priority": 2,
                        "images": ["/data/attachments/20260806-121314-mock.png"]
                    }}
                }}"#
            );
            let intent: Intent = serde_json::from_str(&json).expect("deserializes");
            match intent {
                Intent::NewTask { brainstorm, spec, plan, draft } => {
                    assert_eq!(brainstorm, expected, "{literal}");
                    assert_eq!(spec, Stage::On, "{literal}");
                    assert_eq!(plan, Stage::Off, "{literal}");
                    assert_eq!(draft.text, "Fix the thing");
                    assert_eq!(draft.issue_type.as_deref(), Some("bug"));
                    assert_eq!(draft.priority, Some(2));
                    assert_eq!(draft.images, vec!["/data/attachments/20260806-121314-mock.png"]);
                }
                other => panic!("expected NewTask, got {other:?}"),
            }
        }
    }

    #[test]
    fn a_draft_with_nothing_attached_carries_no_images_and_still_deserializes() {
        // The dialog sends the key only when something is attached, and a
        // payload written before the field existed has none either. A session
        // that refused to start over an absent key would take the whole
        // new-task flow with it.
        let json = r#"{
            "kind": "newTask",
            "brainstorm": "off",
            "spec": "off",
            "plan": "off",
            "draft": { "text": "Fix the thing", "issue_type": null, "priority": null }
        }"#;
        let intent: Intent = serde_json::from_str(json).expect("deserializes");
        match intent {
            Intent::NewTask { draft, .. } => assert!(draft.images.is_empty()),
            other => panic!("expected NewTask, got {other:?}"),
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
            "spec": "auto",
            "plan": "auto",
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
    fn a_setup_intent_deserializes_from_the_front_ends_json() {
        let intent: Intent = serde_json::from_str(r#"{"kind":"setup"}"#).expect("deserializes");
        assert!(matches!(intent, Intent::Setup));
    }

    #[test]
    fn an_intent_reduces_to_the_work_the_panel_names_it_by() {
        use crate::terminal::model::SessionWork as W;
        assert_eq!(Intent::Bare.work(), W::Bare);
        assert_eq!(Intent::Setup.work(), W::Setup);
        assert_eq!(
            Intent::EditTask { id: "smetana-42".into(), title: "Some title".into() }.work(),
            W::EditTask { id: "smetana-42".into() },
            "the id is kept and the title is not — the row draws an identifier"
        );
    }

    #[test]
    fn a_filing_intent_carries_its_draft_across_and_leaves_the_briefing_behind() {
        use crate::terminal::model::SessionWork as W;
        let intent = Intent::NewTask {
            brainstorm: Stage::On,
            spec: Stage::On,
            plan: Stage::On,
            draft: TaskDraft {
                text: "The log drops lines above 10k".into(),
                issue_type: Some("bug".into()),
                priority: Some(1),
                images: vec!["/data/attachments/20260806-121314-mock.png".into()],
            },
        };
        // The prose, the type and the priority are what the right panel draws
        // back. The images and the three stage switches are the agent's
        // briefing and stop here — nothing on screen would show them.
        assert_eq!(
            intent.work(),
            W::NewTask {
                text: "The log drops lines above 10k".into(),
                issue_type: Some("bug".into()),
                priority: Some(1),
            }
        );
    }

    #[test]
    fn a_filing_intent_left_on_auto_carries_the_absence_rather_than_a_value() {
        use crate::terminal::model::SessionWork as W;
        let intent = Intent::NewTask {
            brainstorm: Stage::Auto,
            spec: Stage::Auto,
            plan: Stage::Auto,
            draft: TaskDraft {
                text: "Something".into(),
                issue_type: None,
                priority: None,
                images: vec![],
            },
        };
        assert_eq!(
            intent.work(),
            W::NewTask { text: "Something".into(), issue_type: None, priority: None }
        );
    }

    #[test]
    fn a_stage_is_only_the_persons_to_choose_under_an_on_parent() {
        // The same nine combinations `tests/components/kanban/taskStages.test.js`
        // pins on the front end. The payload's own spec and plan are `On`
        // throughout, so wherever the parent settles the answer, that `On` is
        // exactly what must not survive the crossing.
        for (brainstorm, spec) in [
            (Stage::Auto, Stage::Auto),
            (Stage::Auto, Stage::On),
            (Stage::Auto, Stage::Off),
            (Stage::Off, Stage::Auto),
            (Stage::Off, Stage::On),
            (Stage::Off, Stage::Off),
        ] {
            let settled = brainstorm;
            assert_eq!(
                cascade(brainstorm, spec, Stage::On),
                (settled, settled),
                "{brainstorm:?}/{spec:?}: a stage under a parent that is not On reads as it"
            );
        }

        // Under a discussion, the spec is the person's, and the plan follows
        // whatever the spec ended up being.
        assert_eq!(cascade(Stage::On, Stage::On, Stage::On), (Stage::On, Stage::On));
        assert_eq!(cascade(Stage::On, Stage::On, Stage::Off), (Stage::On, Stage::Off));
        assert_eq!(cascade(Stage::On, Stage::Auto, Stage::On), (Stage::Auto, Stage::Auto));
        assert_eq!(cascade(Stage::On, Stage::Off, Stage::On), (Stage::Off, Stage::Off));
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
