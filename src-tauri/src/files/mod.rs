//! The project's files: the tree in the left panel and the tabs in the centre.
//!
//! `model.rs` is the vocabulary and the pure logic and carries the tests,
//! `fs.rs` is the disk, `commands.rs` is thin commands over it.
//!
//! **A listing is a `read_dir` and one spawn of git.** It was only the first for
//! most of this module's life, and `fs.rs`'s header still opens on what follows
//! from that — no worker, no queue, no watcher — which is all still true. What is
//! no longer true is the cost: `list_dir` asks `git check-ignore` which of the
//! entries it just read are ignored, so the tree can draw those rows muted, and
//! that is a process of about 5-10 ms on top of a `read_dir` measured in
//! fractions of one. The multiplier worth knowing is the focus sweep — `catchUp`
//! in `views/DesktopApp.vue` re-lists **every open folder** when the window is
//! focused, so a focus now costs one git spawn per expanded folder, a dozen at
//! worst and tens of milliseconds altogether. All of it is off the UI thread, in
//! an async command; none of it can fail in a way a person sees.

pub mod commands;
pub mod fs;
pub mod model;
