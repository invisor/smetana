//! Which stored pictures are still wanted, and where a project's pictures live.
//!
//! The whole of the deleting rule is here, with no filesystem anywhere in it:
//! it is a function of a list of files and a snapshot of the board, so the one
//! irreversible thing this app does to somebody's pictures is decided by
//! something a test can drive.
//!
//! **A file's owner is computed, never stored.** There is no record anywhere of
//! which task a picture was attached to, and there cannot be one: the dialog
//! hands the agent absolute paths in a prompt, the agent runs `bd create`
//! itself, and no channel comes back saying which id it wrote. So the question
//! "is anybody still using this file" is answered the only way it can be — by
//! reading the board and looking for the path in it, the same reconstruction
//! `claimedBy` in `terminals.js` makes for the same missing channel.

use std::path::Path;

use crate::tracker::model::{HealthState, Issue};

use super::{slug, AttachmentError};

/// bd's own word for an issue that is over. Everything else — `open`,
/// `in_progress`, `parked`, a custom status this app has never heard of —
/// counts as a task somebody may still open tomorrow, and its pictures stay.
const CLOSED: &str = "closed";

/// FNV-1a, 64 bits: the same hash `status.js` uses on the front end, and
/// written out here rather than borrowed from `DefaultHasher` because that one
/// is documented as free to change between Rust releases. A directory name that
/// moved with the toolchain would orphan every picture already stored under the
/// old one.
fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// The one directory under `attachments/` that belongs to this project.
///
/// It has to be three things at once: derivable from the project's absolute
/// path alone (nothing is written down anywhere that could be lost), the same
/// on every run (a key that moved would strand every picture stored under the
/// old one), and safe as a single path segment — because this string is joined
/// onto the store's root and everything deleted is found by walking the result.
///
/// So it is the folder's own name through the same `slug` a stored file's name
/// goes through, ASCII letters and digits and nothing else, with the hash of
/// the whole path after it. The name is for the person who opens the directory
/// in Finder; the hash is what actually distinguishes them, since two projects
/// called `app` in different places are two projects.
pub fn project_key(project: &Path) -> String {
    let name = project
        .file_name()
        .map(|name| slug(name.to_string_lossy().as_ref()))
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "project".to_owned());
    format!("{name}-{:016x}", fnv1a64(project.to_string_lossy().as_bytes()))
}

/// A name that can only mean a file sitting directly in the directory it came
/// from. Everything deleted is a `read_dir` entry's own `file_name`, which can
/// carry neither of these by construction — this is the second lock on the same
/// door, cheap enough to keep and the kind of thing worth having in front of an
/// irreversible `remove_file`.
pub fn plain_name(name: &str) -> bool {
    !name.is_empty()
        && name != "."
        && name != ".."
        && !name.contains('/')
        && !name.contains('\\')
        && !name.contains('\0')
}

/// One file in the store, as the rule sees it: what it is called inside its
/// directory, the absolute path that would have reached an agent, and its size.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredFile {
    pub name: String,
    pub path: String,
    pub bytes: u64,
}

/// How much of something there is. Files and bytes travel together everywhere
/// here because a person about to delete wants both: seven files is a shrug,
/// seven files of 400 MB is not.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize)]
pub struct Tally {
    pub files: u64,
    pub bytes: u64,
}

impl Tally {
    pub fn add(&mut self, bytes: u64) {
        self.files += 1;
        self.bytes += bytes;
    }
}

/// Whether bd would still call this task somebody's business.
fn unfinished(issue: &Issue) -> bool {
    !issue.status.trim().eq_ignore_ascii_case(CLOSED)
}

/// Every field of an issue a path could have been written into.
///
/// Four, and deliberately every one of the prose fields rather than the one the
/// prompt asks for: the agent decides where the link goes, and a spec that put
/// it under Design instead of in the description would otherwise have its
/// pictures deleted out from under it. Naming a field too many costs a file
/// kept for nothing; naming one too few costs somebody's screenshot, and only
/// one of those two is recoverable.
fn prose(issue: &Issue) -> [Option<&str>; 4] {
    [
        issue.description.as_deref(),
        issue.acceptance_criteria.as_deref(),
        issue.design.as_deref(),
        issue.notes.as_deref(),
    ]
}

/// Why a sweep must not run, or `None` when it may.
///
/// **An empty board and an unreadable board are the same `Snapshot` and
/// opposite facts**, and the whole of this function exists to keep them apart.
/// `removable` reads a board that holds nothing as "no task refers to any of
/// these files", which is the truth when bd was read and the project genuinely
/// has no issues — and a catastrophe when bd could not be read at all, because
/// the worker keeps the project open with an empty store after a failed sync
/// and every attachment of every live task would then be unreferenced. The
/// states that reach here are ordinary, not exotic: no bd on the machine (which
/// `postinstall` explicitly tolerates), a version mismatch, a damaged `.beads`,
/// and a folder that has no tracker at all — where the app deliberately stays
/// open so `bd init` can be offered.
///
/// So the rule is the one `runs/browser.rs` already sets for this repository:
/// anything unobservable reads as "no", loudly. A board that is not `Ok` is not
/// a board, and the pictures in that project's folder have nobody to vouch for
/// them — which is exactly the argument `NoProject` was already written for.
pub fn refusal(project: Option<&Path>, board: &HealthState) -> Option<AttachmentError> {
    if project.is_none() {
        return Some(AttachmentError::NoProject);
    }
    match board {
        HealthState::Ok => None,
        // The health message would be a second copy of what the board area of
        // the app window is already saying; the kind is what this refusal is
        // about, and the front end has its own sentence for it.
        _ => Some(AttachmentError::NoBoard),
    }
}

/// What may go: every file no unfinished task mentions.
///
/// Three cases and one rule. A file named by a task that is not closed stays; a
/// file named only by closed tasks goes; a file nothing at all names goes — and
/// that third case is most of the rubbish, since a dialog closed without filing
/// anything, a thumbnail taken back out, and an agent that never copied the path
/// into the description all land there. It is also the case that makes the
/// directory stop growing, which is the whole point of the exercise.
///
/// A mention is the absolute path appearing anywhere in the text. Nothing finer
/// is possible: the path arrives in a prompt and is written into prose by an
/// agent, so it may sit inside a markdown link, a sentence or a fenced block.
pub fn removable<'a>(files: &'a [StoredFile], issues: &[Issue]) -> Vec<&'a StoredFile> {
    let live: Vec<&str> = issues
        .iter()
        .filter(|issue| unfinished(issue))
        .flat_map(prose)
        .flatten()
        .collect();

    files
        .iter()
        .filter(|file| !live.iter().any(|text| text.contains(file.path.as_str())))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(path: &str, bytes: u64) -> StoredFile {
        let name = path.rsplit('/').next().unwrap_or(path).to_owned();
        StoredFile { name, path: path.to_owned(), bytes }
    }

    fn issue(status: &str, description: &str) -> Issue {
        Issue {
            id: "smetana-x".into(),
            status: status.into(),
            description: Some(description.into()),
            ..Issue::default()
        }
    }

    const A: &str = "/data/attachments/app-0011223344556677/20260806-121314-shot.png";
    const B: &str = "/data/attachments/app-0011223344556677/20260806-121500-mock.png";

    #[test]
    fn a_file_an_unfinished_task_still_names_is_kept() {
        let files = [file(A, 10)];
        let issues = [issue("open", &format!("Broken, see {A}"))];
        assert!(removable(&files, &issues).is_empty());
    }

    #[test]
    fn every_status_that_is_not_closed_counts_as_unfinished() {
        // bd has eleven of them and this app knows three; a status nobody here
        // has heard of must not be read as "over" — that would delete the
        // pictures of a task an agent is working on right now.
        let files = [file(A, 10)];
        for status in ["open", "in_progress", "parked", "ready_to_merge", "hooked", "whatever"] {
            let issues = [issue(status, A)];
            assert!(removable(&files, &issues).is_empty(), "{status} is not a finished task");
        }
    }

    #[test]
    fn a_file_only_a_closed_task_names_is_removable() {
        let files = [file(A, 10)];
        let issues = [issue("closed", &format!("Fixed, see {A}"))];
        assert_eq!(removable(&files, &issues), vec![&files[0]]);
    }

    #[test]
    fn a_file_nothing_at_all_names_is_removable() {
        // The dialog closed without filing anything, the thumbnail taken back
        // out, the agent that never copied the path across: most of the store.
        let files = [file(A, 10)];
        let issues = [issue("open", "nothing to do with pictures")];
        assert_eq!(removable(&files, &issues), vec![&files[0]]);
    }

    #[test]
    fn one_unfinished_mention_outweighs_any_number_of_closed_ones() {
        let files = [file(A, 10)];
        let issues = [
            issue("closed", A),
            issue("closed", A),
            issue("open", &format!("still relevant: {A}")),
        ];
        assert!(removable(&files, &issues).is_empty());
    }

    #[test]
    fn a_path_is_found_in_any_of_the_four_prose_fields() {
        let files = [file(A, 10)];
        let fields: [(&str, fn(String) -> Issue); 4] = [
            ("description", |text| Issue { description: Some(text), ..Issue::default() }),
            ("acceptance criteria", |text| Issue {
                acceptance_criteria: Some(text),
                ..Issue::default()
            }),
            ("design", |text| Issue { design: Some(text), ..Issue::default() }),
            ("notes", |text| Issue { notes: Some(text), ..Issue::default() }),
        ];
        for (name, make) in fields {
            let issues = [Issue { status: "open".into(), ..make(format!("see {A}")) }];
            assert!(removable(&files, &issues).is_empty(), "{name} was not read");
        }
    }

    #[test]
    fn the_path_is_found_inside_prose_rather_than_only_on_a_line_of_its_own() {
        // What an agent writes is a sentence or a markdown link, never a bare
        // path on its own line, so anything stricter than containment would
        // delete the pictures of the tasks that did the right thing.
        let files = [file(A, 10)];
        let issues = [issue("open", &format!("![the crash]({A}) — note the toolbar"))];
        assert!(removable(&files, &issues).is_empty());
    }

    #[test]
    fn each_file_is_judged_on_its_own() {
        let files = [file(A, 10), file(B, 20)];
        let issues = [issue("open", A)];
        assert_eq!(removable(&files, &issues), vec![&files[1]]);
    }

    #[test]
    fn a_board_that_was_read_and_holds_nothing_makes_every_file_removable() {
        // A project whose tracker really is empty, and the honest answer: no
        // task refers to any of it. That this function cannot tell such a board
        // from one that could not be read is the reason `refusal` exists and is
        // asked first — see the test below.
        let files = [file(A, 10), file(B, 20)];
        assert_eq!(removable(&files, &[]), vec![&files[0], &files[1]]);
    }

    #[test]
    fn a_board_that_could_not_be_read_sweeps_nothing() {
        // The other half of the pair above, and the one that costs somebody
        // their screenshots when it is missing: every one of these states
        // leaves the worker holding an open project and an empty snapshot, and
        // `removable` would then call every attachment of every live task
        // unreferenced.
        let project = Path::new("/projects/mine");
        for board in [
            HealthState::Error,
            HealthState::BdVersionMismatch,
            HealthState::NotABeadsRepo,
            HealthState::NoProject,
        ] {
            assert!(
                matches!(refusal(Some(project), &board), Some(AttachmentError::NoBoard)),
                "{board:?} is not a board that can vouch for anything"
            );
        }
    }

    #[test]
    fn a_readable_board_over_an_open_project_is_the_one_case_that_sweeps() {
        assert!(refusal(Some(Path::new("/projects/mine")), &HealthState::Ok).is_none());
    }

    #[test]
    fn with_no_project_the_refusal_says_that_rather_than_blaming_the_board() {
        // Two absences with two answers: nothing is open, against something is
        // open and cannot be read. The person can act on the first.
        assert!(matches!(refusal(None, &HealthState::Ok), Some(AttachmentError::NoProject)));
        assert!(matches!(refusal(None, &HealthState::NoProject), Some(AttachmentError::NoProject)));
    }

    #[test]
    fn a_projects_key_is_one_safe_segment_naming_the_folder() {
        let key = project_key(Path::new("/Users/you/Projects/smetana"));
        assert!(key.starts_with("smetana-"), "{key}");
        assert!(
            key.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '-'),
            "this string is joined onto the store's root: {key}"
        );
    }

    #[test]
    fn the_key_is_the_same_every_run_and_different_per_project() {
        let one = Path::new("/Users/you/Projects/app");
        let two = Path::new("/Users/you/Work/app");
        assert_eq!(project_key(one), project_key(one), "a key that moved would strand every picture under it");
        assert_ne!(
            project_key(one),
            project_key(two),
            "two projects of the same name in different places are two projects"
        );
    }

    #[test]
    fn a_project_name_with_nothing_ascii_in_it_still_gets_a_key() {
        // The name is only for a person reading the directory listing; the hash
        // is what tells two projects apart, so there is always a key. Any
        // script would do; this one reads "project" in Japanese, matching the
        // fixture `stored_name` is tested with next door.
        let key = project_key(Path::new("/Users/you/プロジェクト/しごと"));
        assert!(key.starts_with("project-"), "{key}");
        assert!(key.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '-'), "{key}");
    }

    #[test]
    fn a_key_cannot_climb_out_of_the_store_however_the_folder_is_called() {
        // The name is a slug and the rest is hex, so a folder called `..` or
        // one with a separator in its name produces neither.
        for path in ["/tmp/..", "/tmp/a b/c", "/"] {
            let key = project_key(Path::new(path));
            assert!(plain_name(&key), "{path} produced {key}");
            assert!(!key.starts_with('.'), "{path} produced {key}");
        }
    }

    #[test]
    fn a_name_that_could_mean_another_directory_is_refused() {
        assert!(plain_name("20260806-121314-shot.png"));
        assert!(!plain_name(".."));
        assert!(!plain_name("."));
        assert!(!plain_name(""));
        assert!(!plain_name("../../keys.png"));
        assert!(!plain_name("sub/shot.png"));
    }
}
