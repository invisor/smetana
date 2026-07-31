use std::path::{Path, PathBuf};

use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::mpsc::Sender;

/// Значимых путей ровно три. Всё остальное в .beads — конфиги, бэкапы и
/// кэш git-ремоута — шумит, но к содержимому трекера отношения не имеет.
pub fn is_relevant(path: &Path) -> bool {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    let in_noms = path.parent().and_then(|p| p.file_name()).and_then(|n| n.to_str()) == Some("noms");
    (in_noms && (name == "manifest" || name == "journal.idx")) || name == "last-touched"
}

/// Возвращённый watcher нужно держать живым: при его уничтожении слежение
/// прекращается молча.
pub fn spawn(beads_dir: PathBuf, tx: Sender<()>) -> notify::Result<RecommendedWatcher> {
    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        let Ok(event) = res else { return };
        if event.paths.iter().any(|p| is_relevant(p)) {
            // Схлопывание частых событий делает воркер; здесь достаточно
            // не блокироваться, если очередь уже полна.
            let _ = tx.try_send(());
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
    fn ловит_запись_dolt() {
        assert!(is_relevant(Path::new(
            "/p/.beads/embeddeddolt/smetana/.dolt/noms/manifest"
        )));
        assert!(is_relevant(Path::new(
            "/p/.beads/embeddeddolt/smetana/.dolt/noms/journal.idx"
        )));
    }

    #[test]
    fn ловит_last_touched() {
        assert!(is_relevant(Path::new("/p/.beads/last-touched")));
    }

    #[test]
    fn игнорирует_шум() {
        assert!(!is_relevant(Path::new("/p/.beads/config.yaml")));
        assert!(!is_relevant(Path::new("/p/.beads/backup/LOCK")));
        assert!(!is_relevant(Path::new(
            "/p/.beads/embeddeddolt/smetana/.dolt/git-remote-cache/x/repo.git/config"
        )));
    }
}
