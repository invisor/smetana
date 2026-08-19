//! Thin commands with no state of their own, shaped exactly like `files/`'s.
//!
//! The project and the repository arrive as paths from the front end, which
//! knows both anyway: keeping a second copy of that knowledge here would mean
//! taking a dependency on the tracker for a value that is not this module's.

use std::path::Path;

use super::model::{Branch, ChangeKind, MergeOutcome, OpKind, Repo, Tracking, VcsError, WorkingTree};
use super::run::Attempt;
use super::{model, repos, run};
use crate::agents::oneshot::{self, OneshotError};
use crate::files::model::{looks_binary, BINARY_SNIFF_BYTES, MAX_FILE_BYTES};
use crate::git;

/// **Every command in this file runs its work here, and none of it in the body
/// of the `async fn`.** `spawn_blocking`, rather than making the commands
/// synchronous: Tauri will take either, and this one keeps the door open for a
/// command that has something genuinely asynchronous beside its git call
/// (`vcs_suggest_message` already does).
///
/// The reason used to be the three networked calls alone — they were the first
/// in the module that could take a minute on purpose, where everything else was
/// tens of milliseconds. That argument is now the general one instead of the
/// exception: since `run.rs` gained a ceiling for local reads and writes too,
/// **every** call here has a length this app has committed to waiting, up to
/// five minutes for a write sitting on somebody's `pre-commit` — and it is this
/// wrapper that makes a ceiling that long affordable, since a button stuck for
/// five minutes now costs one button. Every IPC call
/// in the app — the file tree, the editor, the tracker, the terminals — shares
/// the runtime these commands are polled on, so a git that is merely slow would
/// otherwise take workers out of everything else on screen with nothing saying
/// why. The blocking pool is where a thread is *meant* to be parked on a
/// process.
async fn off_the_runtime<T, F>(work: F) -> Result<T, VcsError>
where
    F: FnOnce() -> Result<T, VcsError> + Send + 'static,
    T: Send + 'static,
{
    // A blocking task that panicked, or a runtime shutting down under it: an
    // `Io` refusal in this app's own words, since git never said anything.
    tokio::task::spawn_blocking(work).await.unwrap_or_else(|err| Err(VcsError::Io(err.to_string())))
}

/// The same, for the three commands documented as never refusing.
///
/// They have no error to return and that promise is load-bearing — a project
/// holding a folder that is not a repository still draws a list — so a join
/// that failed answers with what "nothing is known" already looks like here:
/// the empty list. It is the same answer those commands give for a folder git
/// cannot read, which is the only other way they come back with nothing.
///
/// **The log line is what keeps that from being a quiet lie.** The empty list
/// says "this folder holds no repository", which is a statement and not a
/// shrug, and a panicked task would have it made about a folder nobody looked
/// at — against this module's own rule that anything unobservable reads as
/// "no", loudly. Nothing on screen can carry it, since the shape of the answer
/// has no room for a refusal, so it goes where a developer will find it.
async fn off_the_runtime_or_empty<T, F>(work: F) -> T
where
    F: FnOnce() -> T + Send + 'static,
    T: Default + Send + 'static,
{
    tokio::task::spawn_blocking(work).await.unwrap_or_else(|err| {
        log::error!("vcs: a read gave way and is being reported as nothing at all: {err}");
        T::default()
    })
}

/// The repositories of a project. Never a refusal: a folder that is not a
/// repository, or holds none, is an empty list, which the panel draws as an
/// empty state of its own.
#[tauri::command]
pub async fn vcs_repos(project: String) -> Vec<Repo> {
    off_the_runtime_or_empty(move || repos::discover(Path::new(&project))).await
}

/// The working tree of one repository.
///
/// `--untracked-files=normal`, git's own default: `all` would walk into every
/// untracked directory, and a person who wants that opens the file tree.
#[tauri::command]
pub async fn vcs_status(repo: String) -> Result<WorkingTree, VcsError> {
    off_the_runtime(move || working_tree(Path::new(&repo))).await
}

/// The porcelain call itself, written once.
///
/// `vcs_status` is one caller and `attempt` below is the other, and they have
/// to be the same call: "did that merge conflict" is the same question about
/// the same records the panel is already drawing, so a second argument list
/// here would be a second answer free to disagree with what is on screen.
fn working_tree(repo: &Path) -> Result<WorkingTree, VcsError> {
    let out = run::git_read(
        repo,
        &["status", "--porcelain=v2", "-z", "--branch", "--untracked-files=normal"],
    )?;
    Ok(model::parse_status(&out))
}

/// The local branches of one repository, most recently worked on first.
///
/// **This one spawns nothing**, and it is the exception the module header's
/// rule survives rather than one it breaks: a branch list is `refs/heads/`,
/// `packed-refs` and the reflogs, all of which `git.rs` already reads off the
/// disk. The command sits here because it is a section of this panel and
/// because `vcs_checkout` beside it is what a person does with the answer —
/// but not one line of the reading is written a second time, and `git.rs` is
/// not touched: `branches_with_recency` gathers the three sources through
/// `parse_commondir`, so a linked worktree offers the whole repository's list
/// and not the one branch it is itself on, and `by_recency` is what orders
/// them. Nothing here re-sorts that: the branch somebody merges into every day
/// is nowhere in particular alphabetically.
///
/// Never a refusal, the same as `vcs_repos` and for the same reason: a folder
/// outside git has no branches to offer, which is an empty list and not an
/// error.
#[tauri::command]
pub async fn vcs_branches(repo: String) -> Vec<Branch> {
    off_the_runtime_or_empty(move || branch_list(Path::new(&repo))).await
}

/// Where every local branch stands against its upstream, in one process.
///
/// **Apart from `vcs_branches` on purpose.** That command's own documentation
/// promises two things this would end: it spawns nothing at all, and it can
/// never refuse. Both are load-bearing — the first is why the branch list is
/// cheap enough for every window focus, the second is why a project holding a
/// folder that is not a repository still draws a list. So the counts arrive as
/// a second answer and the front end merges the two by name; a branch in one
/// and not the other simply draws no mark until the next refresh, which is the
/// freshness this whole panel already promises.
///
/// Never a refusal, for the same reason `vcs_branches` never refuses: a folder
/// git cannot read has no upstreams to report, and the status read beside it is
/// what puts a message on screen.
///
/// `%(upstream:track,nobracket)` rather than `git status`'s `# branch.ab`: that
/// line answers for the current branch alone, and the mark this feeds is drawn
/// on every row.
#[tauri::command]
pub async fn vcs_tracking(repo: String) -> Vec<Tracking> {
    off_the_runtime_or_empty(move || tracking(Path::new(&repo))).await
}

fn tracking(repo: &Path) -> Vec<Tracking> {
    const FORMAT: &str = "%(refname:short)%00%(upstream:short)%00%(upstream:track,nobracket)";
    run::git_read(repo, &["for-each-ref", "--format", FORMAT, "refs/heads"])
        .map(|out| model::parse_tracking(&out))
        .unwrap_or_default()
}

/// Switch the repository to a branch it already has.
///
/// `git checkout --no-guess <branch> --` and nothing more. **Never `--force`**,
/// and the omission is the feature: the two refusals worth having are exactly
/// the ones force would drive over — a branch already checked out in another
/// worktree, which is what a run's `provisioning` phase cuts, and local changes
/// the checkout would have to overwrite.
///
/// Neither is pre-empted here. Asking git whether it would refuse, and then
/// asking it to do the thing, is two answers about one moment with a window
/// between them; and the sentence git gives is better than one written here,
/// because the person reading it knows git. `run.rs` carries it through
/// untouched with git's exit status beside it.
///
/// **The two extra words are what make the first line true, and each closes a
/// different way of not switching a branch at all.** The list a row was picked
/// from has no watcher behind it, and a run's `merging` phase deletes branches,
/// so the name pressed can be one git no longer has:
///
/// - Without `--no-guess`, git's DWIM *creates* a local branch from
///   `origin/<name>` at the remote's tip and checks that out — measured at exit
///   0. Creating a branch is outside this epic, and somebody who asked to
///   switch to something they could see would be handed a different commit with
///   nothing saying so.
/// - Without the trailing `--`, a name that also exists as a **path** is taken
///   as a path: measured on git 2.34.1, a top-level file named `shadow` with
///   the branch `shadow` gone made `git checkout --no-guess shadow` print
///   "Updated 1 path from the index" and **exit 0**, restoring that file from
///   the index over an uncommitted edit. This command would have answered
///   `Ok(())`, the panel would have refreshed, the tick would not have moved,
///   and somebody's work would be gone with nothing on screen about it.
///
/// With both, a name git cannot resolve as a branch is `fatal: invalid
/// reference: <name>` at exit 128 — a refusal in git's own words, which is what
/// this command commits to everywhere else. Verified against git 2.34.1 that an
/// ordinary local branch still switches at exit 0 with a dirty tree, and that
/// both refusals above still arrive unchanged.
#[tauri::command]
pub async fn vcs_checkout(repo: String, branch: String) -> Result<(), VcsError> {
    off_the_runtime(move || {
        run::git_write(Path::new(&repo), &["checkout", "--no-guess", &branch, "--"])
    })
    .await
}

/// Cut a new branch from an existing one.
///
/// `start` is the branch the row was on, never HEAD: the whole point of the
/// menu item is that the row somebody right-clicked decides where the new
/// branch begins, and a command that quietly used HEAD would give the same
/// answer from every row in the list.
///
/// Two commands rather than one flag, because they are two different acts.
/// `switch -c` writes the working tree — it moves HEAD, and it carries
/// uncommitted work across with it — while `branch` writes one ref and touches
/// the tree not at all. Somebody who cleared the checkbox asked for the second,
/// and running the first with a switch back afterwards would be two writes and
/// a window where the tree is somewhere nobody asked for.
///
/// Everything git refuses comes back in git's own words at exit 128 — a name it
/// will not take (`fatal: '...' is not a valid branch name`), a name already in
/// use (`fatal: a branch named '...' already exists`), a start point it cannot
/// resolve. `branchName.js` refuses the documented cases before the dialog
/// closes; this is what refuses the rest, and it is the one that decides.
///
/// `switch` rather than `checkout -b`, and the choice is the lesson recorded
/// above: `checkout` takes pathspecs, which is how a branch name that also
/// names a file quietly restored that file instead. `switch` takes none at all —
/// splitting them is the whole reason it exists — so there is nothing here for a
/// name to be mistaken for and no `--` to remember. It wants git 2.23 or newer,
/// which is 2019; verified against 2.34.1, the same build the tests run on.
///
/// `branch` takes no pathspec either, so the quiet half needs no guard.
#[tauri::command]
pub async fn vcs_create_branch(
    repo: String,
    name: String,
    start: String,
    switch: bool,
) -> Result<(), VcsError> {
    off_the_runtime(move || create_branch(Path::new(&repo), &name, &start, switch)).await
}

fn create_branch(repo: &Path, name: &str, start: &str, switch: bool) -> Result<(), VcsError> {
    let args = if switch {
        vec!["switch", "-c", name, start]
    } else {
        vec!["branch", name, start]
    };
    run::git_write(repo, &args)
}

/// Bring another branch's work into the one this repository is on.
///
/// `--no-edit` is the only word added, and it is about not hanging rather than
/// about the merge: git opens an editor for the merge message when it is
/// invoked from a terminal, and a child with pipes for stdio is not one — but
/// that is git's rule to change, and a machine configured to edit regardless
/// would leave a process sitting on an editor nobody can see, with the panel
/// waiting on it for as long as the app is open. No strategy, no `--no-ff`, no
/// `--squash`: what this offers is the merge git would do by itself.
///
/// A conflict is **not** a failure here — see `attempt`.
#[tauri::command]
pub async fn vcs_merge(repo: String, branch: String) -> Result<MergeOutcome, VcsError> {
    off_the_runtime(move || attempt(Path::new(&repo), &["merge", "--no-edit", &branch])).await
}

/// Replay this repository's branch on top of another one.
///
/// `onto` rather than `branch`, because that is what it is: the current branch
/// is the one that moves. Nothing else is passed — an interactive rebase would
/// want an editor and a person in front of it, and continuing an interrupted
/// one is outside this epic.
#[tauri::command]
pub async fn vcs_rebase(repo: String, onto: String) -> Result<MergeOutcome, VcsError> {
    off_the_runtime(move || attempt(Path::new(&repo), &["rebase", &onto])).await
}

/// Bring the remote's refs up to date, so the marks mean something.
///
/// `--prune`, so a branch deleted on the remote reads as `gone` rather than as
/// a branch level with an upstream that is not there any more.
///
/// This writes remote-tracking refs and touches neither the working tree nor
/// the index, which is why the panel treats it as a read: it stays live while a
/// run holds the three writes, exactly as the commit box's suggest button does.
#[tauri::command]
pub async fn vcs_fetch(repo: String) -> Result<(), VcsError> {
    off_the_runtime(move || run::git_network(Path::new(&repo), &["fetch", "--prune"])).await
}

/// Bring the upstream's commits into the branch this repository is on.
///
/// `--no-rebase` is explicit and is the point of naming it: with `pull.rebase`
/// set in somebody's config the same button would merge in one repository and
/// rebase in the next, and the conflict dialog would then offer
/// `git merge --abort` for a rebase. The panel's own Rebase item is one
/// right-click away for anybody who wants that.
///
/// `--no-edit` for the reason `vcs_merge` has it: there is no editor here, and
/// git waiting on one is a hang.
///
/// A conflict is an outcome and not a failure, read off the tree rather than
/// off the message — `attempt_with` is the whole of that rule, shared with
/// `vcs_merge`, so a conflicted pull reaches `ConflictModal` as
/// `OpKind::Merge`, whose abort is the right one.
#[tauri::command]
pub async fn vcs_pull(repo: String) -> Result<MergeOutcome, VcsError> {
    off_the_runtime(move || {
        let args = &["pull", "--no-rebase", "--no-edit"];
        attempt_with(Path::new(&repo), args, run::git_network_attempt)
    })
    .await
}

/// Send this branch's commits to its upstream.
///
/// **Never `--force`, and never `--force-with-lease`.** The refusal force would
/// drive over is the one protecting somebody else's commits, and a rejected
/// push comes back in git's own words, which already name the fix.
///
/// `set_upstream` is the one branch: `git push` alone refuses a branch that has
/// no upstream, which is the ordinary state of a branch cut in this very panel.
/// The front end decides it from the tracking record it is already drawing —
/// and a stale decision is harmless in both directions, since `-u` against a
/// branch that has since gained an upstream sets the same one again, and a
/// plain push of a branch that has since lost it is refused in git's words.
/// `origin` is named here because a branch with no upstream has no other answer
/// to give, and a repository whose remote is called something else is told so
/// by git.
#[tauri::command]
pub async fn vcs_push(repo: String, set_upstream: bool) -> Result<(), VcsError> {
    off_the_runtime(move || {
        let dir = Path::new(&repo);
        if set_upstream {
            run::git_network(dir, &["push", "--set-upstream", "origin", "HEAD"])
        } else {
            run::git_network(dir, &["push"])
        }
    })
    .await
}

/// Put the tree back exactly as it was before the operation that conflicted.
///
/// `op` decides the subcommand and arrives as a typed word (`OpKind`), so the
/// only two things this can run are `git merge --abort` and `git rebase
/// --abort`. Nothing was committed, so nothing is lost — which is the whole of
/// why this door can sit beside "resolve it" with no confirmation in front of
/// it. git's refusal, if it has one (there is no operation in progress, the
/// abort itself could not finish), comes back in its own words.
#[tauri::command]
pub async fn vcs_abort(repo: String, op: OpKind) -> Result<(), VcsError> {
    off_the_runtime(move || run::git_write(Path::new(&repo), &[op.word(), "--abort"])).await
}

/// Everything the panel is drawing, as one commit.
///
/// **Two calls, and the first one is the scope.** `git add --all` stages the
/// whole working tree, which is exactly the list this panel shows — untracked
/// files included, since a change set that is mostly new files is the ordinary
/// case here and `git commit -a` would leave every one of them behind. What it
/// costs is stated rather than hidden: somebody who staged one hunk of a file
/// by hand loses that distinction, because this app has no staging of its own
/// to express it with.
///
/// The message is refused **before** the add, and that ordering is the whole
/// reason the check is here at all rather than left to git. git refuses an
/// empty message itself, in good words — but by then the tree is staged, so a
/// slip would leave the index rewritten behind a failure that said nothing
/// about it.
///
/// No `--no-verify`: a repository's hooks are part of what committing means
/// there, and skipping them silently from a button is not this app's decision
/// to take. What that costs is `run::WRITE_CEILING`: a hook is somebody else's
/// program and may reasonably lint, test or compile, so this call is allowed
/// five minutes — the patience this repository's own hooks declare — and then
/// stopped, with git given the chance to take `index.lock` back off the disk on
/// its way out, which is what keeps a stopped commit from leaving a repository
/// nothing can be committed to.
#[tauri::command]
pub async fn vcs_commit(repo: String, message: String) -> Result<(), VcsError> {
    off_the_runtime(move || commit_all(Path::new(&repo), &message)).await
}

/// The two calls themselves, split from the command so the tests at the bottom
/// of this file can drive them against a real repository — which for this one
/// is worth the temp directory: it is the only path in the module that stages
/// anything, and the ordering below is the whole of what keeps a slip from
/// rewriting somebody's index.
fn commit_all(repo: &Path, message: &str) -> Result<(), VcsError> {
    let message = message.trim();
    if message.is_empty() {
        return Err(VcsError::NoMessage);
    }
    run::git_write(repo, &["add", "--all"])?;
    run::git_write(repo, &["commit", "-m", message])
}

/// A commit message for what is in the tree right now, written by the agent.
///
/// Two halves, and they fail differently on purpose. Reading the diff is git
/// and can only refuse in git's words; asking the harness is
/// `agents::oneshot`, which has four ways to fail that a person can act on and
/// keeps them apart. `OneshotError::Git` is where the first lands, so one
/// message reaches the field whichever half gave way.
///
/// `pick` rather than `resolve`, the same call `terminal::service` makes: what
/// answers is whatever is actually installed. A harness with no one-shot form
/// says so through `Unsupported` rather than being hidden — the button is drawn
/// for everybody, because whether the *configured* agent can do this is a fact
/// the front end deliberately does not know (it never learns an agent's name).
#[tauri::command]
pub async fn vcs_suggest_message(
    app: tauri::AppHandle,
    repo: String,
) -> Result<String, OneshotError> {
    // `spawn_blocking`, the same rule as `off_the_runtime` above and for the
    // same reason — that wrapper cannot be the one used, because what comes
    // back here is `OneshotError`, one message for a person however the call
    // gave way. **The whole body is inside it**, not the ask alone: reading the
    // settings file, probing the login shell for a `PATH` and the four git
    // calls of `describe` are every one of them blocking, and the ask itself
    // then waits on a model for as long as ninety seconds.
    tokio::task::spawn_blocking(move || {
        let agent = crate::settings::agent(&app);
        let profile = crate::agents::pick(&agent, crate::shell_env::path())
            .ok_or_else(|| OneshotError::NoAgent(agent.clone()))?;
        let prompt = describe(Path::new(&repo)).map_err(|err| OneshotError::Git(err.to_string()))?;
        oneshot::ask(profile, &prompt)
    })
    .await
    .map_err(|err| OneshotError::Io(err.to_string()))?
}

/// What git has to say about the tree, as the text of a question.
///
/// Untracked paths are gathered from the very tree the panel is drawing, and
/// they have to be gathered separately for a reason that is git's rather than
/// ours: an untracked file is in no diff at all, so a change set of nothing but
/// new files would otherwise be described to the agent as an empty one.
///
/// A repository with no commit yet has no `HEAD` to diff against, and asking
/// anyway is `fatal: ambiguous argument 'HEAD'` — a refusal about the wrong
/// thing entirely. So `HEAD` is verified first, through the one call that reads
/// a non-zero exit as an answer, and its absence leaves the diff empty: in that
/// repository every file is untracked and the list above is the whole change.
fn describe(repo: &Path) -> Result<String, VcsError> {
    let tree = working_tree(repo)?;
    let untracked: Vec<String> = tree
        .changes
        .iter()
        .filter(|change| change.kind == ChangeKind::Untracked)
        .map(|change| change.path.clone())
        .collect();
    let born = run::git_maybe(repo, &["rev-parse", "--verify", "--quiet", "HEAD"], 1)?.is_some();
    let (stat, patch) = if born {
        (run::git_read(repo, &["diff", "HEAD", "--stat"])?, run::git_read(repo, &["diff", "HEAD"])?)
    } else {
        (String::new(), String::new())
    };
    Ok(oneshot::commit_prompt(&stat, &untracked, &patch))
}

/// A merge or a rebase, and the reading of what it left behind.
///
/// **A non-zero exit is not an answer by itself.** git stops the same way for a
/// tree it left conflicted and for one it refused to touch, and the messages
/// that tell those apart are prose that moves between versions. So the tree is
/// read — through the very call `vcs_status` uses — and unmerged records decide.
/// Anything else non-zero is `VcsError::Git` with git's own stderr, untouched,
/// exactly as every other command here refuses.
///
/// **The tree is read twice, before as well as after, and the first read is
/// what makes the rule true.** git refuses to *start* either operation in a
/// tree that already has unmerged entries and leaves those entries exactly
/// where they were, so an "after" read alone reports somebody else's conflict
/// as this operation's — `model::new_conflicts` is that rule and carries the
/// measurement. The cost is one `git status` per merge or rebase: tens of
/// milliseconds in front of an operation that rewrites the working tree.
///
/// A tree that could not be read at all — either time — counts as no conflict,
/// so what the person gets is git's refusal rather than a second failure about
/// a status nobody asked for: the first one is what they can act on, and an
/// unreadable tree is not evidence about what is unmerged in it.
fn attempt(repo: &Path, args: &[&str]) -> Result<MergeOutcome, VcsError> {
    attempt_with(repo, args, run::git_attempt)
}

/// The tree read before, the operation, the tree read after — and
/// `new_conflicts` deciding whether this operation is what conflicted. The
/// runner is a parameter because a pull does all of this over the network and
/// none of the reasoning changes: `git pull`'s non-zero exit is exactly as
/// ambiguous as `git merge`'s.
fn attempt_with(
    repo: &Path,
    args: &[&str],
    run: fn(&Path, &[&str]) -> Result<Attempt, VcsError>,
) -> Result<MergeOutcome, VcsError> {
    let before = working_tree(repo).ok();
    match run(repo, args)? {
        Attempt::Done => Ok(MergeOutcome::Clean),
        Attempt::Refused(refusal) => {
            let files = working_tree(repo)
                .map(|after| model::new_conflicts(before.as_ref(), &after))
                .unwrap_or_default();
            if files.is_empty() {
                Err(refusal)
            } else {
                Ok(MergeOutcome::Conflict { files })
            }
        }
    }
}

/// The command's whole body, synchronous so a test can call it: an `async fn`
/// would need a runtime here for three file reads that never yield.
fn branch_list(path: &Path) -> Vec<Branch> {
    // HEAD is read here rather than compared on the front end, and it is read
    // per worktree while the refs are shared — the distinction `parse_commondir`
    // exists for.
    let current = git::head(path).branch;
    git::by_recency(git::branches_with_recency(path))
        .into_iter()
        .map(|name| Branch { current: current.as_deref() == Some(name.as_str()), name })
        .collect()
}

/// git's own exit code for a name that resolves to nothing. `--quiet` is what
/// makes it a clean answer rather than a failure: git prints not a word, and 1
/// is a code no other outcome of `rev-parse --verify` uses — a repository it
/// could not read at all exits 128 with its own message.
const NO_SUCH_OBJECT: i32 = 1;

/// One file as `HEAD` has it, for the diff against the working tree. `path` is
/// relative to `repo`, exactly as `vcs_status` reported it.
///
/// `Ok(None)` is a file `HEAD` does not have — an added or an untracked one,
/// and every file of a repository with no commit in it yet. That is not a
/// failure and must not read as one: the panel diffs such a file against an
/// empty document, which is what it is.
///
/// **Three calls and each is a machine-readable question.** `git show
/// HEAD:<path>` was the obvious single call and cannot answer the first one: it
/// exits 128 for an absent path and 128 for a folder that is not a repository
/// alike, so telling the two apart would mean reading git's prose — which moves
/// between versions, the reason `model.rs` parses `--porcelain=v2` and never
/// `git status`. `rev-parse --verify --quiet` says the same thing in an exit
/// code.
///
/// The two calls after it are given the **object name** rather than
/// `HEAD:<path>` a second and a third time. HEAD can move while this is in
/// flight — an agent committing in the same tree is the ordinary case here —
/// and asking again by name would let the size belong to one blob and the bytes
/// to another.
///
/// The ceiling and the binary sniff are `files/`'s own, and deliberately: this
/// opens in the same editor, and a file it already refuses to open above 2 MiB
/// must not arrive through a second door. The size is asked for before the
/// bytes are, the way `files/fs.rs` reads the metadata before it reads the
/// file.
#[tauri::command]
pub async fn vcs_file_at_head(repo: String, path: String) -> Result<Option<String>, VcsError> {
    off_the_runtime(move || file_at_head(Path::new(&repo), path)).await
}

/// The command's whole body, off the runtime and synchronous — every line of it
/// is a git call or a decision taken from one.
fn file_at_head(dir: &Path, path: String) -> Result<Option<String>, VcsError> {
    let object = format!("HEAD:{path}");
    let Some(name) = run::git_maybe(
        dir,
        &["rev-parse", "--verify", "--quiet", &object],
        NO_SUCH_OBJECT,
    )?
    else {
        return Ok(None);
    };
    let name = name.trim().to_owned();

    // An answer that will not parse is not a reason to refuse the file: it
    // costs the cheap check and leaves the one below it, which reads the bytes
    // that actually arrived and cannot be wrong about them.
    if let Ok(bytes) = run::git_read(dir, &["cat-file", "-s", &name])?.trim().parse::<u64>() {
        if bytes > MAX_FILE_BYTES {
            return Err(VcsError::TooLarge { path, bytes });
        }
    }

    let bytes = run::git_bytes(dir, &["cat-file", "blob", &name])?;
    if bytes.len() as u64 > MAX_FILE_BYTES {
        return Err(VcsError::TooLarge { path, bytes: bytes.len() as u64 });
    }
    if looks_binary(&bytes[..bytes.len().min(BINARY_SNIFF_BYTES)]) {
        return Err(VcsError::Binary(path));
    }
    // Refused rather than made lossy, for the reason the editor refuses it: a
    // replacement character is a change to the text, and a diff drawn over one
    // would report a difference nobody made.
    String::from_utf8(bytes).map(Some).map_err(|_| VcsError::NotUtf8(path))
}

/// The one thing in this file worth pinning: the mapping above, over the layout
/// that broke `git.rs` once already. Everything else here is an argument list
/// handed to `run.rs`, which is the process table and carries no tests.
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("smetana-vcs-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create the temp directory");
        dir
    }

    /// A project opened as a linked worktree offers the whole repository's
    /// branches, and exactly one of them is the one it is on.
    ///
    /// The refs live in the common directory and HEAD does not, so reading both
    /// beside the worktree's own git directory would answer with the single
    /// branch nobody needs — smetana-5t7, one level up.
    #[test]
    fn a_worktree_lists_every_branch_and_marks_the_one_it_is_on() {
        let root = scratch("worktree-branches");
        let repo = root.join("repo");
        let linked = root.join("wt");
        let wt_git = repo.join(".git/worktrees/wt");
        fs::create_dir_all(repo.join(".git/refs/heads/feat")).expect("create refs/heads/feat");
        fs::create_dir_all(&wt_git).expect("create the worktree git directory");
        fs::create_dir_all(&linked).expect("create the worktree");
        for name in ["main", "staging"] {
            fs::write(repo.join(".git/refs/heads").join(name), "a1b2c3\n").expect("write the ref");
        }
        fs::write(repo.join(".git/refs/heads/feat/x"), "d4e5f6\n").expect("write the nested ref");
        fs::write(repo.join(".git/HEAD"), "ref: refs/heads/main\n").expect("write the main HEAD");
        fs::write(wt_git.join("HEAD"), "ref: refs/heads/feat/x\n").expect("write the worktree HEAD");
        fs::write(wt_git.join("commondir"), "../..\n").expect("write commondir");
        fs::write(linked.join(".git"), format!("gitdir: {}\n", wt_git.display()))
            .expect("write the .git file");

        let branches = branch_list(&linked);
        let names: Vec<&str> = branches.iter().map(|b| b.name.as_str()).collect();
        assert_eq!(names, ["feat/x", "main", "staging"]);
        let current: Vec<&str> =
            branches.iter().filter(|b| b.current).map(|b| b.name.as_str()).collect();
        assert_eq!(current, ["feat/x"]);

        let _ = fs::remove_dir_all(&root);
    }

    /// A folder outside git is an empty list, never a refusal — the answer
    /// `vcs_repos` gives about the same folder.
    #[test]
    fn a_folder_outside_git_has_no_branches_and_no_error() {
        let root = scratch("no-git");
        assert!(branch_list(&root).is_empty());
        let _ = fs::remove_dir_all(&root);
    }

    /// A real repository, since the two tests below are about what git does
    /// rather than about what we pass it. The identity is set in the repository
    /// itself and not through the environment: a machine with no `user.email`
    /// configured is an ordinary machine, and a test that failed there would be
    /// reporting on the machine rather than on the code.
    fn repository(name: &str) -> PathBuf {
        let root = scratch(name);
        run::git_write(&root, &["init", "--quiet"]).expect("git init");
        run::git_write(&root, &["config", "user.email", "test@example.com"])
            .expect("set the email");
        run::git_write(&root, &["config", "user.name", "Test"]).expect("set the name");
        root
    }

    /// The scope of the button, pinned against git itself: `--all` and not
    /// `commit -a`, so a file git has never seen goes in with the rest. A change
    /// set that is mostly new files is the ordinary case in this panel, and
    /// `commit -a` would leave every one of them behind.
    #[test]
    fn everything_in_the_tree_is_committed_including_what_git_was_not_tracking() {
        let repo = repository("commit-all");
        fs::write(repo.join("tracked.txt"), "one\n").expect("write the tracked file");
        run::git_write(&repo, &["add", "tracked.txt"]).expect("stage it");
        run::git_write(&repo, &["commit", "-m", "first"]).expect("commit it");
        fs::write(repo.join("tracked.txt"), "two\n").expect("edit the tracked file");
        fs::write(repo.join("new.txt"), "three\n").expect("write the untracked file");

        commit_all(&repo, "  fix: both of them  ").expect("commit");

        let tree = working_tree(&repo).expect("read the tree");
        assert!(tree.changes.is_empty(), "the tree should be clean: {:?}", tree.changes);
        let last = run::git_read(&repo, &["log", "-1", "--name-only", "--format=%s"])
            .expect("read the log");
        // The message is committed trimmed, which is what the button's own
        // guard reads and what a person typing a trailing newline expects.
        assert!(last.contains("fix: both of them"), "{last}");
        assert!(last.contains("new.txt"), "the untracked file should be in it: {last}");
        assert!(last.contains("tracked.txt"), "{last}");

        let _ = fs::remove_dir_all(&repo);
    }

    /// **The refusal comes before the add**, which is the whole reason it is
    /// ours rather than git's: git refuses an empty message too, in good words,
    /// but by then the tree is staged and a slip has rewritten the index behind
    /// a failure that said nothing about it.
    #[test]
    fn an_empty_message_is_refused_before_anything_is_staged() {
        let repo = repository("commit-empty");
        fs::write(repo.join("new.txt"), "one\n").expect("write the untracked file");

        let refused = commit_all(&repo, " \n ").expect_err("an empty message is refused");

        assert_eq!(refused.kind(), "noMessage");
        let tree = working_tree(&repo).expect("read the tree");
        let staged: Vec<&str> =
            tree.changes.iter().filter(|c| c.staged).map(|c| c.path.as_str()).collect();
        assert!(staged.is_empty(), "nothing should have been staged: {staged:?}");
        assert_eq!(tree.changes.len(), 1);

        let _ = fs::remove_dir_all(&repo);
    }

    /// A repository with two commits on two branches, and HEAD deliberately on
    /// the second: everything the branch tests below are about is the difference
    /// between the row that was clicked and where HEAD happens to be.
    fn two_branches(name: &str) -> (PathBuf, String, String) {
        let repo = repository(name);
        fs::write(repo.join("a.txt"), "one\n").expect("write a file");
        run::git_write(&repo, &["add", "."]).expect("stage");
        run::git_write(&repo, &["commit", "-m", "first"]).expect("commit");
        // Whatever this git calls its first branch — the default is a machine's
        // configuration, not this test's business.
        let first = head_branch(&repo);
        run::git_write(&repo, &["switch", "-c", "second"]).expect("cut the second branch");
        fs::write(repo.join("b.txt"), "two\n").expect("write another file");
        run::git_write(&repo, &["add", "."]).expect("stage");
        run::git_write(&repo, &["commit", "-m", "second"]).expect("commit");
        (repo, first, "second".into())
    }

    fn head_branch(repo: &Path) -> String {
        let out = run::git_read(repo, &["rev-parse", "--abbrev-ref", "HEAD"]).expect("read HEAD");
        out.trim().into()
    }

    fn sha(repo: &Path, rev: &str) -> String {
        run::git_read(repo, &["rev-parse", rev]).expect("resolve the revision").trim().into()
    }

    /// **The start point is the row, never HEAD.** The menu item exists because
    /// the row somebody right-clicked decides where the branch begins; a command
    /// reading HEAD would answer the same from every row in the list, and the
    /// commit somebody chose is exactly the thing they would not be able to see
    /// was wrong.
    #[test]
    fn a_new_branch_starts_at_the_branch_it_was_cut_from() {
        let (repo, first, _) = two_branches("branch-start");

        create_branch(&repo, "cut", &first, false).expect("create the branch");

        assert_eq!(sha(&repo, "cut"), sha(&repo, &first));
        assert_ne!(sha(&repo, "cut"), sha(&repo, "second"));

        let _ = fs::remove_dir_all(&repo);
    }

    /// The checkbox, pinned against git: one call moves HEAD and the other
    /// leaves the tree exactly where it stood.
    #[test]
    fn switching_is_the_checkbox_and_nothing_else_moves_head() {
        let (repo, first, _) = two_branches("branch-switch");

        create_branch(&repo, "quiet", &first, false).expect("create without switching");
        assert_eq!(head_branch(&repo), "second");

        create_branch(&repo, "loud", &first, true).expect("create and switch");
        assert_eq!(head_branch(&repo), "loud");
        assert_eq!(sha(&repo, "HEAD"), sha(&repo, &first));

        let _ = fs::remove_dir_all(&repo);
    }

    /// A name git will not take is git's refusal, in git's words, and nothing is
    /// created. `branchName.js` refuses this one before the dialog closes; what
    /// is pinned here is that the back end does not quietly accept what the
    /// front end happened to miss.
    #[test]
    fn a_name_git_refuses_creates_nothing() {
        let (repo, first, _) = two_branches("branch-bad-name");

        let refused = create_branch(&repo, "no spaces", &first, false).expect_err("git refuses");

        assert_eq!(refused.kind(), "git");
        let branches: Vec<String> = branch_list(&repo).into_iter().map(|b| b.name).collect();
        assert_eq!(branches.iter().filter(|n| n.contains("spaces")).count(), 0, "{branches:?}");

        let _ = fs::remove_dir_all(&repo);
    }
}
