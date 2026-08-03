//! Project files: the types the front end sees, and the pure logic around them.
//!
//! No I/O here: everything that depends on the disk lives in `fs.rs`.
//! That is why this file is the one carrying the tests — same as `settings/model.rs`.

use serde::Serialize;

/// How many entries of one directory we hand over. `FileTree` is not
/// virtualized (it admits as much itself), and one click on `node_modules`
/// without a ceiling wedges the render.
pub const MAX_ENTRIES: usize = 1000;

/// The file size ceiling. A 50 MB `textarea` is a frozen window; saying "too
/// large" is more honest than showing half of it.
pub const MAX_FILE_BYTES: u64 = 2 * 1024 * 1024;

/// How many bytes we sniff for binariness.
pub const BINARY_SNIFF_BYTES: usize = 8 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum EntryKind {
    Dir,
    File,
}

/// A directory entry. `path` is relative to the project root and the separator
/// is always `/` — it is also the key in settings and in the tree map, and the
/// two must not diverge.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub name: String,
    pub path: String,
    pub kind: EntryKind,
}

/// The contents of one directory. `truncated` is how many entries did not fit;
/// zero means "all of them". Silent truncation would read as "there are no more
/// files here", so the number travels to the front end, not only into the log.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Listing {
    pub dir: String,
    pub entries: Vec<Entry>,
    pub truncated: usize,
}

/// `mtime` is milliseconds since the epoch. It is what a write returns and what
/// the front end sends back as `expectedMtime`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileText {
    pub path: String,
    pub text: String,
    pub mtime: i64,
}

/// `mtime: None` — the file is no longer where it was.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Stat {
    pub path: String,
    pub mtime: Option<i64>,
}

#[derive(Debug, thiserror::Error)]
pub enum FilesError {
    #[error("no such file: {0}")]
    NotFound(String),
    #[error("access denied: {0}")]
    Denied(String),
    #[error("not a file: {0}")]
    NotAFile(String),
    #[error("binary file: {0}")]
    Binary(String),
    #[error("file too large: {path} ({bytes} bytes)")]
    TooLarge { path: String, bytes: u64 },
    #[error("not UTF-8 text: {0}")]
    NotUtf8(String),
    #[error("path outside the project: {0}")]
    Outside(String),
    #[error("the file changed on disk: {0}")]
    Stale(String),
    #[error("{0}")]
    Io(String),
}

impl FilesError {
    /// The machine-readable form for the front end. The message text is
    /// diagnostics and speaks the filesystem's language; the decision about what
    /// to show a person is made from this field, not by parsing a string.
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

// Tauri requires a command's error to be serializable. Unlike `SettingsError`,
// one string will not do here: the front end has to tell `stale` from `binary`
// to show a different strip.
impl Serialize for FilesError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut st = s.serialize_struct("FilesError", 2)?;
        st.serialize_field("kind", self.kind())?;
        st.serialize_field("message", &self.to_string())?;
        st.end()
    }
}

/// Directories first, and inside a group by name case-insensitively. The
/// `read_dir` order depends on the filesystem and cannot be relied upon: it is
/// one thing on APFS and another on ext4, and the tree would jump between machines.
pub fn sort_entries(entries: &mut [Entry]) {
    entries.sort_by(|a, b| {
        let dirs_first = (a.kind != EntryKind::Dir).cmp(&(b.kind != EntryKind::Dir));
        dirs_first.then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
}

/// The only thing the tree does not show. Dotfiles are shown: `.beads` is the
/// directory the whole app is built around, and `node_modules` costs nothing
/// under lazy reading until it is clicked.
pub fn skip_in_tree(name: &str) -> bool {
    name == ".git"
}

/// A leading zero byte is the common probe for binariness and the only one that
/// does not get UTF-8 wrong.
pub fn looks_binary(bytes: &[u8]) -> bool {
    bytes.iter().take(BINARY_SNIFF_BYTES).any(|b| *b == 0)
}

/// A cheap first line of defence: a relative path has no business being
/// absolute or containing a `..` component. The real check (a symlink pointing
/// outside) is done by `fs::resolve_within` through `canonicalize` — but that
/// one costs a trip to the disk, while this refusal is free and covered by tests.
///
/// Both separators are cut: WebView2 is among the target webviews, and a path
/// from there may arrive with a backslash.
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
    fn directories_come_first_then_by_name_case_insensitively() {
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
    fn the_read_dir_order_must_not_leak_through() {
        // The same entries in reverse order give the same result.
        let mut a = vec![entry("b.txt", EntryKind::File), entry("a.txt", EntryKind::File)];
        let mut b = vec![entry("a.txt", EntryKind::File), entry("b.txt", EntryKind::File)];
        sort_entries(&mut a);
        sort_entries(&mut b);
        assert_eq!(a, b);
    }

    #[test]
    fn only_git_hides_in_the_tree() {
        assert!(skip_in_tree(".git"));
        assert!(!skip_in_tree(".beads"), ".beads is the heart of the app, it has to be visible");
        assert!(!skip_in_tree(".gitignore"));
        assert!(!skip_in_tree("node_modules"), "lazy reading makes it free");
        assert!(!skip_in_tree("src"));
    }

    #[test]
    fn a_file_with_a_leading_zero_byte_counts_as_binary() {
        assert!(!looks_binary(b"fn main() {}\n"));
        assert!(!looks_binary(&[]), "an empty file is legitimate text");
        assert!(looks_binary(b"MZ\x00\x90"));
    }

    #[test]
    fn a_zero_byte_past_the_probe_does_not_count() {
        let mut bytes = vec![b'a'; BINARY_SNIFF_BYTES];
        bytes.push(0);
        assert!(!looks_binary(&bytes), "we only look at the first BINARY_SNIFF_BYTES");
    }

    #[test]
    fn a_path_leading_outside_is_rejected_before_any_trip_to_the_disk() {
        assert!(reject_traversal("src/App.vue").is_ok());
        assert!(reject_traversal("").is_ok(), "an empty string is the root itself");
        assert!(matches!(reject_traversal("../secrets"), Err(FilesError::Outside(_))));
        assert!(matches!(reject_traversal("src/../../etc/passwd"), Err(FilesError::Outside(_))));
        assert!(matches!(reject_traversal("/etc/passwd"), Err(FilesError::Outside(_))));
        assert!(matches!(reject_traversal("C:\\Windows"), Err(FilesError::Outside(_))));
        assert!(
            reject_traversal("src/..hidden").is_ok(),
            "two dots inside a name are not a climb upwards"
        );
    }

    #[test]
    fn every_error_has_a_machine_readable_form() {
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
    fn an_error_travels_to_the_front_end_as_a_kind_and_text_pair() {
        let json = serde_json::to_value(FilesError::Binary("a.png".into())).unwrap();
        assert_eq!(json["kind"], "binary");
        assert!(json["message"].as_str().unwrap().contains("a.png"));
    }
}
