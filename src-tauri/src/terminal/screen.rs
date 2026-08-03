//! The session's screen: what the app reads to understand what's happening.
//! The agent's raw stream is a mess of cursor moves and redraws, with nothing
//! in it worth searching; the screen is the text a person actually sees.

pub struct Screen {
    parser: vt100::Parser,
}

impl Screen {
    pub fn new(cols: u16, rows: u16) -> Self {
        // No scrollback needed here: the ring holds that for a person, and
        // detection only needs the visible screen.
        Self { parser: vt100::Parser::new(rows, cols, 0) }
    }

    /// Feed a chunk of output. Returns `true` if the chunk contained a bell:
    /// the parser swallows it, but layer A — the detection logic that reads
    /// this screen to decide session state, landing in a later task — needs
    /// to know.
    pub fn feed(&mut self, bytes: &[u8]) -> bool {
        self.parser.process(bytes);
        bytes.contains(&0x07)
    }

    pub fn resize(&mut self, cols: u16, rows: u16) {
        // set_size lives on vt100::Screen, not on Parser.
        self.parser.screen_mut().set_size(rows, cols);
    }

    pub fn lines(&self) -> Vec<String> {
        let screen = self.parser.screen();
        let (_, cols) = screen.size();
        // rows(start_col, width) — despite the name, this is not a row index
        // but a column window: the iterator already walks every visible row
        // in full, yielding for each one its text within columns
        // [start_col, start_col+width).
        screen.rows(0, cols).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn показывает_то_что_напечатали() {
        let mut screen = Screen::new(20, 4);
        screen.feed(b"hello\r\nworld\r\n");
        let lines = screen.lines();
        assert_eq!(lines[0].trim_end(), "hello");
        assert_eq!(lines[1].trim_end(), "world");
    }

    #[test]
    fn перерисовка_курсором_видна_как_итог() {
        let mut screen = Screen::new(20, 4);
        // Printed, returned to the start of the line, printed over it — only
        // the second thing should remain on screen. This is exactly why the
        // raw stream can't be read with a regex, and the screen can.
        screen.feed(b"thinking...\r");
        screen.feed(b"done       ");
        assert_eq!(screen.lines()[0].trim_end(), "done");
    }

    #[test]
    fn звонок_замечен_и_не_попал_на_экран() {
        let mut screen = Screen::new(20, 4);
        assert!(screen.feed(b"ping\x07"), "BEL не замечен");
        assert!(!screen.lines()[0].contains('\u{7}'));
        assert!(!screen.feed(b"quiet"), "BEL померещился");
    }

    #[test]
    fn смена_размера_переносит_содержимое() {
        let mut screen = Screen::new(10, 3);
        screen.feed(b"abc");
        screen.resize(40, 10);
        assert_eq!(screen.lines().len(), 10);
        assert_eq!(screen.lines()[0].trim_end(), "abc");
    }
}
