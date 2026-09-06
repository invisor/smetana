//! Asking a harness one question and reading the answer, with nobody watching.
//!
//! Every other way into an agent in this app starts a *session*: a PTY, a pane
//! somebody can type into, a row in the agent list, an exit code the run loop
//! waits on. This is the other shape, and it exists because a commit message is
//! not work — nothing is claimed, nothing is written to disk, and there is
//! nothing for a person to answer halfway through. What it wants is a string
//! back inside a few seconds.
//!
//! `runs/usage.rs` is the one place that already did this, and this file is
//! deliberately the same spawn with the same failure discipline: `std::process`
//! and no PTY, the login shell's `PATH`, a deadline with a kill behind it. The
//! difference is what a failure costs. An unreadable allowance is no reason to
//! hold a run up, so `usage::read` answers `None` for every way of failing and
//! the caller shrugs; here somebody pressed a button and is watching the field,
//! so each way of failing keeps its own name and reaches the panel as a
//! sentence.
//!
//! Pure apart from `ask`: the prompt and the cleaning of what comes back are
//! ordinary functions, and the tests are at the bottom of this file.

use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde::Serialize;

use crate::agents::Profile;

/// A model writing one line has no business taking longer than this, and a
/// probe that hangs is worse than one that fails: the button would spin with
/// nothing on screen to say why. Longer than `usage.rs`'s 60s because this one
/// reads a diff first, and it is the person's own gesture rather than something
/// between two batches.
const TIMEOUT: Duration = Duration::from_secs(90);

/// How much of the patch is worth sending. Everything above it is cut, and the
/// cut is **announced in the prompt** rather than silent — a model told the
/// whole diff was there when it was not will describe the half it saw as the
/// whole change. `--stat` is never truncated and goes first, so even a cut
/// patch is read against the complete list of files.
///
/// The prompt rides as an argument rather than on stdin, which is what puts a
/// ceiling here at all: `ARG_MAX` is a megabyte on both platforms this ships
/// to, and 48 K leaves that untouchable while being more diff than a commit
/// message has ever needed.
const MAX_PATCH: usize = 48 * 1024;

/// What went wrong, in the shape every command in this app answers with — the
/// same `{ kind, message }` `VcsError` and `FilesError` serialize to, so the
/// front end's one normaliser reads this too.
#[derive(Debug, thiserror::Error)]
pub enum OneshotError {
    /// Nothing to ask: no agent on this machine at all. Named rather than
    /// silent, and the same sentence `VcsError::NoGit` uses — what was looked
    /// for belongs to this side.
    #[error("Smetana looked for {0} on your PATH and found nothing.")]
    NoAgent(String),
    /// There is an agent and it has no way to answer one question without a
    /// session. A different fact from the one above and it reads differently on
    /// screen: nothing is missing and nothing is broken, this harness simply
    /// cannot be asked.
    #[error("{0} cannot be asked a question without starting a session.")]
    Unsupported(String),
    /// git refused before the harness was ever asked. Its own words, untouched,
    /// for the reason `vcs/run.rs` records.
    #[error("{0}")]
    Git(String),
    /// git answered, and there was nothing in the answer to describe.
    ///
    /// **Refused here rather than sent on, and the reason is what a harness does
    /// with the question instead.** Asked to write a commit message for a change
    /// set with no `--stat`, no untracked path and no patch under it, a model
    /// does the only sensible thing: it says the changes are missing and asks
    /// for them. That reply is prose, in whatever language the harness converses
    /// in — which is *not* the person's `commitLanguage`, since that setting
    /// moves the subject of a message and this is not one — and `clean` has no
    /// way to tell a message from a sentence about not having one, so it lands
    /// in the field looking like something to press Commit under.
    ///
    /// The front end gates the button on its own count of changes, and that
    /// count is as fresh as the last window focus: an agent committing into the
    /// same tree is the ordinary case in this app, so by the time the button is
    /// pressed the tree it was drawn for may be gone. This is `commit_all`'s
    /// rule one command over — refuse before the expensive half, in this app's
    /// own words.
    #[error("There is nothing uncommitted here to write a message about.")]
    Nothing,
    /// The harness ran and exited non-zero. Its stderr where there is any —
    /// nobody but the person can tell an expired login from a broken flag.
    #[error("{0}")]
    Failed(String),
    #[error("The agent did not answer within {0} seconds.")]
    Timeout(u64),
    #[error("{0}")]
    Io(String),
}

impl OneshotError {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::NoAgent(_) => "noAgent",
            Self::Unsupported(_) => "unsupported",
            Self::Git(_) => "git",
            Self::Nothing => "nothing",
            Self::Failed(_) => "failed",
            Self::Timeout(_) => "timeout",
            Self::Io(_) => "io",
        }
    }
}

impl Serialize for OneshotError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut st = s.serialize_struct("OneshotError", 2)?;
        st.serialize_field("kind", self.kind())?;
        st.serialize_field("message", &self.to_string())?;
        st.end()
    }
}

/// What the harness is asked for, given what git said.
///
/// Three things go in and the order is the point: the instruction first, so a
/// model that reads no further than the head of a long prompt has the whole
/// task; `--stat` next, which is the complete list of files whatever happens to
/// the patch below it; then the untracked paths, which appear in **no** diff at
/// all and would otherwise be invisible in a change that is mostly new files;
/// and the patch last, where a cut costs the least.
///
/// The instruction names a form rather than describing one, because what comes
/// back goes straight into a field a person then presses Commit under: prose
/// around the message, a code fence, or a second paragraph is all cost. `clean`
/// below is the belt to this braces, and both are needed — models do add the
/// fence.
///
/// `language` is the **English name** of the person's `commitLanguage`, from
/// `agents::language_name` — a name rather than the `zh-Hans` out of the
/// settings file, because a tag is not something to write an instruction in.
/// It moves the prose of the message and nothing else: the form in front of the
/// colon, the six types, the imperative mood and the 72 characters are the same
/// sentence whatever is chosen, since they are grepped and read by people
/// rather than translated. That watershed is `task_language`'s one field over,
/// and the default is `en` because "in English" was written into this prompt
/// outright before the setting existed.
pub fn commit_prompt(
    language: &str,
    stat: &str,
    untracked: &[String],
    patch: &str,
) -> String {
    let mut out = format!(
        "Write a git commit message for the changes below.\n\
         \n\
         Answer with the message and nothing else: one line, at most 72 \
         characters, in the Conventional Commits form `type: subject` (feat, \
         fix, docs, refactor, test, chore). Use the imperative mood. Write the \
         subject in {language}; the type in front of the colon stays English \
         whatever the subject is written in. Do not add a body, an \
         explanation, quotation marks or a code fence.\n"
    );
    if !stat.trim().is_empty() {
        out.push_str("\nFiles changed:\n");
        out.push_str(stat.trim_end());
        out.push('\n');
    }
    if !untracked.is_empty() {
        // Named as new files rather than listed among the rest: to git they are
        // not in the diff below at all, and a model that sees a path only in a
        // list has no way to tell whether it was added or edited.
        out.push_str("\nNew files, not yet tracked by git:\n");
        for path in untracked {
            out.push_str(path);
            out.push('\n');
        }
    }
    let patch = patch.trim();
    if !patch.is_empty() {
        out.push_str("\nThe diff:\n");
        match patch.char_indices().nth(MAX_PATCH) {
            None => out.push_str(patch),
            Some((at, _)) => {
                out.push_str(&patch[..at]);
                out.push_str("\n… the diff is longer than this and was cut here.\n");
            }
        }
    }
    out
}

/// What the harness printed, as a commit message.
///
/// Everything here is a thing a model actually does when asked for one line.
/// The fence is the common one; the leading blank lines come with a preamble
/// the instruction asked it not to write; the quotation marks come from reading
/// "the message" as something to quote. Taking the **first** non-empty line is
/// the one rule that decides the rest: a model that adds a body after all
/// leaves the subject where it belongs, and a subject is what this field is
/// for.
pub fn clean(raw: &str) -> String {
    let line = raw
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty() && !line.starts_with("```"))
        .unwrap_or_default();
    let line = strip_pair(line, '"');
    let line = strip_pair(line, '\'');
    strip_pair(line, '`').to_string()
}

/// One pair of wrappers, and only a pair: a message that opens with a quotation
/// mark and does not close with one is a message, not a quotation.
fn strip_pair(line: &str, mark: char) -> &str {
    match line.strip_prefix(mark).and_then(|rest| rest.strip_suffix(mark)) {
        Some(inner) if !inner.is_empty() => inner.trim(),
        _ => line,
    }
}

/// One stream, read to the end on a thread of its own. The same shape
/// `vcs::run::drain` has, including the `Option`: a stream that was never piped
/// is the empty answer rather than a second case for the caller to handle.
///
/// `read_to_end` and not a line reader: what comes back is handed to
/// `Profile::oneshot_answer`, which is the only thing entitled to decide what
/// shape this harness's output has. A read that fails answers with what it had —
/// this function's job is to stop the child stalling, and the caller already has
/// an exit status and a deadline to judge the result by.
fn drain<S: std::io::Read + Send + 'static>(
    pipe: Option<S>,
) -> std::thread::JoinHandle<Vec<u8>> {
    std::thread::spawn(move || {
        let mut said = Vec::new();
        if let Some(mut pipe) = pipe {
            let _ = pipe.read_to_end(&mut said);
        }
        said
    })
}

/// End the call and reap it, so a killed child is not left defunct.
///
/// **The signal goes to the one process and not to its group**, which is where
/// this parts company with `vcs::run::terminate`, and it is the behaviour this
/// function has always had rather than a corner cut today. What is on the far
/// end differs: git is asked to stop mid-write and has `*.lock` files to remove,
/// so that one signals the group, sleeps a grace and signals again. Here the
/// deadline has already passed on a question nobody is waiting on any more, and
/// a two-second grace on the path a person is watching a spinner over would be
/// paid every time. A descendant the agent left behind is not reached, and that
/// is why the readers below are **not** joined once this has been called.
fn stop(child: &mut std::process::Child) {
    let _ = child.kill();
    let _ = child.wait();
}

/// Ask, and wait, and hand back what the harness printed, untouched.
///
/// Blocking, and called from `spawn_blocking` for the same reason `usage::read`
/// is.
///
/// **Both pipes are drained while the child runs**, on a thread apiece, and that
/// is a correction rather than a flourish. This used to read them only after the
/// child was gone, and it was safe for one reason and no other: the output was
/// bounded, because every caller asks for something small — `ask` for a single
/// line, `tracker::search` for at most twenty ids, in the instruction itself
/// rather than only in the parser.
///
/// What broke that was `oneshot_args` becoming a per-harness answer. The bound
/// was on what the *model* says, and Codex's one-shot form is
/// `codex exec --json`, which wraps that answer in a stream of its own events —
/// a `command_execution` item can carry aggregated output the prompt never asked
/// for. Past a pipe buffer, roughly 64 KB, the child blocks on the write, never
/// exits, and the deadline below kills it: the commit-message button would spin
/// for ninety seconds and then fail, on a question that was answered in the
/// first second.
///
/// So that invariant is **removed rather than re-stated**, since it was a
/// property of every caller's prompt and this function has no way to check one.
///
/// **The readers are joined on the ordinary path and deliberately not on the
/// two that give up.** This is the part that has to be right, and the reasoning
/// that looks obvious is wrong: killing the child does *not* close the pipes,
/// because the child is not necessarily the only thing holding them. Anything
/// the agent started inherited both descriptors, `stop` above signals one
/// process rather than a group, and a `read_to_end` with a live writer still on
/// the other end never returns — so a join there would wait for ever on exactly
/// the thing whose wait had just been given up on, wedging the `spawn_blocking`
/// thread for the life of the process and never handing the caller its
/// `Timeout`. Dropping the handles instead costs two detached threads, which is
/// the trade `vcs::run::bounded` already makes at the same point and for the
/// same reason.
///
/// **What that leaves is the ordinary path's own join, and it is unbounded.**
/// It rests on the harness being the last writer, so the read ends when the
/// harness does; an agent that leaves a background process holding stderr breaks
/// it, and the ceiling above is over the child rather than over this. It is
/// named here rather than fixed, exactly as `vcs::run::bounded` names it: the
/// fix is a channel with a deadline and a leaked thread, and the case has not
/// earned that yet. What it must not cost is somebody's afternoon in the poll
/// loop below.
pub fn ask_raw(
    profile: &'static dyn Profile,
    model: Option<&str>,
    prompt: &str,
) -> Result<String, OneshotError> {
    ask_within(profile, model, prompt, TIMEOUT)
}

/// The whole of `ask_raw` with the ceiling handed in.
///
/// Split out for `vcs::run::bounded`'s reason, which takes its own timeout as a
/// parameter: the give-up path is the one with the care in it, and a test of it
/// against the ninety seconds the product uses would be a test nobody runs.
fn ask_within(
    profile: &'static dyn Profile,
    model: Option<&str>,
    prompt: &str,
    timeout: Duration,
) -> Result<String, OneshotError> {
    let args =
        profile.oneshot_args().ok_or_else(|| OneshotError::Unsupported(profile.binary().into()))?;
    // The model, where the Default role named one. Asked of the profile rather
    // than written out here, exactly as the arguments above it are: a one-shot
    // is a session too, and there is no reason for the button in the Git panel
    // to reach a different model from the one everything else with no role of
    // its own reaches. Empty for a harness that cannot be told, and before the
    // prompt, which is positional.
    let model_args = model.map(|model| profile.model_args(model)).unwrap_or_default();
    let mut command = Command::new(profile.binary());
    command
        .args(args)
        .args(&model_args)
        .arg(prompt)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    // The login shell's PATH, for the reason `vcs/run.rs` and `runs/usage.rs`
    // both record: a bundled app inherits launchd's, where nothing a person
    // installed is reachable.
    if let Some(path) = crate::shell_env::path() {
        command.env("PATH", path);
    }

    let mut child = command.spawn().map_err(|err| match err.kind() {
        std::io::ErrorKind::NotFound => OneshotError::NoAgent(profile.binary().into()),
        _ => OneshotError::Io(err.to_string()),
    })?;

    // Taken off the child before the wait, so the readers own them: a pipe
    // nobody is emptying is what stalls a child with more to say than the
    // buffer holds. Bound by name rather than collected, so which stream a
    // handle carries is said on the line that made it rather than left to the
    // order two reads a dozen lines further down happen to be written in.
    let out = drain(child.stdout.take());
    let err = drain(child.stderr.take());

    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            // Stopped and reaped, and then **returned without joining** — see
            // the header. The readers may still be held open by something the
            // agent started, and waiting on them here is the unbounded hang
            // this ceiling exists to prevent.
            Ok(None) if Instant::now() >= deadline => {
                stop(&mut child);
                return Err(OneshotError::Timeout(timeout.as_secs()));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(100)),
            // The same treatment, for the same reason and one more: without the
            // `stop` this arm returns leaving a live child nobody will ever
            // reap, since dropping a `Child` neither kills nor waits.
            Err(err) => {
                stop(&mut child);
                return Err(OneshotError::Io(err.to_string()));
            }
        }
    };

    // A thread that panicked leaves this call with no output rather than with no
    // answer: what it was carrying is a message to read, and the exit status is
    // the fact the caller branches on.
    let printed = out.join().unwrap_or_default();
    let said = err.join().unwrap_or_default();

    if !status.success() {
        let stderr = String::from_utf8_lossy(&said).trim().to_string();
        return Err(OneshotError::Failed(if stderr.is_empty() {
            format!("{} exited {}.", profile.binary(), status.code().unwrap_or(-1))
        } else {
            stderr
        }));
    }

    // The profile's own reading of its output, not this function's: a harness
    // whose one-shot mode prints the answer alone keeps the default, and
    // nothing changes for it.
    let printed = String::from_utf8_lossy(&printed);
    let answer = profile.oneshot_answer(&printed).trim().to_string();
    if answer.is_empty() {
        // A zero exit and nothing to show for it. Silence is the one outcome
        // that must not reach the field, since an empty field after a spinner
        // is indistinguishable from a button that did nothing.
        return Err(OneshotError::Failed(format!("{} answered with nothing.", profile.binary())));
    }
    Ok(answer)
}

/// Ask for one line, and take one line: the shape a commit message wants.
///
/// The refusal is repeated rather than left to `ask_raw` alone, because the two
/// emptinesses are different facts: that one is a harness that printed nothing,
/// this one is a harness that printed something `clean` found no line in — a
/// bare code fence, say. Both reach the field as the same sentence, since from
/// where a person is sitting they are the same nothing.
pub fn ask(
    profile: &'static dyn Profile,
    model: Option<&str>,
    prompt: &str,
) -> Result<String, OneshotError> {
    let message = clean(&ask_raw(profile, model, prompt)?);
    if message.is_empty() {
        return Err(OneshotError::Failed(format!("{} answered with nothing.", profile.binary())));
    }
    Ok(message)
}

#[cfg(all(test, unix))]
mod spawn_tests {
    use super::*;

    /// A profile whose binary is the shell, so the prompt this module appends as
    /// the positional argument becomes the script that runs. Nothing about the
    /// product uses this; it is the only way to hand `ask_raw` a process whose
    /// output this test chooses.
    ///
    /// Unix only, and gated rather than skipped: `sh` is not on the Windows
    /// target, and a test that quietly passes by not running is worse than one
    /// that is honestly absent.
    struct Sh;

    impl Profile for Sh {
        fn id(&self) -> &'static str {
            "sh"
        }
        fn label(&self) -> &'static str {
            "Shell"
        }
        fn binary(&self) -> &'static str {
            "sh"
        }
        fn delivery(&self) -> crate::agents::SkillDelivery {
            crate::agents::SkillDelivery::Inline
        }
        /// One entry so the trait is satisfied; nothing here asks for a model.
        fn models(&self) -> &'static [(&'static str, &'static str)] {
            &[("sh", "Shell")]
        }
        fn command(&self, _launch: &crate::agents::Launch) -> portable_pty::CommandBuilder {
            portable_pty::CommandBuilder::new(self.binary())
        }
        fn oneshot_args(&self) -> Option<&'static [&'static str]> {
            Some(&["-c"])
        }
    }

    /// The reason both pipes are drained while the child runs rather than after
    /// it is gone.
    ///
    /// A pipe buffer is about 64 KB, and a child with more to say than that
    /// blocks on the write until somebody empties it. Under the old shape
    /// nobody did until the child had exited, so it never exited, and the
    /// deadline killed it ninety seconds later — the commit-message button
    /// spinning that whole time over a question answered in the first second.
    ///
    /// 300 KB is comfortably past the buffer on every platform this ships on,
    /// and it is what `codex exec --json` can put around one answer: the stream
    /// carries the harness's own events, and a `command_execution` item can
    /// carry aggregated output the prompt never asked for. The bound this
    /// function used to rely on was on what the *model* says, which is no longer
    /// the same thing as what the process prints.
    #[test]
    fn a_harness_that_prints_more_than_a_pipe_holds_still_answers() {
        let started = Instant::now();
        let answer = ask_raw(&Sh, None, "printf 'x%.0s' $(seq 1 300000); printf '\nthe answer\n'")
            .expect("a child that outlives its pipe buffer is not a failure");
        assert!(answer.ends_with("the answer"), "the whole of stdout comes back");
        assert!(
            answer.len() > 300_000,
            "nothing was dropped on the way: {} bytes",
            answer.len()
        );
        assert!(
            started.elapsed() < TIMEOUT,
            "it answered rather than being killed at the deadline"
        );
    }

    /// The other pipe, on its own, since a harness that writes its progress to
    /// stderr stalls exactly the same way and the failure looks like a timeout
    /// rather than like a full pipe.
    #[test]
    fn a_harness_that_fills_the_error_pipe_stalls_no_more_than_the_other_one() {
        let answer = ask_raw(&Sh, None, "printf 'e%.0s' $(seq 1 300000) >&2; printf 'the answer\n'")
            .expect("stderr is drained too");
        assert_eq!(answer, "the answer");
    }

    /// And a failure still carries the harness's own words rather than an exit
    /// code, which is what the drained stderr is for.
    #[test]
    fn a_child_that_failed_reaches_the_panel_in_its_own_words() {
        let err = ask_raw(&Sh, None, "echo 'no model configured' >&2; exit 3")
            .expect_err("a non-zero exit is a failure");
        assert!(
            matches!(&err, OneshotError::Failed(said) if said == "no model configured"),
            "{err:?}"
        );
    }

    /// Run `ask_within` on a thread and refuse to wait for ever on it.
    ///
    /// The whole point of the test below is a shape that **hangs** when it is
    /// wrong, and a hanging test is worse than no test: it wedges the suite with
    /// nothing to say why. So the answer is collected through a channel with a
    /// ceiling of its own, and a miss is a failure with a sentence on it. The
    /// thread is left behind in that case, which is acceptable in the one run
    /// that is already failing.
    fn within(prompt: &'static str, timeout: Duration, patience: Duration) -> OneshotError {
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(ask_within(&Sh, None, prompt, timeout).err());
        });
        match rx.recv_timeout(patience) {
            Ok(Some(err)) => err,
            Ok(None) => panic!("expected the deadline to fire"),
            Err(_) => panic!(
                "ask_within never returned: the give-up path is waiting on something unbounded"
            ),
        }
    }

    /// The reason the readers are not joined once the deadline has fired.
    ///
    /// The direct child is killed and reaped, but a **grandchild inherited both
    /// pipes** and is still holding them, so `read_to_end` cannot return and a
    /// join on it would never return either — the caller would never get its
    /// `Timeout` and the `spawn_blocking` thread would be wedged for the life of
    /// the process. That is strictly worse than the stall the draining was
    /// added to fix, which at least ended after ninety seconds.
    ///
    /// `trap "" TERM` is inherited across fork and exec, so the grandchild
    /// survives anything short of the group kill this function deliberately does
    /// not do (see `stop`). It is the case `vcs::run`'s own
    /// `the_kill_still_reaches_a_grandchild_that_refuses_to_stop` covers from
    /// the other side: that one proves the signal arrives, this one proves the
    /// caller is answered whether it arrives or not.
    #[test]
    fn a_descendant_still_holding_the_pipes_does_not_hold_the_deadline() {
        let err = within(
            "sh -c 'trap \"\" TERM; sleep 30' & wait",
            Duration::from_millis(400),
            Duration::from_secs(10),
        );
        assert!(matches!(err, OneshotError::Timeout(_)), "{err:?}");
    }

    /// The same guarantee where nothing was inherited at all, which is the case
    /// that already worked and is worth keeping honest beside the one that did
    /// not: a plain child that will not finish is still answered on time.
    #[test]
    fn a_child_that_will_not_finish_is_given_up_on_and_reaped() {
        let err = within("sleep 30", Duration::from_millis(400), Duration::from_secs(10));
        assert!(matches!(err, OneshotError::Timeout(_)), "{err:?}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ask_still_cleans_what_ask_raw_returns() {
        // The split must leave the commit box exactly where it was: `clean` is
        // what turns a fenced, quoted, prefaced answer into one line, and it now
        // lives in the wrapper rather than in the body.
        assert_eq!(clean("```\n\"fix: a thing\"\n```"), "fix: a thing");
        assert_eq!(clean("\n\nfix: a thing\nand a body\n"), "fix: a thing");
    }

    #[test]
    fn a_fence_and_a_preamble_leave_the_message_behind() {
        assert_eq!(clean("```\nfix: stop the panel losing its scroll\n```"), "fix: stop the panel losing its scroll");
        assert_eq!(clean("\n\n  feat: add a commit box  \n"), "feat: add a commit box");
    }

    #[test]
    fn a_body_after_the_subject_is_dropped() {
        assert_eq!(
            clean("chore: bump the sidecar\n\nIt was three versions behind.\n"),
            "chore: bump the sidecar"
        );
    }

    #[test]
    fn one_pair_of_quotes_comes_off_and_a_lone_one_stays() {
        assert_eq!(clean("\"docs: rewrite the panel's section\""), "docs: rewrite the panel's section");
        assert_eq!(clean("`fix: the branch list`"), "fix: the branch list");
        assert_eq!(clean("fix: quote the \"name\""), "fix: quote the \"name\"");
        assert_eq!(clean("fix: don't lose the tick"), "fix: don't lose the tick");
    }

    #[test]
    fn nothing_at_all_is_an_empty_message_rather_than_a_panic() {
        assert_eq!(clean(""), "");
        assert_eq!(clean("\n\n```\n```\n"), "");
    }

    #[test]
    fn the_prompt_carries_the_stat_the_untracked_and_the_patch() {
        let prompt =
            commit_prompt("English", " a.rs | 2 +-\n", &["b.rs".to_string()], "diff --git a/a.rs\n");
        assert!(prompt.contains("Conventional Commits"));
        assert!(prompt.contains("a.rs | 2 +-"));
        assert!(prompt.contains("New files, not yet tracked by git:\nb.rs"));
        assert!(prompt.contains("diff --git a/a.rs"));
    }

    #[test]
    fn an_empty_section_is_left_out_rather_than_left_blank() {
        let prompt = commit_prompt("English", "", &[], "");
        assert!(!prompt.contains("Files changed"));
        assert!(!prompt.contains("New files"));
        assert!(!prompt.contains("The diff"));
    }

    #[test]
    fn a_long_patch_is_cut_and_says_so() {
        let patch = "x".repeat(MAX_PATCH * 2);
        let prompt = commit_prompt("English", "", &[], &patch);
        assert!(prompt.contains("was cut here"));
        assert!(prompt.len() < MAX_PATCH * 2);
    }

    #[test]
    fn a_patch_under_the_ceiling_is_whole_and_unannounced() {
        let prompt = commit_prompt("English", "", &[], "diff --git a/a.rs\n+one line\n");
        assert!(prompt.contains("+one line"));
        assert!(!prompt.contains("was cut here"));
    }

    #[test]
    fn the_chosen_language_is_named_in_the_instruction() {
        // The literal `in English` is gone from this file: what the person
        // chose is what the model is asked for, and the name comes in already
        // resolved by `agents::language_name`.
        let prompt = commit_prompt("Russian", "", &[], "");
        assert!(prompt.contains("Write the subject in Russian"), "{prompt}");
        assert!(!prompt.contains("in English, in the Conventional Commits"), "{prompt}");
    }

    #[test]
    fn the_form_stays_english_whatever_the_subject_is_written_in() {
        // The setting moves prose, not literals: the type in front of the
        // colon is grepped and read rather than translated, and the two limits
        // around it are the same sentence in every language.
        for language in ["English", "Russian", "Chinese (Simplified)", "Turkish"] {
            let prompt = commit_prompt(language, "", &[], "");
            assert!(prompt.contains("`type: subject`"), "{language}: {prompt}");
            assert!(
                prompt.contains("(feat, fix, docs, refactor, test, chore)"),
                "{language}: {prompt}"
            );
            assert!(prompt.contains("imperative mood"), "{language}: {prompt}");
            assert!(prompt.contains("at most 72 characters"), "{language}: {prompt}");
            assert!(
                prompt.contains("stays English"),
                "{language}: the type must be exempted out loud: {prompt}"
            );
        }
    }

    /// The cut is by character and not by byte, so a diff whose 48 000th
    /// character is a multibyte one is still a `String` afterwards rather than
    /// a panic on a boundary.
    #[test]
    fn cutting_lands_on_a_character_boundary() {
        let patch = "é".repeat(MAX_PATCH * 2);
        let prompt = commit_prompt("English", "", &[], &patch);
        assert!(prompt.contains("was cut here"));
    }
}
