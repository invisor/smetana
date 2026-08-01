//! Диск: чтение каталогов и файлов проекта.
//!
//! Воркера здесь нет намеренно. У трекера он есть потому, что вызов bd стоит
//! около двух секунд и снимком должен владеть кто-то один; `read_dir` стоит
//! миллисекунды и состояния не держит — очередь сторожила бы то, за что никто
//! не борется. Та же причина, по которой её нет у настроек.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use super::model::{
    looks_binary, reject_traversal, sort_entries, Entry, EntryKind, FileText, FilesError, Listing,
    Stat, BINARY_SNIFF_BYTES, MAX_ENTRIES, MAX_FILE_BYTES,
};

/// Ошибка ввода-вывода в термины, которые понимает фронт.
fn io_error(path: &str, err: &std::io::Error) -> FilesError {
    match err.kind() {
        std::io::ErrorKind::NotFound => FilesError::NotFound(path.to_owned()),
        std::io::ErrorKind::PermissionDenied => FilesError::Denied(path.to_owned()),
        _ => FilesError::Io(format!("{path}: {err}")),
    }
}

/// Миллисекунды от эпохи. Файл со временем до 1970-го — не наш случай, но и
/// падать на нём незачем: ноль честнее паники.
pub fn mtime_of(meta: &fs::Metadata) -> i64 {
    meta.modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Абсолютный путь внутри корня — или отказ.
///
/// Два рубежа. Первый, `reject_traversal`, бесплатен и ловит `..` и абсолютный
/// путь. Второй — `canonicalize`: он разворачивает симлинки, и без него ссылка
/// внутри проекта, ведущая наружу, открыла бы что угодно на диске. `root`
/// канонизируется тоже: иначе на macOS `/var/...` против `/private/var/...`
/// не совпало бы никогда.
///
/// Честно про назначение: это ловушка от собственных ошибок и странных имён,
/// а не рубеж от злоумышленника — `root` присылает фронт, и он свой.
pub fn resolve_within(root: &Path, rel: &str) -> Result<PathBuf, FilesError> {
    reject_traversal(rel)?;
    let root = root.canonicalize().map_err(|err| io_error(&root.to_string_lossy(), &err))?;
    let joined = if rel.is_empty() { root.clone() } else { root.join(rel) };
    let full = joined.canonicalize().map_err(|err| io_error(rel, &err))?;
    if !full.starts_with(&root) {
        return Err(FilesError::Outside(rel.to_owned()));
    }
    Ok(full)
}

/// Путь записи относительно корня, всегда через `/`. На Windows `read_dir`
/// вернёт обратный слэш, а ключом в настройках и в карте дерева служит одна и
/// та же строка — разъезжаться ей нельзя.
fn child_path(dir: &str, name: &str) -> String {
    if dir.is_empty() {
        name.to_owned()
    } else {
        format!("{dir}/{name}")
    }
}

pub fn list_dir(root: &Path, rel: &str) -> Result<Listing, FilesError> {
    let full = resolve_within(root, rel)?;
    if !full.is_dir() {
        return Err(FilesError::NotAFile(rel.to_owned()));
    }
    let reader = fs::read_dir(&full).map_err(|err| io_error(rel, &err))?;

    let mut entries = Vec::new();
    for item in reader {
        // Запись, исчезнувшая между `read_dir` и `next`, — не повод ронять
        // весь каталог: пропускаем её и читаем дальше.
        let Ok(item) = item else { continue };
        let name = item.file_name().to_string_lossy().into_owned();
        if super::model::skip_in_tree(&name) {
            continue;
        }
        // `file_type` не ходит по симлинкам — каталогом здесь считается то,
        // что каталог само по себе; ссылку раскроет `resolve_within`, когда
        // по ней кликнут, и там же откажет, если она ведёт наружу.
        let Ok(kind) = item.file_type() else { continue };
        entries.push(Entry {
            path: child_path(rel, &name),
            name,
            kind: if kind.is_dir() { EntryKind::Dir } else { EntryKind::File },
        });
    }

    sort_entries(&mut entries);
    let truncated = entries.len().saturating_sub(MAX_ENTRIES);
    entries.truncate(MAX_ENTRIES);

    Ok(Listing { dir: rel.to_owned(), entries, truncated })
}

pub fn read_text(root: &Path, rel: &str) -> Result<FileText, FilesError> {
    let full = resolve_within(root, rel)?;
    let meta = fs::metadata(&full).map_err(|err| io_error(rel, &err))?;
    if !meta.is_file() {
        return Err(FilesError::NotAFile(rel.to_owned()));
    }
    if meta.len() > MAX_FILE_BYTES {
        return Err(FilesError::TooLarge { path: rel.to_owned(), bytes: meta.len() });
    }

    let bytes = fs::read(&full).map_err(|err| io_error(rel, &err))?;
    if looks_binary(&bytes[..bytes.len().min(BINARY_SNIFF_BYTES)]) {
        return Err(FilesError::Binary(rel.to_owned()));
    }
    let text = String::from_utf8(bytes).map_err(|_| FilesError::NotUtf8(rel.to_owned()))?;

    // Метку берём после чтения: файл, переписанный ровно между `metadata` и
    // `read`, иначе уехал бы во фронт с меткой прошлой версии, и следующая
    // запись затёрла бы чужую правку, считая её своей.
    let meta = fs::metadata(&full).map_err(|err| io_error(rel, &err))?;
    Ok(FileText { path: rel.to_owned(), text, mtime: mtime_of(&meta) })
}

/// Метки времени пачкой. Отказов здесь нет: «файла нет» — это состояние
/// вкладки, а не сбой команды, и ронять из-за него весь проход нельзя.
pub fn stat_many(root: &Path, rels: &[String]) -> Vec<Stat> {
    rels.iter()
        .map(|rel| {
            let mtime = resolve_within(root, rel)
                .ok()
                .and_then(|full| fs::metadata(full).ok())
                .map(|meta| mtime_of(&meta));
            Stat { path: rel.clone(), mtime }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// Свой каталог на каждый тест: имя несёт pid, поэтому параллельные
    /// прогоны не мешают друг другу. Тот же приём, что в `project.rs`.
    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("smetana-files-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("создать временный каталог");
        // Каноничный путь: на macOS /var — симлинк на /private/var, и без
        // этого корень и разрешённый путь никогда не совпали бы.
        dir.canonicalize().expect("канонизировать временный каталог")
    }

    #[test]
    fn каталог_читается_отсортированным_и_без_git() {
        let root = scratch("listing");
        fs::create_dir_all(root.join(".git")).unwrap();
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("README.md"), "x").unwrap();
        fs::write(root.join("app.js"), "x").unwrap();

        let listing = list_dir(&root, "").unwrap();

        let names: Vec<&str> = listing.entries.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, vec!["src", "app.js", "README.md"]);
        assert_eq!(listing.truncated, 0);
        assert_eq!(listing.entries[0].kind, EntryKind::Dir);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn вложенный_каталог_отдаёт_пути_от_корня_через_слэш() {
        let root = scratch("nested");
        fs::create_dir_all(root.join("src/components")).unwrap();
        fs::write(root.join("src/App.vue"), "x").unwrap();

        let listing = list_dir(&root, "src").unwrap();

        assert_eq!(listing.dir, "src");
        assert_eq!(listing.entries[0].path, "src/components");
        assert_eq!(listing.entries[1].path, "src/App.vue");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn слишком_длинный_каталог_обрезается_и_говорит_насколько() {
        let root = scratch("truncate");
        for i in 0..MAX_ENTRIES + 7 {
            fs::write(root.join(format!("f{i:05}.txt")), "x").unwrap();
        }

        let listing = list_dir(&root, "").unwrap();

        assert_eq!(listing.entries.len(), MAX_ENTRIES);
        assert_eq!(listing.truncated, 7, "молчаливая обрезка читалась бы как «файлов больше нет»");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn текст_читается_вместе_с_меткой_времени() {
        let root = scratch("read");
        fs::write(root.join("a.txt"), "привет\n").unwrap();

        let file = read_text(&root, "a.txt").unwrap();

        assert_eq!(file.path, "a.txt");
        assert_eq!(file.text, "привет\n");
        assert!(file.mtime > 0);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn двоичный_и_слишком_большой_файл_не_читаются() {
        let root = scratch("refuse");
        fs::write(root.join("a.bin"), [0x4d, 0x5a, 0x00, 0x90]).unwrap();
        fs::write(root.join("big.txt"), vec![b'x'; (MAX_FILE_BYTES + 1) as usize]).unwrap();
        fs::write(root.join("bad.txt"), [0xff, 0xfe, 0x41]).unwrap();

        assert!(matches!(read_text(&root, "a.bin"), Err(FilesError::Binary(_))));
        assert!(matches!(read_text(&root, "big.txt"), Err(FilesError::TooLarge { .. })));
        assert!(matches!(read_text(&root, "bad.txt"), Err(FilesError::NotUtf8(_))));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn отсутствующий_файл_и_каталог_вместо_файла_различаются() {
        let root = scratch("missing");
        fs::create_dir_all(root.join("src")).unwrap();

        assert!(matches!(read_text(&root, "nope.txt"), Err(FilesError::NotFound(_))));
        assert!(matches!(read_text(&root, "src"), Err(FilesError::NotAFile(_))));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn симлинк_наружу_не_проходит_хотя_путь_выглядит_невинно() {
        let root = scratch("escape");
        let outside = scratch("escape-target");
        fs::write(outside.join("secret.txt"), "не для чтения").unwrap();
        // `reject_traversal` тут бессилен: в пути нет ни "..", ни корня.
        #[cfg(unix)]
        std::os::unix::fs::symlink(&outside, root.join("link")).unwrap();
        #[cfg(not(unix))]
        return;

        assert!(matches!(read_text(&root, "link/secret.txt"), Err(FilesError::Outside(_))));
        let _ = fs::remove_dir_all(&root);
        let _ = fs::remove_dir_all(&outside);
    }

    #[test]
    fn метки_времени_отдаются_пачкой_и_исчезнувший_файл_виден() {
        let root = scratch("stat");
        fs::write(root.join("a.txt"), "x").unwrap();

        let stats = stat_many(&root, &["a.txt".to_string(), "gone.txt".to_string()]);

        assert_eq!(stats.len(), 2);
        assert!(stats[0].mtime.is_some());
        assert_eq!(stats[1].mtime, None, "исчезнувший файл — не ошибка, а состояние");
        let _ = fs::remove_dir_all(&root);
    }
}
