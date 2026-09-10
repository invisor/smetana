//! Driving an agent over its own protocol, rather than reading the terminal it
//! paints. See `docs/superpowers/specs/2026-09-10-conversation-ui-design.md`.

// The subsystem lands in pieces: this is its vocabulary and its memory, and the
// driver, the worker and the commands that call them arrive in later tasks of
// the same stage. Until then every item here is unreachable, which is eleven
// warnings on every build of a tree that otherwise has none — enough noise to
// hide a real one. **Delete this line with the task that adds `commands.rs`**;
// after that, dead code in here means dead code.
#![allow(dead_code)]

pub mod driver;
pub mod journal;
pub mod model;
pub mod permission;
