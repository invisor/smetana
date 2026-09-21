//! What every harness has in common, and the seam that hides what it does not.
//!
//! A driver is one live conversation's codec and nothing else: bytes in, typed
//! events out, and a command line to spawn. The worker owns the process and the
//! journal; nothing about either is knowable from in here, which is what lets a
//! second harness arrive as one more file rather than as a branch in the worker.
//!
//! `LineBuffer` is transport rather than vocabulary: cutting a pipe's bytes into
//! lines is the same job whichever protocol those lines carry, so it sits here
//! beside the seam rather than inside any one codec. It is
//! `terminal::transcript::Transcript`'s buffering half, carried over — the
//! terminal keeps its own copy and goes on using it, because that one renders
//! strings for a pane while this one hands whole lines to a codec, and folding
//! the two together would make one function answer to two callers with
//! different ideas of what a line is worth.

use portable_pty::CommandBuilder;

use std::collections::BTreeMap;
use super::model::{Decision, EventKind};
use crate::agents::Launch;

/// The ceiling on a line that has not ended yet. A tool result carrying a large
/// file is one enormous line, and a session runs for the length of a night:
/// without this the buffer grows for as long as the child declines to write a
/// newline. What it costs is one event, and the rest of that line with it.
pub const MAX_LINE: usize = 1 << 20;

/// What a person can put into a session.
#[derive(Clone, Debug)]
pub enum Input {
    Message { text: String, attachments: Vec<String> },
}

/// One live conversation's codec. The worker owns the process and the journal;
/// a driver owns only the translation, and knows nothing of either.
pub trait Driver: Send {
    /// What to spawn. Built by the profile, so nothing about a command line is
    /// written twice: the worker turns this into a process with `get_argv`,
    /// `get_cwd` and `iter_extra_env_as_str`.
    fn start(&self, launch: &Launch) -> CommandBuilder;

    /// The text this session opens on **over stdin**, when it opens on a brief:
    /// a task to file, an issue to edit, a conflict to resolve. `None` when
    /// nothing is to be written at the start — a bare session, whose only
    /// prompt is a standing instruction and went on the system prompt in
    /// `start`, and a resumed one, which already has somebody's words in it.
    ///
    /// The worker writes this through the same channel a person's message
    /// takes, and journals it as `EventKind::Opening` first. It is the whole
    /// prompt the PTY road would have handed over positionally, byte for byte:
    /// `--input-format stream-json` discards the positional argument, so this
    /// is the one channel left that reaches the model as a *turn* rather than
    /// as a system-prompt clause.
    fn opening(&self, launch: &Launch) -> Option<String>;

    /// Bytes off the child's stdout, as events. Zero events is the commonest
    /// answer and an ordinary one.
    fn feed(&mut self, bytes: &[u8]) -> Vec<EventKind>;

    /// Requests produced while decoding a response. Most line protocols never
    /// need this; JSON-RPC bootstraps its thread after each prior reply.
    fn outgoing(&mut self) -> Vec<Vec<u8>> { Vec::new() }

    /// A protocol that creates its conversation asynchronously confirms that
    /// startup only after its thread exists. `None` keeps the ordinary
    /// line-oriented harness synchronous.
    fn startup(&mut self) -> Option<Result<(), String>> { None }
    fn awaits_startup(&self) -> bool { false }

    /// A person's message, as bytes for the child's stdin.
    fn send(&mut self, input: Input) -> Vec<u8>;

    /// The worker's opening turn. Drivers whose composed prompt already names
    /// attachments can drop their transport list here; app-server Codex keeps
    /// it to emit one localImage per path.
    fn opening_input(&mut self, input: Input) -> Vec<u8> { self.send(input) }

    /// A person's answer to a question. Some harnesses answer over stdin, some
    /// over a channel of their own; `None` means this one needs no bytes here
    /// and the worker should look to the driver's own side channel.
    fn answer(&mut self, id: &str, decision: Decision, answers: Option<BTreeMap<String, String>>) -> Option<Vec<u8>>;

    /// Stop the turn in flight. `None` means this harness has no way to be
    /// asked, and the worker's only recourse is killing the child.
    fn interrupt(&mut self) -> Option<Vec<u8>>;
}

/// Bytes rather than a `String`, so that a multi-byte character split across
/// two reads is decoded once, whole, when its line ends — decoding each chunk
/// as it arrives would put a replacement character in the middle of every word
/// unlucky enough to straddle a read.
pub struct LineBuffer {
    buf: Vec<u8>,
    /// A line was dropped for length: everything up to the next newline is the
    /// rest of it, and handing that on would be handing on a fragment.
    dropped: bool,
}

impl LineBuffer {
    pub fn new() -> Self {
        Self { buf: Vec::new(), dropped: false }
    }

    /// The whole lines the child has finished writing, for the bytes it wrote.
    /// An empty answer is ordinary: a read that lands mid-line has nothing to
    /// give yet.
    pub fn feed(&mut self, bytes: &[u8]) -> Vec<String> {
        self.buf.extend_from_slice(bytes);
        let mut lines = Vec::new();
        while let Some(end) = self.buf.iter().position(|byte| *byte == b'\n') {
            let line: Vec<u8> = self.buf.drain(..=end).collect();
            if self.dropped {
                self.dropped = false;
                continue;
            }
            let line = String::from_utf8_lossy(&line);
            let line = line.trim_end_matches(['\n', '\r']);
            if !line.is_empty() {
                lines.push(line.to_string());
            }
        }
        if self.buf.len() > MAX_LINE {
            self.buf.clear();
            self.dropped = true;
        }
        lines
    }
}

impl Default for LineBuffer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_line_is_only_handed_over_once_it_has_ended() {
        let mut buffer = LineBuffer::new();
        assert!(buffer.feed(b"{\"a\":1}").is_empty(), "no newline yet, nothing to parse");
        assert_eq!(buffer.feed(b"\n"), vec!["{\"a\":1}".to_string()]);
    }

    #[test]
    fn a_character_split_across_two_reads_is_decoded_whole() {
        // Decoding each chunk as it arrives would put a replacement character
        // in the middle of every word unlucky enough to straddle a read.
        let mut buffer = LineBuffer::new();
        // Three em dashes, three bytes apiece, cut at five: the split lands
        // inside the second character rather than between two of them.
        let text = "———".as_bytes();
        let (head, tail) = text.split_at(5);
        assert!(buffer.feed(head).is_empty());
        let mut rest = tail.to_vec();
        rest.push(b'\n');
        assert_eq!(buffer.feed(&rest), vec!["———".to_string()]);
    }

    #[test]
    fn several_lines_in_one_read_all_come_out() {
        let mut buffer = LineBuffer::new();
        assert_eq!(buffer.feed(b"one\ntwo\nthree\n"), vec!["one", "two", "three"]);
    }

    #[test]
    fn a_trailing_carriage_return_is_not_part_of_the_line() {
        let mut buffer = LineBuffer::new();
        assert_eq!(buffer.feed(b"one\r\n"), vec!["one".to_string()]);
    }

    #[test]
    fn a_line_past_the_ceiling_is_dropped_along_with_the_rest_of_itself() {
        // A tool result carrying a large file is one enormous line, and this
        // runs for the length of a night. Handing on the tail of it would be
        // handing on a fragment, so everything up to the next newline goes too.
        let mut buffer = LineBuffer::new();
        let huge = vec![b'x'; MAX_LINE + 1];
        assert!(buffer.feed(&huge).is_empty());
        assert!(buffer.feed(b"still the same line\n").is_empty());
        assert_eq!(buffer.feed(b"a fresh one\n"), vec!["a fresh one".to_string()]);
    }
}
