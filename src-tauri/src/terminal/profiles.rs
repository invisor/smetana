//! Screen templates for specific CLIs. Data, not logic: moving them into
//! configuration when that becomes necessary should be a relocation, not a
//! rewrite.
//!
//! This reads someone else's interface, and an agent's major update breaks
//! it. It breaks softly: no match leaves layer A in place, and the app says
//! "someone is waiting" instead of "here is the question" — it does not lie.

use super::model::{Question, QuestionOption};

/// An option line: `❯ 1. Yes` or `  2. Yes, and don't ask again`.
/// Returns (index, label, whether it is selected).
fn option_line(line: &str) -> Option<(usize, String, bool)> {
    let selected = line.contains('❯');
    let rest = line.trim_start_matches(['│', ' ', '❯']).trim_start();
    let dot = rest.find(". ")?;
    let index: usize = rest[..dot].parse().ok()?;
    let label = rest[dot + 2..].trim_end_matches(['│', ' ']).trim_end().to_owned();
    if label.is_empty() {
        return None;
    }
    Some((index, label, selected))
}

/// Claude Code's permission dialog: a frame, a question line, numbered
/// options. What separates it from any numbered list in ordinary output is
/// the frame around it and a question mark in the text.
pub fn claude(screen: &[String]) -> Option<Question> {
    let framed: Vec<&String> = screen.iter().filter(|l| l.contains('│')).collect();
    if framed.is_empty() {
        return None;
    }

    // The first option line marks where the question ends and the option
    // list begins: everything above it is candidate question text,
    // everything from it on is never text, so an option label that itself
    // ends in '?' cannot be mistaken for the question.
    let first_option = framed.iter().position(|l| option_line(l).is_some())?;

    let mut options = Vec::new();
    let mut selected = None;
    for line in &framed[first_option..] {
        if let Some((index, label, is_selected)) = option_line(line) {
            if is_selected {
                selected = Some(options.len());
            }
            options.push(QuestionOption { label, send: format!("{index}\r") });
        }
    }
    if options.is_empty() {
        return None;
    }

    // A 120-column terminal can be narrower than what a human sees, so the
    // dialog's question can wrap across more than one framed row. Join what
    // is above the first option into a single sentence instead of picking
    // whichever row happens to end in '?' — split_whitespace both drops the
    // frame's padding and collapses any run of whitespace to one space.
    let text = framed[..first_option]
        .iter()
        .flat_map(|l| l.trim_matches(['│', ' ']).split_whitespace())
        .collect::<Vec<_>>()
        .join(" ");
    if !text.ends_with('?') {
        return None;
    }

    Some(Question { text, options, selected })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> Vec<String> {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name);
        std::fs::read_to_string(path).unwrap().lines().map(str::to_owned).collect()
    }

    #[test]
    fn узнаёт_диалог_разрешений() {
        let q = claude(&fixture("claude-permission-dialog.txt")).expect("диалог не узнан");
        assert!(q.text.contains("Do you want to make this edit"), "текст вопроса: {:?}", q.text);
        assert_eq!(q.options.len(), 3);
        assert_eq!(q.options[0].label, "Yes");
        assert_eq!(q.options[0].send, "1\r");
        assert_eq!(q.selected, Some(0));
    }

    #[test]
    fn обычная_работа_не_диалог() {
        let screen: Vec<String> = ["Reading tabs.js", "  1. checked", "Done"].iter().map(|s| (*s).to_owned()).collect();
        assert!(claude(&screen).is_none(), "пронумерованный список принят за вопрос");
    }

    #[test]
    fn наполовину_дорисованная_рамка_не_диалог() {
        let screen: Vec<String> = ["╭───────────╮", "│ Edit file │"].iter().map(|s| (*s).to_owned()).collect();
        assert!(claude(&screen).is_none(), "рамка без вариантов принята за вопрос");
    }

    #[test]
    fn перенесённый_вопрос_склеивается_в_одну_строку() {
        // No title line above the question here on purpose: the assertion
        // below checks the joined text for exact equality, and the join
        // (per the fix) includes everything above the first option line —
        // a title row would have to be part of the expected string too.
        // The existing `узнаёт_диалог_разрешений` test, using the real
        // fixture that does carry a title, checks with `.contains` for
        // exactly this reason.
        let screen: Vec<String> = [
            "╭──────────────────────────────────────────────────────╮",
            "│ Do you want to make this edit to                      │",
            "│ some/very/long/path/to/file.js?                       │",
            "│                                                       │",
            "│ ❯ 1. Yes                                              │",
            "│   2. Yes, and don't ask again this session            │",
            "│   3. No, and tell Claude what to do differently       │",
            "╰──────────────────────────────────────────────────────╯",
        ]
        .iter()
        .map(|s| (*s).to_owned())
        .collect();
        let q = claude(&screen).expect("перенесённый вопрос не узнан");
        assert_eq!(q.text, "Do you want to make this edit to some/very/long/path/to/file.js?");
    }

    #[test]
    fn вариант_с_вопросом_в_ярлыке_не_путают_с_вопросом() {
        let screen: Vec<String> = [
            "╭──────────────────────────────────────────────────────╮",
            "│ Do you want to make this edit to tabs.js?             │",
            "│                                                       │",
            "│ ❯ 1. Yes                                              │",
            "│   2. Wait, are you sure about this?                   │",
            "│   3. No, and tell Claude what to do differently       │",
            "╰──────────────────────────────────────────────────────╯",
        ]
        .iter()
        .map(|s| (*s).to_owned())
        .collect();
        let q = claude(&screen).expect("диалог не узнан");
        assert_eq!(q.text, "Do you want to make this edit to tabs.js?");
    }
}
