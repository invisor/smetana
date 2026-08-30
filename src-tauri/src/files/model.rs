//! Project files: the types the front end sees, and the pure logic around them.
//!
//! No I/O here: everything that depends on the disk lives in `fs.rs`.
//! That is why this file is the one carrying the tests — same as `settings/model.rs`.

use std::collections::HashSet;
use std::path::{Component, Path};

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

/// How many entries one copy may carry, and how many bytes. The module already
/// keeps two ceilings of this kind — 1000 entries per listing, 2 MB per file —
/// and this is the third for the same reason: there is no progress bar in this
/// app, no cancel button and no watcher over the tree, so a copy of
/// `node_modules` is a frozen panel for an unknown number of minutes with
/// nothing on screen to say what is happening or how to stop it. Counted by
/// walking the metadata before a single byte is written, so the refusal arrives
/// with nothing half-done behind it.
///
/// "Copy everything, the way VS Code does" was considered and turned down: VS
/// Code can afford it because it has the progress and the cancel this does not.
pub const MAX_COPY_ENTRIES: usize = 10_000;
pub const MAX_COPY_BYTES: u64 = 1024 * 1024 * 1024;

/// How many names a copy tries before giving up. A hundred `report copy N.md`
/// in one folder is somebody's script gone wrong, not somebody's work, and
/// trying forever would be a loop with the disk in it.
pub const MAX_COPY_NAMES: usize = 100;

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
    /// Whether git ignores this entry — the tree's only reason to draw a row
    /// muted, and the one field here that is not read off the directory.
    /// `false` is what a folder outside any repository answers with, and it is
    /// an answer rather than a failure: there is no `.gitignore` above such a
    /// folder with any say over its children.
    pub ignored: bool,
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
    #[error("something is already there: {0}")]
    AlreadyExists(String),
    #[error("a name that cannot be used: {0:?}")]
    BadName(String),
    #[error("a folder cannot be put inside itself: {0}")]
    IntoSelf(String),
    #[error("too much to copy at once: {path} ({entries} entries, {bytes} bytes)")]
    TooBig { path: String, entries: usize, bytes: u64 },
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
            Self::AlreadyExists(_) => "alreadyExists",
            Self::BadName(_) => "badName",
            Self::IntoSelf(_) => "intoSelf",
            Self::TooBig { .. } => "tooBig",
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

/// The names out of a NUL-separated answer from git.
///
/// `git check-ignore -z` echoes back the pathnames it matched, each terminated
/// by a NUL, so the split always ends in an empty piece — and a run that matched
/// nothing writes nothing at all. Both come out as the empty set, which is also
/// what the caller substitutes for a directory git could not be asked about, so
/// there is one shape of answer here and not three.
pub fn ignored_names(answer: &str) -> HashSet<String> {
    answer.split('\0').filter(|name| !name.is_empty()).map(str::to_owned).collect()
}

/// Lay that set over a listing, by **name**: the question is asked with the
/// working directory set to the folder being read, so what git echoes back is
/// the bare names it was given rather than paths of any kind.
///
/// Assignment and not an `if`, because this is an overlay of one answer and not
/// an accumulation of several — a name git did not return is a name git does not
/// ignore, and saying so is the whole of what the front end draws from.
pub fn mark_ignored(entries: &mut [Entry], ignored: &HashSet<String>) {
    for entry in entries.iter_mut() {
        entry.ignored = ignored.contains(&entry.name);
    }
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

/// The name of an entry about to be made or deleted, judged before anything
/// reaches the disk. Everything downstream of it joins this string onto a
/// directory that has already been checked, so the one thing it has to
/// guarantee is that the result is a **child of that directory** — which makes
/// it a containment check and not a matter of taste.
///
/// The whole of the *containment* half is the last clause: `Path::components`
/// must yield exactly one `Normal`. That refuses `.` and `..`, which name
/// directories that already exist; it refuses a root; and on Windows it refuses
/// a drive prefix, which is the shape that costs something — `Path::join`
/// follows `PathBuf::push`, where a prefixed path *replaces* the receiver, so
/// `C:evil.txt` joined to a folder inside the project is `C:evil.txt` and
/// nothing about the project is left in it.
///
/// Four clauses stand in front of it and none is spare. Two are the cases
/// `components` cannot see: a backslash is not a separator on unix and would
/// pass as part of a name, and a drive prefix is not a prefix on unix either —
/// both are cut by hand, on every platform, the way `reject_traversal` a few
/// lines above cuts the same two shapes for the same reason. The third is the
/// trim, and it is the only thing here that refuses a name of nothing but
/// spaces: `Path::new("   ")` is one perfectly ordinary `Normal` component, so
/// the last clause takes `""` — which has no components at all — and never
/// `"   "`. The fourth, the `/`, the last clause would catch on its own; it
/// stays as the plainest statement of the rule this function exists for.
///
/// A name is also not a path in the plainer sense: `a/b.js` typed into the
/// draft row is two levels of intent, and making the intermediate directory is
/// deliberately not offered.
///
/// The front end checks a name of its own before it calls at all
/// (`components/files/newEntry.js`), and the two sets **overlap rather than
/// nest** — neither is the other's subset, and the pair is not a rule written
/// twice. The field trims first, so ` .. ` never leaves it, where this one
/// takes the string as it was sent and would accept that as an ordinary name;
/// and this one refuses `C:evil.txt`, which the field passes without a word.
/// The safety is not in either being stricter, then: it is in this one being
/// **last**. The field's job is to save a hopeless name a trip across the IPC
/// and to say so in words the person can act on; the guarantee that nothing
/// lands outside the folder it was asked for is here and only here.
///
/// One thing deliberately left to the platform: Windows reserves `con`, `nul`,
/// `aux`, `prn` and `com1`, which pass here and are left to fail at the call
/// that makes the thing — `create_new` for a file, `fs::create_dir` for a
/// folder — with whatever the OS says. That is a refusal either way and not a
/// way out of the directory, so it is not this function's to answer.
pub fn reject_bad_name(name: &str) -> Result<(), FilesError> {
    let one_plain_component = {
        let mut parts = Path::new(name).components();
        matches!(parts.next(), Some(Component::Normal(_))) && parts.next().is_none()
    };
    let bad = name.trim().is_empty()
        || name.contains('/')
        || name.contains('\\')
        || name.chars().nth(1) == Some(':')
        || !one_plain_component;
    if bad {
        return Err(FilesError::BadName(name.to_owned()));
    }
    Ok(())
}

/// The names a copy may take, in the order they are to be tried:
/// `report copy.md`, then `report copy 2.md`, and so on to
/// `MAX_COPY_NAMES`. VS Code's rule, and it is a rule about a string, which is
/// what puts it here rather than in `fs.rs`.
///
/// Where the extension starts is the whole of the difficulty, and three cases
/// settle it. The stem is everything up to the **last** dot, so
/// `archive.tar.gz` becomes `archive.tar copy.gz` and not `archive copy.tar.gz`
/// — the compound suffix is not a thing this can know about. A **leading** dot
/// is part of the name and not a separator, so `.gitignore` becomes
/// `.gitignore copy` rather than ` copy.gitignore`. And a directory has no
/// extension at all whatever dots are in it: `some.dir` becomes
/// `some.dir copy`, because `some copy.dir` would rename the folder.
///
/// Nothing here is ever overwritten and nothing is ever asked. That was decided
/// in place of a "replace or keep both" dialog: a copy is a gesture somebody
/// makes in passing, and a modal over it is a question they did not come to
/// answer.
pub fn copy_candidates(name: &str, is_dir: bool) -> Vec<String> {
    let (stem, ext) = split_extension(name, is_dir);
    (1..=MAX_COPY_NAMES)
        .map(|n| {
            if n == 1 {
                format!("{stem} copy{ext}")
            } else {
                format!("{stem} copy {n}{ext}")
            }
        })
        .collect()
}

/// The stem and the extension — the extension carrying its own dot, so the two
/// halves join back into the original with nothing in between.
fn split_extension(name: &str, is_dir: bool) -> (&str, &str) {
    if is_dir {
        return (name, "");
    }
    match name.rfind('.') {
        Some(at) if at > 0 => name.split_at(at),
        // No dot at all, or a leading one and nothing else: the name is all stem.
        _ => (name, ""),
    }
}

/// Whether `child` is `parent` or lies under it. What a folder may not be
/// copied or moved into.
///
/// `Path::starts_with` compares **components** and not characters, which is the
/// only reason this is one line: `/p/srcx` does not start with `/p/src` the way
/// the two strings do. The other half of the rule cannot be seen from here and
/// belongs to the caller — both paths have to be canonicalized first, because a
/// relative string does not see a symlink, and the folder dropped into may be a
/// link back into the folder being dragged.
///
/// A folder counts as inside itself, deliberately: copying a folder into itself
/// is the same refusal as copying it into its own child, and saying so once is
/// what stops the caller writing the equality case out by hand.
pub fn is_inside(parent: &Path, child: &Path) -> bool {
    child.starts_with(parent)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(name: &str, kind: EntryKind) -> Entry {
        Entry { name: name.into(), path: name.into(), kind, ignored: false }
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
    fn an_answer_with_nothing_in_it_names_nothing() {
        assert!(ignored_names("").is_empty(), "git matched nothing and wrote nothing");
        assert!(ignored_names("\0").is_empty(), "and a lone terminator is still nothing");
    }

    #[test]
    fn one_matched_name_comes_back_as_one() {
        let named = ignored_names("node_modules\0");
        assert_eq!(named.len(), 1);
        assert!(named.contains("node_modules"));
    }

    #[test]
    fn several_matched_names_come_back_as_several() {
        let named = ignored_names("node_modules\0dist\0.DS_Store\0");
        assert_eq!(named.len(), 3);
        assert!(named.contains("node_modules"));
        assert!(named.contains("dist"));
        assert!(named.contains(".DS_Store"));
    }

    #[test]
    fn a_name_holding_a_space_survives_the_split() {
        let named = ignored_names("my notes.log\0");
        assert!(named.contains("my notes.log"), "only the NUL separates, so a space is a name");
    }

    #[test]
    fn the_answer_is_laid_over_the_listing_by_name() {
        let mut list = vec![
            entry("node_modules", EntryKind::Dir),
            entry("src", EntryKind::Dir),
            entry("dist", EntryKind::Dir),
            entry("package.json", EntryKind::File),
        ];

        mark_ignored(&mut list, &ignored_names("node_modules\0dist\0"));

        let muted: Vec<&str> =
            list.iter().filter(|e| e.ignored).map(|e| e.name.as_str()).collect();
        assert_eq!(muted, vec!["node_modules", "dist"]);
    }

    #[test]
    fn a_name_git_did_not_return_is_left_unmarked() {
        let mut list = vec![entry("src", EntryKind::Dir), entry("README.md", EntryKind::File)];

        mark_ignored(&mut list, &ignored_names("node_modules\0"));

        assert!(list.iter().all(|e| !e.ignored), "an answer about another folder marks nothing");
    }

    /// The empty set is what a folder outside any repository answers with, and
    /// the tree draws exactly what it draws today for it.
    #[test]
    fn an_empty_set_leaves_the_whole_listing_at_full_strength() {
        let mut list = vec![entry("node_modules", EntryKind::Dir), entry("src", EntryKind::Dir)];

        mark_ignored(&mut list, &HashSet::new());

        assert!(list.iter().all(|e| !e.ignored));
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
        assert_eq!(FilesError::AlreadyExists("a".into()).kind(), "alreadyExists");
        assert_eq!(FilesError::BadName("a".into()).kind(), "badName");
        assert_eq!(FilesError::IntoSelf("a".into()).kind(), "intoSelf");
        assert_eq!(
            FilesError::TooBig { path: "a".into(), entries: 9, bytes: 9 }.kind(),
            "tooBig"
        );
        assert_eq!(FilesError::Io("a".into()).kind(), "io");
    }

    #[test]
    fn a_name_is_a_name_and_never_a_path() {
        assert!(reject_bad_name("main.rs").is_ok());
        assert!(reject_bad_name(".gitignore").is_ok(), "a dotfile is an ordinary name");
        assert!(reject_bad_name("..hidden").is_ok(), "two dots inside a name are not the parent");
        assert!(matches!(reject_bad_name("a/b.js"), Err(FilesError::BadName(_))));
        assert!(matches!(reject_bad_name("a\\b.js"), Err(FilesError::BadName(_))));
        assert!(matches!(reject_bad_name("."), Err(FilesError::BadName(_))));
        assert!(matches!(reject_bad_name(".."), Err(FilesError::BadName(_))));
        assert!(matches!(reject_bad_name(""), Err(FilesError::BadName(_))));
        assert!(
            matches!(reject_bad_name("   "), Err(FilesError::BadName(_))),
            "spaces alone are an empty name with nothing on screen to say so"
        );
    }

    /// Checked on every platform and not under a `cfg`, because the machine
    /// this runs on is not the machine the app ships to. `Path::join` follows
    /// `PathBuf::push`, where a path carrying a prefix replaces the receiver
    /// outright — so on Windows a name spelled like this would put the new file
    /// outside the project altogether, and a test that only runs on unix is a
    /// test that never sees it.
    #[test]
    fn a_name_that_is_a_windows_drive_is_refused_wherever_this_is_compiled() {
        assert!(matches!(reject_bad_name("C:evil.txt"), Err(FilesError::BadName(_))));
        assert!(matches!(reject_bad_name("C:\\Windows\\evil.txt"), Err(FilesError::BadName(_))));
        assert!(matches!(reject_bad_name("C:/Windows/evil.txt"), Err(FilesError::BadName(_))));
        assert!(matches!(reject_bad_name("\\\\server\\share"), Err(FilesError::BadName(_))));
    }

    /// The clause the other three lean on: whatever the platform makes of the
    /// string, it has to come out as one ordinary component. `.` and `..` are
    /// covered here rather than by name, which is what makes the check hold for
    /// spellings nobody thought to write down.
    #[test]
    fn a_name_has_to_be_exactly_one_ordinary_component() {
        assert!(reject_bad_name("main.rs").is_ok());
        assert!(matches!(reject_bad_name("/"), Err(FilesError::BadName(_))));
        assert!(matches!(reject_bad_name("./a.txt"), Err(FilesError::BadName(_))));
        assert!(matches!(reject_bad_name("a/."), Err(FilesError::BadName(_))));
    }

    #[test]
    fn a_copy_is_named_the_way_vs_code_names_one() {
        let names = copy_candidates("report.md", false);
        assert_eq!(names[0], "report copy.md");
        assert_eq!(names[1], "report copy 2.md");
        assert_eq!(names[2], "report copy 3.md");
    }

    #[test]
    fn a_name_with_no_extension_takes_the_word_on_the_end() {
        let names = copy_candidates("Makefile", false);
        assert_eq!(names[0], "Makefile copy");
        assert_eq!(names[1], "Makefile copy 2");
    }

    /// A leading dot is part of the name and not the start of an extension: a
    /// copy named "space copy dot gitignore" would be a file nobody asked for
    /// and git would never read.
    #[test]
    fn a_dotfile_is_all_stem() {
        let names = copy_candidates(".gitignore", false);
        assert_eq!(names[0], ".gitignore copy");
        assert_eq!(names[1], ".gitignore copy 2");
    }

    /// The **last** dot separates, so a compound suffix keeps only its last
    /// part. `archive copy.tar.gz` would read better and would mean this
    /// function knows which suffixes are compound, which it cannot.
    #[test]
    fn only_the_last_dot_separates_the_extension() {
        assert_eq!(copy_candidates("archive.tar.gz", false)[0], "archive.tar copy.gz");
    }

    /// A directory has no extension whatever dots are in its name: `some copy.dir`
    /// would be a folder renamed rather than a folder copied.
    #[test]
    fn a_directory_has_no_extension_to_keep() {
        let names = copy_candidates("some.dir", true);
        assert_eq!(names[0], "some.dir copy");
        assert_eq!(names[1], "some.dir copy 2");
    }

    #[test]
    fn a_hundred_names_are_offered_and_not_one_more() {
        let names = copy_candidates("report.md", false);
        assert_eq!(names.len(), MAX_COPY_NAMES);
        assert_eq!(names.len(), 100, "the loop over them has the disk in it");
        assert_eq!(names[99], "report copy 100.md");
    }

    #[test]
    fn a_folder_is_inside_itself_and_inside_nothing_that_merely_starts_alike() {
        let src = Path::new("/p/src");
        assert!(is_inside(src, Path::new("/p/src/components")), "a descendant is inside");
        assert!(is_inside(src, src), "and a folder counts as inside itself");
        assert!(
            !is_inside(src, Path::new("/p/srcx")),
            "a longer name sharing a prefix is a neighbour, which components see and characters do not"
        );
        assert!(!is_inside(src, Path::new("/p")), "the parent is not inside the child");
        assert!(!is_inside(src, Path::new("/q/src")), "and neither is a namesake elsewhere");
    }

    #[test]
    fn an_error_travels_to_the_front_end_as_a_kind_and_text_pair() {
        let json = serde_json::to_value(FilesError::Binary("a.png".into())).unwrap();
        assert_eq!(json["kind"], "binary");
        assert!(json["message"].as_str().unwrap().contains("a.png"));
    }
}
