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

/// The SGR presentation of a visible row's marker and header span.
///
/// This deliberately carries only the properties Codex's transcript reader
/// needs. The ordinary text projection stays attribute-free for quiet-state
/// fingerprinting, so a colour-only repaint cannot make a waiting dialog look
/// busy again.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EntryStyle {
    /// The first non-whitespace glyph: Codex's `•`, `◦`, or `›` marker.
    pub dim: bool,
    pub bold: bool,
    pub foreground: bool,
    /// The first ASCII word glyph after a marker. Codex gives the `Explored`
    /// header this bold span while leaving its own bullet dim/default.
    pub header_dim: bool,
    pub header_bold: bool,
    pub header_foreground: bool,
}

impl EntryStyle {
    /// Codex draws completed `Ran` and `Called` tools as coloured bold bullets.
    /// This is a renderer contract, not a classification of English prose.
    pub fn is_coloured_bold_marker(self) -> bool {
        self.bold && self.foreground
    }
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

    /// The visible rows plus each row's first printed glyph style.
    ///
    /// `vt100` deliberately keeps cells' attributes private, but its formatted
    /// screen is their public faithful projection. Read only the SGR state at
    /// each row's first non-whitespace glyph; the text reader remains a reader
    /// of the same visible rows a person sees.
    pub fn lines_with_entry_style(&self) -> (Vec<String>, Vec<EntryStyle>) {
        let lines = self.lines();
        let formatted = self.parser.screen().contents_formatted();
        let styles = entry_style(&formatted, lines.len());
        (lines, styles)
    }
}

/// The SGR presentation of each visible row's marker and header span.
///
/// `contents_formatted` emits a sparse grid with cursor positions and CRLF
/// between adjacent rows. Cursor-position CSI commands keep the row counter
/// aligned with the plain `rows` view; all other CSI commands are irrelevant
/// to entry style.
fn entry_style(formatted: &[u8], rows: usize) -> Vec<EntryStyle> {
    let mut out = vec![EntryStyle::default(); rows];
    let mut seen = vec![false; rows];
    let mut header_seen = vec![false; rows];
    let mut row = 0;
    let mut style = EntryStyle::default();
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
                    let mut codes = codes.into_iter();
                    while let Some(code) = codes.next() {
                        match code {
                            0 => style = EntryStyle::default(),
                            1 => style.bold = true,
                            2 => style.dim = true,
                            22 => {
                                style.bold = false;
                                style.dim = false;
                            }
                            30..=37 | 90..=97 => style.foreground = true,
                            39 => style.foreground = false,
                            38 => match codes.next() {
                                Some(5) => {
                                    let _ = codes.next();
                                    style.foreground = true;
                                }
                                Some(2) => {
                                    let _ = (codes.next(), codes.next(), codes.next());
                                    style.foreground = true;
                                }
                                _ => {}
                            },
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
                    out[row] = style;
                } else if seen[row] && !header_seen[row] && byte.is_ascii_alphanumeric() {
                    header_seen[row] = true;
                    out[row].header_dim = style.dim;
                    out[row].header_bold = style.bold;
                    out[row].header_foreground = style.foreground;
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
    fn preserves_assistant_and_activity_entry_styles_beside_visible_rows() {
        fn capture(bytes: &[u8]) -> (Vec<String>, Vec<EntryStyle>) {
            let mut screen = Screen::new(80, 8);
            screen.feed(bytes);
            screen.lines_with_entry_style()
        }
        let (assistant, assistant_style) = capture(include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/codex-0.155-completed-assistant-question.ansi")));
        assert_eq!(assistant[0].trim_end(), "• Read this? Then confirm.");
        assert_eq!(
            assistant_style[0],
            EntryStyle { dim: true, bold: false, foreground: false, ..EntryStyle::default() }
        );
        for (raw, text) in [
            (include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/codex-0.155-completed-ran-question.ansi")).as_slice(), "• Ran rg 'what?' src"),
            (include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/codex-0.155-completed-called-question.ansi")).as_slice(), "• Called server.tool({\"document\":\"what?\"})"),
        ] {
            let (lines, styles) = capture(raw);
            assert_eq!(lines[0].trim_end(), text);
            assert!(styles[0].is_coloured_bold_marker(), "activity style was lost: {text}");
        }
        let (explored, explored_style) = capture(include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/codex-0.155-completed-explored-question.ansi")));
        assert_eq!(explored[0].trim_end(), "• Explored");
        assert_eq!(explored[2].trim(), "└ Read what?");
        assert_eq!(
            explored_style[0],
            EntryStyle {
                dim: true,
                bold: false,
                foreground: false,
                header_dim: true,
                header_bold: true,
                header_foreground: false,
            }
        );
    }

    #[test]
    fn entry_style_reads_colour_forms_and_sparse_row_positions() {
        fn marker(sgr: &[u8]) -> EntryStyle {
            let mut screen = Screen::new(40, 6);
            let mut raw = b"\x1b[4;1H".to_vec();
            raw.extend_from_slice(sgr);
            raw.extend_from_slice(b"\xe2\x80\xa2 header");
            screen.feed(&raw);
            let (_, styles) = screen.lines_with_entry_style();
            styles[3]
        }
        let reset = marker(b"\x1b[1;31m\x1b[39m");
        assert!(reset.bold && !reset.foreground, "SGR 39 did not restore the default foreground");
        assert!(marker(b"\x1b[1;38;5;208m").is_coloured_bold_marker());
        assert!(marker(b"\x1b[1;38;2;1;2;3m").is_coloured_bold_marker());
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
