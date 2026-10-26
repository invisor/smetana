//! What a worker is given instead of an `AppHandle`: the data directory, a sink for events, the
//! path of the bd sidecar — and nothing else.
//!
//! The workers used to reach all three through `tauri::AppHandle` (`app_data_dir()`, `Emitter::emit`,
//! the shell plugin's `sidecar()`), which tied every module that does work to a running Tauri
//! application and so to a window. This file is Tauri-free on purpose: the headless server in
//! `docs/superpowers/specs/2026-10-25-headless-server-design.md` has no application to take an
//! `AppHandle` from, and the thing it builds in its place is a `Context` of exactly this shape.
//! `host.rs` is the one place that makes one out of an `AppHandle`.
//!
//! Two paths beyond the data directory are here because they are not derivable from it, and each is a
//! plain fact about the machine rather than a convenience: `settings.json` lives in the platform's
//! *config* directory, which is not the data directory on Linux or Windows, and the skill library is
//! read from the bundle's *resource* directory. Collapsing either into `data_dir` would have moved a
//! file on two of the three platforms.

use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use serde::Serialize;

/// Where everything a worker says to the outside goes.
///
/// Events and pushes are one trait rather than two because they have one implementor and one lifetime:
/// the thing that owns the windows and the paired phones is the same thing that can reach both.
pub trait Sink: Send + Sync {
    /// Delivers an event under `name` to whoever is listening. Delivery is best effort and never
    /// fails the caller, as `let _ = app.emit(..)` never did.
    fn emit(&self, name: &str, payload: serde_json::Value);

    /// Hands a push message to every paired phone that registered a token, without waiting for it.
    fn push(&self, message: crate::remote::push::Message);

    /// One committed chunk of a terminal session's output, for the clients that follow that
    /// session over a connection. Called for every session, whether or not a window has it on
    /// screen, and in order: `seq` is the session's own counter and rises by one per call. The
    /// default is to drop it, which is what a sink with no connections wants.
    fn terminal_chunk(&self, _session: u64, _seq: u64, _bytes: &[u8]) {}

    /// New entries of a timeline session's structured log, on the same terms as
    /// [`Sink::terminal_chunk`]. `entries` is the JSON of the list `terminal:log` carries.
    fn terminal_log(&self, _session: u64, _entries: &serde_json::Value) {}
}

/// A sink that drops everything.
struct Nowhere;

impl Sink for Nowhere {
    fn emit(&self, _name: &str, _payload: serde_json::Value) {}
    fn push(&self, _message: crate::remote::push::Message) {}
}

/// The data directory, the event sink and the sidecar path, cheap to clone and share.
#[derive(Clone)]
pub struct Context {
    inner: Arc<Inner>,
}

struct Inner {
    /// Each directory keeps the platform's reason for not having one, as text, because that is what
    /// the callers put in their own error messages.
    data_dir: Result<PathBuf, String>,
    config_dir: Result<PathBuf, String>,
    resource_dir: Result<PathBuf, String>,
    sidecar: Option<PathBuf>,
    sink: Arc<dyn Sink>,
}

impl Context {
    pub fn new(
        data_dir: Result<PathBuf, String>,
        config_dir: Result<PathBuf, String>,
        resource_dir: Result<PathBuf, String>,
        sidecar: Option<PathBuf>,
        sink: Arc<dyn Sink>,
    ) -> Self {
        Self { inner: Arc::new(Inner { data_dir, config_dir, resource_dir, sidecar, sink }) }
    }

    /// A context with no directories, no sidecar and a sink that drops everything: for a test
    /// that needs the type, or a tool that only reads.
    pub fn detached() -> Self {
        let none = || Err("detached".to_owned());
        Self::new(none(), none(), none(), None, Arc::new(Nowhere))
    }

    /// The folder this app owns for what it keeps: attachments, the device key, transcripts.
    pub fn data_dir(&self) -> Result<&Path, &str> {
        self.inner.data_dir.as_deref().map_err(String::as_str)
    }

    pub fn config_dir(&self) -> Result<&Path, &str> {
        self.inner.config_dir.as_deref().map_err(String::as_str)
    }

    /// Where `settings.json` is — `None` only when the platform will not name a config directory.
    pub fn settings_path(&self) -> Option<PathBuf> {
        self.config_dir().ok().map(|dir| dir.join("settings.json"))
    }

    /// The bundle's resource directory, the one the skill library ships in.
    pub fn resource_dir(&self) -> Result<&Path, &str> {
        self.inner.resource_dir.as_deref().map_err(String::as_str)
    }

    /// The bundled `bd` binary. `None` when the running executable has no discoverable location.
    pub fn sidecar(&self) -> Option<&Path> {
        self.inner.sidecar.as_deref()
    }

    /// The directory the bundled `bd` sits in, which an agent's `PATH` is put in front of.
    pub fn sidecar_dir(&self) -> Option<&Path> {
        self.sidecar().and_then(Path::parent)
    }

    /// Emits `payload` as `name`. A payload that does not serialize is dropped, as `emit` always did.
    pub fn emit<S: Serialize + ?Sized>(&self, name: &str, payload: &S) {
        match serde_json::to_value(payload) {
            Ok(value) => self.inner.sink.emit(name, value),
            Err(err) => log::warn!("event {name} does not serialize: {err}"),
        }
    }

    /// `emit` for a payload that is already JSON, which is moved in rather than copied: terminal
    /// output goes out at the rate the screen repaints, and a deep copy of every chunk is not free.
    pub fn emit_json(&self, name: &str, payload: serde_json::Value) {
        self.inner.sink.emit(name, payload);
    }

    /// See [`Sink::terminal_chunk`].
    pub fn terminal_chunk(&self, session: u64, seq: u64, bytes: &[u8]) {
        self.inner.sink.terminal_chunk(session, seq, bytes);
    }

    /// See [`Sink::terminal_log`].
    pub fn terminal_log(&self, session: u64, entries: &serde_json::Value) {
        self.inner.sink.terminal_log(session, entries);
    }

    pub fn push(&self, message: crate::remote::push::Message) {
        self.inner.sink.push(message);
    }
}

/// The runtime the workers' tasks run on.
///
/// `tauri::async_runtime::spawn` was a global, and this is the same global without Tauri's name on
/// it: a `setup` closure and a worker's own thread are not inside any runtime, so a bare
/// `tokio::spawn` would panic there. The host installs the handle once, before the first worker
/// starts; with none installed the call falls back to the runtime it is made from, which is what a
/// `#[tokio::test]` is.
static RUNTIME: OnceLock<tokio::runtime::Handle> = OnceLock::new();

pub fn install_runtime(handle: tokio::runtime::Handle) {
    let _ = RUNTIME.set(handle);
}

pub fn spawn<F>(future: F) -> tokio::task::JoinHandle<F::Output>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    match RUNTIME.get() {
        Some(handle) => handle.spawn(future),
        None => tokio::spawn(future),
    }
}
