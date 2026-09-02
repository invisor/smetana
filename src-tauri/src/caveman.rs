//! Caveman, and whether it is on this machine.
//!
//! Caveman (<https://github.com/JuliusBrussee/caveman>) is somebody else's
//! layer between a CLI agent and its provider: it shortens what the agent reads
//! and writes, so a night of work costs fewer tokens. This app has no part in
//! putting it there. What it needs is the ability to say whether it is there,
//! because a screen that offers to configure something absent, or stays silent
//! about something wired into the person's own `~/.claude/settings.json`, is
//! wrong in both directions.
//!
//! # The truth is on the disk, not in a file of ours
//!
//! It sits beside [`crate::autostart`] for the reason that module's header
//! gives, and it is the second module in this tree with that shape: the fact is
//! the machine's, so nothing here is stored, cached or mirrored into
//! `settings.json`. Caveman can be installed, wired, unwired and removed
//! entirely from outside this app — `caveman setup`, `caveman hooks remove`, or
//! a person editing `~/.claude/settings.json` by hand — and a copy of ours
//! would then disagree with the machine with no way to tell which half is
//! stale. So every answer is read fresh, from four ordinary files, and there is
//! no reconciliation to design.
//!
//! # Files rather than a process
//!
//! Nothing here spawns anything. `caveman --version` would answer the same
//! question at the price of somebody else's CLI start-up, a `PATH` this app did
//! not set, and a "did not answer in time" branch to design — for a fact that
//! is already lying on the disk. The four sources, all plain files:
//!
//! - `~/.caveman/bin/.bin-manifest.json` — the binaries are laid out;
//! - `~/.caveman/integrations/claude.json` — caveman's own journal of what it
//!   replaced in Claude Code's configuration, carrying `pack_version`,
//!   `installed_at`, `detected_agent_version` and one entry per replaced file
//!   with its backup and its sha256 before and after;
//! - `~/.claude/settings.json`, the `hooks` section — whether it is wired in
//!   **right now**. The journal above describes what was done once and can
//!   disagree with the file, which is exactly why both are read;
//! - `<project>/.claude/skills/caveman/` — the project skill, the one form of
//!   caveman that is not global: no proxy, no hooks, one rule file in the
//!   repository.
//!
//! # Four states rather than a boolean
//!
//! The shape [`crate::tracker::access::AccessRepair`] already has, for its
//! reason: what to do about each is different. `absent` has nothing to say
//! about; `binaries-only` is installed and switched off, so the offer is to
//! switch it on; `wired` is working globally, so the offer is to say what it
//! changed; `project-skill-only` is the cheap form, in this repository alone.
//! A boolean would glue the first two together, and those are the two whose
//! advice differs most.

use std::path::Path;

use serde::Serialize;

/// Where caveman keeps itself, under the person's home directory.
const HOME_DIR: &str = ".caveman";

/// The binaries' own manifest. Its presence is what "the binaries are laid
/// out" means; its contents are caveman's business and are never read here.
const BIN_MANIFEST: &str = "bin/.bin-manifest.json";

/// Caveman's journal of what it replaced in Claude Code's configuration.
const CLAUDE_JOURNAL: &str = "integrations/claude.json";

/// Claude Code's own settings, which is where a hook is wired in.
const CLAUDE_SETTINGS: &str = ".claude/settings.json";

/// The project skill, relative to a project's root.
const PROJECT_SKILL: &str = ".claude/skills/caveman";

/// The word a hook's command has to carry to be caveman's. Matched
/// case-insensitively: on Windows the invocation is a path, and a path's case
/// is the filesystem's opinion rather than caveman's.
const MARK: &str = "caveman";

/// How caveman stands on this machine, for this project.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CavemanState {
    /// Neither `~/.caveman` nor a project skill. Nothing is installed.
    Absent,
    /// Installed globally, and no hook in `~/.claude/settings.json` calls it:
    /// present and switched off.
    BinariesOnly,
    /// A hook calls it, so it is working globally right now.
    Wired,
    /// Nothing global, and this project carries `.claude/skills/caveman/`.
    ProjectSkillOnly,
}

/// The answer, which is the state and the facts a line on a screen would need
/// beside it.
///
/// The three facts come from the journal and are all `Option`/empty when it is
/// missing or unreadable, which is an ordinary outcome rather than a failure.
/// `replacedFiles` is here because it is the one thing this app can say that
/// nothing else says: somebody is entitled to see which of their own
/// configuration files another installer rewrote.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Caveman {
    pub state: CavemanState,
    /// `pack_version` from the journal — which release of caveman's native pack
    /// was applied.
    pub pack_version: Option<String>,
    /// `detected_agent_version` from the journal — the Claude Code it was
    /// applied against. Null in the journal itself when caveman could not tell,
    /// so null here means both "no journal" and "the journal does not know",
    /// and neither is worth distinguishing on a screen.
    pub detected_agent_version: Option<String>,
    /// The files the journal says were replaced, in the order it lists them.
    pub replaced_files: Vec<String>,
}

impl Caveman {
    /// A state with no journal behind it.
    fn bare(state: CavemanState) -> Self {
        Caveman {
            state,
            pack_version: None,
            detected_agent_version: None,
            replaced_files: Vec::new(),
        }
    }
}

/// What the settings window asks. `project` is needed for the fourth state
/// alone: the other three are facts about the machine.
#[tauri::command]
pub fn caveman_state(project: String) -> Caveman {
    read(dirs::home_dir().as_deref(), Path::new(&project))
}

/// The whole of the reading, with both roots handed in.
///
/// They are parameters rather than reads of their own so that this is testable
/// without the home directory of whichever machine `cargo test` runs on —
/// [`crate::tracker::access::repair_for`] takes its home for the same reason.
/// `None` is a home directory there is no saying about, and it answers as
/// "nothing global" rather than as an error: a machine with no home has no
/// `~/.caveman` either, and a refusal here would only turn a fact nobody can
/// read into a sentence nobody can act on.
fn read(home: Option<&Path>, project: &Path) -> Caveman {
    let caveman_home = home.map(|home| home.join(HOME_DIR));
    let journal = caveman_home.as_deref().and_then(read_journal);

    let state = if wired(home) {
        // The hooks file is the only source that speaks about right now. It is
        // asked first for exactly that reason: a journal describing an install
        // that has since been unwired must not outvote it, and neither must a
        // `~/.caveman` somebody emptied by hand.
        CavemanState::Wired
    } else if installed_globally(caveman_home.as_deref()) {
        CavemanState::BinariesOnly
    } else if project.join(PROJECT_SKILL).is_dir() {
        CavemanState::ProjectSkillOnly
    } else {
        CavemanState::Absent
    };

    match journal {
        Some(journal) => Caveman {
            state,
            pack_version: journal.pack_version,
            detected_agent_version: journal.detected_agent_version,
            replaced_files: journal.operations.into_iter().filter_map(|op| op.file).collect(),
        },
        None => Caveman::bare(state),
    }
}

/// Is caveman installed globally at all?
///
/// `~/.caveman` is the whole of the question, and the manifest is asked first
/// only because it is the more specific fact — the binaries genuinely laid out,
/// rather than a directory that might be a login and a cache. Neither outvotes
/// the other, and the manifest lives inside the directory, so anything the
/// first check finds the second finds too. What the pair says is that this
/// state is *installed and switched off* rather than *binaries and nothing
/// else*: a `~/.caveman` with an interrupted install inside it is still not
/// `absent`, which is defined as the directory not being there.
fn installed_globally(caveman_home: Option<&Path>) -> bool {
    caveman_home.is_some_and(|home| home.join(BIN_MANIFEST).is_file() || home.is_dir())
}

/// Does any hook in `~/.claude/settings.json` call caveman?
///
/// Unreadable, absent or not JSON at all all answer no. A settings file this
/// app cannot read is not evidence of a hook, and there is nothing to repair:
/// the other three sources still decide the state.
fn wired(home: Option<&Path>) -> bool {
    let Some(path) = home.map(|home| home.join(CLAUDE_SETTINGS)) else {
        return false;
    };
    let Ok(text) = std::fs::read_to_string(path) else {
        return false;
    };
    let Ok(settings) = serde_json::from_str::<serde_json::Value>(&text) else {
        return false;
    };
    match settings.get("hooks") {
        Some(hooks) => any_caveman_command(hooks),
        None => false,
    }
}

/// Every `command` string anywhere under the `hooks` value, tested for the
/// mark.
///
/// A walk rather than the shape written out — `hooks[event][].hooks[].command`
/// — because that shape is Claude Code's and moves with Claude Code, while the
/// question ("does anything here call caveman") does not. A `matcher` or an
/// event name that happens to hold the word is deliberately not counted: only a
/// `command` runs anything.
fn any_caveman_command(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::Object(map) => map.iter().any(|(key, child)| {
            (key == "command" && child.as_str().is_some_and(is_caveman_command))
                || any_caveman_command(child)
        }),
        serde_json::Value::Array(items) => items.iter().any(any_caveman_command),
        _ => false,
    }
}

/// Whether one command line is caveman's. A substring rather than a parsed
/// argv: the invocation can be `caveman shrink-hook`, an absolute path into
/// `~/.caveman/bin`, a `node …/@caveman-ai/cli/dist/index.js shrink-hook`, or
/// the PowerShell wrapping of any of those, and every one of them carries the
/// name somewhere.
fn is_caveman_command(command: &str) -> bool {
    command.to_ascii_lowercase().contains(MARK)
}

/// Caveman's journal, as much of it as this app has any use for.
///
/// Every field is optional and unknown fields are ignored, which is what makes
/// a journal from a newer caveman an ordinary outcome rather than a failure.
#[derive(Debug, serde::Deserialize)]
struct Journal {
    #[serde(default)]
    pack_version: Option<String>,
    #[serde(default)]
    detected_agent_version: Option<String>,
    #[serde(default)]
    operations: Vec<Operation>,
}

/// One replaced file. Caveman also records the backup it took and the sha256
/// before and after; those are read by nothing here, and a field this app does
/// not use is a field it cannot be wrong about.
#[derive(Debug, serde::Deserialize)]
struct Operation {
    #[serde(default)]
    file: Option<String>,
}

/// The journal, or `None` for anything at all that goes wrong reading it.
///
/// Damaged JSON, a journal that is an array, a file with the wrong permissions:
/// all of them lose the three facts and none of them changes the state, which
/// is decided by the other three sources. That is the acceptance criterion and
/// it is also the honest answer — the facts are decoration on a line of text,
/// and a state read from the filesystem is not less true for the decoration
/// being unavailable.
fn read_journal(caveman_home: &Path) -> Option<Journal> {
    let text = std::fs::read_to_string(caveman_home.join(CLAUDE_JOURNAL)).ok()?;
    serde_json::from_str::<Journal>(&text).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// Where a test writes the journal. Built from the module's own constants
    /// rather than spelled out, so that a renamed source moves both halves.
    fn journal_path(home: &Path) -> PathBuf {
        home.join(HOME_DIR).join(CLAUDE_JOURNAL)
    }

    /// A directory of this test's own. The habit `settings::file` and
    /// `tracker::backup` already follow: the process id keeps two runs apart
    /// and the counter keeps two cases within one run apart.
    fn temp_dir(name: &str) -> PathBuf {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir()
            .join(format!("smetana-caveman-{}-{n}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("the temporary directory is made");
        dir
    }

    fn write(path: &Path, body: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("the parent is made");
        }
        fs::write(path, body).expect("the file is written");
    }

    /// The binaries laid out, and nothing else.
    fn lay_out_binaries(home: &Path) {
        write(&home.join(HOME_DIR).join(BIN_MANIFEST), r#"{"release":"v1","artifacts":{}}"#);
    }

    /// A hooks section of Claude Code's own shape, calling caveman.
    fn wire_hooks(home: &Path) {
        write(
            &home.join(CLAUDE_SETTINGS),
            r#"{
              "hooks": {
                "PreToolUse": [
                  {
                    "matcher": "Bash",
                    "hooks": [{ "type": "command", "command": "caveman shrink-hook" }]
                  }
                ]
              }
            }"#,
        );
    }

    /// A journal of caveman's own shape, with two replaced files.
    fn write_journal(home: &Path) {
        write(
            &journal_path(home),
            r#"{
              "schema_version": 1,
              "agent": "claude",
              "pack_version": "2.1.0",
              "installed_at": "2026-08-25T12:30:00.000Z",
              "detected_agent_version": "1.0.44",
              "operations": [
                {
                  "file": "/home/p/.claude/settings.json",
                  "kind": "claude-settings",
                  "backup": "/home/p/.caveman/integrations/backups/claude/x/0.bin",
                  "before_exists": true,
                  "before_sha256": "aa",
                  "after_sha256": "bb"
                },
                {
                  "file": "/home/p/.claude.json",
                  "kind": "claude-mcp",
                  "backup": "/home/p/.caveman/integrations/backups/claude/x/1.bin",
                  "before_exists": false,
                  "before_sha256": null,
                  "after_sha256": "cc"
                }
              ]
            }"#,
        );
    }

    #[test]
    fn nothing_on_the_machine_and_nothing_in_the_project_is_absent() {
        let home = temp_dir("absent-home");
        let project = temp_dir("absent-project");

        let answer = read(Some(&home), &project);

        assert_eq!(answer.state, CavemanState::Absent);
        assert_eq!(answer.pack_version, None);
        assert!(answer.replaced_files.is_empty());
    }

    #[test]
    fn binaries_with_no_hook_calling_them_are_installed_and_switched_off() {
        let home = temp_dir("binaries-home");
        let project = temp_dir("binaries-project");
        lay_out_binaries(&home);
        // A settings file with hooks of somebody else's, which must not count.
        write(
            &home.join(CLAUDE_SETTINGS),
            r#"{"hooks":{"PreToolUse":[{"matcher":"Bash",
               "hooks":[{"type":"command","command":"my-own-linter"}]}]}}"#,
        );

        assert_eq!(read(Some(&home), &project).state, CavemanState::BinariesOnly);
    }

    #[test]
    fn a_hook_that_calls_caveman_is_wired() {
        let home = temp_dir("wired-home");
        let project = temp_dir("wired-project");
        lay_out_binaries(&home);
        wire_hooks(&home);

        assert_eq!(read(Some(&home), &project).state, CavemanState::Wired);
    }

    #[test]
    fn the_skill_in_a_project_with_nothing_global_is_the_project_skill() {
        let home = temp_dir("skill-home");
        let project = temp_dir("skill-project");
        fs::create_dir_all(project.join(PROJECT_SKILL)).expect("the skill directory is made");
        write(&project.join(PROJECT_SKILL).join("SKILL.md"), "# caveman");

        assert_eq!(read(Some(&home), &project).state, CavemanState::ProjectSkillOnly);
    }

    /// The global install wins over the project skill, because the global one
    /// is what is acting on the session either way.
    #[test]
    fn a_project_skill_beside_a_global_install_reads_as_the_global_one() {
        let home = temp_dir("both-home");
        let project = temp_dir("both-project");
        lay_out_binaries(&home);
        fs::create_dir_all(project.join(PROJECT_SKILL)).expect("the skill directory is made");

        assert_eq!(read(Some(&home), &project).state, CavemanState::BinariesOnly);
    }

    #[test]
    fn a_wired_install_carries_the_pack_version_and_what_it_replaced() {
        let home = temp_dir("journal-home");
        let project = temp_dir("journal-project");
        lay_out_binaries(&home);
        wire_hooks(&home);
        write_journal(&home);

        let answer = read(Some(&home), &project);

        assert_eq!(answer.state, CavemanState::Wired);
        assert_eq!(answer.pack_version.as_deref(), Some("2.1.0"));
        assert_eq!(answer.detected_agent_version.as_deref(), Some("1.0.44"));
        assert_eq!(
            answer.replaced_files,
            vec!["/home/p/.claude/settings.json".to_string(), "/home/p/.claude.json".to_string()]
        );
    }

    /// The criterion this module's leniency is written for: a damaged journal
    /// costs the three facts and nothing else. The state still comes from the
    /// binaries and the hooks, which are files of their own.
    #[test]
    fn a_damaged_journal_costs_the_facts_and_not_the_state() {
        let home = temp_dir("damaged-home");
        let project = temp_dir("damaged-project");
        lay_out_binaries(&home);
        wire_hooks(&home);
        write(&journal_path(&home), "{\"pack_version\": \"2.1.0\", trunca");

        let answer = read(Some(&home), &project);

        assert_eq!(answer.state, CavemanState::Wired);
        assert_eq!(answer.pack_version, None);
        assert_eq!(answer.detected_agent_version, None);
        assert!(answer.replaced_files.is_empty());
    }

    /// A journal that parses but is not the shape this app expects — an array,
    /// a newer schema, a field of the wrong type — is the same ordinary
    /// outcome. Only `operations` of the wrong type can lose the whole journal
    /// here, which is why the two are checked apart.
    #[test]
    fn a_journal_of_another_shape_is_an_ordinary_outcome() {
        let home = temp_dir("shape-home");
        let project = temp_dir("shape-project");
        lay_out_binaries(&home);
        write(&journal_path(&home), r#"["not", "an", "object"]"#);

        let answer = read(Some(&home), &project);
        assert_eq!(answer.state, CavemanState::BinariesOnly);
        assert_eq!(answer.pack_version, None);

        // A journal from a newer caveman: fields this app has never heard of
        // are ignored, and the ones it knows still arrive.
        write(
            &journal_path(&home),
            r#"{"schema_version":2,"pack_version":"9.0.0","something_new":{"a":1}}"#,
        );
        let answer = read(Some(&home), &project);
        assert_eq!(answer.pack_version.as_deref(), Some("9.0.0"));
        assert!(answer.replaced_files.is_empty());
    }

    /// Nobody can say where home is. That is not an error to report: there is
    /// no `~/.caveman` on a machine with no `~`, and the project is still
    /// readable.
    #[test]
    fn no_home_directory_is_absent_rather_than_a_failure() {
        let project = temp_dir("nohome-project");

        assert_eq!(read(None, &project).state, CavemanState::Absent);

        fs::create_dir_all(project.join(PROJECT_SKILL)).expect("the skill directory is made");
        assert_eq!(read(None, &project).state, CavemanState::ProjectSkillOnly);
    }

    /// The invocation caveman writes when `caveman` is not on `PATH`: node, and
    /// the CLI's own script path. The name is in the path rather than in the
    /// first word, which is why the match is over the whole line.
    #[test]
    fn a_hook_that_reaches_caveman_through_node_counts() {
        let home = temp_dir("node-home");
        let project = temp_dir("node-project");
        lay_out_binaries(&home);
        write(
            &home.join(CLAUDE_SETTINGS),
            r#"{"hooks":{"PreToolUse":[{"matcher":"Bash","hooks":[{"type":"command",
               "command":"/usr/local/bin/node /usr/local/lib/node_modules/@caveman-ai/cli/dist/index.js shrink-hook"}]}]}}"#,
        );

        assert_eq!(read(Some(&home), &project).state, CavemanState::Wired);
    }

    /// A matcher, an event name or a comment carrying the word is not a hook
    /// that runs anything. Only a `command` counts.
    #[test]
    fn the_word_outside_a_command_is_not_a_wiring() {
        let home = temp_dir("matcher-home");
        let project = temp_dir("matcher-project");
        lay_out_binaries(&home);
        write(
            &home.join(CLAUDE_SETTINGS),
            r#"{"hooks":{"PreToolUse":[{"matcher":"caveman",
               "hooks":[{"type":"command","command":"my-own-linter"}]}]}}"#,
        );

        assert_eq!(read(Some(&home), &project).state, CavemanState::BinariesOnly);
    }

    /// A settings file that is not JSON at all says nothing about a hook, and
    /// must not stop the reading: the state still comes from the other three
    /// sources.
    #[test]
    fn unreadable_claude_settings_are_not_a_wiring_and_not_a_failure() {
        let home = temp_dir("badsettings-home");
        let project = temp_dir("badsettings-project");
        lay_out_binaries(&home);
        write(&home.join(CLAUDE_SETTINGS), "this is not json");

        assert_eq!(read(Some(&home), &project).state, CavemanState::BinariesOnly);
    }

    /// The four names cross the IPC boundary and a screen reads them by name,
    /// so the serialization is the contract rather than the struct — the shape
    /// `autostart::AutostartState` is pinned in the same way.
    #[test]
    fn the_answer_travels_as_a_state_and_three_facts() {
        let json = serde_json::to_string(&Caveman::bare(CavemanState::ProjectSkillOnly))
            .expect("the answer must serialize");
        assert_eq!(
            json,
            r#"{"state":"project-skill-only","packVersion":null,"detectedAgentVersion":null,"replacedFiles":[]}"#
        );
    }

    /// The four state names, which are the whole vocabulary a screen switches
    /// on.
    #[test]
    fn the_four_states_are_spelled_in_kebab_case() {
        let name = |state| {
            let json = serde_json::to_string(&state).expect("a state must serialize");
            json.trim_matches('"').to_string()
        };
        assert_eq!(name(CavemanState::Absent), "absent");
        assert_eq!(name(CavemanState::BinariesOnly), "binaries-only");
        assert_eq!(name(CavemanState::Wired), "wired");
        assert_eq!(name(CavemanState::ProjectSkillOnly), "project-skill-only");
    }
}
