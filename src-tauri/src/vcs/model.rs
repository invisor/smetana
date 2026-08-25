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

/// What a project is made of, and what it is made of that it does not say so.
///
/// One answer rather than two commands, because the two halves are read from
/// the same directory listing in the same breath: asking for them separately
/// would let a clone made between the two calls be in one answer and not the
/// other, and the panel draws them as one block.
///
/// `unlisted` is names and not `Repo`s, and that is the whole difference
/// between the two fields: a repository in the list is one this app will act
/// on — its branch is read, it can be selected, git is run in it — where an
/// unlisted one is a folder the panel can only point at. Reading a branch for
/// it would be answering a question nobody may ask.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectRepos {
    pub repos: Vec<Repo>,
    /// Directories one level below the project root that git can see and
    /// `[project].repos` does not name, in the listing's own order. Always
    /// empty for a project with no configuration, by construction: everything
    /// found there is already in `repos`.
    pub unlisted: Vec<String>,
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

/// One local branch's standing against its upstream.
///
/// Apart from `Branch` rather than folded into it, and the seam is the same one
/// `vcs_branches` and `vcs_tracking` are split on: a branch list is three file
/// reads that cannot fail, and this is a process that can. Two answers merged
/// by name on the front end is what keeps the first of those properties true.
///
/// `gone` is its own field rather than an absent `upstream`: a branch nobody
/// has pushed and a branch whose upstream was deleted on the remote are
/// opposite facts — the first has never had one, the second has lost one — and
/// the panel refuses a pull for different reasons in different words.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Tracking {
    pub branch: String,
    pub upstream: Option<String>,
    pub ahead: u32,
    pub behind: u32,
    pub gone: bool,
}

/// What `git for-each-ref` printed, one record per line, three fields split by
/// `%00`.
///
/// A newline is a safe record separator here where it would not be for a path:
/// `git check-ref-format` forbids control characters in a ref name, so a branch
/// cannot contain one. The `%00` between the fields is what keeps an empty
/// upstream distinguishable from a field that is not there.
///
/// Tolerant in the same direction `parse_status` is tolerant: a line it cannot
/// read is skipped, and a `track` string it does not recognise counts as zero
/// rather than as a failure. This runs against whatever git is on somebody's
/// machine, and a row with no mark beats a panel with no rows.
pub fn parse_tracking(out: &str) -> Vec<Tracking> {
    out.lines().filter_map(parse_tracking_line).collect()
}

fn parse_tracking_line(line: &str) -> Option<Tracking> {
    let mut fields = line.split('\0');
    let branch = fields.next()?.trim();
    if branch.is_empty() {
        return None;
    }
    let upstream = fields.next().unwrap_or("").trim();
    let track = fields.next().unwrap_or("").trim();
    let gone = track == "gone";
    let (ahead, behind) = if gone { (0, 0) } else { counts(track) };
    Some(Tracking {
        branch: branch.to_string(),
        upstream: (!upstream.is_empty()).then(|| upstream.to_string()),
        ahead,
        behind,
        gone,
    })
}

/// `ahead 1, behind 2`, either half alone, or nothing at all.
fn counts(track: &str) -> (u32, u32) {
    let mut ahead = 0;
    let mut behind = 0;
    for part in track.split(',') {
        let part = part.trim();
        if let Some(n) = part.strip_prefix("ahead ") {
            ahead = n.trim().parse().unwrap_or(0);
        } else if let Some(n) = part.strip_prefix("behind ") {
            behind = n.trim().parse().unwrap_or(0);
        }
    }
    (ahead, behind)
}

/// Which of the two operations put a repository where it is, and therefore
/// which one an abort has to undo.
///
/// A typed word rather than a free string, because it crosses the IPC twice in
/// opposite directions: the panel sends it back to `vcs_abort`, and it rides on
/// `agents::Intent::ResolveConflict` into a prompt. A string would reach git as
/// `git <whatever the front end wrote> --abort` and be refused by the one
/// command whose whole job is to put a tree back.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum OpKind {
    Merge,
    Rebase,
}

impl OpKind {
    /// git's own subcommand for it, which is also the word a prompt calls it
    /// by. One copy, so `git merge --abort` and the sentence explaining what
    /// was not to be done cannot come to name different operations.
    pub fn word(self) -> &'static str {
        match self {
            Self::Merge => "merge",
            Self::Rebase => "rebase",
        }
    }
}

/// What a merge or a rebase came to.
///
/// A conflict is an **outcome and not a failure**: nothing was lost, nothing
/// was committed, and the tree is exactly what git left behind — which is the
/// state the panel then offers two doors out of. So it is not a `VcsError`, and
/// the front end branches on `kind` rather than reading a message.
///
/// Which of the two it is, is decided by the **tree** and never by what git
/// said: `git merge`'s prose moves between versions where unmerged records do
/// not, the same reason `parse_status` reads `--porcelain=v2` and never
/// `git status`. `conflicted` below is that reading.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum MergeOutcome {
    /// git finished the operation. There may be a new commit or there may not
    /// — a fast-forward makes none — and the panel draws the tree either way.
    Clean,
    /// git stopped, leaving these paths unmerged. Repository-relative, exactly
    /// as `vcs_status` reports every other path.
    Conflict { files: Vec<String> },
}

/// The paths git left unmerged, and nothing else in the tree.
///
/// Pure, and the whole of how a conflict is told apart from a refusal: a merge
/// that fails because the tree was dirty exits non-zero with nothing unmerged,
/// and a merge that conflicts exits non-zero with `u` records in the status. The
/// modified file sitting beside a conflicted one is not part of the answer — it
/// is not what the agent is being sent to resolve, and naming it in the prompt
/// would send one after work nobody asked about.
pub fn conflicted(tree: &WorkingTree) -> Vec<String> {
    tree.changes
        .iter()
        .filter(|change| change.kind == ChangeKind::Conflicted)
        .map(|change| change.path.clone())
        .collect()
}

/// The unmerged paths **this** operation is answerable for: the tree after it,
/// but only where the tree before it was clean of unmerged records.
///
/// The reading of a conflict is off the tree and never off git's message, and
/// this is the other half of that rule. git **refuses to start** a merge or a
/// rebase in a tree that already has unmerged entries — "Merging is not
/// possible because you have unmerged files", exit 128 — and does nothing at
/// all, leaving those same records in the porcelain. Asked only "are there
/// unmerged records now", the app reads that refusal as a fresh conflict of its
/// own: a dialog naming an operation git never began, whose Abort runs
/// `git merge --abort` against whatever *is* in progress. That is not exotic —
/// leaving a tree conflicted is this app's own designed exit from the dialog,
/// and a repository left mid-merge by an agent is the ordinary case the panel's
/// staleness already admits. The cost of getting it wrong that way is somebody
/// else's staged resolutions thrown away under a button captioned "puts the
/// repository back where it was".
///
/// So an already-unmerged tree attributes nothing, and the caller returns git's
/// own refusal instead — which is what a person can act on, since git's message
/// says exactly what is in the way.
///
/// `None` is a tree that could not be read *before*, and it attributes nothing
/// either: not knowing what was there is not evidence that nothing was. The
/// direction is deliberate and it is still the cheap side, but **what it costs
/// is worse than it looks and is written down here measured rather than
/// assumed**, because a cost recorded lower than the real one is what invites
/// somebody to invert this arm later.
///
/// What the caller returns in that arm is `VcsError::Git`, and `refusal()`
/// carries **stderr only** — while a merge conflict writes nothing to stderr at
/// all. Measured: `git merge --no-edit other` on a real conflict leaves stderr
/// empty and puts "CONFLICT (content): Merge conflict in f.txt / Automatic
/// merge failed" on stdout. So a merge conflict lost to this arm reaches
/// `GitPanel` as "Git did not merge" over an **empty** message block. A rebase
/// is the better half of the same case — `error: could not apply …` does go to
/// stderr — so there the words survive.
///
/// And the conflicted files are not drawn either, in either operation:
/// `write()` in `src/stores/vcs.js` sets `writeError` in its catch and returns,
/// where `refresh()` and `loadHead()` are on the success path — so the tree
/// stays as the panel last read it until the next window focus or a press of
/// the refresh button. Against that: claiming a conflict wrongly offers a
/// person an Abort that destroys work somebody else staged. Still the cheap
/// side, and no longer a cheap-sounding one.
///
/// **This is a comparison of two moments and not a lock, and the residual is
/// named rather than left to be discovered.** An agent that *starts* a
/// conflicting merge in the same tree in between — after the pre-read, before
/// the spawn — leaves `before` clean and `after` unmerged, and its conflict is
/// attributed to us exactly as the one-read version attributed every one. The
/// window is the tens of milliseconds between two `git status` calls, against
/// the 100% that version hit on the same failure, and no arithmetic over these
/// two lists can close it: only asking git what is *in progress* would —
/// a `MERGE_HEAD` / `rebase-merge` probe — which is a file read in a module
/// whose header forbids one, a different mechanism in a different file, and
/// deliberately not taken. So do not read this rule as airtight; read it as a
/// hundredfold narrowing of a failure that used to be certain.
pub fn new_conflicts(before: Option<&WorkingTree>, after: &WorkingTree) -> Vec<String> {
    match before {
        Some(before) if conflicted(before).is_empty() => conflicted(after),
        _ => Vec::new(),
    }
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
    /// A commit was asked for with nothing to call it. git refuses this too, and
    /// in good words — but only after the tree has been staged, which is why
    /// `vcs_commit` answers it first and this variant exists to answer it with.
    #[error("A commit needs a message.")]
    NoMessage,
    /// A call that was still running when its ceiling passed, and was stopped.
    /// Its own variant rather than a `Git { .. }` with an empty stderr: git
    /// said nothing, this app decided, and the sentence has to say so.
    ///
    /// **One variant for all three ceilings, and the sentence says only what is
    /// true of all three.** It read "the remote did not answer" while the only
    /// calls that could produce it were the networked ones; local reads and
    /// writes have ceilings of their own now (`run::READ_CEILING`,
    /// `run::WRITE_CEILING`), and that clause would be a lie about a commit
    /// hook. A second variant was the alternative and buys nothing: both would
    /// carry this same `kind`, since what the panel does about them is
    /// identical, so nothing on the front end could have told them apart — and
    /// the one thing the old clause added is already on screen, where the
    /// panel's own heading names the operation that was refused ("Git did not
    /// reach the remote", "Git did not commit").
    #[error("Smetana stopped git after {0} seconds — it had not finished.")]
    Timeout(u64),
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
            Self::NoMessage => "noMessage",
            Self::Timeout(_) => "timeout",
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

    /// The panel branches on `kind` and never on the message, so a variant whose
    /// kind was not added is one the front end cannot tell from an ordinary
    /// refusal — and this one must not be attributed to git at all: nothing git
    /// said produced it.
    ///
    /// The sentence is checked for what it does **not** say as well. One
    /// variant now answers for three ceilings, two of them local, so a clause
    /// about a remote would be a lie under every commit that outstayed a hook.
    #[test]
    fn a_timeout_is_its_own_kind_and_names_this_app_rather_than_git() {
        let err = VcsError::Timeout(60);

        assert_eq!(err.kind(), "timeout");
        assert!(err.to_string().contains("60"), "the message says how long was waited");
        assert!(err.to_string().starts_with("Smetana"), "the sentence is this app's, not git's");
        assert!(
            !err.to_string().contains("remote"),
            "a local read and a local write reach this variant too"
        );
    }

    /// The rule the whole merge door rests on. `git merge`'s own wording moves
    /// between versions — "Automatic merge failed; fix conflicts" is not a
    /// promise anybody made — while an unmerged record in the porcelain is
    /// documented and stable. So the exit code says only "something happened"
    /// and the tree says what.
    #[test]
    fn a_conflict_is_read_off_the_tree_rather_than_off_the_message() {
        let tree = parse_status(&porcelain(&[
            "u UU N... 100644 100644 100644 100644 a b c src/one.rs",
            "1 .M N... 100644 100644 100644 d e src/two.rs",
        ]));
        assert_eq!(conflicted(&tree), ["src/one.rs"], "only the unmerged records");
    }

    /// The other half of that rule, and the one that decides whether a non-zero
    /// exit reaches the person as git's own refusal: a tree with nothing
    /// unmerged in it is not a conflict, however loudly git complained.
    #[test]
    fn a_tree_with_nothing_unmerged_names_no_conflict_at_all() {
        let tree = parse_status(&porcelain(&[
            "# branch.head main",
            "1 .M N... 100644 100644 100644 d e src/two.rs",
            "? notes.txt",
        ]));
        assert!(conflicted(&tree).is_empty());
    }

    /// The other half of that same rule, and the one a real repository forced.
    ///
    /// git refuses to *start* a merge or a rebase in a tree that already has
    /// unmerged entries — measured: `git merge --no-edit third` against a tree
    /// left conflicted by an earlier merge prints "Merging is not possible
    /// because you have unmerged files", exits 128, and changes nothing, so the
    /// earlier merge's own `u` records are still sitting in the porcelain
    /// afterwards. Read as "are there unmerged records now", that refusal
    /// becomes a conflict of this operation's own: a dialog naming a merge git
    /// never began, whose Abort reaches whatever really is in progress and
    /// throws away resolutions somebody had already staged.
    #[test]
    fn a_tree_already_unmerged_before_the_operation_earns_no_conflict_of_its_own() {
        let unmerged = parse_status(&porcelain(&[
            "u UU N... 100644 100644 100644 100644 a b c src/one.rs",
        ]));
        assert!(
            new_conflicts(Some(&unmerged), &unmerged).is_empty(),
            "git refused and did nothing; the records are the previous operation's"
        );
    }

    #[test]
    fn a_conflict_this_operation_made_is_the_one_it_reports() {
        // The ordinary case, and the one the rule must not cost: a tree with
        // ordinary changes in it, and unmerged records only afterwards.
        let before = parse_status(&porcelain(&[
            "# branch.head main",
            "1 .M N... 100644 100644 100644 d e src/two.rs",
        ]));
        let after = parse_status(&porcelain(&[
            "u UU N... 100644 100644 100644 100644 a b c src/one.rs",
            "1 .M N... 100644 100644 100644 d e src/two.rs",
        ]));
        assert_eq!(new_conflicts(Some(&before), &after), ["src/one.rs"]);
    }

    #[test]
    fn a_tree_that_could_not_be_read_first_attributes_nothing() {
        // Not knowing what was there is not evidence that nothing was, and the
        // caller answers with git's own refusal instead. Losing a dialog costs
        // a conflicted tree drawn in the changes list; claiming one wrongly
        // offers to destroy work.
        let after = parse_status(&porcelain(&[
            "u UU N... 100644 100644 100644 100644 a b c src/one.rs",
        ]));
        assert!(new_conflicts(None, &after).is_empty());
    }

    /// The panel branches on `kind` and reads `files`; both spellings are
    /// written out again in `src/stores/vcs.js`, which is the only other place
    /// they exist.
    #[test]
    fn an_outcome_reaches_the_front_end_tagged_by_kind() {
        assert_eq!(
            serde_json::to_string(&MergeOutcome::Clean).expect("serializes"),
            r#"{"kind":"clean"}"#
        );
        assert_eq!(
            serde_json::to_string(&MergeOutcome::Conflict { files: vec!["src/one.rs".into()] })
                .expect("serializes"),
            r#"{"kind":"conflict","files":["src/one.rs"]}"#
        );
    }

    /// The word the panel sends back for an abort, and the word git is given.
    /// They are the same string by construction here, which is what stops
    /// `vcs_abort` from ever running `git <something else> --abort`.
    #[test]
    fn an_operation_crosses_the_wire_as_the_word_git_knows_it_by() {
        for (json, op, word) in [
            (r#""merge""#, OpKind::Merge, "merge"),
            (r#""rebase""#, OpKind::Rebase, "rebase"),
        ] {
            let parsed: OpKind = serde_json::from_str(json).expect("deserializes");
            assert_eq!(parsed, op, "{json}");
            assert_eq!(op.word(), word);
        }
        assert!(serde_json::from_str::<OpKind>(r#""cherryPick""#).is_err());
    }

    /// An unknown record type is git's business, not ours: skipping it loses
    /// one row, while panicking loses the panel.
    #[test]
    fn an_unrecognised_record_is_passed_over() {
        let tree = parse_status(&porcelain(&["! ignored.txt", "x whatever", "? real.txt"]));
        assert_eq!(tree.changes.len(), 1);
        assert_eq!(tree.changes[0].path, "real.txt");
    }

    /// The four shapes `%(upstream:track,nobracket)` produces, and the two ways a
    /// branch has no answer to give. Pinned here because this string is the whole
    /// of what decides whether a row is drawn orange, and it is the one part of
    /// this feature that a version of git could word differently.
    #[test]
    fn parse_tracking_reads_every_shape_of_track() {
        let out = "main\0origin/main\0ahead 1, behind 2\n\
                   feature/one\0origin/feature/one\0ahead 3\n\
                   feature/two\0origin/feature/two\0behind 4\n\
                   level\0origin/level\0\n\
                   orphan\0\0\n\
                   old\0origin/old\0gone\n";
        let tracking = parse_tracking(out);

        assert_eq!(tracking.len(), 6, "one record per line");
        assert_eq!(
            tracking[0],
            Tracking {
                branch: "main".into(),
                upstream: Some("origin/main".into()),
                ahead: 1,
                behind: 2,
                gone: false
            }
        );
        assert_eq!((tracking[1].ahead, tracking[1].behind), (3, 0));
        assert_eq!((tracking[2].ahead, tracking[2].behind), (0, 4));
        assert_eq!(
            (tracking[3].ahead, tracking[3].behind),
            (0, 0),
            "level with its upstream"
        );
        assert_eq!(tracking[4].upstream, None, "a branch nobody has pushed");
        assert!(!tracking[4].gone, "no upstream is not a gone upstream");
        assert!(tracking[5].gone, "the upstream was deleted on the remote");
    }

    /// A shape this parse has never seen is an ordinary outcome, not a panic and
    /// not a lost row: the branch exists, and leaving it out of the list would
    /// take its row off the panel. Zero counts draw no mark, which is the same
    /// thing on screen as a branch level with its upstream.
    #[test]
    fn parse_tracking_survives_a_track_string_it_does_not_recognise() {
        let tracking = parse_tracking("main\0origin/main\0hinter 1, hinnen 2\n");

        assert_eq!(tracking.len(), 1);
        assert_eq!((tracking[0].ahead, tracking[0].behind), (0, 0));
    }

    /// A slash is ordinary in a branch name and the format's own separator is
    /// `%00`, so nothing about a folder name reaches the field split.
    #[test]
    fn parse_tracking_keeps_a_branch_name_whole() {
        let tracking =
            parse_tracking("fix/legacy/warehouse\0origin/fix/legacy/warehouse\0behind 1\n");

        assert_eq!(tracking[0].branch, "fix/legacy/warehouse");
        assert_eq!(tracking[0].behind, 1);
    }

    /// A blank line is what a repository with no branches at all prints, and an
    /// empty list is the honest answer to it — not a record with no name that
    /// every reader would then have to guard against.
    #[test]
    fn parse_tracking_takes_no_record_from_an_empty_line() {
        assert!(parse_tracking("\n\n").is_empty());
        assert!(parse_tracking("").is_empty());
    }
}
