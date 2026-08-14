//! Turning what a batch prints into what its pane shows.
//!
//! A batch runs its harness in a non-interactive form (`agents::is_batch`), and
//! Claude Code's only streaming form is JSONL — one event per line, hundreds of
//! characters of `{"type":"assistant",…}`, which is not something anybody
//! reads. This is the machinery that cuts that stream into lines and hands each
//! to the profile's own rendering; what a line *means* is `agents/claude.rs`'s
//! knowledge and deliberately not this file's.
//!
//! It sits in `service::absorb`, before the ring, the screen and the pending
//! queue, which is what keeps the subsystem's one invariant: a single stream,
//! three emulations of the same bytes, agreeing by construction.

/// The ceiling on a line that has not ended yet. A tool result carrying a large
/// file is one enormous line, and this runs for the length of a night.
pub const MAX_LINE: usize = 1 << 20;

pub struct Transcript {
    render: fn(&str) -> Vec<String>,
    /// Bytes rather than a `String`, so that a multi-byte character split
    /// across two PTY reads is decoded once, whole, when its line ends —
    /// decoding each chunk as it arrives would put a replacement character in
    /// the middle of every word unlucky enough to straddle a read.
    buf: Vec<u8>,
    /// A line was dropped for length: everything up to the next newline is the
    /// rest of it, and rendering that would be rendering a fragment.
    dropped: bool,
}

impl Transcript {
    pub fn new(render: fn(&str) -> Vec<String>) -> Self {
        Self { render, buf: Vec::new(), dropped: false }
    }

    /// The bytes a person should see, for the bytes the child wrote.
    pub fn feed(&mut self, bytes: &[u8]) -> Vec<u8> {
        self.buf.extend_from_slice(bytes);
        let mut out = String::new();
        while let Some(end) = self.buf.iter().position(|b| *b == b'\n') {
            let line: Vec<u8> = self.buf.drain(..=end).collect();
            if self.dropped {
                self.dropped = false;
                continue;
            }
            let line = String::from_utf8_lossy(&line);
            for rendered in (self.render)(line.trim_end()) {
                out.push_str(&rendered);
                out.push_str("\r\n");
            }
        }
        if self.buf.len() > MAX_LINE {
            self.buf.clear();
            self.dropped = true;
            out.push_str("-- a line too long to show was dropped\r\n");
        }
        out.into_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A rendering rule with nothing of any harness in it: the line's own text,
    /// unless it is the word "skip".
    fn echo(line: &str) -> Vec<String> {
        if line == "skip" { Vec::new() } else { vec![line.to_string()] }
    }

    fn text(bytes: Vec<u8>) -> String {
        String::from_utf8(bytes).unwrap()
    }

    #[test]
    fn every_line_it_emits_ends_the_way_a_terminal_needs() {
        // These bytes never passed through a tty's output processing, so a bare
        // newline would leave xterm.js stepping the text diagonally down the
        // pane.
        let mut t = Transcript::new(echo);
        assert_eq!(text(t.feed(b"one\ntwo\n")), "one\r\ntwo\r\n");
    }

    #[test]
    fn a_line_split_across_chunks_is_rendered_once_and_only_when_it_ends() {
        // A PTY read boundary falls wherever it falls, and a stream-json event
        // is one line: half an event is not renderable and must not be dropped
        // either.
        let mut t = Transcript::new(echo);
        assert_eq!(text(t.feed(b"he")), "");
        assert_eq!(text(t.feed(b"llo")), "");
        assert_eq!(text(t.feed(b" there\n")), "hello there\r\n");
    }

    #[test]
    fn a_multibyte_character_split_across_chunks_survives() {
        // Decoding per chunk would turn the two halves of a multi-byte
        // character into two replacement characters; decoding per line cannot.
        let mut t = Transcript::new(echo);
        let word = "———".as_bytes();
        assert_eq!(text(t.feed(&word[..5])), "");
        assert_eq!(text(t.feed(&word[5..])), "");
        assert_eq!(text(t.feed(b"\n")), "———\r\n");
    }

    #[test]
    fn a_line_worth_nothing_produces_nothing_at_all() {
        let mut t = Transcript::new(echo);
        assert_eq!(text(t.feed(b"skip\nkept\n")), "kept\r\n");
    }

    #[test]
    fn a_line_that_never_ends_is_dropped_rather_than_kept_for_ever() {
        // A tool result carrying a large file is one enormous line. Growing the
        // buffer without bound over a night's run is the failure this ceiling
        // exists for; what it costs is one event, said out loud.
        let mut t = Transcript::new(echo);
        let flood = vec![b'x'; MAX_LINE + 1];
        let out = text(t.feed(&flood));
        assert!(out.contains("too long"), "{out}");
        // And it resyncs: the remains of the dropped line are not rendered, and
        // the next whole line is.
        assert_eq!(text(t.feed(b"xxx\nnext\n")), "next\r\n");
    }
}
