//! Everything about a *run*: a batch of tracker work carried out by agent
//! sessions, from the shape of the project it happens in to the loop that
//! sequences the batches.
//!
//! This stage holds only the two parts that need no worker at all. Reading a
//! config file and walking a folder cost milliseconds and hold no state, so
//! there is nothing for a queue to guard — the same reasoning that keeps
//! `files/` and `git.rs` out of a worker.

pub mod commands;
pub mod config;
pub mod survey;
