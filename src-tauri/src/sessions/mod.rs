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
//! **The title is Claude Code's own `ai-title` when the file has one, and the
//! first thing the person typed when it does not.** It is not a `summary`
//! record: Claude Code writes those, and there are none in a fresh file — 0 of
//! the last 25 checked — so a title taken from them would be absent from every
//! recent session.
//!
//! The person's first words were the whole rule until the `ai-title` record was
//! measured, and the measurement is what changed it. A person who opens nearly
//! every session with the same standing instruction gets the same title on
//! nearly every row: of 299 sessions here, **142 carried one identical phrase**
//! and there were 122 distinct titles between them, so the column read as one
//! sentence repeated and a row was told apart only by its last message. The
//! generated record is in 211 of those files and says what the session was
//! about; taking it gives **214 distinct titles**, and the commonest one is
//! down to 60. `model::generated_title` is that rule, `model::human_text` is
//! the fallback, and both carry their own reasoning — the first `user` record
//! is usually not a person talking at all, but a hook's output, a skill's body
//! carried with `isMeta`, or the echo of a slash command.
//!
//! It is bought inside the existing budget rather than paid for: the record
//! sits tens of kilobytes into a file but only 7 to 88 *lines* in, well within
//! the head the forward pass already parses, so `sessions_list` takes the same
//! 330–355 ms it did before. `read::HEAD_LINES` holds those measurements, and
//! a file that ever carried the record past that line falls back to the person
//! rather than being chased for it.
//!
//! One thing found on disk that this had to learn about, recorded so the next
//! person does not have to find it again: subagent turns have moved out of the
//! transcript into `<session id>/subagents/`, which `read::subagents` does have
//! to know about, because otherwise the count is zero for every session written
//! by a current Claude Code.

//! `act.rs` is the deliberate exception to that, and its own header says why:
//! everything in it is a verb somebody pressed, so each answers with a sentence
//! when it cannot be done. The list stays silent; the menu does not. Which
//! verbs there are is that file's business and is deliberately not counted
//! here — the count has moved once already, and two headers of one module
//! disagreeing is worse than one of them saying less.

pub mod act;
pub mod commands;
pub mod model;
pub mod read;
