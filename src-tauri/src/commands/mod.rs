//! The `#[tauri::command]` wrappers: one file per worker module of `crates/core`.
//!
//! Each command here is the front end's name for a function of the core crate and does no more than
//! hand it its arguments — `State<T>` becomes `&T`, an `AppHandle` becomes the `Context` or the
//! handle the function wants. The logic, and the reasoning that goes with it, is in the matching
//! `commands.rs` of the core, where the headless server can call it too. What is here instead of
//! there is whatever needs the application itself: a restart, the system clipboard, the opener.

pub mod agents;
pub mod attachments;
pub mod client_files;
pub mod files;
pub mod git;
pub mod runs;
pub mod session;
pub mod sessions;
pub mod settings;
pub mod terminal;
pub mod tracker;
pub mod vcs;
