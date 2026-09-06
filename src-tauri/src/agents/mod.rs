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
pub mod codex_sessions;
pub mod commands;
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

/// One repository's share of a branch review: what is compared, and against
/// what. A list of these is the whole of what `Intent::ReviewBranch` was asked
/// to look at.
///
/// `base` and `head` are refs in the spelling git itself takes — `main` for a
/// local branch, `origin/main` for a remote-tracking one — and the choice
/// between the two is settled before anything reaches here. Nothing downstream
/// re-reads a branch list or guesses at a remote: a pair that named "the
/// branch" rather than a ref would mean something different the next time
/// somebody fetched.
///
/// The `rename_all` is inert today and is written all the same. Nothing here
/// is two words, and the camelCase of a single lower-case word is itself, so
/// the attribute could be deleted with nothing on either side of the wire
/// changing. It is here for the field added later, which should not have to
/// remember it. `Intent::RepairTracker`'s variant-level copy is a different
/// case and not a precedent for this one: it earns its place on `bd_version`,
/// a name that genuinely moves, and so does `ReviewBranch`'s own, added with
/// `fetch_failed`. And this is not a guard — the three fields are
/// `String` with no `serde(default)`, so a rename that was needed and missing
/// would be serde refusing the payload by name rather than anything silent.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ReviewPair {
    /// The repository's absolute path. A project can hold several and the
    /// session's own directory is the project rather than any one of them, so
    /// a pair that left this out would name no repository at all.
    pub repo: String,
    pub base: String,
    pub head: String,
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
    /// Correct work that is already finished and merged. Started from the one
    /// row a done card's menu offers, where the play and the edit used to be.
    ///
    /// It carries the id and the title and nothing else, for the reason
    /// `ResolveTask` above spells out: the agent runs `bd show` itself, and a
    /// description copied here would be the board as it stood when a menu
    /// opened rather than as it stands when the session starts.
    ///
    /// Its own variant rather than an `EditTask`, and the difference is the
    /// whole point of it: an edit changes an issue's prose, this changes the
    /// code behind a closed one. Different prompt, different caption on the
    /// row, and — unlike an edit — a commit at the end of it.
    FixTask {
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
    /// Look at a tracker the app could not repair by itself. Started from the
    /// second button under the board's `error` empty state, beside the one
    /// that runs bd's own migrations.
    ///
    /// It carries the whole of the failure, and that is deliberate in the way
    /// `ResolveConflict` above is rather than the way `ResolveTask` is. A
    /// parked task's questions stay in the issue and bd can be asked again;
    /// here **bd is what is broken**, so there is nothing to ask again once the
    /// session has started, and a briefing that is not complete at the moment
    /// it is sent is never going to be completed.
    ///
    /// The `rename_all` on the enum renames the *variants*; a struct variant's
    /// fields need their own, and this one has it so `bd_version` arrives as
    /// `bdVersion` — the same word `tracker_failure` hands the front end, so
    /// what comes back off that command goes straight into this intent with
    /// nothing renamed on the way.
    #[serde(rename_all = "camelCase")]
    RepairTracker {
        /// The project directory. The session's own directory is the project
        /// too, but a prompt that leaves it unsaid would be one an agent has to
        /// guess the subject of.
        dir: String,
        /// The bd this build ships, so the agent can tell "the database is
        /// older than the binary" from "the binary is not what we think it
        /// is". It comes from `tracker::service::EXPECTED_BD_VERSION` by way of
        /// `tracker_failure`, never from a second copy of the number.
        bd_version: String,
        /// The bd command line that failed, without the binary's own name.
        command: String,
        /// What that command printed to stderr, in full and untranslated: it is
        /// bd's own account of the trouble, and the one thing here nothing else
        /// can reconstruct.
        stderr: String,
    },
    /// Pick a Claude Code session up again from its transcript on disk, by the
    /// id that transcript is named after. Started from a card in the Sessions
    /// tab, which is the one place in this app that knows those files exist.
    ///
    /// It is an ordinary agent and goes through the one road every other
    /// session takes — `terminal_create`, a profile's `command`, `Pty::spawn` —
    /// because a second way to start an agent is the place two ways silently
    /// diverge. What is different is only what is added to the command line
    /// (`Profile::resume_args`) and where it runs.
    ///
    /// `cwd` is why this variant carries a path at all, and it is the whole
    /// point of the feature: `claude --resume` resolves an id against the
    /// directory it is run in, so the same id in another folder is a session
    /// Claude Code has never heard of — and a worktree session resumed at the
    /// project root would be an agent that thinks it is in another tree.
    /// `terminal::service` checks it before anything is spawned; nothing here
    /// may assume it is still on disk.
    ///
    /// `title` is the first thing the person typed in that session, already
    /// clipped by `sessions::model::CLIP`, and it is here for the row rather
    /// than for the agent: a resumed session has no tracker work, so without it
    /// the row in the agents panel would either say nothing about which
    /// conversation this is or pass itself off as a claimed task. It is the
    /// person's own words rather than the first record in the file:
    /// `sessions::kickoff` takes them back out of whatever prompt Smetana
    /// wrapped them in, so a row cannot caption a resumed session with the
    /// language paragraph this app opened it with.
    ///
    /// `None` in two cases, both ordinary: a transcript with no human message
    /// in it at all, and a session Smetana started for something nobody types a
    /// word into — a run's batch, a setup, "+ New agent".
    ///
    /// `fork` is the whole difference between the two rows the Sessions tab
    /// offers. `false` is Resume in worktree, which goes on writing into the
    /// transcript it opened; `true` is Continue in a new session, which leaves
    /// that file exactly as it was and starts a second one beside it from the
    /// same history. One variant and not two, because everything else about
    /// them is the same — the directory, the id, the row it draws — and the
    /// arguments are the profile's own answer either way (`resume_args`
    /// against `fork_args`).
    ResumeSession {
        id: String,
        cwd: String,
        title: Option<String>,
        fork: bool,
    },
    /// Work out what this project is made of and write
    /// `.smetana/project.toml`. Started from the dialog a person gets when
    /// they add a project, and from the project row afterwards.
    Setup,
    /// Review what one branch adds to another — in one repository or in
    /// several — and write the result up for somebody to read afterwards.
    ///
    /// It carries the whole of what it was asked to look at rather than a
    /// reference to it, which is the rule `Intent::Run` already holds: the
    /// window that started this can be closed and the selection moved while
    /// the session is still reading, so a briefing that is not complete at the
    /// moment it is sent is never going to be completed.
    ///
    /// It neither writes to the tracker nor commits: it writes two files under
    /// `.smetana/reviews/` and touches nothing else. That much it shares with
    /// `Setup`, `RepairTracker` and `ResumeSession`, and the sentence that said
    /// otherwise here was simply wrong. **What is its own is that it says so:**
    /// `prompt.rs`'s `writes_to_the_tracker` and `commits_to_git` both name it
    /// in a `false` arm rather than letting it fall through the way the other
    /// three do — because its prompt promises the agent it will file nothing
    /// and commit nothing, and a promise kept by an absence somewhere else is
    /// one the next reader cannot check.
    ///
    /// The variant carries a `rename_all` of its own, and it is load-bearing
    /// rather than decorative: the enum's renames the *variants*, so a struct
    /// variant's fields need their own or `fetch_failed` never matches the
    /// wire's `fetchFailed`. With `serde(default)` beside it that failure is
    /// silent — the field arrives empty, the prompt says nothing, and both
    /// suites stay green. `RepairTracker` above paid for exactly that once.
    #[serde(rename_all = "camelCase")]
    ReviewBranch {
        pairs: Vec<ReviewPair>,
        /// The report's path relative to the project, without an extension:
        /// the agent writes `<report>.md` and `<report>.html`. The app chooses
        /// the name rather than the agent, so that the tab it opens afterwards
        /// is at a path the app already knows.
        report: String,
        /// The repositories whose `git fetch` did not work in the moment
        /// before this review was started, named by the same absolute path a
        /// `ReviewPair` names one by.
        ///
        /// **A failed fetch does not call the review off** — what `origin`
        /// holds on this machine is still readable, merely older — so what is
        /// owed is a sentence rather than a refusal. That sentence was on
        /// screen and in a toast from the day the window shipped, and nowhere
        /// else: somebody who never saw the toast had nothing in the report to
        /// learn it from, and an `origin/main` a week old reads exactly like
        /// one a minute old. This field is the half that reaches the prompt,
        /// so that the report can say so about itself.
        ///
        /// Paths and **not** the names the window draws, because the prompt
        /// lists the pairs by path and a sentence naming repositories in a
        /// second vocabulary would be asking the agent to match the two. The
        /// front end derives the names it shows from this very list, which is
        /// what keeps what a person read and what the agent was told from
        /// disagreeing.
        ///
        /// `default` for the reason `TaskDraft::parent` carries: a payload
        /// written before this field existed must still start a session.
        #[serde(default)]
        fetch_failed: Vec<String>,
    },
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
            Intent::FixTask { id, .. } => W::FixTask { id: id.clone() },
            // The repository and the branch being brought in are what a row can
            // draw; the conflicted paths are a briefing for the agent, exactly
            // as a draft's images are, and no row has anywhere to put a dozen
            // of them.
            Intent::ResolveConflict { repo, theirs, .. } => {
                W::ResolveConflict { repo: repo.clone(), theirs: theirs.clone() }
            }
            // Nothing of the failure comes along: the whole of it is a
            // briefing, and there is no id, no path and no branch a row could
            // draw. The caption alone says which work this is.
            Intent::RepairTracker { .. } => W::RepairTracker,
            // The title comes along and the id and the directory do not: the
            // row draws the conversation somebody recognises, and a
            // 36-character UUID and an absolute path are the briefing that got
            // the session started. The card in the Sessions tab is where both
            // of those are already written out.
            // A fork draws the same row as a resume, deliberately: what a
            // person picks a session out of that list for is the conversation,
            // not which file it goes on being written into. `fork` is read here
            // and thrown away — the one thing the row would gain from it is a
            // second caption nobody asked for.
            Intent::ResumeSession { title, .. } => W::ResumeSession { title: title.clone() },
            Intent::Setup => W::Setup,
            // The report's path and not the pairs, which is the reading a
            // conflict's file list gets: the row draws where the answer is
            // going to be, and the refs are the briefing — a review of four
            // repositories has eight of them, and a row is 252px wide.
            Intent::ReviewBranch { report, .. } => W::ReviewBranch { report: report.clone() },
            Intent::Run { .. } => W::Run,
        }
    }
}

/// The languages a session is started with: the one the agent talks to the
/// person in, and the one the prose of a bd issue it writes is in.
///
/// A field per question rather than one for all of them, because they answer
/// different questions and a person may want them apart: a lead who reads
/// Russian may still keep a tracker their whole team reads in English, and a
/// repository whose history is English is not a reason to be spoken to in it.
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
    /// What the prose of a run's batch file is written in — the `did` line for
    /// each task and the batch's `notes`. Only a run's lead ever writes one, so
    /// only `Intent::Run` is told about this; `prompt.rs` records why, and why
    /// the JSON keys around that prose and `runs::report`'s own labels stay
    /// English whatever this says.
    pub report: String,
}

impl Default for Languages {
    fn default() -> Self {
        Self {
            agent: DEFAULT_LANGUAGE.into(),
            task: DEFAULT_LANGUAGE.into(),
            commit: DEFAULT_LANGUAGE.into(),
            report: DEFAULT_LANGUAGE.into(),
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
    /// The person's own standing instruction, or empty. Read from
    /// `settings.json` by the caller, for the reason `languages` and `facts`
    /// are: `prompt.rs` stays pure and the disk stays outside it.
    pub agent_prompt: String,
    /// What a survey of the project found, already rendered. Only a `Setup`
    /// intent has any, and it is read by the caller for the same reason skill
    /// text is: `prompt.rs` stays pure and the disk stays outside it.
    pub facts: Option<String>,
    /// The conversation id this app chose for the session, where the profile can
    /// be told one (`Profile::session_id_args`).
    ///
    /// `None` for a resume — the id is the transcript's own and `--resume`
    /// already carries it — and for a harness with no such flag. The worker
    /// chooses it; `terminal::conversation` is what makes one.
    pub session_id: Option<String>,
    /// The model this session's role asked for, or `None` where nobody has
    /// chosen one and the harness picks for itself. Read from `settings.json`
    /// by the caller, for the reason `languages` is: `prompt.rs` stays pure and
    /// the disk stays outside it.
    ///
    /// Always `None` for `Intent::ResumeSession`, whatever the file says. A
    /// recorded conversation already has a model, and this app arriving with a
    /// second opinion is the same intrusion `prompt::build` refuses when it
    /// declines to compose a prompt for that intent: whatever was settled in
    /// there was settled before this window existed.
    pub model: Option<String>,
    /// The model a run is asked to give the subagents that write its code — the
    /// `code` role's, not this session's. Meaningful only for `Intent::Run`.
    ///
    /// It reaches the agent as one line of the run policy and never as an
    /// argument, because a subagent is spawned inside the lead's own harness
    /// and there is no command line of ours to put a flag on. That makes it a
    /// request rather than a guarantee, and the app cannot check what a
    /// subagent actually ran on.
    pub worker_model: Option<String>,
}

pub trait Profile: Sync {
    fn id(&self) -> &'static str;

    /// This harness's name as a person reads it — "Claude Code", "Codex".
    ///
    /// No default, deliberately: a harness added to `IDS` without a name must
    /// not compile. It is this product's interface copy, so it is English like
    /// the rest of it, and it lives here rather than in the front end because
    /// the front end no longer names agents at all.
    fn label(&self) -> &'static str;
    /// What to exec. Also what we look for on `PATH`.
    fn binary(&self) -> &'static str;
    fn delivery(&self) -> SkillDelivery;

    /// The models this harness accepts, each with the name a person reads in
    /// the settings window. The ids come from the CLI's own `--help`, never
    /// from memory: `.claude/rules/agents.md` records what guessing at a CLI's
    /// vocabulary costs, and this is the same vocabulary `resume_args` and
    /// `batch_args` are already careful about.
    ///
    /// **No default**, unlike almost every other method on this trait, and for
    /// the reason `label` has none: a harness added to `IDS` without a model
    /// list would ship an empty dropdown, which reads as a load that failed
    /// rather than as a decision anybody made. The compiler is the cheapest
    /// place to find that out.
    fn models(&self) -> &'static [(&'static str, &'static str)];

    /// How this harness is told which model to use, as the arguments that go on
    /// its command line.
    ///
    /// The default is an empty vector, and — like `autonomy`'s and
    /// `batch_args`' — it is a working answer rather than a gap: a harness with
    /// no such flag simply cannot be told, so the setting is inert for it
    /// instead of broken.
    fn model_args<'a>(&self, _model: &'a str) -> Vec<&'a str> {
        Vec::new()
    }

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

    /// What this harness's one-shot output *is*, as one answer.
    ///
    /// The pair to `oneshot_args`, and its own method for `parse_usage`'s
    /// reason: a harness that says how to ask a question without saying how to
    /// read the reply leaves the caller reading somebody else's format. The
    /// default is the whole of stdout, which is what `oneshot::ask_raw` did
    /// before this existed and is exactly right for a harness whose one-shot
    /// mode prints the answer alone — Claude Code's `-p` does.
    ///
    /// Codex's does not: `codex exec` prints its own progress around the
    /// answer, so it overrides this and takes the last thing the agent said.
    fn oneshot_answer(&self, stdout: &str) -> String {
        stdout.to_owned()
    }

    /// How this harness is told to pick a recorded session up again by its id,
    /// as the arguments that go in front of everything else on its command
    /// line, or `None` where it cannot be told at all.
    ///
    /// A capability and its arguments in one answer, the shape `usage_command`
    /// and `oneshot_args` already keep, and for the same reason: a profile that
    /// said "yes" without saying how would leave the caller inventing somebody
    /// else's argument grammar. The default is `None`, and it is a working
    /// answer rather than a gap — a harness that cannot be asked is refused
    /// before anything is spawned (`TerminalError::NoResume`), which is the
    /// only honest outcome: starting the agent anyway would put a fresh session
    /// in the worktree while the person is looking at a card promising the
    /// conversation they left.
    ///
    /// Owned strings rather than `&'static [&'static str]`, unlike its
    /// neighbours: the id is the argument, so there is nothing static to hand
    /// back.
    fn resume_args(&self, _session: &str) -> Option<Vec<String>> {
        None
    }

    /// How this harness is told to open a recorded session's history in a
    /// **new** session of its own, leaving the original transcript exactly as
    /// it was.
    ///
    /// Its own method rather than a flag appended to `resume_args`, for the
    /// reason that one already records: a capability and its arguments are one
    /// answer, so a caller never composes somebody else's command line out of
    /// two halves. A harness that can reopen a transcript and cannot branch one
    /// is an ordinary shape, and appending would have invented a flag for it.
    ///
    /// The default is `None` and refuses before anything is spawned
    /// (`TerminalError::NoFork`), exactly as `resume_args`'s does.
    fn fork_args(&self, _session: &str) -> Option<Vec<String>> {
        None
    }

    /// How this harness is told which conversation id to write the session
    /// under, as the arguments that go in front of everything else, or `None`
    /// where it cannot be told at all.
    ///
    /// A capability and its arguments in one answer, the shape `resume_args`
    /// and `fork_args` already keep. What it buys is knowing afterwards what to
    /// resume: the harness would otherwise invent the id and name its
    /// transcript after it, and matching a live session to a transcript by
    /// directory and mtime is a guess that is wrong exactly where two agents
    /// share a directory.
    ///
    /// The default is `None`, and it is a working answer rather than a gap.
    /// **A profile that cannot be told an id records nothing** — see
    /// `terminal::restore` — so such a harness simply never draws a row it
    /// could not resume, and no refusal has to be worded anywhere. That is what
    /// every session did before this existed.
    fn session_id_args(&self, _session: &str) -> Option<Vec<String>> {
        None
    }

    /// Whether this harness names its own conversation, so its id is worth
    /// looking for after the start.
    ///
    /// Asked while a session is being built, which is why it is a plain
    /// question and not a probe: `session_id_after_start` reads a disk, and a
    /// disk read standing in for a capability check would answer "no" on a
    /// machine that simply has not written the file yet.
    fn discovers_session_id(&self) -> bool {
        false
    }

    /// Whatever this harness had **already** recorded, as of now.
    ///
    /// Taken at the instant of the spawn and handed straight back to
    /// `session_id_after_start`, whose whole difficulty it answers: a record
    /// that was already there cannot belong to the session just started, and
    /// nothing else distinguishes the two. Only a resumed session runs in a
    /// directory of its own — everything else runs at the project root — so two
    /// agent sessions in one project share a working directory as a matter of
    /// course, and modification time alone hands the newer one whatever the
    /// older one is still writing.
    ///
    /// Paths rather than a timestamp, and it is worth saying why the obvious
    /// answer is not used: `std::fs::Metadata::created` is not answered on every
    /// filesystem this ships on, so a creation time is a fact this app cannot
    /// rely on having. A path either was in the set or it was not.
    ///
    /// The default is nothing, which is a working answer: a harness that
    /// discovers no id has nothing to take a snapshot of, and the empty set is
    /// also the honest answer for one whose records could not be read at all.
    fn sessions_before_start(&self) -> Vec<std::path::PathBuf> {
        Vec::new()
    }

    /// The id a harness gave a session **it has already started**, when this
    /// app could not name one in advance.
    ///
    /// The other half of `session_id_args`, and deliberately a second method
    /// rather than a fallback inside it: being told an id and finding out an id
    /// are two capabilities, a harness may have either, and a caller composing
    /// one out of the other would be inventing somebody else's behaviour.
    ///
    /// `before` is what `sessions_before_start` answered at the spawn, and the
    /// caller is required to have taken it *then* rather than now — a snapshot
    /// taken at the first poll would already contain this session's own record
    /// and could never match anything.
    ///
    /// The default is `None` — a harness answering neither records no
    /// conversation, which is what every harness but Claude Code did before
    /// this existed.
    fn session_id_after_start(
        &self,
        _cwd: &std::path::Path,
        _started_after: std::time::SystemTime,
        _before: &[std::path::PathBuf],
    ) -> Option<String> {
        None
    }

    /// The line this harness understands as "forget everything said so far and
    /// carry on in this same session", exactly as a person would type it into
    /// the harness's own composer — or `None` where there is no such line.
    ///
    /// It sits beside `resume_args` and `fork_args` for their reason: the app
    /// knows it wants a conversation cleared and does not know the words, and
    /// each CLI has its own. What parts it from all three of its neighbours is
    /// *when* it is read — they are read while a command line is being built,
    /// this one while a session is already running, so what it produces is
    /// written into that session's input rather than spawned. The keystroke
    /// that submits it is the caller's (`terminal::service`), because a profile
    /// carrying one would be describing a keyboard rather than a command.
    ///
    /// The default is `None`, a working answer rather than a gap in the shape
    /// every optional method here keeps. **It is filled in from the harness's
    /// own help and never guessed**, and the reason is that the failure is
    /// silent: a slash command a CLI has never heard of is not an error there,
    /// it is ordinary text, and it reaches the agent as the first line of a
    /// prompt. `codex.rs` keeps the default for that reason, exactly as it
    /// keeps `resume_args`'.
    ///
    /// Nothing on disk is touched by any answer to this: the transcript stays a
    /// file and the Sessions tab goes on listing it, which is what makes the
    /// menu row ask for no confirmation.
    fn clear_command(&self) -> Option<&'static str> {
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

/// Which row of the settings window decides this session's harness and its
/// model. Four named roles and a default, rather than one row per `Intent`:
/// eleven rows is a settings screen nobody reads, and it would still not have
/// separated a run's lead from the subagents it delegates to, since both live
/// behind `Run`.
///
/// The lead is its own role for exactly that reason. A `--model` flag on a run's
/// session sets the **lead's** model and nothing else — the subagents are
/// spawned by the lead inside its own harness and take that harness's default —
/// so "Opus writes the code" and "Opus leads the run" are two different requests
/// and one flag cannot carry both.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Tasks,
    Code,
    RunLead,
    ReviewBranch,
    Default,
}

/// Which role this intent falls into. Pure, and here rather than in `settings`
/// for that reason: the mapping is a rule about intents, while reading somebody's
/// file is not, and this half has to be testable without a disk.
pub fn role_of(intent: &Intent) -> Role {
    match intent {
        Intent::NewTask { .. } | Intent::EditTask { .. } | Intent::ResolveTask { .. } => Role::Tasks,
        Intent::FixTask { .. } | Intent::ResolveConflict { .. } => Role::Code,
        Intent::Run { .. } => Role::RunLead,
        Intent::ReviewBranch { .. } => Role::ReviewBranch,
        // `Bare`, `Setup`, `RepairTracker` — and, deliberately,
        // `ResumeSession`. A resumed conversation keeps the model it was
        // started with and is never told one at all (`Launch::model`), so the
        // row it nominally belongs to costs it nothing either way; putting it
        // anywhere else would only invite somebody to make that row reach it.
        Intent::Bare
        | Intent::Setup
        | Intent::RepairTracker { .. }
        | Intent::ResumeSession { .. } => Role::Default,
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

/// What one harness can do, as the front end needs to know it while a row is
/// being drawn.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Capabilities {
    pub resume: bool,
    pub fork: bool,
    pub clear: bool,
    pub usage: bool,
    pub batch: bool,
    pub oneshot: bool,
}

/// One model a harness offers, as the front end needs it while a dropdown is
/// being drawn: the id that goes on the command line and the name a person
/// reads beside it.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentModel {
    pub id: String,
    pub label: String,
}

/// One harness, as the front end sees it.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentRow {
    pub id: String,
    pub label: String,
    pub capabilities: Capabilities,
    /// What this harness may be asked to run on. Carried in the same row as the
    /// capabilities and for the same reason: the front end draws a model picker
    /// per harness, and a hand-written table over there would be the fifth copy
    /// of a fact this file owns — the four this command abolished are named on
    /// `catalogue` below.
    pub models: Vec<AgentModel>,
}

/// Every shipped harness and what it can do — **asked of the profiles**, never
/// written out.
///
/// This is what replaced four hand-kept lists in `src/`: `AGENTS` in
/// `AgentSettings.vue`, `RESUMES_BY_ID` and `FORKS_BY_ID` in `sessionMenu.js`,
/// `CLEARS_BY_ID` in `agentMenu.js`. Each was a knowing second copy of a fact
/// this file owns, each drifted quietly in both directions, and a third harness
/// meant four edits in two languages.
///
/// The probe string handed to the id-taking methods is never used: those are
/// asked only whether they answer at all.
pub fn catalogue() -> Vec<AgentRow> {
    IDS.iter()
        .filter_map(|id| resolve(id))
        .map(|profile| AgentRow {
            id: profile.id().to_owned(),
            label: profile.label().to_owned(),
            capabilities: Capabilities {
                resume: profile.resume_args("probe").is_some(),
                fork: profile.fork_args("probe").is_some(),
                clear: profile.clear_command().is_some(),
                usage: profile.usage_command().is_some(),
                batch: !profile.batch_args().is_empty(),
                oneshot: profile.oneshot_args().is_some(),
            },
            models: profile
                .models()
                .iter()
                .map(|(id, label)| AgentModel {
                    id: (*id).to_owned(),
                    label: (*label).to_owned(),
                })
                .collect(),
        })
        .collect()
}

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
/// the same argument for every one of them, and for `commitLanguage` it is the letter
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

    /// The two launching verbs of the Sessions tab draw **one** row, and this
    /// is where that is decided: `Intent::work` reads `fork` and drops it. What
    /// a person picks a session out of that list for is the conversation, and
    /// two rows that differed only in which file was being written into would
    /// spend a caption on the half nobody chose between.
    #[test]
    fn a_forked_session_draws_the_same_row_as_a_resumed_one() {
        let work = |fork| {
            Intent::ResumeSession {
                id: "9f1c0a2e-6d4b-4f77-8f1a-0c2b3d4e5f60".into(),
                cwd: "/p/.worktrees/smetana-0cj".into(),
                title: Some("Move the card to done".into()),
                fork,
            }
            .work()
        };
        assert_eq!(work(true), work(false));
        assert_eq!(
            work(true),
            crate::terminal::model::SessionWork::ResumeSession {
                title: Some("Move the card to done".into())
            }
        );
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
                commit: DEFAULT_LANGUAGE.into(),
                report: DEFAULT_LANGUAGE.into()
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
    fn a_fix_task_intent_deserializes_from_the_front_ends_json() {
        // Copied from what `askAgentToFix` in `src/views/DesktopApp.vue` hands
        // to `createSession`: the id and the title and nothing else, the shape
        // an edit and a parked task's questions already travel in.
        let json = r#"{"kind":"fixTask","id":"bd-1","title":"Some title"}"#;
        let intent: Intent = serde_json::from_str(json).expect("deserializes");
        match intent {
            Intent::FixTask { id, title } => {
                assert_eq!(id, "bd-1");
                assert_eq!(title, "Some title");
            }
            other => panic!("expected FixTask, got {other:?}"),
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
    fn a_review_intent_deserializes_from_the_front_ends_json() {
        // Two repositories and both spellings a ref arrives in, because the
        // local/remote choice is settled before this payload is built and the
        // two have to survive the trip unchanged.
        //
        // What this does **not** test is `ReviewPair`'s own `rename_all`, and
        // saying so is the point of the paragraph. `repo`, `base` and `head`
        // are single lower-case words, whose camelCase is themselves, so the
        // attribute is inert today and the whole of this test passes byte for
        // byte with it deleted. It is written all the same, and that is the
        // struct's own doc rather than a guard: the first two-word field added
        // there should not have to remember it. Nor could a missing rename
        // fail quietly here — the three fields are `String` with no
        // `serde(default)`, so serde refuses the payload by name.
        let json = r#"{
            "kind": "reviewBranch",
            "pairs": [
                {"repo": "/p/backend", "base": "main", "head": "feature/smetana-pf40"},
                {"repo": "/p/frontend", "base": "origin/develop", "head": "origin/spike"}
            ],
            "report": ".smetana/reviews/2026-08-31-pf40"
        }"#;
        let intent: Intent = serde_json::from_str(json).expect("deserializes");
        match intent {
            Intent::ReviewBranch { pairs, report, fetch_failed } => {
                // The payload above is the shape this intent shipped in, with
                // no `fetchFailed` in it at all, and it still has to start a
                // session: `serde(default)` is what makes that true and this
                // is where it is held. A review whose fetches all worked sends
                // the same thing, so the empty list is the ordinary case as
                // well as the old one.
                assert!(fetch_failed.is_empty(), "no field is no failed fetch");
                assert_eq!(
                    pairs,
                    [
                        ReviewPair {
                            repo: "/p/backend".into(),
                            base: "main".into(),
                            head: "feature/smetana-pf40".into(),
                        },
                        ReviewPair {
                            repo: "/p/frontend".into(),
                            base: "origin/develop".into(),
                            head: "origin/spike".into(),
                        },
                    ]
                );
                assert_eq!(report, ".smetana/reviews/2026-08-31-pf40");
            }
            other => panic!("expected ReviewBranch, got {other:?}"),
        }
    }

    #[test]
    fn a_review_intent_carries_the_repositories_whose_fetch_did_not_work() {
        // `fetchFailed` and not `fetch_failed`: the field is two words, so
        // this is the first thing in `ReviewBranch` that the variant's own
        // `rename_all` is actually needed for. Without it serde reads the
        // camelCase key as unknown, `default` fills the field with nothing,
        // and the prompt goes out silent about a stale origin with every test
        // in this file still passing — which is why the assertion below is on
        // the contents rather than on the payload being accepted.
        let json = r#"{
            "kind": "reviewBranch",
            "pairs": [
                {"repo": "/p/backend", "base": "origin/main", "head": "feature/smetana-cnk5"}
            ],
            "report": ".smetana/reviews/2026-08-31-cnk5",
            "fetchFailed": ["/p/backend", "/p/shared"]
        }"#;
        let intent: Intent = serde_json::from_str(json).expect("deserializes");
        match intent {
            Intent::ReviewBranch { fetch_failed, .. } => {
                assert_eq!(fetch_failed, ["/p/backend", "/p/shared"]);
            }
            other => panic!("expected ReviewBranch, got {other:?}"),
        }
    }

    #[test]
    fn a_review_intent_carries_the_report_path_and_leaves_the_pairs_behind() {
        use crate::terminal::model::SessionWork as W;
        let intent = Intent::ReviewBranch {
            pairs: vec![ReviewPair {
                repo: "/p/backend".into(),
                base: "main".into(),
                head: "feature/smetana-pf40".into(),
            }],
            report: ".smetana/reviews/2026-08-31-pf40".into(),
            fetch_failed: vec!["/p/backend".into()],
        };
        // The path is where the answer will be and is what the tab opened
        // afterwards is found by; the refs are the agent's briefing, the same
        // reading a conflict's file list gets, and so is the list of fetches
        // that did not work — the tab is opened at a path and knows nothing
        // about how current the refs behind it were.
        assert_eq!(
            intent.work(),
            W::ReviewBranch { report: ".smetana/reviews/2026-08-31-pf40".into() }
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
    fn a_fix_draws_the_issue_it_is_correcting() {
        // The title stays behind, as it does for an edit: a row draws an
        // identifier, and the right panel looks the issue up by this id.
        assert_eq!(
            Intent::FixTask { id: "smetana-42".into(), title: "Some title".into() }.work(),
            crate::terminal::model::SessionWork::FixTask { id: "smetana-42".into() },
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
            fn label(&self) -> &'static str {
                "Plain"
            }
            fn binary(&self) -> &'static str {
                "plain"
            }
            fn delivery(&self) -> SkillDelivery {
                SkillDelivery::Inline
            }
            /// One entry so the trait is satisfied; this fixture is about the
            /// session verbs and never about a model.
            fn models(&self) -> &'static [(&'static str, &'static str)] {
                &[("plain", "Plain")]
            }
            fn command(&self, _launch: &Launch) -> portable_pty::CommandBuilder {
                portable_pty::CommandBuilder::new(self.binary())
            }
        }
        assert!(Plain.batch_args().is_empty());
        assert!(Plain.transcript().is_none());
        // The same shape one method over: a harness nobody has confirmed a
        // clearing line for is asked for none, and the menu row that would
        // send one is greyed rather than sending a guess.
        assert!(Plain.clear_command().is_none());
    }

    #[test]
    fn a_harness_that_says_nothing_about_its_output_is_taken_at_its_word() {
        // The default is today's behaviour to the letter: whatever the process
        // printed is the answer, which is what `oneshot::ask_raw` did before
        // this method existed. Claude Code's `-p` prints the answer alone, so
        // it keeps this and implements nothing.
        struct Plain;
        impl Profile for Plain {
            fn id(&self) -> &'static str {
                "plain"
            }
            fn label(&self) -> &'static str {
                "Plain"
            }
            fn binary(&self) -> &'static str {
                "plain"
            }
            fn delivery(&self) -> SkillDelivery {
                SkillDelivery::Inline
            }
            /// One entry so the trait is satisfied; this fixture is about the
            /// session verbs and never about a model.
            fn models(&self) -> &'static [(&'static str, &'static str)] {
                &[("plain", "Plain")]
            }
            fn command(&self, _launch: &Launch) -> portable_pty::CommandBuilder {
                portable_pty::CommandBuilder::new(self.binary())
            }
        }
        assert_eq!(Plain.oneshot_answer("feat: add the thing"), "feat: add the thing");
        // The pair beside it, and the same shape: a harness that says nothing
        // about naming its own conversation is not asked after the start.
        assert!(!Plain.discovers_session_id());
        assert!(Plain.sessions_before_start().is_empty());
        assert_eq!(
            Plain.session_id_after_start(
                std::path::Path::new("/work/tree"),
                std::time::SystemTime::UNIX_EPOCH,
                &[]
            ),
            None
        );
    }

    #[test]
    fn a_harness_told_its_id_in_advance_has_nothing_to_go_looking_for() {
        // Claude Code is handed `--session-id`, so the discovery half is not
        // its question at all and both defaults stand. Pinned here rather than
        // in `claude.rs` because what is being checked is the trait's division
        // of labour, not that harness's command line.
        let claude = resolve("claude").expect("claude is shipped");
        assert!(claude.session_id_args("probe").is_some());
        assert!(!claude.discovers_session_id());
        assert!(claude.sessions_before_start().is_empty());
        assert_eq!(
            claude.session_id_after_start(
                std::path::Path::new("/work/tree"),
                std::time::SystemTime::UNIX_EPOCH,
                &[]
            ),
            None
        );
    }

    #[test]
    fn every_installed_harness_offers_at_least_one_model() {
        // `models` has no default for exactly this: a harness added to `IDS`
        // without a list would ship an empty dropdown, which reads as a load
        // that failed rather than as anybody's decision. The compiler catches
        // the missing method; this catches an empty one.
        for id in IDS {
            let profile = resolve(id).expect("every id in IDS resolves to a profile");
            assert!(!profile.models().is_empty(), "harness {id} offers no model to choose from");
        }
    }

    #[test]
    fn a_model_offered_by_a_harness_produces_arguments() {
        // Offering models and having no way to be told which one is the pair
        // coming apart: the settings window would draw a live-looking picker
        // over a flag that never reaches a command line.
        for id in IDS {
            let profile = resolve(id).expect("every id in IDS resolves to a profile");
            let (model, _) = profile.models()[0];
            assert!(
                !profile.model_args(model).is_empty(),
                "harness {id} offers models but cannot be told which one to use"
            );
        }
    }

    #[test]
    fn every_intent_falls_into_exactly_one_role() {
        use crate::runs::model::RunMode;
        use Role::*;

        let cases: Vec<(Intent, Role)> = vec![
            (Intent::Bare, Default),
            (Intent::Setup, Default),
            (
                Intent::RepairTracker {
                    dir: "/p".into(),
                    bd_version: "0.0.0".into(),
                    command: "bd list".into(),
                    stderr: "no such file".into(),
                },
                Default,
            ),
            (
                Intent::NewTask {
                    brainstorm: Stage::Auto,
                    spec: Stage::Auto,
                    plan: Stage::Auto,
                    draft: TaskDraft {
                        text: "Something to file".into(),
                        issue_type: None,
                        priority: None,
                        images: Vec::new(),
                        parent: None,
                    },
                },
                Tasks,
            ),
            (Intent::EditTask { id: "a-1".into(), title: "t".into() }, Tasks),
            (Intent::ResolveTask { id: "a-1".into(), title: "t".into() }, Tasks),
            (Intent::FixTask { id: "a-1".into(), title: "t".into() }, Code),
            (
                Intent::ResolveConflict {
                    repo: "/p/backend".into(),
                    op: crate::vcs::model::OpKind::Merge,
                    ours: "main".into(),
                    theirs: "feature/x".into(),
                    files: vec!["src/lib.rs".into()],
                },
                Code,
            ),
            (
                Intent::ReviewBranch {
                    pairs: vec![ReviewPair {
                        repo: "/p/backend".into(),
                        base: "main".into(),
                        head: "feature/x".into(),
                    }],
                    report: ".smetana/reviews/x".into(),
                    fetch_failed: Vec::new(),
                },
                ReviewBranch,
            ),
            (run_intent(RunMode::Auto), RunLead),
            (
                Intent::ResumeSession {
                    id: "9f1c0a2e-6d4b-4f77-8f1a-0c2b3d4e5f60".into(),
                    cwd: "/p/.worktrees/smetana-0cj".into(),
                    title: None,
                    fork: false,
                },
                Default,
            ),
        ];
        assert_eq!(cases.len(), 11, "every variant of Intent has a row here");
        for (intent, expected) in cases {
            assert_eq!(role_of(&intent), expected, "wrong role for {intent:?}");
        }
    }

    #[test]
    fn a_resumed_session_belongs_to_no_role_of_its_own() {
        // It falls into the default row and is never told a model all the same
        // — `Launch::model` and both profiles' `command` say so. Filed here
        // rather than given a row of its own because a row would invite
        // somebody to make it reach a resumed conversation, which already has
        // a model and somebody's words in it.
        let resumed = Intent::ResumeSession {
            id: "9f1c0a2e-6d4b-4f77-8f1a-0c2b3d4e5f60".into(),
            cwd: "/p/.worktrees/smetana-0cj".into(),
            title: None,
            fork: true,
        };
        assert_eq!(role_of(&resumed), Role::Default);
    }

    #[test]
    fn filing_a_task_and_leading_a_run_are_different_rows() {
        // The request this whole feature came from, in one line: three
        // different kinds of work, three places to say which model does them.
        // A second road into a session added later fails here rather than at
        // 3am in a run.
        use crate::runs::model::RunMode;
        let filing = role_of(&Intent::EditTask { id: "a-1".into(), title: "t".into() });
        assert_ne!(filing, role_of(&run_intent(RunMode::Auto)));
        assert_ne!(filing, role_of(&Intent::FixTask { id: "a-1".into(), title: "t".into() }));
    }

    #[test]
    fn every_shipped_harness_has_a_name_to_put_in_front_of_a_person() {
        for id in IDS {
            let profile = resolve(id).expect("every id resolves to a profile");
            assert!(!profile.label().is_empty(), "{id} must carry a label");
        }
    }

    #[test]
    fn the_catalogue_answers_for_every_id_and_asks_the_profiles_themselves() {
        let rows = catalogue();
        assert_eq!(rows.len(), IDS.len(), "one row per shipped harness");

        let claude = rows.iter().find(|row| row.id == "claude").expect("claude is shipped");
        assert_eq!(claude.label, "Claude Code");
        assert!(claude.capabilities.resume && claude.capabilities.fork);
        assert!(claude.capabilities.clear && claude.capabilities.usage);
        assert!(claude.capabilities.batch && claude.capabilities.oneshot);

        let codex = rows.iter().find(|row| row.id == "codex").expect("codex is shipped");
        assert_eq!(codex.label, "Codex");
        assert!(codex.capabilities.resume && codex.capabilities.fork);
        assert!(codex.capabilities.batch && codex.capabilities.oneshot);
        assert!(!codex.capabilities.usage, "this CLI has no command that prints an allowance");
    }

    #[test]
    fn no_row_of_the_catalogue_says_anything_its_own_profile_would_not() {
        // The whole point of the command: the row is derived, so this catches a
        // capability that was written out beside the profiles rather than asked
        // of them. Every id, every field, against the profile itself.
        for row in catalogue() {
            let profile = resolve(&row.id).expect("a listed id must resolve");
            assert_eq!(row.label, profile.label(), "{}: label", row.id);
            assert_eq!(
                row.capabilities.resume,
                profile.resume_args("probe").is_some(),
                "{}: resume",
                row.id
            );
            assert_eq!(
                row.capabilities.fork,
                profile.fork_args("probe").is_some(),
                "{}: fork",
                row.id
            );
            assert_eq!(
                row.capabilities.clear,
                profile.clear_command().is_some(),
                "{}: clear",
                row.id
            );
            assert_eq!(
                row.capabilities.usage,
                profile.usage_command().is_some(),
                "{}: usage",
                row.id
            );
            assert_eq!(
                row.capabilities.batch,
                !profile.batch_args().is_empty(),
                "{}: batch",
                row.id
            );
            assert_eq!(
                row.capabilities.oneshot,
                profile.oneshot_args().is_some(),
                "{}: oneshot",
                row.id
            );
        }
    }

    #[test]
    fn the_catalogue_carries_every_harnesss_models() {
        // The row is where the front end gets the model list from, and the
        // whole reason it does is that the alternative was a fifth hand-written
        // table keyed by agent id.
        for row in catalogue() {
            assert!(!row.models.is_empty(), "catalogue row for {} offers no model", row.id);
            let profile = resolve(&row.id).expect("a listed id must resolve");
            let asked: Vec<(String, String)> = profile
                .models()
                .iter()
                .map(|(id, label)| ((*id).to_owned(), (*label).to_owned()))
                .collect();
            let drawn: Vec<(String, String)> =
                row.models.iter().map(|m| (m.id.clone(), m.label.clone())).collect();
            assert_eq!(drawn, asked, "{}: the row says what its own profile says", row.id);
        }
    }

    #[test]
    fn the_catalogue_reaches_the_front_end_in_the_shape_it_reads() {
        // The store and `mockBackend.js` both read `capabilities.oneshot` and
        // the rest by those names, so the serialization is part of the
        // contract rather than an implementation detail.
        let json = serde_json::to_value(catalogue()).expect("the catalogue serializes");
        let first = json.get(0).expect("at least one harness ships");
        assert!(first.get("id").is_some() && first.get("label").is_some());
        let capabilities = first.get("capabilities").expect("a row carries its capabilities");
        for field in ["resume", "fork", "clear", "usage", "batch", "oneshot"] {
            assert!(capabilities.get(field).is_some_and(serde_json::Value::is_boolean), "{field}");
        }
        // The models travel in the same row and by the names the settings
        // window's dropdown reads them by.
        let models = first.get("models").and_then(serde_json::Value::as_array);
        let models = models.expect("a row carries the models it offers");
        let model = models.first().expect("at least one model is offered");
        assert!(model.get("id").is_some_and(serde_json::Value::is_string));
        assert!(model.get("label").is_some_and(serde_json::Value::is_string));
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

    /// A `Launch` for asking a profile what command line it would actually
    /// build. The skills point at the real bundle resources, the way each
    /// profile's own tests reach them, so nothing here depends on a read
    /// failing.
    fn launch(profile: &'static dyn Profile, intent: Intent) -> Launch {
        Launch {
            profile,
            cwd: PathBuf::from("/tmp/project"),
            intent,
            skills: library::Skills {
                smetana: PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("resources")
                    .join("smetana"),
                superpowers: PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("resources")
                    .join("superpowers"),
                superpowers_installed: false,
            },
            facts: None,
            session_id: None,
            languages: Languages::default(),
            agent_prompt: String::new(),
            model: None,
            worker_model: None,
        }
    }

    fn argv(profile: &'static dyn Profile, intent: Intent) -> Vec<String> {
        profile
            .command(&launch(profile, intent))
            .get_argv()
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect()
    }

    /// Where `wanted` sits inside `args`, as a run of consecutive elements.
    ///
    /// The length guard is not decoration: without it a `wanted` longer than
    /// `args` slices past the end and the test panics where it should have
    /// failed with its own message — which is the difference between reading
    /// "batch_args must lead the argv" and reading a slice index.
    fn run_at(args: &[String], wanted: &[String]) -> Option<usize> {
        if wanted.is_empty() || wanted.len() > args.len() {
            return None;
        }
        (0..=args.len() - wanted.len())
            .find(|&start| args[start..start + wanted.len()] == *wanted)
    }

    #[test]
    fn every_harness_puts_the_arguments_it_answered_with_on_its_own_command_line() {
        // The half the pair above cannot see, and the one that was actually
        // wrong: `terminal::service` installs the translator on `is_batch`
        // alone and refuses a resume on `resume_args` alone, but **applying**
        // either is each profile's own job, inside its own `command` body,
        // because a subcommand has one legal position and only the profile
        // knows where the rest of its line goes. So a profile can answer all
        // three methods correctly and never use any of them, and nothing
        // anywhere fails.
        //
        // What that cost when it happened: an unattended Codex batch spawned
        // the interactive TUI, which sits at its prompt for ever, so
        // `watch_batch` never came round; the JSONL translator went over that
        // TUI's ANSI stream and every row of the pane rendered as nothing; and
        // a restored row spawned a bare `codex` with no prompt in the recorded
        // worktree — a fresh session under a card promising the conversation
        // somebody left, which is exactly what `NoResume` used to prevent for
        // this harness by refusing.
        //
        // Each is checked as a **leading run** rather than by mere presence: an
        // answer scattered through the line, or behind the positional prompt,
        // is a command line this app is guessing somebody else's parser will
        // forgive.
        const ID: &str = "01a0765f-f205-74d0-8dc9-61006c68767f";
        for id in IDS {
            let profile = resolve(id).expect("a listed id must resolve");

            let batch: Vec<String> = profile.batch_args().iter().map(|a| a.to_string()).collect();
            if !batch.is_empty() {
                let args = argv(profile, run_intent(crate::runs::model::RunMode::Auto));
                assert_eq!(
                    run_at(&args, &batch),
                    Some(1),
                    "{id}: batch_args must lead the argv, got {args:?}"
                );
            }

            if let Some(resume) = profile.resume_args(ID) {
                let args = argv(profile, resuming(ID, false));
                assert_eq!(
                    run_at(&args, &resume),
                    Some(1),
                    "{id}: resume_args must lead the argv, got {args:?}"
                );
            }

            if let Some(fork) = profile.fork_args(ID) {
                let args = argv(profile, resuming(ID, true));
                assert_eq!(
                    run_at(&args, &fork),
                    Some(1),
                    "{id}: fork_args must lead the argv, got {args:?}"
                );
            }
        }
    }

    #[test]
    fn no_harness_carries_a_launching_verb_into_a_session_nobody_asked_one_of() {
        // The leak the other way, and it is the worse direction: a `resume` or
        // an `exec` on an ordinary session would reopen somebody else's
        // conversation, or take the interface away from a person who is sitting
        // in front of it.
        const ID: &str = "01a0765f-f205-74d0-8dc9-61006c68767f";
        for id in IDS {
            let profile = resolve(id).expect("a listed id must resolve");
            let verbs: Vec<String> = profile
                .batch_args()
                .iter()
                .map(|a| a.to_string())
                .chain(profile.resume_args(ID).into_iter().flatten())
                .chain(profile.fork_args(ID).into_iter().flatten())
                .filter(|arg| arg != ID)
                .collect();
            for intent in [
                Intent::Bare,
                Intent::Setup,
                Intent::EditTask { id: "smetana-42".into(), title: "t".into() },
                run_intent(crate::runs::model::RunMode::Supervised),
                run_intent(crate::runs::model::RunMode::Solo),
            ] {
                let args = argv(profile, intent);
                for verb in &verbs {
                    assert!(!args.contains(verb), "{id}: {verb} leaked into {args:?}");
                }
            }
        }
    }

    fn resuming(id: &str, fork: bool) -> Intent {
        Intent::ResumeSession {
            id: id.to_owned(),
            cwd: "/tmp/project/.worktrees/smetana-0cj".into(),
            title: Some("Move the card to done".into()),
            fork,
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
