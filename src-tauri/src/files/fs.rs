//! Диск: чтение каталогов и файлов проекта.
//!
//! Воркера здесь нет намеренно. У трекера он есть потому, что вызов bd стоит
//! около двух секунд и снимком должен владеть кто-то один; `read_dir` стоит
//! миллисекунды и состояния не держит — очередь сторожила бы то, за что никто
//! не борется. Та же причина, по которой её нет у настроек.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
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
    read_text_reading_with(root, rel, |full| fs::read(full))
}

/// Тело `read_text` с подменяемым чтением байтов.
///
/// Подмена существует ради одного теста, и другого способа его написать нет:
/// порядок «сначала метка, потом байты» виден только тому, кто успевает
/// переписать файл ровно между этими двумя шагами, а из теста в этот
/// промежуток не попасть ничем, кроме гонки. Замыкание и есть этот промежуток.
fn read_text_reading_with(
    root: &Path,
    rel: &str,
    read_bytes: impl FnOnce(&Path) -> std::io::Result<Vec<u8>>,
) -> Result<FileText, FilesError> {
    let full = resolve_within(root, rel)?;
    let meta = fs::metadata(&full).map_err(|err| io_error(rel, &err))?;
    if !meta.is_file() {
        return Err(FilesError::NotAFile(rel.to_owned()));
    }
    if meta.len() > MAX_FILE_BYTES {
        return Err(FilesError::TooLarge { path: rel.to_owned(), bytes: meta.len() });
    }
    // Метку берём ДО чтения байтов, и второй раз её не снимаем.
    //
    // Атомарно прочитать содержимое вместе с меткой нельзя, а между двумя
    // вызовами файл могут переписать — значит, выбирать приходится не между
    // «верно» и «неверно», а между двумя способами ошибиться:
    //
    //   метка до чтения  — во фронт уедет новое содержимое со старой меткой,
    //                      и следующая запись отказом `Stale` спросит человека;
    //   метка после      — уедет старое содержимое с новой меткой, и следующая
    //                      запись пройдёт сверку и молча затрёт чужую правку.
    //
    // Ошибаться мы обязаны в сторону ложного отказа: он стоит одного вопроса,
    // а молчаливая перезапись стоит чужой работы. Сверка `expected_mtime` в
    // `write_text` существует ровно ради этого, и порядок «метка после» лишал
    // бы её смысла.
    let mtime = mtime_of(&meta);

    let bytes = read_bytes(&full).map_err(|err| io_error(rel, &err))?;
    if looks_binary(&bytes[..bytes.len().min(BINARY_SNIFF_BYTES)]) {
        return Err(FilesError::Binary(rel.to_owned()));
    }
    let text = String::from_utf8(bytes).map_err(|_| FilesError::NotUtf8(rel.to_owned()))?;

    Ok(FileText { path: rel.to_owned(), text, mtime })
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

/// Счётчик временных файлов. Вместе с pid даёт имя, которого нет ни у одной
/// другой записи — ни в этом процессе, ни в соседнем. Тот же приём, что в
/// `settings/file.rs`.
static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

fn temp_path(path: &Path) -> PathBuf {
    let n = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    path.with_file_name(format!(".{name}.{}.{n}.tmp", std::process::id()))
}

/// Запись файла проекта.
///
/// Сверка `expected_mtime` — единственное, ради чего вся эта возня: без неё
/// Cmd+S по вкладке, открытой час назад, молча стёр бы работу агента.
/// Расхождение означает отказ и ноль изменений на диске.
///
/// Дальше — как в `settings/file.rs`: временный файл рядом, `sync_all`,
/// `rename`. Плюс одно, чего там не нужно: перенос прав с оригинала.
/// `rename` подменяет файл целиком, и без этого исполняемый скрипт после
/// сохранения перестал бы запускаться.
pub fn write_text(
    root: &Path,
    rel: &str,
    text: &str,
    expected_mtime: i64,
) -> Result<i64, FilesError> {
    let full = resolve_within(root, rel)?;
    let meta = fs::metadata(&full).map_err(|err| io_error(rel, &err))?;
    if !meta.is_file() {
        return Err(FilesError::NotAFile(rel.to_owned()));
    }
    if mtime_of(&meta) != expected_mtime {
        return Err(FilesError::Stale(rel.to_owned()));
    }

    let temp = temp_path(&full);
    let written = (|| -> std::io::Result<()> {
        let mut file = fs::File::create(&temp)?;
        file.write_all(text.as_bytes())?;
        // Без этого потеря питания может сделать долговечным переименование,
        // но не то, что в файле.
        file.sync_all()
    })();
    if let Err(err) = written {
        let _ = fs::remove_file(&temp);
        return Err(FilesError::Io(format!("{}: {err}", temp.display())));
    }
    if let Err(err) = fs::set_permissions(&temp, meta.permissions()) {
        let _ = fs::remove_file(&temp);
        return Err(FilesError::Io(format!("{}: {err}", temp.display())));
    }
    if let Err(err) = fs::rename(&temp, &full) {
        let _ = fs::remove_file(&temp);
        return Err(io_error(rel, &err));
    }

    let meta = fs::metadata(&full).map_err(|err| io_error(rel, &err))?;
    Ok(mtime_of(&meta))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// Свой каталог на каждый тест: имя несёт pid, поэтому параллельные
    /// прогоны не мешают друг другу. Тот же приём, что в `project.rs`.
    /// Явная метка времени вместо паузы. Разрешение `mtime` на некоторых
    /// файловых системах грубее, чем расстояние между двумя записями подряд, и
    /// тест «метка изменилась» без этого бывает ложно-зелёным. `sleep` дал бы
    /// то же самое, но замедлял бы весь прогон и всё равно зависел бы от
    /// разрешения; выставленная метка не зависит ни от того, ни от другого.
    fn set_mtime(path: &Path, secs: u64) {
        let file = fs::File::options().write(true).open(path).expect("открыть файл ради метки");
        file.set_modified(UNIX_EPOCH + std::time::Duration::from_secs(secs))
            .expect("выставить метку времени");
    }

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

    /// Держит порядок из `read_text`: метка снимается с файла до чтения байтов
    /// и наружу уходит именно она. Стоит вернуть снятие метки после чтения —
    /// и сверка в `write_text` перестанет защищать: во фронт уедет содержимое
    /// одной версии с меткой другой, и следующая запись пройдёт молча.
    #[test]
    fn чужая_правка_после_чтения_отказывает_записи() {
        let root = scratch("read-then-clobber");
        let path = root.join("a.txt");
        fs::write(&path, "работа агента\n").unwrap();
        set_mtime(&path, 1_700_000_000);

        let file = read_text(&root, "a.txt").unwrap();

        assert_eq!(file.text, "работа агента\n");
        assert_eq!(file.mtime, 1_700_000_000_000, "наружу уходит метка прочитанного файла");

        // Так выглядит агент, переписавший файл, пока вкладка была открыта.
        fs::write(&path, "новая работа агента\n").unwrap();
        set_mtime(&path, 1_700_000_060);

        let err = write_text(&root, "a.txt", "мои правки\n", file.mtime);

        assert!(
            matches!(err, Err(FilesError::Stale(_))),
            "запись по метке из read_text обязана отказать, а не затирать чужое: {err:?}"
        );
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            "новая работа агента\n",
            "при отказе на диске не должно измениться ничего"
        );
        let _ = fs::remove_dir_all(&root);
    }

    /// Тот же случай, но пойманный в его настоящий момент: файл переписывают
    /// ровно между снятием метки и чтением байтов. Метка, снятая ДО чтения,
    /// уезжает старой — и следующая запись отказывает `Stale`, то есть
    /// спрашивает человека. Метка, снятая ПОСЛЕ, уехала бы новой, сверка в
    /// `write_text` прошла бы, и работа агента исчезла бы молча. Этот тест
    /// падает ровно на такой перестановке.
    #[test]
    fn метка_снимается_до_чтения_и_потому_не_обгоняет_содержимое() {
        let root = scratch("mtime-before-read");
        let path = root.join("a.txt");
        fs::write(&path, "моё\n").unwrap();
        set_mtime(&path, 1_700_000_000);

        let file = read_text_reading_with(&root, "a.txt", |full| {
            let bytes = fs::read(full)?;
            // Агент переписал файл, пока мы читали его байты.
            fs::write(full, "работа агента\n")?;
            set_mtime(full, 1_700_000_060);
            Ok(bytes)
        })
        .unwrap();

        assert_eq!(file.text, "моё\n");
        assert_eq!(
            file.mtime, 1_700_000_000_000,
            "наружу обязана уйти метка прочитанной версии, а не той, что легла после"
        );

        let err = write_text(&root, "a.txt", "мои правки\n", file.mtime);

        assert!(
            matches!(err, Err(FilesError::Stale(_))),
            "ошибаться надо в сторону ложного отказа, а не молчаливой перезаписи: {err:?}"
        );
        assert_eq!(fs::read_to_string(&path).unwrap(), "работа агента\n");
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

    // Симлинки создаются по-разному, и на не-unix этого теста просто нет —
    // как у соседей ниже. Раньше здесь стоял `#[cfg(not(unix))] return;`
    // посреди тела, и весь остаток теста был недостижим для компилятора.
    #[cfg(unix)]
    #[test]
    fn симлинк_наружу_не_проходит_хотя_путь_выглядит_невинно() {
        let root = scratch("escape");
        let outside = scratch("escape-target");
        fs::write(outside.join("secret.txt"), "не для чтения").unwrap();
        // `reject_traversal` тут бессилен: в пути нет ни "..", ни корня.
        std::os::unix::fs::symlink(&outside, root.join("link")).unwrap();

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

    #[test]
    fn запись_возвращает_новую_метку_и_меняет_файл() {
        let root = scratch("write");
        fs::write(root.join("a.txt"), "было\n").unwrap();
        let before = read_text(&root, "a.txt").unwrap();

        let after = write_text(&root, "a.txt", "стало\n", before.mtime).unwrap();

        assert_eq!(fs::read_to_string(root.join("a.txt")).unwrap(), "стало\n");
        assert!(after >= before.mtime);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn чужая_запись_не_затирается() {
        let root = scratch("stale");
        fs::write(root.join("a.txt"), "моё\n").unwrap();
        let mine = read_text(&root, "a.txt").unwrap();

        // Так выглядит агент, переписавший файл, пока вкладка была открыта.
        let err = write_text(&root, "a.txt", "мои правки\n", mine.mtime - 1);

        assert!(matches!(err, Err(FilesError::Stale(_))));
        assert_eq!(
            fs::read_to_string(root.join("a.txt")).unwrap(),
            "моё\n",
            "при отказе на диске не должно измениться ничего"
        );

        // Проверка, что Stale отказ случился ДО создания temp файла.
        // Если кто-нибудь переставит сверку mtime за File::create, этот тест упадёт.
        let leftovers: Vec<_> = fs::read_dir(&root)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|name| name.ends_with(".tmp"))
            .collect();
        assert!(leftovers.is_empty(), "Stale отказ должен случиться ДО создания temp файла: {leftovers:?}");

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn запись_наружу_отвергается() {
        let root = scratch("write-outside");
        assert!(matches!(
            write_text(&root, "../evil.txt", "x", 0),
            Err(FilesError::Outside(_))
        ));
        let _ = fs::remove_dir_all(&root);
    }

    #[cfg(unix)]
    #[test]
    fn права_исполняемого_файла_переживают_запись() {
        use std::os::unix::fs::PermissionsExt;
        let root = scratch("perms");
        let path = root.join("run.sh");
        fs::write(&path, "#!/bin/sh\necho было\n").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        let before = read_text(&root, "run.sh").unwrap();

        write_text(&root, "run.sh", "#!/bin/sh\necho стало\n", before.mtime).unwrap();

        let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o755, "rename подменил бы режим режимом временного файла");
        let _ = fs::remove_dir_all(&root);
    }

    #[cfg(unix)]
    #[test]
    fn временный_файл_за_собой_не_остаётся() {
        let root = scratch("no-litter");
        fs::write(root.join("a.txt"), "x\n").unwrap();
        let before = read_text(&root, "a.txt").unwrap();

        write_text(&root, "a.txt", "y\n", before.mtime).unwrap();

        let leftovers: Vec<_> = fs::read_dir(&root)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|name| name.ends_with(".tmp"))
            .collect();
        assert!(leftovers.is_empty(), "остались временные файлы: {leftovers:?}");
        let _ = fs::remove_dir_all(&root);
    }

    #[cfg(unix)]
    #[test]
    fn недостаток_прав_блокирует_запись_в_каталог() {
        use std::os::unix::fs::PermissionsExt;

        let root = scratch("deny-write");
        let subdir = root.join("sub");
        fs::create_dir_all(&subdir).unwrap();
        fs::write(subdir.join("a.txt"), "x\n").unwrap();

        let before = read_text(&root, "sub/a.txt").unwrap();

        // Проверка что права действительно заблокируют операцию.
        // Если мы под root, права игнорируются и тест молча пройдёт.
        let test_file = subdir.join(".test");
        if fs::write(&test_file, "test").is_ok() {
            let _ = fs::remove_file(&test_file);
            let _ = fs::remove_dir_all(&root);
            return; // Под root права не работают, не тестируем.
        }

        // Снять право записи с подкаталога.
        fs::set_permissions(&subdir, fs::Permissions::from_mode(0o555)).unwrap();

        let err = write_text(&root, "sub/a.txt", "y\n", before.mtime);

        // Вернуть права СРАЗУ, до уборки — иначе remove_dir_all не сможет удалить каталог.
        fs::set_permissions(&subdir, fs::Permissions::from_mode(0o755)).unwrap();

        // Проверить отказ — write_text должна вернуть ошибку при недостатке прав.
        assert!(err.is_err(), "write_text должна вернуть ошибку при недостатке прав на запись");

        // Оригинальный файл не должен измениться.
        assert_eq!(
            fs::read_to_string(subdir.join("a.txt")).unwrap(),
            "x\n",
            "при отказе оригинальный файл должен остаться неизменным"
        );

        let _ = fs::remove_dir_all(&root);
    }
}
