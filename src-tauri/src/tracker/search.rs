//! Asking the agent which tasks a person meant, and reading the answer.
//!
//! The instant search in the front end is a substring match over the same
//! snapshot and costs nothing; this is the other half, and it is bought rather
//! than free — one question to the harness through `agents::oneshot`, the same
//! quiet mechanism that writes a commit message for the Git panel. No PTY, no
//! row in the agent list, nothing claimed and nothing written.
//!
//! Everything here is pure. The spawning is `oneshot::ask_raw`'s and the
//! snapshot is the worker's; what this file owns is the shape of the question
//! and the reading of the answer, which is what the tests at the bottom pin.

use std::collections::HashSet;

use super::model::Issue;

/// The prompt rides as an argv argument, which is what puts a ceiling here at
/// all: `ARG_MAX` is a megabyte on both platforms this ships to, and 48 K
/// leaves that untouchable. It is the same number `oneshot::MAX_PATCH` uses and
/// for the same reason.
///
/// Measured against this repository's own tracker at 115 issues: ids and titles
/// come to 8.8 K, and the whole prose to 673 K. So titles always fit and the
/// prose never does, which is the shape `corpus` below is written around.
pub const MAX_CORPUS: usize = 48 * 1024;

/// How many ids the agent may answer with. This is not presentation: it is what
/// keeps `oneshot::ask_raw`'s bounded-output invariant true, since both pipes
/// are read only after the child has gone. So the number belongs in the
/// instruction as well as here — see `prompt` below.
pub const MAX_HITS: usize = 20;

/// One issue, as the line that is always affordable.
fn base_line(issue: &Issue) -> String {
    format!(
        "{} · {} · {} · {}\n",
        issue.id,
        issue.issue_type.as_deref().unwrap_or("task"),
        issue.status,
        issue.title.replace('\n', " ")
    )
}

/// The longest prefix of `text` inside `max` characters, cut at a word boundary,
/// and whether anything was cut off.
fn slice_at(text: &str, max: usize) -> (&str, bool) {
    match text.char_indices().nth(max) {
        None => (text, false),
        Some((at, _)) => {
            let cut = text[..at].rfind(char::is_whitespace).unwrap_or(at);
            (&text[..cut], true)
        }
    }
}

/// Every issue the budget can hold, newest first.
///
/// Two passes, and the order is the whole rule. The base line for every issue
/// comes first, because an id and a title are what make an issue nameable at
/// all; whatever is left over is then spent on descriptions, shared evenly.
///
/// **A cut is announced rather than silent**, which is `oneshot::commit_prompt`'s
/// own rule and it matters more here: a model told the whole board was in front
/// of it, when the oldest third was not, will answer "no such task" about a task
/// that exists.
fn corpus(issues: &[Issue], budget: usize) -> String {
    let mut ordered: Vec<&Issue> = issues.iter().collect();
    ordered.sort_by(|a, b| b.updated_at.cmp(&a.updated_at).then(a.id.cmp(&b.id)));

    let mut base = Vec::new();
    let mut spent = 0usize;
    for issue in &ordered {
        let line = base_line(issue);
        if spent + line.len() > budget {
            break;
        }
        spent += line.len();
        base.push((*issue, line));
    }

    let dropped = ordered.len() - base.len();
    let mut out = String::new();
    if dropped > 0 {
        out.push_str(&format!(
            "This project has more issues than fit here. The {} least recently updated \
             were left out, so a task you cannot see below may still exist.\n\n",
            dropped
        ));
        spent += out.len();
    }

    // Whatever room is left, shared evenly. Characters rather than bytes, which
    // is what `slice_at` counts; the difference only ever leaves room unspent.
    let share = budget.saturating_sub(spent) / base.len().max(1);

    out.push_str("The issues:\n\n");
    for (issue, line) in base {
        out.push_str(&line);
        let Some(description) = issue.description.as_deref() else { continue };
        let description = description.trim();
        // Under forty characters there is no phrase left to read, only the
        // first two words of one, so the line is left as its base alone.
        if description.is_empty() || share < 40 {
            continue;
        }
        let (slice, was_cut) = slice_at(description, share);
        out.push_str("    ");
        out.push_str(&slice.replace('\n', " "));
        out.push_str(if was_cut { " …\n" } else { "\n" });
    }
    out
}

/// The whole question, instruction first.
///
/// The order is the point, and it is `commit_prompt`'s: the instruction at the
/// head, so a model that reads no further than the top of a long prompt still
/// has the whole task, and the corpus below it where a cut costs the least.
pub fn prompt(query: &str, issues: &[Issue]) -> String {
    let head = format!(
        "Below is every issue in a project: its id, type, status, title, and the start of its \
         description. Find the ones that match this search — by meaning, not by wording:\n\
         \n\
         {query}\n\
         \n\
         Answer with issue ids, one per line, best match first, at most {MAX_HITS} of them, and \
         nothing else: no prose, no numbering, no code fence. If none of them match, answer with \
         the single word NONE.\n\n"
    );
    let budget = MAX_CORPUS.saturating_sub(head.len());
    format!("{head}{}", corpus(issues, budget))
}

/// What the harness printed, as ids this board actually holds.
///
/// Filtered against the snapshot rather than trusted: a model asked for ids
/// invents one now and then, and an invented id would draw a row for a task
/// that does not exist. `NONE` needs no case of its own — it matches nothing
/// known and falls out here like any other word.
pub fn parse(raw: &str, known: &HashSet<String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for line in raw.lines() {
        let line = line.trim().trim_start_matches(['-', '*', '•']).trim();
        if line.is_empty() || line.starts_with("```") {
            continue;
        }
        let token = line
            .split_whitespace()
            .next()
            .unwrap_or_default()
            .trim_end_matches([',', '.', ':', ';', ')']);
        if known.contains(token) && !out.iter().any(|seen| seen == token) {
            out.push(token.to_string());
        }
        if out.len() >= MAX_HITS {
            break;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn issue(id: &str, title: &str, updated: &str) -> Issue {
        let mut issue = Issue::default();
        issue.id = id.into();
        issue.title = title.into();
        issue.status = "open".into();
        issue.updated_at = updated.into();
        issue
    }

    #[test]
    fn every_issue_contributes_a_line_when_they_all_fit() {
        let issues = vec![
            issue("a-1", "First", "2026-08-01T00:00:00Z"),
            issue("a-2", "Second", "2026-08-02T00:00:00Z"),
        ];
        let out = prompt("anything", &issues);
        assert!(out.contains("a-1"), "{out}");
        assert!(out.contains("a-2"), "{out}");
        assert!(out.contains("First"));
    }

    #[test]
    fn the_instruction_comes_before_the_corpus() {
        let issues = vec![issue("a-1", "First", "2026-08-01T00:00:00Z")];
        let out = prompt("bells", &issues);
        let instruction = out.find("one per line").expect("the instruction is there");
        let corpus = out.find("a-1").expect("the corpus is there");
        assert!(instruction < corpus, "a model reading only the head must have the whole task");
    }

    #[test]
    fn the_query_reaches_the_prompt() {
        assert!(prompt("the bell is silent", &[]).contains("the bell is silent"));
    }

    #[test]
    fn a_description_is_cut_rather_than_dropped() {
        let mut one = issue("a-1", "First", "2026-08-01T00:00:00Z");
        one.description = Some("word ".repeat(20_000));
        let out = prompt("anything", &[one]);
        assert!(out.len() <= MAX_CORPUS + 512, "the prompt stays inside its budget: {}", out.len());
        assert!(out.contains("word"), "some of the description survives the cut");
    }

    #[test]
    fn dropping_issues_is_announced_rather_than_silent() {
        // Enough title bytes that not even the base lines fit.
        let issues: Vec<Issue> = (0..4000)
            .map(|n| issue(&format!("a-{n}"), &"a long title ".repeat(8), "2026-08-01T00:00:00Z"))
            .collect();
        let out = prompt("anything", &issues);
        assert!(out.len() <= MAX_CORPUS + 512);
        assert!(
            out.contains("were left out"),
            "a model told the whole corpus was there when it was not will answer 'no such task' \
             about a task that exists: {out}"
        );
    }

    #[test]
    fn the_newest_issues_are_the_ones_kept() {
        let issues: Vec<Issue> = (0..4000)
            .map(|n| {
                issue(
                    &format!("a-{n}"),
                    &"a long title ".repeat(8),
                    &format!("2026-08-01T00:00:{:02}Z", n % 60),
                )
            })
            .collect();
        let out = prompt("anything", &issues);
        assert!(out.contains(":59Z") || out.contains("a-59"), "the freshest survive the cut");
    }

    #[test]
    fn parse_takes_ids_the_snapshot_knows_and_drops_the_rest() {
        let known: HashSet<String> = ["a-1", "a-2"].iter().map(|s| s.to_string()).collect();
        assert_eq!(parse("a-2\na-1\n", &known), vec!["a-2", "a-1"]);
        assert_eq!(parse("a-9\n", &known), Vec::<String>::new());
    }

    #[test]
    fn parse_survives_the_ways_a_model_dresses_a_list_up() {
        let known: HashSet<String> = ["a-1", "a-2"].iter().map(|s| s.to_string()).collect();
        assert_eq!(parse("```\n- a-1\n* a-2\n```", &known), vec!["a-1", "a-2"]);
        assert_eq!(parse("a-1, the bell one\na-2.", &known), vec!["a-1", "a-2"]);
    }

    #[test]
    fn none_is_an_answer_rather_than_a_failure() {
        let known: HashSet<String> = ["a-1"].iter().map(|s| s.to_string()).collect();
        assert_eq!(parse("NONE", &known), Vec::<String>::new());
    }

    #[test]
    fn parse_says_each_id_once_and_stops_at_the_cap() {
        let known: HashSet<String> = (0..40).map(|n| format!("a-{n}")).collect();
        assert_eq!(parse("a-1\na-1\na-2\n", &known), vec!["a-1", "a-2"]);
        let many = (0..40).map(|n| format!("a-{n}")).collect::<Vec<_>>().join("\n");
        assert_eq!(parse(&many, &known).len(), MAX_HITS);
    }
}
