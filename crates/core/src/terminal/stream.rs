//! What a terminal session keeps so that several clients can follow it over connections:
//! the chunks it has said, numbered, so a client that dropped can ask for what it missed; and
//! who the PTY's size belongs to.
//!
//! Nothing here touches a process or a channel, which is what makes it testable on its own.
//! The worker (`service.rs`) owns one of each per session and is the only caller.

use std::collections::{HashMap, VecDeque};

/// A connection that follows sessions. `LOCAL` is the window of the process the worker runs in,
/// which is not on a connection at all.
pub type ClientId = u64;
pub const LOCAL: ClientId = 0;

/// The newest chunks a session has committed, each under its sequence number, up to a byte
/// budget. The sequence rises by one per chunk and every chunk goes in, so what is held is
/// always a contiguous run ending at the session's current number.
pub struct History {
    chunks: VecDeque<(u64, Vec<u8>)>,
    bytes: usize,
    cap: usize,
}

impl History {
    pub fn new(cap: usize) -> Self {
        Self { chunks: VecDeque::new(), bytes: 0, cap }
    }

    /// Appends the chunk numbered `seq`, dropping the oldest while over the budget. A chunk that
    /// alone exceeds the budget empties the history rather than being held: a client asking for
    /// what came before it would get a partial run, and the ring is the honest answer there.
    pub fn push(&mut self, seq: u64, bytes: Vec<u8>) {
        self.bytes += bytes.len();
        self.chunks.push_back((seq, bytes));
        while self.bytes > self.cap {
            match self.chunks.pop_front() {
                Some((_, dropped)) => self.bytes -= dropped.len(),
                None => break,
            }
        }
    }

    /// Everything after chunk `after`, as one run of bytes, when the history still holds all of
    /// it; `None` when it does not (the front has been trimmed past `after + 1`) or when `after`
    /// is ahead of `newest`, a number from some other life of the session. `newest` is the
    /// session's current number: `after == newest` is an empty run, not a miss.
    pub fn after(&self, after: u64, newest: u64) -> Option<Vec<u8>> {
        if after > newest {
            return None;
        }
        if after == newest {
            return Some(Vec::new());
        }
        let (front, _) = self.chunks.front()?;
        if *front > after + 1 {
            return None;
        }
        let mut out = Vec::new();
        for (seq, bytes) in &self.chunks {
            if *seq > after {
                out.extend_from_slice(bytes);
            }
        }
        Some(out)
    }

    #[cfg(test)]
    pub fn len_bytes(&self) -> usize {
        self.bytes
    }
}

/// Who the PTY's size belongs to when several clients view one session: the one that last wrote
/// to it or last resized it, as in tmux. A client that only watches never takes it.
#[derive(Default)]
pub struct Viewers {
    owner: Option<ClientId>,
    sizes: HashMap<ClientId, (u16, u16)>,
}

impl Viewers {
    /// `client` reported its size. It is remembered, the client becomes the owner, and the size
    /// is the one to apply.
    pub fn resized(&mut self, client: ClientId, cols: u16, rows: u16) -> (u16, u16) {
        self.sizes.insert(client, (cols, rows));
        self.owner = Some(client);
        (cols, rows)
    }

    /// `client` wrote. When it was not already the owner it becomes one, and if it has reported a
    /// size, that is the size to apply now; `None` when nothing needs to change.
    pub fn wrote(&mut self, client: ClientId) -> Option<(u16, u16)> {
        if self.owner == Some(client) {
            return None;
        }
        let size = self.sizes.get(&client).copied()?;
        self.owner = Some(client);
        Some(size)
    }

    /// `client` is gone. Its size is forgotten; the PTY keeps whatever size it has until somebody
    /// writes or resizes.
    pub fn forget(&mut self, client: ClientId) {
        self.sizes.remove(&client);
        if self.owner == Some(client) {
            self.owner = None;
        }
    }
}

/// The state of both, per session.
pub struct OutState {
    pub history: History,
    pub viewers: Viewers,
}

impl OutState {
    pub fn new(history_cap: usize) -> Self {
        Self { history: History::new(history_cap), viewers: Viewers::default() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn history(chunks: &[&str]) -> (History, u64) {
        let mut history = History::new(1 << 20);
        for (i, chunk) in chunks.iter().enumerate() {
            history.push(i as u64 + 1, chunk.as_bytes().to_vec());
        }
        (history, chunks.len() as u64)
    }

    #[test]
    fn a_client_gets_exactly_what_follows_its_number() {
        let (history, newest) = history(&["a", "b", "c", "d"]);
        assert_eq!(history.after(0, newest), Some(b"abcd".to_vec()));
        assert_eq!(history.after(2, newest), Some(b"cd".to_vec()));
        assert_eq!(history.after(4, newest), Some(Vec::new()), "caught up is an empty run");
    }

    #[test]
    fn a_number_from_another_life_of_the_session_is_a_miss() {
        let (history, newest) = history(&["a", "b"]);
        assert_eq!(history.after(9, newest), None);
    }

    #[test]
    fn what_has_been_trimmed_is_a_miss_and_not_a_gap() {
        let mut history = History::new(4);
        for seq in 1..=6u64 {
            history.push(seq, b"xy".to_vec());
        }
        // Budget 4 bytes holds the last two chunks, 5 and 6.
        assert!(history.len_bytes() <= 4);
        assert_eq!(history.after(4, 6), Some(b"xyxy".to_vec()), "chunk 5 is the first it needs, and it is held");
        assert_eq!(history.after(3, 6), None, "chunk 4 is gone, so the ring it is");
        assert_eq!(history.after(0, 6), None);
    }

    #[test]
    fn one_chunk_over_the_budget_leaves_nothing_to_resume_from() {
        let mut history = History::new(4);
        history.push(1, b"ab".to_vec());
        history.push(2, b"abcdefgh".to_vec());
        assert_eq!(history.after(1, 2), None);
        assert_eq!(history.len_bytes(), 0);
    }

    #[test]
    fn the_size_follows_whoever_wrote_or_resized_last() {
        let mut viewers = Viewers::default();
        assert_eq!(viewers.resized(1, 100, 30), (100, 30));
        assert_eq!(viewers.resized(2, 80, 24), (80, 24));
        // 1 writes: the PTY goes back to its size.
        assert_eq!(viewers.wrote(1), Some((100, 30)));
        // 1 writes again: nothing changes.
        assert_eq!(viewers.wrote(1), None);
        // 2 writes: its size is applied.
        assert_eq!(viewers.wrote(2), Some((80, 24)));
    }

    #[test]
    fn a_client_that_never_reported_a_size_never_resizes() {
        let mut viewers = Viewers::default();
        viewers.resized(1, 100, 30);
        assert_eq!(viewers.wrote(7), None);
        assert_eq!(viewers.resized(1, 90, 30), (90, 30));
        assert_eq!(viewers.wrote(7), None, "7 has no size, so the owner is still 1");
    }

    #[test]
    fn a_client_that_leaves_gives_up_its_seat() {
        let mut viewers = Viewers::default();
        viewers.resized(1, 100, 30);
        viewers.resized(2, 80, 24);
        viewers.forget(2);
        assert_eq!(viewers.wrote(1), Some((100, 30)));
        viewers.forget(1);
        assert_eq!(viewers.wrote(1), None);
    }
}
