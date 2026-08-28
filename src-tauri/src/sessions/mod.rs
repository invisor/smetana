//! The Sessions tab: what has been talked about in this project, off disk.
//!
//! Claude Code writes every session it runs to
//! `~/.claude/projects/<working directory>/<session id>.jsonl`, one JSON object
//! per line, and keeps them for good — 276 of them for this project alone. That
//! is a history nothing in the app knew existed: the tab used to draw
//! `agentRows`, the PTY sessions of the **current run of the app**, so a
//! freshly started app showed an empty list of a person's entire working
//! history. This module is the other source, and the live sessions stay where
//! they are, in the left column.
//!
//! `model.rs` is the vocabulary and every rule that can be decided from text,
//! `read.rs` is the disk, `act.rs` is the two verbs a session row's menu has
//! that touch a file rather than read one, and `commands.rs` is the thin layer
//! over both. There is
//! no worker, no queue and no watcher, which is the shape `git.rs` and `files/`
//! already have and for the same reason: the list is read when the tab is
//! opened and when the project changes, and a watcher over a folder that is
//! 844 MB and written by every live session would be a subsystem bought for a
//! read that takes a fraction of a second.
//!
//! **Nothing that reads is an error.** A machine with no `~/.claude/projects`, a
//! folder that cannot be read, a file that cannot be opened, a line that is not
//! JSON, a record type Claude Code invented last week — every one of them means
//! "there is nothing to show for that", which is why the command hands back a
//! plain `Vec` and not a `Result`. `git.rs`'s header argues this at length: a
//! failure toast for a state that is perfectly ordinary is noise, and a person
//! who has never run Claude Code in this project has done nothing wrong. One
//! corrupt line never costs the rest of its file, either.
//!
//! **Membership is decided by the `cwd` inside the file, never by the folder's
//! name.** The name is the working directory with its separators replaced, and
//! that transform is not invertible — a `-` in it could have been a separator,
//! a dot, or a `-` somebody typed — so a folder name cannot say which project a
//! session belongs to. `cwd` can, and it is exact. A session belongs when its
//! `cwd` is the project folder or lies inside it: the root, a worktree under
//! `.worktrees/`, `src-tauri`. The name is used only as a prefilter that can
//! rule a folder out, and `model::folder_could_hold` records why even that is
//! safe.
//!
//! **The title is the first thing the person typed, and it is not a `summary`
//! record.** Claude Code writes `summary` records, and there are none in a
//! fresh file — 0 of the last 25 checked — so a title taken from them would be
//! absent from every recent session. What is there is the conversation itself,
//! and the first `user` record is usually not a person talking: it is a hook's
//! output, a skill's body carried on a `user` record with `isMeta`, or the echo
//! of a slash command wrapped in `<command-name>`. `model::human_text` is that
//! rule and carries the reasoning.
//!
//! Two things found on disk that this deliberately does **not** use, recorded
//! so the next person does not have to find them again. Recent transcripts
//! carry an `ai-title` record — a generated one-line title of the whole
//! session, in 210 of this project's 276 files — which reads far better than
//! the first human message when the person opens every session with the same
//! standing instruction, as this one does; using it is a change to what `title`
//! means and so a decision rather than an implementation detail. And subagent
//! turns have moved out of the transcript into `<session id>/subagents/`, which
//! `read::subagents` does have to know about, because otherwise the count is
//! zero for every session written by a current Claude Code.

//! `act.rs` is the deliberate exception to that, and its own header says why:
//! opening a transcript and deleting one are verbs somebody pressed, so each
//! answers with a sentence when it cannot be done. The list stays silent; the
//! menu does not.

pub mod act;
pub mod commands;
pub mod model;
pub mod read;
