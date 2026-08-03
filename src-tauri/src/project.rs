//! Where to look for a tracker.
//!
//! The vocabulary shared by the tracker and settings. The directory search used
//! to live in `tracker::service`, and settings went there for it — a dependency
//! their own comment called a crutch. Now both depend on this, not on each other.

use std::path::{Path, PathBuf};

/// A folder has a tracker if it holds a `.beads` directory. A file with that
/// name does not make one.
pub fn has_tracker(dir: &Path) -> bool {
    dir.join(".beads").is_dir()
}

/// The nearest ancestor that has a tracker. The folder itself is an ancestor too.
pub fn nearest_tracked_ancestor(start: &Path) -> Option<PathBuf> {
    start.ancestors().find(|dir| has_tracker(dir)).map(Path::to_path_buf)
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
    fn the_search_climbs_to_the_nearest_ancestor() {
        let root = scratch("ancestor");
        let deep = root.join("a/b/c");
        fs::create_dir_all(&deep).unwrap();
        fs::create_dir_all(root.join(".beads")).unwrap();
        assert_eq!(nearest_tracked_ancestor(&deep).as_deref(), Some(root.as_path()));
        assert_eq!(nearest_tracked_ancestor(&root).as_deref(), Some(root.as_path()), "the folder itself is an ancestor too");
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
}
