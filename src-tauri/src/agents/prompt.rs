//! An intent becomes the text the agent opens on. Pure: the skill text, when
//! one is needed, is read by the caller and passed in.

use std::fmt::Write;
use std::path::Path;

use super::library::Skills;
use super::{
    cascade, ImageDelivery, Intent, Languages, ReviewPair, SkillDelivery, Stage, TaskDraft,
};
use crate::runs::model::{RunMode, RunScope, RunSettings};

/// The sentence that makes the agent talk the task through. It has to stand on
/// its own: `Inline` may find no skill text to attach, and `Auto` deliberately
/// attaches none.
const DISCUSS: &str =
    "Before creating anything, agree the design with me first — ask one question at a time — \
     and only then file the task, or tasks, the discussion produces. Everything you settle \
     with me goes into the task itself, including the options you rejected and why: I will \
     not be there when it is picked up, and this conversation reaches nobody who is.";

/// The test the agent applies in `Auto`. Nothing in the app has read the text
/// of the task, so the judgement is the agent's, and the rule has to be sharp
/// enough to be applied by someone who has just read it once.
const JUDGE: &str =
    "Judge first. If this touches more than one place, or the wording admits more than one \
     reading, discuss it with me before creating anything. If it is a single obvious change, \
     just file it.";

/// The skills a harness cannot look up for itself, already read. All are
/// `None` for a `PluginDir` harness — it has the plugins loaded and is told
/// the skills by name — and any of them may be `None` for an `Inline` one when
/// the file could not be read, which is an ordinary outcome, not an error.
pub struct SkillText<'a> {
    /// The app's own filing-a-task skill.
    pub filing: Option<&'a str>,
    /// The app's own resolving-questions skill.
    pub resolving: Option<&'a str>,
    /// superpowers' brainstorming skill. Read only when the switch is `On`.
    pub brainstorming: Option<&'a str>,
    /// superpowers' writing-plans skill. Read only when the plan stage is
    /// `On` after the cascade — an `Auto` is handed the path instead, so a
    /// stage the agent may decline costs one line rather than 7 KB.
    pub plans: Option<&'a str>,
    /// The app's own reviewing-branch-changes skill. Read whenever a branch
    /// review is being started, for the reason `resolving` is: it is the whole
    /// of what the session was opened to do, so there is no branch in which it
    /// goes unread.
    pub reviewing_branch: Option<&'a str>,
}

/// The standard every filed task is held to, said in the prompt rather than
/// left to the filing skill — the same reasoning `DISCUSS` carries: an `Inline`
/// harness may find no skill text to attach, and this is the sentence the
/// feature turns on. A task filed thin is not a smaller task; it is a run
/// stopped overnight on a question nobody is awake to answer, or a task parked
/// unstarted.
///
/// `--validate` is named here and not only in the skill because it is the one
/// mechanical part of the standard: bd itself refuses a description missing the
/// sections its type requires, and prose can be skimmed where a refusal cannot.
const STANDARD: &str =
    "Whoever picks this up works alone and can ask nobody — file it so that it can be \
     carried out and checked off with no further question. Pass --validate to bd create: \
     it refuses a description missing the sections the type requires, and it is the only \
     check standing between a thin task and a run stuck on it at night.";

/// What a follow-up owes beyond an ordinary filing, and every sentence of it is
/// load-bearing.
///
/// The flag is stated exactly because the obvious spelling is wrong. bd's
/// `--deps` takes `type:id` pairs meaning "this issue *is* that type towards
/// that id", so a `blocks:<parent>` pair creates the edge backwards — the new
/// task blocks the parent, and bd hands the new task out as ready immediately.
/// The bare id is the correct form and was verified on the pinned sidecar.
///
/// The wrong form is named as a `type:id` pair rather than written out after
/// the flag, and a test pins that: a prompt is prose an agent skims, and the
/// exact wrong command line spelled out in it is a line that can be copied by
/// eye out of the sentence forbidding it.
///
/// The status is deliberately forbidden rather than left unsaid: the dependency
/// is the whole mechanism. bd hides a blocked issue from `bd ready` on its own
/// and releases it when the blocker closes, with nothing stored — and this
/// repository's board derives its Blocked column the same computed way. An
/// agent that "helpfully" set bd's stored `blocked` status would strand the
/// task for good, since bd never clears that itself.
///
/// The branch sentence is the one thing an implementer cannot work out from the
/// board. A parent is closed only after its work is fast-forwarded into a
/// target branch, so the work is somewhere — but a run is started by a person
/// who picks the branch by hand, and picking the wrong one is the failure this
/// whole feature was asked for.
const FOLLOW_UP: &str =
    "This is a follow-up to bd issue {id}: somebody has asked for further work on what that \
     task already did. Read it first — `bd show {id}` — and say in the new issue's own prose \
     what it refines and what is already done, so that whoever picks it up does not re-read \
     the argument from scratch.\n\n\
     File it as depending on that issue: pass `--deps {id}` to bd create — the bare id, and \
     **never** a `type:id` pair such as `blocks:{id}`, which creates the edge the other way \
     round and would hand the new task out as ready at once. Do not set any status on it: \
     the dependency is the \
     whole mechanism — bd keeps the issue out of `bd ready` while {id} is open and releases \
     it the moment {id} closes, with nothing stored and nothing to go stale.\n\n\
     Say in the acceptance criteria that the work must be cut from, and merged into, a branch \
     that already carries {id}'s changes. A run's target branch is chosen by hand when it is \
     started, and a follow-up merged into a branch without the original work in it is the \
     failure this task exists to avoid.";

/// `FOLLOW_UP` with the parent's id in it. The id appears seven times, which is
/// why this is a substitution rather than seven `write!` arguments.
fn follow_up(parent: &str) -> String {
    FOLLOW_UP.replace("{id}", parent)
}

/// The design document, when the person asked for one outright.
///
/// The path is superpowers' own layout moved under `.smetana/`, and that move
/// is the point: `.smetana/` is this app's folder in a project and
/// `runs::gitignore` keeps it out of the repository, so writing there commits
/// nothing and asks nobody to decide whether it should. It hangs off
/// the discussion and is unreachable without it — the dialog cannot offer this
/// unless Brainstorming is On, and `cascade` settles it the same way here.
const SPEC: &str =
    "Write the design the discussion produces to \
     .smetana/docs/superpowers/specs/YYYY-MM-DD-<topic>-design.md — today's date, and a short \
     slug of what this is about for the topic.";

/// Specifications and plans are English whatever either language setting says,
/// and this is the sentence that says so — once, at the end of whichever of the
/// four stage constants here were emitted, rather than written into each of
/// them and then repeated when two of them meet.
///
/// A design document and a plan are read by whoever picks the work up months
/// later and by every agent after them, and the repository they sit beside is
/// English throughout — so this is the one piece of writing `taskLanguage`
/// deliberately does not move. It names the conversation as well as the
/// setting, because by then the agent is talking to the person in their own
/// language and would otherwise carry that language into the file by mimicry.
/// It names a directory rather than a document, so that the same sentence is
/// true whether one file was asked for or both.
const IN_ENGLISH: &str =
    " Everything written under .smetana/docs/ is in English, whatever language we are talking in: \
     it is read by whoever picks this up months from now, and the repository it sits beside is \
     English throughout.";

/// The same stage on `Auto`: the test stated, the judgement left to the agent,
/// the way `JUDGE` does one level up.
///
/// It must not presume a conversation, and that is the whole shape of this
/// sentence. Brainstorming defaults to `Auto`, so the cascade makes this the
/// default spec position too — and `JUDGE` above it expressly allows "if it is
/// a single obvious change, just file it". In that branch nothing was settled
/// with anybody, and a condition opening "if what we settle" would point at a
/// discussion that never happened.
const SPEC_JUDGE: &str =
    "Whether or not we end up discussing this, decide whether it is worth a design document — \
     more than one moving part, or a decision somebody will want the reasoning for later. If it \
     is, write that design to .smetana/docs/superpowers/specs/YYYY-MM-DD-<topic>-design.md, with \
     today's date and a short slug of the topic. If it is small enough to say in the issue \
     itself, do not write one.";

/// The implementation plan, when it was asked for outright.
const PLAN: &str = "Then write the implementation plan to \
     .smetana/docs/plans/YYYY-MM-DD-<topic>.md, dated and named the same way.";

/// And on `Auto`, held to the same rule as `SPEC_JUDGE`: it asks about the
/// work, not about a document or a conversation that may not exist.
const PLAN_JUDGE: &str =
    "If the work is worth breaking into steps before anyone starts on it, write the \
     implementation plan to .smetana/docs/plans/YYYY-MM-DD-<topic>.md, dated and named the same \
     way. If it is not, do not write one.";

/// What every file written this way owes, said once for both of them.
///
/// The task is filed last so that a session interrupted halfway leaves no card
/// on the board promising documents nobody wrote. The paths are absolute for
/// the reason `IMAGES` gives about a screenshot, plus one of their own:
/// `.smetana/` is ignored, so an ignored file does not travel into the
/// worktree `smetana:provisioning` cuts, and a relative path resolves from
/// nowhere an implementer actually stands. And the issue still has to say what
/// was decided in prose, because the files are on one machine only.
const PAPERWORK: &str =
    "File the task last, once everything you are writing has been written, and copy the absolute \
     path of each file you wrote into the issue description. .smetana/ is not in the repository: \
     nothing in it is committed, do not try to commit it, and on any other machine those paths \
     lead nowhere — so the issue must also say in its own prose what was decided and what the \
     plan is, and stand on that alone.";

/// What a resolving session is for, said in the prompt rather than left to the
/// skill — the same reasoning `STANDARD` and `DISCUSS` carry: an `Inline`
/// harness may find no skill text to attach, and this is the whole of what the
/// session is being started to do.
///
/// The three parts that must survive a missing skill are here and nowhere else.
/// Where the questions are, because an agent hunting for them in the
/// description would answer the wrong thing. One at a time and never on the
/// person's behalf, because the entire reason the task is parked is that
/// guessing was not good enough for the agent that gave up on it. And the order
/// at the end: the status is the last write, so a session interrupted halfway
/// leaves the task parked rather than back in the queue with the answer
/// nowhere — the same rule `PAPERWORK` keeps for filing.
const RESOLVE: &str =
    "This task is parked: an agent working it could not settle something on its own and left \
     the questions in the issue's notes, one to a line, each starting `parked:`. Read the issue \
     first, then put those questions to me — one at a time, in your own words, with whatever \
     context from the issue I need to answer them. Answer none of them yourself and assume \
     nothing: the task is parked precisely because guessing was not good enough. If my answer \
     opens a further question, ask that too.";

/// What the answers owe the issue, and it is two places rather than one.
///
/// The description, because that is the spec: `smetana:provisioning` reads it
/// to decide whether the task can be started at all, and a decision recorded
/// only in the notes is a decision the implementer never sees. The notes,
/// because a `parked:` line with no answer beside it reads as a question still
/// open — to the next person scanning them, and to this app, which reads
/// exactly that pairing when it asks whether moving a task to Ready needs a
/// warning.
const RESOLVE_WRITE: &str =
    "When every question has an answer, write the outcome into the issue itself. Fold each \
     decision into the description, which is the spec whoever picks this up works from, and make \
     sure the acceptance criteria now say what done looks like — if an answer settled that, this \
     is where it goes. Then add one note per question, `resolved: <the answer, in one line>`, so \
     no `parked:` line is left looking unanswered. Only then set the status: \
     `bd update <id> --status open`. That write is last, so a session interrupted halfway leaves \
     the task parked rather than back in the queue with the answer written nowhere.";

/// The one ending that is not a resolution, said out loud because the obvious
/// failure here is a session that unparks a task to have finished something.
/// A task left parked has cost nothing; a task in the queue with its question
/// still open is the very state the app now interrupts a person to prevent.
const RESOLVE_GIVE_UP: &str =
    "If I cannot answer, or we run out of things to say, leave the task parked and change \
     nothing about its status. Say what is still open. A task left parked costs nothing; one put \
     back in the queue with the question still open costs the next agent the same night.";

/// What an edit session is for, and it has to stand on its own.
///
/// This prompt used to stop mid-sentence — `Update bd issue smetana-7 ("x y"): `
/// — on the theory that the agent is being told what to work on and only the
/// person knows the second half, so the person would type it. They never got
/// the chance. A prompt rides as the agent's **positional argument**
/// (`terminal/pty.rs` says why), and both harnesses this app runs submit that
/// argument as the session's first message rather than leaving it in the
/// composer to be finished — so what actually arrived was an instruction cut
/// off at a colon, and the session's first move was to ask whether the message
/// had been truncated. The unfinished half was unreachable by construction, not
/// merely unfilled.
///
/// So the sentence is completed here, and completed by *asking* rather than by
/// guessing. Both other endings cost more: an agent that decides for itself
/// what to change rewrites an issue nobody asked it to touch, and an agent that
/// reports the prompt as broken has spent a whole session saying so. Asking is
/// also what the person is there for — this intent is started from a card's own
/// menu, by somebody sitting at the terminal.
///
/// Nothing else is imposed. An edit is an update, so the filing standard, the
/// paperwork rules and the unpark write all stay out of it, and the tests below
/// pin that each of them does.
const EDIT: &str =
    "Read the issue first, then ask me what to change about it, one question at a time. Nothing \
     outside this prompt says what the change is: do not guess at it, do not decide it yourself, \
     and change nothing about the issue until I have answered.";

/// What a fix session is for. It opens after the issue's id and title, the way
/// `EDIT` does, and there is deliberately no skill in the library behind it:
/// what is wanted is narrow enough to say here, which is the same call
/// `CONFLICT` makes.
///
/// Four things, and each is load-bearing. **Where the code is:** a done task's
/// worktree may well be gone — whether a run removes one is a setting — so the
/// work to read is the project's own tree, and an agent told nothing would go
/// hunting for a branch. **Asking rather than guessing**, for the reason `EDIT`
/// carries: the prompt rides as the session's first message, so there is no
/// second half anybody types. **Finishing the job:** a correction left
/// uncommitted in the working tree is one the next person finds by accident,
/// and a closed issue with no note says nothing about having been reworked.
/// **Handing back what is too big:** the whole point of the row this comes from
/// is the fix too small to file, and Follow-up task is one row below it in the
/// same menu.
///
/// The note it asks for carries no marker. `parked:` and `resolved:` are read
/// by the app to tell an open question from an answered one
/// (`components/kanban/parked.js`), and this note is neither.
const FIX: &str =
    "is closed and its work is already merged, and it may not actually be finished. Read the \
     issue and the work behind it first, then ask me what is wrong with it, one question at a \
     time. Nothing outside this prompt says what the fix is: do not guess at it, and change \
     nothing until I have answered. Then make the correction, commit it, and leave a note on the \
     issue saying what was put right. The issue stays closed — this is a correction to work \
     already done rather than a reopening; if what I ask for turns out to be a piece of work in \
     its own right, say so and we file it as a task instead.";

/// What a conflict session is for, and the whole of it: there is no skill in
/// the library for this and deliberately none added.
///
/// `smetana:merging` is the neighbouring process and is the wrong one — it is
/// about a *task's* worktrees, its gates and its fast-forward into a target
/// branch, and naming it here would start an agent on a process nobody asked
/// for, in a repository that has none of that around it. What is wanted is
/// narrow enough to say in the prompt, which is what `prompt.rs` is for.
const CONFLICT: &str =
    "Git has stopped part-way and left the working tree conflicted, with its own markers in the \
     files below. That tree is exactly as git left it: nothing has been committed and nothing has \
     been cleaned up for you. Read both sides of each conflict before you choose, and keep what \
     both branches were doing rather than taking one side wholesale — if a hunk is genuinely a \
     question rather than a mechanical resolution, ask me about that one.";

/// The second half, and the one the door exists for.
///
/// A person chose this over the abort button that was standing next to it, so
/// an agent that "tidies up" by aborting has undone the only thing it was asked
/// to do — and the panel then shows a clean tree, as though nothing had ever
/// happened, which is the worst way for this to fail because it looks like
/// success. `--abort` is named as the thing not to do rather than merely left
/// unmentioned: it is the first thing every guide about a conflict offers, and
/// a test in this file holds the sentence to naming it.
const CONFLICT_FINISH: &str =
    "Then finish what git started: stage what you resolved and complete the operation, so that the \
     work of both branches ends up in the tree. Do not abort it — no `--abort` of any kind, and no \
     reset that throws the operation away. I chose to have this resolved instead of pressing the \
     abort button that was beside it, and a tree quietly put back is the one outcome that looks \
     like success and is not. If you truly cannot finish, stop and say so in this conversation, \
     leaving the conflict exactly where it is for me to look at.";

/// What a session opens on when the app's own repair was not enough.
///
/// No skill is named, the way none is named for a conflict, and for the same
/// reason: there is no process in the library for this and inventing one would
/// start an agent on somebody else's checklist. What the prompt does carry is
/// the two things the app measured and the agent cannot — that `bd doctor` is
/// unavailable here, so there is no structured verdict to fetch, and that the
/// two migrations have already been run, so repeating them is not the answer.
///
/// The copy is named as a thing to leave alone rather than left unmentioned:
/// `.beads.backup-<UTC>` beside the tracker is the person's only way back, and
/// "tidying up" a stray-looking directory is exactly the helpful act that would
/// take it away.
const REPAIR: &str =
    "Smetana has already taken a copy of .beads beside it, as .beads.backup-<UTC>, and has \
     already run `bd migrate` and `bd migrate schema` against the tracker. Neither was enough. \
     Do not delete or move that copy — it is the only way back. Two things are worth knowing \
     before you start: `bd doctor` is not supported for this database, which is embedded, so \
     there is no structured verdict to fetch; and `bd migrate` ignores `--json`, so anything it \
     tells you is prose meant for a person. Work out what is actually wrong, tell me what you \
     find before you change anything, and do not touch the issues themselves.";

/// What the agent is told to produce when a project has no configuration yet.
/// The file's path is named here rather than left to the skill: a session that
/// could not read the skill must still write to the right place.
const SETUP: &str = "Work out what this project is made of and write .smetana/project.toml — \
     the file Smetana reads before it runs anything here. Check the commands before you write \
     them in, and ask me about anything the folder does not answer.";

/// What a branch review is, said in the prompt rather than left to the skill,
/// and the reason is `SETUP`'s: an `Inline` harness may find no skill text at
/// all, and a session that could not read one must still know what it was
/// started for and what it is not allowed to do.
///
/// The two prohibitions are the sentence that earns this constant, and the
/// reason is what this session is doing rather than what the other intents are:
/// every other intent that reads code either files an issue or makes a commit,
/// and a model that has just read a diff is one step from "I will just fix
/// this". So the boundary is stated here, in front of the work, rather than
/// only in a skill that may never be delivered.
const REVIEW_BRANCH: &str =
    "Review what one branch adds to another and write the result up for somebody to read \
     afterwards. Nothing is being merged here and there is no task behind it: the report is \
     the whole of what this session produces. File nothing in the tracker, run no bd command \
     that writes, and make no commit — the only thing you write is the report itself.";

/// The language the agent talks to the person in, and it goes into every
/// intent — `Bare` included, which is why that one no longer opens on nothing.
///
/// That follows from an English default with no Auto position: an Auto would
/// mean "say nothing about language", which is today's behaviour exactly, so
/// the setting would do nothing for anybody until they changed it. The price is
/// visible and was taken deliberately — "+ New agent" opens having submitted
/// the language paragraphs, all three of them since `writes_to_the_tracker`
/// took `Bare` in — because the alternative is that the one session where a
/// person talks to the agent most is the one session the setting cannot reach.
fn conversation(language: &str) -> String {
    format!(
        "Talk to me in {language}: everything you say to me in this session, whatever language \
         the code, the files and the tracker in front of you happen to be written in."
    )
}

/// The language the prose of a bd issue is written in, and it goes where the
/// agent may write into bd rather than only where filing is the work: `Bare` is
/// in, because a person in that session asks for a task to be filed as readily
/// as for anything else, and that shift is the whole of what
/// `writes_to_the_tracker` decides.
///
/// The caveat is not optional and is why this is prose rather than one clause.
/// What this setting moves is prose; what it must not move is any string
/// another piece of software matches on, and an issue carries two families of
/// those.
///
/// The `##` section headings, because `bd create --validate` matches the
/// wording of a heading and nothing else — a translated `## Acceptance
/// Criteria` is not a stylistic difference, it is bd refusing to create the
/// issue.
///
/// And the notes' markers, because this app reads them itself. `parked:` is
/// written by `runs::queue::parking_note` and by `smetana:running-tasks` when a
/// lead parks by hand, `resolved:` by a resolving session, and
/// `components/kanban/parked.js` matches both — `/^\s*parked:\s*(.+)$/i` and
/// `/^\s*resolved:\s*/i`. Case and leading space are free there; the word and
/// its colon are not. A translated marker fails silently and in the worst
/// place: `openQuestions` returns nothing, so the parked card's "Answer
/// questions" dialog says there is nothing open, and moving that card to Ready
/// stops warning about the question that parked it. This paragraph is what
/// opens that hole — it is the sentence asking for the notes in another
/// language — so it is the sentence that has to close it.
fn task_language(language: &str) -> String {
    format!(
        "Write the prose of any bd issue you create or change in {language}: the title, the body \
         of the description, the acceptance criteria themselves, the notes. Two things stay in \
         English, because they are matched as literal strings rather than read. The `##` section \
         headings, exactly as they are written today — ## Acceptance Criteria, ## Steps to \
         Reproduce, ## Success Criteria, ## Decision, ## Rationale, ## Alternatives Considered: \
         `bd create --validate` matches the wording of a heading and nothing else, so a \
         translated heading is not a difference of style, it is bd refusing the issue. And the \
         markers a note begins with — a note still opens `parked:` or `resolved:` in English, and \
         only what follows the colon is written in {language}, because the app reads those two \
         words to tell an open question from an answered one."
    )
}

/// Whether this session writes into the tracker, which is the whole of what
/// `taskLanguage` is about.
///
/// Four of the five run `bd create` or `bd update` as the work they were
/// opened for. `Bare` is the fifth, and it is in for the reason
/// `commits_to_git` gives for having it: the "+ New agent" session is exactly
/// where a person says "file tasks for this", and a setting that missed it
/// would miss the place it is used most. That is not hypothetical — with
/// `taskLanguage` set to Russian, a bare session split one afternoon's work
/// into five issues and wrote every one of them in English, because nothing in
/// its prompt had said otherwise.
///
/// **The price of that is three paragraphs, and it is paid knowingly.** `Bare`
/// and `Run` are the two intents in which the conversation, the issues and the
/// commits are all three true at once — a lead commits and files all night, and
/// a person in a bare session may ask for anything — so the bare session now
/// opens on those three before any work, which is the shape a run has opened on
/// all along rather than a new one. `Run` takes a fourth on top of them, the
/// language of the report it leaves behind (`leaves_a_run_report`), and a bare
/// session does not: it writes no batch file. `commits_to_git`
/// warns against exactly that shape, and the warning still holds where it was
/// aimed: handing a paragraph to every intent would open a filing session with
/// three of them in front of work that makes no commit. Here it is the other
/// way round, and three short paragraphs in the one session that can do all
/// three things is the smaller evil than a setting that does not reach the
/// place it is used from.
///
/// `Setup`, `ResolveConflict` and `RepairTracker` stay out. A setup session
/// writes one toml file, a conflict session finishes a merge or a rebase git
/// stopped on, a repair session is looking at the tracker's own database —
/// none of the three files an issue, and the last of them could not if it
/// wanted to, since bd is what is broken. Telling any of them how to word one
/// would be prose about something that is not going to happen.
///
/// `ReviewBranch` stays out too, and it is the one that is **written down**
/// rather than left to the fall-through — which is why this is a `match` and
/// no longer a `matches!`. The other four are quiet about it because nothing
/// else in the app claims otherwise; a review's prompt says in its first
/// paragraph that it files nothing, and a reader checking that claim against
/// this function has to find it answered rather than absent.
///
/// `FixTask` is in for one sentence of its prompt: it leaves a note on the
/// issue saying what was put right, and a note is prose somebody reads.
fn writes_to_the_tracker(intent: &Intent) -> bool {
    match intent {
        Intent::NewTask { .. }
        | Intent::EditTask { .. }
        | Intent::ResolveTask { .. }
        | Intent::FixTask { .. }
        | Intent::Run { .. }
        | Intent::Bare => true,
        // Written out rather than left to the fall-through every other `false`
        // here comes from. A branch review is the one intent whose own prompt
        // promises it files nothing, and a promise made in one file and kept by
        // an absence in another is a promise the next reader cannot check.
        Intent::ReviewBranch { .. } => false,
        _ => false,
    }
}

/// The language a git commit message is written in, and it goes only where the
/// agent's own hands reach git.
///
/// The caveat is the same watershed `task_language` holds one field over, and
/// it is why this is prose rather than one clause. What the setting moves is
/// the sentence a person reads; what it must not move is anything another
/// reader matches on — and in a commit message that is what sits in front of
/// the colon, whatever a given project happens to write there.
///
/// **It names no form, and that is the whole of the fix this paragraph got.**
/// It said `type: subject` with the six Conventional Commits types, which is a
/// convention this app has no business imposing on somebody's repository — the
/// session prompt said nothing at all about commit form before this setting
/// existed, and two things paid for it. `smetana:merging` hands the agent a
/// literal `git merge --no-ff … -m "merge: <branch> into <target>"`, and
/// `merge` is not one of the six, so an agent reconciling the two would rewrite
/// a subject that `smetana:provisioning` then greps for by branch name. And
/// this repository's own commit subjects are Russian words in front of the
/// colon — a convention the paragraph would have quietly pushed into English.
/// So the sentence protects whatever is there rather than saying what should
/// be: exactly the shape `task_language` holds for the `##` headings and the
/// `parked:` and `resolved:` markers. `oneshot::commit_prompt` still names the
/// six, and the difference is who is writing — there the app composes the whole
/// message itself, so the form is its own to choose.
///
/// An identifier inside the message is exempted for the same reason one clause
/// later, since a branch name or an issue id after the colon is a name rather
/// than prose, and the greps that find a merge look for exactly those.
///
/// The messages git writes by itself — a `--no-ff` merge, a revert — are named
/// as outside this, because they are git's own text and an agent that took the
/// instruction literally would start rewriting them by hand.
fn commit_language(language: &str) -> String {
    format!(
        "Write the message of any git commit you make in {language}: the subject, and the body \
         under it where you write one. Whatever form this project's commit subjects already \
         take, what sits in front of the colon stays as it is — it is matched and read rather \
         than translated, and a history people grep has to go on answering. An identifier in the \
         message, such as a branch name or an issue id, is a name and travels unchanged for the \
         same reason. Only the prose is written in {language}. A message git writes for you, \
         such as a merge or a revert, is git's own and is left alone."
    )
}

/// Whether this session has any business making a commit, which is the whole of
/// what `commitLanguage` is about — `writes_to_the_tracker`'s shape one field
/// over, and deliberately not the same list.
///
/// `Run` is the obvious one: a lead commits and merges with its own hands for a
/// whole night. `ResolveConflict` finishes a merge or a rebase git stopped, and
/// finishing one is a commit. `Bare` is in for the reason the conversation
/// language is in every intent — the "+ New agent" session is exactly where a
/// person says "commit this", and a setting that missed it would miss the case
/// it was asked for. `FixTask` is the one about an issue that is nonetheless
/// here: it corrects the code behind a closed task rather than the task's own
/// prose, and its prompt asks for that correction to be committed — which is
/// the whole difference between it and the `EditTask` in the paragraph below.
///
/// The rest are out because they do not touch a repository at all — the
/// `match` below is the list, and a number written here would be wrong the
/// next time somebody adds an intent, which is how this comment came to say
/// "the other four" over five of them. `ReviewBranch` is the one of them
/// spelled out instead of falling through, for the reason
/// `writes_to_the_tracker` gives: it reads a diff and its prompt forbids it a
/// commit outright, and that is the intent somebody will come here to check.
/// `NewTask`, `EditTask` and `ResolveTask`
/// write into bd, and what `NewTask` puts on disk goes under `.smetana/`, which
/// `runs::gitignore` keeps out of the repository. `Setup` writes one toml file
/// in the same folder. `RepairTracker` works on `.beads`, which bd owns and
/// commits for itself. Putting the paragraph in every intent instead would open
/// a filing session with three paragraphs about language in front of the work.
fn commits_to_git(intent: &Intent) -> bool {
    match intent {
        Intent::Run { .. }
        | Intent::ResolveConflict { .. }
        | Intent::FixTask { .. }
        | Intent::Bare => true,
        // Named for the reason it is named one function up: the review prompt
        // forbids a commit in so many words, and the predicate behind that
        // sentence has to say the same thing where somebody would look for it.
        Intent::ReviewBranch { .. } => false,
        _ => false,
    }
}

/// The language a run's report is written in, and it moves the prose of the
/// batch file and nothing else in the document.
///
/// The same watershed the two paragraphs above hold, and here it is sharper
/// than in either of them, because on the far side of it sits a program rather
/// than a person's eye. `runs::report::parse_batch` reads `tasks`, `id`, `did`
/// and `notes` through serde by literal match, so a translated key is not a
/// document in another language — it is a batch that left no account of itself,
/// drawn in the report as exactly that. Hence the field names first: a model
/// that reads the sentence and stops has to have met the half that breaks the
/// document.
///
/// An identifier is exempted for the reason it is one paragraph up — a path or
/// a sha inside a `did` line is read rather than translated, and `report::prose`
/// draws it as `<code>`.
///
/// The last clause is not a nicety. There are two reports at the end of a
/// batch: this file, which one program reads to draw a document, and the
/// account the lead gives back in the conversation, which is under the
/// conversation language and stays there. Somebody who set this and watched the
/// terminal for a change would have been told nothing at all, so the paragraph
/// says which of the two it is about.
///
/// `report.rs`'s own words — `smetana · run report`, `closed`, `parked`,
/// `batch N` — are not mentioned here at all, and deliberately: they are this
/// product's interface copy, which CLAUDE.md says is English, and no agent
/// writes them. Nothing in a prompt could move them even if it tried.
fn report_language(language: &str) -> String {
    format!(
        "Write the prose of the batch file you leave when a batch is finished in {language}: the \
         `did` line for each task and the batch's `notes`. The names of the fields are not prose \
         and do not move — `tasks`, `id`, `did` and `notes` stay exactly those four words, \
         because Smetana matches them letter for letter, and a renamed key is a batch that left \
         no account of itself. An identifier inside a line — a path, a symbol, a command, a sha \
         — is read rather than translated and travels unchanged for the same reason. The account \
         you give back in this conversation is a separate report and keeps the language of the \
         conversation: this setting moves the file on disk and nothing you say to me."
    )
}

/// Whether this session ever writes a batch file, which is the whole of what
/// `reportLanguage` is about — the shape `writes_to_the_tracker` and
/// `commits_to_git` hold, and the narrowest of them.
///
/// `Run` alone. It is the only intent that names the file at all, and a session
/// that will never write one has nothing to hear about how to word it: the
/// paragraph would be prose about something that is not going to happen, in
/// front of work that is.
fn leaves_a_run_report(intent: &Intent) -> bool {
    matches!(intent, Intent::Run { .. })
}

/// The line that says whose words come next.
///
/// The person's text is not pasted bare, and the sentence is doing work rather
/// than decorating: everything else in a prompt is this app asking for
/// something, so an unannounced paragraph would read as one more thing the app
/// wants done — and an instruction like "answer briefly" read as a task is a
/// session that answers briefly and does nothing else.
const STANDING: &str =
    "What follows is a standing instruction from the person you are working with. It holds for \
     this whole session, whatever the work below turns out to be:";

/// Whether somebody is in this session to be talked to, which is the whole of
/// what `agentPrompt` is about.
///
/// A negation rather than a list of the conversations, and deliberately. The
/// three predicates above each name a capability a session *has* — it writes
/// into bd, it commits, it leaves a report — and a positive list is the honest
/// shape for those. This one names the **absence of a listener**, so a list of
/// them would be the complement of the rule rather than the rule, and a reader
/// would have to work out what they had in common. No count is written here,
/// for the reason `commits_to_git` gives above: a number is wrong the next time
/// an intent is added and nothing fails when it goes stale — this paragraph
/// had come to say "the eight" over nine of them.
///
/// The second reason is the one that matters in a year. A variant added to
/// `Intent` later is, on the evidence of every variant there is, another
/// conversation, and a negation hands it the person's instruction for free.
/// That is the right default here: an instruction reaching one more
/// conversation is benign, and missing one is the bug this field exists to fix.
/// A positive list would leave the new variant out silently, the same quiet
/// drift `.claude/rules/agents.md` records about `RESUMES_BY_ID`.
///
/// `Intent::Run` is the one exclusion. Nobody is in a run's conversation — the
/// lead works overnight against a queue — so an instruction written for a
/// conversation would shape autonomous work with no one there to correct it.
///
/// `ResumeSession` is deliberately **not** named here, and this is the sentence
/// that stops the next reader "fixing" that. It never reaches this function:
/// `build` refuses it a prompt on its first line, because a resumed
/// conversation already has somebody's words in it. A clause for it would be
/// dead code wearing the clothes of a decision.
fn talks_to_a_person(intent: &Intent) -> bool {
    !matches!(intent, Intent::Run { .. })
}

/// What the session opens on. `None` for exactly one intent, and it is the
/// conversation language that makes every other one `Some`: that sentence is
/// said in all of them, so even the "+ New agent" row opens on one paragraph.
///
/// The exception is `ResumeSession`, and it is the reason the `Option` was
/// worth keeping. A prompt rides as the agent's positional argument and both
/// harnesses **submit** it as the session's first message rather than leaving
/// it in the composer — which is right for a session that is starting and wrong
/// for one that is being picked up. A resumed conversation already has
/// somebody's words in it, and a paragraph about which language to speak,
/// pushed into it by the app the moment it opens, would be this app talking
/// over the person whose session it is. Whatever was settled in there was
/// settled before this window existed.
#[allow(clippy::too_many_arguments)]
pub fn build(
    intent: &Intent,
    delivery: SkillDelivery,
    images: ImageDelivery,
    skills: &Skills,
    facts: Option<&str>,
    text: SkillText,
    languages: &Languages,
    // The person's own standing instruction, or empty. Read from
    // `settings.json` by the caller, for the reason `languages` is: this
    // function stays pure and the disk stays outside it.
    agent_prompt: &str,
) -> Option<String> {
    // Nothing at all for a resumed session, before any of the paragraphs below
    // are composed: see this function's own doc for why a prompt is the one
    // thing that must not reach it.
    if matches!(intent, Intent::ResumeSession { .. }) {
        return None;
    }
    // The language rules come first, before the work rather than after it, for
    // the reason `stages` gives about a skill body: what is said last can be
    // pushed off the top of what the agent reads first by 7 KB of process, and
    // these paragraphs are short enough to cost nothing at the front. There are
    // four of them and one intent takes every one: `Run`, which commits, files
    // and leaves a report behind it. `Bare` takes three of the four — the
    // conversation is said in every intent, and a person at "+ New agent" can
    // ask for a commit or for a task as readily as for anything else, but a
    // bare session writes no batch file. The rest get one or two.
    let mut out = conversation(crate::agents::language_name(&languages.agent));
    if writes_to_the_tracker(intent) {
        out.push_str("\n\n");
        out.push_str(&task_language(crate::agents::language_name(&languages.task)));
    }
    if commits_to_git(intent) {
        out.push_str("\n\n");
        out.push_str(&commit_language(crate::agents::language_name(&languages.commit)));
    }
    if leaves_a_run_report(intent) {
        out.push_str("\n\n");
        out.push_str(&report_language(crate::agents::language_name(&languages.report)));
    }
    // After the four language paragraphs and before the work, which is a
    // decision rather than an order that fell out. Near the front for the
    // reason the languages are: what is said last can be pushed off the top of
    // what the agent reads first by seven kilobytes of skill text. After them
    // rather than before because those paragraphs close silent failures — a
    // translated `## Acceptance Criteria` is bd refusing the issue — and a
    // reader resolves a contradiction in favour of what came later, so a person
    // who deliberately writes across a language setting gets what they wrote
    // and everybody else costs the language rules nothing.
    //
    // Trimmed rather than taken as it stands, and the guard is on the trimmed
    // text rather than on the raw string. A lone newline or a space is what a
    // person clearing this field leaves behind as readily as an empty string,
    // and `!is_empty()` would let it through: the prompt would then announce a
    // standing instruction and follow the announcement with nothing, leaving
    // `STANDING`'s colon dangling — the one shape `no_prompt_stops_mid_sentence`
    // exists to keep out of a prompt — and, for a newline, putting back the
    // `\n\n\n` the empty case is tested against. What is stored stays
    // untouched: the words are the person's, and only what this function pastes
    // into a prompt is tidied.
    let agent_prompt = agent_prompt.trim();
    if !agent_prompt.is_empty() && talks_to_a_person(intent) {
        out.push_str("\n\n");
        out.push_str(STANDING);
        out.push_str("\n\n");
        out.push_str(agent_prompt);
    }
    if let Some(body) = body(intent, delivery, images, skills, facts, text) {
        out.push_str("\n\n");
        out.push_str(&body);
    }
    Some(out)
}

/// The work itself, with nothing about language in it. `None` is the bare
/// session: a person with their own reason, and nothing to impose on them
/// beyond what `build` puts in front of this.
#[allow(clippy::too_many_arguments)]
fn body(
    intent: &Intent,
    delivery: SkillDelivery,
    images: ImageDelivery,
    skills: &Skills,
    facts: Option<&str>,
    text: SkillText,
) -> Option<String> {
    let brainstorming = skills.superpowers.join("skills/brainstorming");
    let plans = skills.superpowers.join("skills/writing-plans");
    match intent {
        Intent::Bare => None,
        Intent::EditTask { id, title } => {
            Some(format!("Update bd issue {id} (\"{title}\"). {EDIT}"))
        }
        Intent::ResolveTask { id, title } => {
            Some(resolve_task(id, title, delivery, skills, text.resolving))
        }
        Intent::FixTask { id, title } => Some(format!("Issue {id} (\"{title}\") {FIX}")),
        Intent::ResolveConflict { repo, op, ours, theirs, files } => {
            Some(resolve_conflict(repo, *op, ours, theirs, files))
        }
        Intent::NewTask { brainstorm, spec, plan, draft } => {
            // The cascade first, and nothing below reads the raw two: a
            // payload can carry a spec chosen under a discussion that has
            // since been switched off, and prose about a document nobody
            // asked for is worse than none.
            let (spec, plan) = cascade(*brainstorm, *spec, *plan);
            Some(new_task(
                *brainstorm,
                spec,
                plan,
                draft,
                delivery,
                images,
                &brainstorming,
                &plans,
                text,
            ))
        }
        Intent::RepairTracker { dir, bd_version, command, stderr } => {
            Some(repair_tracker(dir, bd_version, command, stderr))
        }
        // Unreachable through `build`, which refuses this intent a prompt
        // before it composes anything; stated all the same, because the match
        // is exhaustive and the next variant added to `Intent` has to meet a
        // decision rather than a wildcard.
        Intent::ResumeSession { .. } => None,
        Intent::Setup => Some(setup(delivery, skills, facts)),
        Intent::ReviewBranch { pairs, report } => {
            Some(review_branch(pairs, report, delivery, skills, text.reviewing_branch))
        }
        Intent::Run { settings, reports, batch, remove_worktrees } => {
            Some(run(settings, reports, *batch, *remove_worktrees, delivery, skills))
        }
    }
}

/// What one batch of a run opens on.
///
/// Everything variable is stated here, and everything fixed is left to the
/// skill: the settings are this run's and are not in any file, while how to
/// carry work through is the same every time and is 300 lines nobody should
/// pay for in a prompt. The rest — repositories, gates, hazards — the skill
/// reads out of `.smetana/project.toml` itself, which is also what keeps a
/// batch reading the config as it is now rather than as it was when the run
/// started.
fn run(
    settings: &RunSettings,
    reports: &Path,
    batch: u32,
    remove_worktrees: bool,
    delivery: SkillDelivery,
    skills: &Skills,
) -> String {
    let mut out = String::from("Work this project's bd tracker. ");

    match &settings.scope {
        RunScope::Queue => out.push_str("Take ready tasks from the board."),
        RunScope::Task { id } => {
            let _ = write!(out, "Work only on issue {id}, and nothing else.");
        }
        /* "the children of", not "the children of epic": bd's parent-child is
           the relation, and the parent's own type has nothing to do with it —
           a `feature` with children is how this very tracker is written. */
        RunScope::Epic { id } => {
            let _ = write!(out, "Work only on the children of {id}, and nothing else.");
        }
    }

    out.push_str("\n\nThis run:\n");
    // "wherever it does not exist yet" and not "it does not exist yet": a
    // project of several repositories can carry the branch in some of them, and
    // the dialog's answer is a snapshot taken when it opened, hours before the
    // fifth batch. What travels is permission; `provisioning` asks each
    // repository at the moment it cuts, which is the only place and time the
    // question has a current answer.
    let _ = writeln!(
        out,
        "- merge finished work into `{}`{}",
        settings.target_branch,
        if settings.create_target {
            " — cut it from the repository's own current branch wherever it does not exist yet"
        } else {
            ""
        }
    );
    // Only where there is something to choose between, which is the queue and
    // nothing else — `RunSettings::validate` is what makes it `None` elsewhere.
    // Beside "Work only on issue X, and nothing else" a floor is a second
    // instruction contradicting the first.
    if let Some(floor) = settings.min_priority {
        let _ = writeln!(out, "- take nothing worse than priority P{floor} automatically");
    }
    // Said out loud as this run's number and as beating the file, because the
    // skill reads `[defaults].max_parallel_tasks` on its own and would
    // otherwise treat anything higher than the file's number as a mistake. It
    // is absent in Solo, where there is nobody to spawn — `RunSettings::validate`
    // is what makes it `None` there.
    if let Some(agents) = settings.max_parallel_tasks {
        let _ = writeln!(
            out,
            "- work on at most {agents} task{} at once — this run's number, and it wins over \
             `[defaults].max_parallel_tasks` in the config, upwards as well as down",
            if agents == 1 { "" } else { "s" }
        );
    }
    let _ = writeln!(
        out,
        "- {}",
        match settings.mode {
            RunMode::Auto =>
                "you are on your own — there is no one to ask. Park anything you cannot resolve, \
                 note why, and carry on with the rest",
            RunMode::Supervised =>
                "ask me when something genuinely needs a decision, and keep going otherwise",
            RunMode::Solo =>
                "do the work yourself rather than delegating it, and ask me freely",
        }
    );
    let _ = writeln!(
        out,
        "- {}",
        if settings.live_check {
            "verify each merged task for real before closing it"
        } else {
            "close a task on a green merge; there is no live check this run"
        }
    );
    let _ = writeln!(
        out,
        "- {}",
        if settings.file_findings {
            "findings that are out of scope may be filed as `deferred`, within the budget"
        } else {
            "file nothing new: every out-of-scope finding goes to the digest and nowhere else"
        }
    );
    // Both branches written out, as the two switches above are, and for the
    // same reason: silence would be read as the default, and the default is not
    // what somebody who has just been to the settings window chose.
    //
    // The Off branch asks to be told about it in the report, which is not
    // decoration. Nothing in this app runs `git worktree` or counts one, so a
    // person who left the switch off and forgot hears about the disk from the
    // disk rather than from the app.
    let _ = writeln!(
        out,
        "- {}",
        if remove_worktrees {
            "remove each task's worktree once it is merged and closed"
        } else {
            "leave every worktree where it is — never remove one — and say in your report that \
             they were kept"
        }
    );

    out.push('\n');
    match delivery {
        SkillDelivery::PluginDir => out.push_str(
            "Follow the smetana:running-tasks skill — it is the process, and it names the \
             others it needs.",
        ),
        SkillDelivery::Inline => {
            let _ = write!(
                out,
                "The process is at {} — read it first, and read the skills it names beside it.",
                skills.smetana.join("skills/running-tasks/SKILL.md").display()
            );
        }
    }

    // The one fact about this batch no skill can carry, because it names a path
    // that exists for this run and this batch alone. The app can see the board
    // and its own clock and nothing else — what a session *did* comes back from
    // it as an exit code and nothing more — so the account is asked for here.
    //
    // The backticks are asked for rather than inferred: `report.rs::prose`
    // marks a backtick span and nothing else, because a shape heuristic's
    // false positives land in ordinary prose and cannot be reviewed.
    let _ = write!(
        out,
        "\n\nWhen this batch is finished, write your own account of it to {} — the directory \
         already exists. Smetana reads that file and nobody else does, so it is JSON in exactly \
         this shape: {{\"tasks\": [{{\"id\": \"<bd id>\", \"did\": \"one or two sentences on what \
         you actually did, with every path, symbol, command and sha in backticks\"}}], \
         \"notes\": \"anything about the batch as a whole, or leave it out\"}}. Put a line in \
         it for every task you touched, the ones you parked included, saying what stopped \
         them. This is in addition to the report you hand back in this conversation and \
         replaces no part of it.",
        reports.join(format!("batch-{batch}.json")).display()
    );

    // What the file *is* differs by mode, and the difference is not a nuance.
    //
    // An unattended batch is told to exit, so the app learns the work is over
    // from the process and the file is a record: the report's skeleton is built
    // from the board whatever happens, and a batch that leaves none is named in
    // the document rather than drawn as an empty row.
    //
    // The other two modes keep the session alive afterwards, because a person is
    // in it — so nothing exits, and this file is the only thing that says the
    // work is done. Telling a lead there it may shrug the file off would be
    // false, and the cost of the shrug is a run that hangs with nothing on
    // screen explaining why. Hence a way out that is a sentence a person can
    // read, rather than a silence they would have to guess at.
    out.push_str(if settings.mode.unattended() {
        " If the file cannot be written, carry on regardless — it is a record, not a gate."
    } else {
        " Writing it is how you hand the work back: this session stays open afterwards so that \
          we can keep talking, so the file is the only thing that tells Smetana the work is \
          done. Write it as soon as the work is finished, before anything else you might say. \
          If it cannot be written, say so in this conversation, so that somebody can end the \
          run by hand."
    });
    out
}

/// What a session opens on when it is sent to unpark a task.
///
/// The issue is named and nothing about it is quoted: the agent reads it with
/// bd, and prose copied in here would be the board as it stood when a menu was
/// opened. The skill carries the depth — what makes an answer worth writing
/// down, where in a description it belongs — and the three constants above
/// carry what has to survive a skill that cannot be read at all.
fn resolve_task(
    id: &str,
    title: &str,
    delivery: SkillDelivery,
    skills: &Skills,
    resolving: Option<&str>,
) -> String {
    let mut out = String::new();
    let _ = write!(out, "Resolve bd issue {id} (\"{title}\").\n\n");
    out.push_str(RESOLVE);
    out.push_str("\n\n");
    out.push_str(RESOLVE_WRITE);
    out.push_str("\n\n");
    out.push_str(RESOLVE_GIVE_UP);
    out.push_str("\n\n");
    match delivery {
        SkillDelivery::PluginDir => {
            out.push_str("Use the smetana:resolving-questions skill — it is the process.");
        }
        SkillDelivery::Inline => match resolving {
            // The body, not the path: unlike an `Auto` stage the agent may
            // decline, this skill is the whole of what the session was started
            // to do, so there is no branch in which it goes unread.
            Some(process) => {
                out.push_str("The process:\n\n");
                out.push_str(process);
            }
            None => {
                let skill = skills.smetana.join("skills/resolving-questions/SKILL.md");
                let _ = write!(out, "The process is at {} — read it first.", skill.display());
            }
        },
    }
    out
}

/// What a session opens on when it is sent into a conflicted working tree.
///
/// Three things are named because nothing else can supply them. The repository,
/// because the session's own directory is the project and a project can hold
/// several. Both branches, in the order the operation put them — a merge brings
/// `theirs` into `ours`, a rebase moves `ours` onto `theirs` — since an agent
/// that has them the wrong way round resolves every hunk backwards. And every
/// conflicted path, because the tree is a moment: this is the list git left,
/// and after the first resolution it is no longer readable from anywhere.
///
/// No skill is named in either delivery, so this arm does not take `delivery`
/// at all. That is the decision rather than an omission — see `CONFLICT`.
fn resolve_conflict(
    repo: &str,
    op: crate::vcs::model::OpKind,
    ours: &str,
    theirs: &str,
    files: &[String],
) -> String {
    use crate::vcs::model::OpKind;

    let mut out = String::new();
    // A repository whose own branch could not be read — a HEAD already
    // detached, a branch list that had not landed — sends an empty name rather
    // than a made-up one, and the sentence still has to be true without it.
    let ours = if ours.trim().is_empty() { "the branch it is on" } else { ours };
    let _ = write!(out, "Finish a git {} in {repo}.\n\n", op.word());
    let _ = match op {
        OpKind::Merge => write!(out, "It is merging {theirs} into {ours}. "),
        OpKind::Rebase => write!(out, "It is rebasing {ours} onto {theirs}. "),
    };
    out.push_str(CONFLICT);
    out.push_str("\n\nThe conflicted files:\n\n");
    for file in files {
        let _ = writeln!(out, "{file}");
    }
    out.push('\n');
    out.push_str(CONFLICT_FINISH);
    out
}

/// What a session opens on when it is sent at a tracker nothing here could fix.
///
/// Four things are named because nothing else can supply them, and this is the
/// one prompt in the file that has to be complete at the instant it is sent:
/// the folder, since a session's own directory is the project and the prompt
/// would otherwise leave the subject unsaid; the bd this build ships, since
/// "the database is older than the binary" and "the binary is not what we think
/// it is" are two different faults and the version is what tells them apart;
/// and the failed command with its stderr, because the tracker is what is
/// broken — asking it again after the session starts is exactly what does not
/// work here.
///
/// stderr goes in last and in a block of its own, after every instruction: it
/// is the one part whose length nothing here controls, and an instruction
/// written after it would be the part that gets skimmed.
fn repair_tracker(dir: &str, bd_version: &str, command: &str, stderr: &str) -> String {
    let mut out = String::new();
    let _ = write!(out, "The bd tracker in {dir} is failing and Smetana could not repair it.\n\n");
    let _ = write!(out, "The bundled bd is version {bd_version}. ");
    // A failure that never reached a bd process — a spawn that did not start,
    // a folder with no tracker — has no command to name, and the sentence still
    // has to be true without one.
    let _ = match command.trim().is_empty() {
        true => write!(out, "There is no record of which bd command failed last.\n\n"),
        false => write!(out, "The bd command that failed last was `bd {command}`.\n\n"),
    };
    out.push_str(REPAIR);
    out.push_str("\n\nWhat bd said:\n\n");
    // An empty stderr is said out loud rather than left as a heading with
    // nothing under it: "bd failed and printed nothing" is itself a fact about
    // the failure, and a blank block reads as a prompt that was cut short.
    out.push_str(match stderr.trim().is_empty() {
        true => "(nothing — bd failed without printing to stderr)",
        false => stderr.trim(),
    });
    out
}

/// What a session opens on when it is sent to review a branch.
///
/// Everything variable is named here and nothing about how to review is: the
/// pairs and the report's path belong to this one session and are in no file,
/// while the method is the same every time and is the skill's. That is the
/// same split `run` holds one function up.
///
/// One line per repository, `<repo>: <base> → <head>`, because a project can
/// hold several and a review that ran in one of them is not the review that
/// was asked for. The refs travel exactly as they arrived — `main` or
/// `origin/main` — since which of the two was meant is settled before the
/// intent is built, and an agent resolving it a second time here would be
/// answering a question that already has an answer.
///
/// Both files are named with their extensions rather than left to the skill,
/// and that is the one thing in this prompt the app depends on: it composed
/// that path itself and opens the report at it afterwards, so an agent that
/// wrote only one of the two would leave a tab pointing at nothing.
fn review_branch(
    pairs: &[ReviewPair],
    report: &str,
    delivery: SkillDelivery,
    skills: &Skills,
    reviewing_branch: Option<&str>,
) -> String {
    let mut out = String::from(REVIEW_BRANCH);
    // A review with nothing to compare is not something this app builds — the
    // window that starts one cannot offer an empty list — but the prompt still
    // has to be a set of true sentences rather than a heading with a gap under
    // it, which is the reading `repair_tracker` takes of an empty stderr.
    match pairs.is_empty() {
        true => out.push_str(
            "\n\nNothing was named to compare. Say so and stop, rather than guessing at a \
             repository or at a ref.",
        ),
        false => {
            out.push_str("\n\nWhat to review, one line per repository:\n\n");
            for pair in pairs {
                let _ = writeln!(out, "{}: {} → {}", pair.repo, pair.base, pair.head);
            }
        }
    }
    let _ = write!(
        out,
        "\nWrite the report to {report}.md and {report}.html — both of them, the same review in \
         two forms, at those paths relative to the project. The HTML one is drawn inside Smetana \
         in a sandboxed frame, so it has to carry its own styling and reach nowhere outside \
         itself: no external stylesheet, no font from a network, no script and no image."
    );
    out.push_str("\n\n");
    match delivery {
        SkillDelivery::PluginDir => {
            out.push_str("Use the smetana:reviewing-branch-changes skill — it is the method.");
        }
        SkillDelivery::Inline => match reviewing_branch {
            // The body, not the path, for the reason `resolve_task` gives: this
            // skill is the whole of what the session was started to do, so
            // there is no branch in which it goes unread.
            Some(process) => {
                out.push_str("The method:\n\n");
                out.push_str(process);
            }
            None => {
                let skill = skills.smetana.join("skills/reviewing-branch-changes/SKILL.md");
                let _ = write!(out, "The method is at {} — read it first.", skill.display());
            }
        },
    }
    out
}

fn setup(delivery: SkillDelivery, skills: &Skills, facts: Option<&str>) -> String {
    let mut out = String::from(SETUP);
    out.push_str("\n\n");
    match delivery {
        SkillDelivery::PluginDir => {
            out.push_str("Use the smetana:project-setup skill — it says what the file holds.");
        }
        SkillDelivery::Inline => {
            let skill = skills.smetana.join("skills/project-setup/SKILL.md");
            let _ = write!(
                out,
                "What the file holds is described at {} — read it first.",
                skill.display()
            );
        }
    }
    if let Some(facts) = facts {
        out.push_str("\n\n");
        out.push_str(facts.trim_end());
    }
    out
}

/// What the person pinned, and what they left on Auto. Auto is said out loud
/// rather than left to silence: an agent told nothing about the type would
/// have to invent one anyway, but would not know that inventing it was its
/// job rather than a gap in what it was told.
fn fields(draft: &TaskDraft) -> String {
    let mut given: Vec<String> = Vec::new();
    let mut auto: Vec<&str> = Vec::new();
    match &draft.issue_type {
        Some(kind) => given.push(format!("type {kind}")),
        None => auto.push("type"),
    }
    match draft.priority {
        Some(priority) => given.push(format!("priority P{priority}")),
        None => auto.push("priority"),
    }

    let mut out = String::new();
    if !given.is_empty() {
        let _ = write!(out, "File it with {}.", given.join(" and "));
    }
    if !auto.is_empty() {
        if !out.is_empty() {
            out.push(' ');
        }
        let _ = write!(
            out,
            "Decide the {} yourself, from what is written above.",
            auto.join(" and the ")
        );
    }
    out
}

/// What the agent owes us for an attached image, and it is two things at once.
///
/// The paths, because a described mock is not a mock: whoever picks the task up
/// opens the pictures by the strings in the description, and nothing else in
/// bd carries them. The words, because those paths are on one machine only —
/// they are in this app's data directory, not in the repository, and in
/// somebody else's clone they lead nowhere. Either half alone loses something
/// that cannot be got back from the other.
const IMAGES: &str = "Copy each path into the issue description exactly as it is written above, and \
     also say in words what matters in each picture. The paths are how whoever picks this up opens \
     the images; the words are what is left of them on a machine that does not have the files. \
     Neither on its own is enough.";

/// The images, named. Empty when there are none — the whole block is absent
/// then, rather than a heading with nothing under it.
fn images(paths: &[String], delivery: ImageDelivery) -> String {
    if paths.is_empty() {
        return String::new();
    }
    let one = paths.len() == 1;
    let mut out = String::new();
    let _ = write!(
        out,
        "{} attached to this task, at {} absolute path{} on this machine:\n\n",
        if one { "There is an image" } else { "There are images" },
        if one { "this" } else { "these" },
        if one { "" } else { "s" }
    );
    for path in paths {
        let _ = writeln!(out, "{path}");
    }
    out.push('\n');
    let it = if one { "it" } else { "them" };
    match delivery {
        // The harness took the files on its command line; saying so keeps it
        // from hunting for something it is already holding.
        ImageDelivery::Flag(_) => {
            let _ = write!(
                out,
                "{} attached to this session as well. ",
                if one { "It is" } else { "They are" }
            );
        }
        // The only channel there is: this harness opens an image when the path
        // is in front of it.
        ImageDelivery::InPrompt => {
            let _ = write!(out, "Open and look at {it} before you write anything. ");
        }
    }
    out.push_str(IMAGES);
    out.push_str("\n\n");
    out
}

/// The two stages under the discussion, already cascaded. Empty when both are
/// `Off` — a task filed with no paperwork asked for must carry no prose about
/// paperwork at all, or an agent starts looking for a place to put a document
/// nobody wanted.
///
/// The order is the order of the work: the design, then the plan, then how
/// either reaches whoever picks the task up. A skill body, where one is
/// carried, goes last so that 7 KB of process does not push the instructions
/// off the top of what the agent reads first.
fn stages(
    spec: Stage,
    plan: Stage,
    delivery: SkillDelivery,
    plans: &Path,
    plans_text: Option<&str>,
) -> String {
    if spec == Stage::Off && plan == Stage::Off {
        return String::new();
    }
    let mut out = String::new();
    match spec {
        Stage::On => out.push_str(SPEC),
        Stage::Auto => out.push_str(SPEC_JUDGE),
        Stage::Off => {}
    }
    if spec != Stage::Off && plan != Stage::Off {
        out.push(' ');
    }
    match plan {
        Stage::On => out.push_str(PLAN),
        Stage::Auto => out.push_str(PLAN_JUDGE),
        Stage::Off => {}
    }

    // How the plan's own process reaches this harness. The design document is
    // part of the brainstorming process, which is already named or already
    // pasted whole whenever the spec stage is reachable at all, so only the
    // plan needs a skill of its own.
    match (plan, delivery) {
        (Stage::Off, _) => {}
        (Stage::On, SkillDelivery::PluginDir) => {
            out.push_str(" Use the superpowers:writing-plans skill for it.");
        }
        (Stage::Auto, SkillDelivery::PluginDir) => {
            out.push_str(" If you write one, use the superpowers:writing-plans skill for it.");
        }
        (Stage::Auto, SkillDelivery::Inline) => {
            let _ = write!(
                out,
                " If you write one, the process is at {} — read it first.",
                plans.join("SKILL.md").display()
            );
        }
        // `On` inline: the body itself, appended at the end below.
        (Stage::On, SkillDelivery::Inline) => {}
    }

    // Last of the paragraph, and unconditional: whichever stages were asked
    // for, what they produce is English.
    out.push_str(IN_ENGLISH);

    out.push_str("\n\n");
    out.push_str(PAPERWORK);

    if let (Stage::On, SkillDelivery::Inline, Some(process)) = (plan, delivery, plans_text) {
        out.push_str("\n\nFollow this process for the plan:\n\n");
        out.push_str(process);
    }
    out
}

#[allow(clippy::too_many_arguments)]
fn new_task(
    brainstorm: Stage,
    spec: Stage,
    plan: Stage,
    draft: &TaskDraft,
    delivery: SkillDelivery,
    image_delivery: ImageDelivery,
    brainstorming: &Path,
    plans: &Path,
    text: SkillText,
) -> String {
    let mut out = String::new();
    out.push_str("File a new task in this project's bd tracker. This is what needs doing:\n\n");
    out.push_str(draft.text.trim());
    out.push_str("\n\n");
    out.push_str(&images(&draft.images, image_delivery));
    out.push_str(&fields(draft));
    // After the type and the priority, which are the other two things this task
    // *is*, and before STANDARD, which is about how any task is filed.
    if let Some(parent) = &draft.parent {
        out.push_str("\n\n");
        out.push_str(&follow_up(parent));
    }
    out.push_str("\n\n");
    out.push_str(STANDARD);
    out.push_str("\n\n");

    // How to file one properly is not part of the brainstorming question: an
    // agent that files without any discussion still has to file it well. A
    // harness with a registry is told the name; one without gets the text.
    match delivery {
        SkillDelivery::PluginDir => {
            out.push_str("Use the smetana:filing-a-task skill for how this project wants it worded.\n\n");
        }
        SkillDelivery::Inline => {
            if let Some(filing) = text.filing {
                out.push_str("How this project wants a task filed:\n\n");
                out.push_str(filing);
                out.push_str("\n\n");
            }
        }
    }

    match (brainstorm, delivery) {
        (Stage::Off, _) => {
            out.push_str("File it now. No design discussion is wanted for this one.");
        }
        (Stage::On, SkillDelivery::PluginDir) => {
            out.push_str("Use the superpowers:brainstorming skill. ");
            out.push_str(DISCUSS);
        }
        (Stage::On, SkillDelivery::Inline) => {
            out.push_str(DISCUSS);
            if let Some(process) = text.brainstorming {
                out.push_str("\n\nFollow this process:\n\n");
                out.push_str(process);
            }
        }
        (Stage::Auto, SkillDelivery::PluginDir) => {
            out.push_str(JUDGE);
            out.push_str(" If you decide to discuss it, use the superpowers:brainstorming skill.");
        }
        (Stage::Auto, SkillDelivery::Inline) => {
            out.push_str(JUDGE);
            let _ = write!(
                out,
                " If you decide to discuss it, the process is at {} — read it first.",
                brainstorming.join("SKILL.md").display()
            );
        }
    }

    // Last, because it is the last of the work: what the discussion produced,
    // written down, and only then the task itself.
    let paperwork = stages(spec, plan, delivery, plans, text.plans);
    if !paperwork.is_empty() {
        out.push_str("\n\n");
        out.push_str(&paperwork);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::{Intent, Stage, TaskDraft};
    use std::path::PathBuf;

    fn draft() -> TaskDraft {
        TaskDraft {
            text: "Swap the red for green".into(),
            issue_type: Some("bug".into()),
            priority: Some(2),
            images: Vec::new(),
            parent: None,
        }
    }

    /// The switch under test on its own: both later stages off, so nothing
    /// they produce can land in a prompt these cases are not about.
    fn new_task(brainstorm: Stage) -> Intent {
        Intent::NewTask { brainstorm, spec: Stage::Off, plan: Stage::Off, draft: draft() }
    }

    /// All three switches, as the dialog sends them — the cascade is applied
    /// on this side, so a combination the dialog cannot draw is a fair thing
    /// to pass in.
    fn staged(brainstorm: Stage, spec: Stage, plan: Stage) -> Intent {
        Intent::NewTask { brainstorm, spec, plan, draft: draft() }
    }

    fn skills() -> crate::agents::library::Skills {
        crate::agents::library::Skills {
            smetana: PathBuf::from("/app/resources/smetana"),
            superpowers: PathBuf::from("/app/resources/superpowers"),
            superpowers_installed: false,
        }
    }

    const BRAINSTORMING: &str = "# Brainstorming\n\nAsk one question at a time.";
    const FILING: &str = "# Filing a task\n\nThe title says what needs doing.";
    const RESOLVING: &str = "# Resolving\n\nEverything below the last resolved line is open.";
    const PLANS: &str = "# Writing plans\n\nEvery step names the file it touches.";
    /* No `## ` heading in it, deliberately: the two tests that walk every
       intent for a translated section heading read the whole prompt, and a
       fixture carrying one would fail them for a session that files nothing. */
    const REVIEWING_BRANCH: &str =
        "# Reviewing branch changes\n\nRead the file, not the diff.";

    /// The shipped pair: both settings on their default. What almost every
    /// test here is about is not the language, so this is the fixture that
    /// keeps the language out of the way.
    fn english() -> Languages {
        Languages::default()
    }

    /// One language chosen for both, which is the case a person who does not
    /// work in English is actually in.
    fn russian() -> Languages {
        Languages {
            agent: "ru".into(),
            task: "ru".into(),
            commit: "ru".into(),
            report: "ru".into(),
        }
    }

    /// Nothing read: what a PluginDir harness always gets, and what an Inline
    /// harness gets when the files cannot be read.
    fn nothing() -> SkillText<'static> {
        SkillText {
            filing: None,
            resolving: None,
            brainstorming: None,
            plans: None,
            reviewing_branch: None,
        }
    }

    fn every_skill() -> SkillText<'static> {
        SkillText {
            filing: Some(FILING),
            resolving: Some(RESOLVING),
            brainstorming: Some(BRAINSTORMING),
            plans: Some(PLANS),
            reviewing_branch: Some(REVIEWING_BRANCH),
        }
    }

    /// A floor only where the scope allows one and a number of agents only
    /// where the mode allows one — `RunSettings::validate` is what refuses the
    /// rest, and a fixture that could not be started is not worth writing a
    /// prompt for.
    fn run_settings(mode: RunMode, scope: RunScope) -> RunSettings {
        let min_priority = matches!(scope, RunScope::Queue).then_some(2);
        let max_parallel_tasks = (!matches!(mode, RunMode::Solo)).then_some(3);
        RunSettings {
            scope,
            mode,
            target_branch: "staging".into(),
            create_target: false,
            min_priority,
            max_parallel_tasks,
            live_check: true,
            file_findings: true,
        }
    }

    /// The intent a run's batch actually arrives as. The directory and the
    /// batch number are the run's own and are not in any file, so a fixture
    /// names them the way `runs::service` builds them.
    fn run_intent(settings: RunSettings) -> Intent {
        run_intent_with(settings, true)
    }

    /// `remove_worktrees` is not part of `RunSettings` and never will be — see
    /// the field's own doc on `Intent::Run` — so it is the one thing a fixture
    /// here has to vary by hand. `run_intent` above is the shipped position.
    fn run_intent_with(settings: RunSettings, remove_worktrees: bool) -> Intent {
        Intent::Run {
            settings,
            reports: std::path::PathBuf::from("/p/.smetana/runs/7"),
            batch: 2,
            remove_worktrees,
        }
    }

    fn run_prompt(settings: RunSettings, delivery: SkillDelivery) -> String {
        prompt_of(run_intent(settings), delivery)
    }

    fn prompt_of(intent: Intent, delivery: SkillDelivery) -> String {
        build(&intent, delivery, ImageDelivery::InPrompt, &skills(), None, nothing(), &english(), "")
            .unwrap()
    }

    #[test]
    fn a_run_names_the_exact_file_its_batch_writes_its_account_to() {
        // The app cannot see what a session did — an exit code is the whole of
        // what comes back — so the account is asked for, by name. The number is
        // the app's and not the agent's: a batch working the file name out for
        // itself is one the app could not then match to the batch it timed.
        for delivery in [SkillDelivery::PluginDir, SkillDelivery::Inline] {
            let text = run_prompt(run_settings(RunMode::Auto, RunScope::Queue), delivery);
            assert!(text.contains("/p/.smetana/runs/7/batch-2.json"), "{delivery:?}: {text}");
            assert!(text.contains("\"did\""), "the shape is named too: {text}");
            assert!(
                text.contains("record, not a gate"),
                "a batch that cannot write it carries on: {text}"
            );
            assert!(
                text.contains("replaces no part of it"),
                "the JSON is beside the prose report, never instead of it: {text}"
            );
        }
    }

    #[test]
    fn the_batch_file_asks_for_identifiers_in_backticks() {
        // `report.rs::prose` marks a backtick span and nothing else,
        // deliberately: a shape heuristic cannot be reviewed. So the backticks
        // have to be asked for here, or the rule has nothing to act on.
        for delivery in [SkillDelivery::PluginDir, SkillDelivery::Inline] {
            let text = run_prompt(run_settings(RunMode::Auto, RunScope::Queue), delivery);
            assert!(text.contains("backticks"), "{delivery:?}: {text}");
        }
    }

    #[test]
    fn an_attended_batch_is_told_its_account_is_how_it_hands_the_work_back() {
        // The two modes whose session stays alive after the work is done. There
        // the file is not a record at all — it is the only thing that tells the
        // app the batch is over, because the process never exits — so the
        // wording that lets an unattended batch shrug it off would be false
        // here, and a lead that shrugged would leave the run hanging with
        // nothing on screen to say why.
        for mode in [RunMode::Supervised, RunMode::Solo] {
            for delivery in [SkillDelivery::PluginDir, SkillDelivery::Inline] {
                let scope = RunScope::Task { id: "a-1".into() };
                let text = run_prompt(run_settings(mode, scope), delivery);
                assert!(text.contains("/p/.smetana/runs/7/batch-2.json"), "{mode:?}: {text}");
                assert!(
                    !text.contains("record, not a gate"),
                    "{mode:?}: here it is exactly a gate: {text}"
                );
                assert!(
                    text.contains("how you hand the work back"),
                    "{mode:?}: the file has to be named as the hand-back: {text}"
                );
                assert!(
                    text.contains("say so in this conversation"),
                    "{mode:?}: a file that cannot be written must not be a silence: {text}"
                );
            }
        }
    }

    #[test]
    fn no_other_intent_is_asked_for_a_batch_account() {
        // The file belongs to a run's batch and to nothing else: there is no
        // batch behind a filing session or an edit, and no directory made for
        // one either.
        for intent in [
            Intent::Bare,
            Intent::Setup,
            Intent::EditTask { id: "x-1".into(), title: "T".into() },
            Intent::ResolveTask { id: "x-1".into(), title: "T".into() },
            conflict(crate::vcs::model::OpKind::Merge),
            new_task(Stage::On),
        ] {
            for delivery in [SkillDelivery::PluginDir, SkillDelivery::Inline] {
                let text = build(
                    &intent,
                    delivery,
                    ImageDelivery::InPrompt,
                    &skills(),
                    Some(FACTS),
                    every_skill(),
                    &english(),
                    "",
                )
                .unwrap_or_default();
                assert!(!text.contains(".smetana/runs/"), "{intent:?}/{delivery:?}: {text}");
            }
        }
    }

    #[test]
    fn a_run_names_the_process_skill_in_both_deliveries() {
        let named = run_prompt(run_settings(RunMode::Auto, RunScope::Queue), SkillDelivery::PluginDir);
        assert!(named.contains("smetana:running-tasks"), "{named}");

        let pointed = run_prompt(run_settings(RunMode::Auto, RunScope::Queue), SkillDelivery::Inline);
        assert!(
            pointed.contains("/app/resources/smetana/skills/running-tasks/SKILL.md"),
            "a harness with no registry gets the path: {pointed}"
        );
        assert!(!pointed.contains("smetana:running-tasks"), "it has no registry to name");
    }

    #[test]
    fn every_setting_the_person_chose_reaches_the_prompt() {
        // The config is read by the skill; these are the ones that exist only
        // in this run and are in no file for it to find.
        let text = run_prompt(
            RunSettings {
                target_branch: "release/7".into(),
                min_priority: Some(1),
                ..run_settings(RunMode::Auto, RunScope::Queue)
            },
            SkillDelivery::PluginDir,
        );
        assert!(text.contains("release/7"), "{text}");
        assert!(text.contains("P1"), "{text}");
    }

    #[test]
    fn only_a_queue_is_told_about_a_priority_floor() {
        // "Work only on issue X, and nothing else" beside "take nothing worse
        // than P2" is two instructions that contradict each other, and the
        // work is already named — there is nothing left for a floor to pick.
        let queue = run_prompt(run_settings(RunMode::Auto, RunScope::Queue), SkillDelivery::PluginDir);
        assert!(queue.contains("nothing worse than priority P2"), "{queue}");

        let one = run_prompt(
            run_settings(RunMode::Auto, RunScope::Task { id: "smetana-9".into() }),
            SkillDelivery::PluginDir,
        );
        assert!(!one.contains("nothing worse than priority"), "{one}");

        let epic = run_prompt(
            run_settings(RunMode::Auto, RunScope::Epic { id: "smetana-4".into() }),
            SkillDelivery::PluginDir,
        );
        assert!(!epic.contains("nothing worse than priority"), "{epic}");
    }

    #[test]
    fn the_number_of_agents_is_this_runs_and_says_it_beats_the_config() {
        // The skill reads `[defaults].max_parallel_tasks` for itself, so a
        // number merely stated would be read as an upper bound to stay under —
        // and choosing more than the file says would silently do nothing.
        let text = run_prompt(
            RunSettings { max_parallel_tasks: Some(6), ..run_settings(RunMode::Auto, RunScope::Queue) },
            SkillDelivery::PluginDir,
        );
        assert!(text.contains("at most 6 tasks at once"), "{text}");
        assert!(text.contains("wins over"), "{text}");
        assert!(text.contains("max_parallel_tasks"), "{text}");

        // One is singular, because a prompt reading "at most 1 tasks" is a
        // prompt somebody wrote without looking.
        let one = run_prompt(
            RunSettings { max_parallel_tasks: Some(1), ..run_settings(RunMode::Auto, RunScope::Queue) },
            SkillDelivery::PluginDir,
        );
        assert!(one.contains("at most 1 task at once"), "{one}");

        // Solo delegates to nobody: the line is absent rather than set to one,
        // which would be a second instruction contradicting "do it yourself".
        let solo = run_prompt(
            run_settings(RunMode::Solo, RunScope::Task { id: "smetana-9".into() }),
            SkillDelivery::PluginDir,
        );
        assert!(!solo.contains("at once"), "{solo}");
    }

    #[test]
    fn a_branch_that_may_have_to_be_cut_says_so_and_says_what_from() {
        // `create_target` no longer means "the branch does not exist" — it
        // means cutting it where it does not exist is sanctioned. The
        // difference is the whole of the multi-repository case: `release/8` can
        // sit in two repositories of four, and a prompt claiming it does not
        // exist is false about the two that have it.
        let settings = RunSettings {
            target_branch: "release/8".into(),
            create_target: true,
            ..run_settings(RunMode::Auto, RunScope::Queue)
        };
        let text = run_prompt(settings, SkillDelivery::PluginDir);
        assert!(text.contains("release/8"), "{text}");
        assert!(text.contains("wherever it does not exist yet"), "{text}");
        assert!(text.contains("cut it from the repository's own current branch"), "{text}");

        // And a branch that is everywhere says nothing of the kind: an agent
        // told to cut a branch that is already there rewrites somebody's
        // history or stops on its first command.
        let plain = run_prompt(run_settings(RunMode::Auto, RunScope::Queue), SkillDelivery::PluginDir);
        assert!(!plain.contains("does not exist"), "{plain}");
        assert!(!plain.contains("cut it"), "{plain}");
    }

    #[test]
    fn the_scope_says_what_may_be_touched() {
        let queue = run_prompt(run_settings(RunMode::Auto, RunScope::Queue), SkillDelivery::PluginDir);
        assert!(queue.contains("Take ready tasks"), "{queue}");

        let one = run_prompt(
            run_settings(RunMode::Auto, RunScope::Task { id: "smetana-9".into() }),
            SkillDelivery::PluginDir,
        );
        assert!(one.contains("only on issue smetana-9"), "{one}");
        assert!(one.contains("and nothing else"), "{one}");

        let epic = run_prompt(
            run_settings(RunMode::Auto, RunScope::Epic { id: "smetana-4".into() }),
            SkillDelivery::PluginDir,
        );
        assert!(epic.contains("children of smetana-4"), "{epic}");
    }

    #[test]
    fn each_mode_says_what_to_do_when_something_is_unclear() {
        // The whole difference between the three modes is this one line, and an
        // agent that never reads it would ask a question nobody can answer.
        let auto = run_prompt(run_settings(RunMode::Auto, RunScope::Queue), SkillDelivery::PluginDir);
        assert!(auto.contains("no one to ask"), "{auto}");
        assert!(auto.contains("Park"), "{auto}");

        let supervised =
            run_prompt(run_settings(RunMode::Supervised, RunScope::Queue), SkillDelivery::PluginDir);
        assert!(supervised.contains("ask me"), "{supervised}");
        assert!(!supervised.contains("no one to ask"), "{supervised}");

        let solo = run_prompt(
            run_settings(RunMode::Solo, RunScope::Task { id: "smetana-9".into() }),
            SkillDelivery::PluginDir,
        );
        assert!(solo.contains("yourself rather than delegating"), "{solo}");
    }

    #[test]
    fn the_two_switches_are_stated_in_both_positions() {
        // Silence would be read as the default, and the defaults differ from
        // what a person may have just turned off.
        let on = run_prompt(run_settings(RunMode::Auto, RunScope::Queue), SkillDelivery::PluginDir);
        assert!(on.contains("verify each merged task"), "{on}");
        assert!(on.contains("may be filed as `deferred`"), "{on}");

        let off = run_prompt(
            RunSettings {
                live_check: false,
                file_findings: false,
                ..run_settings(RunMode::Auto, RunScope::Queue)
            },
            SkillDelivery::PluginDir,
        );
        assert!(off.contains("no live check this run"), "{off}");
        assert!(off.contains("file nothing new"), "{off}");
    }

    #[test]
    fn the_worktree_switch_is_stated_in_both_positions_and_neither_carries_the_other() {
        // The third switch in that list, and the one that is not a
        // `RunSettings` field. Silence would be read as the default here too —
        // and the default is "remove", so a run told nothing would sweep up
        // worktrees somebody had just asked it to keep.
        let settings = run_settings(RunMode::Auto, RunScope::Queue);

        let on = prompt_of(run_intent_with(settings.clone(), true), SkillDelivery::PluginDir);
        assert!(on.contains("remove each task's worktree once it is merged and closed"), "{on}");
        assert!(!on.contains("never remove one"), "no trace of the other branch: {on}");

        let off = prompt_of(run_intent_with(settings, false), SkillDelivery::PluginDir);
        assert!(off.contains("never remove one"), "{off}");
        // The report half is the point of the Off branch: this app cannot see a
        // worktree, so nothing but the lead's own account can tell a person
        // what is still on their disk.
        assert!(off.contains("say in your report that they were kept"), "{off}");
        assert!(!off.contains("remove each task's worktree"), "no trace of the other branch: {off}");
    }

    #[test]
    fn a_bare_session_opens_on_the_languages_and_nothing_else() {
        // It used to open on nothing at all, and that changed with the choice
        // of an English default over an Auto position: the one session where a
        // person talks to the agent most cannot be the one the setting never
        // reaches. What is imposed is still only language — a bare session has
        // no work, so there is nothing else to say — but it is three
        // paragraphs, in the order the caller writes them: the conversation,
        // the issues, the commits. "+ New agent" is one of the two intents
        // where every one of those three is true — `Run` is the other and
        // always was — because a person there says "commit this" and "file
        // tasks for this" in the same breath. `Run` alone takes the fourth
        // paragraph as well, which is why this equality is three and not four.
        let text = build(&Intent::Bare, SkillDelivery::PluginDir, ImageDelivery::InPrompt, &skills(), None, nothing(), &english(), "")
            .expect("a bare session opens on the language sentences");
        assert_eq!(
            text,
            format!(
                "{}\n\n{}\n\n{}",
                conversation("English"),
                task_language("English"),
                commit_language("English")
            )
        );

        let russian = build(&Intent::Bare, SkillDelivery::PluginDir, ImageDelivery::InPrompt, &skills(), None, nothing(), &russian(), "")
            .expect("builds");
        assert!(russian.contains("Russian"), "{russian}");
    }

    #[test]
    fn editing_an_issue_names_it_and_asks_what_to_change() {
        let intent = Intent::EditTask { id: "smetana-7".into(), title: "x y".into() };
        let text = build(&intent, SkillDelivery::PluginDir, ImageDelivery::InPrompt, &skills(), None, nothing(), &english(), "").unwrap();
        // The work is the whole of the prompt after the language paragraphs,
        // which is what `ends_with` pins here: an edit session is told what to
        // do and nothing more.
        assert!(text.ends_with(&format!("Update bd issue smetana-7 (\"x y\"). {EDIT}")), "{text}");
        assert!(text.contains("ask me what to change"), "{text}");
    }

    #[test]
    fn fixing_a_done_task_names_it_and_asks_what_is_wrong() {
        let intent = Intent::FixTask { id: "smetana-7".into(), title: "x y".into() };
        let text = build(&intent, SkillDelivery::PluginDir, ImageDelivery::InPrompt, &skills(), None, nothing(), &english(), "").unwrap();
        // The work is the whole of the prompt after the language paragraphs,
        // the way an edit's is.
        assert!(text.ends_with(&format!("Issue smetana-7 (\"x y\") {FIX}")), "{text}");
        // The four things it has to say, each of them load-bearing: where the
        // code is, that it asks rather than guesses, that it finishes the job,
        // and that it hands back anything too big for a conversation.
        assert!(text.contains("already merged"), "{text}");
        assert!(text.contains("ask me what is wrong"), "{text}");
        assert!(text.contains("commit it"), "{text}");
        assert!(text.contains("stays closed"), "{text}");
    }

    #[test]
    fn a_resumed_session_is_handed_no_prompt_at_all() {
        // The one intent `build` answers `None` for, and the reason is that a
        // prompt is not an opening remark here: both harnesses submit the
        // positional argument as the session's first message, and this session
        // already has somebody's words in it. Even the conversation-language
        // paragraph — which reaches every other intent, "+ New agent"
        // included — would be the app typing into a conversation of theirs.
        //
        // Both languages, because the paragraphs are what would otherwise make
        // this `Some` whatever the work says.
        //
        // Both verbs too: a fork opens on a conversation that already has
        // somebody's words in it exactly as a resume does, so the refusal
        // cannot be written against `fork == false`.
        for (languages, fork) in
            [(english(), false), (english(), true), (russian(), false), (russian(), true)]
        {
            let built = build(
                &Intent::ResumeSession {
                    id: "9f1c0a2e".into(),
                    cwd: "/p/.worktrees/smetana-0cj".into(),
                    title: Some("Move the card".into()),
                    fork,
                },
                SkillDelivery::PluginDir,
                ImageDelivery::InPrompt,
                &skills(),
                Some(FACTS),
                every_skill(),
                &languages,
                "",
            );
            assert_eq!(built, None, "{languages:?} put a prompt on a resumed session (fork: {fork})");
        }
    }

    #[test]
    fn no_prompt_stops_mid_sentence() {
        // Every prompt is submitted as the session's first message, not left in
        // a composer for somebody to finish: it rides as the agent's positional
        // argument, and both harnesses send it straight through. The edit
        // prompt used to end `("x y"): ` on the opposite assumption, so the
        // session opened by asking whether the message had been cut off.
        //
        // Trailing punctuation is the whole test, and it is deliberately not a
        // check for some phrase: what makes a prompt broken here is that it
        // hands over mid-instruction, which is exactly what a dangling colon,
        // comma or dash looks like.
        for intent in [
            Intent::Bare,
            Intent::EditTask { id: "x-1".into(), title: "T".into() },
            Intent::ResolveTask { id: "x-1".into(), title: "T".into() },
            Intent::FixTask { id: "x-1".into(), title: "T".into() },
            conflict(crate::vcs::model::OpKind::Merge),
            conflict(crate::vcs::model::OpKind::Rebase),
            repair(),
            // The two holes in the repair briefing, walked as well: the whole
            // sentence about the failed command disappears when nothing
            // recorded one, and the prompt then ends on bd's own words being
            // absent — both are places a sentence could be left hanging.
            Intent::RepairTracker {
                dir: "/p".into(),
                bd_version: "1.1.2".into(),
                command: String::new(),
                stderr: String::new(),
            },
            Intent::Setup,
            review(),
            // The other hole in a review's briefing, walked for the reason the
            // two repair holes above are: with no pair the whole list and its
            // heading disappear, and the sentence that replaces them is one
            // more place a prompt could be left hanging.
            Intent::ReviewBranch {
                pairs: Vec::new(),
                report: ".smetana/reviews/2026-08-31-pf40".into(),
            },
            new_task(Stage::Auto),
            new_task(Stage::On),
            new_task(Stage::Off),
            run_intent(run_settings(RunMode::Auto, RunScope::Queue)),
        ] {
            for delivery in [SkillDelivery::PluginDir, SkillDelivery::Inline] {
                // Both language settings as well: every one of these prompts
                // now opens on a sentence this walk has to cover.
                for languages in [english(), russian()] {
                    let text = build(
                        &intent,
                        delivery,
                        ImageDelivery::InPrompt,
                        &skills(),
                        Some(FACTS),
                        every_skill(),
                        &languages,
                        "",
                    )
                    .unwrap();
                    let end = text.trim_end();
                    assert!(
                        !end.ends_with([':', ',', '—', '-']),
                        "{intent:?}/{delivery:?}/{languages:?} hands the agent an unfinished \
                         instruction: {end}"
                    );
                }
            }
        }
    }

    #[test]
    fn editing_an_issue_is_never_given_a_filing_skill() {
        let intent = Intent::EditTask { id: "smetana-7".into(), title: "x y".into() };
        let text = build(&intent, SkillDelivery::Inline, ImageDelivery::InPrompt, &skills(), None, every_skill(), &english(), "").unwrap();
        assert!(!text.contains("The title says what needs doing"), "nothing is filed here");
    }

    fn resolving(delivery: SkillDelivery, text: SkillText) -> String {
        let intent =
            Intent::ResolveTask { id: "smetana-29j".into(), title: "Show the state".into() };
        build(&intent, delivery, ImageDelivery::InPrompt, &skills(), None, text, &english(), "").unwrap()
    }

    fn repair() -> Intent {
        Intent::RepairTracker {
            dir: "/p/backend".into(),
            bd_version: "1.1.2".into(),
            command: "list --all -n 0 --json".into(),
            stderr: "failed to open store: schema version 41 is older than 53".into(),
        }
    }

    fn repair_prompt(delivery: SkillDelivery) -> String {
        build(&repair(), delivery, ImageDelivery::InPrompt, &skills(), None, every_skill(), &english(), "")
            .unwrap()
    }

    #[test]
    fn a_repair_prompt_names_the_folder_the_bd_version_the_command_and_what_bd_said() {
        // All four, because the tracker is what is broken: an agent cannot ask
        // bd afterwards for any of them, so a briefing incomplete at the moment
        // it is sent stays incomplete. This is the `ResolveConflict` shape and
        // deliberately not the `ResolveTask` one, where almost nothing is
        // carried because the issue can be read again.
        for delivery in [SkillDelivery::PluginDir, SkillDelivery::Inline] {
            let text = repair_prompt(delivery);
            assert!(text.contains("/p/backend"), "the folder: {text}");
            assert!(text.contains("1.1.2"), "the bd this build ships: {text}");
            assert!(text.contains("bd list --all -n 0 --json"), "the failed command: {text}");
            assert!(
                text.contains("schema version 41 is older than 53"),
                "what bd said: {text}"
            );
        }
    }

    #[test]
    fn a_repair_prompt_says_what_the_app_already_tried_and_leaves_the_copy_alone() {
        // The two migrations have been run by the time this session starts, so
        // an agent told nothing would begin by running them again. And the copy
        // beside `.beads` is the person's only way back from a migration, which
        // makes tidying it away the one helpful act that cannot be undone.
        let text = repair_prompt(SkillDelivery::PluginDir);
        assert!(text.contains("`bd migrate` and `bd migrate schema`"), "{text}");
        assert!(text.contains(".beads.backup-"), "{text}");
        assert!(text.contains("Do not delete or move that copy"), "{text}");
        // And the two measurements an agent would otherwise spend a while
        // rediscovering.
        assert!(text.contains("`bd doctor` is not supported"), "{text}");
    }

    /// A failure that never reached a bd process has no command and no stderr,
    /// and the prompt still has to be a set of true sentences rather than one
    /// with two holes in it.
    #[test]
    fn a_repair_prompt_with_nothing_recorded_says_so_rather_than_naming_an_empty_command() {
        let intent = Intent::RepairTracker {
            dir: "/p".into(),
            bd_version: "1.1.2".into(),
            command: String::new(),
            stderr: String::new(),
        };
        let text =
            build(&intent, SkillDelivery::PluginDir, ImageDelivery::InPrompt, &skills(), None, every_skill(), &english(), "")
                .unwrap();
        assert!(text.contains("no record of which bd command failed"), "{text}");
        assert!(!text.contains("`bd `"), "an empty command is not named: {text}");
        assert!(text.contains("bd failed without printing to stderr"), "{text}");
    }

    fn conflict(op: crate::vcs::model::OpKind) -> Intent {
        Intent::ResolveConflict {
            repo: "/p/backend".into(),
            op,
            ours: "main".into(),
            theirs: "develop".into(),
            files: vec!["src/one.rs".into(), "src/two.rs".into()],
        }
    }

    /// Two repositories, four refs, and both spellings a ref arrives in — a
    /// local branch and a remote-tracking one — since the choice between them
    /// is made before the intent is built and both have to survive the trip.
    fn review() -> Intent {
        Intent::ReviewBranch {
            pairs: vec![
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
            ],
            report: ".smetana/reviews/2026-08-31-pf40".into(),
        }
    }

    fn review_prompt(delivery: SkillDelivery) -> String {
        build(&review(), delivery, ImageDelivery::InPrompt, &skills(), None, every_skill(), &english(), "")
            .unwrap()
    }

    #[test]
    fn a_review_prompt_names_every_pair_and_both_report_files() {
        // The whole line per repository rather than the four refs on their own,
        // and that is what makes a dropped pair fail here: a walk that only
        // looked for `main` and `origin/develop` would still pass with the two
        // heads attached to the wrong repositories, or with one pair rendered
        // and the other's refs mentioned anywhere at all.
        for delivery in [SkillDelivery::PluginDir, SkillDelivery::Inline] {
            let text = review_prompt(delivery);
            assert!(
                text.contains("/p/backend: main → feature/smetana-pf40"),
                "the first pair: {text}"
            );
            assert!(
                text.contains("/p/frontend: origin/develop → origin/spike"),
                "the second pair: {text}"
            );
            // Each ref on its own as well, so that a change to the line's shape
            // has to face the question of whether all four still travel.
            for git_ref in ["main", "feature/smetana-pf40", "origin/develop", "origin/spike"] {
                assert!(text.contains(git_ref), "{git_ref} is not in the prompt: {text}");
            }
            // Both files, with their extensions: the app composed this path and
            // opens the report at it, so one written and not the other is a tab
            // pointing at nothing.
            assert!(
                text.contains(".smetana/reviews/2026-08-31-pf40.md"),
                "the markdown report: {text}"
            );
            assert!(
                text.contains(".smetana/reviews/2026-08-31-pf40.html"),
                "the html report: {text}"
            );
            // And what the frame it is drawn in cannot do for it.
            assert!(text.contains("sandboxed frame"), "{text}");
        }
    }

    #[test]
    fn a_review_prompt_names_the_skill_and_carries_it_when_it_has_to() {
        // The split every skill in this file is delivered by: a plugin harness
        // is told the name and fetches it, an inline one is handed the text,
        // and an inline one that could not read the file is handed the path so
        // that the session is not left with the rule and none of the method.
        assert!(
            review_prompt(SkillDelivery::PluginDir).contains("smetana:reviewing-branch-changes"),
            "{}",
            review_prompt(SkillDelivery::PluginDir)
        );
        assert!(review_prompt(SkillDelivery::Inline).contains(REVIEWING_BRANCH), "the body");

        let no_skill = build(
            &review(),
            SkillDelivery::Inline,
            ImageDelivery::InPrompt,
            &skills(),
            None,
            nothing(),
            &english(),
            "",
        )
        .unwrap();
        assert!(
            no_skill
                .contains("/app/resources/smetana/skills/reviewing-branch-changes/SKILL.md"),
            "{no_skill}"
        );
    }

    #[test]
    fn a_review_is_told_it_files_nothing_and_commits_nothing() {
        // The two prohibitions are the reason `REVIEW_BRANCH` is a constant at
        // all: an inline harness may find no skill text, and a model that has
        // just read a diff is one step from filing the findings itself. The
        // predicates are asserted beside the prose so that a change to either
        // has to face the other.
        assert!(!writes_to_the_tracker(&review()), "a review files nothing");
        assert!(!commits_to_git(&review()), "a review commits nothing");
        for delivery in [SkillDelivery::PluginDir, SkillDelivery::Inline] {
            let text = review_prompt(delivery);
            assert!(text.contains("File nothing in the tracker"), "{text}");
            assert!(text.contains("make no commit"), "{text}");
        }
    }

    /// A review that was handed no pair is not something the app builds, and
    /// the prompt still has to be true rather than a heading with a gap under
    /// it — the same hole `repair_tracker` fills for an empty stderr.
    #[test]
    fn a_review_with_no_pair_says_so_rather_than_drawing_an_empty_list() {
        let intent = Intent::ReviewBranch {
            pairs: Vec::new(),
            report: ".smetana/reviews/2026-08-31-pf40".into(),
        };
        let text = build(&intent, SkillDelivery::PluginDir, ImageDelivery::InPrompt, &skills(), None, every_skill(), &english(), "")
            .unwrap();
        assert!(text.contains("Nothing was named to compare"), "{text}");
        assert!(!text.contains("What to review"), "no heading over nothing: {text}");
    }

    fn conflict_prompt(op: crate::vcs::model::OpKind, delivery: SkillDelivery) -> String {
        build(&conflict(op), delivery, ImageDelivery::InPrompt, &skills(), None, every_skill(), &english(), "")
            .unwrap()
    }

    #[test]
    fn a_conflict_prompt_names_the_repository_the_branches_and_the_files() {
        // Every one of them is unavailable to the agent by any other route. The
        // repository, because the session's own directory is the project. The
        // branches, because a stopped rebase leaves HEAD detached and the
        // branch it moved off is readable nowhere. The paths, because the list
        // is what git left at that moment and stops being true at the first
        // resolution.
        use crate::vcs::model::OpKind;

        for op in [OpKind::Merge, OpKind::Rebase] {
            for delivery in [SkillDelivery::PluginDir, SkillDelivery::Inline] {
                let text = conflict_prompt(op, delivery);
                assert!(text.contains("/p/backend"), "the repository: {text}");
                assert!(text.contains("main"), "the branch it was on: {text}");
                assert!(text.contains("develop"), "the other branch: {text}");
                assert!(
                    text.contains("src/one.rs") && text.contains("src/two.rs"),
                    "every file: {text}"
                );
            }
        }
    }

    #[test]
    fn a_conflict_prompt_puts_the_branches_the_way_the_operation_did() {
        // A merge brings `theirs` into `ours`; a rebase moves `ours` onto
        // `theirs`. An agent handed them the wrong way round resolves every
        // hunk backwards, and nothing downstream would notice.
        use crate::vcs::model::OpKind;

        let merge = conflict_prompt(OpKind::Merge, SkillDelivery::PluginDir);
        assert!(merge.contains("merging develop into main"), "{merge}");

        let rebase = conflict_prompt(OpKind::Rebase, SkillDelivery::PluginDir);
        assert!(rebase.contains("rebasing main onto develop"), "{rebase}");
    }

    /// The person chose this door over the abort button that was beside it. An
    /// agent that tidies up by aborting has undone the only thing it was asked
    /// to do, and the panel would then show a clean tree as if nothing had
    /// happened — the one failure here that looks like success.
    #[test]
    fn a_conflict_prompt_forbids_aborting() {
        use crate::vcs::model::OpKind;

        for op in [OpKind::Merge, OpKind::Rebase] {
            let text = conflict_prompt(op, SkillDelivery::PluginDir);
            assert!(text.contains("--abort"), "it names the thing not to do: {text}");
            assert!(text.contains("Do not abort"), "{text}");
        }
    }

    #[test]
    fn a_conflict_in_a_repository_with_no_branch_name_still_reads_as_a_sentence() {
        // The panel sends an empty name rather than inventing one when HEAD is
        // already detached or the branch list had not landed. "merging develop
        // into ." would be the alternative.
        use crate::vcs::model::OpKind;

        let intent = Intent::ResolveConflict {
            repo: "/p/backend".into(),
            op: OpKind::Merge,
            ours: String::new(),
            theirs: "develop".into(),
            files: vec!["src/one.rs".into()],
        };
        let text =
            build(&intent, SkillDelivery::PluginDir, ImageDelivery::InPrompt, &skills(), None, nothing(), &english(), "")
                .unwrap();
        assert!(text.contains("merging develop into the branch it is on"), "{text}");
    }

    #[test]
    fn a_conflict_prompt_names_no_skill_at_all() {
        // Nothing was added to the library for this, and `smetana:merging` is
        // the wrong process rather than the near one: it is about a task's
        // worktrees, its gates and its fast-forward, none of which is what a
        // person pressing merge in the panel asked for.
        use crate::vcs::model::OpKind;

        for delivery in [SkillDelivery::PluginDir, SkillDelivery::Inline] {
            let text = conflict_prompt(OpKind::Merge, delivery);
            assert!(!text.contains("smetana:"), "{delivery:?}: {text}");
            assert!(!text.contains("SKILL.md"), "{delivery:?}: {text}");
            // And it is not a session that writes into bd either, so the
            // paragraph about how to word an issue has no business here.
            assert!(!text.contains("bd create --validate"), "{delivery:?}: {text}");
        }
    }

    #[test]
    fn resolving_names_the_issue_and_says_where_the_questions_are() {
        // The questions are deliberately not in the payload — they are the
        // issue's own notes — so the prompt has to say so, or the agent hunts
        // for them in the description and answers the wrong thing.
        let text = resolving(SkillDelivery::PluginDir, nothing());
        assert!(text.contains("smetana-29j"), "{text}");
        assert!(text.contains("Show the state"), "{text}");
        assert!(text.contains("parked:"), "{text}");
    }

    #[test]
    fn resolving_survives_a_skill_that_cannot_be_read() {
        // The three rules that make this session what it is stand on their own,
        // the way DISCUSS and STANDARD do: an Inline harness may find no file.
        for delivery in [SkillDelivery::PluginDir, SkillDelivery::Inline] {
            let text = resolving(delivery, nothing());
            assert!(text.contains(RESOLVE), "{delivery:?}: {text}");
            assert!(text.contains(RESOLVE_WRITE), "{delivery:?}: {text}");
            assert!(text.contains(RESOLVE_GIVE_UP), "{delivery:?}: {text}");
        }
    }

    #[test]
    fn resolving_writes_the_status_last_and_never_invents_an_answer() {
        // The two failures this session has: unparking a task whose answer went
        // nowhere, and answering on the person's behalf — which is the very
        // thing the agent that parked it refused to do.
        let text = resolving(SkillDelivery::PluginDir, nothing());
        assert!(text.contains("bd update <id> --status open"), "{text}");
        assert!(text.contains("That write is last"), "{text}");
        assert!(text.contains("one at a time"), "{text}");
        assert!(text.contains("Answer none of them yourself"), "{text}");
    }

    #[test]
    fn resolving_reaches_each_harness_the_way_that_harness_takes_a_skill() {
        let named = resolving(SkillDelivery::PluginDir, every_skill());
        assert!(named.contains("smetana:resolving-questions"), "{named}");
        assert!(!named.contains(RESOLVING), "a registry carries no skill body: {named}");

        // The body rather than the path, unlike an Auto stage: there is no
        // branch of this session in which the process goes unread.
        let carried = resolving(SkillDelivery::Inline, every_skill());
        assert!(carried.contains(RESOLVING), "{carried}");
        assert!(!carried.contains("smetana:resolving-questions"), "{carried}");

        // And the path when the file could not be read, which is an ordinary
        // outcome rather than an error.
        let pointed = resolving(SkillDelivery::Inline, nothing());
        assert!(
            pointed.contains("/app/resources/smetana/skills/resolving-questions/SKILL.md"),
            "{pointed}"
        );
    }

    #[test]
    fn resolving_is_never_handed_the_filing_skill_or_asked_to_file_anything() {
        // It updates one issue that already exists. `STANDARD` names `bd create`
        // and would send this session to validate a call it is not making.
        let text = resolving(SkillDelivery::Inline, every_skill());
        assert!(!text.contains(STANDARD), "{text}");
        assert!(!text.contains(FILING), "{text}");
        no_paperwork(&text, "resolving");
    }

    #[test]
    fn no_other_intent_is_told_to_unpark_anything() {
        // The status write is the dangerous half: leaking it would have an edit
        // session or a setup put a parked task back in the queue.
        for intent in [
            Intent::Bare,
            Intent::Setup,
            Intent::EditTask { id: "x-1".into(), title: "T".into() },
            run_intent(run_settings(RunMode::Auto, RunScope::Queue)),
        ] {
            for delivery in [SkillDelivery::PluginDir, SkillDelivery::Inline] {
                let text =
                    build(&intent, delivery, ImageDelivery::InPrompt, &skills(), Some(FACTS), every_skill(), &english(), "")
                        .unwrap_or_default();
                assert!(!text.contains(RESOLVE_WRITE), "{intent:?}/{delivery:?}: {text}");
                assert!(!text.contains("smetana:resolving-questions"), "{intent:?}/{delivery:?}");
            }
        }
    }

    fn drafted(draft: TaskDraft) -> String {
        let intent =
            Intent::NewTask { brainstorm: Stage::Off, spec: Stage::Off, plan: Stage::Off, draft };
        build(&intent, SkillDelivery::PluginDir, ImageDelivery::InPrompt, &skills(), None, nothing(), &english(), "").unwrap()
    }

    #[test]
    fn the_persons_own_words_reach_the_agent_whole() {
        let text = drafted(TaskDraft {
            text: "  The board flashes twice.\n\nIt should flash once.  ".into(),
            ..draft()
        });
        // Whole, including the blank line: what a person typed into a
        // multi-line field is one piece of prose, not a title with an
        // afterthought, and only the trailing whitespace is ours to drop.
        assert!(text.contains("The board flashes twice.\n\nIt should flash once.\n\n"));
        assert!(!text.contains("  The board"), "the leading padding is not the person's text");
    }

    #[test]
    fn a_pinned_type_and_priority_are_stated_as_settled() {
        let text = drafted(draft());
        assert!(text.contains("File it with type bug and priority P2."), "{text}");
        assert!(!text.contains("Decide the"), "nothing is left to the agent here");
    }

    #[test]
    fn auto_hands_the_field_to_the_agent_by_name() {
        // Both on Auto, then one of each: an agent told nothing about a field
        // would still have to choose, and would not know that choosing was
        // its job — so every combination says which fields are its to decide.
        let both = drafted(TaskDraft { issue_type: None, priority: None, ..draft() });
        assert!(both.contains("Decide the type and the priority yourself"), "{both}");
        assert!(!both.contains("File it with"), "nothing was pinned");

        let typed = drafted(TaskDraft { priority: None, ..draft() });
        assert!(typed.contains("File it with type bug."), "{typed}");
        assert!(typed.contains("Decide the priority yourself"), "{typed}");

        let prioritised = drafted(TaskDraft { issue_type: None, ..draft() });
        assert!(prioritised.contains("File it with priority P2."), "{prioritised}");
        assert!(prioritised.contains("Decide the type yourself"), "{prioritised}");
    }

    #[test]
    fn a_follow_up_names_its_parent_and_the_exact_dependency_flag() {
        // The bare id and not `blocks:<id>`. Verified on the pinned sidecar: the
        // `type:id` form reads as "this issue IS that type towards that id", so
        // `blocks:<parent>` says the new task blocks the parent — the edge
        // backwards, and it reads like the right thing.
        let text = drafted(TaskDraft { parent: Some("smetana-3uv".into()), ..draft() });
        assert!(text.contains("smetana-3uv"), "{text}");
        assert!(text.contains("--deps smetana-3uv"), "{text}");
        assert!(!text.contains("--deps blocks:"), "{text}");
    }

    #[test]
    fn a_follow_up_is_told_which_branch_its_work_belongs_on() {
        // The one thing an implementer cannot work out from the board: a run's
        // target branch is picked by hand when it is started.
        let text = drafted(TaskDraft { parent: Some("smetana-3uv".into()), ..draft() });
        assert!(text.contains("branch that already carries smetana-3uv's changes"), "{text}");
    }

    #[test]
    fn a_follow_up_is_told_not_to_set_a_status() {
        // bd never clears its stored `blocked` status itself, so an agent being
        // helpful there would strand the task out of `bd ready` for good.
        let text = drafted(TaskDraft { parent: Some("smetana-3uv".into()), ..draft() });
        assert!(text.contains("Do not set any status on it"), "{text}");
    }

    #[test]
    fn an_ordinary_filing_says_nothing_about_a_parent() {
        // Absence is the common case and must leave the prompt exactly as it
        // was: prose about a relationship nobody asked for is worse than none.
        let text = drafted(draft());
        assert!(!text.contains("follow-up"), "{text}");
        assert!(!text.contains("--deps"), "{text}");
    }

    fn with_images(image_delivery: ImageDelivery) -> String {
        let intent = Intent::NewTask {
            brainstorm: Stage::Off,
            spec: Stage::Off,
            plan: Stage::Off,
            draft: TaskDraft {
                images: vec![
                    "/data/attachments/20260806-121314-mock.png".into(),
                    "/data/attachments/20260806-121315-flow.png".into(),
                ],
                ..draft()
            },
        };
        build(&intent, SkillDelivery::PluginDir, image_delivery, &skills(), None, nothing(), &english(), "").unwrap()
    }

    #[test]
    fn attached_images_reach_the_prompt_by_path_in_either_delivery() {
        // Even the harness that is handed the files on its command line is told
        // the paths: they have to end up in the issue description, and the
        // command line is not somewhere the agent can read them back from.
        for delivery in [ImageDelivery::InPrompt, ImageDelivery::Flag("-i")] {
            let text = with_images(delivery);
            assert!(text.contains("/data/attachments/20260806-121314-mock.png"), "{delivery:?}: {text}");
            assert!(text.contains("/data/attachments/20260806-121315-flow.png"), "{delivery:?}: {text}");
        }
    }

    #[test]
    fn both_the_paths_and_a_description_of_them_are_demanded() {
        // The whole point of the feature: a described mock is not a mock, and a
        // path is nothing on a machine that does not have the file. Checking
        // against the constant rather than a retyped substring, the same as the
        // brainstorming prose is checked.
        for delivery in [ImageDelivery::InPrompt, ImageDelivery::Flag("-i")] {
            assert!(with_images(delivery).contains(IMAGES), "{delivery:?}");
        }
    }

    #[test]
    fn a_harness_holding_the_files_is_not_sent_to_open_them_and_one_without_is() {
        assert!(with_images(ImageDelivery::InPrompt).contains("Open and look at them"));
        let flagged = with_images(ImageDelivery::Flag("-i"));
        assert!(!flagged.contains("Open and look at them"), "{flagged}");
        assert!(flagged.contains("attached to this session"), "{flagged}");
    }

    #[test]
    fn one_image_is_spoken_of_in_the_singular() {
        // A prompt reading "There are images ... at these absolute paths" over a
        // single line is a prompt somebody wrote without looking, the same
        // objection the run's "at most 1 tasks" already carries.
        let intent = Intent::NewTask {
            brainstorm: Stage::Off,
            spec: Stage::Off,
            plan: Stage::Off,
            draft: TaskDraft { images: vec!["/data/a.png".into()], ..draft() },
        };
        let text =
            build(&intent, SkillDelivery::PluginDir, ImageDelivery::InPrompt, &skills(), None, nothing(), &english(), "")
                .unwrap();
        assert!(text.contains("There is an image attached to this task, at this absolute path"), "{text}");
        assert!(text.contains("Open and look at it before"), "{text}");
    }

    #[test]
    fn a_task_with_no_images_says_nothing_at_all_about_images() {
        // A heading with nothing under it would have the agent looking for
        // files nobody attached.
        let text = drafted(draft());
        assert!(!text.contains(IMAGES), "{text}");
        assert!(!text.contains("attached"), "{text}");
    }

    #[test]
    fn switched_off_it_asks_for_no_discussion() {
        // Checking against the constants themselves, not a retyped substring,
        // is what keeps this test from drifting away from the prose it
        // guards: neither DISCUSS nor JUDGE contains the word "brainstorm",
        // so a leak of either into the Off arm would say nothing about the
        // process and still pass a substring check on that word alone.
        for delivery in [SkillDelivery::PluginDir, SkillDelivery::Inline] {
            let text = build(&new_task(Stage::Off), delivery, ImageDelivery::InPrompt, &skills(), None, every_skill(), &english(), "").unwrap();
            assert!(!text.contains(DISCUSS), "{delivery:?}: off must not carry the discussion prose");
            assert!(!text.contains(JUDGE), "{delivery:?}: off must not carry the judgement prose");
        }
    }

    #[test]
    fn the_standard_holds_in_every_position_of_the_switch_and_both_deliveries() {
        // The whole point of the feature: a task filed thin costs a night, so
        // the bar is not the filing skill's to keep alone. An Inline harness
        // may find no skill text at all — `nothing()` is that case — and the
        // standard still has to reach the agent, which is why it is a constant
        // in the prompt rather than a paragraph in a file that may be missing.
        for delivery in [SkillDelivery::PluginDir, SkillDelivery::Inline] {
            for mode in [Stage::Off, Stage::Auto, Stage::On] {
                let text =
                    build(&new_task(mode), delivery, ImageDelivery::InPrompt, &skills(), None, nothing(), &english(), "")
                        .unwrap();
                assert!(text.contains(STANDARD), "{delivery:?}/{mode:?}: {text}");
            }
        }
    }

    #[test]
    fn the_standard_is_only_asked_of_a_task_being_filed() {
        // It names `bd create`, which none of the others runs: a run files
        // through `running-tasks` under its own rules, and editing an issue is
        // an update. Leaking it would tell those sessions to validate a call
        // they are not making.
        for intent in [Intent::Bare, Intent::Setup, Intent::EditTask { id: "x-1".into(), title: "T".into() }] {
            let text = build(&intent, SkillDelivery::Inline, ImageDelivery::InPrompt, &skills(), None, every_skill(), &english(), "")
                .unwrap_or_default();
            assert!(!text.contains(STANDARD), "{intent:?}: {text}");
        }
    }

    #[test]
    fn a_plugin_dir_harness_is_told_the_filing_skill_by_name() {
        // Mirrors an_inline_harness_carries_the_filing_skill_whatever_the_switch_says
        // from the PluginDir side of the same guarantee: filing applies to
        // every NewTask whatever the switch says.
        for mode in [Stage::Off, Stage::Auto, Stage::On] {
            let text = build(&new_task(mode), SkillDelivery::PluginDir, ImageDelivery::InPrompt, &skills(), None, every_skill(), &english(), "").unwrap();
            assert!(text.contains("smetana:filing-a-task"), "{mode:?}");
            assert!(!text.contains(FILING), "{mode:?}: no registry should carry the skill body");
        }
    }

    #[test]
    fn an_inline_harness_carries_the_filing_skill_whatever_the_switch_says() {
        // The rules for filing a task are not part of the brainstorming
        // question: an agent that files without discussion still has to file
        // it properly.
        for mode in [Stage::Off, Stage::Auto, Stage::On] {
            let text = build(&new_task(mode), SkillDelivery::Inline, ImageDelivery::InPrompt, &skills(), None, every_skill(), &english(), "").unwrap();
            assert!(text.contains("The title says what needs doing"), "{mode:?}");
            assert!(!text.contains("smetana:filing-a-task"), "{mode:?}: no registry to name");
        }
    }

    #[test]
    fn switched_on_a_plugin_dir_harness_is_told_the_skill_name() {
        let text =
            build(&new_task(Stage::On), SkillDelivery::PluginDir, ImageDelivery::InPrompt, &skills(), None, nothing(), &english(), "").unwrap();
        assert!(text.contains("superpowers:brainstorming"));
    }

    #[test]
    fn switched_on_an_inline_harness_carries_the_whole_process() {
        let text = build(&new_task(Stage::On), SkillDelivery::Inline, ImageDelivery::InPrompt, &skills(), None, every_skill(), &english(), "").unwrap();
        assert!(text.contains("Ask one question at a time."));
        assert!(
            !text.contains("superpowers:brainstorming"),
            "an inline harness has no skill registry"
        );
    }

    #[test]
    fn on_inline_degrades_to_the_rule_when_the_skill_cannot_be_read() {
        let text =
            build(&new_task(Stage::On), SkillDelivery::Inline, ImageDelivery::InPrompt, &skills(), None, nothing(), &english(), "").unwrap();
        assert!(text.contains("agree the design"), "the instruction survives a missing file");
    }

    #[test]
    fn auto_leaves_the_judgement_to_the_agent() {
        let text =
            build(&new_task(Stage::Auto), SkillDelivery::PluginDir, ImageDelivery::InPrompt, &skills(), None, nothing(), &english(), "").unwrap();
        assert!(text.contains("more than one"), "auto states the test the agent applies");
    }

    #[test]
    fn auto_on_an_inline_harness_points_at_the_file_rather_than_pasting_it() {
        let text = build(&new_task(Stage::Auto), SkillDelivery::Inline, ImageDelivery::InPrompt, &skills(), None, every_skill(), &english(), "").unwrap();
        assert!(text.contains("/app/resources/superpowers/skills/brainstorming/SKILL.md"));
        assert!(
            !text.contains("Ask one question at a time."),
            "auto must not pay for 10 KB the agent may not use"
        );
    }

    /// A prompt for a filing session with all three switches set.
    fn staged_prompt(
        brainstorm: Stage,
        spec: Stage,
        plan: Stage,
        delivery: SkillDelivery,
    ) -> String {
        build(
            &staged(brainstorm, spec, plan),
            delivery,
            ImageDelivery::InPrompt,
            &skills(),
            None,
            every_skill(),
            &english(),
            "",
        )
        .unwrap()
    }

    /// Every piece of prose the two stages can produce. Checked against the
    /// constants rather than retyped substrings, for the reason
    /// `switched_off_it_asks_for_no_discussion` gives: a retyped fragment
    /// drifts away from the prose it guards and keeps passing.
    fn no_paperwork(text: &str, whose: &str) {
        for prose in [SPEC, SPEC_JUDGE, PLAN, PLAN_JUDGE, PAPERWORK] {
            assert!(!text.contains(prose), "{whose} must carry no paperwork prose: {text}");
        }
    }

    #[test]
    fn both_stages_off_ask_for_no_documents_at_all() {
        // A heading with nothing under it is the images objection again: an
        // agent told where a design document would go starts looking for a
        // reason to write one. Every way of arriving at Off is here — the
        // switches themselves, a discussion switched off under them, and a
        // spec switched off under the discussion.
        for delivery in [SkillDelivery::PluginDir, SkillDelivery::Inline] {
            for (brainstorm, spec, plan) in [
                (Stage::Off, Stage::Off, Stage::Off),
                (Stage::Off, Stage::On, Stage::On),
                (Stage::On, Stage::Off, Stage::Off),
                (Stage::On, Stage::Off, Stage::On),
            ] {
                let text = staged_prompt(brainstorm, spec, plan, delivery);
                no_paperwork(&text, &format!("{delivery:?}/{brainstorm:?}/{spec:?}/{plan:?}"));
            }
        }
    }

    #[test]
    fn no_other_intent_is_asked_for_a_spec_or_a_plan() {
        // The two stages belong to filing and to nothing else: a run files
        // through `running-tasks` under its own rules, editing an issue is an
        // update, and a setup session writes one named file.
        for intent in [
            Intent::Bare,
            Intent::Setup,
            Intent::EditTask { id: "x-1".into(), title: "T".into() },
            run_intent(run_settings(RunMode::Auto, RunScope::Queue)),
            run_intent(run_settings(RunMode::Solo, RunScope::Task { id: "x-2".into() })),
        ] {
            for delivery in [SkillDelivery::PluginDir, SkillDelivery::Inline] {
                let text =
                    build(&intent, delivery, ImageDelivery::InPrompt, &skills(), Some(FACTS), every_skill(), &english(), "")
                        .unwrap_or_default();
                no_paperwork(&text, &format!("{intent:?}/{delivery:?}"));
            }
        }
    }

    #[test]
    fn the_cascade_is_applied_before_any_prose_is_written() {
        // The payload is not the screen: a person can turn Brainstorming off
        // after choosing a spec, and what arrives here would then ask for a
        // design document from a session that discusses nothing.
        for delivery in [SkillDelivery::PluginDir, SkillDelivery::Inline] {
            let orphaned = staged_prompt(Stage::Off, Stage::On, Stage::On, delivery);
            no_paperwork(&orphaned, &format!("{delivery:?}: spec On under brainstorming Off"));

            // And one level down: a plan under a spec nobody settled is the
            // same defect one link along the chain.
            let planless = staged_prompt(Stage::On, Stage::Off, Stage::On, delivery);
            assert!(!planless.contains(PLAN), "{delivery:?}: {planless}");
            assert!(!planless.contains(PLAN_JUDGE), "{delivery:?}: {planless}");
            no_paperwork(&planless, &format!("{delivery:?}: plan On under spec Off"));
        }
    }

    #[test]
    fn each_document_is_named_by_the_path_it_goes_to() {
        // The one thing the agent cannot work out for itself: superpowers puts
        // these at the project root, and this app wants them under .smetana/.
        let text = staged_prompt(Stage::On, Stage::On, Stage::On, SkillDelivery::PluginDir);
        assert!(
            text.contains(".smetana/docs/superpowers/specs/YYYY-MM-DD-<topic>-design.md"),
            "{text}"
        );
        assert!(text.contains(".smetana/docs/plans/YYYY-MM-DD-<topic>.md"), "{text}");

        // An Auto stage names the same place — the judgement is whether to
        // write one, never where it goes.
        let judged = staged_prompt(Stage::On, Stage::Auto, Stage::Auto, SkillDelivery::PluginDir);
        assert!(
            judged.contains(".smetana/docs/superpowers/specs/YYYY-MM-DD-<topic>-design.md"),
            "{judged}"
        );
        assert!(judged.contains(".smetana/docs/plans/YYYY-MM-DD-<topic>.md"), "{judged}");
    }

    #[test]
    fn the_task_is_filed_last_and_the_paths_reach_the_issue_absolute() {
        // Filing last is what makes an interrupted session leave nothing
        // behind; the absolute paths are what make the files reachable from
        // the worktree an implementer actually stands in, since an ignored
        // folder does not travel into one.
        for stages in [(Stage::On, Stage::On), (Stage::Auto, Stage::Auto), (Stage::On, Stage::Off)] {
            let text = staged_prompt(Stage::On, stages.0, stages.1, SkillDelivery::PluginDir);
            assert!(text.contains(PAPERWORK), "{stages:?}: {text}");
        }
    }

    #[test]
    fn nothing_is_ever_asked_to_be_committed() {
        // `.smetana/` is kept out of the repository by runs::gitignore, so
        // there is nothing to commit — and an agent that tried would be
        // fighting a rule the app enforces in code.
        let text = staged_prompt(Stage::On, Stage::On, Stage::On, SkillDelivery::PluginDir);
        assert!(text.contains("not in the repository"), "{text}");
        assert!(text.contains("do not try to commit it"), "{text}");
        assert!(text.contains("stand on that alone"), "the issue owes its own prose: {text}");
    }

    #[test]
    fn a_plan_reaches_each_harness_the_way_that_harness_takes_a_skill() {
        // The same trade `Stage` already makes, one stage down: a name
        // for a registry, the body for a harness without one, and only where
        // the stage was actually asked for.
        let named = staged_prompt(Stage::On, Stage::On, Stage::On, SkillDelivery::PluginDir);
        assert!(named.contains("superpowers:writing-plans"), "{named}");
        assert!(!named.contains(PLANS), "a registry carries no skill body: {named}");

        let carried = staged_prompt(Stage::On, Stage::On, Stage::On, SkillDelivery::Inline);
        assert!(carried.contains(PLANS), "{carried}");
        assert!(
            !carried.contains("superpowers:writing-plans"),
            "an inline harness has no skill registry: {carried}"
        );
    }

    #[test]
    fn an_auto_plan_is_pointed_at_the_file_rather_than_handed_it() {
        // 7 KB the agent may decline to use, against one line naming where it
        // is — the same choice `Stage::Auto` makes for brainstorming.
        let text = staged_prompt(Stage::On, Stage::On, Stage::Auto, SkillDelivery::Inline);
        assert!(
            text.contains("/app/resources/superpowers/skills/writing-plans/SKILL.md"),
            "{text}"
        );
        assert!(!text.contains(PLANS), "auto must not pay for a skill it may not use: {text}");
    }

    #[test]
    fn an_asked_for_plan_survives_a_skill_that_cannot_be_read() {
        // The instruction stands on its own, the way DISCUSS does: an Inline
        // harness may find no file there at all.
        let text = build(
            &staged(Stage::On, Stage::On, Stage::On),
            SkillDelivery::Inline,
            ImageDelivery::InPrompt,
            &skills(),
            None,
            nothing(),
            &english(),
            "",
        )
        .unwrap();
        assert!(text.contains(PLAN), "{text}");
        assert!(text.contains(PAPERWORK), "{text}");
    }

    const FACTS: &str = "- backend — npm\n    npm run test\n";

    #[test]
    fn setting_a_project_up_carries_the_survey_and_names_the_file_to_write() {
        let text =
            build(&Intent::Setup, SkillDelivery::PluginDir, ImageDelivery::InPrompt, &skills(), Some(FACTS), nothing(), &english(), "")
                .expect("a setup session opens on something");
        assert!(text.contains(".smetana/project.toml"), "{text}");
        assert!(text.contains("npm run test"), "the survey reaches the agent: {text}");
    }

    #[test]
    fn a_plugin_dir_harness_is_told_the_setup_skill_by_name() {
        let text =
            build(&Intent::Setup, SkillDelivery::PluginDir, ImageDelivery::InPrompt, &skills(), Some(FACTS), nothing(), &english(), "")
                .expect("builds");
        assert!(text.contains("smetana:project-setup"), "{text}");
    }

    #[test]
    fn an_inline_harness_is_given_the_setup_skill_s_path_rather_than_its_body() {
        // The same choice `Auto` already makes for brainstorming: a path costs
        // one line, the body costs kilobytes the session may never need.
        let text = build(
            &Intent::Setup,
            SkillDelivery::Inline,
            ImageDelivery::InPrompt,
            &skills(),
            Some(FACTS),
            every_skill(),
            &english(),
            "",
        )
        .expect("builds");
        assert!(text.contains("/app/resources/smetana/skills/project-setup/SKILL.md"), "{text}");
        assert!(!text.contains("The title says what needs doing"), "nothing is filed here");
    }

    /// Every intent there is, in both deliveries — the walk the language tests
    /// below share, since what makes a language rule worth anything is that no
    /// session escapes it.
    fn every_intent() -> Vec<Intent> {
        vec![
            Intent::Bare,
            Intent::Setup,
            Intent::EditTask { id: "x-1".into(), title: "T".into() },
            Intent::ResolveTask { id: "x-1".into(), title: "T".into() },
            Intent::FixTask { id: "x-1".into(), title: "T".into() },
            repair(),
            review(),
            new_task(Stage::Auto),
            new_task(Stage::On),
            new_task(Stage::Off),
            staged(Stage::On, Stage::On, Stage::On),
            run_intent(run_settings(RunMode::Auto, RunScope::Queue)),
        ]
    }

    fn in_language(intent: &Intent, delivery: SkillDelivery, languages: &Languages) -> String {
        build(
            intent,
            delivery,
            ImageDelivery::InPrompt,
            &skills(),
            Some(FACTS),
            every_skill(),
            languages,
            "",
        )
        .expect("every intent opens on at least the language sentence")
    }

    /// The same call as `in_language`, with a standing instruction in it.
    fn with_standing(intent: &Intent, delivery: SkillDelivery, standing: &str) -> String {
        build(
            intent,
            delivery,
            ImageDelivery::InPrompt,
            &skills(),
            Some(FACTS),
            every_skill(),
            &russian(),
            standing,
        )
        .expect("every intent opens on at least the language sentence")
    }

    /// Every intent there is: `every_intent` plus the two conflict kinds it
    /// does not carry. The whole vocabulary, walked rather than listed, and the
    /// chain written once — the tests below wanted it and each had written its
    /// own copy.
    fn every_intent_and_the_conflicts() -> Vec<Intent> {
        every_intent()
            .into_iter()
            .chain([
                conflict(crate::vcs::model::OpKind::Merge),
                conflict(crate::vcs::model::OpKind::Rebase),
            ])
            .collect()
    }

    /// Every intent somebody is present for: the whole vocabulary above minus
    /// the run — walked rather than written out as a list of eight, because a
    /// list goes stale the next time an intent is added and nothing fails when
    /// it does.
    fn every_conversation() -> Vec<Intent> {
        every_intent_and_the_conflicts()
            .into_iter()
            .filter(|intent| !matches!(intent, Intent::Run { .. }))
            .collect()
    }

    #[test]
    fn a_standing_instruction_reaches_every_session_somebody_is_in() {
        for intent in every_conversation() {
            for delivery in [SkillDelivery::PluginDir, SkillDelivery::Inline] {
                let text = with_standing(&intent, delivery, "Always use pnpm.");
                assert!(
                    text.contains("Always use pnpm."),
                    "{intent:?}/{delivery:?}: {text}"
                );
                assert!(text.contains(STANDING), "{intent:?}/{delivery:?}: {text}");
            }
        }
    }

    #[test]
    fn a_standing_instruction_stands_after_the_languages_and_before_the_work() {
        // The order is the decision, not an accident of where the block was
        // pasted: a reader resolves a contradiction in favour of what came
        // later, so the person's own words have to follow the language rules
        // rather than precede them, and both have to precede the work for the
        // reason `build` gives about seven kilobytes of skill text.
        for delivery in [SkillDelivery::PluginDir, SkillDelivery::Inline] {
            let intent = Intent::EditTask { id: "x-1".into(), title: "T".into() };
            let text = with_standing(&intent, delivery, "Always use pnpm.");
            let standing = text.find(STANDING).expect("the framing line is there");
            let language = text.find("Write the prose of any bd issue").expect("a task language");
            let work = text.find("Read the issue first").expect("the work");
            assert!(language < standing, "{delivery:?}: {text}");
            assert!(standing < work, "{delivery:?}: {text}");
        }
    }

    #[test]
    fn a_run_is_never_given_the_standing_instruction() {
        // Nobody is in a run's conversation. An instruction written for one
        // would shape autonomous work overnight with no one to correct it, on
        // top of the four language paragraphs a run already opens with.
        for delivery in [SkillDelivery::PluginDir, SkillDelivery::Inline] {
            let intent = run_intent(run_settings(RunMode::Auto, RunScope::Queue));
            let text = with_standing(&intent, delivery, "Always use pnpm.");
            assert!(!text.contains("Always use pnpm."), "{delivery:?}: {text}");
            assert!(!text.contains(STANDING), "{delivery:?}: {text}");
        }
    }

    #[test]
    fn an_empty_standing_instruction_costs_a_prompt_nothing() {
        // Not one word, and not one blank line: the empty default has to leave
        // every prompt exactly as it was before this field existed, and a
        // stray separator is the way that quietly stops being true.
        //
        // Whitespace counts as empty and is walked with it. A lone space or
        // newline is what a person clearing the field leaves behind as readily
        // as nothing at all, and it must not buy a framing line announcing an
        // instruction that is not there — with a newline it would also put back
        // the very `\n\n\n` this test forbids.
        for intent in every_intent_and_the_conflicts() {
            for delivery in [SkillDelivery::PluginDir, SkillDelivery::Inline] {
                for standing in ["", " ", "\n", "\n\n", "  \t\n "] {
                    let text = with_standing(&intent, delivery, standing);
                    assert!(
                        !text.contains(STANDING),
                        "{intent:?}/{delivery:?}/{standing:?}: {text}"
                    );
                    assert!(
                        !text.contains("\n\n\n"),
                        "an empty instruction left a blank line behind: \
                         {intent:?}/{delivery:?}/{standing:?}: {text}"
                    );
                }
            }
        }
    }

    #[test]
    fn every_intent_names_the_conversation_language() {
        // `Bare` included, and that is the point rather than a side effect: it
        // is the session where a person talks to the agent most, and an English
        // default with no Auto position is what makes the setting real from the
        // first run.
        for intent in every_intent() {
            for delivery in [SkillDelivery::PluginDir, SkillDelivery::Inline] {
                let text = in_language(&intent, delivery, &russian());
                assert!(
                    text.contains("Talk to me in Russian"),
                    "{intent:?}/{delivery:?}: {text}"
                );
            }
        }
    }

    #[test]
    fn only_a_session_that_writes_to_bd_is_told_the_task_language() {
        // The ones that run `bd create` or `bd update` as the work they were
        // opened for, plus `Bare` — in because it is where a person says "file
        // tasks for this", the same reason `commits_to_git` has it, and the
        // case the setting was asked for in the first place. It costs that one
        // session a third paragraph about language, which is cheaper than a
        // bare session filing English issues under a Russian setting. The
        // `matches!` below is the list; no count is written here, because a
        // count is wrong the next time an intent is added and nothing fails
        // when it goes stale.
        //
        // `Setup`, `ResolveConflict` and `RepairTracker` stay out: one writes a
        // toml file, one finishes a merge or a rebase git stopped on, and one
        // is looking at the tracker's own database — none of them files an
        // issue, and the last could not if it wanted to, since bd is what is
        // broken. The paragraph there would be prose about something that will
        // not happen.
        let intents = every_intent_and_the_conflicts();
        for delivery in [SkillDelivery::PluginDir, SkillDelivery::Inline] {
            for intent in &intents {
                let text = in_language(intent, delivery, &russian());
                let told = text.contains("Write the prose of any bd issue");
                let writes = matches!(
                    intent,
                    Intent::NewTask { .. }
                        | Intent::EditTask { .. }
                        | Intent::ResolveTask { .. }
                        | Intent::FixTask { .. }
                        | Intent::Run { .. }
                        | Intent::Bare
                );
                assert_eq!(writes_to_the_tracker(intent), writes, "{intent:?}");
                assert_eq!(told, writes, "{intent:?}/{delivery:?}: {text}");
                if writes {
                    assert!(text.contains("in Russian"), "{intent:?}/{delivery:?}: {text}");
                }
            }
        }
    }

    #[test]
    fn only_a_session_that_touches_git_is_told_the_commit_language() {
        // The ones that make a commit with their own hands: a run's lead
        // commits and merges all night, a conflict session finishes the merge
        // or the rebase git stopped on, and a bare session is where a person
        // says "commit this". The `matches!` below is the list, and no count is
        // written here — the comment that did say one was already off by one
        // before `RepairTracker` made it off by two.
        //
        // The rest write into bd, or into `.smetana/`, or into `.beads` which
        // bd commits for itself, and none of those is a commit of this
        // session's making — telling them how to word one would be a paragraph
        // about something that will not happen.
        let intents = every_intent_and_the_conflicts();
        for delivery in [SkillDelivery::PluginDir, SkillDelivery::Inline] {
            for intent in &intents {
                let commits = matches!(
                    intent,
                    Intent::Run { .. }
                        | Intent::ResolveConflict { .. }
                        | Intent::FixTask { .. }
                        | Intent::Bare
                );
                assert_eq!(commits_to_git(intent), commits, "{intent:?}");

                let text = in_language(intent, delivery, &russian());
                let told = text.contains("Write the message of any git commit");
                assert_eq!(told, commits, "{intent:?}/{delivery:?}: {text}");
                if commits {
                    assert!(text.contains("in Russian"), "{intent:?}/{delivery:?}: {text}");
                }
            }
        }
    }

    #[test]
    fn the_commit_paragraph_protects_a_form_without_legislating_one() {
        // The watershed the section headings hold one field over: what sits in
        // front of the colon is matched and read rather than translated, so the
        // setting moves the prose and leaves that alone.
        //
        // **The absence is the assertion here**, and it is what the earlier
        // version of this paragraph got wrong. Naming `type: subject` and the
        // six Conventional Commits types imposes a convention on somebody
        // else's repository: `smetana:merging` commits `merge: <branch> into
        // <target>`, which is not one of the six, and `smetana:provisioning`
        // greps that subject for the branch name afterwards. A prompt telling
        // the agent the form is something else is a merge subject rewritten and
        // a blocker no longer found.
        for delivery in [SkillDelivery::PluginDir, SkillDelivery::Inline] {
            for intent in [Intent::Bare, conflict(crate::vcs::model::OpKind::Merge), run_intent(run_settings(RunMode::Auto, RunScope::Queue))] {
                let text = in_language(&intent, delivery, &russian());
                assert!(
                    text.contains("what sits in front of the colon stays as it is"),
                    "{intent:?}/{delivery:?}: {text}"
                );
                assert!(
                    text.contains("is a name and travels unchanged"),
                    "{intent:?}/{delivery:?} lets an identifier be translated: {text}"
                );
                assert!(
                    !text.contains("Conventional Commits"),
                    "{intent:?}/{delivery:?} legislates a commit convention: {text}"
                );
                assert!(
                    !text.contains("feat, fix, docs"),
                    "{intent:?}/{delivery:?} names a type set the project may not use: {text}"
                );
                assert!(text.contains("in Russian"), "{intent:?}/{delivery:?}: {text}");
            }
        }
    }

    /// A run session is told; a filing session is not. The two ends of
    /// `commits_to_git`, named on their own so a change to the predicate that
    /// happens to keep the walk above green still has to face these two.
    #[test]
    fn a_run_is_told_the_commit_language_and_a_filing_session_is_not() {
        let run = in_language(
            &run_intent(run_settings(RunMode::Auto, RunScope::Queue)),
            SkillDelivery::PluginDir,
            &russian(),
        );
        assert!(run.contains("Write the message of any git commit"), "{run}");

        let filing = in_language(&new_task(Stage::Off), SkillDelivery::PluginDir, &russian());
        assert!(
            !filing.contains("Write the message of any git commit"),
            "a filing session commits nothing: {filing}"
        );
    }

    #[test]
    fn only_a_run_is_told_the_report_language() {
        // A run's lead is the only session that ever writes a batch file, so it
        // is the only one with anything to word. The predicate is asserted
        // beside the text, in both deliveries, so that a change to either has
        // to face the other.
        //
        // One conflict kind rather than the `every_intent_and_the_conflicts()`
        // the two tests above walk, and deliberately: `leaves_a_run_report`
        // cannot tell a merge from a rebase, so the second kind would be the
        // same assertion twice. Not an oversight to sweep up into that helper.
        let intents: Vec<Intent> = every_intent()
            .into_iter()
            .chain([conflict(crate::vcs::model::OpKind::Merge)])
            .collect();
        for delivery in [SkillDelivery::PluginDir, SkillDelivery::Inline] {
            for intent in &intents {
                let leaves = matches!(intent, Intent::Run { .. });
                assert_eq!(leaves_a_run_report(intent), leaves, "{intent:?}");

                let text = in_language(intent, delivery, &russian());
                let told = text.contains("Write the prose of the batch file");
                assert_eq!(told, leaves, "{intent:?}/{delivery:?}: {text}");
                if leaves {
                    assert!(text.contains("in Russian"), "{intent:?}/{delivery:?}: {text}");
                }
            }
        }
    }

    #[test]
    fn the_report_paragraph_keeps_the_json_keys_and_names_the_other_report() {
        // Asserted against the predicate's own text rather than against the
        // whole prompt, and that is the point of the test: the shape of the
        // file is already printed a paragraph below in the `Run` prompt, so a
        // walk over the whole text would pass with the exception missing
        // altogether.
        //
        // `report::parse_batch` reads these four through serde by literal
        // match, so a translated key is not a document in another language — it
        // is a batch drawn as having left no account of itself. And the last
        // assertion is the other half: two reports come out of a batch, and
        // somebody who set this and watched the terminal would otherwise have
        // been told nothing at all.
        let text = report_language("Russian");
        for key in ["`tasks`", "`id`", "`did`", "`notes`"] {
            assert!(text.contains(key), "{key} is not named as staying put: {text}");
        }
        assert!(text.contains("in Russian"), "{text}");
        assert!(
            text.contains("separate report") && text.contains("language of the conversation"),
            "the account given in the conversation is not named as its own report: {text}"
        );
    }

    #[test]
    fn the_report_language_moves_no_word_the_document_writes_itself() {
        // `runs::report` renders its own labels and this setting must not be
        // read as an instruction about them: they are interface copy, CLAUDE.md
        // says interface copy is English, and no agent writes them. A prompt
        // that started naming them would be the first step towards a table of
        // twelve translations in Rust.
        let text = report_language("Russian");
        for label in ["run report", "closed", "parked", "batches"] {
            assert!(!text.contains(label), "{label} is named as if it moved: {text}");
        }
    }

    #[test]
    fn the_section_headings_stay_english_whatever_the_task_language_is() {
        // `bd create --validate` matches the wording of a heading and nothing
        // else, so a translated `## Acceptance Criteria` is not a difference of
        // style — it is bd refusing to create the issue. The literal strings
        // are asserted rather than the caveat's presence, because the caveat is
        // only worth having if the headings themselves travel intact.
        for delivery in [SkillDelivery::PluginDir, SkillDelivery::Inline] {
            for intent in every_intent() {
                let text = in_language(&intent, delivery, &russian());
                if !text.contains("Write the prose of any bd issue") {
                    assert!(!text.contains("## "), "{intent:?}/{delivery:?} names a heading: {text}");
                    continue;
                }
                assert!(text.contains("stay in English"), "{intent:?}/{delivery:?}: {text}");
                assert!(text.contains("## Acceptance Criteria"), "{intent:?}/{delivery:?}: {text}");
                assert!(text.contains("## Steps to Reproduce"), "{intent:?}/{delivery:?}: {text}");
                assert!(text.contains("## Success Criteria"), "{intent:?}/{delivery:?}: {text}");
            }
        }
    }

    #[test]
    fn the_notes_markers_stay_english_whatever_the_task_language_is() {
        // The same class of literal-string dependency the headings are, and
        // this paragraph is what opens the hole by asking for the notes in
        // another language. `components/kanban/parked.js` matches
        // `/^\s*parked:\s*(.+)$/i` and `/^\s*resolved:\s*/i`, so a translated
        // marker leaves `openQuestions` empty: the parked card's dialog says
        // nothing is open and moving it to Ready stops warning.
        for delivery in [SkillDelivery::PluginDir, SkillDelivery::Inline] {
            for intent in every_intent() {
                let text = in_language(&intent, delivery, &russian());
                if !text.contains("Write the prose of any bd issue") {
                    continue;
                }
                assert!(text.contains("`parked:`"), "{intent:?}/{delivery:?}: {text}");
                assert!(text.contains("`resolved:`"), "{intent:?}/{delivery:?}: {text}");
                assert!(
                    text.contains("in English"),
                    "{intent:?}/{delivery:?} asks for the markers untranslated: {text}"
                );
            }
        }

        // A resolving session is the one that writes `resolved:` lines, so the
        // rule has to reach it whatever else its prompt says.
        let resolving = in_language(
            &Intent::ResolveTask { id: "x-1".into(), title: "T".into() },
            SkillDelivery::Inline,
            &russian(),
        );
        assert!(resolving.contains("`resolved:`"), "{resolving}");
    }

    #[test]
    fn a_design_document_and_a_plan_are_always_english() {
        // The one piece of writing neither setting moves: both are read by
        // whoever picks the work up months later and by every agent after them,
        // and the repository they sit beside is English throughout.
        for delivery in [SkillDelivery::PluginDir, SkillDelivery::Inline] {
            for languages in [
                english(),
                russian(),
                Languages {
                    agent: "ja".into(),
                    task: "de".into(),
                    commit: "it".into(),
                    report: "ko".into(),
                },
            ] {
                for (spec, plan) in
                    [(Stage::On, Stage::On), (Stage::On, Stage::Off), (Stage::Auto, Stage::Auto)]
                {
                    let text =
                        in_language(&staged(Stage::On, spec, plan), delivery, &languages);
                    assert!(text.contains(IN_ENGLISH), "{delivery:?}/{languages:?}/{spec:?}: {text}");
                    assert!(text.contains("is in English"), "{delivery:?}/{languages:?}: {text}");
                }
            }
        }
    }

    #[test]
    fn a_language_nobody_ships_reads_as_the_default_rather_than_as_itself() {
        // `Settings::validate` drops one on the way to the file, but this
        // function is pure and takes what it is handed — and a tag written into
        // the prompt raw would be an instruction nobody can follow.
        let text = in_language(
            &Intent::Bare,
            SkillDelivery::PluginDir,
            &Languages {
                agent: "xx".into(),
                task: "xx".into(),
                commit: "xx".into(),
                report: "xx".into(),
            },
        );
        assert!(text.contains("Talk to me in English"), "{text}");
        assert!(!text.contains("xx"), "{text}");
    }

    #[test]
    fn a_setup_session_survives_a_survey_that_found_nothing() {
        // `render` always produces text, but a caller that could not run the
        // survey at all passes None, and the instruction still has to stand.
        let text = build(&Intent::Setup, SkillDelivery::PluginDir, ImageDelivery::InPrompt, &skills(), None, nothing(), &english(), "")
            .expect("builds");
        assert!(text.contains(".smetana/project.toml"), "{text}");
    }
}
