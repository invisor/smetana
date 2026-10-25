//! The facts a setup session is told about the project, computed once and in
//! one place: both the PTY worker (`terminal::service`) and the session worker
//! (`session::service`) start setup sessions, and what the agent is told must
//! be what the disk says at the moment the session starts, whichever road
//! started it.

use std::path::Path;

use crate::agents::Intent;

/// Only a `Setup` or a `Bootstrap` intent pays for the walk. `None` for every
/// other intent.
///
/// A `Setup` session pays for the survey and the browser walk; a `Bootstrap`
/// session pays for the browser walk alone — there is nothing in an empty
/// folder to survey, and `project-setup`'s `live_check` section still needs to
/// know what this machine can drive.
pub fn for_intent(project: &Path, intent: &Intent) -> Option<String> {
    if !matches!(intent, Intent::Setup | Intent::Bootstrap) {
        return None;
    }
    let bootstrap = matches!(intent, Intent::Bootstrap);
    // Before the agent writes anything, so the folder it is about to create
    // is already ignored when it appears rather than after somebody has
    // staged it. Failing costs a line in a .gitignore; refusing to start the
    // session over it would cost the whole feature, so this is logged and
    // stepped over.
    //
    // Setup's folder may or may not be a repository yet, and `ensure`'s own
    // guard is right for it — a `.gitignore` meaning nothing to nobody is not
    // this app's to create. Bootstrap's folder is `survey::is_empty`, which
    // is never a repository: `bd init` has run and `git init` has not, so
    // `ensure`'s guard would skip it outright and the repository the skill
    // creates a few lines later would open with `.smetana/` already
    // untracked. `ensure_before_git` is the same write without that guard,
    // safe here because git picks up whatever `.gitignore` already exists the
    // moment `git init` creates the repository over it.
    let wrote = if bootstrap {
        super::gitignore::ensure_before_git(project)
    } else {
        super::gitignore::ensure(project)
    };
    if let Err(err) = wrote {
        // Not ".smetana/": `ensure` writes whatever of its own list the file
        // is missing, and that list has grown.
        log::warn!("[runs] could not amend .gitignore: {err}");
    }
    // Two blocks and not one for Setup: the scan says what this project is,
    // and the browser facts say what the machine it will be worked on can do.
    // Only `[live_check].mode = "browser"` reaches outside the repository for
    // a tool, so it is the one part of the file the folder alone cannot
    // answer. Bootstrap gets the second block only — the folder is empty, so
    // there is nothing for `survey::run` to find, but the founding session
    // ends by running `project-setup` itself and its `live_check` section
    // reads the same browser facts.
    //
    // No busy project, deliberately: that is one run holding Playwright's
    // single profile at this moment, and the question here is whether the
    // tool is installed at all.
    let mut facts = String::new();
    if !bootstrap {
        facts.push_str(&super::survey::render(&super::survey::run(project)));
        facts.push('\n');
    }
    facts.push_str(&super::browser::render(&super::browser::detect(project, None)));
    Some(facts)
}
