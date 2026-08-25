//! The project's files: the tree in the left panel and the tabs in the centre.
//!
//! `model.rs` is the vocabulary and the pure logic and carries the tests,
//! `fs.rs` is the disk, `commands.rs` is thin commands over it.
//!
//! **A listing is a `read_dir` and one spawn of git.** It was only the first for
//! most of this module's life, and `fs.rs`'s header still opens on what follows
//! from that — no worker, no queue, no watcher — which is all still true. What is
//! no longer true is the cost: `list_dir` asks `git check-ignore` which of the
//! entries it just read are ignored, so the tree can draw those rows muted.
//!
//! **The numbers, measured rather than guessed** (Apple Silicon, macOS, warm
//! cache, `git check-ignore -z --stdin` in the shape `mark_git_ignored` makes
//! it). An ordinary listing of a couple of dozen names in this repository, whose
//! index holds 508 files: **7 ms median**, against a `read_dir` measured in
//! fractions of one. A full `MAX_ENTRIES` listing — 1000 names — in a small
//! repository: **16 ms**.
//!
//! **It is the index that scales it, not the listing.** `check-ignore` consults
//! the index, which is what buys the `git add -f` case, so the cost follows the
//! size of the repository rather than the size of the folder: the same 1000-name
//! listing measured against a 50 000-file index came to **0.5 s**. That is the
//! worst case worth knowing about — a wide folder in a monorepo — and what it
//! costs is the tree rows arriving a beat late, not a stalled window, for the
//! reason below.
//!
//! **Two multipliers.** `catchUp` in `views/DesktopApp.vue` re-lists **every open
//! folder** when the window is focused, so a focus costs one git spawn per
//! expanded folder — a dozen at worst — and `refreshDirs` in `stores/files.js`
//! fires them as a `Promise.all`, so they arrive at once rather than in turn.
//! That is precisely why `files_list` runs its work on the **blocking pool** and
//! not in the body of its `async fn`: see `commands.rs`, and `vcs/commands.rs`
//! for the rule it follows. None of it can fail in a way a person sees.

pub mod commands;
pub mod fs;
pub mod model;
