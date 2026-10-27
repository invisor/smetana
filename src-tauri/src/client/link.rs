//! The connection to the server, kept alive.
//!
//! A [`Link`] is the one thing the rest of the shell talks to: it holds the connection when there is
//! one, the [`View`] the webview is told, and the rule for what a call does when there is none. The
//! [`supervise`] task is its only writer: it finds or starts the local server, connects, re-emits
//! the server's events into the webview, and when the connection ends it begins again.
//!
//! **No call is ever replayed.** A call made while the connection is down is refused with a reason
//! (`View::refusal`) and forgotten, and one in flight when the connection ends is answered with
//! what is known: that it was never sent, or that it may or may not have been done. The only call
//! that waits is one made while the *first* connection of this run is still being made, because the
//! app has nothing to draw until it exists and the stores read as it mounts.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use remote_protocol::{BridgeError, CLOSE_PROTOCOL, CLOSE_UNKNOWN, CLOSE_VERSION};
use serde_json::{json, Value};
use smetanad::client::{Client, ClientError, RemoteRecord, CLOSED_BEFORE_REPLY, SEND_FAILED};
use smetanad::server;
use tokio::sync::{watch, Notify};

use super::spawn::{self, Config};
use super::target::Target;
use super::state::{
    Backoff, Outdated, Phase, ServerInfo, View, FIRST_CONNECTION_WAIT, SLOW_POLL, STATE_EVENT,
};

/// Where events and the state go: the webview, in the app; a recorder, in a test.
pub type Sink = Arc<dyn Fn(&str, Value) + Send + Sync>;

struct Inner {
    view: View,
    client: Option<Arc<Client>>,
}

pub struct Link {
    sink: Sink,
    inner: Mutex<Inner>,
    /// Which server this device uses. Changed only by [`Link::set_target`], which also moves
    /// `generation`: the connection loop compares it to know that the one it holds is the old choice.
    target: Mutex<Target>,
    generation: std::sync::atomic::AtomicU64,
    /// Told when the target changes, so a connected loop does not sit on the server that was left.
    retarget: Notify,
    /// Bumped on every change of the view, so a call waiting for the first connection wakes.
    changed: watch::Sender<u64>,
    /// A person pressed retry: the supervisor stops waiting and tries now.
    wake: Notify,
}

impl Link {
    #[cfg(test)]
    pub fn new(sink: Sink, local: bool) -> Arc<Link> {
        Self::with_target(sink, if local { Target::Local } else { Target::Remote { node: String::new(), relay: String::new() } })
    }

    pub fn with_target(sink: Sink, target: Target) -> Arc<Link> {
        Arc::new(Link {
            sink,
            inner: Mutex::new(Inner {
                view: View { target: target.id(), ..View::new(target.is_local()) },
                client: None,
            }),
            target: Mutex::new(target),
            generation: std::sync::atomic::AtomicU64::new(0),
            retarget: Notify::new(),
            changed: watch::channel(0).0,
            wake: Notify::new(),
        })
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub fn view(&self) -> View {
        self.lock().view.clone()
    }

    pub fn target(&self) -> Target {
        self.target.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).clone()
    }

    fn generation(&self) -> u64 {
        self.generation.load(std::sync::atomic::Ordering::SeqCst)
    }

    /// Moves this device to another server. The connection to the one it leaves is dropped by the
    /// loop that holds it (the server itself is not touched: a local one keeps running), and the
    /// view goes back to "connecting" for the new one with its connection count kept, so the stores
    /// read their snapshots again when it is up.
    pub fn set_target(&self, target: Target) {
        {
            let mut held = self.target.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            if *held == target {
                return;
            }
            *held = target.clone();
        }
        self.generation.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        self.update(|inner| {
            inner.client = None;
            let view = &mut inner.view;
            view.local = target.is_local();
            view.target = target.id();
            view.state = Phase::Connecting;
            view.attempt = 0;
            view.reason = Some("switching server".to_owned());
            view.server = None;
            view.outdated = None;
        });
        self.retarget.notify_one();
        self.wake.notify_one();
    }

    /// Changes the view and tells the webview. The event carries the whole view, never a diff.
    fn update(&self, change: impl FnOnce(&mut Inner)) {
        let view = {
            let mut inner = self.lock();
            change(&mut inner);
            inner.view.clone()
        };
        self.changed.send_modify(|version| *version += 1);
        (self.sink)(STATE_EVENT, serde_json::to_value(&view).unwrap_or(Value::Null));
    }

    /// Asks the supervisor to try again now instead of at the end of its wait.
    pub fn retry_now(&self) {
        self.wake.notify_one();
    }

    /// Forwards one command. `Err` is what the webview's `invoke` is rejected with.
    pub async fn call(&self, cmd: &str, args: Value) -> Result<Value, Value> {
        let client = self.ready(super::forward::is_read(cmd)).await?;
        client.call(cmd, args).await.map_err(|err| rejection(cmd, err))
    }

    /// The connection, or the reason there is none.
    ///
    /// Only a read (`hold`) waits for the first connection; anything else is refused at once, the same
    /// as when the connection is down.
    async fn ready(&self, hold: bool) -> Result<Arc<Client>, Value> {
        let mut changes = self.changed.subscribe();
        let deadline = tokio::time::Instant::now() + FIRST_CONNECTION_WAIT;
        loop {
            let (view, client) = {
                let inner = self.lock();
                (inner.view.clone(), inner.client.clone())
            };
            if let (Phase::Connected, Some(client)) = (view.state, client) {
                return Ok(client);
            }
            if !(hold && view.first_connection_pending()) {
                return Err(Value::String(view.refusal()));
            }
            match tokio::time::timeout_at(deadline, changes.changed()).await {
                Ok(Ok(())) => {}
                _ => return Err(Value::String(view.refusal())),
            }
        }
    }

    fn connected(&self, client: Arc<Client>, server: ServerInfo, outdated: Option<Outdated>) {
        self.update(|inner| {
            inner.client = Some(client);
            let view = &mut inner.view;
            view.state = Phase::Connected;
            view.epoch += 1;
            view.attempt = 0;
            view.reason = None;
            view.server = Some(server);
            view.outdated = outdated;
        });
    }

    fn lost(&self, why: &str) {
        self.update(|inner| {
            inner.client = None;
            inner.view.state = Phase::Reconnecting;
            inner.view.reason = Some(why.to_owned());
        });
    }

    /// A state nothing here can mend: shown with its reason and left.
    pub fn broken(&self, reason: String) {
        self.update(|inner| {
            inner.view.state = Phase::Failed;
            inner.view.reason = Some(reason);
        });
    }

    fn failed(&self, phase: Phase, reason: String, server: Option<ServerInfo>) {
        self.update(|inner| {
            let view = &mut inner.view;
            // Failing before the first connection is still "connecting", and failing after one is
            // "reconnecting", unless the failure is one of the two that wait for a person.
            view.state = match phase {
                Phase::Incompatible | Phase::Unpaired => phase,
                _ if view.epoch == 0 => Phase::Connecting,
                _ => Phase::Reconnecting,
            };
            view.attempt += 1;
            view.reason = Some(reason);
            if server.is_some() {
                view.server = server;
            }
        });
    }

    async fn pause(&self, how_long: Duration) {
        let _ = tokio::time::timeout(how_long, self.wake.notified()).await;
    }
}

/// What a call that came back with an error is rejected with.
pub fn rejection(cmd: &str, err: BridgeError) -> Value {
    match err {
        // The handler's own error, in the form the webview's `invoke` would have received it.
        BridgeError::Handler(value) => value,
        BridgeError::Forbidden => {
            Value::String(format!("The server does not allow this device to call `{cmd}`."))
        }
        BridgeError::UnknownCommand => Value::String(format!(
            "The server does not know the command `{cmd}`; it may be older than this app."
        )),
        BridgeError::Internal(message) if message.starts_with(SEND_FAILED) => Value::String(
            "The connection to the server was lost before the request was sent. Nothing was changed.".to_owned(),
        ),
        BridgeError::Internal(message) if message == CLOSED_BEFORE_REPLY => Value::String(
            "The connection to the server was lost before it answered, so the change may or may not have been made. Look before doing it again."
                .to_owned(),
        ),
        BridgeError::Internal(message) => Value::String(message),
    }
}

/// How an attempt that did not connect failed.
#[derive(Debug, PartialEq)]
enum Failure {
    /// Will be tried again soon, with backoff.
    Transient(String),
    Unpaired(String),
    Incompatible(String, Option<ServerInfo>),
}

fn classify(err: ClientError) -> Failure {
    match err {
        ClientError::Setup(reason) | ClientError::Closed(reason) => Failure::Transient(reason),
        ClientError::Refused(reason) => Failure::Unpaired(format!("the server refused the pairing: {reason}")),
        ClientError::Code(CLOSE_UNKNOWN) | ClientError::Code(CLOSE_PROTOCOL) => {
            Failure::Unpaired("the server does not know this device".to_owned())
        }
        ClientError::Code(CLOSE_VERSION) => Failure::Incompatible("the server speaks another protocol version".to_owned(), None),
        ClientError::Code(code) => Failure::Transient(format!("the server closed the connection (code {code})")),
    }
}

/// What a local server running an older build is told, when it speaks a protocol this one cannot.
fn incompatible_reason(server: &ServerInfo, config: &Config) -> String {
    format!(
        "the server on this machine speaks protocol {}, this app {}; restart it with `smetanad restart --data-dir {}`, which asks before ending anything",
        server.protocol_major,
        remote_server::PROTOCOL_MAJOR,
        config.server_dir.display()
    )
}

/// Whether `running` is a later version than `bundled`, comparing dotted numbers. A version that
/// does not parse is not "newer": it is left to the ordinary mismatch path.
fn newer(running: &str, bundled: &str) -> bool {
    fn parts(text: &str) -> Option<Vec<u64>> {
        text.split(['.', '-', '+']).take_while(|p| p.chars().all(|c| c.is_ascii_digit()) && !p.is_empty()).map(|p| p.parse().ok()).collect()
    }
    match (parts(running), parts(bundled)) {
        (Some(a), Some(b)) if !a.is_empty() && !b.is_empty() => a > b,
        _ => false,
    }
}

enum Gate {
    /// It is older, and could not be replaced now.
    Outdated(Outdated),
    /// It was asked to restart and has said it will.
    Restarting,
}

/// A local server older than the one in the bundle is replaced through the restart gateway: nothing
/// is ended unless nothing is alive; otherwise the list of what would end is handed to the person.
async fn gate(client: &Client, running: &str, may_restart: bool) -> Gate {
    let mut outdated = Outdated {
        running: running.to_owned(),
        bundled: smetanad::VERSION.to_owned(),
        work: json!([]),
        can_restart: false,
    };
    let advertised = client.hello.capabilities.iter().any(|name| name == "server_restart");
    if !advertised || !may_restart {
        return Gate::Outdated(outdated);
    }
    match client.call("server_restart", json!({ "mode": "now", "confirmed": null })).await {
        Ok(reply) => match reply["outcome"].as_str() {
            Some("restarting") => Gate::Restarting,
            _ => {
                outdated.can_restart = true;
                outdated.work = reply.get("work").cloned().unwrap_or_else(|| json!([]));
                Gate::Outdated(outdated)
            }
        },
        Err(err) => {
            log::warn!("client: the older local server would not restart: {err}");
            Gate::Outdated(outdated)
        }
    }
}

enum Attempt {
    /// A connection was made and has now ended.
    Ended,
    Failed(Failure),
    /// The local server was asked to restart for the new version.
    Restarting,
}

async fn attempt(link: &Arc<Link>, config: &Config, restarted_from: &mut Option<String>) -> Attempt {
    let generation = link.generation();
    match link.target() {
        Target::Local => attempt_local(link, config, restarted_from, generation).await,
        Target::Remote { node, relay } => attempt_remote(link, config, RemoteRecord { node, relay }, generation).await,
    }
}

/// Subscribes to the events a server advertises. Attaching to a terminal is its own subscription
/// (`terminal_attach`), and `terminal:log` follows it, so neither is asked for here.
async fn listen_to_all(client: &Client) -> Result<(), Failure> {
    for name in client.hello.capabilities.iter().filter(|name| name.contains(':')) {
        if matches!(name.as_str(), "terminal:output" | "terminal:log") {
            continue;
        }
        if let Err(err) = client.listen(name).await {
            client.close();
            return Err(Failure::Transient(err.to_string()));
        }
    }
    Ok(())
}

/// Holds a connection: re-emits the server's events until it ends, the target changes, or (for the
/// server this app started) the server's lock is released. Answers why it ended, and whether it
/// ended because the person chose another server.
async fn hold(
    link: &Arc<Link>,
    client: &Arc<Client>,
    mut events: tokio::sync::mpsc::UnboundedReceiver<(String, Value)>,
    generation: u64,
    owned_dir: Option<&std::path::Path>,
) -> (&'static str, bool) {
    let mut watch_owner = tokio::time::interval(Duration::from_secs(1));
    loop {
        tokio::select! {
            event = events.recv() => match event {
                Some((name, payload)) => (link.sink)(&name, payload),
                None => return ("the connection to the server ended", false),
            },
            // A server killed without a goodbye is noticed by the transport only after its idle
            // timeout; its lock is released at once.
            _ = watch_owner.tick() => {
                if link.generation() != generation {
                    client.close();
                    return ("switching server", true);
                }
                if let Some(dir) = owned_dir {
                    if !server::is_owned(dir) {
                        return ("the server stopped", false);
                    }
                }
            }
            _ = link.retarget.notified() => {
                if link.generation() != generation {
                    client.close();
                    return ("switching server", true);
                }
            }
        }
    }
}

/// A server somewhere else. Its updates are its operator's, so none of the lock-step the local
/// server gets applies: a different major is the only mismatch that stops anything, and the reason
/// says which side to update.
async fn attempt_remote(link: &Arc<Link>, config: &Config, record: RemoteRecord, generation: u64) -> Attempt {
    let mut client = match Client::connect_remote(&config.identity_dir, &record).await {
        Ok(client) => client,
        Err(err) => return Attempt::Failed(classify(err)),
    };
    let hello = client.hello.clone();
    let info = ServerInfo {
        version: hello.app_version.clone(),
        protocol_major: hello.protocol_major,
        protocol_minor: hello.protocol_minor,
    };
    if hello.protocol_major != remote_server::PROTOCOL_MAJOR {
        client.close();
        return Attempt::Failed(Failure::Incompatible(remote_incompatible_reason(&info), Some(info)));
    }
    let Some(events) = client.take_events() else {
        return Attempt::Failed(Failure::Transient("the connection has no event stream".to_owned()));
    };
    let client = Arc::new(client);
    if let Err(failure) = listen_to_all(&client).await {
        return Attempt::Failed(failure);
    }
    // The person may have chosen another server while this one was being reached.
    if link.generation() != generation {
        client.close();
        return Attempt::Ended;
    }
    link.connected(client.clone(), info, None);
    let (why, switched) = hold(link, &client, events, generation, None).await;
    if !switched {
        link.lost(why);
    }
    client.close();
    Attempt::Ended
}

/// What a remote server of another major is told: which side is older cannot be known from a number
/// alone, but the numbers are, so they are named.
fn remote_incompatible_reason(server: &ServerInfo) -> String {
    let ours = remote_server::PROTOCOL_MAJOR;
    let which = if server.protocol_major < ours {
        "update smetanad on the server"
    } else {
        "update this app"
    };
    format!("the server speaks protocol {}, this app {ours}; {which}", server.protocol_major)
}

async fn attempt_local(
    link: &Arc<Link>,
    config: &Config,
    restarted_from: &mut Option<String>,
    generation: u64,
) -> Attempt {
    {
        let config = config.clone();
        match tokio::task::spawn_blocking(move || spawn::ensure(&config)).await {
            Ok(Ok(())) => {}
            Ok(Err(reason)) => return Attempt::Failed(Failure::Transient(reason)),
            Err(err) => return Attempt::Failed(Failure::Transient(err.to_string())),
        }
    }
    let mut client = match Client::connect_as(&config.server_dir, &config.identity_dir).await {
        Ok(client) => client,
        Err(err) => return Attempt::Failed(classify(err)),
    };
    let hello = client.hello.clone();
    let info = ServerInfo {
        version: hello.app_version.clone(),
        protocol_major: hello.protocol_major,
        protocol_minor: hello.protocol_minor,
    };
    if hello.protocol_major != remote_server::PROTOCOL_MAJOR {
        client.close();
        let reason = incompatible_reason(&info, config);
        return Attempt::Failed(Failure::Incompatible(reason, Some(info)));
    }
    let Some(events) = client.take_events() else {
        return Attempt::Failed(Failure::Transient("the connection has no event stream".to_owned()));
    };
    let client = Arc::new(client);
    if let Err(failure) = listen_to_all(&client).await {
        return Attempt::Failed(failure);
    }
    let mut outdated = None;
    if newer(&hello.app_version, smetanad::VERSION) {
        client.close();
        let reason = format!(
            "the server on this machine is {}, newer than this app's {}; update the app",
            hello.app_version,
            smetanad::VERSION
        );
        return Attempt::Failed(Failure::Incompatible(reason, Some(info)));
    }
    if hello.app_version != smetanad::VERSION {
        let may_restart = restarted_from.as_deref() != Some(hello.app_version.as_str());
        match gate(&client, &hello.app_version, may_restart).await {
            Gate::Outdated(found) => outdated = Some(found),
            Gate::Restarting => {
                *restarted_from = Some(hello.app_version.clone());
                client.close();
                return Attempt::Restarting;
            }
        }
    }
    // Before the first call can be made: a read of the settings that was answered ahead of this
    // would show the server's defaults while the old file's server half was still on its way.
    if let Some(path) = &config.settings {
        super::migrate::run(&client, path).await;
    }
    if link.generation() != generation {
        client.close();
        return Attempt::Ended;
    }
    link.connected(client.clone(), info, outdated);
    let (why, switched) = hold(link, &client, events, generation, Some(&config.server_dir)).await;
    if !switched {
        link.lost(why);
    }
    client.close();
    Attempt::Ended
}

/// Keeps the connection up for as long as the app runs. Never returns.
pub async fn supervise(link: Arc<Link>, config: Config) {
    let mut backoff = Backoff::default();
    let mut restarted_from: Option<String> = None;
    loop {
        match attempt(&link, &config, &mut restarted_from).await {
            Attempt::Ended => {
                backoff.reset();
                link.pause(Backoff::FIRST).await;
            }
            Attempt::Restarting => {
                backoff.reset();
                link.failed(Phase::Connecting, "restarting the server for this version".to_owned(), None);
                link.pause(Backoff::FIRST * 2).await;
            }
            Attempt::Failed(Failure::Transient(reason)) => {
                log::info!("client: no connection: {reason}");
                link.failed(Phase::Connecting, reason, None);
                link.pause(backoff.next()).await;
            }
            Attempt::Failed(Failure::Unpaired(reason)) => {
                log::warn!("client: unpaired: {reason}");
                link.failed(Phase::Unpaired, reason, None);
                link.pause(SLOW_POLL).await;
            }
            Attempt::Failed(Failure::Incompatible(reason, info)) => {
                log::warn!("client: incompatible: {reason}");
                link.failed(Phase::Incompatible, reason, info);
                link.pause(SLOW_POLL).await;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_handlers_own_error_reaches_the_webview_untouched() {
        let value = json!({ "kind": "bd", "message": "no such issue" });
        assert_eq!(rejection("tracker_update", BridgeError::Handler(value.clone())), value);
    }

    #[test]
    fn every_rejection_of_a_call_that_did_not_reach_the_server_carries_the_words_the_front_end_looks_for() {
        use super::super::state::{MAYBE_SENT, NOT_SENT};
        let never = rejection("x", BridgeError::Internal(format!("{SEND_FAILED}: broken pipe")));
        let maybe = rejection("x", BridgeError::Internal(CLOSED_BEFORE_REPLY.to_owned()));
        assert!(never.as_str().unwrap().contains(NOT_SENT));
        assert!(maybe.as_str().unwrap().contains(MAYBE_SENT));
        // A refusal by the server itself is none of these.
        assert!(!rejection("x", BridgeError::Forbidden).as_str().unwrap().contains(NOT_SENT));
    }

    #[test]
    fn a_request_that_never_left_says_so_and_one_that_may_have_says_that() {
        let never = rejection("run_start", BridgeError::Internal(format!("{SEND_FAILED}: broken pipe")));
        assert!(never.as_str().unwrap().contains("Nothing was changed"), "{never}");
        let maybe = rejection("run_start", BridgeError::Internal(CLOSED_BEFORE_REPLY.to_owned()));
        assert!(maybe.as_str().unwrap().contains("may or may not have been made"), "{maybe}");
    }

    #[test]
    fn a_refusal_by_the_server_names_the_command() {
        assert!(rejection("x_y", BridgeError::Forbidden).as_str().unwrap().contains("`x_y`"));
        assert!(rejection("x_y", BridgeError::UnknownCommand).as_str().unwrap().contains("older than this app"));
    }

    #[test]
    fn only_a_later_server_is_newer() {
        assert!(newer("0.2.0", "0.1.9"));
        assert!(newer("0.10.0", "0.9.0"));
        assert!(!newer("0.1.0", "0.1.0"));
        assert!(!newer("0.0.9", "0.1.0"));
        assert!(!newer("garbage", "0.1.0"));
    }

    #[test]
    fn what_a_close_code_means_for_the_state() {
        assert!(matches!(classify(ClientError::Code(CLOSE_UNKNOWN)), Failure::Unpaired(_)));
        assert!(matches!(classify(ClientError::Code(CLOSE_VERSION)), Failure::Incompatible(..)));
        assert!(matches!(classify(ClientError::Refused("no".into())), Failure::Unpaired(_)));
        assert!(matches!(classify(ClientError::Setup("no file".into())), Failure::Transient(_)));
        assert!(matches!(classify(ClientError::Code(remote_protocol::CLOSE_SLOW)), Failure::Transient(_)));
    }

    #[tokio::test]
    async fn a_call_with_no_connection_is_refused_with_the_reason_once_one_has_been_made() {
        let link = Link::new(Arc::new(|_, _| {}), true);
        link.update(|inner| {
            inner.view.state = Phase::Reconnecting;
            inner.view.epoch = 2;
        });
        let err = link.call("tracker_update", json!({})).await.unwrap_err();
        assert!(err.as_str().unwrap().contains("Nothing was changed"), "{err}");
    }

    #[tokio::test(start_paused = true)]
    async fn a_call_before_the_first_connection_waits_for_it_and_then_gives_up_with_a_reason() {
        let link = Link::new(Arc::new(|_, _| {}), true);
        let call = {
            let link = link.clone();
            tokio::spawn(async move { link.call("tracker_snapshot", json!({})).await })
        };
        tokio::time::sleep(FIRST_CONNECTION_WAIT + Duration::from_secs(1)).await;
        let err = call.await.unwrap().unwrap_err();
        assert!(err.as_str().unwrap().contains("Not connected to the server yet"), "{err}");
    }

    #[tokio::test]
    async fn a_write_before_the_first_connection_is_refused_at_once_and_not_held() {
        let link = Link::new(Arc::new(|_, _| {}), true);
        let err = link.call("terminal_create", json!({})).await.unwrap_err();
        assert!(err.as_str().unwrap().contains("Not connected to the server yet"), "{err}");
    }

    #[tokio::test]
    async fn every_change_of_state_is_told_to_the_webview_whole() {
        let seen: Arc<Mutex<Vec<(String, Value)>>> = Arc::default();
        let recorder = seen.clone();
        let link = Link::new(Arc::new(move |name, payload| recorder.lock().unwrap().push((name.to_owned(), payload))), true);
        link.failed(Phase::Connecting, "starting".into(), None);
        link.lost("gone");
        let seen = seen.lock().unwrap();
        assert_eq!(seen.len(), 2);
        assert!(seen.iter().all(|(name, _)| name == STATE_EVENT));
        assert_eq!(seen[1].1["state"], "reconnecting");
        assert_eq!(seen[1].1["reason"], "gone");
    }

    // The link against a real server, in this process: the same `Server` the binary runs, reached
    // over the loopback endpoint through the same local credential.

    use std::path::PathBuf;
    use smetanad::server::{Options, Server};

    fn bd() -> PathBuf {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("binaries");
        std::fs::read_dir(&dir)
            .ok()
            .and_then(|entries| {
                entries
                    .flatten()
                    .map(|entry| entry.path())
                    .find(|path| path.file_name().is_some_and(|name| name.to_string_lossy().starts_with("bd-")))
            })
            .unwrap_or_else(|| panic!("no bd sidecar in {}: run `npm run fetch-bd`", dir.display()))
    }

    /// One runtime for the whole process: the workers spawn onto the handle the first server installs.
    fn block_on<F: std::future::Future>(future: F) -> F::Output {
        static RUNTIME: std::sync::OnceLock<tokio::runtime::Runtime> = std::sync::OnceLock::new();
        RUNTIME
            .get_or_init(|| tokio::runtime::Builder::new_multi_thread().enable_all().build().expect("a runtime"))
            .block_on(future)
    }

    async fn start_server(dir: &std::path::Path) -> Server {
        let mut options = Options::new(dir.join("server"));
        options.bd = Some(bd());
        Server::start(options).await.expect("the server starts")
    }

    async fn until(what: &str, mut done: impl FnMut() -> bool) {
        for _ in 0..400 {
            if done() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        panic!("timed out waiting for {what}");
    }

    #[test]
    fn the_link_connects_forwards_reemits_loses_the_server_refuses_and_comes_back() {
        block_on(async {
            let dir = tempfile::tempdir().unwrap();
            let server = start_server(dir.path()).await;

            let seen: Arc<Mutex<Vec<(String, Value)>>> = Arc::default();
            let recorder = seen.clone();
            let link = Link::new(Arc::new(move |name, payload| recorder.lock().unwrap().push((name.to_owned(), payload))), true);
            let config = Config {
                server_dir: dir.path().join("server"),
                identity_dir: dir.path().to_owned(),
                inherit_from: None,
                exe: None,
                bd: None,
                resource_dir: None,
                log: dir.path().join("smetanad.log"),
                settings: None,
            };
            let supervisor = tokio::spawn(supervise(link.clone(), config));

            // A call made before the connection exists waits for it rather than failing.
            let early = link.call("terminal_list", json!({ "project": "/tmp/none" })).await;
            assert_eq!(early, Ok(json!([])), "forwarded, and answered by the server");
            let view = link.view();
            assert_eq!((view.state, view.epoch), (Phase::Connected, 1));
            assert_eq!(view.server.as_ref().unwrap().version, smetanad::VERSION);
            assert!(view.outdated.is_none());

            // The server's own failure arrives in its own form.
            let refused = link.call("no_such_command", json!({})).await.unwrap_err();
            assert!(refused.as_str().unwrap().contains("no_such_command"), "{refused}");

            // An event on the server is the webview's event under the same name.
            server.remote.hub.publish("run:state", &json!({ "marker": 41 }));
            until("the event to be re-emitted", || {
                seen.lock().unwrap().iter().any(|(name, payload)| name == "run:state" && payload["marker"] == 41)
            })
            .await;

            // The server goes: the state says so, and a call is refused with the reason and forgotten.
            server.shutdown().await;
            until("the loss to be seen", || link.view().state != Phase::Connected).await;
            let view = link.view();
            assert_eq!(view.state, Phase::Reconnecting);
            let refused = link.call("tracker_update", json!({ "id": "x", "patch": {} })).await.unwrap_err();
            assert!(refused.as_str().unwrap().contains("Nothing was changed"), "{refused}");

            // It comes back, and the link reconnects by itself, with a new epoch.
            let server = start_server(dir.path()).await;
            until("the link to reconnect", || link.view().state == Phase::Connected).await;
            assert_eq!(link.view().epoch, 2, "a new connection is a new epoch, which is what resyncs the stores");
            assert_eq!(link.call("terminal_list", json!({ "project": "/tmp/none" })).await, Ok(json!([])));
            // The refused write was not kept and did not run when the connection came back.
            let states: Vec<String> = seen
                .lock()
                .unwrap()
                .iter()
                .filter(|(name, _)| name == STATE_EVENT)
                .map(|(_, payload)| payload["state"].as_str().unwrap().to_owned())
                .collect();
            assert!(states.contains(&"connected".to_owned()) && states.contains(&"reconnecting".to_owned()), "{states:?}");

            supervisor.abort();
            server.shutdown().await;
        });
    }

    #[test]
    fn choosing_a_remote_server_leaves_the_local_one_running_and_unrestarted_and_going_back_reconnects() {
        block_on(async {
            let dir = tempfile::tempdir().unwrap();
            let server = start_server(dir.path()).await;
            let server_dir = dir.path().join("server");
            let link = Link::new(Arc::new(|_, _| {}), true);
            let config = Config {
                server_dir: server_dir.clone(),
                identity_dir: dir.path().to_owned(),
                inherit_from: None,
                // No executable: a start of the local server would fail with "not in this build",
                // which is how this test sees that none was attempted.
                exe: None,
                bd: None,
                resource_dir: None,
                log: dir.path().join("smetanad.log"),
                settings: None,
            };
            let supervisor = tokio::spawn(supervise(link.clone(), config));
            link.call("terminal_list", json!({ "project": "/tmp/none" })).await.unwrap();
            assert!(link.view().local);

            // To a server that cannot be dialled (a damaged record): the old connection is let go of,
            // the view says it is another server, and nothing about the local one changed.
            link.set_target(Target::Remote { node: "garbage".into(), relay: "https://relay.example.com".into() });
            assert!(!link.view().local);
            assert_eq!(link.view().target, "garbage", "the view says which server, so the front end can tell it changed");
            until("the connection to the old server to be dropped", || link.view().state != Phase::Connected).await;
            assert!(smetanad::server::is_owned(&server_dir), "the local server was not touched");
            let refused = link.call("tracker_update", json!({ "id": "x", "patch": {} })).await.unwrap_err();
            assert!(refused.as_str().unwrap().contains("Nothing was changed"), "{refused}");

            // While the choice is remote the local server is not started again, even when it is gone.
            server.shutdown().await;
            tokio::time::sleep(Duration::from_secs(2)).await;
            let reason = link.view().reason.unwrap_or_default();
            assert!(!reason.contains("not in this build"), "a start of the local server was attempted: {reason}");
            assert!(reason.contains("key"), "the failure is about the remote record: {reason}");

            // Back to the local one: it is found again (here, started by the test) and the epoch moves.
            let server = start_server(dir.path()).await;
            link.set_target(Target::Local);
            assert!(link.view().local);
            until("the link to reconnect to the local server", || link.view().state == Phase::Connected).await;
            assert_eq!(link.view().epoch, 2, "a new connection is a new epoch, which is what resyncs the stores");

            // Setting the target it already has changes nothing.
            let epoch = link.view().epoch;
            link.set_target(Target::Local);
            assert_eq!((link.view().state, link.view().epoch), (Phase::Connected, epoch));

            supervisor.abort();
            server.shutdown().await;
        });
    }

    #[test]
    fn an_old_settings_file_is_migrated_before_the_first_call_and_a_patch_is_broadcast() {
        use smetana_core::settings::{file, split};
        block_on(async {
            let dir = tempfile::tempdir().unwrap();
            let server = start_server(dir.path()).await;
            let device = dir.path().join("device").join("settings.json");
            std::fs::create_dir_all(device.parent().unwrap()).unwrap();
            std::fs::write(
                &device,
                r#"{"version":1,"appearance":{"theme":"light"},"agent":"codex","agentPrompt":"be brief",
                    "openProjects":["/work/a"],"lastProject":"/work/a"}"#,
            )
            .unwrap();

            let seen: Arc<Mutex<Vec<(String, Value)>>> = Arc::default();
            let recorder = seen.clone();
            let link = Link::new(Arc::new(move |name, payload| recorder.lock().unwrap().push((name.to_owned(), payload))), true);
            let config = Config {
                server_dir: dir.path().join("server"),
                identity_dir: dir.path().to_owned(),
                inherit_from: None,
                exe: None,
                bd: None,
                resource_dir: None,
                log: dir.path().join("smetanad.log"),
                settings: Some(device.clone()),
            };
            let supervisor = tokio::spawn(supervise(link.clone(), config));

            // The first read already sees the migrated server half.
            let loaded = link.call("settings_load", json!({ "project": null })).await.unwrap();
            assert_eq!(loaded["agent"], "codex");
            assert_eq!(loaded["agentPrompt"], "be brief");
            assert_eq!(loaded["openProjects"], json!(["/work/a"]));
            // The device keeps its own half and has given the other up.
            let kept = file::load(&device).0;
            assert_eq!(kept.appearance.theme, "dark", "the look went to the server inside the adopted half");
            assert_eq!(loaded["appearance"]["theme"], "light");
            assert!(split::legacy_server_patch(&kept).is_none());
            assert!(device.with_file_name("settings.pre-split.json").exists());

            // A patch from this client is announced to every client, this one included.
            link.call("settings_patch", json!({ "patch": { "model": "" , "agentLanguage": "ru" } })).await.unwrap();
            until("the broadcast", || {
                seen.lock().unwrap().iter().any(|(name, p)| name == "settings:changed" && p["agentLanguage"] == "ru" && p["agent"] == "codex")
            })
            .await;
            // And the device half is refused on the server's road.
            link.call("settings_patch", json!({ "patch": { "editor": { "wordWrap": true } } })).await.unwrap();
            let after = link.call("settings_load", json!({ "project": null })).await.unwrap();
            assert_eq!(after["editor"]["wordWrap"], false, "the server keeps no copy of the device's fields");

            supervisor.abort();
            server.shutdown().await;
        });
    }

    #[test]
    fn a_step_5_install_has_its_device_half_seeded_from_the_servers_file_before_the_first_call() {
        use smetana_core::settings::file;
        block_on(async {
            let dir = tempfile::tempdir().unwrap();
            // What step 5 left: the whole document on the server, only dialogs on the device.
            let server_dir = dir.path().join("server");
            std::fs::create_dir_all(&server_dir).unwrap();
            std::fs::write(
                server_dir.join("settings.json"),
                r#"{"version":1,"appearance":{"theme":"light","density":"compact","uiFontSize":15},"agent":"codex",
                    "layout":{"leftWidth":300},"openProjects":["/work/a"],"lastProject":"/work/a",
                    "projects":{"/work/a":{"selectedTask":"a-1","sideTab":"git"}}}"#,
            )
            .unwrap();
            let device = dir.path().join("device").join("settings.json");
            std::fs::create_dir_all(device.parent().unwrap()).unwrap();
            std::fs::write(&device, r#"{"version":1,"dialogs":{"run":{"width":600,"height":500}}}"#).unwrap();
            let server = start_server(dir.path()).await;

            let link = Link::new(Arc::new(|_, _| {}), true);
            let config = Config {
                server_dir: server_dir.clone(),
                identity_dir: dir.path().to_owned(),
                inherit_from: None,
                exe: None,
                bd: None,
                resource_dir: None,
                log: dir.path().join("smetanad.log"),
                settings: Some(device.clone()),
            };
            let supervisor = tokio::spawn(supervise(link.clone(), config));
            let loaded = link.call("settings_load", json!({ "project": "/work/a" })).await.unwrap();
            assert_eq!(loaded["agent"], "codex", "the server keeps its half");

            let seeded = file::load(&device).0;
            assert!(seeded.device_seeded);
            assert_eq!(seeded.appearance.ui_font_size, 15);
            assert_eq!(loaded["appearance"]["theme"], "light", "the theme stays the server's");
            assert_eq!(seeded.layout.left_width, 300);
            assert_eq!(seeded.projects["/work/a"].selected_task.as_deref(), Some("a-1"));
            assert_eq!(seeded.projects["/work/a"].side_tab, "git");
            assert!(seeded.dialogs.contains_key("run"), "this machine's own dialog sizes are kept");
            assert_eq!(seeded.agent, "claude", "the server's half is not copied to the device");

            supervisor.abort();
            server.shutdown().await;
        });
    }
}
