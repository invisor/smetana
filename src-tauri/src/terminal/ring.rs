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
        let mut out = Vec::with_capacity(self.buf.len() + 4);
        if self.dropped {
            out.extend_from_slice(b"\x1b[0m");
        }
        out.extend(self.buf.iter().copied());
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn пока_влезает_отдаёт_всё_как_есть() {
        let mut ring = Ring::new(64);
        ring.push(b"hello\nworld\n");
        assert_eq!(ring.snapshot(), b"hello\nworld\n");
    }

    #[test]
    fn переполнение_режет_по_переводу_строки() {
        let mut ring = Ring::new(16);
        // 20 bytes against a 16 cap: the extra four go, and the trim carries
        // the start forward to a newline — a whole line is lost, not half a
        // line.
        ring.push(b"aaaa\nbbbb\ncccc\ndddd\n");
        let text = String::from_utf8(ring.snapshot()).unwrap();
        // The attribute reset comes first — an open colour sequence stayed
        // in the dropped part, and without the reset it would keep colouring
        // everything after it.
        assert!(text.starts_with("\u{1b}[0m"), "нет сброса атрибутов: {text:?}");
        assert_eq!(text.trim_start_matches("\u{1b}[0m"), "bbbb\ncccc\ndddd\n");
    }

    #[test]
    fn обрезка_не_рвёт_escape_последовательность() {
        let mut ring = Ring::new(12);
        ring.push(b"\x1b[31mred text here\nplain\n");
        let text = String::from_utf8(ring.snapshot()).unwrap();
        let body = text.trim_start_matches("\u{1b}[0m");
        assert!(!body.contains("[31"), "хвост escape-последовательности уехал в снимок: {body:?}");
    }

    #[test]
    fn переполнение_без_перевода_строки_режет_по_возврату_каретки() {
        let mut ring = Ring::new(16);
        // No newline, but there is a carriage return: trim against that.
        ring.push(b"aaaa\rbbbb\rcccc\rdddd\r");
        let text = String::from_utf8(ring.snapshot()).unwrap();
        // The attribute reset comes first.
        assert!(text.starts_with("\u{1b}[0m"), "нет сброса атрибутов: {text:?}");
        // After the reset, the snapshot should start at the second line (after the first \r).
        assert_eq!(text.trim_start_matches("\u{1b}[0m"), "bbbb\rcccc\rdddd\r");
    }

    #[test]
    fn переполнение_без_границ_сохраняет_буфер() {
        let mut ring = Ring::new(10);
        // Fill exactly to the cap.
        ring.push(b"aaaaaaaaaa");
        assert_eq!(ring.snapshot().len(), 10, "буфер должен быть полным");
        // Push one more byte — overflow. The excess 1 byte is drained, and
        // the remaining 10 bytes have neither \n nor \r. The buffer must not
        // be cleared.
        ring.push(b"b");
        let text = String::from_utf8(ring.snapshot()).unwrap();
        // Check that the buffer is non-empty and includes the attribute reset.
        assert!(text.starts_with("\u{1b}[0m"), "нет сброса атрибутов: {text:?}");
        let body = text.trim_start_matches("\u{1b}[0m");
        // The body should be exactly 10 bytes — the capacity, not less.
        assert_eq!(body.len(), 10, "буфер должен сохранить 10 байт, а сохранил: {body:?}");
        // The point of the test is that the buffer is not cleared and not empty.
        assert!(!body.is_empty(), "буфер не должен быть пуст после переполнения без границ");
    }
}
