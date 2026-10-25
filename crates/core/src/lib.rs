//! The part of Smetana that does the work, with no window: the tracker worker and the bd sidecar,
//! PTY and driven sessions, runs, worktrees, files, git and the settings file.
//!
//! It has no `tauri` in its dependency tree, on purpose. Where a worker used to reach for an
//! `AppHandle` it is given a [`context::Context`] instead — the data directory, an event sink and the
//! path of the sidecar — and the desktop shell (`src-tauri`) builds one from its `AppHandle` in
//! `host.rs`. The headless server in `docs/superpowers/specs/2026-10-25-headless-server-design.md`
//! builds its own.
//!
//! What stays out is whatever belongs to the machine a person sits at: the `#[tauri::command]`
//! wrappers (each module's `commands.rs` holds the logic they call, as plain functions), windows, the
//! updater, the login item, the system clipboard and the opener. `remote` here is only the push half
//! of the companion; its endpoint and pairing are the shell's.

pub mod agents;
pub mod attachments;
pub mod context;
pub mod files;
pub mod git;
pub mod project;
pub mod remote;
pub mod rlimit;
pub mod runs;
pub mod session;
pub mod sessions;
pub mod settings;
pub mod shell_env;
pub mod terminal;
pub mod tracker;
pub mod vcs;
