//! Where to look for a tracker.
//!
//! The vocabulary shared by the tracker and settings. The directory search used
//! to live in `tracker::service`, and settings went there for it — a dependency
//! their own comment called a crutch. Now both depend on this, not on each other.

use std::path::{Path, PathBuf};

/// What inside a `.beads` directory makes it an actual bd workspace, rather
/// than bd's own global folder. `bd where`, run against the sidecar binary,
/// accepts any one of these — a `.beads` holding only `eventsData/` is not a
/// workspace to bd either, and that is exactly what `~/.beads` holds on any
/// machine that has ever run bd: nothing chosen, just bd's own bookkeeping.
/// Without this list, every folder under the home directory would resolve to
/// the home directory itself, because `~/.beads` would pass as a tracker.
const TRACKER_MARKERS: &[&str] = &["metadata.json", "config.yaml", "embeddeddolt"];

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

/// Whether this folder carries a `.git` entry, directory or file — a worktree
/// stores it as a file pointing at the real repository elsewhere. Marks the
/// boundary the climb in `nearest_tracked_ancestor` will not cross.
fn has_git_entry(dir: &Path) -> bool {
    dir.join(".git").exists()
}

/// The nearest ancestor that has a tracker. The folder itself is an ancestor
/// too, and is checked for a tracker before the git boundary below can rule
/// it out — so a folder that carries both `.git` and `.beads` is found as
/// itself.
///
/// The climb stops at the first ancestor that carries a `.git` entry: that
/// folder is still checked for a tracker, nothing above it is. This mirrors
/// bd itself — from inside a nested repository bd does not see a `.beads`
/// that sits above it, because the nested `.git` is a repository boundary bd
/// respects. Short of a `.git` anywhere, the climb goes all the way to the
/// filesystem root, the same as before this boundary existed, because bd
/// finds a tracker through ancestors with no git in them too. This is a
/// question for the filesystem, not for git: no `git` process runs here.
pub fn nearest_tracked_ancestor(start: &Path) -> Option<PathBuf> {
    for dir in start.ancestors() {
        if has_tracker(dir) {
            return Some(dir.to_path_buf());
        }
        if has_git_entry(dir) {
            return None;
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
        // A worktree's .git is a file pointing elsewhere, not a directory.
        let root = scratch("git-boundary-file");
        fs::create_dir_all(root.join(".beads")).unwrap();
        fs::write(root.join(".beads/metadata.json"), "{}").unwrap();
        let inner = root.join("inner");
        fs::create_dir_all(&inner).unwrap();
        fs::write(inner.join(".git"), "gitdir: /elsewhere/.git/worktrees/inner").unwrap();
        let deep = inner.join("src");
        fs::create_dir_all(&deep).unwrap();
        assert_eq!(
            nearest_tracked_ancestor(&deep),
            None,
            "a .git file marks a worktree boundary just as a .git directory does"
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
        let _ = fs::remove_dir_all(&root);
    }
}
