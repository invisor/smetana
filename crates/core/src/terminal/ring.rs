//! The raw-byte ring: what the person saw, and what xterm.js repaints from
//! when it attaches to a session. Screen parsing is not its concern — that's
//! screen.rs, and it reads the same stream independently.

use std::collections::VecDeque;

pub struct Ring {
    buf: VecDeque<u8>,
    cap: usize,
    /// Something has already fallen off the front — the snapshot now starts
    /// mid-session, so any attributes left open in the dropped chunk need
    /// to be reset.
    dropped: bool,
}

impl Ring {
    pub fn new(cap: usize) -> Self {
        Self { buf: VecDeque::new(), cap, dropped: false }
    }

    pub fn push(&mut self, bytes: &[u8]) {
        self.buf.extend(bytes);
        if self.buf.len() <= self.cap {
            return;
        }
        self.dropped = true;
        let excess = self.buf.len() - self.cap;
        self.buf.drain(..excess);
        // Trim further, up to the nearest newline. Without this the snapshot
        // could start mid escape sequence, and the first line after a long
        // session would arrive as garbage — looks like a broken terminal,
        // but only one byte is broken.
        if let Some(nl) = self.buf.iter().position(|b| *b == b'\n') {
            self.buf.drain(..=nl);
        } else if let Some(cr) = self.buf.iter().position(|b| *b == b'\r') {
            // A carriage return is also a safe boundary: 0x0D cannot appear
            // inside a CSI sequence, whose parameter bytes occupy 0x20-0x3F
            // and whose final byte occupies 0x40-0x7E.
            self.buf.drain(..=cr);
        }
        // If there is neither a newline nor a carriage return, leave the
        // buffer as is. dropped=true is already set, so snapshot() will
        // still emit the attribute reset. A few garbage characters from a
        // truncated sequence at the start of the scrollback are an
        // acceptable price for keeping the output, versus clearing the
        // whole buffer and losing the session's context.
    }

    pub fn snapshot(&self) -> Vec<u8> {
        self.snapshot_without_tail(0)
    }

    /// The snapshot as it was `tail` bytes ago: what the ring held before the last `tail` bytes
    /// were pushed. A reader that is told the number of the last chunk *committed* needs the
    /// ring to end there, and the bytes since then (`pending` in the worker) are still on their
    /// way to it as the next chunk. A tail longer than the ring leaves nothing.
    pub fn snapshot_without_tail(&self, tail: usize) -> Vec<u8> {
        let keep = self.buf.len().saturating_sub(tail);
        let mut out = Vec::with_capacity(keep + 4);
        if self.dropped {
            out.extend_from_slice(b"\x1b[0m");
        }
        out.extend(self.buf.iter().take(keep).copied());
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn while_it_fits_everything_comes_back_as_is() {
        let mut ring = Ring::new(64);
        ring.push(b"hello\nworld\n");
        assert_eq!(ring.snapshot(), b"hello\nworld\n");
    }

    #[test]
    fn a_snapshot_without_a_tail_ends_where_the_ring_ended_before_it() {
        let mut ring = Ring::new(64);
        ring.push(b"hello ");
        ring.push(b"world");
        assert_eq!(ring.snapshot_without_tail(5), b"hello ");
        assert_eq!(ring.snapshot_without_tail(0), b"hello world");
        assert!(ring.snapshot_without_tail(99).is_empty());
    }

    #[test]
    fn an_overflow_cuts_at_a_line_break() {
        let mut ring = Ring::new(16);
        // 20 bytes against a 16 cap: the extra four go, and the trim carries
        // the start forward to a newline — a whole line is lost, not half a
        // line.
        ring.push(b"aaaa\nbbbb\ncccc\ndddd\n");
        let text = String::from_utf8(ring.snapshot()).unwrap();
        // The attribute reset comes first — an open colour sequence stayed
        // in the dropped part, and without the reset it would keep colouring
        // everything after it.
        assert!(text.starts_with("\u{1b}[0m"), "no attribute reset: {text:?}");
        assert_eq!(text.trim_start_matches("\u{1b}[0m"), "bbbb\ncccc\ndddd\n");
    }

    #[test]
    fn trimming_does_not_tear_an_escape_sequence() {
        let mut ring = Ring::new(12);
        ring.push(b"\x1b[31mred text here\nplain\n");
        let text = String::from_utf8(ring.snapshot()).unwrap();
        let body = text.trim_start_matches("\u{1b}[0m");
        assert!(!body.contains("[31"), "the tail of an escape sequence made it into the snapshot: {body:?}");
    }

    #[test]
    fn an_overflow_with_no_line_break_cuts_at_a_carriage_return() {
        let mut ring = Ring::new(16);
        // No newline, but there is a carriage return: trim against that.
        ring.push(b"aaaa\rbbbb\rcccc\rdddd\r");
        let text = String::from_utf8(ring.snapshot()).unwrap();
        // The attribute reset comes first.
        assert!(text.starts_with("\u{1b}[0m"), "no attribute reset: {text:?}");
        // After the reset, the snapshot should start at the second line (after the first \r).
        assert_eq!(text.trim_start_matches("\u{1b}[0m"), "bbbb\rcccc\rdddd\r");
    }

    #[test]
    fn an_overflow_with_no_boundaries_keeps_the_buffer() {
        let mut ring = Ring::new(10);
        // Fill exactly to the cap.
        ring.push(b"aaaaaaaaaa");
        assert_eq!(ring.snapshot().len(), 10, "the buffer must be full");
        // Push one more byte — overflow. The excess 1 byte is drained, and
        // the remaining 10 bytes have neither \n nor \r. The buffer must not
        // be cleared.
        ring.push(b"b");
        let text = String::from_utf8(ring.snapshot()).unwrap();
        // Check that the buffer is non-empty and includes the attribute reset.
        assert!(text.starts_with("\u{1b}[0m"), "no attribute reset: {text:?}");
        let body = text.trim_start_matches("\u{1b}[0m");
        // The body should be exactly 10 bytes — the capacity, not less.
        assert_eq!(body.len(), 10, "the buffer had to keep 10 bytes, and kept: {body:?}");
        // The point of the test is that the buffer is not cleared and not empty.
        assert!(!body.is_empty(), "the buffer must not be empty after an overflow with no boundaries");
    }
}
