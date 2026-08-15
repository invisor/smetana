//! What a project is made of.
//!
//! One rule with two arms, and the rule itself is pure — `names` takes the
//! configured list and a directory listing and knows nothing about either
//! disk — which is what puts the tests in this file.

use std::path::{Path, PathBuf};

use super::model::Repo;
use crate::runs::config::{self, ConfigState};

/// The repositories of a project, by name.
///
/// The config is the truth about a project that has one, in its own order: it
/// is what the run dialog already merges into (`runs::commands::target_branches`).
/// Otherwise the answer is the root itself plus whatever sits one level below
/// it — `"."` first, always, with `discover` dropping it when the root is not a
/// repository.
///
/// Discovery stops at one level, and that is a decision rather than a limit. It
/// is a fallback for a workspace laid out the obvious way — five sibling
/// repositories in one folder, opened before anybody set the project up for
/// runs — and not a search: walking deeper would find every vendored dependency
/// with a `.git` in it, and the honest answer for anything more complicated is
/// `[project].repos`.
pub fn names(configured: Option<Vec<String>>, listing: Vec<String>) -> Vec<String> {
    // An empty configured list is not a configuration: it says a project with
    // no repositories, which nothing downstream could do anything with.
    if let Some(repos) = configured {
        if !repos.is_empty() {
            return repos;
        }
    }
    let mut out = vec![".".to_string()];
    out.extend(listing);
    out
}

/// One entry per repository git can actually see, with the branch each is on.
///
/// The branch comes from `git::head`, not from a `git` call: it is a file read
/// of `HEAD`, and the whole list costs one read per repository.
///
/// A name that resolves to nothing readable — a missing folder, one with no
/// `.git` — is left out rather than shown as broken, which is the rule
/// `git::combine` already applies and for the same reason: one typo in the
/// config would otherwise fill the panel with rows nothing can be done about.
pub fn discover(root: &Path) -> Vec<Repo> {
    // Three inputs mean "no config" and it is the same three `repo_lists`
    // treats alike: no file, a file that will not parse, and an empty list.
    let configured = match config::load(root) {
        ConfigState::Ok { config } if !config.project.repos.is_empty() => {
            Some(config.project.repos.clone())
        }
        _ => None,
    };
    names(configured, one_level_down(root))
        .into_iter()
        .filter_map(|name| {
            let path = resolve(root, &name);
            crate::git::git_dir(&path)?;
            let head = crate::git::head(&path);
            Some(Repo {
                name,
                path: path.to_string_lossy().into_owned(),
                branch: head.branch,
                detached: head.detached,
            })
        })
        .collect()
}

/// `.` is the root itself rather than `root/.`: this string is drawn in a row
/// and handed back as the argument every other command in this module takes.
fn resolve(root: &Path, name: &str) -> PathBuf {
    if name == "." { root.to_path_buf() } else { root.join(name) }
}

/// The directories one level below the root that hold a `.git`, in name order.
///
/// Sorted here rather than left in `read_dir`'s: that order is the
/// filesystem's, one thing on APFS and another on ext4, and the panel's list
/// would come out differently on two machines looking at the same folder.
fn one_level_down(root: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(root) else { return Vec::new() };
    let mut out: Vec<String> = entries
        .flatten()
        .filter(|entry| entry.path().is_dir() && entry.path().join(".git").exists())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    out.sort();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_config_is_the_truth_when_there_is_one() {
        let names = names(Some(vec!["backend".into(), "admin".into()]), vec!["frontend".into()]);
        assert_eq!(names, ["backend", "admin"], "in the config's order, and nothing else");
    }

    #[test]
    fn without_a_config_the_root_and_one_level_down_are_the_answer() {
        let names = names(None, vec!["admin".into(), "backend".into()]);
        assert_eq!(names, [".", "admin", "backend"]);
    }

    /// The case this arm exists for: a folder holding several repositories,
    /// opened before anybody set the project up for runs. Asking only the root
    /// would name the accidental repository that folder happens to be — the
    /// very defect the run dialog already paid for once.
    #[test]
    fn a_workspace_of_siblings_is_not_reduced_to_its_container() {
        let names = names(None, vec!["admin".into(), "backend".into(), "frontend".into()]);
        assert!(names.len() > 1, "not just the container: {names:?}");
    }

    #[test]
    fn an_empty_configured_list_falls_through_to_discovery() {
        assert_eq!(names(Some(vec![]), vec!["admin".into()]), [".", "admin"]);
    }

    #[test]
    fn a_lone_repository_is_the_root_and_nothing_else() {
        assert_eq!(names(None, vec![]), ["."]);
    }
}
