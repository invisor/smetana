//! `smetanad`, the headless Smetana server: the core and the iroh endpoint behind a command line.
//!
//! `server` is `smetanad run`; `client`, `local` and `ops` are the other subcommands, which are
//! clients of a running server: they reach it the way a desktop does, over its loopback endpoint,
//! paired by the credential file in the data directory, and call the same commands a desktop calls.
//! There is no second control channel and no signal or pid-file protocol to keep in step with the
//! first.

pub mod cli;
pub mod client;
pub mod inherit;
pub mod local;
pub mod logger;
pub mod ops;
pub mod server;

/// The version of this build of the server, which is the app's: `build.rs` reads it from `tauri.conf.json`,
/// the one place a release bumps (the crate manifests stay 0.1.0). A desktop that ships one compares it with what a running
/// server says in its `HelloReply` and replaces an older one through the restart gateway.
pub const VERSION: &str = env!("SMETANA_APP_VERSION");
