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
pub mod oneshot;
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
    /// The issue this task is a follow-up to, when the dialog was opened from a
    /// card's own menu rather than from "+ New task".
    ///
    /// The id and not the title: the agent runs `bd show` on it anyway, and a
    /// title copied here would be the board as it stood when a menu opened
    /// rather than as it stands when the session starts — the same reason
    /// `Intent::ResolveTask` deliberately carries almost nothing. The dialog
    /// does draw the title, and reads it from the store without crossing this
    /// boundary at all.
    ///
    /// `default` for the reason `images` above carries: a payload written
    /// before this field existed must still start a session.
    #[serde(default)]
    pub parent: Option<String>,
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
    /// Answer what a run could not settle on its own, and unpark the task.
    /// Started from a parked card's own menu, and from the dialog a person
    /// gets when they move one to Ready with the question still open.
    ///
    /// It carries the id and the title and nothing else — deliberately not the
    /// questions. They are `parked:` lines in the issue's own notes, the agent
    /// reads the issue anyway, and a copy sent from the front end would be the
    /// board as it stood when a menu opened rather than as it stands when the
    /// session starts.
    ResolveTask {
        id: String,
        title: String,
    },
    /// Finish a merge or a rebase the Git panel started and git stopped on
    /// conflicts. Started from the modal that opens the moment it does — the
    /// same idiom as "Ask agent to edit" and "Answer questions", because the
    /// app has no merge editor and resolving a conflict is work rather than a
    /// dialog.
    ///
    /// It carries the whole of what the agent needs, and that is deliberate
    /// where `ResolveTask` deliberately carries almost nothing: a parked task's
    /// questions are in the issue and bd can be asked again, while a conflicted
    /// tree is a moment — the paths are what git left unmerged *then*, and the
    /// branch a rebase moved off is not readable from HEAD any more, since a
    /// stopped rebase leaves it detached.
    ResolveConflict {
        /// The repository's absolute path. The session's own directory is the
        /// project, which is not the same folder in a project of several
        /// repositories, so the prompt names this one.
        repo: String,
        /// Which of the two operations stopped. **`op` and not `kind`**: the
        /// enum above is tagged `kind`, and a field of that name would be the
        /// tag's own.
        op: crate::vcs::model::OpKind,
        /// The branch this repository was on when it started.
        ours: String,
        /// The branch being merged in, or the one being rebased onto.
        theirs: String,
        /// Every path git left unmerged.
        files: Vec<String>,
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
        /// Absolute directory this run's batches write their own account into,
        /// one JSON file per batch — `<project>/.smetana/runs/<token>/`, which
        /// `runs/gitignore.rs` already keeps out of the repository. The app
        /// cannot see what a session did (nothing comes back from one but an
        /// exit code), so the lead is asked for it, and a batch that leaves
        /// nothing is named in the report rather than drawn as an empty row.
        reports: std::path::PathBuf,
        /// Which batch of the run this session is — the `<n>` in `batch-<n>.json`
        /// under that directory. The prompt names the whole file rather than
        /// the folder, because a number the agent had to work out for itself
        /// is a number the app could not then match to the batch it timed.
        batch: u32,
        /// Whether this run removes each task's worktree once it is merged and
        /// closed — `settings.json`'s `git.removeWorktrees`, read once when the
        /// run started.
        ///
        /// A field of its own rather than a member of `RunSettings`, which is
        /// where its two neighbours in the prompt (`live_check`,
        /// `file_findings`) live. `settings.json` keeps a per-project mirror of
        /// `RunSettings` — what the run dialog opens on — so anything added
        /// there acquires a second, per-project memory of itself, and that
        /// stale copy would ride in from the dialog and silently beat the one
        /// global answer a person set in the settings window. This slot exists
        /// for exactly that: a fact about the run the dialog never asked about.
        remove_worktrees: bool,
    },
}

impl Intent {
    /// What this reduces to for the two panels that draw a session. It lives
    /// here rather than in `terminal::model` because it is knowledge about
    /// `Intent` — which of its payload is drawn and which is only a briefing
    /// for the agent — and the answer moves whenever a variant does.
    ///
    /// A draft's four fields come along and its `images` do not: the right
    /// panel draws the prose, the type, the priority and the parent a follow-up
    /// was filed against, and the paths of the
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
                // The parent comes along, unlike the images: the panel draws it
                // as a row, and it is the one thing that makes this draft
                // different from any other.
                parent: draft.parent.clone(),
            },
            Intent::EditTask { id, .. } => W::EditTask { id: id.clone() },
            Intent::ResolveTask { id, .. } => W::ResolveTask { id: id.clone() },
            // The repository and the branch being brought in are what a row can
            // draw; the conflicted paths are a briefing for the agent, exactly
            // as a draft's images are, and no row has anywhere to put a dozen
            // of them.
            Intent::ResolveConflict { repo, theirs, .. } => {
                W::ResolveConflict { repo: repo.clone(), theirs: theirs.clone() }
            }
            Intent::Setup => W::Setup,
            Intent::Run { .. } => W::Run,
        }
    }
}

/// The languages a session is started with: the one the agent talks to the
/// person in, and the one the prose of a bd issue it writes is in.
///
/// Three fields rather than one, because they answer different questions and a
/// person may want them apart: a lead who reads Russian may still keep a
/// tracker their whole team reads in English, and a repository whose history is
/// English is not a reason to be spoken to in it.
///
/// It travels on the `Launch` rather than through `terminal_create`'s
/// signature, so that a session started by a person and a batch started by a
/// run get the same answer by construction — `terminal::service` reads it once,
/// from the file, where both paths already meet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Languages {
    /// What the agent says to the person is written in.
    pub agent: String,
    /// What the agent writes into bd is written in. The `##` section headings
    /// are the exception and stay English whatever this says — `prompt.rs`
    /// records why.
    pub task: String,
    /// What a git commit message is written in — both the one the Git panel's
    /// button asks for and the ones an agent writes with its own hands during a
    /// run. Whatever sits in front of the colon is the exception and does not
    /// move, for the reason `prompt.rs` and `oneshot::commit_prompt` both
    /// record: it is grepped and read rather than translated. Which form that
    /// is differs by caller — the button's prompt names Conventional Commits
    /// because the app composes that message itself, while a session is told to
    /// leave its project's own convention where it found it.
    pub commit: String,
}

impl Default for Languages {
    fn default() -> Self {
        Self {
            agent: DEFAULT_LANGUAGE.into(),
            task: DEFAULT_LANGUAGE.into(),
            commit: DEFAULT_LANGUAGE.into(),
        }
    }
}

/// Everything a spawn needs before any agent has looked at it.
pub struct Launch {
    pub profile: &'static dyn Profile,
    pub cwd: PathBuf,
    pub intent: Intent,
    pub skills: library::Skills,
    /// What the session speaks and what it writes into the tracker. Read from
    /// `settings.json` by the caller, for the reason `facts` is: `prompt.rs`
    /// stays pure and the disk stays outside it.
    pub languages: Languages,
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

    /// How this harness is asked one question with nobody watching, as the
    /// arguments that go in front of the prompt. `agents::oneshot` is the
    /// caller, and the commit-message button in the Git panel is what wants it.
    ///
    /// Not the same question as `batch_args`, though Claude Code answers both
    /// with `-p`: that one is "carry this batch out and exit" and comes with a
    /// stream format and a translator, because a person watches a batch work.
    /// This one is "answer this and exit", and what is wanted is the answer on
    /// stdout with nothing around it.
    ///
    /// The default is `None`, a working answer rather than a gap in the same
    /// shape as `usage_command`'s: a harness with no non-interactive form
    /// simply cannot be asked, and the panel draws no button rather than one
    /// that fails every time it is pressed.
    fn oneshot_args(&self) -> Option<&'static [&'static str]> {
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

    /// How this harness is told to carry one batch out and **exit**, as
    /// arguments in front of everything else on its command line.
    ///
    /// A run's loop reads the board again only once the batch's process is gone
    /// (`runs::service::watch_batch` waits on `await_exit`), and an ordinary
    /// interactive session never goes: it finishes the work, reports, and sits
    /// at its prompt. So a batch nobody is watching is started in the harness's
    /// non-interactive form, where finishing the work and exiting are one event.
    ///
    /// In front of everything else because a harness may answer with a
    /// subcommand rather than a flag, and a subcommand has one legal position.
    ///
    /// The default is nothing, and — like `autonomy`'s — it is a working answer
    /// rather than a gap: a harness with no non-interactive form runs exactly as
    /// every harness ran before this existed.
    fn batch_args(&self) -> &'static [&'static str] {
        &[]
    }

    /// How one line of what a batch prints becomes what a person sees in the
    /// pane, or `None` where the harness's own output is already that.
    ///
    /// It is a pair with `batch_args` and belongs to the same profile for the
    /// same reason layer B does: what a harness emits in its non-interactive
    /// form is knowledge about that harness. Claude Code's only streaming form
    /// is JSONL, so its answer is a translator; a harness that prints readable
    /// progress by itself keeps the default and its bytes pass through
    /// untouched.
    ///
    /// A function pointer rather than a trait object: the rendering is pure —
    /// one line in, zero or more lines out — and nothing about it needs to
    /// borrow the profile.
    fn transcript(&self) -> Option<fn(&str) -> Vec<String>> {
        None
    }
}

/// What a profile needs added to run a batch.
#[derive(Default, Debug, PartialEq, Eq)]
pub struct Autonomy {
    pub args: Vec<&'static str>,
    pub env: Vec<(&'static str, &'static str)>,
}

/// Is this session an unattended run's batch — the one kind that has to end by
/// itself, and therefore not an interactive session at all?
///
/// Written once because two separate things hang off it: the arguments in
/// `Profile::batch_args` and the translator in `Profile::transcript`. Two copies
/// of the condition would eventually disagree about which sessions have a person
/// in them, and the disagreement would be silent in both directions — a
/// supervised session with no interface, or a batch that never ends.
///
/// `RunMode::unattended` is the same predicate `watch_batch` already reads to
/// decide whether an unanswered question ends the batch, so this cannot drift
/// from that either.
pub fn is_batch(intent: &Intent) -> bool {
    matches!(intent, Intent::Run { settings, .. } if settings.mode.unattended())
}

/// The closed list of agent ids, and the only copy of it. `settings/model.rs`
/// validates against this rather than repeating it: the side-tab set is
/// already written out twice in this codebase and the cost is recorded in
/// CLAUDE.md — a value that survives the session and silently comes back as
/// something else.
pub const IDS: [&str; 2] = ["claude", "codex"];

/// The languages a person may pick, as BCP-47 ids with the English name of
/// each, and the only copy of that list — `settings/model.rs` validates against
/// it rather than repeating it, exactly as it does for `IDS` above and for the
/// reason recorded there.
///
/// The name is carried beside the id because it is not decoration: it is what
/// `prompt.rs` writes into the prompt. An agent told to answer in `zh-Hans` is
/// being handed a tag out of a settings file, where "Chinese (Simplified)" is
/// a sentence.
///
/// English is first, and `the_default_language_leads_the_table` pins that:
/// `language_name` falls back to the head of this list, and a table reordered
/// under it would silently start naming some other language as the default.
pub const LANGUAGES: [(&str, &str); 12] = [
    ("en", "English"),
    ("ru", "Russian"),
    ("zh-Hans", "Chinese (Simplified)"),
    ("es", "Spanish"),
    ("hi", "Hindi"),
    ("pt", "Portuguese"),
    ("fr", "French"),
    ("de", "German"),
    ("ja", "Japanese"),
    ("ko", "Korean"),
    ("it", "Italian"),
    ("tr", "Turkish"),
];

/// What every language setting means when nobody has chosen and what a value off
/// the table falls back to. English rather than an "Auto" that adds nothing to
/// the prompt: an Auto default would be today's behaviour exactly, so the
/// setting would do nothing for anybody until they went and changed it. It is
/// the same argument for all three, and for `commitLanguage` it is the letter
/// of today's behaviour as well — `oneshot::commit_prompt` asked for a message
/// in English outright before the setting existed.
pub const DEFAULT_LANGUAGE: &str = "en";

/// Whether this is a language the app ships. `settings/model.rs` asks, so that
/// a hand-edited file loses one field rather than a section.
pub fn known_language(id: &str) -> bool {
    LANGUAGES.iter().any(|(known, _)| *known == id)
}

/// The English name to write into a prompt for a language id, and total rather
/// than optional: an id nobody ships reads as the default's name. `build` is
/// pure and takes whatever it is handed, and an unknown tag written into the
/// prompt raw would be an instruction nobody can follow.
pub fn language_name(id: &str) -> &'static str {
    LANGUAGES.iter().find(|(known, _)| *known == id).map_or(LANGUAGES[0].1, |(_, name)| *name)
}

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
    fn the_default_language_leads_the_table() {
        // `language_name` falls back to the head of the list, so a table
        // reordered under it would quietly start naming Russian, or Hindi, as
        // what an unknown id means.
        assert_eq!(LANGUAGES[0].0, DEFAULT_LANGUAGE);
        assert!(known_language(DEFAULT_LANGUAGE));
    }

    #[test]
    fn every_shipped_language_has_a_name_and_nothing_else_does() {
        for (id, name) in LANGUAGES {
            assert!(known_language(id), "{id}");
            assert_eq!(language_name(id), name, "{id}");
        }
        assert!(!known_language("xx"));
        assert_eq!(language_name("xx"), "English", "an id nobody ships reads as the default");
    }

    #[test]
    fn a_session_with_nothing_chosen_speaks_the_default() {
        assert_eq!(
            Languages::default(),
            Languages {
                agent: DEFAULT_LANGUAGE.into(),
                task: DEFAULT_LANGUAGE.into(),
                commit: DEFAULT_LANGUAGE.into()
            }
        );
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
    fn a_conflict_intent_deserializes_from_the_front_ends_json() {
        // Copied from what `resolveConflictWithAgent` in
        // `src/views/DesktopApp.vue` hands to `createSession`. `op` and not
        // `kind` is the load-bearing part: the enum's own tag is `kind`, so a
        // field by that name would be the tag's and this payload would not
        // deserialize at all — a modal whose one door refused to open.
        let json = r#"{
            "kind": "resolveConflict",
            "repo": "/p/backend",
            "op": "rebase",
            "ours": "main",
            "theirs": "develop",
            "files": ["src/one.rs", "src/two.rs"]
        }"#;
        let intent: Intent = serde_json::from_str(json).expect("deserializes");
        match intent {
            Intent::ResolveConflict { repo, op, ours, theirs, files } => {
                assert_eq!(repo, "/p/backend");
                assert_eq!(op, crate::vcs::model::OpKind::Rebase);
                assert_eq!(ours, "main");
                assert_eq!(theirs, "develop");
                assert_eq!(files, ["src/one.rs", "src/two.rs"]);
            }
            other => panic!("expected ResolveConflict, got {other:?}"),
        }
    }

    #[test]
    fn a_conflict_intent_carries_where_it_happened_and_leaves_the_paths_behind() {
        use crate::terminal::model::SessionWork as W;
        let intent = Intent::ResolveConflict {
            repo: "/p/backend".into(),
            op: crate::vcs::model::OpKind::Merge,
            ours: "main".into(),
            theirs: "develop".into(),
            files: vec!["src/one.rs".into(), "src/two.rs".into()],
        };
        // The repository and the branch coming in are what a row draws; the
        // conflicted paths are the agent's briefing, exactly as a draft's
        // images are, and the branch the repository was on is only wanted to
        // put the prompt's sentence the right way round.
        assert_eq!(
            intent.work(),
            W::ResolveConflict { repo: "/p/backend".into(), theirs: "develop".into() }
        );
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
                parent: None,
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
                parent: None,
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
                parent: None,
            },
        };
        assert_eq!(
            intent.work(),
            W::NewTask {
                text: "Something".into(),
                issue_type: None,
                priority: None,
                parent: None,
            }
        );
    }

    #[test]
    fn a_new_task_intent_without_a_parent_still_deserializes() {
        // A payload written before this field existed must still start a
        // session. Absence is the ordinary case: every task filed from
        // "+ New task" has no parent at all.
        let intent: Intent = serde_json::from_value(serde_json::json!({
            "kind": "newTask",
            "brainstorm": "auto",
            "spec": "auto",
            "plan": "auto",
            "draft": { "text": "x", "issue_type": null, "priority": null }
        }))
        .expect("deserializes");
        match intent {
            Intent::NewTask { draft, .. } => assert_eq!(draft.parent, None),
            other => panic!("expected NewTask, got {other:?}"),
        }
    }

    #[test]
    fn a_follow_up_carries_the_parents_id_through_to_the_panel() {
        // The id is what the draft panel draws, and the only confirmation the
        // person who pressed the menu row gets that the parent was carried at
        // all — the dialog has closed by then.
        let intent: Intent = serde_json::from_value(serde_json::json!({
            "kind": "newTask",
            "brainstorm": "off",
            "spec": "off",
            "plan": "off",
            "draft": { "text": "x", "issue_type": null, "priority": null, "parent": "smetana-3uv" }
        }))
        .expect("deserializes");
        match intent.work() {
            crate::terminal::model::SessionWork::NewTask { parent, .. } => {
                assert_eq!(parent.as_deref(), Some("smetana-3uv"));
            }
            other => panic!("expected NewTask work, got {other:?}"),
        }
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

    fn run_intent(mode: crate::runs::model::RunMode) -> Intent {
        Intent::Run {
            settings: crate::runs::model::RunSettings {
                scope: crate::runs::model::RunScope::Queue,
                mode,
                target_branch: "main".into(),
                create_target: false,
                min_priority: Some(2),
                max_parallel_tasks: (!matches!(mode, crate::runs::model::RunMode::Solo))
                    .then_some(3),
                live_check: true,
                file_findings: true,
            },
            reports: std::path::PathBuf::from("/p/.smetana/runs/7"),
            batch: 1,
            remove_worktrees: true,
        }
    }

    #[test]
    fn only_a_run_with_nobody_watching_is_a_batch() {
        // The whole rule, in one place, because two things hang off it: the
        // arguments that make the process exit, and the translator that makes
        // what it prints readable. A session with a person in front of it is
        // neither — taking their interface away would take away what they
        // started.
        use crate::runs::model::RunMode;

        assert!(is_batch(&run_intent(RunMode::Auto)));
        assert!(!is_batch(&run_intent(RunMode::Supervised)));
        assert!(!is_batch(&run_intent(RunMode::Solo)));
        assert!(!is_batch(&Intent::Bare));
        assert!(!is_batch(&Intent::Setup));
        assert!(!is_batch(&Intent::EditTask { id: "a-1".into(), title: "t".into() }));
    }

    #[test]
    fn a_harness_that_was_given_neither_answer_keeps_the_session_it_has() {
        // The defaults are working answers rather than gaps, the shape every
        // other optional method on this trait keeps: such a harness runs
        // exactly as it does today. For Codex that is deliberate and recorded
        // — its own task — not an oversight here.
        struct Plain;
        impl Profile for Plain {
            fn id(&self) -> &'static str {
                "plain"
            }
            fn binary(&self) -> &'static str {
                "plain"
            }
            fn delivery(&self) -> SkillDelivery {
                SkillDelivery::Inline
            }
            fn command(&self, _launch: &Launch) -> portable_pty::CommandBuilder {
                portable_pty::CommandBuilder::new(self.binary())
            }
        }
        assert!(Plain.batch_args().is_empty());
        assert!(Plain.transcript().is_none());
    }

    #[test]
    fn a_translator_is_only_ever_installed_over_a_stream_that_was_asked_for() {
        // The two answers are a pair, and nothing else in the app checks that
        // they agree: `terminal::service` installs the translator on `is_batch`
        // alone, without knowing whether this profile's `command` actually put
        // `batch_args` on the line — each harness applies them itself, in its
        // own body, because they have to lead the argv.
        //
        // Both ways of getting it wrong are silent, and one is worse than the
        // other. A profile answering `transcript` and forgetting the arguments
        // gets a JSONL translator over an interactive TUI's ANSI stream: every
        // line is unparseable, every line renders as nothing, and the pane is
        // blank for the length of a batch with nothing in any log to say why.
        // The other way round only leaves a pane of raw JSON, which at least
        // says what happened.
        for id in IDS {
            let profile = resolve(id).expect("a listed id must resolve");
            assert_eq!(
                profile.transcript().is_some(),
                !profile.batch_args().is_empty(),
                "{id}: a harness reads its own non-interactive output or asks for neither"
            );
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
