//! The vocabulary this module hands to the front end, and the pure parse of
//! `git status --porcelain=v2 -z --branch`.
//!
//! Nothing here touches the disk or the process table — no `std::fs`, no
//! `std::process` — and that is the point rather than a coincidence: the parse
//! is a function over a `&str`, so the tests at the bottom are the whole of
//! what says it reads git's output correctly. `run.rs` hands it the bytes and
//! knows nothing about what is in them.
//!
//! The machine-readable form and never the human one: `git status`'s prose
//! moves between versions, while `--porcelain=v2` is documented and stable.

use serde::Serialize;

/// What a changed file is. `staged` and `unstaged` are the two halves of the
/// porcelain's `XY` — the index against `HEAD` and the working tree against the
/// index — kept apart rather than folded into one flag, because a file an agent
/// has already staged and a file it has only written are different things to
/// somebody reading the list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Change {
    pub path: String,
    /// Where a rename came from. `None` for everything else.
    pub orig_path: Option<String>,
    pub kind: ChangeKind,
    pub staged: bool,
    pub unstaged: bool,
}

/// The kinds git's own status letters name. A letter this list has never heard
/// of is not an error — see `classify`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ChangeKind {
    Modified,
    Added,
    Deleted,
    Renamed,
    Copied,
    TypeChanged,
    Untracked,
    Conflicted,
}

/// One repository's working tree as the panel draws it.
///
/// `branch` and `detached` are kept apart for the reason `git::Head` keeps them
/// apart: a short hash drawn where a branch name goes has to say so, and a
/// component cannot tell the two apart once they share a field.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkingTree {
    pub branch: Option<String>,
    pub detached: Option<String>,
    pub changes: Vec<Change>,
}

/// One repository of a project, as `repos.rs` finds it.
///
/// The branch is `git::head`'s answer and therefore a file read: the whole list
/// costs one read per repository and not one process per repository.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Repo {
    /// As written in `[project].repos`, or as the directory is called. `.` is
    /// the project root itself.
    pub name: String,
    /// Absolute, because every command in this module takes one.
    pub path: String,
    pub branch: Option<String>,
    pub detached: Option<String>,
}

/// One local branch of a repository, as the panel lists it.
///
/// Two fields and no more: an ahead/behind count, an upstream and a remote
/// branch are all outside this epic, and a field carried across the wire with
/// nothing drawing it is a field nobody notices going wrong.
///
/// `current` rather than the front end comparing the name against the row's
/// branch: HEAD is read here, beside the list itself, so the two cannot come
/// from different moments.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Branch {
    pub name: String,
    pub current: bool,
}

/// How many characters of a detached HEAD's hash to show — git's own default
/// for an abbreviated object name, and the same number `git.rs` uses.
const SHORT_HASH: usize = 7;

#[derive(Debug, thiserror::Error)]
pub enum VcsError {
    /// There is no `git` on this machine, or none this process can reach. Its
    /// own error rather than an empty answer: the rule `runs/browser.rs` sets
    /// for the whole repository is that anything unobservable reads as "no",
    /// loudly.
    ///
    /// A whole sentence, because the panel draws it as it stands, and it names
    /// what was looked for the way `TerminalError::NoAgent` does: the program
    /// name belongs to this side, and a second copy of it on the front end is
    /// the kind that drifts.
    #[error("Smetana looked for {0} on your PATH and found nothing.")]
    NoGit(String),
    /// git refused, and its own words are what the person reads. They know git;
    /// a message rewritten here would be a worse version of one they can
    /// already act on.
    #[error("{stderr}")]
    Git { status: i32, stderr: String },
    /// The three refusals a file at `HEAD` shares with the editor, and they
    /// carry `FilesError`'s own `kind` strings deliberately: the same file
    /// opened in a tab and opened as a diff has to be refused for the same
    /// reason in the same words, and the front end already has one table
    /// keyed by those strings (`fileErrorText` in `stores/files.js`). A second
    /// vocabulary for the same three facts is how the two halves start
    /// disagreeing. A test in this file pins them to each other.
    #[error("binary file: {0}")]
    Binary(String),
    #[error("file too large: {path} ({bytes} bytes)")]
    TooLarge { path: String, bytes: u64 },
    #[error("not UTF-8 text: {0}")]
    NotUtf8(String),
    #[error("{0}")]
    Io(String),
}

impl VcsError {
    /// The machine-readable half, the same shape `FilesError` uses: the panel
    /// decides what to draw from this rather than by reading the message.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::NoGit(_) => "noGit",
            Self::Git { .. } => "git",
            Self::Binary(_) => "binary",
            Self::TooLarge { .. } => "tooLarge",
            Self::NotUtf8(_) => "notUtf8",
            Self::Io(_) => "io",
        }
    }
}

// Tauri wants a command's error serializable, and one string will not do: the
// panel says something different for "no git on this machine" than for a
// repository git refused to read.
impl Serialize for VcsError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut st = s.serialize_struct("VcsError", 2)?;
        st.serialize_field("kind", self.kind())?;
        st.serialize_field("message", &self.to_string())?;
        st.end()
    }
}

/// The tree as `git status --porcelain=v2 -z --branch` describes it.
///
/// Pure, and deliberately tolerant: an unrecognised record is skipped rather
/// than refused. This runs on whatever the git on somebody's machine prints,
/// and losing one row beats losing the panel.
pub fn parse_status(out: &str) -> WorkingTree {
    let mut tree = WorkingTree::default();
    // `-z` terminates each record; the trailing empty piece is not a record.
    let mut records = out.split('\0').filter(|r| !r.is_empty());
    while let Some(record) = records.next() {
        match record.split_once(' ') {
            Some(("#", rest)) => header(&mut tree, rest),
            // A rename's original path is the record after it, not a change.
            Some(("2", rest)) => {
                let orig = records.next().map(str::to_owned);
                if let Some(mut change) = ordinary(rest, 9) {
                    change.orig_path = orig;
                    tree.changes.push(change);
                }
            }
            Some(("1", rest)) => tree.changes.extend(ordinary(rest, 8)),
            Some(("u", rest)) => tree.changes.extend(unmerged(rest)),
            Some(("?", path)) => tree.changes.push(Change::untracked(path)),
            _ => {}
        }
    }
    tree
}

/// A `# <key> <value>` header line.
///
/// The two that matter arrive in git's own order — the oid before the head —
/// and this deliberately does not depend on that: the oid is kept as a
/// provisional detached hash only while no branch has been seen, and a named
/// branch clears it. Reading them positionally would leave a repository whose
/// header git reorders showing a hash beside a branch name.
fn header(tree: &mut WorkingTree, rest: &str) {
    let Some((key, value)) = rest.split_once(' ') else { return };
    match key {
        // `(initial)` is a repository with no commit yet: there is no hash to
        // abbreviate and nothing detached about it.
        "branch.oid" if tree.branch.is_none() && value != "(initial)" => {
            tree.detached = value.get(..SHORT_HASH).map(str::to_owned);
        }
        "branch.head" if value == "(detached)" => {}
        "branch.head" => {
            tree.branch = Some(value.to_owned());
            tree.detached = None;
        }
        _ => {}
    }
}

/// A `1` or a `2` record: `fields` space-separated fields, of which the last is
/// the path.
///
/// The path is whatever is left after the fixed ones, taken whole — never
/// `split(' ').last()`, which would cut `my notes.txt` in half. A record with
/// fewer fields than it should have is skipped rather than read at an offset.
fn ordinary(rest: &str, fields: usize) -> Option<Change> {
    // `splitn` leaves everything after the last separator it needed in the
    // final piece, which is exactly the path — spaces and all.
    let parts: Vec<&str> = rest.splitn(fields, ' ').collect();
    if parts.len() != fields {
        return None;
    }
    let path = parts[fields - 1];
    if path.is_empty() {
        return None;
    }
    let (kind, staged, unstaged) = classify(parts[0]);
    Some(Change { path: path.to_owned(), orig_path: None, kind, staged, unstaged })
}

/// A `u` record: a file with a conflict in it, and ten fields rather than
/// eight — three stages of modes and three object names.
///
/// The `XY` of an unmerged entry names which side did what (`UU`, `AA`, `DU`
/// …), and none of that changes what the panel draws or what a person does
/// about it, so the whole family is one kind.
fn unmerged(rest: &str) -> Option<Change> {
    let parts: Vec<&str> = rest.splitn(10, ' ').collect();
    if parts.len() != 10 {
        return None;
    }
    let path = parts[9];
    if path.is_empty() {
        return None;
    }
    // Staged is deliberately false: a conflict is not work somebody has put in
    // the index, it is work waiting in the tree in front of them.
    Some(Change {
        path: path.to_owned(),
        orig_path: None,
        kind: ChangeKind::Conflicted,
        staged: false,
        unstaged: true,
    })
}

impl Change {
    /// A `?` record, which carries nothing but its path.
    fn untracked(path: &str) -> Self {
        Self {
            path: path.to_owned(),
            orig_path: None,
            kind: ChangeKind::Untracked,
            staged: false,
            unstaged: true,
        }
    }
}

/// `X` is the index against HEAD, `Y` the working tree against the index; `.`
/// is "unchanged on that side". The kind is the more specific of the two, with
/// the working tree winning when both moved — that is the change a person is
/// looking at.
///
/// A letter neither side recognises reads as a modification rather than
/// refusing the record: the file did change, and the one thing this cannot
/// honestly do is leave it off the list.
fn classify(xy: &str) -> (ChangeKind, bool, bool) {
    let mut letters = xy.chars();
    let index = letters.next().unwrap_or('.');
    let worktree = letters.next().unwrap_or('.');
    let staged = index != '.';
    let unstaged = worktree != '.';
    let letter = if unstaged { worktree } else { index };
    (kind_of(letter), staged, unstaged)
}

fn kind_of(letter: char) -> ChangeKind {
    match letter {
        'A' => ChangeKind::Added,
        'D' => ChangeKind::Deleted,
        'R' => ChangeKind::Renamed,
        'C' => ChangeKind::Copied,
        'T' => ChangeKind::TypeChanged,
        'U' => ChangeKind::Conflicted,
        '?' => ChangeKind::Untracked,
        _ => ChangeKind::Modified,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The real thing is NUL-terminated; writing `\0` into every fixture by
    /// hand reads worse than joining records here.
    fn porcelain(records: &[&str]) -> String {
        records.iter().map(|r| format!("{r}\0")).collect()
    }

    #[test]
    fn reads_the_branch_off_the_header() {
        let tree = parse_status(&porcelain(&["# branch.oid abc123", "# branch.head develop"]));
        assert_eq!(tree.branch.as_deref(), Some("develop"));
        assert_eq!(tree.detached, None);
    }

    #[test]
    fn a_detached_head_is_not_dressed_up_as_a_branch() {
        let tree = parse_status(&porcelain(&["# branch.oid abc123def456", "# branch.head (detached)"]));
        assert_eq!(tree.branch, None);
        assert_eq!(tree.detached.as_deref(), Some("abc123d"));
    }

    #[test]
    fn an_ordinary_change_carries_which_side_it_is_on() {
        let tree = parse_status(&porcelain(&["1 .M N... 100644 100644 100644 aaa bbb src/main.rs"]));
        assert_eq!(tree.changes.len(), 1);
        let change = &tree.changes[0];
        assert_eq!(change.path, "src/main.rs");
        assert_eq!(change.kind, ChangeKind::Modified);
        assert!(!change.staged, "the index matches HEAD");
        assert!(change.unstaged, "the working tree does not match the index");
    }

    #[test]
    fn a_file_staged_by_an_agent_is_reported_as_staged() {
        let tree = parse_status(&porcelain(&["1 A. N... 000000 100644 100644 000 bbb new.rs"]));
        assert!(tree.changes[0].staged);
        assert!(!tree.changes[0].unstaged);
        assert_eq!(tree.changes[0].kind, ChangeKind::Added);
    }

    /// The one record that is two NUL-separated fields rather than one. Reading
    /// it as a single field puts the original path into the next record's slot
    /// and every change after a rename is nonsense.
    #[test]
    fn a_rename_consumes_the_record_after_it_as_its_original_path() {
        let tree = parse_status(&porcelain(&[
            "2 R. N... 100644 100644 100644 aaa bbb R100 new/name.rs",
            "old/name.rs",
            "1 .M N... 100644 100644 100644 ccc ddd after.rs",
        ]));
        assert_eq!(tree.changes.len(), 2, "the original path is not a change of its own");
        assert_eq!(tree.changes[0].kind, ChangeKind::Renamed);
        assert_eq!(tree.changes[0].path, "new/name.rs");
        assert_eq!(tree.changes[0].orig_path.as_deref(), Some("old/name.rs"));
        assert_eq!(tree.changes[1].path, "after.rs");
    }

    #[test]
    fn an_unmerged_record_is_a_conflict() {
        let tree = parse_status(&porcelain(&[
            "u UU N... 100644 100644 100644 100644 aaa bbb ccc src/both.rs",
        ]));
        assert_eq!(tree.changes[0].kind, ChangeKind::Conflicted);
        assert_eq!(tree.changes[0].path, "src/both.rs");
    }

    #[test]
    fn untracked_files_are_changes_too() {
        let tree = parse_status(&porcelain(&["? notes.txt"]));
        assert_eq!(tree.changes[0].kind, ChangeKind::Untracked);
        assert!(tree.changes[0].unstaged);
    }

    /// `-z` exists for this: a path may hold a space, and it may hold a
    /// newline. Neither ends a record.
    #[test]
    fn a_path_may_hold_a_space_or_a_newline() {
        let tree = parse_status(&porcelain(&[
            "1 .M N... 100644 100644 100644 aaa bbb my notes.txt",
            "? odd\nname.txt",
        ]));
        assert_eq!(tree.changes[0].path, "my notes.txt");
        assert_eq!(tree.changes[1].path, "odd\nname.txt");
    }

    #[test]
    fn a_clean_tree_is_an_empty_list_and_not_a_failure() {
        let tree = parse_status(&porcelain(&["# branch.oid abc", "# branch.head main"]));
        assert!(tree.changes.is_empty());
        assert_eq!(tree.branch.as_deref(), Some("main"));
    }

    /// The diff and the editor open the same files and refuse them for the same
    /// three reasons. The front end reads the `kind` and nothing else, so the
    /// day one of these strings moves, the diff would start refusing in the
    /// generic "could not read this file" while the tab beside it still names
    /// the reason — silent, and a test is the only place that could notice.
    #[test]
    fn the_refusals_shared_with_the_editor_carry_the_editor_s_own_kinds() {
        use crate::files::model::FilesError;

        let path = || "src/main.rs".to_string();
        assert_eq!(VcsError::Binary(path()).kind(), FilesError::Binary(path()).kind());
        assert_eq!(
            VcsError::TooLarge { path: path(), bytes: 1 }.kind(),
            FilesError::TooLarge { path: path(), bytes: 1 }.kind()
        );
        assert_eq!(VcsError::NotUtf8(path()).kind(), FilesError::NotUtf8(path()).kind());
    }

    /// An unknown record type is git's business, not ours: skipping it loses
    /// one row, while panicking loses the panel.
    #[test]
    fn an_unrecognised_record_is_passed_over() {
        let tree = parse_status(&porcelain(&["! ignored.txt", "x whatever", "? real.txt"]));
        assert_eq!(tree.changes.len(), 1);
        assert_eq!(tree.changes[0].path, "real.txt");
    }
}
