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
pub fn commit_prompt(stat: &str, untracked: &[String], patch: &str) -> String {
    let mut out = String::new();
    out.push_str(
        "Write a git commit message for the changes below.\n\
         \n\
         Answer with the message and nothing else: one line, at most 72 \
         characters, in English, in the Conventional Commits form \
         `type: subject` (feat, fix, docs, refactor, test, chore). Use the \
         imperative mood. Do not add a body, an explanation, quotation marks \
         or a code fence.\n",
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

/// Ask, and wait, and hand back what the harness printed, untouched.
///
/// Blocking, and called from `spawn_blocking` for the same reason `usage::read`
/// is.
///
/// One invariant travels with this function and has to be preserved by every
/// caller: **both pipes are read only after the child is gone**, which is safe
/// for one reason and no other — the output is bounded, so neither pipe can
/// fill and stall the child while we wait for it. `ask` below bounds it by
/// asking for a single line; `tracker::search` bounds it by asking for at most
/// twenty ids, in the instruction itself rather than only in the parser. A
/// caller that lets a model answer at length would fill a pipe and stall the
/// child until the deadline kills it.
pub fn ask_raw(profile: &'static dyn Profile, prompt: &str) -> Result<String, OneshotError> {
    let args =
        profile.oneshot_args().ok_or_else(|| OneshotError::Unsupported(profile.binary().into()))?;
    let mut command = Command::new(profile.binary());
    command
        .args(args)
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

    let deadline = Instant::now() + TIMEOUT;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if Instant::now() >= deadline => {
                let _ = child.kill();
                return Err(OneshotError::Timeout(TIMEOUT.as_secs()));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(100)),
            Err(err) => return Err(OneshotError::Io(err.to_string())),
        }
    }

    let out = child.wait_with_output().map_err(|err| OneshotError::Io(err.to_string()))?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
        return Err(OneshotError::Failed(if stderr.is_empty() {
            format!("{} exited {}.", profile.binary(), out.status.code().unwrap_or(-1))
        } else {
            stderr
        }));
    }

    let answer = String::from_utf8_lossy(&out.stdout).trim().to_string();
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
pub fn ask(profile: &'static dyn Profile, prompt: &str) -> Result<String, OneshotError> {
    let message = clean(&ask_raw(profile, prompt)?);
    if message.is_empty() {
        return Err(OneshotError::Failed(format!("{} answered with nothing.", profile.binary())));
    }
    Ok(message)
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
        let prompt = commit_prompt(" a.rs | 2 +-\n", &["b.rs".to_string()], "diff --git a/a.rs\n");
        assert!(prompt.contains("Conventional Commits"));
        assert!(prompt.contains("a.rs | 2 +-"));
        assert!(prompt.contains("New files, not yet tracked by git:\nb.rs"));
        assert!(prompt.contains("diff --git a/a.rs"));
    }

    #[test]
    fn an_empty_section_is_left_out_rather_than_left_blank() {
        let prompt = commit_prompt("", &[], "");
        assert!(!prompt.contains("Files changed"));
        assert!(!prompt.contains("New files"));
        assert!(!prompt.contains("The diff"));
    }

    #[test]
    fn a_long_patch_is_cut_and_says_so() {
        let patch = "x".repeat(MAX_PATCH * 2);
        let prompt = commit_prompt("", &[], &patch);
        assert!(prompt.contains("was cut here"));
        assert!(prompt.len() < MAX_PATCH * 2);
    }

    #[test]
    fn a_patch_under_the_ceiling_is_whole_and_unannounced() {
        let prompt = commit_prompt("", &[], "diff --git a/a.rs\n+one line\n");
        assert!(prompt.contains("+one line"));
        assert!(!prompt.contains("was cut here"));
    }

    /// The cut is by character and not by byte, so a diff whose 48 000th
    /// character is a multibyte one is still a `String` afterwards rather than
    /// a panic on a boundary.
    #[test]
    fn cutting_lands_on_a_character_boundary() {
        let patch = "é".repeat(MAX_PATCH * 2);
        let prompt = commit_prompt("", &[], &patch);
        assert!(prompt.contains("was cut here"));
    }
}
