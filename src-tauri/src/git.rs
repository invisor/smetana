//! Which branch the project is on.
//!
//! Reading, not running. `git rev-parse --abbrev-ref HEAD` would cost a
//! process spawn for one line of text that git keeps in plain form on disk,
//! and the scope bar asks for it on every window focus. So this is the same
//! shape as `files/`: no worker, no queue, no watcher — a couple of pure
//! functions that carry the tests, and one thin command over them. Freshness
//! comes from window focus, exactly like the file tree's.
//!
//! Nothing here is an error. A folder that is not a repository, a `.git` that
//! cannot be read, a HEAD in a shape this does not recognise — all of them
//! mean the same thing to the bar: it has no branch to show. A failure toast
//! for "this folder is not a git repository" would be noise about a state
//! that is perfectly ordinary.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Serialize;

/// What HEAD points at. Both fields empty means "no branch to show" — the
/// only two ways to be interesting are being on a branch and being detached.
#[derive(Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Head {
    pub branch: Option<String>,
    /// A short hash, when HEAD names a commit instead of a branch.
    pub detached: Option<String>,
}

/// How many characters of a detached HEAD's hash to show — git's own default
/// for an abbreviated object name.
const SHORT_HASH: usize = 7;

/// `HEAD`'s content. A symbolic ref names the branch; anything else that looks
/// like an object name is a detached HEAD.
///
/// `refs/heads/` is stripped because that prefix is on every branch and says
/// nothing; a ref outside it (a bare `refs/tags/...`, which git itself writes
/// during some operations) keeps its path, since dropping the prefix there
/// would make a tag look like a branch of the same name.
pub fn parse_head(contents: &str) -> Head {
    let line = contents.trim();
    if let Some(target) = line.strip_prefix("ref:") {
        let target = target.trim();
        if target.is_empty() {
            return Head::default();
        }
        let branch = target.strip_prefix("refs/heads/").unwrap_or(target);
        return Head { branch: Some(branch.to_owned()), detached: None };
    }
    // An object name and nothing else. Length is deliberately not checked
    // against 40 or 64: git is on its way to sha256, and "hex and long
    // enough" is the part of that which is not going to change.
    let hex = line.len() >= SHORT_HASH && line.chars().all(|c| c.is_ascii_hexdigit());
    if hex {
        return Head { branch: None, detached: Some(line[..SHORT_HASH].to_owned()) };
    }
    Head::default()
}

/// The `.git` of a linked worktree is a file, not a directory: one
/// `gitdir: <path>` line pointing into the main repository's
/// `.git/worktrees/<name>`, where that worktree's own HEAD lives. The path is
/// usually absolute, but git accepts a relative one, and it is relative to the
/// directory holding the `.git` file.
pub fn parse_gitdir(contents: &str, project: &Path) -> Option<PathBuf> {
    let path = contents.trim().strip_prefix("gitdir:")?.trim();
    if path.is_empty() {
        return None;
    }
    let path = Path::new(path);
    Some(if path.is_absolute() { path.to_path_buf() } else { project.join(path) })
}

/// The project's git directory: `.git` itself in an ordinary clone, whatever
/// the `.git` file points at in a linked worktree.
///
/// This is the per-worktree half of the repository, and only a handful of
/// things live in it: `HEAD`, `ORIG_HEAD`, the index and `logs/HEAD`. Anything
/// shared — `refs/heads/`, `packed-refs`, `logs/refs/heads/` — is in the common
/// directory instead, which is what [`common_dir`] resolves.
///
/// Public because `runs::commands::target_branches` needs to tell a directory
/// that is not a repository from a repository with nothing in it: both hand
/// back an empty branch list, and only this can say which happened.
pub fn git_dir(project: &Path) -> Option<PathBuf> {
    let dot_git = project.join(".git");
    if dot_git.is_dir() {
        return Some(dot_git);
    }
    parse_gitdir(&std::fs::read_to_string(&dot_git).ok()?, project)
}

/// Where the shared half of the repository is, out of a `commondir` file.
///
/// A linked worktree's git directory holds a `commondir` line naming the
/// repository everything else is shared with — usually the relative `../..`,
/// which from `.git/worktrees/<name>` is `.git`, and it is relative to that
/// directory rather than to the checkout. An absolute path is accepted too,
/// which is what git writes when the worktree was made with one.
pub fn parse_commondir(contents: &str, git_dir: &Path) -> Option<PathBuf> {
    let path = contents.trim();
    if path.is_empty() {
        return None;
    }
    let path = Path::new(path);
    Some(if path.is_absolute() { path.to_path_buf() } else { git_dir.join(path) })
}

/// The directory the refs actually live in.
///
/// A main checkout has no `commondir` file at all — it *is* the common
/// directory — and so is the answer whenever the file is missing, empty or
/// unreadable, the same "nothing here is an error" this whole file keeps to.
fn common_dir(git_dir: &Path) -> PathBuf {
    std::fs::read_to_string(git_dir.join("commondir"))
        .ok()
        .and_then(|text| parse_commondir(&text, git_dir))
        .unwrap_or_else(|| git_dir.to_path_buf())
}

pub fn head(project: &Path) -> Head {
    let Some(dir) = git_dir(project) else { return Head::default() };
    std::fs::read_to_string(dir.join("HEAD")).map(|text| parse_head(&text)).unwrap_or_default()
}

#[tauri::command]
pub fn git_head(project: String) -> Head {
    head(Path::new(&project))
}

/// The prefix every local branch ref carries.
const HEADS: &str = "refs/heads/";

/// Branch names out of a `packed-refs` file.
///
/// Reading `refs/heads/` alone would be wrong in the case that matters most: a
/// fresh clone has almost nothing loose on disk, git having packed the lot, so
/// a person who just cloned would be offered one branch out of forty. The
/// format is one `<sha> <ref>` per line, `#` for the header, and `^<sha>` on a
/// line of its own for what an annotated tag points at — that last one has no
/// ref name at all and is skipped rather than parsed into a branch called `^…`.
pub fn parse_packed_refs(contents: &str) -> Vec<String> {
    parse_packed_heads(contents).into_iter().map(|(name, _)| name).collect()
}

/// The same file, with each branch's commit still attached: `(name, sha)`.
///
/// The parser lives here and `parse_packed_refs` is a projection of it rather
/// than a second pass over the same lines — the header, the `^` continuation
/// and the `refs/heads/` prefix are one rule, and two copies of it would agree
/// until somebody fixed one.
pub fn parse_packed_heads(contents: &str) -> Vec<(String, String)> {
    contents
        .lines()
        .filter(|line| !line.starts_with('#') && !line.starts_with('^'))
        .filter_map(|line| line.split_once(' '))
        .filter_map(|(sha, name)| {
            name.trim().strip_prefix(HEADS).map(|name| (name.to_owned(), sha.trim().to_owned()))
        })
        .collect()
}

/// The unix timestamp out of one `logs/refs/heads/<branch>` line.
///
/// The format is `<old sha> <new sha> <who> <unix time> <zone>`, a tab, then
/// what was done. The name and the email in the middle can hold spaces, so the
/// two numbers are counted from the end of that first half rather than from
/// its start. Anything that does not read that way — an empty line, a file
/// somebody truncated, a format git has yet to invent — is `None`, which the
/// caller treats as "no local history", not as a failure.
pub fn parse_reflog_time(line: &str) -> Option<i64> {
    let entry = line.split_once('\t').map_or(line, |(entry, _)| entry);
    let fields: Vec<&str> = entry.split_whitespace().collect();
    // Both shas, an identity of at least one field, the time and the zone: four
    // is the shortest a real entry gets, and the guard is what stops a stray
    // pair of numbers somewhere else in the file reading as a timestamp.
    if fields.len() < 4 {
        return None;
    }
    fields[fields.len() - 2].parse().ok()
}

/// When this machine last moved a branch, from its own reflog.
///
/// The whole file is read for its last line, which sounds worse than it is:
/// these are a few kilobytes each, git expires them at ninety days, and the
/// list is built once when the run dialog opens rather than on every window
/// focus the way the scope bar's HEAD is. Reading backwards from the end would
/// buy nothing measurable and cost a seek loop over a text format.
fn touched_at(logs: &Path, branch: &str) -> Option<i64> {
    let contents = std::fs::read_to_string(logs.join(branch)).ok()?;
    contents.lines().rev().find_map(parse_reflog_time)
}

/// The order the dialog offers branches in: what was worked on here most
/// recently, first.
///
/// Freshness is deliberately local activity rather than commit date. A branch
/// a colleague pushed to an hour ago, never touched on this machine, is not
/// what this person is about to merge into; the branch they merged into
/// yesterday is. So a branch with no reflog does not sort as "very old" — it
/// sorts outside the recency group entirely, into the alphabetical tail, which
/// is where a fresh clone leaves nearly everything.
pub fn by_recency(mut branches: Vec<(String, Option<i64>)>) -> Vec<String> {
    branches.sort_by(|(a, at), (b, bt)| match (at, bt) {
        (Some(x), Some(y)) => y.cmp(x).then_with(|| a.cmp(b)),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => a.cmp(b),
    });
    branches.into_iter().map(|(name, _)| name).collect()
}

/// Every loose ref under `refs/heads/`, with the directories folded back into
/// the name: `feature/x` is a directory and a file on disk and one branch to
/// everybody else.
fn loose_branches(heads: &Path, prefix: &str, out: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(heads) else { return };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let full = if prefix.is_empty() { name } else { format!("{prefix}/{name}") };
        match entry.file_type() {
            Ok(kind) if kind.is_dir() => loose_branches(&entry.path(), &full, out),
            Ok(_) => out.push(full),
            Err(_) => {}
        }
    }
}

/// A branch a run could merge into, and the repositories that do not have it.
///
/// `missing_in` rather than `present_in` because the absence is what the dialog
/// draws and what somebody has to decide about: an empty list is the ordinary
/// case and there is nothing to do with it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BranchOption {
    pub name: String,
    /// In the order the repositories were given, which is `[project].repos`'s
    /// order and therefore the project's own statement about what depends on
    /// what.
    pub missing_in: Vec<String>,
}

/// One list per repository, folded into one list of options.
///
/// Three rules, and the middle one is the only new judgement here. A branch's
/// freshness is the **most recent** touch across the repositories that have it:
/// `develop` opened an hour ago in `backend` and a month ago in `admin` is an
/// hour old, because it is one branch to the person merging into it. Ordering
/// inside a group is `by_recency`'s and deliberately not a second rule written
/// here. And complete comes before partial, which is the whole of the structure
/// the field draws.
///
/// A repository that git cannot see must never reach this — see
/// `runs::commands::target_branches`, which is where that is decided, because
/// an empty list here cannot say whether the directory had no branches or was
/// not a repository at all.
pub fn combine(per_repo: Vec<(String, Vec<(String, Option<i64>)>)>) -> Vec<BranchOption> {
    use std::collections::{BTreeMap, BTreeSet};

    let repos: Vec<&str> = per_repo.iter().map(|(name, _)| name.as_str()).collect();
    let mut best: BTreeMap<String, Option<i64>> = BTreeMap::new();
    let mut present: BTreeMap<String, BTreeSet<&str>> = BTreeMap::new();
    for (repo, list) in &per_repo {
        for (name, at) in list {
            let slot = best.entry(name.clone()).or_default();
            *slot = match (*slot, *at) {
                (Some(x), Some(y)) => Some(x.max(y)),
                (Some(x), None) => Some(x),
                (None, y) => y,
            };
            present.entry(name.clone()).or_default().insert(repo.as_str());
        }
    }

    let missing_for = |name: &str| -> Vec<String> {
        let has = present.get(name);
        repos
            .iter()
            .filter(|repo| !has.is_some_and(|set| set.contains(*repo)))
            .map(|repo| (*repo).to_string())
            .collect()
    };

    let (complete, partial): (Vec<_>, Vec<_>) =
        best.iter().map(|(name, at)| (name.clone(), *at)).partition(|(name, _)| missing_for(name).is_empty());

    let mut out = Vec::new();
    for group in [complete, partial] {
        for name in by_recency(group) {
            let missing_in = missing_for(&name);
            out.push(BranchOption { name, missing_in });
        }
    }
    out
}

/// The local branches, most recently worked on first, without duplicates.
///
/// The order is the whole point of the list: on a project with a couple of
/// dozen branches, the one somebody merges into every day is nowhere in
/// particular alphabetically. `by_recency` says what "recently" means and why
/// it is the reflog that answers it.
///
/// All three sources are read from the **common** directory, which is the same
/// directory in an ordinary clone and a different one in a linked worktree:
/// `refs/heads/`, `packed-refs` and `logs/refs/heads/` are shared by every
/// worktree of a repository and simply do not exist next to a linked
/// worktree's HEAD. Reading them from the per-worktree directory left the
/// dialog offering exactly one branch — the one the worktree is on, which is
/// the one nobody needs to merge into. HEAD stays per-worktree, because that
/// is the branch this checkout is actually on.
///
/// Nothing here is an error, the same as everywhere else in this file: a folder
/// outside git, or one whose refs cannot be read, has no branches to offer and
/// the dialog shows an empty list. The current branch is included even when
/// neither source lists it — a repository with exactly one commitless branch
/// has no ref file for it at all, and offering nothing to merge into would be
/// worse than offering the one branch that exists.
///
/// **A test helper, and compiled only for the tests.** Nothing in production calls it any
/// more — the run dialog reaches these same three sources through `branches_with_recency`
/// and `by_recency`, one repository at a time. It stays because this file's own tests read
/// the sources through it, and the linked-worktree case below is the one they would lose
/// first. The `cfg` is what says so out loud: without it the compiler calls it dead code
/// on every build, and a warning nobody can act on is how a real one goes unread.
#[cfg(test)]
fn branches(project: &Path) -> Vec<String> {
    by_recency(branches_with_recency(project))
}

/// The same three sources as `branches`, with each name's reflog stamp still
/// attached. Split out for `combine`, which has to compare those stamps across
/// repositories before anything can be ordered — `branches` throws them away,
/// and a list already collapsed to names cannot say which of two repositories
/// saw `develop` more recently.
pub fn branches_with_recency(project: &Path) -> Vec<(String, Option<i64>)> {
    let Some(dir) = git_dir(project) else { return Vec::new() };
    let common = common_dir(&dir);
    let mut out = Vec::new();
    loose_branches(&common.join("refs/heads"), "", &mut out);
    if let Ok(packed) = std::fs::read_to_string(common.join("packed-refs")) {
        out.extend(parse_packed_refs(&packed));
    }
    if let Some(current) = head(project).branch {
        out.push(current);
    }
    out.sort();
    out.dedup();
    let logs = common.join("logs/refs/heads");
    out.into_iter().map(|name| { let at = touched_at(&logs, &name); (name, at) }).collect()
}

/// The commit a loose ref names, or nothing.
///
/// A ref file holds an object name and that is all this wants; git also writes
/// symbolic refs (`ref: refs/heads/other`) in a few places, and one of those is
/// not a commit to quote at anybody. Hex and long enough is the check, the same
/// one `parse_head` makes and for the same reason: git is on its way to sha256.
fn loose_tip(heads: &Path, branch: &str) -> Option<String> {
    let text = std::fs::read_to_string(heads.join(branch)).ok()?;
    let line = text.trim();
    let hex = line.len() >= SHORT_HASH && line.chars().all(|c| c.is_ascii_hexdigit());
    hex.then(|| line.to_owned())
}

/// The branch a task's work was left on, out of every local branch and its
/// commit.
///
/// The convention is `provisioning`'s and it is a rule that skill states rather
/// than a habit: `slug = "<id>-<short-kebab-title>"`, `branch =
/// "<fix|feature>/<slug>"`, and the id is in the slug precisely so that a
/// worktree found afterwards can be *proved* to belong to the task looking for
/// it. So the test is the last segment of the name: the task's id, or the id
/// followed by a hyphen. Matching the id anywhere in the name would take
/// `fix/smetana-1ab-see-also-smetana-0t4` for this task's branch, and matching
/// it as a bare prefix would take `smetana-0t4x`'s.
///
/// Several branches for one task means somebody cut a second one, and the first
/// by name is the one named: a note pointing at one of them is worth more than a
/// note pointing at none, and the order is stable, so a run does not say two
/// different things about the same board.
pub fn task_branch(refs: &[(String, String)], id: &str) -> Option<(String, String)> {
    refs.iter()
        .filter(|(name, _)| {
            let slug = name.rsplit('/').next().unwrap_or(name);
            slug == id || slug.strip_prefix(id).is_some_and(|rest| rest.starts_with('-'))
        })
        .map(|(name, sha)| (name.clone(), short(sha)))
        .next()
}

/// How a commit is written into a note a person reads: git's own abbreviation.
fn short(sha: &str) -> String {
    sha.chars().take(SHORT_HASH).collect()
}

/// Where one task's work was left in this repository — its branch and the
/// commit at the tip of it — or nothing, which is the ordinary answer for a
/// task nobody cut a branch for.
///
/// Reading, not running, like everything else in this file: the two sources are
/// `refs/heads/` and `packed-refs`, both in the **common** directory, and a
/// loose ref wins over a packed one because that is what git itself does with a
/// branch that has moved since it was packed. Nothing here is an error — a
/// folder outside git, or refs that cannot be read, simply has no work to name,
/// and the note the caller is writing is worth having without it.
pub fn task_work(project: &Path, id: &str) -> Option<(String, String)> {
    let common = common_dir(&git_dir(project)?);
    let mut refs: BTreeMap<String, String> = BTreeMap::new();
    if let Ok(packed) = std::fs::read_to_string(common.join("packed-refs")) {
        refs.extend(parse_packed_heads(&packed));
    }
    let heads = common.join("refs/heads");
    let mut loose = Vec::new();
    loose_branches(&heads, "", &mut loose);
    for name in loose {
        if let Some(sha) = loose_tip(&heads, &name) {
            refs.insert(name, sha);
        }
    }
    let refs: Vec<(String, String)> = refs.into_iter().collect();
    task_branch(&refs, id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// Where a test's directory would be, without touching the disk — the
    /// absolute-`commondir` case has to name the path before anything is
    /// written into it.
    fn scratch_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("smetana-git-{}-{name}", std::process::id()))
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = scratch_path(name);
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create the temp directory");
        dir
    }

    #[test]
    fn a_branch_reads_without_its_prefix() {
        let head = parse_head("ref: refs/heads/feat/worktree-rename\n");
        assert_eq!(head.branch.as_deref(), Some("feat/worktree-rename"));
        assert_eq!(head.detached, None);
    }

    #[test]
    fn a_ref_outside_refs_heads_stays_as_it_is() {
        // Otherwise a tag named like a branch would look like a branch.
        let head = parse_head("ref: refs/tags/v1.0\n");
        assert_eq!(head.branch.as_deref(), Some("refs/tags/v1.0"));
    }

    #[test]
    fn a_detached_head_is_a_short_hash() {
        let head = parse_head("9a1b2c3d4e5f60718293a4b5c6d7e8f901234567\n");
        assert_eq!(head.branch, None);
        assert_eq!(head.detached.as_deref(), Some("9a1b2c3"));
    }

    #[test]
    fn junk_in_head_simply_means_there_is_nothing_to_show() {
        assert_eq!(parse_head(""), Head::default());
        assert_eq!(parse_head("ref:\n"), Head::default());
        assert_eq!(parse_head("something off\n"), Head::default());
    }

    #[test]
    fn gitdir_leads_into_the_worktrees_directory() {
        let project = Path::new("/Users/you/wt/feature");
        let abs = parse_gitdir("gitdir: /Users/you/repo/.git/worktrees/feature\n", project);
        assert_eq!(abs.as_deref(), Some(Path::new("/Users/you/repo/.git/worktrees/feature")));

        let rel = parse_gitdir("gitdir: ../repo/.git/worktrees/feature\n", project);
        assert_eq!(rel.as_deref(), Some(Path::new("/Users/you/wt/feature/../repo/.git/worktrees/feature")));

        assert_eq!(parse_gitdir("nothing to do with git\n", project), None);
        assert_eq!(parse_gitdir("gitdir:\n", project), None);
    }

    #[test]
    fn a_folder_without_git_is_not_an_error() {
        let root = scratch("no-git");
        assert_eq!(head(&root), Head::default());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn an_ordinary_repository_reads_from_disk() {
        let root = scratch("plain");
        fs::create_dir_all(root.join(".git")).unwrap();
        fs::write(root.join(".git/HEAD"), "ref: refs/heads/main\n").unwrap();
        assert_eq!(head(&root).branch.as_deref(), Some("main"));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_worktree_reads_its_own_head_not_the_main_repositorys() {
        let root = scratch("worktree");
        let repo = root.join("repo");
        let linked = root.join("wt");
        fs::create_dir_all(repo.join(".git/worktrees/wt")).unwrap();
        fs::create_dir_all(&linked).unwrap();
        fs::write(repo.join(".git/HEAD"), "ref: refs/heads/main\n").unwrap();
        fs::write(repo.join(".git/worktrees/wt/HEAD"), "ref: refs/heads/feat/x\n").unwrap();
        fs::write(linked.join(".git"), format!("gitdir: {}\n", repo.join(".git/worktrees/wt").display())).unwrap();

        assert_eq!(head(&linked).branch.as_deref(), Some("feat/x"));
        let _ = fs::remove_dir_all(&root);
    }
    /// A linked worktree on disk, laid out the way git lays one out: the shared
    /// half of the repository under `repo/.git`, and next to the worktree's own
    /// HEAD nothing but `commondir` and a per-worktree `logs/HEAD`. `commondir`
    /// is written relative unless a path is given, since that is what git
    /// writes. Returns the main checkout and the linked one.
    fn linked_worktree(name: &str, commondir: Option<&str>) -> (PathBuf, PathBuf, PathBuf) {
        let root = scratch(name);
        let repo = root.join("repo");
        let linked = root.join("wt");
        let wt_git = repo.join(".git/worktrees/wt");
        fs::create_dir_all(wt_git.join("logs")).expect("create the worktree git directory");
        fs::create_dir_all(&linked).expect("create the worktree");
        fs::write(repo.join(".git/HEAD"), "ref: refs/heads/main\n").expect("write the main HEAD");
        fs::write(wt_git.join("HEAD"), "ref: refs/heads/feat/x\n").expect("write the worktree HEAD");
        // Per-worktree, and deliberately not what branch ordering reads.
        fs::write(wt_git.join("logs/HEAD"), "").expect("write the worktree HEAD log");
        fs::write(wt_git.join("commondir"), format!("{}\n", commondir.unwrap_or("../.."))).expect("write commondir");
        fs::write(linked.join(".git"), format!("gitdir: {}\n", wt_git.display())).expect("write the .git file");
        (root, repo, linked)
    }

    #[test]
    fn commondir_resolves_against_the_worktrees_own_git_directory() {
        let git_dir = Path::new("/Users/you/repo/.git/worktrees/feature");
        let relative = parse_commondir("../..\n", git_dir);
        assert_eq!(relative.as_deref(), Some(Path::new("/Users/you/repo/.git/worktrees/feature/../..")));

        let absolute = parse_commondir("/Users/you/repo/.git\n", git_dir);
        assert_eq!(absolute.as_deref(), Some(Path::new("/Users/you/repo/.git")));

        assert_eq!(parse_commondir("", git_dir), None);
        assert_eq!(parse_commondir("  \n", git_dir), None);
    }

    #[test]
    fn a_repository_without_a_commondir_file_is_its_own_common_directory() {
        // The ordinary clone, which must keep reading exactly where it did.
        let dir = scratch("commondir-absent");
        assert_eq!(common_dir(&dir), dir);
        fs::remove_dir_all(&dir).expect("remove the temp directory");
    }

    #[test]
    fn a_worktree_offers_every_branch_of_the_repository_not_only_its_own() {
        // The bug this pins: `refs/heads/`, `packed-refs` and `logs/refs/heads/`
        // are shared and live in the common directory, so reading them next to
        // the worktree's HEAD found nothing and the dialog offered one branch —
        // the one the worktree is already on, which is the one nobody merges
        // into.
        let (root, repo, linked) = linked_worktree("worktree-branches", None);
        let heads = repo.join(".git/refs/heads");
        fs::create_dir_all(heads.join("feat")).expect("create refs/heads/feat");
        for name in ["main", "staging"] {
            fs::write(heads.join(name), "a1b2c3\n").expect("write the ref");
        }
        fs::write(heads.join("feat/x"), "d4e5f6\n").expect("write the nested ref");
        // Packed and never touched here: the fresh-clone case, in the tail.
        fs::write(repo.join(".git/packed-refs"), "c0ffee refs/heads/zeta\n").expect("write packed");

        let logs = repo.join(".git/logs/refs/heads");
        fs::create_dir_all(logs.join("feat")).expect("create the logs");
        let log = |at: i64| format!("0000000 a1b2c3 flexo <f@e.com> {at} +0300\tcommit: x\n");
        fs::write(logs.join("main"), log(100)).expect("write main's log");
        fs::write(logs.join("staging"), log(300)).expect("write staging's log");
        fs::write(logs.join("feat/x"), log(200)).expect("write the nested log");

        // The same list as in the main checkout, in reflog order rather than the
        // alphabetical tail every branch would fall into with no logs found.
        let expected = vec!["staging", "feat/x", "main", "zeta"];
        assert_eq!(branches(&linked), expected);
        assert_eq!(branches(&repo), expected);
        // And HEAD is still the worktree's own, not the main checkout's `main`.
        assert_eq!(head(&linked).branch.as_deref(), Some("feat/x"));

        fs::remove_dir_all(&root).expect("remove the temp directory");
    }

    #[test]
    fn a_worktree_with_an_absolute_commondir_reads_the_same_place() {
        // git writes an absolute path when the worktree was created with one.
        let common = scratch_path("worktree-abs").join("repo/.git");
        let (root, repo, linked) = linked_worktree("worktree-abs", Some(&common.display().to_string()));
        fs::create_dir_all(repo.join(".git/refs/heads")).expect("create refs/heads");
        fs::write(repo.join(".git/refs/heads/other"), "a1b2c3\n").expect("write the ref");

        assert_eq!(branches(&linked), vec!["feat/x".to_string(), "other".to_string()]);
        fs::remove_dir_all(&root).expect("remove the temp directory");
    }

    #[test]
    fn packed_refs_gives_up_its_branches_and_keeps_its_tags_out() {
        // A fresh clone keeps nearly every branch in here and almost nothing
        // loose on disk, so a reader that skipped this file would offer one
        // branch out of forty.
        let packed = "\
# pack-refs with: peeled fully-peeled sorted
a1b2c3 refs/heads/main
d4e5f6 refs/heads/feature/runs
0f0f0f refs/tags/v1.0.0
^99887766
c0ffee refs/remotes/origin/main
";
        assert_eq!(parse_packed_refs(packed), vec!["main", "feature/runs"]);
    }

    #[test]
    fn an_annotated_tags_peel_line_never_becomes_a_branch() {
        // `^<sha>` has no ref name on it at all; splitting it on a space would
        // produce nothing, and not skipping it explicitly has been a source of
        // junk entries in every hand-written parser of this file.
        assert!(parse_packed_refs("^0123456789abcdef\n").is_empty());
    }

    #[test]
    fn an_empty_or_header_only_file_has_no_branches() {
        assert!(parse_packed_refs("").is_empty());
        assert!(parse_packed_refs("# pack-refs with: peeled\n").is_empty());
    }

    #[test]
    fn a_nested_branch_keeps_its_slash() {
        let dir = scratch("branches");
        let heads = dir.join(".git/refs/heads/feature");
        fs::create_dir_all(&heads).expect("create refs/heads/feature");
        fs::write(heads.join("runs"), "a1b2c3\n").expect("write the ref");
        fs::write(dir.join(".git/refs/heads/main"), "d4e5f6\n").expect("write main");
        fs::write(dir.join(".git/HEAD"), "ref: refs/heads/main\n").expect("write HEAD");

        assert_eq!(branches(&dir), vec!["feature/runs".to_string(), "main".to_string()]);
        fs::remove_dir_all(&dir).expect("remove the temp directory");
    }

    #[test]
    fn the_current_branch_is_offered_even_with_no_ref_file_for_it() {
        // A repository with no commit yet has a HEAD pointing at a branch that
        // exists nowhere on disk. Offering nothing to merge into would be worse
        // than offering the one branch there is.
        let dir = scratch("branches-empty");
        fs::create_dir_all(dir.join(".git/refs/heads")).expect("create refs/heads");
        fs::write(dir.join(".git/HEAD"), "ref: refs/heads/main\n").expect("write HEAD");

        assert_eq!(branches(&dir), vec!["main".to_string()]);
        fs::remove_dir_all(&dir).expect("remove the temp directory");
    }

    #[test]
    fn a_folder_outside_git_simply_has_no_branches() {
        let dir = scratch("branches-nogit");
        assert!(branches(&dir).is_empty());
        fs::remove_dir_all(&dir).expect("remove the temp directory");
    }

    #[test]
    fn a_branch_in_both_places_is_offered_once() {
        let dir = scratch("branches-dup");
        fs::create_dir_all(dir.join(".git/refs/heads")).expect("create refs/heads");
        fs::write(dir.join(".git/refs/heads/main"), "a1b2c3\n").expect("write main");
        fs::write(dir.join(".git/packed-refs"), "a1b2c3 refs/heads/main\n").expect("write packed");
        fs::write(dir.join(".git/HEAD"), "ref: refs/heads/main\n").expect("write HEAD");

        assert_eq!(branches(&dir), vec!["main".to_string()]);
        fs::remove_dir_all(&dir).expect("remove the temp directory");
    }

    #[test]
    fn a_reflog_line_gives_up_its_time_from_the_end_not_the_start() {
        // The identity sits between the shas and the time and holds two spaces
        // of its own, so counting fields from the front would land on the
        // e-mail address.
        let line = "d363fe4 45c4160 Ada Lovelace <ada@example.com> 1785970400 +0300\tmerge staging";
        assert_eq!(parse_reflog_time(line), Some(1785970400));
    }

    #[test]
    fn a_reflog_entry_without_a_message_still_has_a_time() {
        let line = "0000000 a1b2c3 flexo <f@example.com> 1700000000 -0800";
        assert_eq!(parse_reflog_time(line), Some(1700000000));
    }

    #[test]
    fn a_line_that_is_not_a_reflog_entry_is_an_ordinary_none() {
        // Not an error anywhere: a branch whose log cannot be read simply has
        // no local history and goes to the alphabetical tail.
        assert_eq!(parse_reflog_time(""), None);
        assert_eq!(parse_reflog_time("1785970400 +0300"), None);
        assert_eq!(parse_reflog_time("d363fe4 45c4160 flexo <f@e.com> yesterday +0300\tx"), None);
    }

    #[test]
    fn the_branch_worked_on_last_comes_first() {
        let ordered = by_recency(vec![
            ("main".to_string(), Some(100)),
            ("staging".to_string(), Some(300)),
            ("feature/runs".to_string(), Some(200)),
        ]);
        assert_eq!(ordered, vec!["staging", "feature/runs", "main"]);
    }

    #[test]
    fn a_branch_never_touched_here_waits_in_the_alphabetical_tail() {
        // The case a fresh clone makes ordinary: everything is in packed-refs
        // and nothing has a log yet. Those branches are not "very old" — they
        // are outside the recency question, and alphabetical is the answer they
        // had before this ordering existed.
        let ordered = by_recency(vec![
            ("zebra".to_string(), None),
            ("main".to_string(), Some(100)),
            ("apple".to_string(), None),
        ]);
        assert_eq!(ordered, vec!["main", "apple", "zebra"]);
    }

    #[test]
    fn two_branches_touched_in_the_same_second_stay_in_a_settled_order() {
        let ordered = by_recency(vec![
            ("b".to_string(), Some(100)),
            ("a".to_string(), Some(100)),
        ]);
        assert_eq!(ordered, vec!["a", "b"]);
    }

    #[test]
    fn the_list_on_disk_comes_back_in_the_order_it_was_worked_in() {
        let dir = scratch("branches-recency");
        fs::create_dir_all(dir.join(".git/refs/heads")).expect("create refs/heads");
        fs::create_dir_all(dir.join(".git/logs/refs/heads/feature")).expect("create the logs");
        for name in ["alpha", "main", "staging"] {
            fs::write(dir.join(".git/refs/heads").join(name), "a1b2c3\n").expect("write the ref");
        }
        fs::create_dir_all(dir.join(".git/refs/heads/feature")).expect("create refs/heads/feature");
        fs::write(dir.join(".git/refs/heads/feature/runs"), "d4e5f6\n").expect("write the nested ref");
        fs::write(dir.join(".git/HEAD"), "ref: refs/heads/main\n").expect("write HEAD");
        // `zeta` is packed and has no log — the fresh-clone case, in the tail.
        fs::write(dir.join(".git/packed-refs"), "c0ffee refs/heads/zeta\n").expect("write packed");

        let log = |at: i64| format!("0000000 a1b2c3 flexo <f@e.com> {at} +0300\tcommit: x\n");
        fs::write(dir.join(".git/logs/refs/heads/main"), log(100)).expect("write main's log");
        fs::write(dir.join(".git/logs/refs/heads/alpha"), log(300)).expect("write alpha's log");
        fs::write(dir.join(".git/logs/refs/heads/feature/runs"), log(200)).expect("write the nested log");

        assert_eq!(branches(&dir), vec!["alpha", "feature/runs", "main", "staging", "zeta"]);
        fs::remove_dir_all(&dir).expect("remove the temp directory");
    }

    #[test]
    fn a_branch_every_repository_has_comes_before_one_that_is_short() {
        // The two groups are the whole of the visible structure: a branch a run
        // can merge into everywhere is a different kind of answer from one that
        // would have to be cut somewhere first.
        let out = combine(vec![
            ("backend".into(), vec![("develop".into(), Some(20)), ("release/7".into(), Some(30))]),
            ("admin".into(), vec![("develop".into(), Some(10))]),
        ]);
        assert_eq!(
            out,
            vec![
                BranchOption { name: "develop".into(), missing_in: vec![] },
                BranchOption { name: "release/7".into(), missing_in: vec!["admin".into()] },
            ]
        );
    }

    #[test]
    fn freshness_is_the_most_recent_touch_anywhere() {
        // `develop` was opened an hour ago in one repository and a month ago in
        // another. Taking the first repository's answer, or the least of them,
        // buries the branch somebody is actually in behind one they have not
        // touched.
        let out = combine(vec![
            ("backend".into(), vec![("stale".into(), Some(50)), ("develop".into(), Some(10))]),
            ("admin".into(), vec![("stale".into(), Some(20)), ("develop".into(), Some(99))]),
        ]);
        assert_eq!(out.iter().map(|o| o.name.as_str()).collect::<Vec<_>>(), ["develop", "stale"]);
    }

    #[test]
    fn a_branch_with_no_reflog_anywhere_still_sorts_into_the_alphabetical_tail() {
        // `by_recency`'s own rule, and this must go on being its rule rather
        // than a second one written here.
        let out = combine(vec![(
            "backend".into(),
            vec![("zeta".into(), Some(1)), ("beta".into(), None), ("alpha".into(), None)],
        )]);
        assert_eq!(out.iter().map(|o| o.name.as_str()).collect::<Vec<_>>(), ["zeta", "alpha", "beta"]);
    }

    #[test]
    fn the_repositories_a_branch_is_missing_from_are_named_in_the_order_given() {
        // The config's order, which is its statement about what depends on what.
        // A set would print them in whatever order it hashed to, and the field
        // would read differently between two runs of the same project.
        let out = combine(vec![
            ("backend".into(), vec![("release/7".into(), Some(1))]),
            ("frontend".into(), vec![]),
            ("admin".into(), vec![]),
        ]);
        assert_eq!(out[0].missing_in, vec!["frontend".to_string(), "admin".to_string()]);
    }

    #[test]
    fn one_repository_answers_exactly_as_the_single_project_list_always_did() {
        // The no-regression case, and it is the common one: this project's own
        // config is `repos = ["."]`, and so is every project that is one
        // repository. Nothing may be partial when there is nothing to be short
        // of.
        let list = vec![("main".into(), Some(2)), ("develop".into(), Some(9))];
        let out = combine(vec![(".".into(), list.clone())]);
        assert_eq!(out.iter().map(|o| o.name.clone()).collect::<Vec<_>>(), by_recency(list));
        assert!(out.iter().all(|o| o.missing_in.is_empty()), "{out:?}");
    }

    #[test]
    fn no_repositories_at_all_is_an_empty_list_and_not_a_panic() {
        assert_eq!(combine(vec![]), vec![]);
    }

    #[test]
    fn packed_refs_keeps_each_branchs_commit_beside_its_name() {
        // The half `parse_packed_refs` throws away, and the reason the two are
        // one parser: a note saying where a task's work was left needs the sha.
        let packed = "\
# pack-refs with: peeled fully-peeled sorted
a1b2c3d4e5f6 refs/heads/main
d4e5f6a1b2c3 refs/heads/fix/smetana-08f-batch-clean-exit
0f0f0f0f0f0f refs/tags/v1.0.0
^9988776655
";
        assert_eq!(
            parse_packed_heads(packed),
            vec![
                ("main".to_string(), "a1b2c3d4e5f6".to_string()),
                ("fix/smetana-08f-batch-clean-exit".to_string(), "d4e5f6a1b2c3".to_string()),
            ]
        );
    }

    #[test]
    fn a_tasks_branch_is_the_one_whose_slug_is_its_id() {
        let refs = vec![
            ("main".to_string(), "aaaaaaaaaaaa".to_string()),
            ("fix/smetana-0t4-batch-clean-exit".to_string(), "d3a4309abcdef".to_string()),
            ("feature/smetana-2ya-something".to_string(), "e139e79abcdef".to_string()),
        ];
        assert_eq!(
            task_branch(&refs, "smetana-0t4"),
            Some(("fix/smetana-0t4-batch-clean-exit".to_string(), "d3a4309".to_string()))
        );
        assert_eq!(task_branch(&refs, "smetana-xxx"), None);
    }

    #[test]
    fn a_branch_that_merely_mentions_the_id_is_not_that_tasks_branch() {
        // Both ways of matching loosely, and both of them put somebody else's
        // work in a note about this task. The rule is `provisioning`'s slug:
        // the last segment is the id, or the id and a hyphen.
        let refs = vec![
            ("fix/smetana-0t4x-another-task".to_string(), "aaaaaaaaaa".to_string()),
            ("fix/smetana-1ab-follow-up-to-smetana-0t4".to_string(), "bbbbbbbbbb".to_string()),
        ];
        assert_eq!(task_branch(&refs, "smetana-0t4"), None);

        // The bare id on its own is a branch somebody cut by hand, and it does
        // belong to the task.
        let bare = vec![("smetana-0t4".to_string(), "cccccccccc".to_string())];
        assert_eq!(
            task_branch(&bare, "smetana-0t4"),
            Some(("smetana-0t4".to_string(), "ccccccc".to_string()))
        );
    }

    #[test]
    fn a_tasks_work_is_found_loose_or_packed_and_the_loose_ref_wins() {
        let dir = scratch("task-work");
        let heads = dir.join(".git/refs/heads/fix");
        fs::create_dir_all(&heads).expect("create refs/heads/fix");
        fs::write(dir.join(".git/HEAD"), "ref: refs/heads/main\n").expect("write HEAD");
        // Packed says one thing about this branch and the loose ref says
        // another, which is git's own ordinary state after a branch moves.
        fs::write(
            dir.join(".git/packed-refs"),
            "0000000000 refs/heads/fix/smetana-08f-a-task\n\
             1111111111 refs/heads/feature/smetana-2ya-another\n",
        )
        .expect("write packed-refs");
        fs::write(heads.join("smetana-08f-a-task"), "d3a4309abcdef\n").expect("write the ref");

        assert_eq!(
            task_work(&dir, "smetana-08f"),
            Some(("fix/smetana-08f-a-task".to_string(), "d3a4309".to_string()))
        );
        assert_eq!(
            task_work(&dir, "smetana-2ya"),
            Some(("feature/smetana-2ya-another".to_string(), "1111111".to_string())),
            "a branch that exists only in packed-refs still answers"
        );
        assert_eq!(task_work(&dir, "smetana-000"), None);
        fs::remove_dir_all(&dir).expect("remove the temp directory");
    }

    #[test]
    fn a_symbolic_ref_is_not_a_commit_to_quote_at_anybody() {
        let dir = scratch("task-work-symref");
        let heads = dir.join(".git/refs/heads");
        fs::create_dir_all(heads.join("fix")).expect("create refs/heads/fix");
        fs::write(heads.join("fix/smetana-08f-a-task"), "ref: refs/heads/main\n")
            .expect("write the ref");
        assert_eq!(task_work(&dir, "smetana-08f"), None);

        // And a folder outside git has no work to name, which is not an error
        // either — the note is worth having without it.
        assert_eq!(task_work(Path::new("/nowhere/at/all"), "smetana-08f"), None);
        fs::remove_dir_all(&dir).expect("remove the temp directory");
    }
}
