//! Whether a task's work has already reached the branch it was meant for.
//!
//! The other half of the question `git.rs` answers. That file finds the branch
//! a task's work was left on — by the id in the last segment of its name — out
//! of `refs/heads` and `packed-refs`, and it spawns nothing, which its own
//! header forbids. "Has it been merged" cannot be read off a ref: it is a walk
//! over commits, so it lives here, where the git binary is started anyway.
//!
//! **The predicate is `git merge-base --is-ancestor <branch> <target>` and
//! nothing else.** A merge commit naming the branch would be the obvious test
//! and it is the wrong one: the case this was written for
//! (holiday-curb-a769) was a **fast-forward**, where no merge commit exists at
//! all and the task's id appears in no commit message anywhere — which is also
//! why `bd orphans`, searching commit messages for an id, cannot see it either.
//! Ancestry sees the ordinary merge, the squash landed as one commit, and the
//! fast-forward alike, because all three end with the branch's tip reachable
//! from the target's.
//!
//! **Ancestry alone is not enough, and the gap is not a corner case.** A
//! branch cut from the target and never committed to has a tip identical to
//! the point it was cut from — a commit is always an ancestor of itself — so
//! `is_ancestor` answers yes for it exactly as it does for a fast-forward.
//! smetana-ypwh closed this way: fourteen files sat uncommitted in the task's
//! worktree, its branch had never moved off `main`'s tip, and the sweep read
//! "tip equals target tip" as "merged" and closed a task whose work had not
//! even been committed, let alone landed anywhere. Comparing the two tips
//! directly (`tip != target`) would catch that case and break the one this
//! module exists for: a fast-forward leaves the branch's tip equal to the
//! target's tip too, by definition, and that equality is not optional evidence
//! there — it is the whole of what fast-forward means. So the two cases —
//! fast-forward and never-committed — are indistinguishable from the refs
//! alone, and any fix has to look past them.
//!
//! **What breaks the tie is the task's worktree.** So before a tip equal to
//! (or descended from) the target's is read as `Merged`, `standing` asks
//! `git worktree list --porcelain` for a worktree checked out to that branch
//! and, if one exists, `git status --porcelain` inside it. Anything
//! uncommitted there downgrades the answer to `Behind` — the branch may well
//! be an ancestor of the target, but the worktree says the task is not done,
//! and that is worth more than the ref. The check only runs once
//! `is_ancestor` has already said yes, so a task with no branch, or one that
//! is plainly behind, still costs nothing beyond the ref read `git.rs` already
//! does.
//!
//! **The trade this buys is not free, and it is not the one it looks like.**
//! It is tempting to read "fast-forwarded branches have clean worktrees" into
//! the predicate above, and that reading is backwards for this project's own
//! merge mechanism. The merging step removes a task's worktree **without
//! `--force`** and treats a dirty or locked one refusing removal as a line in
//! its report rather than a stop — `git.removeWorktrees` turned off leaves
//! every worktree in place regardless — so a worktree that lingers here is
//! usually exactly the one that could not be removed *because* it was dirty.
//! The clean ones are the ones that already went. So this predicate does
//! permanently withhold the closure from a task somebody merged by hand whose
//! worktree was left behind dirty — an editor with an unsaved buffer, a stray
//! build artefact, a file the merge itself never touched. That is accepted
//! for the reason the module opens with: the two ways of being wrong are not
//! symmetric, and a task withheld this way is not lost, only stuck in
//! `ready_to_merge` where a person can still close it by hand; a task closed
//! on a guess is not recoverable at all.
//!
//! **The main checkout is a worktree too, as far as `git worktree list` is
//! concerned**, and nothing here treats it differently. If a task's branch
//! happens to be checked out in the main checkout rather than a linked one —
//! an agent working directly on it, or a person who checked it out themselves
//! — the status read lands there, and *that checkout's* dirt, whatever it is
//! about, withholds the closure exactly as a linked worktree's would. That is
//! correct under this predicate, not a gap in it: the tree the branch is
//! sitting in has uncommitted changes either way, and this module has no way
//! to know that they belong to some other piece of work.
//!
//! Rejected: requiring `git rev-list <target>..<branch>` to be non-empty —
//! "the branch has a commit the target lacks" is a test for "not yet merged
//! by rebase or squash", and an honestly fast-forwarded branch fails it on
//! purpose, which is exactly the case that must keep closing. Reading the
//! branch's reflog for the commit it was cut from was rejected too: a reflog
//! expires, and a fresh clone — which is what a project's repository often is
//! by the time this sweep runs — has none to read.
//!
//! Local refs only. Nothing here asks a remote anything: a fetch would put a
//! network call on a timer, and a branch merged only on somebody's server is
//! not merged on this machine, which is the one this app can see.

use std::path::Path;

use super::model::VcsError;
use super::{repos, run};

/// The exit code `git merge-base --is-ancestor` uses for "no". Every other
/// non-zero exit is a refusal — a name git cannot resolve, a repository it
/// cannot read — and must not be read as an answer.
const NOT_AN_ANCESTOR: i32 = 1;

/// Whether every commit of `of` is already in `into`.
///
/// Both are handed to git exactly as given: this takes a revision, not a branch
/// name, so `HEAD` works beside `refs/heads/feature/x`. The one caller that
/// wants the ambiguity resolved spells the full ref itself, which is the only
/// place the choice belongs — a prefix added here would break the other caller.
///
/// Exit 0 is yes, exit 1 is no, anything else is git refusing in its own words.
pub fn is_ancestor(repo: &Path, of: &str, into: &str) -> Result<bool, VcsError> {
    run::git_maybe(repo, &["merge-base", "--is-ancestor", of, into], NOT_AN_ANCESTOR)
        .map(|out| out.is_some())
}

/// Whether `branch` has a worktree of its own here and, if it does, that
/// worktree has anything uncommitted in it.
///
/// `git worktree list --porcelain` is one call shared by every worktree of a
/// repository, so asking it of `repo` — whichever checkout `standing` was
/// handed — sees a linked worktree cut for the task even though the task's own
/// edits never touch this one. Entries are separated by blank lines and each
/// names its path first (`worktree <path>`) and its branch after
/// (`branch refs/heads/<name>`), so the path most recently seen when the
/// wanted branch line turns up is the one asked for a status.
///
/// A branch with no worktree at all — the ordinary state of one merged days
/// ago, whose worktree somebody has since removed — is **not** treated as
/// dirty: there is nowhere left for uncommitted work to be sitting, so the
/// absence itself is not evidence of anything and must not block the closure
/// fast-forward exists to make happen. **A worktree record whose directory is
/// gone is the same case, not the error case.** git does not drop a worktree
/// record when the directory under it disappears — `rm -rf` on a gitignored,
/// routinely-cleaned folder, a move, an unmounted volume — it keeps printing
/// the `branch` line with `prunable gitdir file points to non-existent
/// location` beside it, for as long as nobody runs `git worktree prune` by
/// hand — no code in this application ever runs it; the only occurrences in
/// the tree are steps inside shipped skill documents, taken by an agent for
/// another purpose and never on a timer. Spawning `git status` there would
/// not read a status at all: `Command::current_dir` fails before git even
/// starts, and treating that failure as "unmeasurable" would withhold the
/// closure forever over a directory that cannot hold uncommitted work because
/// it does not exist — the same permanent loss this predicate exists to stop,
/// arrived at from the other side. So the path is checked with `is_dir`
/// first, off the same stat this app already trusts elsewhere, and a path
/// that is not a directory reads exactly as no worktree does: not dirty.
///
/// An error reading the list, or reading the status of a worktree whose
/// directory does exist, is the genuine unmeasurable case: git could not be
/// asked, so there is no evidence either way, and per the module header's
/// stated asymmetry it is read as "yes, treat it as unfinished" rather than
/// "no" — a closure skipped for one more sweep costs a minute, a closure
/// granted on a guess can lose work for good. It is also logged, the way
/// `standing`'s own `Err` arm already is: a closure withheld for a reason
/// nobody can see is a defect nobody can find, and silence here is exactly
/// what let a `VcsError::NoGit` — "git is not installed" — stand in for "the
/// worktree's own directory is gone" with nothing to say so.
fn worktree_has_uncommitted_work(repo: &Path, branch: &str) -> bool {
    let Ok(list) = run::git_read(repo, &["worktree", "list", "--porcelain"]) else {
        return true;
    };
    let wanted = format!("branch refs/heads/{branch}");
    let mut path: Option<&str> = None;
    for line in list.lines() {
        if let Some(p) = line.strip_prefix("worktree ") {
            path = Some(p);
        } else if line == wanted {
            let Some(path) = path else { return true };
            if !Path::new(path).is_dir() {
                return false;
            }
            return match run::git_read(Path::new(path), &["status", "--porcelain"]) {
                Ok(status) => !status.trim().is_empty(),
                Err(err) => {
                    log::debug!(
                        "could not read the status of {branch}'s worktree at {path}: {err}"
                    );
                    true
                }
            };
        }
    }
    false
}

/// What one repository has to say about one task.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Standing {
    /// There is no branch of this task's here. The repository has no opinion:
    /// a project's repositories are not all touched by every task, and one that
    /// was not touched must not hold a closure up.
    Absent,
    /// The branch is here, its tip is an ancestor of the target's, and its
    /// worktree — if it has one — has nothing uncommitted in it.
    Merged { branch: String, tip: String },
    /// The branch is here and its tip is not in the target, or the tip is in
    /// the target but its worktree still has uncommitted work, or the question
    /// could not be put to git at all, which is the same answer on purpose. A
    /// repository that cannot be read is not evidence that work was merged, and
    /// the two ways of being wrong here are not equal: a task closed early
    /// loses the work somebody still has to merge, while a task left open costs
    /// one sweep sixty seconds later.
    Behind,
}

/// A task whose branch is in the target branch everywhere it exists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergedTask {
    pub id: String,
    /// The branch the work was left on, and the short sha of its tip — what the
    /// closing note is written from, so a person reading the board can see
    /// where the closure came from.
    pub branch: String,
    pub tip: String,
}

/// The rule, and the whole of what decides a closure: **merged in every
/// repository that has the branch, and in at least one.**
///
/// Two ways of being wrong it is arranged against. A task whose branch is in
/// `backend` and not in `frontend` is half done, and closing it would say the
/// work is finished while a repository still has it outstanding — so one
/// `Behind` refuses the lot. And a repository the task never touched has no
/// branch to be behind on, so counting it as a "no" would mean nothing in a
/// multi-repository project ever closed.
///
/// A task with no branch anywhere is not closed either, which is the same rule
/// read from its other end: there is no evidence at all, and a slug nobody cut
/// a branch for is exactly the case where a guess would be wrong.
///
/// The branch and tip handed back are the first merged repository's. In a
/// project where several repositories carry the task's branch they are the same
/// name with different tips, and the note quotes one of them: the order is
/// `[project].repos`', so it is stable, and a note pointing at one of them is
/// worth more than a note pointing at none — the same judgement `git.rs` makes
/// when a task has two branches.
pub fn merged_in_all(standings: &[Standing]) -> Option<(&str, &str)> {
    let mut landed = None;
    for standing in standings {
        match standing {
            Standing::Behind => return None,
            Standing::Merged { branch, tip } => {
                landed.get_or_insert((branch.as_str(), tip.as_str()));
            }
            Standing::Absent => {}
        }
    }
    landed
}

/// One repository's answer about one task.
///
/// The branch is found by reading refs (`git::task_work`, no process at all),
/// so a repository with nothing of this task's in it costs no spawn — which is
/// what makes asking about every task on a timer affordable.
///
/// The full `refs/heads/<branch>` is what git is asked about rather than the
/// bare name: a tag and a branch may share one, and the target here is a *local*
/// branch by decision, so naming it exactly is what keeps a remote-tracking ref
/// of the same name out of the answer.
///
/// Ancestry alone only gets to a candidate, not an answer: once it says yes,
/// `worktree_has_uncommitted_work` gets one more look before the task is
/// called `Merged`, for the reason the module header gives. It runs nowhere
/// else, so a branch that is plainly behind, or absent, still costs exactly
/// the one `merge-base` call it cost before this existed.
pub fn standing(repo: &Path, id: &str, target: &str) -> Standing {
    let Some((branch, tip)) = crate::git::task_work(repo, id) else {
        return Standing::Absent;
    };
    match is_ancestor(repo, &format!("refs/heads/{branch}"), &format!("refs/heads/{target}")) {
        Ok(true) if worktree_has_uncommitted_work(repo, &branch) => Standing::Behind,
        Ok(true) => Standing::Merged { branch, tip },
        Ok(false) => Standing::Behind,
        // Ordinary rather than alarming, and that is why it is not a warning:
        // the likeliest cause is a repository that simply has no branch by the
        // target's name — a perfectly normal state in a project whose
        // repositories are not kept in step — and this runs on a timer, so a
        // warning would repeat once a minute for ever.
        Err(err) => {
            log::debug!("could not tell whether {branch} is in {target}: {err}");
            Standing::Behind
        }
    }
}

/// Which of these tasks have landed, across every repository the project has.
///
/// The repository list is `repos::discover`'s — `[project].repos` where there
/// is one, the root and its immediate repositories where there is not — so this
/// sweep and the Git panel are looking at the same project. A folder git cannot
/// read is left out by that same function rather than counted as a repository
/// missing every branch.
pub fn merged_tasks(root: &Path, target: &str, ids: &[String]) -> Vec<MergedTask> {
    let repos = repos::discover(root).repos;
    ids.iter()
        .filter_map(|id| {
            let standings: Vec<Standing> =
                repos.iter().map(|repo| standing(Path::new(&repo.path), id, target)).collect();
            let (branch, tip) = merged_in_all(&standings)?;
            Some(MergedTask { id: id.clone(), branch: branch.to_owned(), tip: tip.to_owned() })
        })
        .collect()
}

/// The sentence the board carries under a task the app closed by itself.
///
/// It names the branch, the short sha of its tip and the branch it reached,
/// because a closure nobody performed has to say where it came from: without
/// those three, "closed" is indistinguishable from an agent's own closing step
/// or from somebody's slip.
pub fn reason(task: &MergedTask, target: &str) -> String {
    format!("{} ({}) is already in {target}; closed automatically", task.branch, task.tip)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn merged(branch: &str, tip: &str) -> Standing {
        Standing::Merged { branch: branch.into(), tip: tip.into() }
    }

    #[test]
    fn a_branch_in_the_target_everywhere_it_exists_is_merged() {
        let standings = [merged("feature/x-a769", "a1b2c3d"), Standing::Absent];
        assert_eq!(merged_in_all(&standings), Some(("feature/x-a769", "a1b2c3d")));
    }

    /// The half-done case, and the reason the rule is "every repository" rather
    /// than "any": merged in the backend and outstanding in the frontend is
    /// work somebody still has to finish.
    #[test]
    fn one_repository_still_holding_the_work_refuses_the_whole_task() {
        let standings = [merged("feature/x-a769", "a1b2c3d"), Standing::Behind];
        assert_eq!(merged_in_all(&standings), None);
    }

    /// The other half of the same rule: a repository the task never touched has
    /// no branch to be behind on, and counting it would close nothing ever.
    #[test]
    fn a_repository_without_the_branch_does_not_hold_a_closure_up() {
        let standings = [Standing::Absent, merged("feature/x-a769", "a1b2c3d"), Standing::Absent];
        assert_eq!(merged_in_all(&standings), Some(("feature/x-a769", "a1b2c3d")));
    }

    #[test]
    fn a_task_with_no_branch_anywhere_is_not_closed() {
        assert_eq!(merged_in_all(&[Standing::Absent, Standing::Absent]), None);
        assert_eq!(merged_in_all(&[]), None);
    }

    /// A note pointing at one branch beats a note pointing at none, so the
    /// first repository to have landed the work is the one quoted.
    #[test]
    fn the_note_quotes_the_first_repository_that_landed_it() {
        let standings = [merged("feature/x-a769", "aaaaaaa"), merged("feature/x-a769", "bbbbbbb")];
        assert_eq!(merged_in_all(&standings), Some(("feature/x-a769", "aaaaaaa")));
    }

    #[test]
    fn the_note_names_the_branch_its_tip_and_where_it_landed() {
        let task = MergedTask {
            id: "holiday-curb-a769".into(),
            branch: "feature/holiday-curb-a769-nano-banana-2".into(),
            tip: "a1b2c3d".into(),
        };
        assert_eq!(
            reason(&task, "develop"),
            "feature/holiday-curb-a769-nano-banana-2 (a1b2c3d) is already in develop; \
             closed automatically"
        );
    }

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("smetana-merged-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("make the scratch directory");
        dir
    }

    /// A repository on `develop` with one commit. The default branch name is a
    /// machine's configuration rather than this test's business, so it is named
    /// here.
    fn repository(root: &Path, name: &str) -> PathBuf {
        let repo = if name == "." { root.to_path_buf() } else { root.join(name) };
        fs::create_dir_all(&repo).expect("make the repository directory");
        run::git_write(&repo, &["init", "--quiet"]).expect("git init");
        run::git_write(&repo, &["config", "user.email", "test@example.com"]).expect("set the email");
        run::git_write(&repo, &["config", "user.name", "Test"]).expect("set the name");
        fs::write(repo.join("a.txt"), "one\n").expect("write a file");
        run::git_write(&repo, &["add", "."]).expect("stage");
        run::git_write(&repo, &["commit", "-m", "first"]).expect("commit");
        run::git_write(&repo, &["branch", "-M", "develop"]).expect("name the branch");
        repo
    }

    /// A branch off `develop` with one commit on it, whose message mentions
    /// nothing — the id travels in the branch name and nowhere else, which is
    /// the whole of the case this module exists for.
    fn cut(repo: &Path, branch: &str, file: &str) {
        run::git_write(repo, &["checkout", "-q", "-b", branch]).expect("cut the branch");
        fs::write(repo.join(file), "two\n").expect("write a file");
        run::git_write(repo, &["add", "."]).expect("stage");
        run::git_write(repo, &["commit", "-m", "work"]).expect("commit");
        run::git_write(repo, &["checkout", "-q", "develop"]).expect("go back");
    }

    /// A worktree checked out to a task's branch and nowhere near it: cutting
    /// a fresh branch from the target and never committing anything to it
    /// leaves the tip identical to the point it was cut from, and this is the
    /// case smetana-ypwh closed by mistake. `is_ancestor` alone cannot tell it
    /// apart from a fast-forward — both answer "the tip is an ancestor of the
    /// target" — so only the worktree's own dirtiness does.
    #[test]
    fn a_branch_at_the_targets_tip_with_a_dirty_worktree_is_not_closed() {
        let root = scratch("dirty-worktree");
        let repo = repository(&root, ".");
        let worktree = root.join("task-worktree");
        run::git_write(
            &repo,
            &[
                "worktree",
                "add",
                worktree.to_str().expect("a UTF-8 worktree path"),
                "-b",
                "feature/smetana-a769-work",
                "develop",
            ],
        )
        .expect("add the linked worktree");
        fs::write(worktree.join("uncommitted.txt"), "not committed\n")
            .expect("write an uncommitted file");

        assert_eq!(standing(&repo, "smetana-a769", "develop"), Standing::Behind);
        assert!(merged_tasks(&root, "develop", &["smetana-a769".to_string()]).is_empty());

        let _ = fs::remove_dir_all(&root);
    }

    /// The same branch at the same tip, but with nothing outstanding in its
    /// worktree: this is the fast-forward case seen from the worktree's side
    /// rather than the ref's, and the fix must not have broken it.
    #[test]
    fn a_branch_at_the_targets_tip_with_a_clean_worktree_still_closes() {
        let root = scratch("clean-worktree");
        let repo = repository(&root, ".");
        let worktree = root.join("task-worktree");
        run::git_write(
            &repo,
            &[
                "worktree",
                "add",
                worktree.to_str().expect("a UTF-8 worktree path"),
                "-b",
                "feature/smetana-a769-work",
                "develop",
            ],
        )
        .expect("add the linked worktree");

        let head = run::git_read(&repo, &["rev-parse", "HEAD"]).expect("read the tip");
        assert_eq!(
            standing(&repo, "smetana-a769", "develop"),
            Standing::Merged {
                branch: "feature/smetana-a769-work".into(),
                tip: head.trim().chars().take(7).collect(),
            }
        );

        let _ = fs::remove_dir_all(&root);
    }

    /// The stale-record case: somebody removed the worktree's directory by
    /// hand instead of running `git worktree remove`, so git keeps the
    /// `branch` record — this is `prunable gitdir file points to
    /// non-existent location`, reproduced against a real repository rather
    /// than assumed. Without the directory check this reads as unmeasurable
    /// and answers `Behind` forever, since no code in this application ever
    /// runs `git worktree prune` — the only occurrences in the tree are steps
    /// inside shipped skill documents, taken by an agent for another purpose
    /// and never on a timer; with the check, it reads exactly as no worktree
    /// does.
    #[test]
    fn a_worktree_record_whose_directory_is_gone_does_not_withhold_the_closure() {
        let root = scratch("removed-worktree-dir");
        let repo = repository(&root, ".");
        let worktree = root.join("task-worktree");
        run::git_write(
            &repo,
            &[
                "worktree",
                "add",
                worktree.to_str().expect("a UTF-8 worktree path"),
                "-b",
                "feature/smetana-a769-work",
                "develop",
            ],
        )
        .expect("add the linked worktree");
        // `rm -rf` on the directory, not `git worktree remove`: the record in
        // `.git/worktrees/` is left standing, exactly as it is after a
        // gitignored worktree folder is cleaned up by hand.
        fs::remove_dir_all(&worktree).expect("remove the worktree directory by hand");
        let list = run::git_read(&repo, &["worktree", "list", "--porcelain"])
            .expect("read the worktree list");
        assert!(
            list.contains("branch refs/heads/feature/smetana-a769-work"),
            "the record must survive the directory going: {list}"
        );

        let head = run::git_read(&repo, &["rev-parse", "HEAD"]).expect("read the tip");
        assert_eq!(
            standing(&repo, "smetana-a769", "develop"),
            Standing::Merged {
                branch: "feature/smetana-a769-work".into(),
                tip: head.trim().chars().take(7).collect(),
            }
        );

        let _ = fs::remove_dir_all(&root);
    }

    /// The case the whole feature came from: a fast-forward merge leaves no
    /// merge commit and no mention of the task's id in any message, so nothing
    /// that reads commit messages can see it. Ancestry can.
    #[test]
    fn a_fast_forward_merge_is_seen_although_no_commit_names_the_task() {
        let root = scratch("fast-forward");
        let repo = repository(&root, ".");
        cut(&repo, "feature/smetana-a769-work", "b.txt");
        run::git_write(&repo, &["merge", "--ff-only", "feature/smetana-a769-work"])
            .expect("fast-forward");

        let log = run::git_read(&repo, &["log", "--format=%s%n%b"]).expect("read the log");
        assert!(!log.contains("smetana-a769"), "no commit names the task: {log}");
        // git's own abbreviation length, taken off the full sha rather than
        // asked of `--short`: that flag lengthens the answer when a repository
        // has a collision, and `git::task_work` truncates instead.
        let head = run::git_read(&repo, &["rev-parse", "HEAD"]).expect("read the tip");
        assert_eq!(
            merged_tasks(&root, "develop", &["smetana-a769".to_string()]),
            vec![MergedTask {
                id: "smetana-a769".into(),
                branch: "feature/smetana-a769-work".into(),
                tip: head.trim().chars().take(7).collect(),
            }]
        );

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn an_ordinary_merge_commit_is_seen_too() {
        let root = scratch("merge-commit");
        let repo = repository(&root, ".");
        cut(&repo, "feature/smetana-a769-work", "b.txt");
        // A commit on the target as well, so the merge cannot fast-forward.
        fs::write(repo.join("c.txt"), "three\n").expect("write a file");
        run::git_write(&repo, &["add", "."]).expect("stage");
        run::git_write(&repo, &["commit", "-m", "meanwhile"]).expect("commit");
        run::git_write(&repo, &["merge", "--no-ff", "-m", "merge", "feature/smetana-a769-work"])
            .expect("merge");

        let found = merged_tasks(&root, "develop", &["smetana-a769".to_string()]);
        assert_eq!(found.len(), 1, "{found:?}");

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_branch_nobody_merged_is_left_alone() {
        let root = scratch("unmerged");
        let repo = repository(&root, ".");
        cut(&repo, "feature/smetana-a769-work", "b.txt");

        assert_eq!(standing(&repo, "smetana-a769", "develop"), Standing::Behind);
        assert!(merged_tasks(&root, "develop", &["smetana-a769".to_string()]).is_empty());

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_task_nobody_cut_a_branch_for_is_absent_rather_than_behind() {
        let root = scratch("no-branch");
        let repo = repository(&root, ".");

        assert_eq!(standing(&repo, "smetana-a769", "develop"), Standing::Absent);
        assert!(merged_tasks(&root, "develop", &["smetana-a769".to_string()]).is_empty());

        let _ = fs::remove_dir_all(&root);
    }

    /// A target branch this repository does not have is not evidence that
    /// anything was merged: git refuses the question, and a refusal reads as
    /// "no".
    #[test]
    fn a_target_branch_that_is_not_here_reads_as_not_merged() {
        let root = scratch("no-target");
        let repo = repository(&root, ".");
        cut(&repo, "feature/smetana-a769-work", "b.txt");

        assert_eq!(standing(&repo, "smetana-a769", "nothing-by-that-name"), Standing::Behind);

        let _ = fs::remove_dir_all(&root);
    }

    /// Two repositories, one of which never saw this task: it must not hold the
    /// closure up, and the note comes from the one that did.
    #[test]
    fn a_second_repository_without_the_branch_does_not_stop_the_closure() {
        let root = scratch("two-repos-absent");
        fs::create_dir_all(root.join("backend")).expect("make the folder");
        fs::create_dir_all(root.join("frontend")).expect("make the folder");
        let backend = repository(&root, "backend");
        repository(&root, "frontend");
        cut(&backend, "feature/smetana-a769-work", "b.txt");
        run::git_write(&backend, &["merge", "--ff-only", "feature/smetana-a769-work"])
            .expect("fast-forward");

        let found = merged_tasks(&root, "develop", &["smetana-a769".to_string()]);
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].branch, "feature/smetana-a769-work");

        let _ = fs::remove_dir_all(&root);
    }

    /// The same two repositories with the branch in both, merged in one only:
    /// half-merged work stays open.
    #[test]
    fn a_task_merged_in_one_repository_of_two_stays_open() {
        let root = scratch("two-repos-half");
        fs::create_dir_all(root.join("backend")).expect("make the folder");
        fs::create_dir_all(root.join("frontend")).expect("make the folder");
        let backend = repository(&root, "backend");
        let frontend = repository(&root, "frontend");
        cut(&backend, "feature/smetana-a769-work", "b.txt");
        cut(&frontend, "feature/smetana-a769-work", "b.txt");
        run::git_write(&backend, &["merge", "--ff-only", "feature/smetana-a769-work"])
            .expect("fast-forward");

        assert!(merged_tasks(&root, "develop", &["smetana-a769".to_string()]).is_empty());

        let _ = fs::remove_dir_all(&root);
    }
}
