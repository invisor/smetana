# Runs, stage 1: the project's shape — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Teach the app what a project is made of — its repositories, the commands that mean "green", how it comes up and how it is verified — by surveying the folder for free and letting an agent turn that survey into `.smetana/project.toml`, the file every later stage of the runs feature reads.

**Architecture:** A new `src-tauri/src/runs/` module, opened with the two parts that need no worker: `config.rs` (the schema and a parse that refuses damage rather than degrading) and `survey.rs` (a deterministic one-level walk of the project folder, no agent and no tokens). One thin command over them, a new `Intent::Setup` that opens an agent session with the survey pasted in as facts, and a bundled `project-setup` skill that tells the agent what to write. The front end learns one new state — this project is not set up — and offers it on adding a project and afterwards from the project row.

**Tech Stack:** Rust (Tauri 2, serde, toml), Vue 3, vitest, cargo test.

## Global Constraints

- **All comments, test names, assertion messages, `thiserror` strings, log lines and UI copy are English.** Commit messages are Russian, matching the whole history.
- UI copy is sentence case. Identifiers in mono, prose in sans.
- Components carry no scoped CSS and no classes: every visual value is a `computed` style object of `var(--token)` references. Never a hex colour, a px value or a font literal.
- A new or changed component must be reachable from `src/views/Gallery.vue` and checked by eye in all four theme × density combinations (`?theme=dark|light`, `?density=comfortable|compact`).
- `npm test` covers pure front-end modules and stores only; `cd src-tauri && cargo test` covers Rust. There is no component test runner and no linter — do not invent one, and never call a change "tested" because the build succeeded.
- Front-end tests live in `tests/`, mirroring `src/`, and mock exactly one thing: the IPC transport, through `tests/support/ipc.js`.
- `src/stores/*.js` are the only files in `src/` allowed to know Tauri exists.
- The build target is `es2021` / `chrome100` / `safari15`. Do not reach for newer APIs.
- Design source of truth: `docs/superpowers/specs/2026-08-05-runs-design.md`.

---

## File Structure

**Created:**

| path | responsibility |
|---|---|
| `src-tauri/src/runs/mod.rs` | the module's doc comment and its submodules; nothing else lives here yet |
| `src-tauri/src/runs/config.rs` | `.smetana/project.toml`: the schema, a pure `parse`, `load` returning a `ConfigState`, and the refusal policy |
| `src-tauri/src/runs/survey.rs` | what the folder looks like before anyone configured it: repositories, toolchains, candidate commands, and `render` for the agent |
| `src-tauri/src/runs/commands.rs` | one thin `#[tauri::command]`: `project_config` |
| `src-tauri/resources/smetana/skills/project-setup/SKILL.md` | what the agent writes into the file, and what it must not guess |
| `src/stores/runs.js` | the config state for the active project; the seventh file in `src/` that knows Tauri exists |
| `src/components/run/SetupProjectModal.vue` | the ok/cancel dialog shown when a project is added unconfigured |
| `tests/stores/runs.test.js` | the store's tests, through the mocked IPC transport |

**Modified:**

| path | change |
|---|---|
| `src-tauri/Cargo.toml` | the `toml` dependency |
| `src-tauri/src/lib.rs` | `mod runs;` and `runs::commands::project_config` in the handler list |
| `src-tauri/src/agents/mod.rs` | `Intent::Setup`; `Launch.facts` |
| `src-tauri/src/agents/prompt.rs` | `build` takes `&Skills` and `facts` instead of a brainstorming path; the `Setup` arm |
| `src-tauri/src/agents/claude.rs` | passes `&launch.skills` and `launch.facts`; the join it used to compute moves into `prompt.rs` |
| `src-tauri/src/agents/codex.rs` | the same, plus reading the `project-setup` skill for `Inline` |
| `src-tauri/src/terminal/service.rs` | `Request::Create` renders the survey for a `Setup` intent and puts it on the `Launch` |
| `src/stores/projects.js` | `addProject` returns the path it added |
| `src/stores/mockBackend.js` | answers `project_config` |
| `src/components/shell/ProjectList.vue` | a `needsSetup` mark on the active row that emits `setup` |
| `src/components/index.js` | exports `SetupProjectModal` |
| `src/views/Gallery.vue` | renders it |
| `src/views/DesktopApp.vue` | loads the config on switching projects, opens the dialog on adding one, starts the `Setup` session |

**Not touched in this stage:** the board, the play buttons, `runs/service.rs`, `runs/queue.rs`, `runs/preflight.rs`. They belong to stages 3 and 4.

---

### Task 1: The config schema and its refusal policy

**Files:**
- Create: `src-tauri/src/runs/mod.rs`, `src-tauri/src/runs/config.rs`
- Modify: `src-tauri/Cargo.toml`, `src-tauri/src/lib.rs:1-8`

**Interfaces:**
- Produces: `runs::config::{ProjectConfig, Project, Defaults, Repo, Preflight, HealthCheck, Merge, Regenerate, LiveCheck, LiveCheckMode, ConfigState}`, `runs::config::CONFIG_PATH: &str`, `runs::config::parse(&str) -> Result<ProjectConfig, String>`, `runs::config::load(&Path) -> ConfigState`.

- [ ] **Step 1: Add the dependency**

```sh
cd src-tauri && cargo add toml
```

Nothing else in the tree parses TOML. JSON is already available and is the wrong format here for one reason: the `hazards` field is multi-line prose a person edits, and a format without comments or block strings would make the file hostile to the human who has to read it.

- [ ] **Step 2: Create the module and register it**

`src-tauri/src/runs/mod.rs`:

```rust
//! Everything about a *run*: a batch of tracker work carried out by agent
//! sessions, from the shape of the project it happens in to the loop that
//! sequences the batches.
//!
//! This stage holds only the two parts that need no worker at all. Reading a
//! config file and walking a folder cost milliseconds and hold no state, so
//! there is nothing for a queue to guard — the same reasoning that keeps
//! `files/` and `git.rs` out of a worker.

pub mod commands;
pub mod config;
pub mod survey;
```

Add `mod runs;` to the module list at the top of `src-tauri/src/lib.rs`, in alphabetical order (after `mod project;`). Create empty `commands.rs` and `survey.rs` files for now with a single `//!` line each, so the module compiles; Tasks 2 and 3 fill them.

- [ ] **Step 3: Write the failing tests**

Create `src-tauri/src/runs/config.rs` with only the test module below and a `use super::*;` — it will not compile until Step 5, which is the point.

```rust
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
}
```

- [ ] **Step 4: Run the tests to verify they fail**

Run: `cd src-tauri && cargo test runs::config`
Expected: FAIL — compilation errors, `cannot find function parse in this scope`.

- [ ] **Step 5: Write the schema and the loader**

Put this above the test module in `src-tauri/src/runs/config.rs`:

```rust
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
/// summary would say.
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
```

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cd src-tauri && cargo test runs::config`
Expected: PASS, six tests.

- [ ] **Step 7: Add the loader's own tests**

Append to the test module:

```rust
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
```

- [ ] **Step 8: Run the tests to verify they pass**

Run: `cd src-tauri && cargo test runs::config`
Expected: PASS, nine tests.

- [ ] **Step 9: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/src/lib.rs src-tauri/src/runs/
git commit -m "feat(runs): схема .smetana/project.toml, повреждённый файл — отказ"
```

---

### Task 2: The survey — what a folder looks like before anyone configured it

**Files:**
- Modify: `src-tauri/src/runs/survey.rs`

**Interfaces:**
- Consumes: nothing from Task 1.
- Produces: `runs::survey::{Survey, RepoSurvey}`, `runs::survey::toolchains(&[String]) -> Vec<String>`, `runs::survey::npm_scripts(&str) -> Vec<String>`, `runs::survey::candidates(&[String], &[String]) -> Vec<String>`, `runs::survey::run(&Path) -> Survey`, `runs::survey::render(&Survey) -> String`.

- [ ] **Step 1: Write the failing tests for the pure parts**

Replace `src-tauri/src/runs/survey.rs` with only this test module plus `use super::*;`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifests_name_the_toolchains_present() {
        let names: Vec<String> = ["package.json", "Cargo.toml", "README.md"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert_eq!(toolchains(&names), ["npm", "cargo"]);
    }

    #[test]
    fn the_toolchain_order_is_fixed_rather_than_the_directory_s() {
        // The rendered facts go into a prompt. A list that shuffles between
        // runs would make two identical projects read as two different ones.
        let one: Vec<String> = ["Cargo.toml", "package.json"].iter().map(|s| s.to_string()).collect();
        let other: Vec<String> = ["package.json", "Cargo.toml"].iter().map(|s| s.to_string()).collect();
        assert_eq!(toolchains(&one), toolchains(&other));
    }

    #[test]
    fn a_folder_with_no_manifest_has_no_toolchain() {
        assert!(toolchains(&["README.md".to_string()]).is_empty());
    }

    #[test]
    fn npm_scripts_come_back_sorted_and_by_name_only() {
        let json = r#"{"name":"x","scripts":{"test":"vitest","build":"vite build"}}"#;
        assert_eq!(npm_scripts(json), ["build", "test"]);
    }

    #[test]
    fn a_package_json_without_scripts_yields_none() {
        assert_eq!(npm_scripts(r#"{"name":"x"}"#), Vec::<String>::new());
        // Unreadable JSON is an ordinary outcome here, not an error: the agent
        // is about to read the folder itself anyway.
        assert_eq!(npm_scripts("not json"), Vec::<String>::new());
    }

    #[test]
    fn npm_candidates_are_the_scripts_spelled_as_commands() {
        let found = candidates(&["npm".to_string()], &["lint".to_string(), "test".to_string()]);
        assert_eq!(found, ["npm run lint", "npm run test"]);
    }

    #[test]
    fn the_other_toolchains_offer_their_conventional_commands() {
        // These are candidates, not findings: nothing here checked that the
        // project actually has clippy or a test binary. `render` says so, and
        // the agent verifies before writing them into the config.
        assert_eq!(
            candidates(&["cargo".to_string()], &[]),
            ["cargo fmt --check", "cargo clippy -- -D warnings", "cargo test"]
        );
        assert_eq!(candidates(&["go".to_string()], &[]), ["go vet ./...", "go test ./..."]);
    }

    #[test]
    fn the_rendered_facts_name_every_repository_and_say_what_is_unverified() {
        let survey = Survey {
            repos: vec![RepoSurvey {
                path: "backend".into(),
                toolchains: vec!["npm".into()],
                candidates: vec!["npm run test".into()],
            }],
            compose_files: vec!["backend/docker-compose.yml".into()],
        };
        let text = render(&survey);
        assert!(text.contains("backend"));
        assert!(text.contains("npm run test"));
        assert!(text.contains("backend/docker-compose.yml"));
        // The one thing the agent must not read as settled.
        assert!(text.contains("not verified"), "{text}");
    }

    #[test]
    fn an_empty_survey_still_renders_something_the_agent_can_act_on() {
        let text = render(&Survey::default());
        assert!(text.contains("No git repository"), "{text}");
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cd src-tauri && cargo test runs::survey`
Expected: FAIL — `cannot find function toolchains in this scope`.

- [ ] **Step 3: Implement the pure parts**

Put above the test module:

```rust
//! What a project looks like from the outside, before anyone has configured it.
//!
//! Deterministic, free, and nobody's opinion. Finding the repositories under a
//! folder and reading their manifests costs milliseconds and no tokens, so it
//! happens when a project is added rather than when a run starts. What it
//! cannot decide — which of the commands it found is a gate, what belongs in
//! `hazards`, whether a live check exists at all — is exactly what the agent
//! is for, and `render` hands it over saying so.

use std::path::Path;

use serde::Serialize;

/// Subdirectories are looked at one level deep. A folder holding four
/// repositories and a single repository both resolve at that depth, and going
/// deeper starts finding vendored checkouts, `node_modules` and worktrees.
const DEPTH_NOTE: &str = "one level";

/// The ceiling `files/model.rs` already uses for one directory's entries.
const MAX_ENTRIES: usize = 1000;

/// Manifest file name → the toolchain it means, in the order they are reported.
const MANIFESTS: [(&str, &str); 5] = [
    ("package.json", "npm"),
    ("Cargo.toml", "cargo"),
    ("go.mod", "go"),
    ("pyproject.toml", "python"),
    ("Makefile", "make"),
];

#[derive(Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Survey {
    pub repos: Vec<RepoSurvey>,
    /// Anything that looks like a stand, relative to the project root.
    pub compose_files: Vec<String>,
}

#[derive(Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoSurvey {
    /// Relative to the project root; `"."` when the root is itself a repository.
    pub path: String,
    pub toolchains: Vec<String>,
    /// Commands in the form they would be run. Candidates only.
    pub candidates: Vec<String>,
}

/// Which toolchains a directory's file names imply. The order is `MANIFESTS`'s,
/// never the directory's: these end up in a prompt, and a list that shuffles
/// between runs makes one project read as two.
pub fn toolchains(file_names: &[String]) -> Vec<String> {
    MANIFESTS
        .iter()
        .filter(|(manifest, _)| file_names.iter().any(|name| name == manifest))
        .map(|(_, toolchain)| (*toolchain).to_string())
        .collect()
}

/// The script names in a `package.json`, sorted. Anything unparseable answers
/// with none: the agent reads the folder itself, and a survey that failed on
/// one file must not fail the rest.
pub fn npm_scripts(package_json: &str) -> Vec<String> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(package_json) else {
        return Vec::new();
    };
    let Some(scripts) = value.get("scripts").and_then(|s| s.as_object()) else {
        return Vec::new();
    };
    let mut names: Vec<String> = scripts.keys().cloned().collect();
    names.sort();
    names
}

/// Commands worth offering. For npm these are the project's own scripts; for
/// the rest they are the conventions of that toolchain, which nothing here has
/// checked exist.
pub fn candidates(toolchains: &[String], npm_scripts: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for toolchain in toolchains {
        match toolchain.as_str() {
            "npm" => out.extend(npm_scripts.iter().map(|s| format!("npm run {s}"))),
            "cargo" => out.extend(
                ["cargo fmt --check", "cargo clippy -- -D warnings", "cargo test"]
                    .iter()
                    .map(|s| (*s).to_string()),
            ),
            "go" => out.extend(["go vet ./...", "go test ./..."].iter().map(|s| (*s).to_string())),
            "python" => out.push("pytest".to_string()),
            "make" => out.push("make test".to_string()),
            _ => {}
        }
    }
    out
}

/// The survey as the agent reads it. Prose rather than JSON: this goes into a
/// prompt, and the one sentence that must survive is the disclaimer — a list
/// of candidates read as a list of findings is how a `pytest` nobody installed
/// ends up in the config as a gate.
pub fn render(survey: &Survey) -> String {
    let mut out = String::from("What a scan of this folder found (");
    out.push_str(DEPTH_NOTE);
    out.push_str(" deep). The commands are candidates, not verified — they are what the\n\
                  manifests suggest, and nothing here ran any of them:\n\n");

    if survey.repos.is_empty() {
        out.push_str("No git repository was found in this folder or directly under it.\n");
    }
    for repo in &survey.repos {
        out.push_str("- ");
        out.push_str(&repo.path);
        if repo.toolchains.is_empty() {
            out.push_str(" — no manifest recognised");
        } else {
            out.push_str(" — ");
            out.push_str(&repo.toolchains.join(", "));
        }
        out.push('\n');
        for candidate in &repo.candidates {
            out.push_str("    ");
            out.push_str(candidate);
            out.push('\n');
        }
    }
    if !survey.compose_files.is_empty() {
        out.push_str("\nCompose files, which may be how this project's services come up:\n");
        for file in &survey.compose_files {
            out.push_str("- ");
            out.push_str(file);
            out.push('\n');
        }
    }
    out
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cd src-tauri && cargo test runs::survey`
Expected: PASS, nine tests.

- [ ] **Step 5: Write the failing test for the disk walk**

Append to the test module:

```rust
    #[test]
    fn a_folder_of_repositories_resolves_to_one_entry_each() {
        let root = std::env::temp_dir().join(format!(
            "smetana-survey-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock is after the Unix epoch")
                .as_nanos()
        ));
        for repo in ["backend", "frontend"] {
            std::fs::create_dir_all(root.join(repo).join(".git")).expect("create a fake repo");
        }
        std::fs::write(
            root.join("backend/package.json"),
            r#"{"scripts":{"test":"vitest","lint":"eslint ."}}"#,
        )
        .expect("write package.json");
        std::fs::write(root.join("backend/docker-compose.yml"), "services:\n").expect("write compose");
        // Not a repository: it must not appear at all.
        std::fs::create_dir_all(root.join("notes")).expect("create a plain folder");

        let survey = run(&root);

        assert_eq!(
            survey.repos.iter().map(|r| r.path.as_str()).collect::<Vec<_>>(),
            ["backend", "frontend"],
            "only git repositories, sorted"
        );
        assert_eq!(survey.repos[0].toolchains, ["npm"]);
        assert_eq!(survey.repos[0].candidates, ["npm run lint", "npm run test"]);
        assert_eq!(survey.compose_files, ["backend/docker-compose.yml"]);

        std::fs::remove_dir_all(&root).expect("clean up");
    }

    #[test]
    fn a_repository_at_the_root_is_the_monorepo_case() {
        let root = std::env::temp_dir().join(format!(
            "smetana-survey-mono-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock is after the Unix epoch")
                .as_nanos()
        ));
        std::fs::create_dir_all(root.join(".git")).expect("create the fake repo");
        std::fs::write(root.join("Cargo.toml"), "[package]\n").expect("write Cargo.toml");

        let survey = run(&root);

        assert_eq!(survey.repos.len(), 1);
        assert_eq!(survey.repos[0].path, ".");
        assert_eq!(survey.repos[0].toolchains, ["cargo"]);

        std::fs::remove_dir_all(&root).expect("clean up");
    }
```

- [ ] **Step 6: Run to verify it fails**

Run: `cd src-tauri && cargo test runs::survey`
Expected: FAIL — `cannot find function run in this scope`.

- [ ] **Step 7: Implement the walk**

Append to the implementation, above the tests:

```rust
/// The names of the files (not directories) directly inside `dir`, capped.
/// An unreadable directory answers with none — every caller here treats that
/// as "nothing recognised", which is the truthful answer.
fn file_names(dir: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(dir) else { return Vec::new() };
    entries
        .flatten()
        .take(MAX_ENTRIES)
        .filter(|entry| entry.file_type().map(|t| t.is_file()).unwrap_or(false))
        .filter_map(|entry| entry.file_name().into_string().ok())
        .collect()
}

fn survey_repo(root: &Path, relative: &str) -> RepoSurvey {
    let dir = if relative == "." { root.to_path_buf() } else { root.join(relative) };
    let names = file_names(&dir);
    let found = toolchains(&names);
    let scripts = if found.iter().any(|t| t == "npm") {
        std::fs::read_to_string(dir.join("package.json"))
            .map(|text| npm_scripts(&text))
            .unwrap_or_default()
    } else {
        Vec::new()
    };
    RepoSurvey { path: relative.to_string(), toolchains: found.clone(), candidates: candidates(&found, &scripts) }
}

const COMPOSE_NAMES: [&str; 2] = ["docker-compose.yml", "docker-compose.yaml"];

fn compose_in(root: &Path, relative: &str) -> Vec<String> {
    let dir = if relative == "." { root.to_path_buf() } else { root.join(relative) };
    COMPOSE_NAMES
        .iter()
        .filter(|name| dir.join(name).is_file())
        .map(|name| if relative == "." { (*name).to_string() } else { format!("{relative}/{name}") })
        .collect()
}

/// A directory is a repository when it holds a `.git` — a file counts as well
/// as a directory, since that is what a linked worktree has.
fn is_repo(dir: &Path) -> bool {
    dir.join(".git").exists()
}

/// The folder, one level deep. A root that is itself a repository is the
/// monorepo case and reports as `"."`; otherwise every immediate subdirectory
/// holding a `.git` is one repository. Sorted, for the same reason the
/// toolchain order is fixed.
pub fn run(root: &Path) -> Survey {
    let mut relatives: Vec<String> = Vec::new();
    if is_repo(root) {
        relatives.push(".".to_string());
    } else if let Ok(entries) = std::fs::read_dir(root) {
        let mut found: Vec<String> = entries
            .flatten()
            .take(MAX_ENTRIES)
            .filter(|entry| entry.file_type().map(|t| t.is_dir()).unwrap_or(false))
            .filter_map(|entry| entry.file_name().into_string().ok())
            .filter(|name| !name.starts_with('.'))
            .filter(|name| is_repo(&root.join(name)))
            .collect();
        found.sort();
        relatives = found;
    }

    let repos: Vec<RepoSurvey> = relatives.iter().map(|r| survey_repo(root, r)).collect();
    let mut compose_files: Vec<String> = Vec::new();
    if !is_repo(root) {
        compose_files.extend(compose_in(root, "."));
    }
    for relative in &relatives {
        compose_files.extend(compose_in(root, relative));
    }

    Survey { repos, compose_files }
}
```

- [ ] **Step 8: Run the tests to verify they pass**

Run: `cd src-tauri && cargo test runs::survey`
Expected: PASS, eleven tests.

- [ ] **Step 9: Commit**

```bash
git add src-tauri/src/runs/survey.rs
git commit -m "feat(runs): скан проекта — репозитории, тулчейны, кандидаты в гейты"
```

---

### Task 3: The command

**Files:**
- Modify: `src-tauri/src/runs/commands.rs`, `src-tauri/src/lib.rs:57-83`

**Interfaces:**
- Consumes: `runs::config::{load, ConfigState}`.
- Produces: the `project_config` command, taking `{ project: String }` and answering a `ConfigState` (`{"state":"missing"}`, `{"state":"broken","message":"…"}`, `{"state":"ok","config":{…}}`).

- [ ] **Step 1: Write the command**

`src-tauri/src/runs/commands.rs`:

```rust
//! The thin layer over `config.rs`. There is no worker to queue behind and no
//! state to guard: reading one file costs milliseconds, the same reasoning
//! that keeps `files/` and `git.rs` out of a worker.

use std::path::Path;

use super::config::{self, ConfigState};

/// Every outcome is a state, so this cannot fail: a project with no config is
/// the ordinary case, and an unreadable one is `Broken` with what the OS said.
#[tauri::command]
pub fn project_config(project: String) -> ConfigState {
    config::load(Path::new(&project))
}
```

- [ ] **Step 2: Register it**

In `src-tauri/src/lib.rs`, add `runs::commands::project_config,` to the `tauri::generate_handler![…]` list, after the `git::git_head` line.

- [ ] **Step 3: Verify it compiles and the suite is green**

Run: `cd src-tauri && cargo test`
Expected: PASS, the whole suite.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/runs/commands.rs src-tauri/src/lib.rs
git commit -m "feat(runs): команда project_config"
```

---

### Task 4: The `Setup` intent

**Files:**
- Modify: `src-tauri/src/agents/mod.rs:69-91`, `src-tauri/src/agents/prompt.rs`, `src-tauri/src/agents/claude.rs:38-58`, `src-tauri/src/agents/codex.rs:37-62`, `src-tauri/src/terminal/service.rs:395-410`

**Interfaces:**
- Consumes: `runs::survey::{run, render}`.
- Produces: `agents::Intent::Setup`, `agents::Launch.facts: Option<String>`, and a changed signature — `prompt::build(intent: &Intent, delivery: SkillDelivery, skills: &library::Skills, facts: Option<&str>, text: SkillText) -> Option<String>`.

The signature change replaces the `brainstorming: &Path` parameter. Both profiles currently compute `launch.skills.superpowers.join("skills/brainstorming")` before calling; that join moves into `prompt.rs`, so passing the whole `Skills` removes a duplicated line rather than adding a parameter.

- [ ] **Step 1: Write the failing tests**

In `src-tauri/src/agents/prompt.rs`'s test module, replace the `fn path()` helper with:

```rust
    fn skills() -> crate::agents::library::Skills {
        crate::agents::library::Skills {
            smetana: PathBuf::from("/app/resources/smetana"),
            superpowers: PathBuf::from("/app/resources/superpowers"),
            superpowers_installed: false,
        }
    }
```

Then update every existing `build(...)` call in that module: `&path()` becomes `&skills()`, and a `None` is inserted for `facts` before the `SkillText` argument. The one test that asserts on the brainstorming path (`auto_on_an_inline_harness_points_at_the_file_rather_than_pasting_it`) keeps its expected string unchanged — the join now happens inside `build` and produces the same path.

Add:

```rust
    const FACTS: &str = "- backend — npm\n    npm run test\n";

    #[test]
    fn setting_a_project_up_carries_the_survey_and_names_the_file_to_write() {
        let text =
            build(&Intent::Setup, SkillDelivery::PluginDir, &skills(), Some(FACTS), nothing())
                .expect("a setup session opens on something");
        assert!(text.contains(".smetana/project.toml"), "{text}");
        assert!(text.contains("npm run test"), "the survey reaches the agent: {text}");
    }

    #[test]
    fn a_plugin_dir_harness_is_told_the_setup_skill_by_name() {
        let text =
            build(&Intent::Setup, SkillDelivery::PluginDir, &skills(), Some(FACTS), nothing())
                .expect("builds");
        assert!(text.contains("smetana:project-setup"), "{text}");
    }

    #[test]
    fn an_inline_harness_is_given_the_setup_skill_s_path_rather_than_its_body() {
        // The same choice `Auto` already makes for brainstorming: a path costs
        // one line, the body costs kilobytes the session may never need.
        let text = build(
            &Intent::Setup,
            SkillDelivery::Inline,
            &skills(),
            Some(FACTS),
            SkillText { filing: Some(FILING), brainstorming: Some(BRAINSTORMING) },
        )
        .expect("builds");
        assert!(text.contains("/app/resources/smetana/skills/project-setup/SKILL.md"), "{text}");
        assert!(!text.contains("The title says what needs doing"), "nothing is filed here");
    }

    #[test]
    fn a_setup_session_survives_a_survey_that_found_nothing() {
        // `render` always produces text, but a caller that could not run the
        // survey at all passes None, and the instruction still has to stand.
        let text = build(&Intent::Setup, SkillDelivery::PluginDir, &skills(), None, nothing())
            .expect("builds");
        assert!(text.contains(".smetana/project.toml"), "{text}");
    }
```

Add to `src-tauri/src/agents/mod.rs`'s test module:

```rust
    #[test]
    fn a_setup_intent_deserializes_from_the_front_ends_json() {
        let intent: Intent = serde_json::from_str(r#"{"kind":"setup"}"#).expect("deserializes");
        assert!(matches!(intent, Intent::Setup));
    }
```

- [ ] **Step 2: Run to verify they fail**

Run: `cd src-tauri && cargo test agents`
Expected: FAIL — `no variant named Setup`, and arity errors on `build`.

- [ ] **Step 3: Add the variant and the field**

In `src-tauri/src/agents/mod.rs`, add to `Intent`:

```rust
    /// Work out what this project is made of and write
    /// `.smetana/project.toml`. Started from the dialog a person gets when
    /// they add a project, and from the project row afterwards.
    Setup,
```

and to `Launch`:

```rust
    /// What a survey of the project found, already rendered. Only a `Setup`
    /// intent has any, and it is read by the caller for the same reason skill
    /// text is: `prompt.rs` stays pure and the disk stays outside it.
    pub facts: Option<String>,
```

Fix the two `fn launch(...)` test helpers in `claude.rs` and `codex.rs` and the one in `terminal/pty.rs` by adding `facts: None` to each struct literal.

- [ ] **Step 4: Change `build` and add the `Setup` arm**

In `src-tauri/src/agents/prompt.rs`:

```rust
use super::library::Skills;

/// What the agent is told to produce when a project has no configuration yet.
/// The file's path is named here rather than left to the skill: a session that
/// could not read the skill must still write to the right place.
const SETUP: &str = "Work out what this project is made of and write .smetana/project.toml — \
     the file Smetana reads before it runs anything here. Check the commands before you write \
     them in, and ask me about anything the folder does not answer.";

pub fn build(
    intent: &Intent,
    delivery: SkillDelivery,
    skills: &Skills,
    facts: Option<&str>,
    text: SkillText,
) -> Option<String> {
    let brainstorming = skills.superpowers.join("skills/brainstorming");
    match intent {
        Intent::Bare => None,
        Intent::EditTask { id, title } => Some(format!("Update bd issue {id} (\"{title}\"): ")),
        Intent::NewTask { brainstorm, draft } => {
            Some(new_task(*brainstorm, draft, delivery, &brainstorming, text))
        }
        Intent::Setup => Some(setup(delivery, skills, facts)),
    }
}

fn setup(delivery: SkillDelivery, skills: &Skills, facts: Option<&str>) -> String {
    let mut out = String::from(SETUP);
    out.push_str("\n\n");
    match delivery {
        SkillDelivery::PluginDir => {
            out.push_str("Use the smetana:project-setup skill — it says what the file holds.");
        }
        SkillDelivery::Inline => {
            let skill = skills.smetana.join("skills/project-setup/SKILL.md");
            let _ = write!(
                out,
                "What the file holds is described at {} — read it first.",
                skill.display()
            );
        }
    }
    if let Some(facts) = facts {
        out.push_str("\n\n");
        out.push_str(facts.trim_end());
    }
    out
}
```

Keep `new_task`'s own signature as it is — it already takes the brainstorming path.

- [ ] **Step 5: Update the two profiles**

In `claude.rs`, replace the block that computes `brainstorming` and calls `build` with:

```rust
        let text = prompt::SkillText { filing: None, brainstorming: None };
        if let Some(built) =
            prompt::build(&launch.intent, self.delivery(), &launch.skills, launch.facts.as_deref(), text)
        {
            cmd.arg(built);
        }
```

In `codex.rs`, drop the `let brainstorming = …join(…)` line and call:

```rust
        if let Some(built) =
            prompt::build(&launch.intent, self.delivery(), &launch.skills, launch.facts.as_deref(), text)
        {
            cmd.arg(built);
        }
```

- [ ] **Step 6: Run to verify the agent tests pass**

Run: `cd src-tauri && cargo test agents`
Expected: PASS.

- [ ] **Step 7: Render the survey where the `Launch` is built**

In `src-tauri/src/terminal/service.rs`, inside `Request::Create`, above the `let launch = …`:

```rust
            // Only a Setup session pays for the walk, and it happens here
            // rather than in the front end so that what the agent is told is
            // what the disk says at the moment the session starts.
            let facts = matches!(intent, agents::Intent::Setup)
                .then(|| crate::runs::survey::render(&crate::runs::survey::run(Path::new(&project))));
```

and add `facts,` to the `agents::Launch { … }` literal.

- [ ] **Step 8: Run the whole suite**

Run: `cd src-tauri && cargo test`
Expected: PASS.

- [ ] **Step 9: Commit**

```bash
git add src-tauri/src/agents src-tauri/src/terminal/service.rs src-tauri/src/terminal/pty.rs
git commit -m "feat(agents): интент Setup — сессия настройки проекта с фактами скана"
```

---

### Task 5: The `project-setup` skill

**Files:**
- Create: `src-tauri/resources/smetana/skills/project-setup/SKILL.md`

**Interfaces:**
- Consumes: nothing. It is read by the agent, by name (`smetana:project-setup`) or by path.
- Produces: nothing in code. `tauri.conf.json` already bundles `resources/smetana/**/*`, so a new skill directory needs no build change.

- [ ] **Step 1: Write the skill**

```markdown
---
name: project-setup
description: Use when Smetana asks you to set a project up for runs — writing .smetana/project.toml from what the folder actually contains
---

# Setting a project up for runs

Smetana runs tracker work by starting agent sessions in git worktrees, merging
what they produce into one branch, and closing the task. Before it can do any
of that it has to know four things about this project, and only you can find
them out: which repositories it is made of, what "green" means in each of them,
how the project comes up, and how a finished piece of work is verified.

You write that into `.smetana/project.toml`, beside `.beads/`. The prompt you
were given already carries a scan of the folder — repositories, manifests and
the commands those manifests suggest. **Those commands are candidates, not
findings. Nothing ran them.**

## What the file holds

    [project]
    repos = ["backend", "frontend"]   # relative paths; a single repository is ["."]

    [defaults]
    target_branch = "staging"          # what the run dialog offers first
    min_priority = 2                   # tasks below this are not taken automatically
    max_parallel_tasks = 3
    review_passes = 5

    [repo.backend]
    setup = "npm install"              # run once in a fresh worktree
    gates = ["npm run typecheck", "npm test"]
    env_files = [".env"]               # copied in from the main checkout

    [preflight]
    commands = ["docker compose up -d"]
    health = [{ url = "http://localhost:4001/health" }, { tcp = 5433 }]

    [merge]
    hazards = """
    Prose. See below.
    """

    [[merge.regenerate]]
    paths = ["admin/src/api-types.ts"]
    command = "npm run generate:api-types"

    [live_check]
    mode = "browser"                   # browser | command | none

Only `[project]` is required. Every other section may be left out, and leaving
one out is better than filling it with a guess.

## How to fill it in

**Verify every gate before you write it.** Run it. A gate that does not exist,
or that is red on a clean checkout, is worse than no gate: every merge will
either fail for a reason nobody caused or pass having proved nothing. If a
command is red on the untouched project, say so to the person rather than
writing it in.

**Order `repos` by what depends on what.** Whatever produces an API contract
merges before whatever consumes it. In a single-repository project this is
`["."]` and there is nothing to order.

**`hazards` is for what a rule cannot express.** It is read by the agent that
merges, after every merge, and it is where you record the things git does not
flag: two branches emitting a migration with the same number off one base,
generated files that must be regenerated rather than merged, a lockfile whose
clean merge installs something different. Write it as instructions to someone
who has just arrived. Leave it out if the project genuinely has none.

**`regenerate` is for what a rule can express**: a path that is never merged by
hand, and the command that reproduces it.

**`live_check`** is how a merged task is verified beyond its tests.
`mode = "browser"` when there is a UI a person would click through — then use
`notes` to say how the stand comes up and how to sign in. `mode = "command"`
with a `command` when there is an end-to-end suite instead. `mode = "none"`
when there is neither; the toggle in Smetana then says so rather than
pretending.

## Ask about what the folder cannot answer

The scan cannot tell you which branch work should merge into, whether a red
command is expected, or what breaks quietly in this codebase. Ask — one
question at a time. This file is read by every run from now on, and a guess in
it is a guess repeated nightly.

## When you are done

Write the file, then show the person what you wrote and what you could not
determine. Do not start any work on the tracker: setting the project up is the
whole task.
```

- [ ] **Step 2: Check it is where the app looks for it**

Run: `ls src-tauri/resources/smetana/skills/project-setup/SKILL.md`
Expected: the path exists. `library::read_skill` joins `skills/<name>/SKILL.md`, and `prompt.rs` names the same path for `Inline`.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/resources/smetana/skills/project-setup
git commit -m "feat(skills): project-setup — как собрать .smetana/project.toml"
```

---

### Task 6: The front-end store

**Files:**
- Create: `src/stores/runs.js`, `tests/stores/runs.test.js`
- Modify: `tests/support/stores.js:24-40`

**Interfaces:**
- Consumes: the `project_config` command.
- Produces: `runsState` (`{ project, config }` where `config` is the `ConfigState` shape), `loadConfig(project)`, `needsSetup` (computed boolean), `configError` (computed string or null).

- [ ] **Step 1: Add the store to the test graph**

In `tests/support/stores.js`, add `import('../../src/stores/runs.js')` to the `Promise.all` array and `runs` to both the destructuring and the returned `stores` object, keeping the existing order convention (append it last, after `git`).

- [ ] **Step 2: Write the failing tests**

`tests/stores/runs.test.js`:

```js
import { describe, expect, it } from 'vitest'
import { loadStores } from '../support/stores.js'

const OK = { state: 'ok', config: { project: { repos: ['.'] } } }

describe('the active project\'s run configuration', () => {
  it('a configured project needs no setup', async () => {
    const { ipc, stores } = await loadStores()
    ipc.on('project_config', OK)

    await stores.runs.loadConfig('/p')

    expect(stores.runs.runsState.config.state).toBe('ok')
    expect(stores.runs.needsSetup.value).toBe(false)
    expect(stores.runs.configError.value).toBe(null)
    expect(ipc.calls('project_config')).toEqual([{ project: '/p' }])
  })

  it('a project with no file needs setup, and that is not an error', async () => {
    const { ipc, stores } = await loadStores()
    ipc.on('project_config', { state: 'missing' })

    await stores.runs.loadConfig('/p')

    expect(stores.runs.needsSetup.value).toBe(true)
    // Missing is the ordinary case: every project starts here, and nothing
    // about it belongs in a toast.
    expect(stores.runs.configError.value).toBe(null)
  })

  it('a damaged file is an error, and not an invitation to overwrite it', async () => {
    const { ipc, stores } = await loadStores()
    ipc.on('project_config', { state: 'broken', message: 'unknown field `gate`' })

    await stores.runs.loadConfig('/p')

    expect(stores.runs.configError.value).toContain('gate')
    // The setup dialog must not be offered for a file that exists: the agent
    // would write over something the person cannot currently read.
    expect(stores.runs.needsSetup.value).toBe(false)
  })

  it('with no project there is nothing to ask about', async () => {
    const { ipc, stores } = await loadStores()
    ipc.on('project_config', OK)
    await stores.runs.loadConfig('/p')

    await stores.runs.loadConfig(null)

    expect(stores.runs.needsSetup.value).toBe(false)
    expect(stores.runs.runsState.config.state).toBe('missing')
    expect(ipc.calls('project_config')).toEqual([{ project: '/p' }])
  })

  it('a response for the project we already left is dropped', async () => {
    // The same guard git.js and terminals.js carry: two calls in flight have no
    // ordering guarantee, and without this the last response would win rather
    // than the last call — one project's configuration under another's name.
    const { ipc, stores } = await loadStores()
    const answers = { '/slow': OK, '/fast': { state: 'missing' } }
    ipc.on('project_config', ({ project }) => answers[project])

    const slow = stores.runs.loadConfig('/slow')
    const fast = stores.runs.loadConfig('/fast')
    await Promise.all([slow, fast])

    expect(stores.runs.runsState.project).toBe('/fast')
    expect(stores.runs.runsState.config.state).toBe('missing')
  })

  it('a failed command leaves no stale configuration behind', async () => {
    const { ipc, stores } = await loadStores()
    ipc.on('project_config', OK)
    await stores.runs.loadConfig('/p')

    ipc.fail('project_config', new Error('nope'))
    await stores.runs.loadConfig('/other')

    expect(stores.runs.runsState.config.state).toBe('missing')
  })
})
```

- [ ] **Step 3: Run to verify they fail**

Run: `npm test -- runs`
Expected: FAIL — cannot resolve `src/stores/runs.js`.

- [ ] **Step 4: Write the store**

`src/stores/runs.js`:

```js
/* Whether the active project is set up for runs, and with what. The seventh
   file in this directory that knows Tauri exists; components see a reactive
   object and two computeds.

   Deliberately small, like git.js: this is a file read, there is no worker
   behind it, and freshness comes from switching projects and from a setup
   session finishing. */
import { computed, reactive } from 'vue'
import { invoke } from '@tauri-apps/api/core'

const NONE = { state: 'missing' }

export const runsState = reactive({
  project: null,
  /* The back end's own state object: missing | broken | ok. Kept as it
     arrived rather than unpacked into flags, so a state this front end has
     not heard of cannot silently read as one of the others. */
  config: NONE
})

/* An offer to set the project up, not a warning: most projects are here, and
   `broken` is deliberately excluded — a file that exists and cannot be parsed
   is something to fix, and running the setup over it would write across
   somebody's work. */
export const needsSetup = computed(() => runsState.config.state === 'missing')

export const configError = computed(() =>
  runsState.config.state === 'broken' ? runsState.config.message : null
)

/* Guarded against its own stale response exactly as git.js and terminals.js
   are: two calls can be in flight with no ordering guarantee, and the last
   response winning over the last call would show one project's configuration
   under another project's name. */
export async function loadConfig(project) {
  runsState.project = project
  if (!project) {
    runsState.config = NONE
    return
  }
  try {
    const config = await invoke('project_config', { project })
    if (runsState.project !== project) return
    runsState.config = config
  } catch (err) {
    if (runsState.project !== project) return
    /* Not a folder's fault: every real outcome is a state, so reaching here
       means the call itself failed. We fall back to "not configured", which
       offers the setup rather than claiming a configuration we do not have. */
    console.error('[runs] reading the project config failed:', err)
    runsState.config = NONE
  }
}
```

- [ ] **Step 5: Run to verify they pass**

Run: `npm test -- runs`
Expected: PASS, six tests.

- [ ] **Step 6: Commit**

```bash
git add src/stores/runs.js tests/stores/runs.test.js tests/support/stores.js
git commit -m "feat(runs): стор конфигурации проекта со сторожем от устаревшего ответа"
```

---

### Task 7: The setup dialog

**Files:**
- Create: `src/components/run/SetupProjectModal.vue`
- Modify: `src/components/index.js:49-72`, `src/views/Gallery.vue`

**Interfaces:**
- Consumes: `Modal`, `Button` from the component library.
- Produces: `<SetupProjectModal :open :name :busy @close @confirm />`, where `name` is the project's folder name.

- [ ] **Step 1: Write the component**

`src/components/run/SetupProjectModal.vue`:

```vue
<script setup>
import { computed } from 'vue'
import Modal from '../overlays/Modal.vue'
import Button from '../core/Button.vue'

/* Shown once, when a project is added and has no .smetana/project.toml. It
   states what will happen before anything happens: a session starts, a folder
   is read, and a file appears in the person's repository. None of that should
   arrive unannounced — adding a project to a list is otherwise a read. */
const props = defineProps({
  open: { type: Boolean, default: false },
  name: { type: String, default: '' },
  busy: { type: Boolean, default: false }
})

defineEmits(['close', 'confirm'])

const description = computed(() =>
  props.name
    ? `${props.name} has no run configuration yet.`
    : 'This project has no run configuration yet.'
)

const body = {
  display: 'flex',
  flexDirection: 'column',
  gap: 'var(--space-4)',
  fontSize: 'var(--text-sm)',
  lineHeight: 'var(--leading-normal)',
  color: 'var(--text-secondary)'
}
const pathStyle = {
  font: 'var(--weight-medium) var(--text-xs)/1 var(--font-mono)',
  color: 'var(--text-primary)'
}
</script>

<template>
  <Modal
    :open="open"
    :closable="!busy"
    title="Set this project up?"
    :description="description"
    @close="$emit('close')"
  >
    <div :style="body">
      <p>
        An agent will look through the folder — its repositories, their manifests and scripts —
        and write what it finds to <span :style="pathStyle">.smetana/project.toml</span>.
        It will ask about anything the folder does not answer.
      </p>
      <p>Nothing else is changed, and you can review the file before any run uses it.</p>
    </div>
    <template #footer>
      <Button variant="ghost" :disabled="busy" @click="$emit('close')">Cancel</Button>
      <Button variant="primary" :disabled="busy" @click="$emit('confirm')">
        {{ busy ? 'Starting…' : 'Set up' }}
      </Button>
    </template>
  </Modal>
</template>
```

- [ ] **Step 2: Export and gallery it**

In `src/components/index.js`, beside the other overlay exports:

```js
export { default as SetupProjectModal } from './run/SetupProjectModal.vue'
```

In `src/views/Gallery.vue`, add `SetupProjectModal` to the import list (alphabetical) and render it next to `NewTaskModal`, inside the same kind of clipping box:

```vue
      <div :style="{ position: 'relative', height: '320px', border: 'var(--border-w) solid var(--border)', overflow: 'hidden' }">
        <SetupProjectModal :open="true" name="holiday-curb" @close="() => {}" @confirm="() => {}" />
      </div>
```

- [ ] **Step 3: Check it by eye**

Run: `npm run dev`, then open each of
`http://localhost:5173/?view=gallery&theme=dark&density=comfortable`,
`…&theme=dark&density=compact`, `…&theme=light&density=comfortable`, `…&theme=light&density=compact`.
Expected: the dialog reads correctly in all four; no hardcoded colour or size gives it away in light mode; the mono path sits on the sans prose without changing the line height.

- [ ] **Step 4: Commit**

```bash
git add src/components/run src/components/index.js src/views/Gallery.vue
git commit -m "feat(components): SetupProjectModal — предложение настроить проект"
```

---

### Task 8: Wiring — on adding a project, and afterwards

**Files:**
- Modify: `src/stores/projects.js:110-145`, `src/stores/mockBackend.js`, `src/components/shell/ProjectList.vue:14-25,96-108`, `src/views/DesktopApp.vue`

**Interfaces:**
- Consumes: `runs.js` (`loadConfig`, `needsSetup`, `runsState`), `SetupProjectModal`, `createSession`.
- Produces: nothing further stages depend on beyond `addProject` now returning `string | null`.

- [ ] **Step 1: Make `addProject` say what it added**

In `src/stores/projects.js`, change the tail of `addProject` so every path returns a value:

```js
  moving = true
  try {
    if (path !== settings.activeProject && !(await confirmUnsaved())) return null
    await flushPending()
    if (!settings.openProjects.includes(path)) settings.openProjects.push(path)
    if (path === settings.activeProject) return path
    await moveTo(path)
    return path
  } finally {
    moving = false
  }
```

and the three early exits above it (`if (moving) return null`, the failed picker, `if (!picked) return null`, the second `if (moving) return null`). The caller needs to tell "added" from "cancelled", and a bare `return` reads as the latter.

- [ ] **Step 2: Answer the command in browser mode**

`src/stores/mockBackend.js` dispatches with a chain of `if (command === '…')` inside `mockIPC((command, payload) => …)`. Add one beside the other read commands, before the write rejections:

```js
    /* The first mock project is set up, the second is not: without one of each
       there is nowhere to see either state under npm run dev. */
    if (command === 'project_config') {
      return payload?.project === MOCK_PROJECTS[0]
        ? { state: 'ok', config: { project: { repos: ['.'] } } }
        : { state: 'missing' }
    }
```

`terminal_create` with a `setup` intent is deliberately left to fall through to the existing loud rejection: a browser has no agent to start, and a dialog that closed as if it had would be worse than one that fails.

- [ ] **Step 3: Mark the row**

In `src/components/shell/ProjectList.vue`, add a prop beside `canAddAgent`:

```js
  /* Only ever about the active row, for the same reason canAddAgent is: the
     configuration is read on switching projects, and probing every row would
     be a command per project for a mark nobody is looking at. */
  needsSetup: { type: Boolean, default: false }
```

add `'setup'` to `defineEmits`, and render the marker before the "New agent" button:

```vue
        <Tooltip v-if="needsSetup && p.path === activePath" label="Not set up for runs" side="right">
          <IconButton icon="settings-2" label="Set up for runs" size="sm" @click.stop="emit('setup', p.path)" />
        </Tooltip>
```

Register `settings-2` in `src/components/core/icons.js` if it is not there: import `Settings2` from `lucide-vue-next` and add the `'settings-2': Settings2` entry, keeping the alphabetical order of that map.

- [ ] **Step 4: Wire the view**

In `src/views/DesktopApp.vue`:

```js
import SetupProjectModal from '../components/run/SetupProjectModal.vue'
import { loadConfig, needsSetup, runsState } from '../stores/runs.js'

/* The project whose setup is being offered. Null when the dialog is closed —
   it is asked about one project at a time, and the path is what the session
   needs. */
const setupFor = ref(null)
const settingUp = ref(false)

/* Adding a project is a read until this point: the dialog is where it becomes
   a session and a file in somebody's repository. */
const onAddProject = async () => {
  const added = await addProject()
  if (!added) return
  await loadConfig(added)
  if (needsSetup.value) setupFor.value = added
}

const startSetup = async () => {
  const path = setupFor.value
  if (!path || settingUp.value) return
  settingUp.value = true
  try {
    project.sideTab = 'agents'
    project.activeTab = 'terminal'
    await createSession(path, { kind: 'setup' })
    setupFor.value = null
  } catch {
    // already reported by createSession; the dialog stays open
  } finally {
    settingUp.value = false
  }
}
```

Replace the two `@click="addProject"` call sites (the panel's "+" `IconButton` and the `no-project` empty state's button) with `@click="onAddProject"`.

Load the configuration whenever the project changes, beside the existing per-project effects — add `loadConfig(activePath.value)` to the same `watch`/`watchEffect` that already reacts to `activePath` (the one that calls `loadHead`), so a switch brings the new project's state and clears the old one's.

Pass the mark and handle the row's request:

```vue
          <ProjectList
            …
            :needs-setup="needsSetup"
            @setup="setupFor = $event"
          />
```

and render the dialog beside `NewTaskModal`:

```vue
        <SetupProjectModal
          :open="!!setupFor"
          :name="setupFor ? basenameOf(setupFor) : ''"
          :busy="settingUp"
          @close="setupFor = null"
          @confirm="startSetup"
        />
```

`basenameOf` already exists in this file for the unsaved-changes dialog; if it is scoped differently, use `basename` from `../stores/projects.js`, which is exported.

- [ ] **Step 5: Run the front-end suite**

Run: `npm test`
Expected: PASS. If a projects-store test asserted on `addProject`'s return value being undefined, update it to the path — that is the change this task makes.

- [ ] **Step 6: Check it by eye in the real app**

Run: `npm run tauri dev`
Expected, in order:
1. Add a folder with no `.smetana/project.toml` → the dialog appears naming that folder.
2. Cancel → no session starts, the project is in the list, and the active row carries the setup button.
3. Press that button → the dialog again; confirm → the side panel switches to Agents, the centre to Terminal, and a session starts with the survey in its prompt.
4. Switch to a project that has a config → no dialog, no mark.

- [ ] **Step 7: Commit**

```bash
git add src/stores/projects.js src/stores/mockBackend.js src/components/shell/ProjectList.vue src/components/core/icons.js src/views/DesktopApp.vue
git commit -m "feat(runs): предложение настроить проект при добавлении и с его строки"
```

---

## What this stage does not do

- No play buttons, no run dialog, no worker. A configured project looks exactly
  as it does today; the file it now has is read by nothing yet.
- No editing the config from inside the app. It is a file in the project, and
  the editor in the centre column already opens it.
- No re-survey when a project's shape changes. The person runs the setup again
  from the row, and the agent rewrites the file.

## Self-review notes

Checked against `docs/superpowers/specs/2026-08-05-runs-design.md`:

- The spec says an unconfigured project shows "the same calm card where the
  board would be". **That is wrong and is not built here.** Most projects will
  be unconfigured, and the board is the app's main screen — replacing it would
  take the app away to advertise a feature. The mark on the project row and,
  from stage 4, the disabled play button with its reason, carry it instead. The
  spec's "The survey runs when a project is added" section should be corrected
  to match.
- Everything else in that section — the survey costing nothing, no writes and
  no session on open, the dialog on adding, the agent writing the file, an
  unconfigured project being an ordinary state — is implemented by Tasks 2, 5,
  7 and 8.
- `deny_unknown_fields` is stricter than the spec spells out; it follows from
  the spec's refusal policy, and Task 1's test says why.
