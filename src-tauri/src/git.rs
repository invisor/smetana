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
fn git_dir(project: &Path) -> Option<PathBuf> {
    let dot_git = project.join(".git");
    if dot_git.is_dir() {
        return Some(dot_git);
    }
    parse_gitdir(&std::fs::read_to_string(&dot_git).ok()?, project)
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
    contents
        .lines()
        .filter(|line| !line.starts_with('#') && !line.starts_with('^'))
        .filter_map(|line| line.split_once(' ').map(|(_, name)| name.trim()))
        .filter_map(|name| name.strip_prefix(HEADS))
        .map(str::to_owned)
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

/// The local branches, most recently worked on first, without duplicates.
///
/// The order is the whole point of the list: on a project with a couple of
/// dozen branches, the one somebody merges into every day is nowhere in
/// particular alphabetically. `by_recency` says what "recently" means and why
/// it is the reflog that answers it. `git_dir` already resolves a linked
/// worktree to the directory holding its logs, so this works from one without
/// anything extra.
///
/// Nothing here is an error, the same as everywhere else in this file: a folder
/// outside git, or one whose refs cannot be read, has no branches to offer and
/// the dialog shows an empty list. The current branch is included even when
/// neither source lists it — a repository with exactly one commitless branch
/// has no ref file for it at all, and offering nothing to merge into would be
/// worse than offering the one branch that exists.
pub fn branches(project: &Path) -> Vec<String> {
    let Some(dir) = git_dir(project) else { return Vec::new() };
    let mut out = Vec::new();
    loose_branches(&dir.join("refs/heads"), "", &mut out);
    if let Ok(packed) = std::fs::read_to_string(dir.join("packed-refs")) {
        out.extend(parse_packed_refs(&packed));
    }
    if let Some(current) = head(project).branch {
        out.push(current);
    }
    out.sort();
    out.dedup();
    let logs = dir.join("logs/refs/heads");
    let touched = out
        .into_iter()
        .map(|name| {
            let at = touched_at(&logs, &name);
            (name, at)
        })
        .collect();
    by_recency(touched)
}

#[tauri::command]
pub fn git_branches(project: String) -> Vec<String> {
    branches(Path::new(&project))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("smetana-git-{}-{name}", std::process::id()));
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
}
