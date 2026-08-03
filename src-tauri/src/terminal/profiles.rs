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

    let mut options = Vec::new();
    let mut selected = None;
    for line in &framed {
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

    let text = framed
        .iter()
        .map(|l| l.trim_matches(['│', ' ']).trim())
        .find(|l| l.ends_with('?'))?
        .to_owned();

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
}
