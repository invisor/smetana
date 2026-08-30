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
use super::usage::{self, AgentUsage};

/// Every outcome is a state, so this cannot fail: a project with no config is
/// the ordinary case, and an unreadable one is `Broken` with what the OS said.
#[tauri::command]
pub fn project_config(project: String) -> ConfigState {
    config::load(Path::new(&project))
}

/// The four keys of `[defaults]`, written back — the one part of a run
/// configuration this app edits itself. Everything else in the file is the
/// setup agent's, and `[merge].hazards` in particular is prose a lead reads,
/// which is why the write is a surgical `toml_edit` pass rather than a serde
/// round trip. `config::save_defaults` carries the whole of that argument.
///
/// Fallible, unlike `project_config` above, and deliberately: reading has a
/// state for every outcome and writing does not. "There is no file" and "the
/// file will not parse" are both real answers here, and both have to reach the
/// person who pressed Save rather than being folded into a shrug.
///
/// The menu already refuses both, so this is the back stop — the file can
/// change under an open window, and a project somebody set up in a text editor
/// while the dialog stood open is exactly the case a form cannot repair.
#[tauri::command]
pub fn project_config_save_defaults(
    project: String,
    defaults: config::Defaults,
) -> Result<(), String> {
    config::save_defaults(Path::new(&project), &defaults)
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
///
/// One case that guard does not catch: a stale linked worktree, whose `.git`
/// is a file and whose `gitdir:` target has since gone. `git_dir` parses the
/// pointer without checking it resolves, so it answers `Some` and the folder
/// counts as a readable repository with no branches at all — after which every
/// branch in the project reads as missing from it, which is the very outcome
/// leaving unreadable repositories out exists to prevent.
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

/// What is left of the subscription, for the Agents tab in the settings window.
///
/// The same reading the run gate makes before every batch
/// (`runs::service::ask`), asked from the other end of the app and answered in
/// a shape with a person in mind rather than a decision: three distinguishable
/// states, and the band beside the percentages rather than the thresholds that
/// produced it. `usage::report` is the whole of that mapping and is where its
/// tests are; what is left here is getting it its two arguments.
///
/// Infallible, like `project_config` and `browser_tools` above and for their
/// reason: every outcome is a state. A machine with no agent installed, an
/// agent that cannot be asked and a probe that came back empty are all answers.
///
/// **Both halves are blocking and neither may sit on the async runtime.**
/// `shell_env::path` can be a login shell's whole start-up on its first call,
/// and the probe is somebody else's CLI with a 60-second ceiling over it — a
/// minute of the runtime's workers taken out of the file tree and the board,
/// with nothing on screen saying why. So each goes to the blocking pool, and
/// they go separately so that the failure of either lands as what it actually
/// is: a joined task that fell over before the profile was resolved has nobody
/// to name, while one that fell over during the probe has an agent and no
/// reading, which is exactly `Unreadable`.
///
/// Nothing is cached between this and the run gate, deliberately: a reading is
/// minutes old the moment it is taken, and a person pressing Refresh is asking
/// the harness rather than asking us what it last said.
///
/// **The caller may name the agent, and the settings window always does.** That
/// is not a convenience: the front end owns the truth about this field and the
/// file is up to a debounce behind it (`SAVE_DELAY`, 400 ms in
/// `stores/settings.js`), so a window that changed the agent and asked in the
/// same breath would be answered about the agent it had just left — reliably
/// rather than racily, and for up to the sixty seconds the probe may take, with
/// a heading honest enough about who answered to look exactly like
/// `agents::pick`'s legitimate substitution. `None` is for a caller with no
/// opinion and keeps the file as the answer.
#[tauri::command]
pub async fn agent_usage(app: AppHandle, agent: Option<String>) -> AgentUsage {
    let profile = tokio::task::spawn_blocking(move || {
        let id = wanted(agent, || crate::settings::agent(&app));
        crate::agents::pick(&id, crate::shell_env::path())
    })
    .await
    .unwrap_or(None);
    let Some(profile) = profile else { return usage::report(None, None) };
    // `read` answers `None` for a profile with no `usage_command` without
    // spawning anything, so this costs nothing for Codex; `report` is what
    // tells that `None` apart from a probe's.
    let reading = tokio::task::spawn_blocking(move || usage::read(profile)).await.unwrap_or(None);
    usage::report(Some(profile), reading)
}

/// Which agent id `agent_usage` resolves: what the caller asked for, and the
/// file only for a caller that asked for nothing.
///
/// The fall-back is a closure rather than a value so that a caller who named an
/// agent costs no disk read at all — which is also the half of this worth
/// pinning, since the read is invisible from the answer.
///
/// A blank id is nobody rather than a name: it can only come from a field that
/// was never filled, and asking `pick` for `""` would silently substitute the
/// first installed profile as though it had been chosen.
fn wanted(asked: Option<String>, configured: impl FnOnce() -> String) -> String {
    asked
        .map(|id| id.trim().to_owned())
        .filter(|id| !id.is_empty())
        .unwrap_or_else(configured)
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
    fn the_agent_the_caller_named_wins_over_the_one_on_disk() {
        // The settings window always names one, because the file is up to a
        // debounce behind what it is showing: reading the file here would probe
        // the agent somebody has just switched away from. The panic is the
        // assertion — a caller who named an agent must not cost a disk read.
        assert_eq!(
            wanted(Some("codex".into()), || panic!("the file must not be read for a named agent")),
            "codex"
        );
    }

    #[test]
    fn a_caller_with_no_opinion_still_gets_the_configured_agent() {
        // Every other caller, and what this command did before it took an
        // argument at all.
        assert_eq!(wanted(None, || "claude".into()), "claude");
    }

    #[test]
    fn a_blank_name_is_nobody_rather_than_an_agent() {
        // `pick("")` substitutes the first installed profile, which would read
        // as a choice rather than as the empty field it came from.
        assert_eq!(wanted(Some("   ".into()), || "claude".into()), "claude");
        assert_eq!(wanted(Some(String::new()), || "claude".into()), "claude");
    }

    /// The command is a thin wrapper, so what it owes a test is the pair the
    /// wrapper decides: a good save reaches the file, and a bad one comes back
    /// as a message rather than as a panic.
    #[test]
    fn saving_the_defaults_reaches_the_file() {
        let root = scratch("command-save-defaults");
        config(&root, "[project]\nrepos = [\".\"]\n");

        project_config_save_defaults(
            root.to_string_lossy().into_owned(),
            config::Defaults {
                target_branch: Some("develop".into()),
                min_priority: 1,
                max_parallel_tasks: 6,
                review_passes: 2,
            },
        )
        .expect("save the defaults");

        match config::load(&root) {
            ConfigState::Ok { config } => {
                assert_eq!(config.defaults.target_branch.as_deref(), Some("develop"));
                assert_eq!(config.defaults.max_parallel_tasks, 6);
            }
            other => panic!("expected a loadable config, got {other:?}"),
        }
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn saving_the_defaults_over_a_damaged_file_answers_with_the_parse_error() {
        let root = scratch("command-save-defaults-broken");
        config(&root, "this is not toml at all [[[");

        let err = project_config_save_defaults(
            root.to_string_lossy().into_owned(),
            config::Defaults::default(),
        )
        .expect_err("a damaged file is refused");
        assert!(!err.is_empty());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn saving_the_defaults_where_there_is_no_file_answers_rather_than_panicking() {
        // The other of the two refusals, and the one the menu words differently:
        // a project nobody has set up has nothing for a form to draw.
        let root = scratch("command-save-defaults-absent");
        let err = project_config_save_defaults(
            root.to_string_lossy().into_owned(),
            config::Defaults::default(),
        )
        .expect_err("there is no file to edit");
        assert!(!err.is_empty());
        let _ = fs::remove_dir_all(&root);
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
