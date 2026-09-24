//! The session's screen: what the app reads to understand what's happening.
//! The agent's raw stream is a mess of cursor moves and redraws, with nothing
//! in it worth searching; the screen is the text a person actually sees.

/// The bell as the parser understands it, not as a byte scan would. 0x07 is
/// both the C0 BEL and the terminator of an OSC string, and the difference is
/// not visible without parsing: `ESC ] 0 ; title BEL` sets the window title
/// and rings nothing. Claude Code sends exactly that as the first thing it
/// prints, so a scan for the byte lit "needs you" on every freshly started
/// agent — until the person opened the tab, which cleared the bell and made
/// the row go quiet on its own. vte calls `audible_bell` only for the real
/// one; inside an OSC the same byte ends the string and never reaches here.
#[derive(Default)]
struct Bell(bool);

impl vt100::Callbacks for Bell {
    fn audible_bell(&mut self, _: &mut vt100::Screen) {
        self.0 = true;
    }
}

pub struct Screen {
    parser: vt100::Parser<Bell>,
}

impl Screen {
    pub fn new(cols: u16, rows: u16) -> Self {
        // No scrollback needed here: the ring holds that for a person, and
        // detection only needs the visible screen.
        Self { parser: vt100::Parser::new_with_callbacks(rows, cols, 0, Bell::default()) }
    }

    /// Feed a chunk of output. Returns `true` if the chunk rang the bell: the
    /// parser swallows it, but layer A — the detection logic that reads this
    /// screen to decide session state — needs to know.
    pub fn feed(&mut self, bytes: &[u8]) -> bool {
        self.parser.callbacks_mut().0 = false;
        self.parser.process(bytes);
        self.parser.callbacks().0
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

    /// The visible rows plus whether each row's first printed glyph is dim.
    ///
    /// Codex renders its activity summaries with a dim `•`, while a completed
    /// assistant reply owns a bold one. `vt100` deliberately keeps cells'
    /// attributes private, but its formatted screen is the public faithful
    /// projection of those attributes. Read only the SGR state at each row's
    /// first non-whitespace glyph; the text reader remains a reader of the
    /// same visible rows a person sees.
    pub fn lines_with_entry_dim(&self) -> (Vec<String>, Vec<bool>) {
        let lines = self.lines();
        let formatted = self.parser.screen().contents_formatted();
        let styles = entry_dim(&formatted, lines.len());
        (lines, styles)
    }
}

/// Whether each visible row begins in SGR dim style.
///
/// `contents_formatted` emits a sparse grid with cursor positions and CRLF
/// between adjacent rows. Only SGR's reset, dim, and normal-intensity controls
/// affect the one fact callers need. Cursor-position CSI commands keep the row
/// counter aligned with the plain `rows` view; all other CSI commands are
/// irrelevant to entry style.
fn entry_dim(formatted: &[u8], rows: usize) -> Vec<bool> {
    let mut out = vec![false; rows];
    let mut seen = vec![false; rows];
    let mut row = 0;
    let mut dim = false;
    let mut at = 0;
    while at < formatted.len() && row < rows {
        match formatted[at] {
            b'\x1b' if formatted.get(at + 1) == Some(&b'[') => {
                let Some(end) = formatted[at + 2..]
                    .iter()
                    .position(|byte| (b'@'..=b'~').contains(byte))
                    .map(|offset| at + 2 + offset)
                else {
                    break;
                };
                let codes: Vec<u16> = formatted[at + 2..end]
                        .split(|byte| *byte == b';')
                        .map(|part| std::str::from_utf8(part).ok().and_then(|part| part.parse::<u16>().ok()).unwrap_or(0))
                        .collect();
                if formatted[end] == b'm' {
                    for code in codes {
                        match code {
                            0 | 22 => dim = false,
                            2 => dim = true,
                            _ => {}
                        }
                    }
                } else if matches!(formatted[end], b'H' | b'f') {
                    row = codes.first().copied().unwrap_or(1).saturating_sub(1) as usize;
                }
                at = end + 1;
            }
            b'\r' => at += 1,
            b'\n' => {
                row += 1;
                at += 1;
            }
            byte => {
                if !seen[row] && !byte.is_ascii_whitespace() {
                    seen[row] = true;
                    out[row] = dim;
                }
                at += 1;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shows_what_was_printed() {
        let mut screen = Screen::new(20, 4);
        screen.feed(b"hello\r\nworld\r\n");
        let lines = screen.lines();
        assert_eq!(lines[0].trim_end(), "hello");
        assert_eq!(lines[1].trim_end(), "world");
    }

    #[test]
    fn preserves_dim_entry_style_beside_the_visible_rows() {
        let mut screen = Screen::new(40, 3);
        screen.feed(b"\x1b[2m\xe2\x80\xa2 \x1b[22mCalled server.tool(what?)\r\n\x1b[1m\xe2\x80\xa2 \x1b[22mRead this? Then confirm.\r\n");
        let (lines, dim) = screen.lines_with_entry_dim();
        assert_eq!(lines[0].trim_end(), "• Called server.tool(what?)");
        assert_eq!(lines[1].trim_end(), "• Read this? Then confirm.");
        assert_eq!(dim, vec![true, false, false]);
    }

    #[test]
    fn a_cursor_repaint_is_seen_as_the_final_result() {
        let mut screen = Screen::new(20, 4);
        // Printed, returned to the start of the line, printed over it — only
        // the second thing should remain on screen. This is exactly why the
        // raw stream can't be read with a regex, and the screen can.
        screen.feed(b"thinking...\r");
        screen.feed(b"done       ");
        assert_eq!(screen.lines()[0].trim_end(), "done");
    }

    #[test]
    fn the_bell_is_noticed_and_never_reaches_the_screen() {
        let mut screen = Screen::new(20, 4);
        assert!(screen.feed(b"ping\x07"), "BEL went unnoticed");
        assert!(!screen.lines()[0].contains('\u{7}'));
        assert!(!screen.feed(b"quiet"), "BEL was imagined");
    }

    #[test]
    fn a_window_title_is_not_a_bell() {
        // OSC ends with that same 0x07 byte, and Claude Code sets the window
        // title as the very first thing it prints: `ESC ] 0 ; * Claude Code BEL`.
        // Taking that for a bell means lighting up "someone is waiting" on every start.
        let mut screen = Screen::new(40, 4);
        assert!(
            !screen.feed(b"\x1b]0;\xe2\x9c\xb3 Claude Code\x07\x1b[?25l"),
            "the OSC terminator was taken for a bell"
        );
    }

    #[test]
    fn a_title_split_across_chunks_is_not_a_bell_either() {
        // The PTY is read in 4 KiB pieces, and a chunk boundary falls wherever
        // it falls. The parse state lives in the parser between calls, so the
        // tail of an OSC does not look like a bell on its own.
        let mut screen = Screen::new(40, 4);
        assert!(!screen.feed(b"\x1b]0;Claude"), "the start of an OSC was taken for a bell");
        assert!(!screen.feed(b" Code\x07"), "the tail of an OSC was taken for a bell");
    }

    #[test]
    fn a_bell_after_a_title_is_still_a_bell() {
        let mut screen = Screen::new(40, 4);
        assert!(screen.feed(b"\x1b]0;title\x07ready\x07"), "a bell following an OSC was lost");
    }

    #[test]
    fn a_resize_carries_the_contents_over() {
        let mut screen = Screen::new(10, 3);
        screen.feed(b"abc");
        screen.resize(40, 10);
        assert_eq!(screen.lines().len(), 10);
        assert_eq!(screen.lines()[0].trim_end(), "abc");
    }
}
