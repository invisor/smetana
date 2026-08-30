//! `.smetana/project.toml` — the shape of the project a run works in.
//!
//! Declarative where the work is mechanical, prose where it needs judgement.
//! `regenerate` is a rule that can be applied without thinking; `hazards` is
//! the part no list of paths can express — two branches emitting the same
//! migration number off one base is not a pattern, it is a thing to look for —
//! and it stays as text the lead reads.
//!
//! A damaged file refuses to load. That is the opposite of `settings/model.rs`,
//! where a broken section loses itself and the app carries on, and it is
//! opposite for the right reason: there the cost of leniency is a forgotten
//! panel width, here it is a run whose gates quietly went missing and whose
//! green merges therefore mean nothing. Hence `deny_unknown_fields` throughout:
//! a typo must be louder than a silence.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Where the file lives, relative to the project root — beside `.beads/`.
pub const CONFIG_PATH: &str = ".smetana/project.toml";

/// This one derive is both the TOML schema and the shape that crosses IPC —
/// every other payload in the app is camelCase, and this one is deliberately
/// not. Splitting them (a `rename_all` for JSON, the plain field names for
/// TOML) would mean the JSON and the file spell the same field two different
/// ways, and that costs more than one unusual payload: `target_branch` in the
/// file and `targetBranch` in the front end's console would look like two
/// different settings to whoever is debugging one report against the other.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectConfig {
    pub project: Project,
    #[serde(default)]
    pub defaults: Defaults,
    /// Keyed by the repository path as written in `project.repos`.
    #[serde(default)]
    pub repo: BTreeMap<String, Repo>,
    #[serde(default)]
    pub preflight: Option<Preflight>,
    #[serde(default)]
    pub merge: Option<Merge>,
    #[serde(default)]
    pub live_check: Option<LiveCheck>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Project {
    /// Paths relative to the project root. A monorepo is `["."]`: one entry,
    /// and every mechanism downstream is the same as it is for four.
    pub repos: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Defaults {
    /// What the run dialog offers first. `None` means it falls back to the
    /// branch the project is on — a project that has not chosen one yet must
    /// not have `main` chosen for it.
    pub target_branch: Option<String>,
    pub min_priority: u8,
    pub max_parallel_tasks: u8,
    pub review_passes: u8,
}

impl Default for Defaults {
    fn default() -> Self {
        Self { target_branch: None, min_priority: 2, max_parallel_tasks: 3, review_passes: 5 }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Repo {
    /// Run once in a freshly created worktree.
    pub setup: Option<String>,
    /// What "green" means here. An empty list is legitimate, not an oversight:
    /// a docs-only repository's gate is the unresolved-conflict check and the
    /// commit, and nothing else.
    pub gates: Vec<String>,
    /// Copied from the main checkout into a fresh worktree, which has no
    /// gitignored file in it.
    pub env_files: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Preflight {
    pub commands: Vec<String>,
    pub health: Vec<HealthCheck>,
}

/// Untagged: the two forms are told apart by the key that is present, which is
/// what makes the file readable — `{ url = "…" }` beside `{ tcp = 5433 }`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged, deny_unknown_fields)]
pub enum HealthCheck {
    Url { url: String },
    Tcp { tcp: u16 },
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Merge {
    /// Prose: what must be checked after every merge and does not reduce to a
    /// rule. Read by the lead, never by this app.
    pub hazards: Option<String>,
    pub regenerate: Vec<Regenerate>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Regenerate {
    pub paths: Vec<String>,
    pub command: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LiveCheck {
    pub mode: LiveCheckMode,
    /// The command form's command; the other two modes ignore it.
    #[serde(default)]
    pub command: Option<String>,
    /// Prose for the browser form: how the stand comes up, how to sign in.
    #[serde(default)]
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LiveCheckMode {
    Browser,
    Command,
    None,
}

/// What this project's configuration is, as the front end sees it. A state
/// rather than a `Result`, and for the same reason the tracker's health is one:
/// "there is no file yet" is an ordinary condition that most projects are in,
/// and only `Broken` is a failure worth a message.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", tag = "state")]
pub enum ConfigState {
    Missing,
    Broken { message: String },
    Ok { config: Box<ProjectConfig> },
}

pub fn path_in(root: &Path) -> PathBuf {
    root.join(CONFIG_PATH)
}

/// Pure, so the tests need no directory. The message is the parser's own: it
/// already names the key and the line, which is more than a hand-written
/// summary would say. One shape does worse: `HealthCheck` is untagged, and a
/// value matching neither of its variants comes back as "data did not match
/// any variant of untagged enum HealthCheck" with a line and column but no
/// key name — serde has already thrown the field name away by the time an
/// untagged enum fails.
pub fn parse(text: &str) -> Result<ProjectConfig, String> {
    toml::from_str(text).map_err(|err| err.to_string())
}

/// A file that is absent and a file that cannot be read are deliberately not
/// the same answer. Reading "permission denied" as "not configured" would put
/// the setup dialog in front of a person over a file that already exists and
/// that they cannot see — and the agent would then write over it.
pub fn load(root: &Path) -> ConfigState {
    match std::fs::read_to_string(path_in(root)) {
        Ok(text) => match parse(&text) {
            Ok(config) => ConfigState::Ok { config: Box::new(config) },
            Err(message) => ConfigState::Broken { message },
        },
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => ConfigState::Missing,
        Err(err) => ConfigState::Broken { message: err.to_string() },
    }
}

/// Rewrite `[defaults]` in a file's text, leaving every other byte alone.
///
/// Through `toml_edit` rather than by serializing `ProjectConfig` back out,
/// and the difference is the point: this file is written by the setup agent
/// and edited by people, so it holds comments and prose (`[merge].hazards` is
/// explicitly text a lead reads). A serde round trip drops every comment and
/// reorders the keys into the struct's order, so somebody changing one number
/// would find a colleague's note gone.
///
/// The result is parsed again before it is returned. A save that leaves behind
/// a file the app then refuses to load turns a wrong number into a broken
/// project, and that is the one failure worth spending a check on.
pub fn with_defaults(text: &str, defaults: &Defaults) -> Result<String, String> {
    // The whole file has to parse first: an edit applied to a damaged file
    // would write back a repair nobody asked for.
    parse(text)?;

    let mut doc = text.parse::<toml_edit::DocumentMut>().map_err(|err| err.to_string())?;

    let table = doc
        .entry("defaults")
        .or_insert(toml_edit::Item::Table(toml_edit::Table::new()))
        .as_table_mut()
        .ok_or_else(|| "defaults is not a table".to_string())?;
    // An implicit table is one that exists only because something under it
    // does, and it is not written out at all. A file with no `[defaults]`
    // section of its own would otherwise take the four keys and emit none of
    // them.
    table.set_implicit(false);

    match &defaults.target_branch {
        Some(branch) => table["target_branch"] = toml_edit::value(branch.as_str()),
        // Removed, never emptied — see the test of the same name.
        None => {
            table.remove("target_branch");
        }
    }
    table["min_priority"] = toml_edit::value(i64::from(defaults.min_priority));
    table["max_parallel_tasks"] = toml_edit::value(i64::from(defaults.max_parallel_tasks));
    table["review_passes"] = toml_edit::value(i64::from(defaults.review_passes));

    let out = doc.to_string();
    parse(&out)?;
    Ok(out)
}

/// Read, rewrite, write back atomically.
///
/// The temporary file is created in the same directory and renamed over the
/// target, which is the shape `settings/file.rs` already uses and for its
/// reason: a rename within one directory is the only thing the filesystem
/// promises to do atomically. Its name carries the pid and a counter for that
/// file's reason too — two overlapping writes sharing one name would have the
/// first rename what the second had not finished writing.
///
/// A file that is not there is refused rather than created. This edits four
/// keys of a configuration; the rest of it is the setup agent's, and a file
/// holding nothing but `[defaults]` would not load at all — `[project].repos`
/// has no default.
pub fn save_defaults(root: &Path, defaults: &Defaults) -> Result<(), String> {
    let path = path_in(root);
    let text =
        std::fs::read_to_string(&path).map_err(|err| format!("{}: {err}", path.display()))?;
    let out = with_defaults(&text, defaults)?;

    let temp = temp_path(&path);
    if let Err(err) = write_all(&temp, &out) {
        // We clean up after ourselves: the name is unique, and there is nobody
        // to reuse a half-written file anyway.
        let _ = std::fs::remove_file(&temp);
        return Err(format!("{}: {err}", temp.display()));
    }
    std::fs::rename(&temp, &path).map_err(|err| {
        let _ = std::fs::remove_file(&temp);
        format!("{}: {err}", path.display())
    })
}

/// A counter for temp files. Together with the pid it gives a name no other
/// write has — neither in this process nor in a neighbouring one.
static TEMP_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// `project.<pid>.<n>.tmp` beside the target, in `.smetana/` rather than in the
/// system temp directory: a rename across filesystems is not a rename at all.
fn temp_path(path: &Path) -> PathBuf {
    let n = TEMP_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let name = format!("project.{}.{n}.tmp", std::process::id());
    match path.parent() {
        Some(dir) => dir.join(name),
        None => PathBuf::from(name),
    }
}

/// Flushed before the rename: without that a power loss could make the rename
/// durable and not what is in the file.
fn write_all(temp: &Path, text: &str) -> std::io::Result<()> {
    use std::io::Write;
    let mut file = std::fs::File::create(temp)?;
    file.write_all(text.as_bytes())?;
    file.sync_all()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The whole file, in the shape the spec documents. Written out by hand
    /// rather than serialized from a value and read back: a round trip would
    /// only agree with itself, and what this has to pin is the spelling a
    /// person types.
    const FULL: &str = r#"
[project]
repos = ["backend", "frontend", "admin"]

[defaults]
target_branch = "staging"
min_priority = 1
max_parallel_tasks = 4
review_passes = 3

[repo.backend]
setup = "npm install"
gates = ["npm run typecheck", "npm test"]
env_files = [".env"]

[repo.core]
gates = ["cargo test"]

[preflight]
commands = ["docker compose up -d"]
health = [{ url = "http://localhost:4001/health" }, { tcp = 5433 }]

[merge]
hazards = "Check the migration journal after every merge."

[[merge.regenerate]]
paths = ["admin/src/api-types.ts"]
command = "npm run generate:api-types"

[live_check]
mode = "browser"
notes = "The stand comes up from the live-staging worktrees."
"#;

    #[test]
    fn the_documented_file_parses_into_every_field() {
        let config = parse(FULL).expect("the documented shape parses");

        assert_eq!(config.project.repos, ["backend", "frontend", "admin"]);
        assert_eq!(config.defaults.target_branch.as_deref(), Some("staging"));
        assert_eq!(config.defaults.min_priority, 1);
        assert_eq!(config.defaults.max_parallel_tasks, 4);
        assert_eq!(config.defaults.review_passes, 3);

        let backend = config.repo.get("backend").expect("the backend section");
        assert_eq!(backend.setup.as_deref(), Some("npm install"));
        assert_eq!(backend.gates, ["npm run typecheck", "npm test"]);
        assert_eq!(backend.env_files, [".env"]);

        // A repository in another language differs in nothing but its commands.
        assert_eq!(config.repo.get("core").expect("the core section").gates, ["cargo test"]);

        let preflight = config.preflight.expect("the preflight section");
        assert_eq!(preflight.commands, ["docker compose up -d"]);
        assert_eq!(
            preflight.health,
            [
                HealthCheck::Url { url: "http://localhost:4001/health".into() },
                HealthCheck::Tcp { tcp: 5433 },
            ]
        );

        let merge = config.merge.expect("the merge section");
        assert_eq!(merge.hazards.as_deref(), Some("Check the migration journal after every merge."));
        assert_eq!(merge.regenerate.len(), 1);
        assert_eq!(merge.regenerate[0].paths, ["admin/src/api-types.ts"]);
        assert_eq!(merge.regenerate[0].command, "npm run generate:api-types");

        let live = config.live_check.expect("the live_check section");
        assert_eq!(live.mode, LiveCheckMode::Browser);
        assert_eq!(live.command, None);
    }

    #[test]
    fn a_monorepo_is_one_entry_and_nothing_else_differs() {
        let config = parse("[project]\nrepos = [\".\"]\n").expect("parses");
        assert_eq!(config.project.repos, ["."]);
    }

    #[test]
    fn the_absent_sections_take_their_defaults() {
        // Everything below [project] is optional: a project with one gate list
        // and no stand is a whole configuration.
        let config = parse("[project]\nrepos = [\".\"]\n").expect("parses");
        assert_eq!(config.defaults.min_priority, 2);
        assert_eq!(config.defaults.max_parallel_tasks, 3);
        assert_eq!(config.defaults.review_passes, 5);
        assert_eq!(config.defaults.target_branch, None);
        assert!(config.repo.is_empty());
        assert!(config.preflight.is_none());
        assert!(config.merge.is_none());
        assert!(config.live_check.is_none());
    }

    #[test]
    fn a_missing_repos_list_is_a_refusal_rather_than_an_empty_project() {
        let err = parse("[defaults]\nmin_priority = 0\n").expect_err("no repos, no project");
        assert!(err.contains("project"), "the message names what is missing: {err}");
    }

    #[test]
    fn a_misspelled_key_is_a_refusal_rather_than_a_silently_empty_list() {
        // `gate` instead of `gates` would otherwise mean a repository with no
        // gates at all, and every merge in it would come back green having
        // proved nothing. This is the exact failure the refusal policy exists
        // for, and it is why the structs deny unknown fields.
        let err = parse("[project]\nrepos = [\".\"]\n\n[repo.core]\ngate = [\"cargo test\"]\n")
            .expect_err("an unknown key is a refusal");
        assert!(err.contains("gate"), "the message names the key: {err}");
    }

    #[test]
    fn an_unknown_live_check_mode_is_a_refusal() {
        let err = parse("[project]\nrepos = [\".\"]\n\n[live_check]\nmode = \"playwright\"\n")
            .expect_err("the mode is a closed set");
        assert!(err.contains("mode"), "{err}");
    }

    /// A directory nothing else in the suite touches. Built the way
    /// `library.rs` builds its fixture root: pid plus nanoseconds, so two test
    /// binaries running at once cannot collide.
    fn temp_root(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "smetana-config-test-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock is after the Unix epoch")
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).expect("create the fixture root");
        dir
    }

    fn write_config(root: &Path, text: &str) {
        let path = path_in(root);
        std::fs::create_dir_all(path.parent().expect("the config has a parent directory"))
            .expect("create .smetana");
        std::fs::write(path, text).expect("write the config");
    }

    #[test]
    fn a_folder_with_no_config_is_missing_rather_than_broken() {
        let root = temp_root("absent");
        assert_eq!(load(&root), ConfigState::Missing);
        std::fs::remove_dir_all(&root).expect("clean up");
    }

    #[test]
    fn a_readable_config_loads() {
        let root = temp_root("ok");
        write_config(&root, "[project]\nrepos = [\".\"]\n");
        match load(&root) {
            ConfigState::Ok { config } => assert_eq!(config.project.repos, ["."]),
            other => panic!("expected Ok, got {other:?}"),
        }
        std::fs::remove_dir_all(&root).expect("clean up");
    }

    /// The four keys are set, and everything the person wrote around them stays
    /// exactly where it was. This is the whole reason the write goes through
    /// `toml_edit` rather than through a serde round trip.
    #[test]
    fn setting_the_defaults_keeps_comments_and_every_other_key() {
        let before = "\
# how this project is laid out
[project]
repos = [\"backend\", \"frontend\"]

[defaults]
# the branch everything lands on
target_branch = \"staging\"
min_priority = 1
max_parallel_tasks = 4
review_passes = 3

[merge]
hazards = \"two branches emitting one migration number\"
";
        let wanted = Defaults {
            target_branch: Some("main".into()),
            min_priority: 3,
            max_parallel_tasks: 2,
            review_passes: 7,
        };

        let after = with_defaults(before, &wanted).expect("rewrite the defaults");

        assert!(after.contains("# how this project is laid out"), "{after}");
        assert!(after.contains("# the branch everything lands on"), "{after}");
        assert!(
            after.contains("hazards = \"two branches emitting one migration number\""),
            "{after}"
        );

        let parsed = parse(&after).expect("the result parses");
        assert_eq!(parsed.defaults, wanted);
        assert_eq!(parsed.project.repos, ["backend", "frontend"]);
        assert_eq!(
            parsed.merge.expect("the merge section").hazards.as_deref(),
            Some("two branches emitting one migration number")
        );
    }

    /// No target branch means the key is gone, not the key set to an empty
    /// string: `Option<String>` reads absence as "fall back to the branch the
    /// project is on", and "" is a branch name of length zero.
    #[test]
    fn no_target_branch_removes_the_key_rather_than_emptying_it() {
        let before = "[project]\nrepos = [\".\"]\n\n[defaults]\ntarget_branch = \"staging\"\n";
        let wanted = Defaults { target_branch: None, ..Defaults::default() };

        let after = with_defaults(before, &wanted).expect("rewrite the defaults");

        assert!(!after.contains("target_branch"), "the key is gone, not emptied: {after}");
        assert_eq!(parse(&after).expect("the result parses").defaults.target_branch, None);
    }

    /// A file whose author never wrote a `[defaults]` section still has to be
    /// editable — most files start that way, since every key in the table has a
    /// default and the setup agent leaves out what it has nothing to say about.
    #[test]
    fn a_missing_defaults_table_is_created() {
        let before = "[project]\nrepos = [\".\"]\n";
        let wanted = Defaults {
            target_branch: Some("main".into()),
            min_priority: 0,
            max_parallel_tasks: 1,
            review_passes: 1,
        };

        let after = with_defaults(before, &wanted).expect("rewrite the defaults");

        assert!(after.contains("[defaults]"), "the section is written out: {after}");
        assert_eq!(parse(&after).expect("the result parses").defaults, wanted);
    }

    /// A file that does not parse is refused rather than replaced. The menu
    /// already refuses this case, and this is the back stop: the file can
    /// change under an open window.
    #[test]
    fn a_file_that_will_not_parse_is_refused() {
        let err = with_defaults("this is not toml at all [[[", &Defaults::default())
            .expect_err("a damaged file is refused");
        assert!(!err.is_empty());
    }

    #[test]
    fn saving_writes_the_file_and_leaves_no_temporary_behind() {
        let root = temp_root("save-defaults");
        write_config(&root, "[project]\nrepos = [\".\"]\n\n[defaults]\nmin_priority = 1\n");

        let wanted = Defaults {
            target_branch: Some("main".into()),
            min_priority: 4,
            max_parallel_tasks: 2,
            review_passes: 9,
        };
        save_defaults(&root, &wanted).expect("save the defaults");

        match load(&root) {
            ConfigState::Ok { config } => assert_eq!(config.defaults, wanted),
            other => panic!("expected a loadable config, got {other:?}"),
        }

        // The temp file's name is its own per call, so we look not for a
        // particular name but for none being left in the directory at all.
        let leftovers: Vec<_> = std::fs::read_dir(root.join(".smetana"))
            .expect("read the directory")
            .filter_map(|entry| entry.ok().map(|entry| entry.file_name()))
            .filter(|name| name.to_string_lossy().ends_with(".tmp"))
            .collect();
        assert!(leftovers.is_empty(), "a temporary file was left behind: {leftovers:?}");
        std::fs::remove_dir_all(&root).expect("clean up");
    }

    /// Nothing reaches the disk over a file that will not parse — not even the
    /// four keys that would have been perfectly good. A save that left behind a
    /// half-repaired file would be the app deciding what somebody meant.
    #[test]
    fn saving_over_a_damaged_file_changes_nothing_on_disk() {
        let root = temp_root("save-defaults-damaged");
        let damaged = "this is not toml at all [[[";
        write_config(&root, damaged);

        let err = save_defaults(&root, &Defaults::default()).expect_err("a damaged file is refused");

        assert!(!err.is_empty());
        assert_eq!(
            std::fs::read_to_string(path_in(&root)).expect("the file is still there"),
            damaged
        );
        std::fs::remove_dir_all(&root).expect("clean up");
    }

    /// A file that is not there is refused rather than created: this edits four
    /// keys of a configuration, and the rest of one is the setup agent's.
    #[test]
    fn saving_over_a_file_that_is_not_there_is_refused() {
        let root = temp_root("save-defaults-missing");
        let err = save_defaults(&root, &Defaults::default()).expect_err("there is no file to edit");
        assert!(!err.is_empty());
        std::fs::remove_dir_all(&root).expect("clean up");
    }

    #[test]
    fn a_damaged_config_is_broken_and_carries_what_the_parser_said() {
        let root = temp_root("broken");
        write_config(&root, "[project]\nrepos = \"backend\"\n");
        match load(&root) {
            ConfigState::Broken { message } => {
                assert!(message.contains("repos"), "the message names the key: {message}");
            }
            other => panic!("expected Broken, got {other:?}"),
        }
        std::fs::remove_dir_all(&root).expect("clean up");
    }
}
