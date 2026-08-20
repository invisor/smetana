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

/// What one described issue costs beyond the description itself: the four
/// spaces it is indented by, and the widest of the two ways it can end
/// (`" \u{2026}\n"`, three bytes of ellipsis among them). Charged against the
/// budget before the descriptions are shared out, because a budget that counts
/// only the text inside the decoration is a budget the decoration then walks
/// straight out of — nine bytes an issue, which at four hundred issues was
/// three and a half kilobytes nobody had accounted for.
const DECORATION: usize = "    ".len() + " \u{2026}\n".len();

/// Room held back from the base lines for the two sentences written around
/// them: the "The issues:" heading, and the notice naming how many issues were
/// left out. Reserved in advance rather than added afterwards, because the
/// notice exists only when the budget ran out — which is precisely the moment
/// there would be no room left to write it in.
const PREAMBLE: usize = 256;

/// The merge lock's label. **The third deliberate copy of this string**,
/// beside `LOCK_LABEL` in `src/stores/tracker.js` — which records why the
/// duplication is accepted — and `runs::queue::LOCK_LABEL`, which is
/// `pub(super)` and belongs to a module that already depends on this one.
const LOCK_LABEL: &str = "smetana-lock";

/// Whether an issue is the merge lock: two leads serializing their merges,
/// which is coordination rather than work.
///
/// Left out of the question as well as out of the answer. The front end drops
/// an id it cannot resolve to a card, so a lock could never reach the screen
/// either way — but describing it to the model spends one of the base lines a
/// tight prompt is counting, and lets it spend one of the twenty answer slots
/// on something nobody can be shown. The exclusion then reads the same on both
/// sides of the wire, which is the other half of why it is here.
pub fn is_lock(issue: &Issue) -> bool {
    issue.labels.iter().any(|label| label == LOCK_LABEL)
}

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

/// The longest prefix of `text` inside `max` **bytes**, cut at a word boundary,
/// and whether anything was cut off.
///
/// Bytes and not characters, and the difference is the whole of this function's
/// history. The budget above is a byte budget — it is a ceiling on an argv
/// argument — and counting the slices out in characters bought up to four times
/// the prompt it was told it was buying. This tracker's own issues are written
/// in Russian, two bytes to the letter: one long description came to 60 K
/// against a 48 K ceiling, and a hundred and fifteen of them to 85 K. Nothing
/// crashed, because `ARG_MAX` is a megabyte and the drift had two orders of
/// magnitude to play with — which is exactly why it would have gone on
/// unnoticed, and why the ceiling is only worth having if it is real.
fn slice_at(text: &str, max: usize) -> (&str, bool) {
    if text.len() <= max {
        return (text, false);
    }
    // The last character boundary at or inside `max`, then the last word
    // boundary at or inside that. `str::floor_char_boundary` is still unstable,
    // so the walk is by hand; slicing anywhere else would panic.
    let mut end = 0;
    for (at, ch) in text.char_indices() {
        if at + ch.len_utf8() > max {
            break;
        }
        end = at + ch.len_utf8();
    }
    let cut = text[..end].rfind(char::is_whitespace).unwrap_or(end);
    (&text[..cut], true)
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
    let for_base = budget.saturating_sub(PREAMBLE);
    for issue in &ordered {
        let line = base_line(issue);
        if spent + line.len() > for_base {
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
    }
    out.push_str("The issues:\n\n");

    // Whatever room is left, shared evenly — in bytes, which is the unit the
    // ceiling is written in and the unit `slice_at` cuts by. Everything already
    // written counts against it, and so does the decoration each described line
    // has yet to cost, charged for every issue rather than only for the ones
    // that turn out to carry a description: over-charging leaves room unspent,
    // and under-charging is how a ceiling stops being one.
    spent += out.len() + base.len() * DECORATION;
    let share = budget.saturating_sub(spent) / base.len().max(1);

    for (issue, line) in base {
        out.push_str(&line);
        let Some(description) = issue.description.as_deref() else { continue };
        let description = description.trim();
        // Under forty bytes there is no phrase left to read, only the first
        // word or two of one — fewer still where the text is not ASCII — so the
        // line is left as its base alone.
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

    /// A word that is not one byte to the letter, which is the only kind that
    /// can see the difference between a byte budget and a character one.
    ///
    /// It stands in for the Russian this project's own tracker is written in —
    /// two bytes a letter, the same width as the `\u{e9}` below — and the
    /// three-byte character is here because two bytes alone would leave the
    /// widest case untested. It is spelled in Latin script because there is no
    /// Cyrillic anywhere in this tree, deliberately, and a fixture string is
    /// named in that rule.
    const WIDE_WORD: &str = "caf\u{e9}\u{2026} ";

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
        for word in ["word ", WIDE_WORD] {
            let mut one = issue("a-1", "First", "2026-08-01T00:00:00Z");
            one.description = Some(word.repeat(20_000));
            let out = prompt("anything", &[one]);
            assert!(
                out.len() <= MAX_CORPUS,
                "the prompt stays inside its budget on {word:?}: {}",
                out.len()
            );
            assert!(out.contains(word.trim()), "some of the description survives the cut");
        }
    }

    /// The ceiling is in bytes, and a corpus that is not ASCII is where a
    /// budget spent in characters shows: every slice comes back up to four
    /// times the size it was budgeted at, and the whole prompt with it. An
    /// ASCII fixture cannot see this at all, which is how it went unnoticed.
    #[test]
    fn a_budget_in_bytes_holds_where_the_text_is_not_ascii() {
        let issues: Vec<Issue> = (0..115)
            .map(|n| {
                let mut one =
                    issue(&format!("a-{n}"), "A title", &format!("2026-08-01T00:00:{:02}Z", n % 60));
                one.description = Some(WIDE_WORD.repeat(4_000));
                one
            })
            .collect();
        let out = prompt("anything", &issues);
        assert!(out.len() <= MAX_CORPUS, "the prompt stays inside its budget: {}", out.len());
        assert!(out.contains(WIDE_WORD.trim()), "the descriptions are there, cut rather than gone");
    }

    #[test]
    fn dropping_issues_is_announced_rather_than_silent() {
        // Enough title bytes that not even the base lines fit — once in ASCII
        // and once in text that is not, since a title is budgeted in bytes too.
        for word in ["a long title ", WIDE_WORD] {
            let issues: Vec<Issue> = (0..4000)
                .map(|n| issue(&format!("a-{n}"), &word.repeat(8), "2026-08-01T00:00:00Z"))
                .collect();
            let out = prompt("anything", &issues);
            assert!(out.len() <= MAX_CORPUS, "budget on {word:?}: {}", out.len());
            assert!(
                out.contains("were left out"),
                "a model told the whole corpus was there when it was not will answer 'no such \
                 task' about a task that exists: {out}"
            );
        }
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
    fn the_merge_lock_is_not_an_issue_anybody_meant() {
        let mut lock = issue("a-9", "merge lock", "2026-08-01T00:00:00Z");
        lock.labels = vec!["smetana-lock".into()];
        assert!(is_lock(&lock));
        assert!(!is_lock(&issue("a-1", "First", "2026-08-01T00:00:00Z")));

        // Neither described to the model nor accepted back from it: the caller
        // builds both the corpus and the known set from the filtered list.
        let kept: Vec<Issue> = vec![lock.clone(), issue("a-1", "First", "2026-08-01T00:00:00Z")]
            .into_iter()
            .filter(|one| !is_lock(one))
            .collect();
        let out = prompt("anything", &kept);
        assert!(!out.contains("a-9"), "the lock is not in the question: {out}");
        assert!(out.contains("a-1"));
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
