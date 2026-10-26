//! What the connection to the server is doing, as the webview is told it.
//!
//! One value, [`View`], is both the answer to `connection_state` and the payload of the
//! `connection:state` event, so a window opened halfway through an outage draws the same thing as one
//! that watched it begin. Everything here is plain data and plain rules: the supervisor in `link.rs`
//! moves the state, this file says what each state is called, how long to wait between attempts, and
//! what a call refused in it is told.

use std::time::Duration;

use serde::Serialize;
use serde_json::Value;

/// The event the state travels in. The same string as `STATE_EVENT` in `src/stores/connection.js`.
pub const STATE_EVENT: &str = "connection:state";

/// The words every rejection the shell makes for a call that did not reach the server's handler
/// contains, in one of two forms: the call never left (`NOT_SENT`), or it left and the answer was lost
/// (`MAYBE_SENT`). The front end tells such a rejection from the server's own refusal by finding
/// either in it (`transportFailure` in `src/stores/settingsSplit.js`, which holds the same two
/// strings): change one here and that regex and its test move with it. `every_rejection_...` pins this.
pub const NOT_SENT: &str = "Nothing was changed";
#[cfg_attr(not(test), allow(dead_code))]
pub const MAYBE_SENT: &str = "may or may not have been made";

/// Where the connection is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Phase {
    /// No connection has been made yet in this run of the app: the local server is being found or
    /// started, or the first attempts have failed.
    Connecting,
    Connected,
    /// A connection that was up has been lost and is being made again.
    Reconnecting,
    /// The server speaks another major version of the protocol. Nothing else is attempted.
    Incompatible,
    /// The server does not know this device, and could not be asked to pair it.
    Unpaired,
    /// This build cannot even look for a server (no data directory to keep one in). Nothing retries.
    Failed,
}

/// What the server said about itself in its `HelloReply`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerInfo {
    pub version: String,
    pub protocol_major: u16,
    pub protocol_minor: u16,
}

/// A local server older than the one in the bundle, and what it would take to replace it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Outdated {
    /// The version running.
    pub running: String,
    /// The version in the bundle.
    pub bundled: String,
    /// The live work the restart would end, as the server listed it (`server_restart`'s `work`), or
    /// empty when the restart could not even be asked for.
    pub work: Value,
    /// Whether the server can be asked to restart at all: an older one may not advertise it.
    pub can_restart: bool,
}

/// The whole of it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct View {
    /// Always `client`: the shell has a connection to draw. The other answer, `browser`, is the one
    /// `mockBackend.js` gives where there is no shell, and is never produced here.
    pub mode: &'static str,
    pub state: Phase,
    /// How many connections have been made so far. A new number with `connected` is what makes the
    /// stores read their snapshots again; the same number repeated does not.
    pub epoch: u64,
    /// Attempts since the last connection (or since start).
    pub attempt: u32,
    /// The last thing that went wrong, in words a person can read.
    pub reason: Option<String>,
    /// The server is on this machine and this app started it.
    pub local: bool,
    /// Which server this is: `local`, or the remote server's key. A different value on a new
    /// connection means the projects and settings on screen belong to another server, and the
    /// front end takes the new one's instead of carrying its own over.
    pub target: String,
    pub server: Option<ServerInfo>,
    pub outdated: Option<Outdated>,
}

impl View {
    pub fn new(local: bool) -> View {
        View {
            mode: "client",
            state: Phase::Connecting,
            epoch: 0,
            attempt: 0,
            reason: None,
            local,
            target: if local { "local".to_owned() } else { "remote".to_owned() },
            server: None,
            outdated: None,
        }
    }

    /// Whether the call that finds the connection like this waits for it rather than being refused:
    /// only while the very first connection of this run is being made, because the app has nothing
    /// to draw until it has, and the stores read as it mounts.
    pub fn first_connection_pending(&self) -> bool {
        self.state == Phase::Connecting && self.epoch == 0
    }

    /// What a call is told when it cannot be sent. Every one of these says that nothing was changed:
    /// the call never left, and it is not kept to be sent later.
    pub fn refusal(&self) -> String {
        let why = match &self.reason {
            Some(reason) if !reason.is_empty() => format!(" ({reason})"),
            _ => String::new(),
        };
        match self.state {
            Phase::Connected => "The server is connected.".to_owned(),
            Phase::Connecting => format!(
                "Not connected to the server yet{why}. Nothing was changed and the request was not kept: try again once it is connected."
            ),
            Phase::Reconnecting => format!(
                "The connection to the server was lost and is being made again{why}. Nothing was changed and the request was not kept: try again once it is back."
            ),
            Phase::Incompatible => format!(
                "This app and the server speak different versions of the protocol{why}. Nothing was changed: update whichever is older."
            ),
            Phase::Unpaired => format!(
                "This app is not paired with the server{why}. Nothing was changed."
            ),
            Phase::Failed => format!("This app cannot reach a server{why}. Nothing was changed."),
        }
    }
}

/// How long to wait before the next attempt: half a second, doubling, never more than ten.
#[derive(Debug, Default)]
pub struct Backoff {
    failures: u32,
}

impl Backoff {
    pub const FIRST: Duration = Duration::from_millis(500);
    pub const CEILING: Duration = Duration::from_secs(10);

    pub fn next(&mut self) -> Duration {
        let wait = Self::FIRST.saturating_mul(1u32.checked_shl(self.failures).unwrap_or(u32::MAX));
        self.failures = self.failures.saturating_add(1);
        wait.min(Self::CEILING)
    }

    pub fn reset(&mut self) {
        self.failures = 0;
    }
}

/// How often a state nothing will fix by itself is looked at again: the server may have been updated
/// or restarted by a person in the meantime.
pub const SLOW_POLL: Duration = Duration::from_secs(15);

/// How long a call made while the first connection is still being made waits for it.
pub const FIRST_CONNECTION_WAIT: Duration = Duration::from_secs(45);

#[cfg(test)]
mod tests {
    use super::*;

    /// The front end's `transportFailure` looks for `NOT_SENT` / `MAYBE_SENT` in a rejection; every state's
    /// refusal must carry one.
    #[test]
    fn every_refusal_carries_the_words_the_front_end_tells_a_lost_connection_by() {
        let mut view = View::new(true);
        for state in [Phase::Connecting, Phase::Reconnecting, Phase::Incompatible, Phase::Unpaired, Phase::Failed] {
            view.state = state;
            assert!(view.refusal().contains(NOT_SENT), "{state:?}: {}", view.refusal());
        }
    }

    #[test]
    fn backoff_doubles_and_stops_at_its_ceiling() {
        let mut backoff = Backoff::default();
        let waits: Vec<u64> = (0..8).map(|_| backoff.next().as_millis() as u64).collect();
        assert_eq!(waits, [500, 1000, 2000, 4000, 8000, 10_000, 10_000, 10_000]);
        backoff.reset();
        assert_eq!(backoff.next(), Backoff::FIRST, "a connection made starts the count again");
    }

    #[test]
    fn backoff_does_not_overflow_after_a_very_long_outage() {
        let mut backoff = Backoff::default();
        for _ in 0..200 {
            assert!(backoff.next() <= Backoff::CEILING);
        }
    }

    #[test]
    fn only_the_first_connection_is_waited_for() {
        let mut view = View::new(true);
        assert!(view.first_connection_pending());
        view.state = Phase::Connected;
        view.epoch = 1;
        assert!(!view.first_connection_pending());
        view.state = Phase::Reconnecting;
        assert!(!view.first_connection_pending(), "a lost connection refuses, it does not hold calls");
        view.state = Phase::Connecting;
        assert!(!view.first_connection_pending(), "once connected in this run, never a held call again");
    }

    #[test]
    fn every_refusal_says_nothing_was_changed() {
        let mut view = View::new(true);
        for state in [Phase::Connecting, Phase::Reconnecting, Phase::Incompatible, Phase::Unpaired, Phase::Failed] {
            view.state = state;
            assert!(view.refusal().contains("Nothing was changed"), "{state:?}: {}", view.refusal());
        }
        view.state = Phase::Reconnecting;
        view.reason = Some("the server did not answer".into());
        assert!(view.refusal().contains("(the server did not answer)"));
    }

    #[test]
    fn the_view_is_what_the_front_end_reads() {
        let mut view = View::new(true);
        view.state = Phase::Reconnecting;
        view.epoch = 3;
        view.server = Some(ServerInfo { version: "0.1.0".into(), protocol_major: 1, protocol_minor: 9 });
        let json = serde_json::to_value(&view).unwrap();
        assert_eq!(json["mode"], "client");
        assert_eq!(json["state"], "reconnecting");
        assert_eq!(json["epoch"], 3);
        assert_eq!(json["server"]["protocolMajor"], 1);
        assert!(json["outdated"].is_null());
    }
}
