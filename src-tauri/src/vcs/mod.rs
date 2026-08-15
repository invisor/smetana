//! Everything that needs the git binary.
//!
//! There are two git modules in this tree and the split is by mechanism, not by
//! subject. `git.rs` reads what git keeps on disk in plain form — `HEAD`,
//! `refs/heads`, `packed-refs`, the reflogs — and spawns nothing, because the
//! scope bar's branch is refreshed on every window focus and a process for one
//! line is not worth it. Nothing in this module can be read that way: the state
//! of a working tree, the content of a diff, and the whole of checkout, merge
//! and rebase are what the binary is for.
//!
//! Do not fold the two together. Doing it one way drags process spawning into a
//! file whose own header forbids it; doing it the other way makes every cheap
//! read pay for a process.
//!
//! There is no worker here, for the reason `files/` has none: `git status`
//! costs tens of milliseconds against a bd call's two seconds, and this module
//! owns no snapshot — the front end holds the list. Concurrent writes are
//! serialised by git's own `index.lock`, whose refusal is shown as it is.

pub mod model;
