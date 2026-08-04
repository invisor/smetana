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
}
