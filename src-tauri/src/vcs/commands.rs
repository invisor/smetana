//! Thin commands with no state of their own, shaped exactly like `files/`'s.
//!
//! The project and the repository arrive as paths from the front end, which
//! knows both anyway: keeping a second copy of that knowledge here would mean
//! taking a dependency on the tracker for a value that is not this module's.

use std::path::Path;

use super::model::{
    parse_name_status, Branch, ChangeKind, Comparison, InProgress, Landed, MergeOutcome, OpKind,
    ProjectRepos, Tracking, VcsError, WorkingTree,
};
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

/// The same, for the commands documented as never refusing.
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
/// has no room for a refusal, so it goes where a developer will find it — and
/// the caller names itself, because `JoinError`'s own text carries a task id
/// and the panic and never the command, which in a wrapper they all share
/// would leave the line unable to say which read gave way.
async fn off_the_runtime_or_empty<T, F>(read: &'static str, work: F) -> T
where
    F: FnOnce() -> T + Send + 'static,
    T: Default + Send + 'static,
{
    tokio::task::spawn_blocking(work).await.unwrap_or_else(|err| {
        log::error!("vcs: {read} gave way and is being reported as nothing at all: {err}");
        T::default()
    })
}

/// The repositories of a project, and the ones on disk it does not name.
///
/// Never a refusal: a folder that is not a repository, or holds none, is an
/// empty list, which the panel draws as an empty state of its own. Both halves
/// are empty in that case, which is what `ProjectRepos::default` means here —
/// the second half is a statement about a list, and there is no list.
#[tauri::command]
pub async fn vcs_repos(project: String) -> ProjectRepos {
    off_the_runtime_or_empty("vcs_repos", move || repos::discover(Path::new(&project))).await
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
    off_the_runtime_or_empty("vcs_branches", move || branch_list(Path::new(&repo))).await
}

/// The branches `origin` is known to have, as of the last fetch.
///
/// **`origin` and no other remote**, and that is the app rather than this
/// command being narrow: nowhere in it is there a notion of a second remote, so
/// a `remote` argument would be a vocabulary neither side could yet say a word
/// about — one more thing to pass through and nothing to pass.
///
/// This one spawns nothing either, for `vcs_branches`' reason and through the
/// same file: `git::remote_branches` reads `refs/remotes/origin/` and
/// `packed-refs` off the disk, out of the common directory, so a linked
/// worktree answers for the whole repository.
///
/// **Beside `vcs_branches` rather than inside it.** That command promises two
/// things — it runs no process and it cannot refuse — and both are kept here, so
/// the split is not about either: it is that a remote branch and a local one of
/// the same name are two different things to check out, and one list holding
/// both could not say which a name was. The caller picks a side before it picks
/// a name.
///
/// Never a refusal: a folder outside git, a repository nobody has fetched into,
/// a clone with no `origin` at all — every one of them is the empty list, which
/// is an ordinary answer and not an error.
///
/// **A name and the stamp of the fetch that last moved it** —
/// `git::RemoteBranch` — rather than the bare name this answered with before. An
/// age is drawn beside each of these names now and the name alone cannot say
/// it. The prefix is still off, so whoever builds a ref for git is still the
/// side that puts it back.
#[tauri::command]
pub async fn vcs_remote_branches(repo: String) -> Vec<git::RemoteBranch> {
    off_the_runtime_or_empty("vcs_remote_branches", move || {
        git::remote_branches(Path::new(&repo), "origin")
    })
    .await
}

/// When this repository last fetched, in epoch seconds, or `null`.
///
/// **A third command in this file that spawns nothing**, for `vcs_branches`'
/// reason and through the same file: `git::last_fetch` stats `FETCH_HEAD` in
/// the repository's git directory and in its common one, which is a disk read
/// and not a process. Why that file and nothing else — and why both directories
/// — is written down there.
///
/// Beside `vcs_remote_branches` rather than inside it, though the front end asks
/// the two together: what `origin` holds is a statement about branches and this
/// is one fact about the repository, true whether or not anybody wanted a list.
/// Folding it into the list would make it a field repeated on every row, or a
/// second shape wrapping one — and it would leave a repository with no remote
/// branches at all with nowhere to say when it last asked.
///
/// Never a refusal, and `null` is an ordinary answer with three meanings the
/// screen draws the same way: nobody has fetched here, this is not a repository,
/// or the file could not be stat-ed. A time this app is unsure of is worse than
/// no time, since "fetched 2m ago" is read as a promise that the refs are that
/// fresh.
#[tauri::command]
pub async fn vcs_last_fetch(repo: String) -> Option<i64> {
    off_the_runtime_or_empty("vcs_last_fetch", move || git::last_fetch(Path::new(&repo))).await
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
    off_the_runtime_or_empty("vcs_tracking", move || tracking(Path::new(&repo))).await
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

/// Delete a local branch.
///
/// **Not the current one, and that is refused here rather than only in the
/// menu.** The window that asks the question is an OS window of its own with no
/// scrim over the board, so HEAD can move while it stands — an agent working in
/// the same tree, or a checkout in a terminal — and the press that arrives
/// afterwards would be about a branch that has since become the one the
/// repository is standing on. `git branch -d` refuses that too, in good words;
/// what this buys is a `kind` the front end can act on rather than prose it
/// would have to read.
///
/// The head is read through `git::head`, which is three file reads and no
/// process at all, so the guard costs nothing next to the delete behind it.
///
/// **Why a refused delete is asked about a second time instead of being read.**
/// Without `force` this runs `git branch -d`, which declines for several
/// different reasons at the same exit code — the branch is not merged, it is
/// checked out in another worktree, it does not exist. Only the first of those
/// has a way forward, and the way forward loses commits, so the window has to
/// know which it was before it offers `Delete anyway`. git says which in
/// **prose**, and nothing in `run.rs` fixes the locale, so a substring search
/// here would pass on the machine it was written on and quietly stop working on
/// somebody else's — the rule this module keeps everywhere (`--porcelain=v2`
/// over `git status`, an unmerged record over a merge's message). So the answer
/// comes from a second question with an exit code for an answer:
/// `git merge-base --is-ancestor <branch> HEAD`, exit 1 for "no" through
/// `run::git_maybe`. Only a definite "no" becomes `VcsError::NotMerged`;
/// everything else, the probe itself failing included, is handed back as git
/// refused it. The extra process runs on the failure path alone.
///
/// One case is knowingly imprecise and is cheaper left so: a branch that is
/// both unmerged **and** checked out in another worktree answers "not merged"
/// and is offered `Delete anyway`, which git then declines in its own words in
/// the same window. Naming that case exactly means parsing `git worktree list
/// --porcelain` on every refusal, and what it would buy is one press.
///
/// With `force` it is `git branch -D`, and there is no second question to ask:
/// the person has already been told what they are losing.
#[tauri::command]
pub async fn vcs_delete_branch(
    repo: String,
    branch: String,
    force: bool,
) -> Result<(), VcsError> {
    off_the_runtime(move || delete_branch(Path::new(&repo), &branch, force)).await
}

fn delete_branch(repo: &Path, branch: &str, force: bool) -> Result<(), VcsError> {
    if git::head(repo).branch.as_deref() == Some(branch) {
        return Err(VcsError::CurrentBranch(branch.to_owned()));
    }
    let flag = if force { "-D" } else { "-d" };
    match run::git_attempt(repo, &["branch", flag, branch])? {
        Attempt::Done => Ok(()),
        Attempt::Refused(refused) => {
            if !force && matches!(merged_into_head(repo, branch), Ok(false)) {
                return Err(VcsError::NotMerged(branch.to_owned()));
            }
            Err(refused)
        }
    }
}

/// Whether every commit on this branch is already in the branch the repository
/// is on. Exit 0 is yes, exit 1 is no, anything else is a refusal — which is
/// `run::git_maybe`'s own shape, with the code named by the caller because this
/// function is the only one that knows which non-zero exit was an answer.
fn merged_into_head(repo: &Path, branch: &str) -> Result<bool, VcsError> {
    run::git_maybe(repo, &["merge-base", "--is-ancestor", branch, "HEAD"], 1).map(|out| out.is_some())
}

/// Rename a local branch.
///
/// **`-m` and never `-M`**, which is the whole of what this command decides.
/// The forced form takes a name another branch already holds and writes over
/// it, which loses commits nobody asked to lose; the plain one refuses, and
/// git's refusal reaches the panel in git's own words like every other refusal
/// here. There is no second question to ask, unlike a refused delete: nothing
/// about this one has a way forward that costs anything.
///
/// **No guard on the branch the repository is standing on**, which is where
/// this parts company with `delete_branch` above. `git branch -m` renames the
/// branch HEAD is on and HEAD travels with the ref, so a typo in the name of
/// the branch somebody is working in is the ordinary case rather than the edge
/// one — there is nothing here for a guard to protect.
///
/// `branchName.js` refuses git's documented shapes before the window closes;
/// this is what refuses the rest — a name already taken, a branch held by
/// another worktree — and it is the one that decides.
///
/// Nothing reaches the remote: the upstream keeps its own name, the new name is
/// not pushed and the old one is not deleted there. That is a different act with
/// different consequences and it is not what this offers.
#[tauri::command]
pub async fn vcs_rename_branch(repo: String, from: String, to: String) -> Result<(), VcsError> {
    off_the_runtime(move || rename_branch(Path::new(&repo), &from, &to)).await
}

fn rename_branch(repo: &Path, from: &str, to: &str) -> Result<(), VcsError> {
    run::git_write(repo, &["branch", "-m", from, to])
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
    off_the_runtime(move || {
        let dir = Path::new(&repo);
        attempt(dir, &["merge", "--no-edit", &branch], &local_ref(&branch))
    })
    .await
}

/// Replay this repository's branch on top of another one.
///
/// `onto` rather than `branch`, because that is what it is: the current branch
/// is the one that moves. Nothing else is passed — an interactive rebase would
/// want an editor and a person in front of it, and continuing an interrupted
/// one is outside this epic.
#[tauri::command]
pub async fn vcs_rebase(repo: String, onto: String) -> Result<MergeOutcome, VcsError> {
    off_the_runtime(move || attempt(Path::new(&repo), &["rebase", &onto], &local_ref(&onto))).await
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
        attempt_with(Path::new(&repo), args, Other::after(UPSTREAM), run::git_network_attempt)
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
/// **The measurement is taken before the push and read backwards**, which is
/// the one thing about this command that is not obvious. Afterwards the
/// upstream has been moved to where HEAD is and the same question answers
/// nothing every time; and the question itself is the mirror of the other three
/// — what this side has that the other did not — so the range is
/// `<upstream>..<head>` rather than the other way round.
///
/// A branch with no upstream is every field `null` and never a row of zeros:
/// nothing was measured, and a zero would say the remote already had this
/// branch, which is the opposite of what publishing one means.
#[tauri::command]
pub async fn vcs_push(repo: String, set_upstream: bool) -> Result<Landed, VcsError> {
    off_the_runtime(move || {
        let dir = Path::new(&repo);
        let head = object(dir, "HEAD");
        let upstream = object(dir, UPSTREAM);
        let sent = landed(
            dir,
            (upstream.as_deref(), head.as_deref()),
            (upstream.as_deref(), head.as_deref()),
        );
        if set_upstream {
            run::git_network(dir, &["push", "--set-upstream", "origin", "HEAD"])?;
        } else {
            run::git_network(dir, &["push"])?;
        }
        Ok(sent)
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

/// Which operation, if any, this repository is part-way through.
///
/// **Asked of git as a process and never read off the disk.** `vcs/`'s header
/// forbids a file read and that is not being relaxed here: `git rev-parse -q
/// --verify MERGE_HEAD` is `run.rs` spawning git, the one thing this module
/// exists to do, where stat-ing `.git/MERGE_HEAD` would be a second way of
/// knowing the same fact and a second way of being wrong about it.
///
/// Called only for a tree that already shows unmerged paths, so a clean
/// repository — nearly every repository, nearly always — pays no process for
/// it. That is the caller's rule and not this function's: asked of a clean
/// tree it simply answers `None`.
#[tauri::command]
pub async fn vcs_in_progress(repo: String) -> Result<Option<InProgress>, VcsError> {
    off_the_runtime(move || in_progress(Path::new(&repo))).await
}

/// The questions themselves, split from the command so the tests at the foot of
/// this file can drive them against a real repository — which this one needs
/// more than any other here, because what it is about is **what git leaves
/// behind**, and no argument list inspected on its own would have caught it.
///
/// **A ref that exists is not an operation in progress, and that distinction
/// cost a defect.** The first version asked `rev-parse -q --verify REBASE_HEAD`
/// and read an answer as "a rebase is going on". git writes `REBASE_HEAD` when a
/// rebase stops and — on the default `--merge` backend and on `-i`, though not
/// on `--apply` — **never removes it**, so from the first rebase somebody
/// finished with `--continue` onward that question answers yes forever. Every
/// later conflicted tree with no `MERGE_HEAD` was then called a rebase: the
/// panel drew the button over a `git cherry-pick`, `Abort` ran `git rebase
/// --abort` and died with "No rebase in progress?", and "Resolve with an agent"
/// briefed an agent with write access to say "Finish a git rebase" over a tree
/// that was mid-cherry-pick. The trigger is exactly the workflow this feature
/// exists for.
///
/// So the rebase arm is gated on `git rebase --show-current-patch`, which is a
/// question about the operation rather than about a file git forgot to sweep up:
/// measured on git 2.34.1 it exits 0 while a rebase is stopped on all three
/// backends (`--merge`, `--apply`, `-i`) and 128 when there is none, stale
/// `REBASE_HEAD` beside a conflicted cherry-pick included. `REBASE_HEAD` stays,
/// but only as the **name** source it was always reliable for.
///
/// Two things about the exit codes are load-bearing. `rev-parse -q --verify`
/// exits 1 for a ref that is not there, which is an answer and not a refusal —
/// the one case `git_maybe` was written for. And 128 is git's *generic* fatal
/// code, which is only safe to read as "no rebase" **in this position**: a
/// repository git cannot read at all has already refused at the `MERGE_HEAD`
/// call above, which exits 128 there and comes back as `VcsError::Git` carrying
/// git's own words.
///
/// The one cost worth naming is that `--show-current-patch` prints the patch of
/// the commit git stopped on, and `git_maybe` keeps standard output. It is one
/// commit's diff, read and dropped, on a path only a conflicted tree reaches.
fn in_progress(repo: &Path) -> Result<Option<InProgress>, VcsError> {
    let named = |rev: &str| -> Result<Option<String>, VcsError> {
        let line =
            run::git_maybe(repo, &["name-rev", "--name-only", "--refs=refs/heads/*", rev], 1)?;
        Ok(line.as_deref().and_then(model::branch_from_name_rev))
    };
    // A merge first, because it is the one whose two names are both exact.
    // `MERGE_HEAD` is the commit being brought in, and during a merge HEAD is
    // still on the branch it is being brought into. This ref git does sweep up,
    // and the operation has no other trace to ask about.
    if run::git_maybe(repo, &["rev-parse", "-q", "--verify", "MERGE_HEAD"], 1)?.is_some() {
        let head = run::git_maybe(repo, &["symbolic-ref", "--short", "-q", "HEAD"], 1)?;
        let ours = head.map(|line| line.trim().to_string()).filter(|name| !name.is_empty());
        return Ok(Some(InProgress { op: OpKind::Merge, ours, theirs: named("MERGE_HEAD")? }));
    }
    // Is a rebase actually going on — see the header for why the ref alone is
    // not that question.
    if run::git_maybe(repo, &["rebase", "--show-current-patch"], 128)?.is_some() {
        // `REBASE_HEAD` is the commit git stopped on, which belongs to the
        // branch being rebased. So it names `ours` and never `theirs`, and a
        // rebase that is genuinely in progress always has it.
        //
        // The onto is deliberately left unknown. `name-rev` on HEAD answers
        // `undefined` the moment one commit has been applied, and the only
        // remaining source is `.git/rebase-merge/onto` — the file read this
        // module does not do. Both the dialog and the prompt read correctly
        // without it; a guess would read correctly and be false.
        return Ok(Some(InProgress {
            op: OpKind::Rebase,
            ours: named("REBASE_HEAD")?,
            theirs: None,
        }));
    }
    // A tree can be conflicted with neither of these in progress — a
    // cherry-pick, a revert, a stash pop, a `checkout --merge`. Neither of the
    // dialog's two doors is true for those, so "nothing" is the honest answer
    // and the panel draws no button.
    Ok(None)
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
        // The same file the agent id came from, one field over, and read the
        // same way — a session's own commits are told this language by
        // `agents::prompt`, so the button and the run agree by construction
        // rather than by two people remembering to keep them in step.
        let language = crate::agents::language_name(&crate::settings::languages(&app).commit);
        // Nothing to describe is this command's own refusal and never a
        // question put to a model — `OneshotError::Nothing` carries the whole
        // of why, and what it costs to get this one wrong.
        let prompt = describe(Path::new(&repo), language)
            .map_err(|err| OneshotError::Git(err.to_string()))?
            .ok_or(OneshotError::Nothing)?;
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
///
/// `language` is the English name of the person's `commitLanguage`, already
/// resolved by the caller — this function is git and text, and reading a
/// settings file here would be a second road to the same answer.
///
/// `Ok(None)` is a tree with nothing uncommitted in it: no `--stat`, no
/// untracked path, no patch. That is not a failure and is deliberately not
/// phrased as one here — this function is git and text, so it answers what git
/// said, and the sentence a person reads is the caller's
/// (`OneshotError::Nothing`, which carries the reason it must not be sent on).
/// What it must never do is build the prompt anyway: "write a commit message
/// for the changes below", with nothing below it, is a question rather than an
/// instruction, and what comes back is an answer to that question.
fn describe(repo: &Path, language: &str) -> Result<Option<String>, VcsError> {
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
    if stat.trim().is_empty() && untracked.is_empty() && patch.trim().is_empty() {
        return Ok(None);
    }
    Ok(Some(oneshot::commit_prompt(language, &stat, &untracked, &patch)))
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
fn attempt(repo: &Path, args: &[&str], other: &str) -> Result<MergeOutcome, VcsError> {
    attempt_with(repo, args, Other::before(other), run::git_attempt)
}

/// git's own name for the branch the current one tracks. One spelling, because
/// it is asked in two commands and a second copy is the half that drifts.
const UPSTREAM: &str = "@{upstream}";

/// A local branch as a full ref.
///
/// The name arrives from the front end, and a full ref is what stops one that
/// begins with a dash being read by `rev-parse` as a flag — the rule
/// `vcs_compare` already keeps for the branch it resolves. It is also exactly
/// what git resolves the bare name to, so nothing about the measurement moves.
fn local_ref(branch: &str) -> String {
    format!("refs/heads/{branch}")
}

/// The other side of a write, and **when** it is resolved.
///
/// The three local operations resolve it before: their ref is local, and the
/// operation ran against the state it was in at the start — resolving it
/// afterwards would count a commit an agent pushed into that branch while the
/// merge was running, which nobody merged.
///
/// A pull is the one that resolves after, and it is a fact about `git pull`
/// rather than a preference: a pull is a fetch and then a merge, so before it
/// runs `origin/main` is exactly as stale as the last fetch left it, and a
/// count against that would report what was already known.
struct Other<'a> {
    rev: &'a str,
    after: bool,
}

impl<'a> Other<'a> {
    fn before(rev: &'a str) -> Self {
        Self { rev, after: false }
    }

    fn after(rev: &'a str) -> Self {
        Self { rev, after: true }
    }
}

/// What a name resolves to right now, or nothing at all.
///
/// **A refusal is swallowed here, and that is the rule for every measurement in
/// this file.** A repository with no commit in it has no HEAD, a branch nobody
/// has published has no upstream, and neither of those may turn a merge that
/// worked into an error. `rev-parse --verify --quiet` exits `NO_SUCH_OBJECT`
/// for a name that resolves to nothing and says not a word; anything else
/// non-zero is a repository git could not read, which is a question this
/// function has no business raising either — the operation it is measuring has
/// already succeeded.
fn object(repo: &Path, rev: &str) -> Option<String> {
    let out = run::git_maybe(repo, &["rev-parse", "--verify", "--quiet", rev], NO_SUCH_OBJECT)
        .ok()
        .flatten()?;
    let name = out.trim();
    (!name.is_empty()).then(|| name.to_owned())
}

/// How many commits `to` has that `from` does not. `None` for a git that
/// declined or an answer that is not a number — `object`'s rule, one call over.
fn commits_between(repo: &Path, from: &str, to: &str) -> Option<u32> {
    let out = run::git_read(repo, &["rev-list", "--count", &format!("{from}..{to}")]).ok()?;
    out.trim().parse().ok()
}

/// Files, insertions and deletions between two trees. The parse is
/// `model::parse_shortstat`, which is where the three shapes and the one
/// refusal are written down.
fn tree_delta(repo: &Path, from: &str, to: &str) -> Option<(u32, u32, u32)> {
    let out = run::git_read(repo, &["diff", "--shortstat", from, to]).ok()?;
    model::parse_shortstat(&out)
}

/// The two measurements as one record: the commits over one range, the tree
/// delta over another.
///
/// **Two ranges and not one**, which is the whole of the table this feature was
/// designed around. The commits come from the other side — what it had that
/// this one did not — where the delta is what the working tree actually became,
/// which is HEAD before against HEAD after. A range whose either end could not
/// be resolved is not measured at all, and answers `None` rather than nothing
/// having happened.
fn landed(
    repo: &Path,
    commits: (Option<&str>, Option<&str>),
    delta: (Option<&str>, Option<&str>),
) -> Landed {
    let commits = match commits {
        (Some(from), Some(to)) => commits_between(repo, from, to),
        _ => None,
    };
    let (files, insertions, deletions) = match delta {
        (Some(from), Some(to)) => tree_delta(repo, from, to),
        _ => None,
    }
    .map_or((None, None, None), |(files, insertions, deletions)| {
        (Some(files), Some(insertions), Some(deletions))
    });
    Landed { commits, files, insertions, deletions }
}

/// The tree read before, the operation, the tree read after — and
/// `new_conflicts` deciding whether this operation is what conflicted. The
/// runner is a parameter because a pull does all of this over the network and
/// none of the reasoning changes: `git pull`'s non-zero exit is exactly as
/// ambiguous as `git merge`'s.
///
/// **HEAD is read here as well, either side of the write, and this is the only
/// place it can be.** The front end could ask afterwards and would be asking
/// about a HEAD that may have moved — an agent committing into the same tree is
/// the ordinary case in this app — so the answer would be about somebody else's
/// commit with nothing on screen saying so.
///
/// Every one of those reads is allowed to fail without taking the operation
/// with it: a measurement nobody could take is `None` and never a refusal.
fn attempt_with(
    repo: &Path,
    args: &[&str],
    other: Other<'_>,
    run: fn(&Path, &[&str]) -> Result<Attempt, VcsError>,
) -> Result<MergeOutcome, VcsError> {
    let before = working_tree(repo).ok();
    let head_before = object(repo, "HEAD");
    let other_before = (!other.after).then(|| object(repo, other.rev)).flatten();
    match run(repo, args)? {
        Attempt::Done => {
            let theirs = if other.after { object(repo, other.rev) } else { other_before };
            let head_after = object(repo, "HEAD");
            Ok(MergeOutcome::Clean {
                landed: landed(
                    repo,
                    (head_before.as_deref(), theirs.as_deref()),
                    (head_before.as_deref(), head_after.as_deref()),
                ),
            })
        }
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
/// How it is read is `file_at_rev`'s, and that is where the three calls, the
/// ceiling, the binary sniff and the UTF-8 refusal are written down — once,
/// which is the point: this opens in the same editor, and a file that editor
/// already refuses to open above 2 MiB must not arrive through a second door.
#[tauri::command]
pub async fn vcs_file_at_head(repo: String, path: String) -> Result<Option<String>, VcsError> {
    off_the_runtime(move || file_at_head(Path::new(&repo), path)).await
}

/// One file as `HEAD` has it, for the diff against the working tree.
///
/// A thin caller over `file_at_rev` since the branch comparison arrived. The
/// ceiling, the binary sniff and the UTF-8 refusal are written once for exactly
/// the reason this function's own header has always claimed: a file the editor
/// refuses above 2 MiB must not arrive through a second door.
fn file_at_head(dir: &Path, path: String) -> Result<Option<String>, VcsError> {
    file_at_rev(dir, "HEAD", path)
}

/// Whether a string is an object name, and therefore safe to hand git as a
/// revision.
///
/// The front end never composes a revision of its own: it sends back a sha
/// `vcs_compare` resolved for it. So this can be as narrow as hex and lose
/// nothing, and what it buys is that no caller can smuggle a flag — `git
/// rev-parse --verify --quiet -- output=…` would be read as one, and there is no
/// `--` to hide behind in the middle of `{rev}:{path}`.
fn is_object_name(rev: &str) -> bool {
    !rev.is_empty() && rev.len() <= 64 && rev.chars().all(|c| c.is_ascii_hexdigit())
}

/// One file as `rev` has it. `path` is relative to `repo`.
///
/// `Ok(None)` is a revision that does not have the file — an added or an
/// untracked one, every file of a repository with no commit in it yet, and
/// every file one side of a comparison added. That is not a failure and must
/// not read as one: the caller diffs such a file against an empty document,
/// which is what it is.
///
/// **Three calls and each is a machine-readable question.** `git show
/// <rev>:<path>` was the obvious single call and cannot answer the first one: it
/// exits 128 for an absent path and 128 for a folder that is not a repository
/// alike, so telling the two apart would mean reading git's prose. `rev-parse
/// --verify --quiet` says the same thing in an exit code.
///
/// The two calls after it are given the **object name** rather than
/// `<rev>:<path>` a second and a third time, so the size cannot belong to one
/// blob and the bytes to another. The size is asked for before the bytes are,
/// the way `files/fs.rs` reads the metadata before it reads the file.
fn file_at_rev(dir: &Path, rev: &str, path: String) -> Result<Option<String>, VcsError> {
    let object = format!("{rev}:{path}");
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

/// One file as a given revision has it, for the branch comparison's two panes.
///
/// The revision is an object name and nothing else — see `is_object_name`. It
/// is one `vcs_compare` resolved, which is also what keeps the bytes on screen
/// belonging to the commit the file list was read from.
#[tauri::command]
pub async fn vcs_file_at_rev(
    repo: String,
    rev: String,
    path: String,
) -> Result<Option<String>, VcsError> {
    if !is_object_name(&rev) {
        return Err(VcsError::BadRevision(rev));
    }
    off_the_runtime(move || file_at_rev(Path::new(&repo), &rev, path)).await
}

/// Which of the two readings of "what has this branch changed" is being asked
/// for. The window carries both because neither is wrong and each is the wrong
/// one half the time — see the design document.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// `git diff <merge-base>..<branch>` — what the branch added since it
    /// split. What a pull request shows.
    Diverged,
    /// `git diff HEAD <branch>` — how the two trees differ right now, which
    /// also draws what the current branch added as a change being undone.
    Direct,
}

impl Mode {
    /// Anything unrecognised is the default the window opens in. A mode is a
    /// switch on screen, not a state worth a refusal.
    fn parse(name: &str) -> Self {
        if name == "direct" { Mode::Direct } else { Mode::Diverged }
    }
}

/// What differs between the current branch and another one.
///
/// The right-hand side is resolved through the **full `refs/heads/` prefix**,
/// which is what stops a branch name being read as a flag or as an ambiguous
/// rev — the name comes from the front end. The left-hand side is the literal
/// `HEAD`, never a branch name, which costs nothing and answers a detached
/// checkout for free: it compares against where the person is standing, with no
/// name to invent.
fn compare(dir: &Path, branch: &str, mode: Mode) -> Result<Comparison, VcsError> {
    let reference = format!("refs/heads/{branch}");
    let Some(right) =
        run::git_maybe(dir, &["rev-parse", "--verify", "--quiet", &reference], NO_SUCH_OBJECT)?
    else {
        return Err(VcsError::NoSuchBranch(branch.to_owned()));
    };
    let right = right.trim().to_owned();

    let left = match mode {
        Mode::Direct => run::git_read(dir, &["rev-parse", "--verify", "HEAD"])?.trim().to_owned(),
        Mode::Diverged => {
            // `merge-base` exits 1 with nothing to say when the two share no
            // history, which is the same clean "no such answer" shape
            // `rev-parse --verify --quiet` uses above.
            match run::git_maybe(dir, &["merge-base", "HEAD", &right], NO_SUCH_OBJECT)? {
                Some(base) => base.trim().to_owned(),
                None => return Err(VcsError::Unrelated),
            }
        }
    };

    let out = run::git_read(dir, &["diff", "--name-status", "-z", &left, &right])?;
    Ok(Comparison { left, right, files: parse_name_status(&out) })
}

/// What a branch differs from the current one by, for the compare window.
///
/// Read-only from end to end, which is why nothing about a run or an operation
/// in flight refuses it — see `branchMenu.js` for the other half of that rule.
#[tauri::command]
pub async fn vcs_compare(
    repo: String,
    branch: String,
    mode: String,
) -> Result<Comparison, VcsError> {
    let mode = Mode::parse(&mode);
    off_the_runtime(move || compare(Path::new(&repo), &branch, mode)).await
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

    /// Real git in a scratch repository, for the tests below that are about
    /// what git answers rather than about what we pass it. The identity is in
    /// the environment rather than in the repository: a machine with no
    /// `user.email` configured is an ordinary machine, and a test that failed
    /// there would be reporting on the machine rather than on the code.
    fn git(dir: &Path, args: &[&str]) {
        let status = std::process::Command::new("git")
            .args(args)
            .current_dir(dir)
            .env("GIT_AUTHOR_NAME", "t")
            .env("GIT_AUTHOR_EMAIL", "t@example.com")
            .env("GIT_COMMITTER_NAME", "t")
            .env("GIT_COMMITTER_EMAIL", "t@example.com")
            .status()
            .expect("run git");
        assert!(status.success(), "git {args:?}");
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

    /// A repository on `main` with one commit, and a `feature` branch holding
    /// two more. The three tests below are all about what git answers to a
    /// measurement, so nothing here is prepared: it is a real merge of a real
    /// branch.
    fn with_a_branch_to_merge(name: &str) -> PathBuf {
        let repo = repository(name);
        fs::write(repo.join("a.txt"), "one\n").expect("write a.txt");
        run::git_write(&repo, &["add", "a.txt"]).expect("stage a.txt");
        run::git_write(&repo, &["commit", "-m", "first"]).expect("commit a.txt");
        // The name git initialises with moves between versions and between
        // machines; the tests below name this branch, so it is named here.
        run::git_write(&repo, &["branch", "-M", "main"]).expect("name the branch");
        run::git_write(&repo, &["checkout", "-b", "feature"]).expect("cut the branch");
        fs::write(repo.join("b.txt"), "one\ntwo\nthree\n").expect("write b.txt");
        run::git_write(&repo, &["add", "b.txt"]).expect("stage b.txt");
        run::git_write(&repo, &["commit", "-m", "second"]).expect("commit b.txt");
        fs::write(repo.join("c.txt"), "four\n").expect("write c.txt");
        run::git_write(&repo, &["add", "c.txt"]).expect("stage c.txt");
        run::git_write(&repo, &["commit", "-m", "third"]).expect("commit c.txt");
        run::git_write(&repo, &["checkout", "main"]).expect("go back to main");
        repo
    }

    /// What the corner's phrase is written from, against real git: the commits
    /// the other branch had that this one did not, and what the working tree
    /// actually became.
    #[test]
    fn a_merge_reports_what_the_other_branch_brought() {
        let repo = with_a_branch_to_merge("merge-landed");

        let outcome = attempt(&repo, &["merge", "--no-edit", "feature"], &local_ref("feature"))
            .expect("merge");

        assert_eq!(
            outcome,
            MergeOutcome::Clean {
                landed: Landed {
                    commits: Some(2),
                    files: Some(2),
                    insertions: Some(4),
                    deletions: Some(0)
                }
            }
        );

        let _ = fs::remove_dir_all(&repo);
    }

    /// The case this whole feature exists for: git answered "Already up to
    /// date", the panel looks exactly as it did after the merge that worked,
    /// and the two mean opposite things. Both counters are a real zero here and
    /// not an unknown, which is what lets the phrase say so.
    #[test]
    fn merging_the_same_branch_twice_reports_nothing_the_second_time() {
        let repo = with_a_branch_to_merge("merge-again");
        attempt(&repo, &["merge", "--no-edit", "feature"], &local_ref("feature"))
            .expect("the first merge");

        let outcome = attempt(&repo, &["merge", "--no-edit", "feature"], &local_ref("feature"))
            .expect("the second merge");

        assert_eq!(
            outcome,
            MergeOutcome::Clean {
                landed: Landed {
                    commits: Some(0),
                    files: Some(0),
                    insertions: Some(0),
                    deletions: Some(0)
                }
            }
        );

        let _ = fs::remove_dir_all(&repo);
    }

    /// A branch nobody has published, which is what `vcs_push` measures against
    /// before it publishes one. **Every field is `null` and not a zero**: a row
    /// of zeros would say the remote already had this branch, which is the
    /// opposite of what is about to happen.
    #[test]
    fn a_branch_with_no_upstream_measures_nothing_at_all() {
        let repo = with_a_branch_to_merge("push-unpublished");
        let head = object(&repo, "HEAD");
        let upstream = object(&repo, UPSTREAM);

        assert!(head.is_some(), "a repository with a commit in it has a HEAD");
        assert_eq!(upstream, None, "nothing has been pushed from here");
        assert_eq!(
            landed(
                &repo,
                (upstream.as_deref(), head.as_deref()),
                (upstream.as_deref(), head.as_deref())
            ),
            Landed::default()
        );

        let _ = fs::remove_dir_all(&repo);
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

    /// **A clean tree is answered, not asked about.** The button is gated on the
    /// front end's count of changes and that count is as fresh as the last
    /// window focus, so pressing it against a tree an agent has since committed
    /// is an ordinary way to arrive here — and a model handed "write a commit
    /// message for the changes below" with nothing below it answers with a
    /// sentence asking for them, in its own conversational language, which then
    /// sits in the field looking like a message.
    #[test]
    fn a_clean_tree_has_nothing_to_describe_and_is_not_put_to_a_model() {
        let repo = repository("describe-clean");
        fs::write(repo.join("a.txt"), "one\n").expect("write a file");
        run::git_write(&repo, &["add", "."]).expect("stage");
        run::git_write(&repo, &["commit", "-m", "first"]).expect("commit");

        assert_eq!(describe(&repo, "English").expect("read the tree"), None);

        let _ = fs::remove_dir_all(&repo);
    }

    /// The other side of the same rule, and both halves are needed: a tree with
    /// something in it still builds the prompt, and an untracked file alone is
    /// enough — it appears in no diff at all, so a change set of nothing but new
    /// files would otherwise read as an empty one.
    #[test]
    fn an_untracked_file_alone_is_something_to_describe() {
        let repo = repository("describe-untracked");
        fs::write(repo.join("a.txt"), "one\n").expect("write a file");
        run::git_write(&repo, &["add", "."]).expect("stage");
        run::git_write(&repo, &["commit", "-m", "first"]).expect("commit");
        fs::write(repo.join("new.txt"), "two\n").expect("write the untracked file");

        let prompt = describe(&repo, "English").expect("read the tree").expect("something to say");
        assert!(prompt.contains("New files, not yet tracked by git:\nnew.txt"), "{prompt}");

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

    /// The front end never invents a revision — it sends back a sha this module
    /// gave it — so anything that is not an object name is refused before it
    /// reaches git, where a leading dash would be read as a flag.
    #[test]
    fn only_an_object_name_is_a_revision() {
        assert!(is_object_name("a1b2c3d"));
        assert!(is_object_name(&"0".repeat(40)));
        for bad in ["", "HEAD", "--output=/tmp/x", "main", "a1b2c3d^", &"a".repeat(65)] {
            assert!(!is_object_name(bad), "{bad:?}");
        }
    }

    /// The same file, asked for by sha and by HEAD, is the same file — which is
    /// what says the two doors go through one implementation.
    #[test]
    fn a_file_reads_the_same_by_sha_as_by_head() {
        let dir = scratch("file-at-rev");
        git(&dir, &["init", "-q"]);
        fs::write(dir.join("a.txt"), "one\n").expect("write the file");
        git(&dir, &["add", "-A"]);
        git(&dir, &["commit", "-qm", "one"]);

        let head = run::git_read(&dir, &["rev-parse", "HEAD"]).expect("resolve HEAD");
        let sha = head.trim();

        assert_eq!(file_at_rev(&dir, sha, "a.txt".into()).expect("read"), Some("one\n".into()));
        assert_eq!(file_at_head(&dir, "a.txt".into()).expect("read"), Some("one\n".into()));

        let _ = fs::remove_dir_all(&dir);
    }

    /// A revision that does not have the file is not a failure: it is exactly a
    /// file added on the other side, and the empty pane is the truth.
    #[test]
    fn a_revision_without_the_file_answers_none() {
        let dir = scratch("file-at-rev-absent");
        git(&dir, &["init", "-q"]);
        fs::write(dir.join("a.txt"), "one\n").expect("write the file");
        git(&dir, &["add", "-A"]);
        git(&dir, &["commit", "-qm", "one"]);
        let head = run::git_read(&dir, &["rev-parse", "HEAD"]).expect("resolve HEAD");

        assert_eq!(file_at_rev(&dir, head.trim(), "gone.txt".into()).expect("read"), None);

        let _ = fs::remove_dir_all(&dir);
    }

    /// The two readings of the same pair of branches, and the whole reason the
    /// window carries a switch: a file only the current branch touched is
    /// absent from one and present in the other.
    #[test]
    fn the_two_modes_disagree_where_the_current_branch_moved_on() {
        let dir = scratch("compare-modes");
        git(&dir, &["init", "-q", "-b", "main"]);
        fs::write(dir.join("shared.txt"), "base\n").expect("write");
        git(&dir, &["add", "-A"]);
        git(&dir, &["commit", "-qm", "base"]);

        git(&dir, &["checkout", "-qb", "feature"]);
        fs::write(dir.join("branch-only.txt"), "x\n").expect("write");
        git(&dir, &["add", "-A"]);
        git(&dir, &["commit", "-qm", "on the branch"]);

        git(&dir, &["checkout", "-q", "main"]);
        fs::write(dir.join("main-only.txt"), "y\n").expect("write");
        git(&dir, &["add", "-A"]);
        git(&dir, &["commit", "-qm", "on main"]);

        let diverged = compare(&dir, "feature", Mode::Diverged).expect("compare");
        let paths: Vec<&str> = diverged.files.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(paths, vec!["branch-only.txt"]);

        let direct = compare(&dir, "feature", Mode::Direct).expect("compare");
        let mut paths: Vec<&str> = direct.files.iter().map(|f| f.path.as_str()).collect();
        paths.sort_unstable();
        assert_eq!(paths, vec!["branch-only.txt", "main-only.txt"]);

        let _ = fs::remove_dir_all(&dir);
    }

    /// The endpoints come back resolved, because every file read afterwards is
    /// by sha: HEAD may move while the window stands open — an agent committing
    /// into this very tree is the ordinary case here.
    #[test]
    fn the_endpoints_come_back_as_object_names() {
        let dir = scratch("compare-shas");
        git(&dir, &["init", "-q", "-b", "main"]);
        fs::write(dir.join("a.txt"), "one\n").expect("write");
        git(&dir, &["add", "-A"]);
        git(&dir, &["commit", "-qm", "one"]);
        git(&dir, &["branch", "feature"]);

        let out = compare(&dir, "feature", Mode::Diverged).expect("compare");
        assert!(is_object_name(&out.left), "{:?}", out.left);
        assert!(is_object_name(&out.right), "{:?}", out.right);
        assert!(out.files.is_empty());

        let _ = fs::remove_dir_all(&dir);
    }

    /// A branch deleted while the window stood open. Its own refusal, and not
    /// git's 128 with a sentence about an ambiguous argument.
    #[test]
    fn a_branch_that_is_gone_is_refused_by_name() {
        let dir = scratch("compare-gone");
        git(&dir, &["init", "-q", "-b", "main"]);
        fs::write(dir.join("a.txt"), "one\n").expect("write");
        git(&dir, &["add", "-A"]);
        git(&dir, &["commit", "-qm", "one"]);

        assert!(matches!(
            compare(&dir, "never-existed", Mode::Diverged),
            Err(VcsError::NoSuchBranch(_))
        ));

        let _ = fs::remove_dir_all(&dir);
    }

    /// Two histories with no commit in common. "From where they diverged" has
    /// no answer, and it says so rather than quietly drawing the other mode's
    /// diff under a switch that claims this one.
    #[test]
    fn unrelated_histories_refuse_the_diverged_mode_and_allow_the_direct_one() {
        let dir = scratch("compare-unrelated");
        git(&dir, &["init", "-q", "-b", "main"]);
        fs::write(dir.join("a.txt"), "one\n").expect("write");
        git(&dir, &["add", "-A"]);
        git(&dir, &["commit", "-qm", "one"]);

        git(&dir, &["checkout", "-q", "--orphan", "other"]);
        git(&dir, &["rm", "-r", "-q", "-f", "."]);
        fs::write(dir.join("b.txt"), "two\n").expect("write");
        git(&dir, &["add", "-A"]);
        git(&dir, &["commit", "-qm", "two"]);
        git(&dir, &["checkout", "-q", "main"]);

        assert!(matches!(compare(&dir, "other", Mode::Diverged), Err(VcsError::Unrelated)));
        assert!(compare(&dir, "other", Mode::Direct).is_ok());

        let _ = fs::remove_dir_all(&dir);
    }

    /// A repository on `main` with one commit, so the delete tests below have
    /// somewhere to stand. The branches they need are cut per test, since what
    /// distinguishes them is entirely whether the branch has a commit of its
    /// own.
    fn deletable(name: &str) -> PathBuf {
        let dir = scratch(name);
        git(&dir, &["init", "-q", "-b", "main"]);
        fs::write(dir.join("a.txt"), "one\n").expect("write");
        git(&dir, &["add", "-A"]);
        git(&dir, &["commit", "-qm", "one"]);
        dir
    }

    fn names(repo: &Path) -> Vec<String> {
        branch_list(repo).into_iter().map(|branch| branch.name).collect()
    }

    /// The ordinary case: a branch whose commits are all in the current one.
    /// `git branch -d` takes it, nothing is lost, and the row goes.
    #[test]
    fn a_branch_merged_into_the_current_one_is_deleted_without_force() {
        let repo = deletable("delete-merged");
        git(&repo, &["branch", "spent"]);
        assert!(names(&repo).iter().any(|name| name == "spent"));

        delete_branch(&repo, "spent", false).expect("delete the merged branch");

        assert!(!names(&repo).iter().any(|name| name == "spent"));

        let _ = fs::remove_dir_all(&repo);
    }

    /// A branch holding a commit the current one does not. `-d` refuses, and
    /// what comes back is **this app's own variant** rather than git's prose —
    /// which is what lets the window offer a second button for exactly this
    /// case and for no other.
    #[test]
    fn a_branch_with_its_own_commits_is_refused_as_not_merged() {
        let repo = deletable("delete-unmerged");
        git(&repo, &["checkout", "-q", "-b", "work"]);
        fs::write(repo.join("b.txt"), "two\n").expect("write");
        git(&repo, &["add", "-A"]);
        git(&repo, &["commit", "-qm", "two"]);
        git(&repo, &["checkout", "-q", "main"]);

        let refused = delete_branch(&repo, "work", false).expect_err("git refuses this");

        assert_eq!(refused.kind(), "notMerged");
        assert!(names(&repo).iter().any(|name| name == "work"), "nothing was deleted");

        let _ = fs::remove_dir_all(&repo);
    }

    /// The way forward the variant above exists to unlock: the same branch,
    /// asked for again with `force`, and `git branch -D` takes it.
    #[test]
    fn the_same_branch_goes_when_the_delete_is_forced() {
        let repo = deletable("delete-forced");
        git(&repo, &["checkout", "-q", "-b", "work"]);
        fs::write(repo.join("b.txt"), "two\n").expect("write");
        git(&repo, &["add", "-A"]);
        git(&repo, &["commit", "-qm", "two"]);
        git(&repo, &["checkout", "-q", "main"]);

        delete_branch(&repo, "work", false).expect_err("refused first");
        delete_branch(&repo, "work", true).expect("forced through");

        assert!(!names(&repo).iter().any(|name| name == "work"));

        let _ = fs::remove_dir_all(&repo);
    }

    /// The branch the repository is standing on, refused **before** git is
    /// asked anything. The menu greys the item too, and this is the half that
    /// still holds when HEAD moves under an open window.
    #[test]
    fn the_branch_the_repository_is_on_is_refused_by_this_module() {
        let repo = deletable("delete-current");

        let refused = delete_branch(&repo, "main", false).expect_err("the current branch");

        assert_eq!(refused.kind(), "currentBranch");
        assert!(names(&repo).iter().any(|name| name == "main"));
        // Forcing does not get past it either: `-D` would be refused by git for
        // the same reason, and the point of the guard is that the answer is the
        // same whichever button was pressed.
        assert_eq!(
            delete_branch(&repo, "main", true).expect_err("still the current branch").kind(),
            "currentBranch"
        );

        let _ = fs::remove_dir_all(&repo);
    }

    /// A branch checked out in a second worktree. git refuses it and `-D` would
    /// not help, so the refusal has to arrive in **git's own words** and not as
    /// `NotMerged` — which is the whole reason the reason is asked for
    /// separately rather than read off the stderr.
    #[test]
    fn a_branch_held_by_another_worktree_comes_back_in_git_own_words() {
        let repo = deletable("delete-in-worktree");
        let elsewhere = repo.join("linked");
        git(&repo, &["branch", "held"]);
        git(&repo, &["worktree", "add", "-q", elsewhere.to_str().expect("path"), "held"]);

        let refused = delete_branch(&repo, "held", false).expect_err("git holds this one");

        assert_eq!(refused.kind(), "git", "not `notMerged`: forcing would fail the same way");
        assert!(names(&repo).iter().any(|name| name == "held"));

        let _ = fs::remove_dir_all(&repo);
    }

    /// A name no branch has. git refuses in its own words, and the probe behind
    /// the refusal must not turn that into something else: `merge-base
    /// --is-ancestor` cannot resolve the name either, which is a refusal rather
    /// than the "no" that means unmerged.
    #[test]
    fn a_branch_that_does_not_exist_is_refused_as_git_refused_it() {
        let repo = deletable("delete-missing");

        let refused = delete_branch(&repo, "never-existed", false).expect_err("no such branch");

        assert_eq!(refused.kind(), "git");

        let _ = fs::remove_dir_all(&repo);
    }

    /// The ordinary rename: a branch the repository is not standing on. The row
    /// changes its name and the commit stays exactly where it was — a rename
    /// moves one ref and touches history not at all, which is the difference
    /// between this and every other write in this module.
    ///
    /// The fixture is the delete tests' above, and it is what these want too: a
    /// repository on `main`, named rather than left to the machine's
    /// `init.defaultBranch`, with one commit on it.
    #[test]
    fn rename_branch_moves_the_name_and_leaves_the_commit_where_it_was() {
        let repo = deletable("rename-plain");
        git(&repo, &["branch", "spike"]);
        let was = sha(&repo, "spike");

        rename_branch(&repo, "spike", "fix/spike").expect("rename the branch");

        assert!(!names(&repo).iter().any(|name| name == "spike"));
        assert!(names(&repo).iter().any(|name| name == "fix/spike"));
        assert_eq!(sha(&repo, "fix/spike"), was);
        assert_eq!(head_branch(&repo), "main", "the tree did not move");

        let _ = fs::remove_dir_all(&repo);
    }

    /// **The current branch is renamable**, which is where this command parts
    /// company with the delete beside it: `git branch -m` renames the branch
    /// HEAD is on and HEAD travels with the ref. A typo in the name of the
    /// branch somebody is working in is the ordinary case, so there is no guard
    /// here and this is the half that says so.
    #[test]
    fn rename_branch_takes_the_branch_the_repository_is_standing_on() {
        let repo = deletable("rename-current");
        let was = sha(&repo, "HEAD");

        rename_branch(&repo, "main", "trunk").expect("rename the current branch");

        assert_eq!(head_branch(&repo), "trunk");
        assert_eq!(sha(&repo, "HEAD"), was, "HEAD travelled with the ref");
        assert!(!names(&repo).iter().any(|name| name == "main"));

        let _ = fs::remove_dir_all(&repo);
    }

    /// `-m` and not `-M`. The forced form would write over the branch that
    /// already carries the new name and lose its commits; the plain one refuses
    /// in git's own words, and both branches are still there afterwards with
    /// what they had.
    #[test]
    fn rename_branch_refuses_a_name_another_branch_already_holds() {
        let repo = deletable("rename-taken");
        git(&repo, &["branch", "spike"]);
        git(&repo, &["checkout", "-q", "-b", "taken"]);
        fs::write(repo.join("b.txt"), "two\n").expect("write");
        git(&repo, &["add", "-A"]);
        git(&repo, &["commit", "-qm", "two"]);
        git(&repo, &["checkout", "-q", "main"]);
        let held = sha(&repo, "taken");

        let refused = rename_branch(&repo, "spike", "taken").expect_err("git refuses this");

        assert_eq!(refused.kind(), "git");
        assert!(names(&repo).iter().any(|name| name == "spike"), "nothing was renamed");
        assert_eq!(sha(&repo, "taken"), held, "nothing was written over");

        let _ = fs::remove_dir_all(&repo);
    }

    /// A repository with two branches whose one file disagrees, so a merge, a
    /// rebase or a cherry-pick between them all conflict. `main` is named
    /// explicitly rather than left to the machine's `init.defaultBranch`: a
    /// test that read the branch off the machine would be reporting on the
    /// machine.
    fn conflicting(name: &str) -> PathBuf {
        let repo = repository(name);
        run::git_write(&repo, &["checkout", "-q", "-b", "main"]).expect("name the branch");
        fs::write(repo.join("f.txt"), "base\n").expect("write the file");
        run::git_write(&repo, &["add", "-A"]).expect("stage");
        run::git_write(&repo, &["commit", "-m", "base"]).expect("commit the base");
        run::git_write(&repo, &["checkout", "-q", "-b", "feature"]).expect("cut feature");
        fs::write(repo.join("f.txt"), "feature\n").expect("edit on feature");
        run::git_write(&repo, &["commit", "-am", "feature"]).expect("commit on feature");
        run::git_write(&repo, &["checkout", "-q", "main"]).expect("back to main");
        fs::write(repo.join("f.txt"), "main\n").expect("edit on main");
        run::git_write(&repo, &["commit", "-am", "main"]).expect("commit on main");
        repo
    }

    /// git stopped a merge: both names are exact, because HEAD is still on the
    /// branch being merged into and `MERGE_HEAD` is the commit coming in.
    #[test]
    fn a_stopped_merge_is_read_with_both_of_its_branches() {
        let repo = conflicting("in-progress-merge");
        let _ = run::git_write(&repo, &["merge", "feature"]);

        let answer = in_progress(&repo).expect("ask git").expect("a merge is in progress");

        assert_eq!(answer.op, OpKind::Merge);
        assert_eq!(answer.ours.as_deref(), Some("main"));
        assert_eq!(answer.theirs.as_deref(), Some("feature"));

        let _ = fs::remove_dir_all(&repo);
    }

    /// git stopped a rebase: the operation is exact and `ours` is the branch
    /// whose commit is being applied. The onto is `None` by construction — see
    /// `in_progress`.
    #[test]
    fn a_stopped_rebase_is_read_with_the_branch_it_is_moving() {
        let repo = conflicting("in-progress-rebase");
        run::git_write(&repo, &["checkout", "-q", "feature"]).expect("onto feature");
        let _ = run::git_write(&repo, &["rebase", "main"]);

        let answer = in_progress(&repo).expect("ask git").expect("a rebase is in progress");

        assert_eq!(answer.op, OpKind::Rebase);
        assert_eq!(answer.ours.as_deref(), Some("feature"));
        assert_eq!(answer.theirs, None);

        let _ = fs::remove_dir_all(&repo);
    }

    /// **The one this function exists in its current shape for.** git writes
    /// `.git/REBASE_HEAD` when a rebase stops and never removes it on the
    /// default backend, so a repository where anybody has ever finished a
    /// rebase with `--continue` answers `rev-parse -q --verify REBASE_HEAD`
    /// with a sha for the rest of its life. Asked that question alone, every
    /// later conflicted tree with no `MERGE_HEAD` reads as a rebase — and this
    /// tree is a `git cherry-pick`, where `git rebase --abort` fails with "No
    /// rebase in progress?" and "finish the rebase" is not the work.
    ///
    /// The whole point is that it is driven against real git: the stale file is
    /// git's own behaviour, and nothing that fed this function a prepared
    /// answer could see it.
    #[test]
    fn a_finished_rebase_does_not_make_the_next_conflict_a_rebase() {
        let repo = conflicting("in-progress-stale");
        // A rebase that stops, is resolved, and is carried to the end. This is
        // what leaves `REBASE_HEAD` behind.
        //
        // **The backend is pinned rather than read off the machine**, the rule
        // `conflicting` states about the branch name: `apply` sweeps
        // `REBASE_HEAD` on `--continue` where the default `merge` does not, so
        // a contributor with `rebase.backend = apply` in their global config
        // would fail this test over their own configuration. `-c` beats a
        // config file at every level, and pinning the start is enough — the
        // backend is settled when the rebase begins and `--continue` reads it
        // back out of the state git kept.
        run::git_write(&repo, &["checkout", "-q", "feature"]).expect("onto feature");
        let _ = run::git_write(&repo, &["-c", "rebase.backend=merge", "rebase", "main"]);
        fs::write(repo.join("f.txt"), "resolved\n").expect("resolve");
        run::git_write(&repo, &["add", "f.txt"]).expect("stage the resolution");
        run::git_write(&repo, &["-c", "core.editor=true", "rebase", "--continue"])
            .expect("finish the rebase");
        assert!(
            run::git_maybe(&repo, &["rev-parse", "-q", "--verify", "REBASE_HEAD"], 1)
                .expect("ask git")
                .is_some(),
            "this test is pointless unless git left REBASE_HEAD behind"
        );
        assert_eq!(in_progress(&repo).expect("ask git"), None, "the rebase is over");

        // Now a cherry-pick that conflicts, with that stale ref still there.
        // Cut from **before** main's own commit, so the patch it carries is
        // against a line main has since changed and git has to stop.
        run::git_write(&repo, &["checkout", "-q", "-b", "side", "main~1"]).expect("cut side");
        fs::write(repo.join("f.txt"), "side\n").expect("edit on side");
        run::git_write(&repo, &["commit", "-am", "side"]).expect("commit on side");
        run::git_write(&repo, &["checkout", "-q", "main"]).expect("back to main");
        let _ = run::git_write(&repo, &["cherry-pick", "side"]);
        let tree = working_tree(&repo).expect("read the tree");
        assert!(
            tree.changes.iter().any(|c| c.kind == ChangeKind::Conflicted),
            "the cherry-pick should have left the tree conflicted: {:?}",
            tree.changes
        );

        assert_eq!(
            in_progress(&repo).expect("ask git"),
            None,
            "a cherry-pick is neither of the two operations this dialog has doors for"
        );

        let _ = fs::remove_dir_all(&repo);
    }

    /// A repository sitting still. The caller never asks this of a clean tree,
    /// but the function's own answer for one is stated rather than assumed.
    #[test]
    fn a_repository_in_the_middle_of_nothing_answers_nothing() {
        let repo = conflicting("in-progress-clean");

        assert_eq!(in_progress(&repo).expect("ask git"), None);

        let _ = fs::remove_dir_all(&repo);
    }

    /// **A folder git cannot read is a refusal and never "no rebase".**
    ///
    /// The rebase arm reads exit 128 as its answer, which is git's *generic*
    /// fatal code, and git exits 128 for **both** questions here ("fatal: not a
    /// git repository"). What keeps that from turning a repository nobody can
    /// read into a confident `None` — a panel drawing no button and saying
    /// nothing about why — is that some question in this function still refuses
    /// it, and until this test nothing mechanical held that.
    ///
    /// Measured, so the guarantee is not overstated: **swapping the order of
    /// the two arms alone does not break it**, because the merge question then
    /// refuses a step later and the answer is the same. What breaks it is
    /// dropping the `MERGE_HEAD` question, or loosening the exit code it takes
    /// as an answer so that it stops refusing — both of which fail here.
    ///
    /// The idiom is `a_folder_outside_git_has_no_branches_and_no_error`'s, and
    /// the opposite answer on purpose: that command promises never to refuse,
    /// and this one carries git's own words.
    #[test]
    fn a_folder_outside_git_is_refused_rather_than_called_idle() {
        let root = scratch("in-progress-no-git");

        let refused = in_progress(&root).expect_err("git cannot read this folder");

        assert_eq!(refused.kind(), "git");

        let _ = fs::remove_dir_all(&root);
    }
}
