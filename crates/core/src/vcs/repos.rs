//! What a project is made of.
//!
//! Two rules, both pure — `names` and `unlisted` take the configured list and a
//! directory listing and know nothing about either disk — which is what puts
//! the tests in this file. The second is the first read the other way round:
//! what is on disk that the configuration does not name.

use std::path::{Path, PathBuf};

use super::model::{ProjectRepos, Repo};
use crate::runs::config::{self, ConfigState};

/// The configured list, or `None` for a project that has not stated one.
///
/// An empty configured list is not a configuration: it says a project with no
/// repositories, which nothing downstream could do anything with. Both rules
/// below read it through here rather than spelling the check twice — they have
/// to agree about what "configured" means, or the second would call a folder
/// unnamed against a list the first is not drawing.

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
    if let Some(repos) = stated(configured) {
        return repos;
    }
    let mut out = vec![".".to_string()];
    out.extend(listing);
    out
}

/// Which repositories found on disk the configuration does not name.
///
/// The second half of the answer above and the whole of what the panel has to
/// say about a folder it is not drawing: somebody clones a repository into
/// their project from a terminal, and a configured project's list cannot grow
/// on its own — `[project].repos` is the truth about a project, for the runs
/// machinery as much as for this panel, and offering a repository runs know
/// nothing about would trade one silence for a louder lie. So the folder is
/// named instead, beside the door that fixes it: the setup agent, which is the
/// only thing in this app that writes that file.
///
/// **A project with no configuration answers with nothing at all, always**, and
/// by construction rather than by a branch: everything the listing holds is in
/// `names`'s own answer there, so nothing can be left over. That is what keeps
/// this from being a second concept a reader has to hold.
///
/// A configured name with nothing on disk behind it is not this rule's business
/// either — it is not in the listing, so it cannot come back out of it. The
/// panel already drops such a name from the list itself, in silence, and saying
/// something about it here would be a second answer about the same typo.
pub fn unlisted(configured: Option<Vec<String>>, listing: Vec<String>) -> Vec<String> {
    let Some(repos) = stated(configured) else { return Vec::new() };
    let named: Vec<String> = repos.iter().map(|name| key(name)).collect();
    listing.into_iter().filter(|name| !named.contains(&key(name))).collect()
}

/// What two spellings of one folder have in common.
///
/// `resolve` below takes `admin`, `./admin` and `admin/` to the same directory,
/// so a configuration written any of those ways names that folder — and a rule
/// comparing the strings as written would accuse a repository the panel is
/// drawing one row above of not being in the file it was read from.
fn key(name: &str) -> String {
    name.trim_end_matches('/').trim_start_matches("./").to_string()
}

fn stated(configured: Option<Vec<String>>) -> Option<Vec<String>> {
    configured.filter(|repos| !repos.is_empty())
}

/// One entry per repository git can actually see, with the branch each is on —
/// and, beside them, the folders on disk the configuration does not name.
///
/// The branch comes from `git::head`, not from a `git` call: it is a file read
/// of `HEAD`, and the whole list costs one read per repository.
///
/// A name that resolves to nothing readable — a missing folder, one with no
/// `.git` — is left out rather than shown as broken, which is the rule
/// `git::combine` already applies and for the same reason: one typo in the
/// config would otherwise fill the panel with rows nothing can be done about.
pub fn discover(root: &Path) -> ProjectRepos {
    // Three inputs mean "no config" and it is the same three `repo_lists`
    // treats alike: no file, a file that will not parse, and an empty list.
    let configured = match config::load(root) {
        ConfigState::Ok { config } if !config.project.repos.is_empty() => {
            Some(config.project.repos.clone())
        }
        _ => None,
    };
    // The listing is read in both arms, and for a configured project that is a
    // read this function used to congratulate itself on having removed: one
    // `read_dir` of the root plus a `.git` stat per entry, on every window
    // focus and every press of the refresh button. It is bought back
    // deliberately, and the price is exact — it is no longer thrown away on the
    // next line but is the whole of `unlisted`, and it is the same work every
    // project *without* a configuration already pays at the same cadence.
    let listing = one_level_down(root);
    let listed = names(configured.clone(), listing.clone());
    let unlisted = unlisted(configured, listing);
    let repos = listed
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
        .collect();
    ProjectRepos { repos, unlisted }
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

    #[test]
    fn a_config_naming_everything_on_disk_leaves_nothing_unlisted() {
        let left = unlisted(
            Some(vec![".".into(), "admin".into(), "backend".into()]),
            vec!["admin".into(), "backend".into()],
        );
        assert!(left.is_empty(), "nothing to say about a project that names it all: {left:?}");
    }

    /// The report this rule was written for: a repository cloned into the
    /// project from a terminal, which a configured list can never grow to hold.
    #[test]
    fn a_folder_the_config_misses_is_named() {
        let left = unlisted(
            Some(vec![".".into(), "admin".into()]),
            vec!["admin".into(), "newrepo".into()],
        );
        assert_eq!(left, ["newrepo"]);
    }

    /// Empty by construction rather than by a branch: every folder found is
    /// already in `names`'s answer, so there is nothing left over to say.
    #[test]
    fn a_project_without_a_config_never_has_anything_unlisted() {
        assert!(unlisted(None, vec!["admin".into(), "backend".into()]).is_empty());
        assert!(unlisted(Some(vec![]), vec!["admin".into()]).is_empty());
    }

    /// The other direction, and it is not this rule's business: a name in the
    /// config with nothing on disk behind it is dropped from the list in
    /// silence by `discover`, and must not come back as a folder nobody has.
    #[test]
    fn a_configured_name_with_nothing_behind_it_is_not_unlisted() {
        let left = unlisted(
            Some(vec![".".into(), "admin".into(), "gone".into()]),
            vec!["admin".into()],
        );
        assert!(left.is_empty(), "a missing name is not a folder to point at: {left:?}");
    }

    /// `resolve` takes all three spellings to the same directory, so a rule
    /// comparing the strings as written would accuse a repository of not being
    /// in the file the row above it was read from.
    #[test]
    fn a_folder_named_another_way_round_is_still_named() {
        assert!(unlisted(Some(vec!["./admin".into()]), vec!["admin".into()]).is_empty());
        assert!(unlisted(Some(vec!["admin/".into()]), vec!["admin".into()]).is_empty());
    }
}
