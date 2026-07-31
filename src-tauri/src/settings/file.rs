//! Диск: где лежит файл настроек, как он читается и как пишется.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use super::model::{parse, Outcome, Settings};

/// Счётчик временных файлов. Вместе с pid даёт имя, которого нет ни у одной
/// другой записи — ни в этом процессе, ни в соседнем.
static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Почему файл не прочитался. Диагностика для лога, не для интерфейса.
#[derive(Debug)]
pub enum Problem {
    Broken,
    TooNew,
    /// Файл есть, но прочитать его не вышло: не хватило прав, это каталог,
    /// сбойнул диск. От испорченного файла это отличается тем, что копировать
    /// нечего — `fs::copy` того же файла упадёт по той же причине.
    Unreadable,
}

/// Читает настройки. Отсутствие файла — не ошибка, а первый запуск.
/// Испорченный и слишком новый файл не выбрасываем: он мог быть чьей-то
/// работой, поэтому уезжает в `.bak`, а приложение стартует с умолчаний.
/// Файл, который есть, но не читается (нет прав, это каталог и т.п.), —
/// не первый запуск: копию снять нельзя, поэтому просто сообщаем о проблеме.
pub fn load(path: &Path) -> (Settings, Option<Problem>) {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return (Settings::default(), None);
        }
        Err(err) => {
            log::warn!("настройки: не удалось прочитать {}: {err}", path.display());
            return (Settings::default(), Some(Problem::Unreadable));
        }
    };
    match parse(&text) {
        Outcome::Ok(settings) => (settings, None),
        Outcome::Broken => {
            back_up(path);
            (Settings::default(), Some(Problem::Broken))
        }
        Outcome::TooNew => {
            back_up(path);
            (Settings::default(), Some(Problem::TooNew))
        }
    }
}

/// Запись атомарна: сначала соседний файл, потом переименование. Иначе обрыв
/// на середине оставил бы половину JSON, и следующий запуск потерял бы всё.
/// Содержимое сбрасывается на диск до переименования — без этого потеря
/// питания может сделать долговечным переименование, но не то, что в файле.
/// Имя временного файла своё на каждый вызов: одно общее имя две записи
/// внахлёст поделили бы, и первая переименовала бы недописанное второй.
pub fn save(path: &Path, settings: &Settings) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|err| format!("{}: {err}", dir.display()))?;
    }
    let text = serde_json::to_string_pretty(settings).map_err(|err| err.to_string())?;
    let temp = temp_path(path);
    if let Err(err) = write_all(&temp, &text) {
        // Мусор за собой убираем сами: имя уникальное, и переиспользовать
        // недописанный файл всё равно некому.
        let _ = fs::remove_file(&temp);
        return Err(format!("{}: {err}", temp.display()));
    }
    fs::rename(&temp, path).map_err(|err| {
        let _ = fs::remove_file(&temp);
        format!("{}: {err}", path.display())
    })
}

/// `settings.<pid>.<n>.tmp` рядом с целью: переименование внутри одного
/// каталога — единственное, что файловая система обещает делать атомарно.
fn temp_path(path: &Path) -> PathBuf {
    let n = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let name = format!("settings.{}.{n}.tmp", std::process::id());
    match path.parent() {
        Some(dir) => dir.join(name),
        None => PathBuf::from(name),
    }
}

fn write_all(temp: &Path, text: &str) -> std::io::Result<()> {
    let mut file = fs::File::create(temp)?;
    file.write_all(text.as_bytes())?;
    file.sync_all()
}

fn back_up(path: &Path) {
    let backup = path.with_extension("json.bak");
    if let Err(err) = fs::copy(path, &backup) {
        log::warn!("настройки: не удалось сохранить копию в {}: {err}", backup.display());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU32, Ordering};

    static COUNTER: AtomicU32 = AtomicU32::new(0);

    /// Свой каталог на каждый тест: cargo гоняет их параллельно в одном процессе.
    fn temp_dir() -> PathBuf {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("smetana-settings-{}-{n}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("каталог для теста");
        dir
    }

    #[test]
    fn a_missing_file_is_the_first_run() {
        let dir = temp_dir();

        let (settings, problem) = load(&dir.join("settings.json"));

        assert_eq!(settings, Settings::default());
        assert!(problem.is_none(), "отсутствие файла — не проблема");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_unreadable_file_is_reported_without_a_backup() {
        let dir = temp_dir();
        let path = dir.join("settings.json");
        // Каталог на месте файла — портируемый способ получить ошибку чтения,
        // отличную от NotFound, без chmod (тот под root ведёт себя иначе).
        fs::create_dir_all(&path).expect("подготовка");

        let (settings, problem) = load(&path);

        assert_eq!(settings, Settings::default());
        assert!(matches!(problem, Some(Problem::Unreadable)));
        assert!(!dir.join("settings.json.bak").exists(), "копировать нечего — файла не было");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_broken_file_is_kept_as_a_backup() {
        let dir = temp_dir();
        let path = dir.join("settings.json");
        fs::write(&path, "{не json").expect("подготовка");

        let (settings, problem) = load(&path);

        assert_eq!(settings, Settings::default());
        assert!(matches!(problem, Some(Problem::Broken)));
        assert_eq!(
            fs::read_to_string(dir.join("settings.json.bak")).expect("копия рядом"),
            "{не json"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_newer_file_is_kept_as_a_backup_too() {
        let dir = temp_dir();
        let path = dir.join("settings.json");
        fs::write(&path, r#"{"version":99,"appearance":{"theme":"light"}}"#).expect("подготовка");

        let (settings, problem) = load(&path);

        assert_eq!(settings, Settings::default());
        assert!(matches!(problem, Some(Problem::TooNew)));
        assert!(dir.join("settings.json.bak").exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn what_was_saved_is_what_is_read_back() {
        let dir = temp_dir();
        // Каталога настроек может ещё не быть — запись создаёт его сама.
        let path = dir.join("nested").join("settings.json");
        let mut settings = Settings::default();
        settings.appearance.theme = "light".into();
        settings.layout.right_collapsed = true;

        save(&path, &settings).expect("запись");
        let (read_back, problem) = load(&path);

        assert_eq!(read_back, settings);
        assert!(problem.is_none());
        // Имя временного файла теперь своё на каждый вызов, поэтому смотрим
        // не на конкретное имя, а на то, что в каталоге не осталось ни одного.
        let leftovers: Vec<_> = fs::read_dir(path.parent().expect("каталог"))
            .expect("обход каталога")
            .filter_map(|entry| entry.ok().map(|entry| entry.file_name()))
            .filter(|name| name.to_string_lossy().ends_with(".tmp"))
            .collect();
        assert!(leftovers.is_empty(), "временный файл не остаётся: {leftovers:?}");
        let _ = fs::remove_dir_all(&dir);
    }
}
