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
    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        match res {
            Ok(event) => {
                if event.paths.iter().any(|p| is_relevant(p)) {
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
