//! The thin layer over `config.rs`. There is no worker to queue behind and no
//! state to guard: reading one file costs milliseconds, the same reasoning
//! that keeps `files/` and `git.rs` out of a worker.

use std::path::{Path, PathBuf};

use tauri::{AppHandle, Manager, State};
use tokio::sync::oneshot;

use super::browser::{self, BrowserTools};
use super::config::{self, ConfigState, LiveCheckMode};
use super::model::{Run, RunError, RunSettings};
use super::service::{Request, RunHandle};

/// Every outcome is a state, so this cannot fail: a project with no config is
/// the ordinary case, and an unreadable one is `Broken` with what the OS said.
#[tauri::command]
pub fn project_config(project: String) -> ConfigState {
    config::load(Path::new(&project))
}

/// What this project's runs may merge into: every local branch of every
/// repository `[project].repos` names, and which of them each branch is
/// missing from.
///
/// **The config is read here rather than taken from the front end, and that is
/// the point of the command existing at all.** `runs.js` fills its copy through
/// its own `project_config` call, and the run dialog is shown before that has
/// landed — which is the whole of smetana-6gs, where the branch rule ran once
/// against a list that was not there yet. A repository list passed down from
/// the front end would be the same race under a new name. Read inside one
/// command, there is no order between the two facts to get wrong.
///
/// It lives in `runs/` rather than in `git.rs` because "what may this run merge
/// into" is a question about a run; `git.rs` keeps the pure combining rule and
/// stays a leaf, knowing nothing about project configuration.
#[tauri::command]
pub fn target_branches(project: String) -> Vec<crate::git::BranchOption> {
    crate::git::combine(repo_lists(Path::new(&project)))
}

/// One entry per repository git can actually see.
///
/// Three inputs give the same answer and it is the one this has always given:
/// no config file, a config that will not parse, and `repos = ["."]` all ask
/// the project root. A single-repository project is the common case — this
/// project is one — and nothing about it changes.
///
/// A name pointing at a missing folder, or at one with no `.git`, is left out
/// rather than counted as missing every branch. `git_dir` is what tells that
/// apart from a repository with nothing in it, since both hand back an empty
/// list of branches.
fn repo_lists(root: &Path) -> Vec<(String, Vec<(String, Option<i64>)>)> {
    let names: Vec<String> = match config::load(root) {
        ConfigState::Ok { config } if !config.project.repos.is_empty() => config.project.repos.clone(),
        _ => vec![".".to_string()],
    };
    names
        .into_iter()
        .filter_map(|name| {
            let path: PathBuf = root.join(&name);
            crate::git::git_dir(&path)?;
            Some((name, crate::git::branches_with_recency(&path)))
        })
        .collect()
}

/// Whether there is anything on this machine to drive a browser with, asked
/// before the run dialog opens so its live-check toggle can be switched off and
/// blocked with a reason rather than starting a run that fails inside the check.
///
/// Infallible for `project_config`'s reason: every outcome here is a state. A
/// machine with no Playwright and no extension is the answer, not a failure —
/// and the one thing that genuinely could fail, asking the run worker about
/// busy-ness, falls back to "nothing is holding it". That is the lenient
/// direction, and it is the right one for this fact: the tool-presence half is
/// what the "unobservable reads as no" rule is about, and a worker that cannot
/// answer is a worker no run could be going through anyway.
///
/// `AppHandle` rather than `State<'_, RunHandle>`: an async command borrowing
/// state has to return a `Result`, and there is nothing here to put in one.
#[tauri::command]
pub async fn browser_tools(app: AppHandle, project: String) -> BrowserTools {
    // `try_state` rather than `state`: the latter panics when the worker is not
    // managed yet, and a panic inside a read that answers "what does this
    // machine have" is the one outcome this command has no honest shape for.
    let candidates = match app.try_state::<RunHandle>() {
        Some(handle) => {
            let handle = handle.inner().clone();
            ask(&handle, Request::BrowserBusy).await.unwrap_or_default()
        }
        None => Vec::new(),
    };

    // The worker knows a run asked for a live check; only the project's own
    // config says whether that check opens a browser. A run whose live check is
    // a declared command needs no browser and is holding nothing, and naming it
    // as the reason this toggle is blocked would be an invention.
    let holder = candidates.into_iter().find(|other| {
        matches!(
            config::load(Path::new(other)),
            ConfigState::Ok { config }
                if config.live_check.as_ref().map(|live| live.mode) == Some(LiveCheckMode::Browser)
        )
    });

    browser::detect(Path::new(&project), holder)
}

/// How everything here reaches the worker, shaped exactly like the tracker's:
/// put a request on the worker's queue and await the reply. The outer failure is
/// delivery to the worker; the inner one, where there is one, is about the run
/// itself. `browser_tools` above is the one caller that swallows the outer
/// failure rather than passing it on, for the reason recorded on it.
async fn ask<T>(
    handle: &RunHandle,
    make: impl FnOnce(oneshot::Sender<T>) -> Request,
) -> Result<T, RunError> {
    let (tx, rx) = oneshot::channel();
    handle
        .0
        .send(make(tx))
        .await
        .map_err(|_| RunError::Terminal("the run worker is not running".into()))?;
    rx.await.map_err(|_| RunError::Terminal("the run worker did not answer".into()))
}

#[tauri::command]
pub async fn run_start(
    handle: State<'_, RunHandle>,
    project: String,
    settings: RunSettings,
) -> Result<Run, RunError> {
    ask(&handle, |tx| Request::Start(project, Box::new(settings), tx)).await?
}

/// Cooperative: this answers as soon as the worker has noted the request, and
/// the batch in flight is still going. `Run.stopping` is what says so, and the
/// run's own event says when it is actually over.
///
/// Named by the run's token rather than the project: a project holds several
/// runs now, and the stop has to reach exactly the one whose bar segment was
/// pressed. `None` back is a run that ended before the stop arrived.
#[tauri::command]
pub async fn run_stop(handle: State<'_, RunHandle>, token: u64) -> Result<Option<Run>, RunError> {
    ask(&handle, |tx| Request::Stop(token, tx)).await
}

/// The `run:state` event fires before the webview can subscribe — the same
/// shape `tracker_health` has, and for the same reason. The set rather than
/// one run: the project may hold several, and `runs.js` keeps them whole the
/// way it kept the single one.
#[tauri::command]
pub async fn run_state(handle: State<'_, RunHandle>, project: String) -> Result<Vec<Run>, RunError> {
    ask(&handle, |tx| Request::State(project, tx)).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("smetana-tb-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create the temp directory");
        dir
    }

    /// A repository with one branch and nothing else — enough for `git_dir` to
    /// find it and for `branches_with_recency` to name the branch through HEAD.
    fn repo(root: &Path, at: &str, branch: &str) {
        let git = root.join(at).join(".git");
        fs::create_dir_all(&git).expect("create the git directory");
        fs::write(git.join("HEAD"), format!("ref: refs/heads/{branch}\n")).expect("write HEAD");
    }

    fn config(root: &Path, body: &str) {
        fs::create_dir_all(root.join(".smetana")).expect("create .smetana");
        fs::write(root.join(".smetana/project.toml"), body).expect("write the config");
    }

    #[test]
    fn a_project_with_no_config_answers_from_its_own_root() {
        // The single-repository case, and the one this has always been. Nothing
        // can be partial when there is nothing to be short of.
        let root = scratch("no-config");
        repo(&root, ".", "main");
        let out = target_branches(root.to_string_lossy().into_owned());
        assert_eq!(out.iter().map(|o| o.name.as_str()).collect::<Vec<_>>(), ["main"]);
        assert!(out.iter().all(|o| o.missing_in.is_empty()), "{out:?}");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_config_that_will_not_parse_answers_from_the_root_rather_than_nothing() {
        // A damaged config is `runs::service`'s to shout about, and it does.
        // Emptying the branch field over it would disable Run with no sentence
        // anywhere near the field saying why.
        let root = scratch("broken-config");
        repo(&root, ".", "main");
        config(&root, "this is not toml at all [[[");
        let out = target_branches(root.to_string_lossy().into_owned());
        assert_eq!(out.iter().map(|o| o.name.as_str()).collect::<Vec<_>>(), ["main"]);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_monorepo_of_one_answers_from_the_root_too() {
        let root = scratch("dot-repo");
        repo(&root, ".", "develop");
        config(&root, "[project]\nrepos = [\".\"]\n");
        let out = target_branches(root.to_string_lossy().into_owned());
        assert_eq!(out.iter().map(|o| o.name.as_str()).collect::<Vec<_>>(), ["develop"]);
        assert!(out.iter().all(|o| o.missing_in.is_empty()), "{out:?}");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn the_named_repositories_are_asked_and_not_the_folder_holding_them() {
        // The defect this exists for: the root of `holiday-curb` is its own
        // repository on `master`, and no run will ever merge into it.
        let root = scratch("several");
        repo(&root, ".", "master");
        repo(&root, "backend", "develop");
        repo(&root, "admin", "develop");
        config(&root, "[project]\nrepos = [\"backend\", \"admin\"]\n");
        let out = target_branches(root.to_string_lossy().into_owned());
        assert_eq!(out.iter().map(|o| o.name.as_str()).collect::<Vec<_>>(), ["develop"]);
        assert!(out[0].missing_in.is_empty(), "{out:?}");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_branch_only_some_repositories_have_says_which_ones_lack_it() {
        let root = scratch("partial");
        repo(&root, "backend", "release/7");
        repo(&root, "admin", "develop");
        config(&root, "[project]\nrepos = [\"backend\", \"admin\"]\n");
        let out = target_branches(root.to_string_lossy().into_owned());
        let by_name: Vec<(&str, &[String])> =
            out.iter().map(|o| (o.name.as_str(), o.missing_in.as_slice())).collect();
        assert!(by_name.contains(&("release/7", &["admin".to_string()][..])), "{out:?}");
        assert!(by_name.contains(&("develop", &["backend".to_string()][..])), "{out:?}");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_name_that_is_not_a_repository_is_left_out_rather_than_missing_everything() {
        // One typo in `[project].repos` would otherwise make every branch
        // partial and empty the field's top group, hiding the real question
        // behind a fault that has nothing to do with it. `merging` stops at
        // that repository with a message naming it, which is more than this
        // dialog could say.
        let root = scratch("typo");
        repo(&root, "backend", "develop");
        config(&root, "[project]\nrepos = [\"backend\", \"beckend\"]\n");
        let out = target_branches(root.to_string_lossy().into_owned());
        assert_eq!(out, vec![crate::git::BranchOption { name: "develop".into(), missing_in: vec![] }]);
        let _ = fs::remove_dir_all(&root);
    }
}
