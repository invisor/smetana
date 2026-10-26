//! What a client with a remote server cannot do on its own disk: browse the server's folders, clone
//! a repository onto it, and fetch a file back from it.
//!
//! Step 7 of `docs/superpowers/specs/2026-10-25-headless-server-design.md` ("Things that name a path
//! on the client"). On one machine a path picked in a dialog is a path the server can open; on a
//! remote server it is not, so the client asks the server instead. Three commands, all of them
//! confined, and **the confinement is the whole of this file**:
//!
//! - [`folders_list`] lists the directories under one root (the server user's home), and refuses any
//!   path whose real location is outside it. A symlink that leaves the root is not followed and not
//!   listed.
//! - [`project_clone`] runs `git clone` into a new folder directly under a folder inside that root.
//! - [`file_fetch`] reads one file, and only from the attachment store or from a project's
//!   `.smetana/reports` folder (`ArtifactRoots`). The check is made on the *real* path of both sides:
//!   `..`, a symlink inside a root pointing out of it and an absolute path elsewhere are all the same
//!   refusal. This is the command that lets a client open a run report or an attachment that lives on
//!   a machine it cannot reach, and it must never become a way to read the server's other files.
//!
//! Nothing here knows about a device, a connection or Tauri: the dispatcher hands the roots in.

use std::path::{Component, Path, PathBuf};
use std::time::Duration;

use base64::Engine;
use serde::Serialize;

use crate::attachments::MAX_ATTACHMENT_BYTES;

/// How many directories one listing carries at most. A folder with more is cut, and the reply says so.
pub const MAX_ENTRIES: usize = 2000;

/// How long a clone may run before it is given up. A large repository over a slow link is minutes.
pub const CLONE_TIMEOUT: Duration = Duration::from_secs(15 * 60);

#[derive(Debug, thiserror::Error)]
pub enum PlacementError {
    /// The path is not under the root (or the roots) this command may reach.
    #[error("{0} is outside the folders this server lets a client reach")]
    Outside(String),
    #[error("{0} is not a folder")]
    NotAFolder(String),
    #[error("{0} is not a file")]
    NotAFile(String),
    #[error("{0}")]
    Io(String),
    #[error("{name} is {bytes} bytes; the ceiling is {MAX_ATTACHMENT_BYTES} bytes")]
    TooLarge { name: String, bytes: u64 },
    #[error("{0}")]
    BadRequest(String),
    #[error("{0}")]
    Clone(String),
}

/// Travels as `{ kind, message }`, the form the front end reads a refusal in: `message` is the
/// sentence a person is shown, not the bare path the variant carries.
impl Serialize for PlacementError {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let kind = match self {
            PlacementError::Outside(_) => "outside",
            PlacementError::NotAFolder(_) => "not-a-folder",
            PlacementError::NotAFile(_) => "not-a-file",
            PlacementError::Io(_) => "io",
            PlacementError::TooLarge { .. } => "too-large",
            PlacementError::BadRequest(_) => "bad-request",
            PlacementError::Clone(_) => "clone",
        };
        let mut out = serializer.serialize_struct("PlacementError", 2)?;
        out.serialize_field("kind", kind)?;
        out.serialize_field("message", &self.to_string())?;
        out.end()
    }
}

fn io(path: &Path, err: std::io::Error) -> PlacementError {
    PlacementError::Io(format!("{}: {err}", path.display()))
}

/// The folder a client browses from: the server user's home. `None` where the environment names none.
pub fn browse_root() -> Option<PathBuf> {
    ["HOME", "USERPROFILE"]
        .into_iter()
        .find_map(|name| std::env::var_os(name).map(PathBuf::from).filter(|path| !path.as_os_str().is_empty()))
}

/// Whether `real` (already canonical) lies under `root` (already canonical), equal included.
fn under(root: &Path, real: &Path) -> bool {
    real.starts_with(root)
}

/// The canonical form of `path`, which resolves every `..` and every symlink on the way.
fn real(path: &Path) -> Result<PathBuf, PlacementError> {
    std::fs::canonicalize(path).map_err(|err| io(path, err))
}

// ---- the folder browser ---------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderEntry {
    pub name: String,
    pub path: String,
    /// A git repository: it has a `.git` inside.
    pub repo: bool,
    /// It has a `.beads` inside: a project this app has already set up a tracker in.
    pub tracker: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderListing {
    /// The top of what may be browsed. The client never offers a way above it.
    pub root: String,
    /// The folder listed, real and absolute.
    pub path: String,
    /// The folder above it, or `None` at the root.
    pub parent: Option<String>,
    pub entries: Vec<FolderEntry>,
    /// There were more than [`MAX_ENTRIES`] and the rest were left off.
    pub truncated: bool,
    /// Whether this folder is itself a git repository / has a tracker.
    pub repo: bool,
    pub tracker: bool,
}

fn text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

/// Lists the directories directly under `requested` (the root itself when `None`), refusing anything
/// whose real location is not under `root`. Hidden folders (a leading dot) are not listed: the
/// browser is for finding a project, and `.ssh` is not one. Files are not listed.
pub fn folders_list(root: &Path, requested: Option<&str>) -> Result<FolderListing, PlacementError> {
    let root = real(root)?;
    let wanted = match requested {
        None => root.clone(),
        Some(path) => {
            let path = Path::new(path);
            if !path.is_absolute() {
                return Err(PlacementError::BadRequest(format!("{} is not an absolute path", path.display())));
            }
            real(path)?
        }
    };
    if !under(&root, &wanted) {
        return Err(PlacementError::Outside(text(&wanted)));
    }
    if !wanted.is_dir() {
        return Err(PlacementError::NotAFolder(text(&wanted)));
    }
    let mut entries = Vec::new();
    let mut truncated = false;
    let read = std::fs::read_dir(&wanted).map_err(|err| io(&wanted, err))?;
    for item in read.flatten() {
        let name = item.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }
        // A symlink to a folder is a folder; one that leaves the root is not listed at all.
        let Ok(resolved) = std::fs::canonicalize(item.path()) else { continue };
        if !resolved.is_dir() || !under(&root, &resolved) {
            continue;
        }
        if entries.len() >= MAX_ENTRIES {
            truncated = true;
            break;
        }
        entries.push(FolderEntry {
            name,
            repo: resolved.join(".git").exists(),
            tracker: resolved.join(".beads").is_dir(),
            path: text(&resolved),
        });
    }
    entries.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()).then_with(|| a.name.cmp(&b.name)));
    let parent = if wanted == root { None } else { wanted.parent().map(text) };
    Ok(FolderListing {
        root: text(&root),
        parent,
        truncated,
        repo: wanted.join(".git").exists(),
        tracker: wanted.join(".beads").is_dir(),
        path: text(&wanted),
        entries,
    })
}

// ---- clone ----------------------------------------------------------------------------------

/// The folder name a repository address ends in: `https://host/team/app.git` is `app`.
pub fn clone_name(url: &str) -> Option<String> {
    let trimmed = url.trim().trim_end_matches('/');
    let last = trimmed.rsplit(['/', ':']).next()?;
    let last = last.strip_suffix(".git").unwrap_or(last);
    valid_folder_name(last).then(|| last.to_owned())
}

/// A name that is one plain path segment: no separator, no `..`, no leading dot or dash.
// Mirrored by `validFolderName` in `src/components/projects/folderBrowser.js`; change one, change both.
pub fn valid_folder_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 255
        && !name.starts_with(['.', '-'])
        && !name.contains(['/', '\\', '\0'])
        && !name.chars().any(char::is_control)
}

/// Whether an address is one `git clone` is told to try. An address starting with `-` would be read
/// as an option, and the `ext::` transport runs a command, so neither is let through; `file:` and a
/// bare path would copy something off the server's own disk, which is not what "clone a repository"
/// asks for.
// Mirrored by `validCloneUrl` in `src/components/projects/folderBrowser.js`; change one, change both.
pub fn valid_clone_url(url: &str) -> bool {
    let url = url.trim();
    if url.is_empty() || url.starts_with('-') || url.chars().any(char::is_control) || url.contains(char::is_whitespace) {
        return false;
    }
    let lower = url.to_ascii_lowercase();
    if ["https://", "http://", "ssh://", "git://"].iter().any(|scheme| lower.starts_with(scheme)) {
        return true;
    }
    // The scp form: `git@host:team/app.git`. A `::` is a remote-helper transport (`ext::`).
    !lower.contains("::") && !lower.contains("://") && url.split_once(':').is_some_and(|(host, path)| {
        (host.contains('@') || host.contains('.')) && !path.is_empty() && !host.contains(['/', '\\']) && !path.starts_with(':')
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Cloned {
    /// The new folder, real and absolute.
    pub path: String,
}

/// The arguments of the clone, kept apart so a test can pin them.
pub fn clone_args(url: &str, dest: &Path) -> Vec<std::ffi::OsString> {
    let mut args: Vec<std::ffi::OsString> = Vec::new();
    for setting in ["protocol.ext.allow=never", "protocol.file.allow=never"] {
        args.push("-c".into());
        args.push(setting.into());
    }
    args.push("clone".into());
    args.push("--".into());
    args.push(url.trim().into());
    args.push(dest.into());
    args
}

/// Clones `url` into `parent/<name>`. `parent` must be a folder under `root`; `name` defaults to the
/// address's last segment. The destination must not exist. Git never asks anything on the terminal
/// (`GIT_TERMINAL_PROMPT=0`, no ssh password prompt): a repository that wants credentials the server
/// does not have fails with git's own words.
pub async fn project_clone(
    root: &Path,
    url: String,
    parent: String,
    name: Option<String>,
) -> Result<Cloned, PlacementError> {
    if !valid_clone_url(&url) {
        return Err(PlacementError::BadRequest(
            "That is not an address git can clone from (use https://, ssh://, git:// or the user@host:path form).".into(),
        ));
    }
    let name = match name.filter(|name| !name.trim().is_empty()) {
        Some(name) => name.trim().to_owned(),
        None => clone_name(&url)
            .ok_or_else(|| PlacementError::BadRequest("Could not work out a folder name from the address; give one.".into()))?,
    };
    if !valid_folder_name(&name) {
        return Err(PlacementError::BadRequest(format!("{name:?} is not a usable folder name.")));
    }
    let listing_root = real(root)?;
    let parent_real = real(Path::new(&parent))?;
    if !under(&listing_root, &parent_real) {
        return Err(PlacementError::Outside(text(&parent_real)));
    }
    if !parent_real.is_dir() {
        return Err(PlacementError::NotAFolder(text(&parent_real)));
    }
    let dest = parent_real.join(&name);
    // Created here, exclusively: git clones into an empty folder, and the folder being ours is what
    // makes it safe to remove on failure. Two clones to one name cannot both get past this line, so
    // neither can delete what the other is writing.
    match std::fs::create_dir(&dest) {
        Ok(()) => {}
        Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
            return Err(PlacementError::BadRequest(format!("{} already exists.", dest.display())));
        }
        Err(err) => return Err(io(&dest, err)),
    }
    // Removed on every way out of this function - an error, the timeout, the caller dropping the
    // future - and only ever the folder this call made.
    let made = Made(Some(dest.clone()));
    run_clone(clone_args(&url, &dest), ssh_batch_mode().await, CLONE_TIMEOUT).await?;
    let path = text(&real(&dest)?);
    made.keep();
    Ok(Cloned { path })
}

/// A folder this call created, removed when the guard is dropped unless it was kept.
struct Made(Option<PathBuf>);

impl Made {
    fn keep(mut self) {
        self.0 = None;
    }
}

impl Drop for Made {
    fn drop(&mut self) {
        if let Some(dir) = self.0.take() {
            let _ = std::fs::remove_dir_all(dir);
        }
    }
}

/// Kills the whole process group git was started in when dropped while armed: `git clone` runs
/// helpers (`git-remote-https`, `ssh`, `index-pack`) that outlive a kill of git alone and would go on
/// writing into a folder that is being removed. Disarmed once git has been waited for, since a pid
/// that has been reaped may be somebody else's by then.
struct GroupKill(Option<u32>);

impl GroupKill {
    fn disarm(&mut self) {
        self.0 = None;
    }
}

impl Drop for GroupKill {
    fn drop(&mut self) {
        #[cfg(unix)]
        if let Some(pid) = self.0.take() {
            // SAFETY: a negative pid is a process group; the group was made by this call's spawn.
            unsafe {
                libc::kill(-(pid as i32), libc::SIGKILL);
            }
        }
        #[cfg(not(unix))]
        let _ = self.0.take();
    }
}

/// Whether to make ssh refuse to ask anything (`BatchMode`): only when the operator has not chosen how
/// git runs ssh, by `GIT_SSH_COMMAND`, `GIT_SSH` or `core.sshCommand`. An operator's own setting wins,
/// and a host key must then be known to the service user (see `server/smetanad/README.md`).
async fn ssh_batch_mode() -> bool {
    if std::env::var_os("GIT_SSH_COMMAND").is_some() || std::env::var_os("GIT_SSH").is_some() {
        return false;
    }
    let configured = tokio::process::Command::new("git")
        .args(["config", "--get", "core.sshCommand"])
        .stdin(std::process::Stdio::null())
        .output()
        .await
        .map(|out| out.status.success() && !out.stdout.iter().all(u8::is_ascii_whitespace))
        .unwrap_or(false);
    !configured
}

/// Runs `git` with `args` (already validated) and says why it failed. No terminal, no prompt.
async fn run_clone(
    args: Vec<std::ffi::OsString>,
    ssh_batch: bool,
    timeout: Duration,
) -> Result<(), PlacementError> {
    run_program("git", args, ssh_batch, timeout).await
}

async fn run_program(
    program: impl AsRef<std::ffi::OsStr>,
    args: Vec<std::ffi::OsString>,
    ssh_batch: bool,
    timeout: Duration,
) -> Result<(), PlacementError> {
    let mut command = tokio::process::Command::new(program);
    command
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);
    if ssh_batch {
        command.env("GIT_SSH_COMMAND", "ssh -o BatchMode=yes");
    }
    // Its own process group, so the helpers it starts can be ended with it.
    #[cfg(unix)]
    command.process_group(0);
    let child = command.spawn().map_err(|err| PlacementError::Clone(format!("could not run git: {err}")))?;
    let mut group = GroupKill(child.id());
    let output = tokio::time::timeout(timeout, child.wait_with_output()).await;
    let output = match output {
        // The guard ends the group as it drops, here and on any other way out.
        Err(_) => return Err(PlacementError::Clone("The clone took longer than fifteen minutes and was stopped.".into())),
        Ok(output) => output,
    };
    group.disarm();
    let output = output.map_err(|err| PlacementError::Clone(format!("could not run git: {err}")))?;
    if output.status.success() {
        return Ok(());
    }
    let why = String::from_utf8_lossy(&output.stderr);
    let why = why.trim();
    Err(PlacementError::Clone(if why.is_empty() { "git clone failed.".into() } else { why.into() }))
}

// ---- reading a file back --------------------------------------------------------------------

/// Where `file_fetch` may read from.
#[derive(Debug, Clone, Default)]
pub struct ArtifactRoots {
    /// The attachment store, when it can be located.
    pub attachments: Option<PathBuf>,
    /// The projects the server knows; each contributes its `.smetana/reports` folder.
    pub projects: Vec<PathBuf>,
}

/// The `.smetana/reports` folder of `project`, real, **only if it really is inside the project**: a
/// `reports` or `.smetana` that is a symlink to somewhere else would otherwise make that somewhere a
/// root.
fn reports_root(project: &Path) -> Option<PathBuf> {
    let project = std::fs::canonicalize(project).ok()?;
    let reports = std::fs::canonicalize(project.join(".smetana").join("reports")).ok()?;
    (reports.starts_with(project.join(".smetana")) && reports.is_dir()).then_some(reports)
}

impl ArtifactRoots {
    /// Every root that exists right now, real.
    pub fn resolved(&self) -> Vec<PathBuf> {
        let mut roots: Vec<PathBuf> = self.attachments.iter().filter_map(|root| std::fs::canonicalize(root).ok()).collect();
        roots.extend(self.projects.iter().filter_map(|project| reports_root(project)));
        roots
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Fetched {
    pub name: String,
    pub bytes: u64,
    /// The file's bytes, base64.
    pub data: String,
}

/// Reads `path` if, and only if, its real location is a file inside one of the roots.
pub fn file_fetch(roots: &ArtifactRoots, path: &str) -> Result<Fetched, PlacementError> {
    let asked = Path::new(path);
    if !asked.is_absolute() {
        return Err(PlacementError::BadRequest(format!("{path} is not an absolute path")));
    }
    // A `..` is refused outright, before the filesystem is asked: canonicalisation would resolve it,
    // but a path that climbs has no business being sent by an honest client.
    if asked.components().any(|part| matches!(part, Component::ParentDir)) {
        return Err(PlacementError::Outside(path.to_owned()));
    }
    let resolved = roots.resolved();
    let target = match std::fs::canonicalize(asked) {
        Ok(target) => target,
        // A file that does not exist is told apart from one that is not allowed only when the
        // *lexical* path is inside a root; otherwise the answer is the same "outside" as for a file
        // that does exist, so the command is no way to ask whether a path exists.
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            let near = asked.parent().and_then(|dir| std::fs::canonicalize(dir).ok());
            return Err(match near {
                Some(dir) if resolved.iter().any(|root| under(root, &dir)) => io(asked, err),
                _ => PlacementError::Outside(path.to_owned()),
            });
        }
        Err(_) => return Err(PlacementError::Outside(path.to_owned())),
    };
    if !resolved.iter().any(|root| under(root, &target)) {
        return Err(PlacementError::Outside(path.to_owned()));
    }
    let meta = std::fs::metadata(&target).map_err(|err| io(&target, err))?;
    let name = target.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
    if !meta.is_file() {
        return Err(PlacementError::NotAFile(name));
    }
    if meta.len() > MAX_ATTACHMENT_BYTES {
        return Err(PlacementError::TooLarge { name, bytes: meta.len() });
    }
    let bytes = std::fs::read(&target).map_err(|err| io(&target, err))?;
    Ok(Fetched {
        name,
        bytes: bytes.len() as u64,
        data: base64::engine::general_purpose::STANDARD.encode(bytes),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir() -> tempfile::TempDir {
        tempfile::tempdir().unwrap()
    }

    fn touch(path: &Path, bytes: &[u8]) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, bytes).unwrap();
    }

    // ---- the folder browser

    #[test]
    fn the_root_is_listed_with_its_folders_only_sorted_and_hidden_ones_left_out() {
        let tmp = dir();
        let root = tmp.path();
        std::fs::create_dir_all(root.join("beta/.git")).unwrap();
        std::fs::create_dir_all(root.join("Alpha/.beads")).unwrap();
        std::fs::create_dir_all(root.join(".ssh")).unwrap();
        touch(&root.join("notes.txt"), b"x");
        let listing = folders_list(root, None).unwrap();
        let names: Vec<_> = listing.entries.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, ["Alpha", "beta"]);
        assert!(listing.entries[0].tracker && !listing.entries[0].repo);
        assert!(listing.entries[1].repo && !listing.entries[1].tracker);
        assert_eq!(listing.parent, None, "the root has nothing above it to offer");
        assert!(!listing.truncated);
    }

    #[test]
    fn a_subfolder_names_its_parent_and_the_parent_of_the_root_is_never_offered() {
        let tmp = dir();
        std::fs::create_dir_all(tmp.path().join("a/b")).unwrap();
        let sub = real(&tmp.path().join("a")).unwrap();
        let listing = folders_list(tmp.path(), Some(&text(&sub))).unwrap();
        assert_eq!(listing.entries.len(), 1);
        assert_eq!(listing.parent.as_deref(), Some(text(&real(tmp.path()).unwrap()).as_str()));
    }

    #[test]
    fn a_path_outside_the_root_is_refused() {
        let tmp = dir();
        let other = dir();
        let root = tmp.path().join("home");
        std::fs::create_dir_all(&root).unwrap();
        let err = folders_list(&root, Some(&text(other.path()))).unwrap_err();
        assert!(matches!(err, PlacementError::Outside(_)), "{err}");
        let err = folders_list(&root, Some("/")).unwrap_err();
        assert!(matches!(err, PlacementError::Outside(_)), "{err}");
    }

    #[test]
    fn dot_dot_cannot_climb_out_of_the_root() {
        let tmp = dir();
        let root = tmp.path().join("home");
        std::fs::create_dir_all(root.join("a")).unwrap();
        std::fs::create_dir_all(tmp.path().join("secret")).unwrap();
        let sneaky = format!("{}/a/../../secret", root.display());
        let err = folders_list(&root, Some(&sneaky)).unwrap_err();
        assert!(matches!(err, PlacementError::Outside(_)), "{err}");
    }

    #[test]
    fn a_relative_path_is_a_bad_request() {
        let tmp = dir();
        let err = folders_list(tmp.path(), Some("a/b")).unwrap_err();
        assert!(matches!(err, PlacementError::BadRequest(_)), "{err}");
    }

    #[cfg(unix)]
    #[test]
    fn a_symlink_leaving_the_root_is_neither_listed_nor_followed() {
        let tmp = dir();
        let root = tmp.path().join("home");
        let outside = tmp.path().join("outside");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::create_dir_all(outside.join("inner")).unwrap();
        std::os::unix::fs::symlink(&outside, root.join("escape")).unwrap();
        std::fs::create_dir_all(root.join("fine")).unwrap();
        // A link to a folder inside the root is a folder like any other.
        std::os::unix::fs::symlink(root.join("fine"), root.join("alias")).unwrap();
        let listing = folders_list(&root, None).unwrap();
        let names: Vec<_> = listing.entries.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, ["alias", "fine"], "the escaping link is not offered");
        // Asking for it by name is the same refusal as any other outside path.
        let err = folders_list(&root, Some(&text(&root.join("escape")))).unwrap_err();
        assert!(matches!(err, PlacementError::Outside(_)), "{err}");
        let err = folders_list(&root, Some(&text(&root.join("escape/inner")))).unwrap_err();
        assert!(matches!(err, PlacementError::Outside(_)), "{err}");
    }

    #[test]
    fn a_file_is_not_a_folder() {
        let tmp = dir();
        touch(&tmp.path().join("f.txt"), b"x");
        let err = folders_list(tmp.path(), Some(&text(&tmp.path().join("f.txt")))).unwrap_err();
        assert!(matches!(err, PlacementError::NotAFolder(_)), "{err}");
    }

    #[test]
    fn a_huge_folder_is_cut_and_says_so() {
        let tmp = dir();
        for i in 0..(MAX_ENTRIES + 5) {
            std::fs::create_dir(tmp.path().join(format!("d{i:05}"))).unwrap();
        }
        let listing = folders_list(tmp.path(), None).unwrap();
        assert_eq!(listing.entries.len(), MAX_ENTRIES);
        assert!(listing.truncated);
    }

    // ---- clone

    #[test]
    fn the_folder_name_comes_from_the_last_segment_of_the_address() {
        assert_eq!(clone_name("https://host/team/app.git").as_deref(), Some("app"));
        assert_eq!(clone_name("git@host:team/app.git").as_deref(), Some("app"));
        assert_eq!(clone_name("https://host/team/app/").as_deref(), Some("app"));
        assert_eq!(clone_name("https://host/").as_deref(), Some("host"));
        assert_eq!(clone_name("").as_deref(), None);
    }

    #[test]
    fn a_folder_name_is_one_plain_segment() {
        for good in ["app", "my app", "app-2", "a.b"] {
            assert!(valid_folder_name(good), "{good}");
        }
        for bad in ["", "..", ".hidden", "-x", "a/b", "a\\b", "a\0b", "a\nb"] {
            assert!(!valid_folder_name(bad), "{bad:?}");
        }
    }

    #[test]
    fn only_real_clone_addresses_are_let_through() {
        for good in [
            "https://github.com/team/app.git",
            "http://host/app",
            "ssh://git@host/team/app.git",
            "git://host/app.git",
            "git@github.com:team/app.git",
        ] {
            assert!(valid_clone_url(good), "{good}");
        }
        for bad in [
            "",
            "-oProxyCommand=evil",
            "--upload-pack=evil",
            "ext::sh -c evil",
            "ext::sh%20-c%20evil",
            "file:///etc",
            "/etc/passwd",
            "../x",
            "https://host/a b",
            "fd::17",
        ] {
            assert!(!valid_clone_url(bad), "{bad}");
        }
    }

    #[test]
    fn git_is_told_to_refuse_command_transports_and_to_stop_reading_options() {
        let args: Vec<String> =
            clone_args("https://h/a.git", Path::new("/home/u/a")).iter().map(|a| a.to_string_lossy().into_owned()).collect();
        let at = args.iter().position(|a| a == "clone").unwrap();
        assert_eq!(args[at + 1], "--", "the address comes after the end of options");
        assert!(args.windows(2).any(|w| w[0] == "-c" && w[1] == "protocol.ext.allow=never"));
    }

    #[tokio::test]
    async fn a_clone_into_a_parent_outside_the_root_is_refused_before_git_runs() {
        let tmp = dir();
        let root = tmp.path().join("home");
        std::fs::create_dir_all(&root).unwrap();
        let outside = dir();
        let err = project_clone(&root, "https://h/a.git".into(), text(outside.path()), None).await.unwrap_err();
        assert!(matches!(err, PlacementError::Outside(_)), "{err}");
    }

    #[tokio::test]
    async fn a_clone_onto_an_existing_folder_is_refused() {
        let tmp = dir();
        std::fs::create_dir_all(tmp.path().join("a")).unwrap();
        let err = project_clone(tmp.path(), "https://h/a.git".into(), text(tmp.path()), None).await.unwrap_err();
        assert!(matches!(err, PlacementError::BadRequest(_)), "{err}");
    }

    #[tokio::test]
    async fn a_name_that_climbs_is_refused() {
        let tmp = dir();
        let err = project_clone(tmp.path(), "https://h/a.git".into(), text(tmp.path()), Some("../x".into()))
            .await
            .unwrap_err();
        assert!(matches!(err, PlacementError::BadRequest(_)), "{err}");
    }

    fn git_in(dir: &Path, args: &[&str]) {
        assert!(std::process::Command::new("git")
            .args(args)
            .current_dir(dir)
            .env("GIT_AUTHOR_NAME", "t")
            .env("GIT_AUTHOR_EMAIL", "t@t")
            .env("GIT_COMMITTER_NAME", "t")
            .env("GIT_COMMITTER_EMAIL", "t@t")
            .status()
            .unwrap()
            .success());
    }

    fn have_git() -> bool {
        std::process::Command::new("git").arg("--version").output().is_ok()
    }

    #[tokio::test]
    async fn the_clone_runner_clones_a_repository_and_reports_a_failure_in_gits_words() {
        // `run_clone` is the real spawn, environment and failure path; `project_clone` hands it
        // `clone_args`, which refuse `file:` and so cannot be pointed at a repository on this disk.
        if !have_git() {
            return;
        }
        let tmp = dir();
        let src = tmp.path().join("src");
        std::fs::create_dir_all(&src).unwrap();
        git_in(&src, &["init", "-q"]);
        touch(&src.join("a.txt"), b"hi");
        git_in(&src, &["add", "."]);
        git_in(&src, &["commit", "-q", "-m", "x"]);
        let dest = tmp.path().join("copy");
        let args = |from: &Path, to: &Path| -> Vec<std::ffi::OsString> {
            vec!["clone".into(), "-q".into(), "--".into(), from.into(), to.into()]
        };
        run_clone(args(&src, &dest), true, Duration::from_secs(60)).await.unwrap();
        assert!(dest.join(".git").exists() && dest.join("a.txt").exists());
        let err = run_clone(args(&tmp.path().join("nothing"), &tmp.path().join("x")), true, Duration::from_secs(60))
            .await
            .unwrap_err();
        assert!(matches!(err, PlacementError::Clone(ref why) if !why.is_empty()), "{err}");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn a_timeout_ends_git_and_the_helpers_it_started() {
        use std::os::unix::fs::PermissionsExt;
        let tmp = dir();
        let pid_file = tmp.path().join("helper.pid");
        // A stand-in for git that starts a helper which would outlive it, then waits.
        let script = tmp.path().join("fake-git");
        std::fs::write(
            &script,
            format!("#!/bin/sh\nsleep 60 &\necho $! > {}\nwait\n", pid_file.display()),
        )
        .unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        let err = run_program(&script, vec![], false, Duration::from_secs(3)).await.unwrap_err();
        assert!(matches!(err, PlacementError::Clone(ref why) if why.contains("longer")), "{err}");
        let helper: i32 = std::fs::read_to_string(&pid_file).unwrap().trim().parse().unwrap();
        let mut gone = false;
        for _ in 0..50 {
            // SAFETY: signal 0 only asks whether the process exists.
            if unsafe { libc::kill(helper, 0) } != 0 {
                gone = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        assert!(gone, "the helper git started was left running");
    }

    #[test]
    fn the_folder_a_clone_made_goes_with_the_guard_unless_it_was_kept() {
        let tmp = dir();
        let made = tmp.path().join("made");
        let kept = tmp.path().join("kept");
        std::fs::create_dir_all(&made).unwrap();
        std::fs::create_dir_all(&kept).unwrap();
        drop(Made(Some(made.clone())));
        Made(Some(kept.clone())).keep();
        assert!(!made.exists());
        assert!(kept.exists());
    }

    #[tokio::test]
    async fn a_failed_clone_removes_the_folder_it_made_and_a_second_clone_of_the_name_is_refused_meanwhile() {
        if !have_git() {
            return;
        }
        let tmp = dir();
        let root = tmp.path().join("home");
        std::fs::create_dir_all(&root).unwrap();
        // An address git accepts and cannot reach: the connection is refused at once.
        let url = "git://127.0.0.1:1/none.git".to_owned();
        let err = project_clone(&root, url.clone(), text(&root), Some("app".into())).await.unwrap_err();
        assert!(matches!(err, PlacementError::Clone(_)), "{err}");
        assert!(!root.join("app").exists(), "what the call made is removed on failure");
        // A folder that was not made by this call is never touched, failure or not.
        std::fs::create_dir_all(root.join("mine")).unwrap();
        std::fs::write(root.join("mine/keep.txt"), "x").unwrap();
        let err = project_clone(&root, url, text(&root), Some("mine".into())).await.unwrap_err();
        assert!(matches!(err, PlacementError::BadRequest(_)), "{err}");
        assert!(root.join("mine/keep.txt").exists());
    }

    // ---- reading a file back

    struct Rig {
        _tmp: tempfile::TempDir,
        base: PathBuf,
        roots: ArtifactRoots,
    }

    fn rig() -> Rig {
        let tmp = dir();
        let base = std::fs::canonicalize(tmp.path()).unwrap();
        let attachments = base.join("data/attachments");
        let project = base.join("work/app");
        touch(&attachments.join("p-1/pic.png"), b"png-bytes");
        touch(&project.join(".smetana/reports/run-1.html"), b"<html>report</html>");
        touch(&project.join("src/main.rs"), b"fn main() {}");
        touch(&base.join("secret.txt"), b"secret");
        std::fs::create_dir_all(base.join("data")).unwrap();
        let roots = ArtifactRoots { attachments: Some(attachments), projects: vec![project] };
        Rig { _tmp: tmp, base, roots }
    }

    #[test]
    fn an_attachment_and_a_report_can_be_read() {
        let rig = rig();
        let pic = file_fetch(&rig.roots, &text(&rig.base.join("data/attachments/p-1/pic.png"))).unwrap();
        assert_eq!(pic.name, "pic.png");
        assert_eq!(base64::engine::general_purpose::STANDARD.decode(pic.data).unwrap(), b"png-bytes");
        let report = file_fetch(&rig.roots, &text(&rig.base.join("work/app/.smetana/reports/run-1.html"))).unwrap();
        assert_eq!(report.bytes, 19);
    }

    #[test]
    fn a_file_elsewhere_in_the_project_is_refused() {
        let rig = rig();
        let err = file_fetch(&rig.roots, &text(&rig.base.join("work/app/src/main.rs"))).unwrap_err();
        assert!(matches!(err, PlacementError::Outside(_)), "{err}");
    }

    #[test]
    fn an_absolute_path_outside_both_roots_is_refused() {
        let rig = rig();
        for path in [text(&rig.base.join("secret.txt")), "/etc/passwd".to_owned(), "/".to_owned()] {
            let err = file_fetch(&rig.roots, &path).unwrap_err();
            assert!(matches!(err, PlacementError::Outside(_)), "{path}: {err}");
        }
    }

    #[test]
    fn a_relative_path_is_refused() {
        let rig = rig();
        let err = file_fetch(&rig.roots, "data/attachments/p-1/pic.png").unwrap_err();
        assert!(matches!(err, PlacementError::BadRequest(_)), "{err}");
    }

    #[test]
    fn dot_dot_out_of_a_root_is_refused_even_when_it_lands_on_a_real_file() {
        let rig = rig();
        let sneaky = format!("{}/data/attachments/p-1/../../../secret.txt", rig.base.display());
        let err = file_fetch(&rig.roots, &sneaky).unwrap_err();
        assert!(matches!(err, PlacementError::Outside(_)), "{err}");
        // Even a `..` that stays inside is not accepted: no honest client sends one.
        let inside = format!("{}/data/attachments/p-1/../p-1/pic.png", rig.base.display());
        assert!(matches!(file_fetch(&rig.roots, &inside).unwrap_err(), PlacementError::Outside(_)));
    }

    #[cfg(unix)]
    #[test]
    fn a_symlink_inside_a_root_pointing_outside_is_refused() {
        let rig = rig();
        let link = rig.base.join("data/attachments/p-1/link.txt");
        std::os::unix::fs::symlink(rig.base.join("secret.txt"), &link).unwrap();
        let err = file_fetch(&rig.roots, &text(&link)).unwrap_err();
        assert!(matches!(err, PlacementError::Outside(_)), "{err}");
        // A symlinked folder inside a root that leads out is the same.
        let dir_link = rig.base.join("data/attachments/p-1/etc");
        std::os::unix::fs::symlink(&rig.base, &dir_link).unwrap();
        let err = file_fetch(&rig.roots, &text(&dir_link.join("secret.txt"))).unwrap_err();
        assert!(matches!(err, PlacementError::Outside(_)), "{err}");
    }

    #[cfg(unix)]
    #[test]
    fn a_reports_folder_that_is_a_symlink_out_of_the_project_is_not_a_root() {
        let rig = rig();
        let project = rig.base.join("work/evil");
        std::fs::create_dir_all(project.join(".smetana")).unwrap();
        let loot = rig.base.join("loot");
        touch(&loot.join("passwd"), b"root:x");
        std::os::unix::fs::symlink(&loot, project.join(".smetana/reports")).unwrap();
        let roots = ArtifactRoots { attachments: None, projects: vec![project.clone()] };
        let err = file_fetch(&roots, &text(&project.join(".smetana/reports/passwd"))).unwrap_err();
        assert!(matches!(err, PlacementError::Outside(_)), "{err}");
        let err = file_fetch(&roots, &text(&loot.join("passwd"))).unwrap_err();
        assert!(matches!(err, PlacementError::Outside(_)), "{err}");
    }

    #[test]
    fn a_missing_file_inside_a_root_says_so_and_one_outside_says_nothing_about_existence() {
        let rig = rig();
        let missing_in = file_fetch(&rig.roots, &text(&rig.base.join("data/attachments/p-1/none.png"))).unwrap_err();
        assert!(matches!(missing_in, PlacementError::Io(_)), "{missing_in}");
        let missing_out = file_fetch(&rig.roots, &text(&rig.base.join("nothing-here.txt"))).unwrap_err();
        let present_out = file_fetch(&rig.roots, &text(&rig.base.join("secret.txt"))).unwrap_err();
        assert!(matches!(missing_out, PlacementError::Outside(_)));
        assert!(matches!(present_out, PlacementError::Outside(_)));
    }

    #[test]
    fn a_folder_inside_a_root_is_not_a_file_and_a_big_file_is_too_large() {
        let rig = rig();
        let err = file_fetch(&rig.roots, &text(&rig.base.join("data/attachments/p-1"))).unwrap_err();
        assert!(matches!(err, PlacementError::NotAFile(_)), "{err}");
        let big = rig.base.join("data/attachments/p-1/big.bin");
        std::fs::write(&big, vec![0u8; MAX_ATTACHMENT_BYTES as usize + 1]).unwrap();
        let err = file_fetch(&rig.roots, &text(&big)).unwrap_err();
        assert!(matches!(err, PlacementError::TooLarge { .. }), "{err}");
    }

    #[test]
    fn a_refusal_travels_with_the_sentence_a_person_reads_and_a_kind_a_program_can_test() {
        let err = PlacementError::Outside("/etc/passwd".into());
        let value = serde_json::to_value(&err).unwrap();
        assert_eq!(value["kind"], "outside");
        assert_eq!(value["message"], "/etc/passwd is outside the folders this server lets a client reach");
    }

    #[test]
    fn no_roots_means_no_reads() {
        let rig = rig();
        let err = file_fetch(&ArtifactRoots::default(), &text(&rig.base.join("data/attachments/p-1/pic.png"))).unwrap_err();
        assert!(matches!(err, PlacementError::Outside(_)), "{err}");
    }
}
