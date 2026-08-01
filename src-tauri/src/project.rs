//! Где искать трекер.
//!
//! Общий словарь трекера и настроек. Раньше поиск каталога жил в
//! `tracker::service`, и настройки ходили за ним туда — зависимость, которую
//! их собственный комментарий называл подпоркой. Теперь оба зависят отсюда, а
//! не друг от друга.

use std::path::{Path, PathBuf};

/// В каталоге есть трекер, если в нём лежит каталог `.beads`. Файл с таким
/// именем трекером не делает.
pub fn has_tracker(dir: &Path) -> bool {
    dir.join(".beads").is_dir()
}

/// Ближайший предок, в котором есть трекер. Сам каталог тоже предок.
pub fn nearest_tracked_ancestor(start: &Path) -> Option<PathBuf> {
    start.ancestors().find(|dir| has_tracker(dir)).map(Path::to_path_buf)
}

/// Чем открыться в самый первый раз, когда список проектов ещё пуст.
///
/// Просто `current_dir` не годится ни в одном настоящем запуске: под
/// `npm run tauri dev` бинарник стартует из `src-tauri/`, а собранное
/// macOS-приложение, открытое из Finder, — вообще из `/`. Поэтому идём вверх
/// по предкам.
///
/// Не нашлось ничего — проекта нет, и это не беда: список пуст, человек
/// выберет каталог сам. Раньше здесь возвращался рабочий каталог, и
/// приложение говорило «здесь нет .beads» про каталог, который никто не
/// выбирал.
pub fn default_project() -> Option<PathBuf> {
    nearest_tracked_ancestor(&std::env::current_dir().ok()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// Свой каталог на каждый тест: имя несёт pid, поэтому параллельные
    /// прогоны не мешают друг другу.
    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("smetana-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("создать временный каталог");
        dir
    }

    #[test]
    fn каталог_с_beads_отслеживается() {
        let root = scratch("has-tracker");
        assert!(!has_tracker(&root), "пустой каталог трекером не считается");
        fs::create_dir_all(root.join(".beads")).unwrap();
        assert!(has_tracker(&root));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn файл_с_именем_beads_не_считается_трекером() {
        let root = scratch("beads-file");
        fs::write(root.join(".beads"), "не каталог").unwrap();
        assert!(!has_tracker(&root));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn поиск_поднимается_до_ближайшего_предка() {
        let root = scratch("ancestor");
        let deep = root.join("a/b/c");
        fs::create_dir_all(&deep).unwrap();
        fs::create_dir_all(root.join(".beads")).unwrap();
        assert_eq!(nearest_tracked_ancestor(&deep).as_deref(), Some(root.as_path()));
        assert_eq!(nearest_tracked_ancestor(&root).as_deref(), Some(root.as_path()), "сам каталог тоже предок");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn без_beads_нигде_выше_поиск_ничего_не_находит() {
        let root = scratch("nothing");
        let deep = root.join("x/y");
        fs::create_dir_all(&deep).unwrap();
        assert_eq!(nearest_tracked_ancestor(&deep), None, "во временном каталоге и над ним трекера быть не должно");
        let _ = fs::remove_dir_all(&root);
    }
}
