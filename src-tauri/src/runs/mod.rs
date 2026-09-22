//! Everything about a *run*: a batch of tracker work carried out by agent
//! sessions, from the shape of the project it happens in to the loop that
//! sequences the batches.
//!
//! This stage holds only the two parts that need no worker at all. Reading a
//! config file and walking a folder cost milliseconds and hold no state, so
//! there is nothing for a queue to guard — the same reasoning that keeps
//! `files/` and `git.rs` out of a worker.

pub mod awake;
pub mod browser;
pub mod commands;
pub mod config;
// The service consumes this once attempt rotation is wired; keeping the pure
// selector independently testable also lets its policy be validated without a
// live harness or process group.
#[allow(dead_code)]
pub mod failover;
pub mod gitignore;
pub mod journal;
pub mod model;
pub mod preflight;
pub mod procs;
pub mod queue;
pub mod recovery;
pub mod registry;
pub mod report;
pub mod reports;
pub mod service;
pub mod setup_facts;
pub mod summary;
pub mod survey;
pub mod usage;
