//! Driving an agent over its own protocol, rather than reading the terminal it
//! paints. See `docs/superpowers/specs/2026-09-10-conversation-ui-design.md`.

pub mod commands;
pub mod driver;
pub mod history;
pub mod journal;
pub mod model;
pub mod permission;
pub mod service;
