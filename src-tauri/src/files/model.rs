//! Файлы проекта: типы, которые видит фронт, и чистая логика вокруг них.
//!
//! Здесь нет ввода-вывода: всё, что зависит от диска, живёт в `fs.rs`.
//! Поэтому именно этот файл покрыт тестами — как `settings/model.rs`.

use serde::Serialize;

/// Сколько записей одного каталога отдаём. `FileTree` не виртуализирован (он
/// сам это признаёт), и один клик по `node_modules` без потолка вешает рендер.
pub const MAX_ENTRIES: usize = 1000;

/// Потолок размера файла. `textarea` на 50 МБ — это зависшее окно; честнее
/// сказать «слишком велик», чем показать половину.
pub const MAX_FILE_BYTES: u64 = 2 * 1024 * 1024;

/// Сколько байт нюхаем на предмет двоичности.
pub const BINARY_SNIFF_BYTES: usize = 8 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum EntryKind {
    Dir,
    File,
}

/// Запись каталога. `path` относителен корня проекта, разделитель всегда `/` —
/// он же ключ в настройках и в карте дерева, и разъезжаться им нельзя.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub name: String,
    pub path: String,
    pub kind: EntryKind,
}

/// Содержимое одного каталога. `truncated` — сколько записей не поместилось;
/// ноль значит «все». Молчаливая обрезка читалась бы как «здесь больше нет
/// файлов», поэтому число едет во фронт, а не только в лог.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Listing {
    pub dir: String,
    pub entries: Vec<Entry>,
    pub truncated: usize,
}

/// `mtime` — миллисекунды от эпохи. Именно он возвращается после записи и
/// именно его фронт присылает обратно как `expectedMtime`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileText {
    pub path: String,
    pub text: String,
    pub mtime: i64,
}

/// `mtime: None` — файла на месте больше нет.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Stat {
    pub path: String,
    pub mtime: Option<i64>,
}

#[derive(Debug, thiserror::Error)]
pub enum FilesError {
    #[error("файла нет: {0}")]
    NotFound(String),
    #[error("нет доступа: {0}")]
    Denied(String),
    #[error("это не файл: {0}")]
    NotAFile(String),
    #[error("двоичный файл: {0}")]
    Binary(String),
    #[error("файл слишком велик: {path} ({bytes} байт)")]
    TooLarge { path: String, bytes: u64 },
    #[error("не текст в UTF-8: {0}")]
    NotUtf8(String),
    #[error("путь вне проекта: {0}")]
    Outside(String),
    #[error("файл изменился на диске: {0}")]
    Stale(String),
    #[error("{0}")]
    Io(String),
}

impl FilesError {
    /// Машинный вид для фронта. Текст сообщения — диагностика и говорит
    /// языком файловой системы; решение, что показать человеку, принимается
    /// по этому полю, а не разбором строки.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::NotFound(_) => "notFound",
            Self::Denied(_) => "denied",
            Self::NotAFile(_) => "notAFile",
            Self::Binary(_) => "binary",
            Self::TooLarge { .. } => "tooLarge",
            Self::NotUtf8(_) => "notUtf8",
            Self::Outside(_) => "outside",
            Self::Stale(_) => "stale",
            Self::Io(_) => "io",
        }
    }
}

// Tauri требует, чтобы ошибка команды умела сериализоваться. В отличие от
// `SettingsError`, одной строкой тут не обойтись: фронту нужно отличать
// `stale` от `binary`, чтобы показать разную полоску.
impl Serialize for FilesError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut st = s.serialize_struct("FilesError", 2)?;
        st.serialize_field("kind", self.kind())?;
        st.serialize_field("message", &self.to_string())?;
        st.end()
    }
}

/// Каталоги первыми, внутри группы — по имени без учёта регистра. Порядок
/// `read_dir` зависит от файловой системы, и полагаться на него нельзя:
/// на APFS он один, на ext4 другой, и дерево прыгало бы между машинами.
pub fn sort_entries(entries: &mut [Entry]) {
    entries.sort_by(|a, b| {
        let dirs_first = (a.kind != EntryKind::Dir).cmp(&(b.kind != EntryKind::Dir));
        dirs_first.then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
}

/// Единственное, чего в дереве не видно. Точечные файлы показываем: `.beads` —
/// каталог, вокруг которого построено приложение, а `node_modules` при ленивом
/// чтении не стоит ничего, пока по нему не кликнули.
pub fn skip_in_tree(name: &str) -> bool {
    name == ".git"
}

/// Нулевой байт в начале — общепринятая проба на двоичность и единственная,
/// которая не ошибается на UTF-8.
pub fn looks_binary(bytes: &[u8]) -> bool {
    bytes.iter().take(BINARY_SNIFF_BYTES).any(|b| *b == 0)
}

/// Дешёвый первый рубеж: относительный путь не имеет права быть абсолютным и
/// содержать компонент `..`. Настоящую проверку (симлинк, указывающий наружу)
/// делает `fs::resolve_within` через `canonicalize` — но она стоит обращения к
/// диску, а этот отказ бесплатен и покрыт тестами.
///
/// Разделители режем оба: среди целевых вебвью есть WebView2, и путь оттуда
/// может прийти с обратным слэшем.
pub fn reject_traversal(rel: &str) -> Result<(), FilesError> {
    let looks_absolute = rel.starts_with('/')
        || rel.starts_with('\\')
        || rel.chars().nth(1) == Some(':');
    let climbs = rel.split(['/', '\\']).any(|part| part == "..");
    if looks_absolute || climbs {
        return Err(FilesError::Outside(rel.to_owned()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(name: &str, kind: EntryKind) -> Entry {
        Entry { name: name.into(), path: name.into(), kind }
    }

    #[test]
    fn каталоги_идут_первыми_потом_по_имени_без_учёта_регистра() {
        let mut list = vec![
            entry("README.md", EntryKind::File),
            entry("src", EntryKind::Dir),
            entry("Cargo.toml", EntryKind::File),
            entry(".beads", EntryKind::Dir),
            entry("app.js", EntryKind::File),
        ];

        sort_entries(&mut list);

        let names: Vec<&str> = list.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, vec![".beads", "src", "app.js", "Cargo.toml", "README.md"]);
    }

    #[test]
    fn порядок_read_dir_не_должен_просачиваться() {
        // Одни и те же записи в обратном порядке дают тот же результат.
        let mut a = vec![entry("b.txt", EntryKind::File), entry("a.txt", EntryKind::File)];
        let mut b = vec![entry("a.txt", EntryKind::File), entry("b.txt", EntryKind::File)];
        sort_entries(&mut a);
        sort_entries(&mut b);
        assert_eq!(a, b);
    }

    #[test]
    fn в_дереве_прячется_только_git() {
        assert!(skip_in_tree(".git"));
        assert!(!skip_in_tree(".beads"), ".beads — сердце приложения, он обязан быть виден");
        assert!(!skip_in_tree(".gitignore"));
        assert!(!skip_in_tree("node_modules"), "ленивое чтение делает его бесплатным");
        assert!(!skip_in_tree("src"));
    }

    #[test]
    fn двоичным_считается_файл_с_нулевым_байтом_в_начале() {
        assert!(!looks_binary(b"fn main() {}\n"));
        assert!(!looks_binary(&[]), "пустой файл — законный текст");
        assert!(looks_binary(b"MZ\x00\x90"));
    }

    #[test]
    fn нулевой_байт_за_пределами_пробы_не_считается() {
        let mut bytes = vec![b'a'; BINARY_SNIFF_BYTES];
        bytes.push(0);
        assert!(!looks_binary(&bytes), "смотрим только первые BINARY_SNIFF_BYTES");
    }

    #[test]
    fn путь_наружу_отвергается_до_всякого_обращения_к_диску() {
        assert!(reject_traversal("src/App.vue").is_ok());
        assert!(reject_traversal("").is_ok(), "пустая строка — это сам корень");
        assert!(matches!(reject_traversal("../secrets"), Err(FilesError::Outside(_))));
        assert!(matches!(reject_traversal("src/../../etc/passwd"), Err(FilesError::Outside(_))));
        assert!(matches!(reject_traversal("/etc/passwd"), Err(FilesError::Outside(_))));
        assert!(matches!(reject_traversal("C:\\Windows"), Err(FilesError::Outside(_))));
        assert!(
            reject_traversal("src/..hidden").is_ok(),
            "две точки внутри имени — не выход наверх"
        );
    }

    #[test]
    fn у_каждой_ошибки_есть_машинный_вид() {
        assert_eq!(FilesError::NotFound("a".into()).kind(), "notFound");
        assert_eq!(FilesError::Denied("a".into()).kind(), "denied");
        assert_eq!(FilesError::NotAFile("a".into()).kind(), "notAFile");
        assert_eq!(FilesError::Binary("a".into()).kind(), "binary");
        assert_eq!(FilesError::TooLarge { path: "a".into(), bytes: 9 }.kind(), "tooLarge");
        assert_eq!(FilesError::NotUtf8("a".into()).kind(), "notUtf8");
        assert_eq!(FilesError::Outside("a".into()).kind(), "outside");
        assert_eq!(FilesError::Stale("a".into()).kind(), "stale");
        assert_eq!(FilesError::Io("a".into()).kind(), "io");
    }

    #[test]
    fn ошибка_едет_во_фронт_парой_вид_и_текст() {
        let json = serde_json::to_value(FilesError::Binary("a.png".into())).unwrap();
        assert_eq!(json["kind"], "binary");
        assert!(json["message"].as_str().unwrap().contains("a.png"));
    }
}
