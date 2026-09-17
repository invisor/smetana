//! Where to look for a tracker.
//!
//! The vocabulary shared by the tracker and settings. The directory search used
//! to live in `tracker::service`, and settings went there for it — a dependency
//! their own comment called a crutch. Now both depend on this, not on each other.

use std::path::{Path, PathBuf};

/// What inside a `.beads` directory makes it an actual bd workspace, rather
/// than bd's own global folder. `bd where`, run against the sidecar binary,
/// accepts any one of these — `metadata.json` or `config.yaml` own the
/// workspace directly, `embeddeddolt/` is the database itself, and
/// `redirect` points `bd where` at a workspace kept elsewhere (a worktree's
/// own `.beads` holding nothing but that file still resolves, `(via redirect
/// from …)`, to the tracker it points at). A `.beads` holding only
/// `eventsData/` is not a workspace to bd, and that is exactly what
/// `~/.beads` holds on any machine that has ever run bd: nothing chosen, just
/// bd's own bookkeeping. Without this list, every folder under the home
/// directory would resolve to the home directory itself, because `~/.beads`
/// would pass as a tracker.
const TRACKER_MARKERS: &[&str] = &["metadata.json", "config.yaml", "embeddeddolt", "redirect"];

/// A folder has a tracker if it holds a `.beads` directory carrying at least
/// one of `TRACKER_MARKERS`. A file named `.beads` does not make one, and
/// neither does a `.beads` with nothing in it but `eventsData/` — bd's own
/// global folder in `$HOME`, not a project's.
pub fn has_tracker(dir: &Path) -> bool {
    let beads = dir.join(".beads");
    if !beads.is_dir() {
        return false;
    }
    TRACKER_MARKERS.iter().any(|marker| beads.join(marker).exists())
}

/// What `.git` directly under a folder is, if anything: an ordinary
/// repository's own directory, or a linked worktree's file pointing at the
/// real one elsewhere.
enum GitEntry {
    /// An ordinary repository — or the main checkout of one with linked
    /// worktrees, which keeps its own `.git` as a directory too.
    Directory,
    /// A linked worktree's own `.git`: one `gitdir:` line away from the
    /// checkout that actually owns the repository.
    File,
}

/// What kind of `.git` entry sits directly in this folder, if any. Marks the
/// boundary the climb in `nearest_tracked_ancestor` will not cross past
/// without first taking the jump a `File` calls for.
fn git_entry(dir: &Path) -> Option<GitEntry> {
    let dot_git = dir.join(".git");
    if dot_git.is_dir() {
        Some(GitEntry::Directory)
    } else if dot_git.is_file() {
        Some(GitEntry::File)
    } else {
        None
    }
}

/// The one jump a linked worktree's `.git` file takes to the checkout that
/// actually owns the repository — `bd where`'s own path, not this app's
/// invention (see `.claude/rules/tracker.md`).
///
/// Reuses `git::git_dir` and `git::common_dir` rather than re-parsing
/// `gitdir:`/`commondir` here: `git_dir(dir)` follows the `.git` file to the
/// worktree's own per-checkout git directory
/// (`<main>/.git/worktrees/<name>`), and `common_dir` of that is the shared
/// half — the main checkout's `.git` itself. Its parent is the main checkout's
/// root, which is what this hands back.
///
/// `None` when that root does not exist: a `gitdir:` line pointing at a
/// checkout somebody deleted, or one this cannot parse at all. `bd` does not
/// find a tracker through a broken `gitdir` either, so refusing the jump here
/// is the same honest "no tracker" the caller already gives for any other
/// boundary.
///
/// `common_dir` hands back an unnormalised path — `<worktree-git>/../..`, not
/// `<main>/.git` — because it never touches the disk beyond one read, and
/// `Path::parent` only strips the last syntactic component, so calling it on
/// that path directly would strip one `..` rather than climb one real
/// directory. `lexically_normalized` collapses the `..`s first, in plain
/// component algebra with no filesystem call in it — `canonicalize` would do
/// the same job and also resolve symlinks along the way, which is a question
/// this file otherwise never asks (`/tmp` itself is one on macOS, and a test
/// against it caught this the first time round).
fn main_checkout(dir: &Path) -> Option<PathBuf> {
    let git_dir = crate::git::git_dir(dir)?;
    let common = lexically_normalized(&crate::git::common_dir(&git_dir));
    let root = common.parent()?.to_path_buf();
    root.is_dir().then_some(root)
}

/// Collapses `.` and `..` components without touching the disk — the same
/// plain component algebra a build tool's own path-cleaning does. Only
/// [`main_checkout`] needs it, for the reason given there.
fn lexically_normalized(path: &Path) -> PathBuf {
    use std::path::Component;
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// The nearest ancestor that has a tracker. The folder itself is an ancestor
/// too, and is checked for a tracker before the git boundary below can rule
/// it out — so a folder that carries both `.git` and `.beads` is found as
/// itself.
///
/// The climb stops at the first ancestor that carries a `.git` **directory**:
/// that folder is still checked for a tracker, nothing above it is. This
/// mirrors bd itself — from inside a nested repository bd does not see a
/// `.beads` that sits above it, because the nested `.git` is a repository
/// boundary bd respects. Short of a `.git` anywhere, the climb goes all the
/// way to the filesystem root, the same as before this boundary existed,
/// because bd finds a tracker through ancestors with no git in them too.
///
/// A `.git` **file** is not that boundary — it is a linked worktree, the same
/// repository as whatever it points at, and bd goes straight through it to
/// answer with the main checkout's tracker (smetana-1wgi; measured with the
/// sidecar against a worktree both nested under its repository, Smetana's own
/// layout, and standing beside it as a sibling, `git worktree add ../x`).
/// So the climb takes exactly one jump through `main_checkout` and checks
/// that root for a tracker — not the ordinary loop continuing from there,
/// because it does not need to: the main checkout's own `.git` is always a
/// directory, so a second pass over it would find the very same boundary
/// immediately and stop. No recursion, no cycle to guard against, and a
/// broken or missing jump target answers `None` exactly as any other boundary
/// with no tracker on it does. This is a question for the filesystem, not for
/// git: no `git` process runs here, `main_checkout` included.
pub fn nearest_tracked_ancestor(start: &Path) -> Option<PathBuf> {
    for dir in start.ancestors() {
        if has_tracker(dir) {
            return Some(dir.to_path_buf());
        }
        match git_entry(dir) {
            Some(GitEntry::Directory) => return None,
            Some(GitEntry::File) => return main_checkout(dir).filter(|root| has_tracker(root)),
            None => {}
        }
    }
    None
}

/// What to open with on the very first run, when the project list is still empty.
///
/// Plain `current_dir` is no good in any real launch: under `npm run tauri dev`
/// the binary starts from `src-tauri/`, and a built macOS app opened from
/// Finder starts from `/` altogether. Hence the walk up the ancestors.
///
/// Nothing found means there is no project, and that is no trouble: the list is
/// empty and a person will pick a folder themselves. This used to return the
/// working directory, and the app said "there is no .beads here" about a folder
/// nobody had chosen.
pub fn default_project() -> Option<PathBuf> {
    nearest_tracked_ancestor(&std::env::current_dir().ok()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// A directory of its own per test: the name carries the pid, so parallel
    /// runs do not get in each other's way.
    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("smetana-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create the temp directory");
        dir
    }

    /// A linked worktree on disk, laid out the way git lays one out: the
    /// shared half of the repository under `<main>/.git`, and the worktree's
    /// own `.git` file one `gitdir:` line away from
    /// `<main>/.git/worktrees/wt`, which itself carries the `commondir` line
    /// git writes there. `nested` decides whether the worktree sits *under*
    /// the main checkout — Smetana's own layout, `.worktrees/<slug>` — or
    /// *beside* it, what a plain `git worktree add ../x` makes; both are
    /// exercised because bd answers with the same tracker either way
    /// (smetana-1wgi). Returns the scratch root, the main checkout and the
    /// linked worktree.
    fn linked_worktree(name: &str, nested: bool) -> (PathBuf, PathBuf, PathBuf) {
        let root = scratch(name);
        let main = root.join("main");
        let linked = if nested { main.join(".worktrees/wt") } else { root.join("wt") };
        let wt_git = main.join(".git/worktrees/wt");
        fs::create_dir_all(&wt_git).expect("create the worktree's own git directory");
        fs::create_dir_all(&linked).expect("create the linked worktree");
        fs::write(wt_git.join("commondir"), "../..\n").expect("write commondir");
        fs::write(linked.join(".git"), format!("gitdir: {}\n", wt_git.display())).expect("write the .git file");
        (root, main, linked)
    }

    #[test]
    fn a_folder_with_beads_is_tracked() {
        let root = scratch("has-tracker");
        assert!(!has_tracker(&root), "an empty folder does not count as a tracker");
        fs::create_dir_all(root.join(".beads")).unwrap();
        fs::write(root.join(".beads/metadata.json"), "{}").unwrap();
        assert!(has_tracker(&root));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_file_named_beads_does_not_count_as_a_tracker() {
        let root = scratch("beads-file");
        fs::write(root.join(".beads"), "not a directory").unwrap();
        assert!(!has_tracker(&root));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_beads_folder_with_only_events_data_is_not_a_tracker() {
        // This is what `~/.beads` looks like on any machine that has ever run
        // bd: bd's own global folder, not a chosen project.
        let root = scratch("events-data-only");
        fs::create_dir_all(root.join(".beads/eventsData")).unwrap();
        assert!(!has_tracker(&root), "a .beads with only eventsData is bd's own global folder, not a tracker");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_beads_folder_with_config_yaml_is_tracked() {
        let root = scratch("config-yaml");
        fs::create_dir_all(root.join(".beads")).unwrap();
        fs::write(root.join(".beads/config.yaml"), "").unwrap();
        assert!(has_tracker(&root));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_beads_folder_with_embeddeddolt_is_tracked() {
        let root = scratch("embeddeddolt");
        fs::create_dir_all(root.join(".beads/embeddeddolt")).unwrap();
        assert!(has_tracker(&root));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_beads_folder_with_only_a_redirect_is_tracked() {
        // A worktree's own .beads holds nothing but this file and points
        // `bd where` at the real workspace elsewhere; bd accepts it as a
        // workspace, so this app must too (smetana-0hrt review pass 1).
        let root = scratch("redirect");
        fs::create_dir_all(root.join(".beads")).unwrap();
        fs::write(root.join(".beads/redirect"), "/elsewhere/.beads").unwrap();
        assert!(has_tracker(&root));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn the_search_climbs_to_the_nearest_ancestor() {
        let root = scratch("ancestor");
        let deep = root.join("a/b/c");
        fs::create_dir_all(&deep).unwrap();
        fs::create_dir_all(root.join(".beads")).unwrap();
        fs::write(root.join(".beads/metadata.json"), "{}").unwrap();
        assert_eq!(nearest_tracked_ancestor(&deep).as_deref(), Some(root.as_path()));
        assert_eq!(nearest_tracked_ancestor(&root).as_deref(), Some(root.as_path()), "the folder itself is an ancestor too");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn the_climb_skips_a_beads_folder_with_no_marker_and_continues() {
        let root = scratch("skip-unmarked");
        let deep = root.join("a/b");
        fs::create_dir_all(&deep).unwrap();
        // A bare .beads partway up, the shape of bd's own global folder —
        // must not stop the climb before it reaches the real tracker above.
        fs::create_dir_all(root.join("a/.beads/eventsData")).unwrap();
        fs::create_dir_all(root.join(".beads")).unwrap();
        fs::write(root.join(".beads/metadata.json"), "{}").unwrap();
        assert_eq!(nearest_tracked_ancestor(&deep).as_deref(), Some(root.as_path()));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn with_no_beads_anywhere_above_the_search_finds_nothing() {
        let root = scratch("nothing");
        let deep = root.join("x/y");
        fs::create_dir_all(&deep).unwrap();
        assert_eq!(nearest_tracked_ancestor(&deep), None, "there must be no tracker in the temp directory or above it");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn the_climb_does_not_pass_a_git_directory() {
        let root = scratch("git-boundary-dir");
        fs::create_dir_all(root.join(".beads")).unwrap();
        fs::write(root.join(".beads/metadata.json"), "{}").unwrap();
        let inner = root.join("inner");
        fs::create_dir_all(inner.join(".git")).unwrap();
        let deep = inner.join("src");
        fs::create_dir_all(&deep).unwrap();
        assert_eq!(
            nearest_tracked_ancestor(&deep),
            None,
            "a nested repository's own .git is a boundary the climb must not cross"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn the_climb_does_not_pass_a_git_file() {
        // A linked worktree's .git is a file pointing at the main checkout
        // that owns the repository (smetana-1wgi): bd goes straight through
        // it and answers with that checkout's tracker, so the climb takes
        // the same jump rather than treating the file as a boundary.
        let (root, main, linked) = linked_worktree("git-boundary-file", true);
        fs::create_dir_all(main.join(".beads/embeddeddolt")).unwrap();
        let deep = linked.join("src");
        fs::create_dir_all(&deep).unwrap();
        assert_eq!(
            nearest_tracked_ancestor(&deep).as_deref(),
            Some(main.as_path()),
            "a .git file jumps to the main checkout it points at, the same place bd answers from"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_tracker_is_found_through_ancestors_with_no_git() {
        let root = scratch("no-git-ancestors");
        let deep = root.join("a/b/c");
        fs::create_dir_all(&deep).unwrap();
        fs::create_dir_all(root.join(".beads")).unwrap();
        fs::write(root.join(".beads/metadata.json"), "{}").unwrap();
        assert_eq!(
            nearest_tracked_ancestor(&deep).as_deref(),
            Some(root.as_path()),
            "bd itself finds a tracker through ancestors with no .git in them"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_folder_with_both_git_and_beads_is_found_as_itself() {
        let root = scratch("git-and-beads");
        fs::create_dir_all(root.join(".git")).unwrap();
        fs::create_dir_all(root.join(".beads")).unwrap();
        fs::write(root.join(".beads/metadata.json"), "{}").unwrap();
        assert_eq!(nearest_tracked_ancestor(&root).as_deref(), Some(root.as_path()));

        // The path `project_root` actually takes on every folder pick inside a
        // tracked repository: start below the root, not at it. Checking only
        // the root itself proves the first loop iteration orders the checks
        // correctly and nothing else — swapping the tracker and git checks
        // inside the loop would still pass that assertion while breaking this
        // one (smetana-0hrt review pass 1).
        let src = root.join("src");
        fs::create_dir_all(&src).unwrap();
        assert_eq!(nearest_tracked_ancestor(&src).as_deref(), Some(root.as_path()));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_sibling_worktrees_git_file_jumps_to_the_main_checkout_too() {
        // The measured case in smetana-1wgi: the linked worktree is not a
        // descendant of the main checkout at all — `git worktree add ../x` —
        // so climbing ancestors could never reach it on its own. bd finds the
        // tracker by following `gitdir:`, and so must this.
        let (root, main, linked) = linked_worktree("git-file-sibling", false);
        fs::create_dir_all(main.join(".beads/embeddeddolt")).unwrap();
        let deep = linked.join("src");
        fs::create_dir_all(&deep).unwrap();
        assert_eq!(
            nearest_tracked_ancestor(&deep).as_deref(),
            Some(main.as_path()),
            "a worktree beside the main checkout still resolves to it through gitdir"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_git_file_pointing_nowhere_stays_a_boundary() {
        // A `gitdir:` line naming a checkout that does not exist — the main
        // checkout deleted, or the file simply corrupt. No second jump is
        // attempted; this is the honest "no tracker" bd itself gives here.
        let root = scratch("git-file-broken");
        let linked = root.join("wt");
        fs::create_dir_all(&linked).unwrap();
        fs::write(linked.join(".git"), "gitdir: /nowhere/on/this/machine/.git/worktrees/wt\n").unwrap();
        // A tracker sits above the worktree, and must not be reachable by
        // simply ignoring the broken .git file and climbing past it.
        fs::create_dir_all(root.join(".beads/embeddeddolt")).unwrap();
        let deep = linked.join("src");
        fs::create_dir_all(&deep).unwrap();
        assert_eq!(
            nearest_tracked_ancestor(&deep),
            None,
            "a gitdir pointing nowhere is still a boundary, not a hole the climb falls through"
        );
        let _ = fs::remove_dir_all(&root);
    }
}
