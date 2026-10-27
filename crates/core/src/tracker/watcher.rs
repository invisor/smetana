use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::mpsc::Sender;

/// There are exactly three paths that matter. Everything else in .beads —
/// configs, backups and the git-remote cache — makes noise but has nothing to
/// do with the tracker's contents.
pub fn is_relevant(path: &Path) -> bool {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    let in_noms = path.parent().and_then(|p| p.file_name()).and_then(|n| n.to_str()) == Some("noms");
    (in_noms && (name == "manifest" || name == "journal.idx")) || name == "last-touched"
}

/// Whether a write to a relevant file means somebody changed the tracker.
///
/// A write to `noms/manifest` or `noms/journal.idx` by itself means nothing:
/// bd opens the embedded Dolt on every call, and even a purely reading
/// `bd list` rewrites both files. Measured on bd 1.1.2 (2026-10-28, a
/// throwaway database; three `bd list -n 1 --json` and one
/// `bd list --all -n 0 --updated-after … --json`): every read moved the
/// mtime of `manifest` and `journal.idx` and left their bytes identical
/// (same size, same hash), and left `last-touched` completely alone. One
/// `bd create` then changed the content of all three (manifest hash,
/// journal.idx grew, last-touched got a new timestamp). Taking the mtime as
/// the signal therefore made every sync trigger the next one, about once a
/// second, at 40-50 % CPU per bd launch.
///
/// So the event is compared by content: a file counts as changed only when
/// its bytes differ from the last ones seen at that path. This loses nothing
/// to a suppression window (no external write can hide inside one); the one
/// cost is that the first sight of a path always counts as a change, which is
/// a single harmless extra sync after start.
#[derive(Default)]
pub struct Fingerprints {
    seen: HashMap<PathBuf, u64>,
}

impl Fingerprints {
    /// `bytes` is the file's current content, `None` if it could not be read
    /// (gone, or caught mid-rename) - which always counts as a change, since
    /// a miss costs more than a repeat.
    pub fn changed(&mut self, path: &Path, bytes: Option<&[u8]>) -> bool {
        let Some(bytes) = bytes else {
            self.seen.remove(path);
            return true;
        };
        let mut h = DefaultHasher::new();
        bytes.hash(&mut h);
        let digest = h.finish();
        self.seen.insert(path.to_path_buf(), digest) != Some(digest)
    }

    /// Reads the file and asks [`Self::changed`] about it.
    pub fn changed_on_disk(&mut self, path: &Path) -> bool {
        let bytes = std::fs::read(path).ok();
        self.changed(path, bytes.as_deref())
    }
}

/// What the watcher tells the worker.
pub enum WatchEvent {
    /// Something was written in `.beads` — time to catch up.
    Changed,
    /// The watching broke mid-flight. All that remains is the periodic
    /// once-a-minute sweep, and more than the log should know about that.
    Failed(String),
}

/// The returned watcher has to be kept alive: dropping it stops the watching
/// silently.
pub fn spawn(beads_dir: PathBuf, tx: Sender<WatchEvent>) -> notify::Result<RecommendedWatcher> {
    let mut seen = Fingerprints::default();
    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        match res {
            Ok(event) => {
                // Not `any(..)` over a short-circuit: every relevant path has to
                // be fingerprinted so that its baseline stays current.
                let mut changed = false;
                for p in event.paths.iter().filter(|p| is_relevant(p)) {
                    changed |= seen.changed_on_disk(p);
                }
                if changed {
                    // The worker collapses frequent events; all that matters
                    // here is not to block when the queue is already full.
                    let _ = tx.try_send(WatchEvent::Changed);
                }
            }
            // An error swallowed here would mean an app that lives on the
            // sixty-second sweep alone until a restart and says nothing about
            // it. An overflowing queue threatens no loss: it is full exactly
            // when events are streaming, that is, when the watching is alive.
            Err(e) => {
                let _ = tx.try_send(WatchEvent::Failed(e.to_string()));
            }
        }
    })?;
    watcher.watch(&beads_dir, RecursiveMode::Recursive)?;
    Ok(watcher)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn catches_a_dolt_write() {
        assert!(is_relevant(Path::new(
            "/p/.beads/embeddeddolt/smetana/.dolt/noms/manifest"
        )));
        assert!(is_relevant(Path::new(
            "/p/.beads/embeddeddolt/smetana/.dolt/noms/journal.idx"
        )));
    }

    #[test]
    fn a_rewrite_with_the_same_bytes_is_not_a_change() {
        let mut f = Fingerprints::default();
        let p = Path::new("/p/.beads/embeddeddolt/t/.dolt/noms/manifest");
        // First sight counts.
        assert!(f.changed(p, Some(b"root-1")));
        // Our own bd list: the file was written again, the bytes are the same.
        assert!(!f.changed(p, Some(b"root-1")));
        assert!(!f.changed(p, Some(b"root-1")));
        // Somebody else wrote: the content moved.
        assert!(f.changed(p, Some(b"root-2")));
        assert!(!f.changed(p, Some(b"root-2")));
        // An unreadable file is never trusted to be unchanged.
        assert!(f.changed(p, None));
        assert!(f.changed(p, Some(b"root-2")));
    }

    #[test]
    fn paths_are_tracked_independently() {
        let mut f = Fingerprints::default();
        let a = Path::new("/p/.beads/last-touched");
        let b = Path::new("/p/.beads/embeddeddolt/t/.dolt/noms/manifest");
        assert!(f.changed(a, Some(b"x")));
        assert!(f.changed(b, Some(b"x")));
        assert!(!f.changed(a, Some(b"x")));
        assert!(f.changed(a, Some(b"y")));
        assert!(!f.changed(b, Some(b"x")));
    }

    #[test]
    fn reads_the_real_file() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("manifest");
        std::fs::write(&p, "a").unwrap();
        let mut f = Fingerprints::default();
        assert!(f.changed_on_disk(&p));
        std::fs::write(&p, "a").unwrap(); // touched, not changed
        assert!(!f.changed_on_disk(&p));
        std::fs::write(&p, "b").unwrap();
        assert!(f.changed_on_disk(&p));
        std::fs::remove_file(&p).unwrap();
        assert!(f.changed_on_disk(&p));
    }

    #[test]
    fn catches_last_touched() {
        assert!(is_relevant(Path::new("/p/.beads/last-touched")));
    }

    #[test]
    fn ignores_the_noise() {
        assert!(!is_relevant(Path::new("/p/.beads/config.yaml")));
        assert!(!is_relevant(Path::new("/p/.beads/backup/LOCK")));
        assert!(!is_relevant(Path::new(
            "/p/.beads/embeddeddolt/smetana/.dolt/git-remote-cache/x/repo.git/config"
        )));
    }
}
