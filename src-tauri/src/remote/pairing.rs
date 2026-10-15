//! One-time pairing secrets: issued on the desktop, shown as a QR code, spent by
//! the first phone that presents them.
//!
//! A secret is good for two minutes and for one use. "Used" means *presented*,
//! not *confirmed*: the secret is burned the moment a request carries it, so a
//! rejected or timed-out request leaves nothing behind that could be tried
//! again. Whoever photographs the QR still has to get past the confirmation on
//! the desktop, and gets exactly one attempt at it.
//!
//! The clock is a function handed in, so a test can move it. Nothing here
//! touches the network, the disk or Tauri.

use std::sync::{Arc, Mutex};

use remote_protocol::{PairingTicket, PublicKey, SECRET_LEN};
use tokio::sync::oneshot;

/// How long a freshly issued secret stays good, in seconds.
pub const SECRET_TTL_SECS: u64 = 120;

/// Seconds since the Unix epoch, as the ticket's `expires_at` counts them.
pub type Clock = Arc<dyn Fn() -> u64 + Send + Sync>;

pub fn system_clock() -> Clock {
    Arc::new(|| {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    })
}

struct Issued {
    secret: [u8; SECRET_LEN],
    expires_at: u64,
}

#[derive(Default)]
struct State {
    issued: Option<Issued>,
    /// The person's answer to the request that is on screen, if one is.
    waiting: Option<(u64, oneshot::Sender<bool>)>,
    /// The id the next wait gets.
    next_wait: u64,
}

pub struct Pairing {
    state: Mutex<State>,
    clock: Clock,
}

/// A request that has been entered and is waiting for the person.
pub struct Waiting {
    pub id: u64,
    answer: oneshot::Receiver<bool>,
}

impl Waiting {
    /// The person's answer, or `false` once `secs` have gone by or the request is
    /// withdrawn. Silence is a refusal.
    pub async fn answer(self, secs: u64) -> bool {
        matches!(
            tokio::time::timeout(std::time::Duration::from_secs(secs), self.answer).await,
            Ok(Ok(true))
        )
    }
}

impl Pairing {
    pub fn new(clock: Clock) -> Self {
        Self { state: Mutex::new(State::default()), clock }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub fn now(&self) -> u64 {
        (self.clock)()
    }

    /// Issues a ticket, replacing any earlier one: there is one QR on screen at
    /// a time, and the older code stops working the moment a newer one appears.
    pub fn begin(&self, desktop: PublicKey, relay: &str) -> PairingTicket {
        let secret: [u8; SECRET_LEN] = rand::random();
        let expires_at = self.now() + SECRET_TTL_SECS;
        self.lock().issued = Some(Issued { secret, expires_at });
        PairingTicket { desktop, relay: relay.to_owned(), secret, expires_at }
    }

    /// Withdraws the ticket and refuses a request that is waiting on the person.
    pub fn cancel(&self) {
        let mut state = self.lock();
        state.issued = None;
        if let Some((_, waiting)) = state.waiting.take() {
            let _ = waiting.send(false);
        }
    }

    /// Whether a ticket that has not yet expired is outstanding. Without one the
    /// desktop has no reason to read a single byte from a key it does not know.
    pub fn is_open(&self) -> bool {
        let now = self.now();
        self.lock().issued.as_ref().is_some_and(|issued| now < issued.expires_at)
    }

    /// Checks a presented secret and burns it, whatever the outcome of what
    /// follows. On success, the seconds left of its term.
    pub fn spend(&self, presented: &[u8; SECRET_LEN]) -> Result<u64, &'static str> {
        let now = self.now();
        let mut state = self.lock();
        let Some(issued) = state.issued.as_ref() else {
            return Err("this pairing code is not valid");
        };
        if !same(&issued.secret, presented) {
            // A wrong secret does not burn the right one: anybody who can reach
            // the endpoint could otherwise cancel a pairing by guessing.
            return Err("this pairing code is not valid");
        }
        let expires_at = issued.expires_at;
        state.issued = None;
        if now >= expires_at {
            return Err("this pairing code has expired");
        }
        Ok(expires_at - now)
    }

    /// Puts a request in front of the person. One at a time.
    pub fn wait_for_decision(&self) -> Result<Waiting, &'static str> {
        let mut state = self.lock();
        if state.waiting.is_some() {
            return Err("another phone is waiting to be confirmed");
        }
        let (tx, rx) = oneshot::channel();
        let id = state.next_wait;
        state.next_wait += 1;
        state.waiting = Some((id, tx));
        Ok(Waiting { id, answer: rx })
    }

    /// Forgets the wait with this id; called when it is over, however it ended.
    /// Clears only on a match, so a late finish cannot wipe a newer request that
    /// took the slot in between. `true` when this call is the one that cleared it
    /// (a decision or a cancel has already taken it otherwise).
    pub fn finish_waiting(&self, id: u64) -> bool {
        let mut state = self.lock();
        if state.waiting.as_ref().is_some_and(|(held, _)| *held == id) {
            state.waiting = None;
            true
        } else {
            false
        }
    }

    /// The person's answer. `false` when there was nothing to answer.
    pub fn decide(&self, accept: bool) -> bool {
        match self.lock().waiting.take() {
            Some((_, waiting)) => {
                let _ = waiting.send(accept);
                true
            }
            None => false,
        }
    }
}

/// Compared without stopping at the first difference, so the time taken says
/// nothing about how much of a guess was right.
fn same(a: &[u8; SECRET_LEN], b: &[u8; SECRET_LEN]) -> bool {
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    fn pairing() -> (Pairing, Arc<AtomicU64>) {
        let now = Arc::new(AtomicU64::new(1_000));
        let clock = now.clone();
        (Pairing::new(Arc::new(move || clock.load(Ordering::SeqCst))), now)
    }

    fn key() -> PublicKey {
        iroh::SecretKey::from_bytes(&[1; 32]).public()
    }

    #[test]
    fn a_secret_is_accepted_once() {
        let (pairing, _) = pairing();
        let ticket = pairing.begin(key(), "https://relay.example");
        assert_eq!(ticket.expires_at, 1_000 + SECRET_TTL_SECS);
        assert!(pairing.is_open());
        assert_eq!(pairing.spend(&ticket.secret), Ok(SECRET_TTL_SECS));
        assert!(pairing.spend(&ticket.secret).is_err(), "the second use is refused");
        assert!(!pairing.is_open());
    }

    #[test]
    fn an_expired_secret_is_refused() {
        let (pairing, now) = pairing();
        let ticket = pairing.begin(key(), "https://relay.example");
        now.store(1_000 + SECRET_TTL_SECS, Ordering::SeqCst);
        assert!(!pairing.is_open());
        assert_eq!(pairing.spend(&ticket.secret), Err("this pairing code has expired"));
    }

    #[test]
    fn the_last_second_still_counts() {
        let (pairing, now) = pairing();
        let ticket = pairing.begin(key(), "https://relay.example");
        now.store(1_000 + SECRET_TTL_SECS - 1, Ordering::SeqCst);
        assert_eq!(pairing.spend(&ticket.secret), Ok(1));
    }

    #[test]
    fn a_wrong_secret_neither_passes_nor_burns_the_right_one() {
        let (pairing, _) = pairing();
        let ticket = pairing.begin(key(), "https://relay.example");
        assert!(pairing.spend(&[0; SECRET_LEN]).is_err());
        assert!(pairing.spend(&ticket.secret).is_ok());
    }

    #[test]
    fn a_newer_ticket_replaces_the_older_one() {
        let (pairing, _) = pairing();
        let first = pairing.begin(key(), "https://relay.example");
        let second = pairing.begin(key(), "https://relay.example");
        assert!(pairing.spend(&first.secret).is_err());
        assert!(pairing.spend(&second.secret).is_ok());
    }

    #[test]
    fn a_cancelled_ticket_is_gone() {
        let (pairing, _) = pairing();
        let ticket = pairing.begin(key(), "https://relay.example");
        pairing.cancel();
        assert!(!pairing.is_open());
        assert!(pairing.spend(&ticket.secret).is_err());
    }

    #[tokio::test]
    async fn the_person_answers_a_waiting_request() {
        let (pairing, _) = pairing();
        let waiting = pairing.wait_for_decision().unwrap();
        assert!(pairing.wait_for_decision().is_err(), "one at a time");
        assert!(pairing.decide(true));
        assert!(waiting.answer(5).await);
        assert!(!pairing.decide(true), "nothing left to answer");
    }

    #[tokio::test]
    async fn a_late_finish_does_not_clear_a_newer_request() {
        let (pairing, _) = pairing();
        let first = pairing.wait_for_decision().unwrap();
        assert!(pairing.decide(false));
        let second = pairing.wait_for_decision().unwrap();
        assert!(!pairing.finish_waiting(first.id), "the old request no longer owns the slot");
        assert!(pairing.wait_for_decision().is_err(), "the newer one is still waiting");
        assert!(pairing.finish_waiting(second.id));
        assert!(pairing.wait_for_decision().is_ok());
    }

    #[tokio::test]
    async fn cancelling_refuses_a_waiting_request() {
        let (pairing, _) = pairing();
        let waiting = pairing.wait_for_decision().unwrap();
        pairing.cancel();
        assert!(!waiting.answer(5).await);
    }
}
