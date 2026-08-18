//! The run's document: a self-contained HTML file under `.smetana/reports/`.
//!
//! Pure — values in, a string out — with the disk left to `service.rs`, the
//! same split `attachments/cleanup.rs` keeps.
//!
//! **The styling exception.** Everywhere else in this repository a visual value
//! is a `var(--token)` reference and never a literal, and this file is the
//! third documented exception beside `files/editor/theme.js` and
//! `terminal/theme.js`. It is a wider one than either: there are no tokens
//! around this document at all, because its whole purpose is to be readable in
//! a browser with no app in the picture — no stylesheet of ours is loaded there
//! and nothing would resolve a custom property. What replaces the rule is
//! narrower than "anything goes": no external stylesheet, no font off a
//! network, no script, no image. Everything the document needs it carries, and
//! it reaches nowhere at all.
//!
//! The other rule with no exception to it: `did` is text an agent wrote, going
//! into a document a person opens, so every value that reaches the output goes
//! through `escape` at its own call site.

use serde::Deserialize;

use crate::runs::summary::{TaskLine, Tasks};

/// One task as the batch's own file describes it. `did` is free text an agent
/// wrote, and it is escaped on the way into the document without exception.
#[derive(Debug, Clone, Deserialize)]
pub struct BatchTask {
    pub id: String,
    #[serde(default)]
    pub did: Option<String>,
}

/// What one batch contributed. `reported` is false when the batch left no file
/// or left one that could not be read — a batch that was killed, crashed or
/// cancelled. The distinction is drawn rather than smoothed over: an empty row
/// reads as "nothing was done", which is a different claim.
#[derive(Debug, Clone)]
pub struct BatchLine {
    pub n: u32,
    pub seconds: u64,
    pub tasks: Vec<BatchTask>,
    pub notes: Option<String>,
    pub reported: bool,
}

/// What `parse_batch` answers: the file's contents, and whether it was readable
/// at all.
pub struct ParsedBatch {
    pub tasks: Vec<BatchTask>,
    pub notes: Option<String>,
    pub reported_ok: bool,
}

#[derive(Deserialize)]
struct BatchFile {
    #[serde(default)]
    tasks: Vec<BatchTask>,
    #[serde(default)]
    notes: Option<String>,
}

/// Everything the document is made of. `tasks` is `Option` for the reason
/// `RunSummary::tasks` is: an unreadable board and an empty board are opposite
/// facts, and this document must never turn the first into a confident zero.
pub struct RunReport<'a> {
    /// What this document is a report of, which is the run's mode read out —
    /// `RunMode::report_title`. This file places the words and owns none of
    /// them.
    pub title: &'a str,
    pub project: &'a str,
    pub scope: &'a str,
    pub finished: &'a str,
    pub seconds: u64,
    pub tasks: Option<&'a Tasks>,
    pub batches: &'a [BatchLine],
}

/// Malformed is an ordinary outcome, not an error: the batch's tasks still
/// appear from the board, and the document says the batch left no account of
/// itself.
pub fn parse_batch(text: &str) -> ParsedBatch {
    match serde_json::from_str::<BatchFile>(text) {
        Ok(file) => ParsedBatch { tasks: file.tasks, notes: file.notes, reported_ok: true },
        Err(_) => ParsedBatch { tasks: vec![], notes: None, reported_ok: false },
    }
}

/// The batch a task's row belongs to: **the last one to name it**, which is what
/// `.rev()` is for. A task can be touched twice in one run — Phase R recovers an
/// orphan that a later batch then carries through — and the row it sits in is
/// filed under the status it ended in, so the account and the time beside it
/// both have to come from the batch that produced that ending.
///
/// One search, deliberately, and everything about a row hangs off it. Two
/// searches is how the account and the duration came to name different batches:
/// "the last batch that mentioned it" and "the last batch that held it alone"
/// are not the same batch, and for a solo batch followed by a shared one the row
/// showed the shared batch's words beside the solo batch's hour.
fn last_naming<'a>(batches: &'a [BatchLine], id: &str) -> Option<&'a BatchLine> {
    batches.iter().rev().find(|b| b.tasks.iter().any(|t| t.id == id))
}

/// A task gets a duration of its own only when the batch that owns its row held
/// exactly one task — then the two are the same number and nothing is being
/// inferred. Otherwise the row shows none, because dividing a batch's hours by
/// its task count would be a number that looks measured and is not, and so would
/// a number borrowed from a batch that did other work.
fn duration_of(batch: &BatchLine) -> Option<u64> {
    (batch.tasks.len() == 1).then_some(batch.seconds)
}

/// What a task's row says about time: `duration_of` applied to the one batch
/// `last_naming` picked, never to a batch found by a search of its own.
pub fn task_duration(batches: &[BatchLine], id: &str) -> Option<u64> {
    last_naming(batches, id).and_then(duration_of)
}

/// Hours and minutes, or minutes, or seconds. Never a bare count of seconds
/// past a minute — nobody reads a night as 8040.
pub fn human(seconds: u64) -> String {
    let (hours, minutes) = (seconds / 3600, (seconds % 3600) / 60);
    if hours > 0 {
        format!("{hours}h {minutes}m")
    } else if minutes > 0 {
        format!("{minutes}m")
    } else {
        format!("{seconds}s")
    }
}

/// The ampersand first, or every replacement after it would be escaped a second
/// time. The apostrophe is in the list although nothing here writes a
/// single-quoted attribute: this is the one function standing between agent
/// text and a document, and a narrower one would have to be widened by whoever
/// next writes an attribute.
pub fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

pub fn render(report: &RunReport) -> String {
    // Built with `push_str` rather than one big `format!`, so every escaped
    // value is visibly escaped at its own call site.
    let mut out = String::new();
    out.push_str("<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\">");
    out.push_str("<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">");
    out.push_str("<title>");
    out.push_str(&escape(report.title));
    out.push_str("</title><style>");
    out.push_str(&style());
    out.push_str("</style></head><body>");
    out.push_str("<h1>");
    out.push_str(&escape(report.title));
    out.push_str("</h1><p class=\"meta\">");
    out.push_str(&escape(report.project));
    out.push_str(" &middot; ");
    out.push_str(&escape(report.scope));
    out.push_str(" &middot; finished ");
    out.push_str(&escape(report.finished));
    out.push_str("</p>");

    match report.tasks {
        // Not a zero, and not a silence either: the reason the lists are
        // missing is itself the thing worth saying.
        None => out.push_str(
            "<p class=\"unknown\">The board could not be read, so what moved on it is unknown. \
             Nothing below is a count of zero.</p>",
        ),
        Some(tasks) => {
            section(&mut out, "Closed", &tasks.closed, report.batches);
            section(&mut out, "Parked", &tasks.parked, report.batches);
        }
    }

    if !report.batches.is_empty() {
        out.push_str("<h2>Batches</h2>");
        for batch in report.batches {
            out.push_str("<h3>Batch ");
            out.push_str(&batch.n.to_string());
            out.push_str(" <span class=\"meta\">");
            out.push_str(&human(batch.seconds));
            out.push_str("</span></h3>");
            if !batch.reported {
                out.push_str("<p class=\"unknown\">This batch left no account of itself.</p>");
                continue;
            }
            match &batch.notes {
                Some(notes) => {
                    out.push_str("<p>");
                    out.push_str(&escape(notes));
                    out.push_str("</p>");
                }
                None => out.push_str("<p class=\"meta\">No notes on the batch as a whole.</p>"),
            }
        }
    }

    out.push_str("<p class=\"total\">Total ");
    out.push_str(&human(report.seconds));
    out.push_str("</p></body></html>");
    out
}

/// One list of tasks, each with whatever its batch said it did and — where the
/// batch held it alone — how long that batch took.
fn section(out: &mut String, title: &str, lines: &[TaskLine], batches: &[BatchLine]) {
    out.push_str("<h2>");
    out.push_str(title);
    out.push_str(" (");
    out.push_str(&lines.len().to_string());
    out.push_str(")</h2>");
    if lines.is_empty() {
        out.push_str("<p class=\"meta\">None.</p>");
        return;
    }
    out.push_str("<table>");
    for line in lines {
        // One rule answers the whole row — both what was done and how long it
        // took — because `task_duration` is `last_naming` too. Two *rules* is
        // what let the columns name different batches, and a duration belonging
        // to a different piece of work is the same lie as an account belonging
        // to one.
        let batch = last_naming(batches, &line.id);
        out.push_str("<tr><td class=\"id\">");
        out.push_str(&escape(&line.id));
        out.push_str("</td><td>");
        out.push_str(&escape(&line.title));
        out.push_str("</td><td>");
        let did = batch
            .and_then(|b| b.tasks.iter().find(|t| t.id == line.id))
            .and_then(|t| t.did.as_deref());
        match did {
            Some(text) => out.push_str(&escape(text)),
            // A dash rather than an empty cell: the batch either said nothing
            // about this task or left no account at all, and both are already
            // named under Batches.
            None => out.push_str("&mdash;"),
        }
        out.push_str("</td><td class=\"meta\">");
        // The public rule rather than `duration_of` on the batch above, so the
        // number drawn here is the one the tests exercise. It resolves to the
        // very same batch — both go through `last_naming` — so this cannot
        // disagree with the account beside it.
        match task_duration(batches, &line.id) {
            Some(seconds) => out.push_str(&human(seconds)),
            None => out.push_str("&mdash;"),
        }
        out.push_str("</td></tr>");
    }
    out.push_str("</table>");
}

/// The document's palette, light — and the complete one, because it stands on a
/// bare `:root` and every dark block below only redefines these names. A colour
/// whose sole definition sat inside a media query or an attribute block would
/// simply be absent for the reader that block does not match, and nothing on
/// screen would say a colour had gone missing.
///
/// The values are exactly the ones this file has always drawn. The *names* are
/// the document's own and deliberately not the app's token names: these are not
/// the values `tokens/color-surfaces.css` holds, so calling this `#fff`
/// `--canvas` would read as a copy of a token the app paints differently and
/// drift from it in silence. See the module comment for why literals live here
/// and nowhere else in this repository.
///
/// `color-scheme` is the one declaration here that is not a colour, and it earns
/// its place: the frame's scrollbar and every default the user agent paints —
/// selection among them — follow it rather than the rules below, so without it a
/// dark report comes with a light scrollbar down its side.
const LIGHT: &str = "color-scheme:light;\
--doc-fg:#1a1a1a;--doc-bg:#fff;--doc-meta:#666;--doc-rule:#eee;--doc-rule-strong:#ddd";

/// The same names again, dark: the values that used to sit under
/// `prefers-color-scheme:dark` and nowhere else. Written once and placed twice,
/// because the document has two readers that ask differently — see `style`.
const DARK: &str = "color-scheme:dark;\
--doc-fg:#e6e6e6;--doc-bg:#141414;--doc-meta:#999;--doc-rule:#2a2a2a;--doc-rule-strong:#2a2a2a";

/// What the document actually draws, through the names above and no colour of its
/// own. The type, the spacing and the mono stack are character for character what
/// this file wrote before: the palette moved into custom properties and nothing
/// was restyled.
const RULES: &str = "\
body{font:14px/1.5 -apple-system,BlinkMacSystemFont,Segoe UI,Helvetica,Arial,sans-serif;\
max-width:52rem;margin:2rem auto;padding:0 1rem;color:var(--doc-fg);background:var(--doc-bg)}\
h1{font-size:1.5rem;margin:0 0 .25rem}\
h2{font-size:1.05rem;margin:2rem 0 .5rem}\
h3{font-size:.95rem;font-weight:600;margin:1.25rem 0 .25rem}\
.meta{color:var(--doc-meta);font-size:.85rem;font-weight:400}\
.unknown{color:var(--doc-meta);font-style:italic}\
.total{margin-top:2rem;border-top:1px solid var(--doc-rule-strong);padding-top:.75rem;\
font-weight:600}\
table{border-collapse:collapse;width:100%}\
td{border-top:1px solid var(--doc-rule);padding:.4rem .5rem;vertical-align:top}\
.id{font-family:ui-monospace,SFMono-Regular,Menlo,monospace;white-space:nowrap}";

/// The document's whole appearance.
///
/// **The palette is placed three times because the document has two readers that
/// ask differently.** Opened in a browser, with nothing of ours loaded, it has
/// only `prefers-color-scheme` to go on, and that is left exactly as it was.
/// Opened in a tab of this app it is handed `data-theme` by
/// `src/components/run/reportTheme.js` — the app has already resolved `system`
/// and knows which of the two it is painting — and the attribute has to win in
/// *both* directions.
///
/// Winning is said twice, and the hard direction is the one that needs it said
/// at all: a light app on a dark machine, the mirror of the bug this fixed, where
/// the machine's answer has to be refused rather than merely followed. The media
/// query is guarded with `:not([data-theme="light"])`, so that reader is never
/// told dark in the first place; and the attribute blocks are written after it,
/// so at the equal specificity these selectors have, source order settles it.
/// Either would carry the case alone. Both are here because they fail differently
/// — the guard says which reader the query is for and survives being moved, and
/// the order says nothing and does not.
///
/// The frame cannot be reached from our side to have an attribute set on it —
/// `sandbox=""` gives the document its own origin — which is why the attribute
/// goes into the string the frame is built from and not onto an element. Nothing
/// rewrites what a past run wrote: a document already on disk carries this
/// stylesheet only if it was written after this change.
fn style() -> String {
    format!(
        ":root{{{LIGHT}}}\
         @media(prefers-color-scheme:dark){{:root:not([data-theme=\"light\"]){{{DARK}}}}}\
         :root[data-theme=\"dark\"]{{{DARK}}}\
         :root[data-theme=\"light\"]{{{LIGHT}}}\
         {RULES}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runs::summary::{TaskLine, Tasks};

    fn line(id: &str) -> TaskLine {
        TaskLine { id: id.into(), title: format!("{id} title") }
    }

    fn batch(n: u32) -> BatchLine {
        BatchLine { n, seconds: 600, tasks: vec![], notes: None, reported: true }
    }

    fn report<'a>(seconds: u64, tasks: Option<&'a Tasks>, batches: &'a [BatchLine]) -> RunReport<'a> {
        RunReport {
            title: "Run report",
            project: "/p",
            scope: "the queue",
            finished: "2026-08-12 14:31",
            seconds,
            tasks,
            batches,
        }
    }

    #[test]
    fn the_document_is_named_for_what_it_covers() {
        // A run is one task in Solo, one batch in Crew and a night of batches
        // in Autopilot, and somebody opening the document should read which of
        // those they are looking at. The words are `RunMode::report_title`'s and
        // this file only places them — in the tab's title as well as the
        // heading, since a report opened beside another one is told apart by the
        // tab first.
        let html = render(&RunReport { title: "Task report", ..report(90, None, &[]) });

        assert!(html.contains("<title>Task report</title>"), "{html}");
        assert!(html.contains("<h1>Task report</h1>"), "{html}");
        assert!(!html.contains("Run report"), "and nothing is left calling it the other thing");
    }

    #[test]
    fn agent_text_is_escaped_before_it_reaches_the_document() {
        let mut batch = batch(1);
        batch.tasks = vec![BatchTask {
            id: "a-1".into(),
            did: Some("fixed <script>alert(1)</script> & moved on".into()),
        }];
        let tasks = Tasks { closed: vec![line("a-1")], parked: vec![] };
        let html = render(&report(8040, Some(&tasks), &[batch]));
        assert!(!html.contains("<script"), "an agent wrote this text and a person opens it");
        assert!(html.contains("&lt;script&gt;"));
        assert!(html.contains("&amp; moved on"), "the ampersand is escaped once, not twice");
    }

    #[test]
    fn a_report_with_no_diff_says_so_rather_than_showing_zero() {
        let html = render(&report(120, None, &[]));
        assert!(
            html.contains("could not be read"),
            "an unreadable board and an empty board are opposite facts"
        );
        assert!(!html.contains(">0<"), "no confident zero over a number nobody measured");
        assert!(!html.contains("Closed (0)"), "and no empty section standing in for one");
    }

    #[test]
    fn a_batch_that_left_no_file_is_named_rather_than_drawn_empty() {
        let mut quiet = batch(2);
        quiet.reported = false;
        let tasks = Tasks { closed: vec![line("a-1")], parked: vec![] };
        let html = render(&report(600, Some(&tasks), &[quiet]));
        assert!(html.contains("left no account of itself"));
        assert!(html.contains("a-1"), "and the board still says which task moved");
    }

    #[test]
    fn the_document_carries_its_own_styles_and_reaches_no_network() {
        let tasks = Tasks { closed: vec![line("a-1")], parked: vec![line("a-2")] };
        let html = render(&report(60, Some(&tasks), &[batch(1)]));
        assert!(html.contains("<style>"), "self-contained: it opens with no app around it");
        assert!(!html.contains("http://") && !html.contains("https://"));
        assert!(!html.contains("<script"), "no script, in the document or out of it");
        assert!(!html.contains("<img"), "and no image");
    }

    /// Everything between a selector's `{` and the `}` that closes it. No palette
    /// block in this stylesheet holds a nested rule, so the first `}` is the
    /// right one and a parser is not owed here.
    fn block<'a>(html: &'a str, selector: &str) -> &'a str {
        let at = html.find(selector).unwrap_or_else(|| panic!("no {selector} in the document"));
        let open = at + selector.len();
        let close = html[open..].find('}').expect("an unclosed block");
        &html[open..open + close]
    }

    /// The custom properties a block defines, by name. `color-scheme` and any
    /// other ordinary declaration is not a name a palette has to match on.
    fn names(block: &str) -> Vec<&str> {
        block
            .split(';')
            .filter_map(|declaration| declaration.split(':').next())
            .map(str::trim)
            .filter(|name| name.starts_with("--"))
            .collect()
    }

    #[test]
    fn the_document_carries_both_the_media_branch_and_rules_by_attribute() {
        // The two readers of this document ask differently. In a browser, with
        // nothing of ours loaded, it has only `prefers-color-scheme` to go on;
        // in a tab of this app it is handed `data-theme` by `reportTheme.js`,
        // because an empty sandbox puts the frame's own DOM out of reach and the
        // string is the only thing our side can compose.
        let html = render(&report(90, None, &[]));

        assert!(html.contains(":root{"), "the light palette stands on a bare root");
        assert!(
            html.contains("@media(prefers-color-scheme:dark){"),
            "the browser's reading of the document is untouched: {html}"
        );
        assert!(html.contains(":root[data-theme=\"dark\"]{"), "{html}");
        assert!(html.contains(":root[data-theme=\"light\"]{"), "{html}");
    }

    #[test]
    fn the_attribute_beats_the_machine_in_both_directions() {
        // A dark app on a light machine is the easy half — the machine says
        // nothing and the attribute block is the only dark one that applies. The
        // mirror is what needs two mechanisms: a light app on a dark machine is
        // told dark by the media query unless something stops it, so the query
        // is guarded against the attribute *and* the attribute blocks are written
        // after it, where equal specificity is settled by source order.
        let html = render(&report(90, None, &[]));

        assert!(
            html.contains("@media(prefers-color-scheme:dark){:root:not([data-theme=\"light\"]){"),
            "the machine's answer applies only where the app has not said light: {html}"
        );
        let media = html.find("@media(prefers-color-scheme:dark)").expect("the media query");
        for selector in [":root[data-theme=\"dark\"]{", ":root[data-theme=\"light\"]{"] {
            let at = html.find(selector).expect(selector);
            assert!(at > media, "{selector} has to come after the media query, or it loses to it");
        }
    }

    #[test]
    fn no_colour_is_defined_only_where_one_of_the_readers_would_never_look() {
        // A value whose only definition sits inside a media query or an attribute
        // block is simply absent for the reader that block does not apply to — a
        // light machine with no attribute would draw the document with half its
        // palette missing, and nothing on screen would say so. So the bare root
        // carries the complete light palette and every dark block only redefines.
        let html = render(&report(90, None, &[]));
        let light = names(block(&html, ":root{"));
        assert!(!light.is_empty(), "the light palette is the one that must be complete");

        // Both dark placements, and the media one is the point: it is itself a
        // block one reader never looks in — a document opened in a browser on a
        // light machine — and the two agreeing today is only the consequence of
        // one constant being interpolated twice. That is the accident this test
        // is insurance against, so checking the attribute block alone would leave
        // it insuring nothing.
        for selector in [":root[data-theme=\"dark\"]{", ":root:not([data-theme=\"light\"]){"] {
            let dark = names(block(&html, selector));
            assert!(!dark.is_empty(), "{selector} has to redefine something");
            for name in dark {
                assert!(light.contains(&name), "{name} is defined only under {selector}");
            }
        }
    }

    #[test]
    fn the_palette_is_the_only_thing_the_rules_name_a_colour_through() {
        // The colours are written in one place, and after this change that place
        // is the two palette constants: a literal left behind in a rule would be
        // a colour no attribute and no media query could move, which is exactly
        // the bug this fixed.
        let html = render(&report(90, None, &[]));
        let rules = &html[html.find("body{font:").expect("the body rule")..];
        let rules = &rules[..rules.find("</style>").expect("the end of the stylesheet")];
        // A hex is the shape this file has always written, and the other two are
        // the shapes a later hand would reach for; a rule naming any of them would
        // hold a colour no attribute and no media query could move.
        for form in ["#", "rgb(", "hsl("] {
            assert!(!rules.contains(form), "a colour outside the palette blocks: {rules}");
        }
    }

    #[test]
    fn the_total_is_the_runs_own_duration_and_stands_at_the_end() {
        let tasks = Tasks::default();
        let html = render(&report(8040, Some(&tasks), &[]));
        assert!(html.contains("Total 2h 14m"));
        assert!(html.trim_end().ends_with("</body></html>"));
        let total = html.find("Total 2h 14m").expect("the total");
        assert!(
            html.find("Closed (0)").expect("the closed section") < total,
            "the total is the last thing said"
        );
    }

    #[test]
    fn a_batch_holding_one_task_gives_that_task_its_own_duration() {
        let mut only = batch(1);
        only.seconds = 3600;
        only.tasks = vec![BatchTask { id: "a-1".into(), did: None }];
        assert_eq!(task_duration(&[only], "a-1"), Some(3600));
    }

    #[test]
    fn a_batch_holding_several_tasks_gives_none_of_them_a_duration() {
        let mut shared = batch(1);
        shared.tasks = vec![
            BatchTask { id: "a-1".into(), did: None },
            BatchTask { id: "a-2".into(), did: None },
        ];
        assert_eq!(
            task_duration(&[shared], "a-1"),
            None,
            "dividing a batch by its task count is a number that looks measured"
        );
    }

    #[test]
    fn a_task_alone_in_its_batch_shows_a_time_and_one_sharing_a_batch_shows_none() {
        let mut alone = batch(1);
        alone.seconds = 3600;
        alone.tasks = vec![BatchTask { id: "a-1".into(), did: None }];
        let mut shared = batch(2);
        shared.seconds = 7260;
        shared.tasks = vec![
            BatchTask { id: "a-2".into(), did: None },
            BatchTask { id: "a-3".into(), did: None },
        ];
        let tasks = Tasks { closed: vec![line("a-1"), line("a-2")], parked: vec![] };
        let html = render(&report(11000, Some(&tasks), &[alone, shared]));
        // The rows themselves, not the whole document: every batch prints its
        // own duration under Batches, which says nothing about a task.
        let start = html.find("<table>").expect("the closed table");
        let end = html.find("</table>").expect("the closed table");
        let rows = &html[start..end];
        assert!(rows.contains("1h 0m"), "the batch that held one task lends it its own number");
        assert!(!rows.contains("2h 1m"), "and the batch that held two lends nothing");
    }

    #[test]
    fn a_task_touched_twice_shows_what_the_last_batch_said_about_it() {
        // Phase R recovers an orphan and parks it; a later batch takes it and
        // carries it through. The row is filed under the status it ended in, so
        // the account beside it has to be the one that matches — the first
        // would read "parked because X" under Closed, which is exactly the lie
        // `a_task_parked_then_closed_is_counted_once_by_where_it_ended`
        // forbids one column over.
        let mut first = batch(1);
        first.seconds = 3600;
        first.tasks =
            vec![BatchTask { id: "a-1".into(), did: Some("parked: the spec is unclear".into()) }];
        let mut second = batch(2);
        second.seconds = 7260;
        second.tasks =
            vec![BatchTask { id: "a-1".into(), did: Some("reviewed and merged".into()) }];

        let tasks = Tasks { closed: vec![line("a-1")], parked: vec![] };
        let html = render(&report(11000, Some(&tasks), &[first, second]));
        assert!(html.contains("reviewed and merged"), "{html}");
        assert!(!html.contains("the spec is unclear"), "an earlier account under a later status");

        assert_eq!(
            task_duration(&[batch_naming(1, 3600, &["a-1"]), batch_naming(2, 7260, &["a-1"])], "a-1"),
            Some(7260),
            "and the time beside it is that same batch's"
        );
    }

    #[test]
    fn a_task_handed_on_to_a_shared_batch_shows_no_duration_at_all() {
        // The other order, and the one two separate searches got wrong: batch 1
        // held `a-1` alone, batch 2 took it on beside `a-2`. The row carries
        // batch 2's account, so the rule has to be asked of batch 2 — which
        // held two tasks and therefore lends none of them a number. Borrowing
        // batch 1's hour would be a figure that looks measured and belongs to
        // different work, which is why dividing a batch by its task count was
        // refused in the first place.
        let alone = batch_naming(1, 3600, &["a-1"]);
        let shared = batch_naming(2, 7260, &["a-1", "a-2"]);
        assert_eq!(task_duration(&[alone.clone(), shared.clone()], "a-1"), None);

        let tasks = Tasks { closed: vec![line("a-1")], parked: vec![] };
        let html = render(&report(11000, Some(&tasks), &[alone, shared]));
        let start = html.find("<table>").expect("the closed table");
        let end = html.find("</table>").expect("the closed table");
        let rows = &html[start..end];
        assert!(!rows.contains("1h 0m"), "an hour measured against other work: {rows}");
        assert!(!rows.contains("2h 1m"), "and the shared batch lends nothing either: {rows}");
    }

    #[test]
    fn a_task_named_only_by_a_shared_batch_and_later_alone_takes_the_solo_time() {
        // And the mirror of it, so the rule reads as "ask the batch that owns
        // the row" rather than as "a shared batch anywhere suppresses a time".
        let shared = batch_naming(1, 7260, &["a-1", "a-2"]);
        let alone = batch_naming(2, 3600, &["a-1"]);
        assert_eq!(task_duration(&[shared, alone], "a-1"), Some(3600));
    }

    /// One batch holding the named tasks, which is the shape every lookup in
    /// `section` is about. Several ids make it a shared batch, one makes it a
    /// solo one — the distinction the duration rule turns on.
    fn batch_naming(n: u32, seconds: u64, ids: &[&str]) -> BatchLine {
        BatchLine {
            n,
            seconds,
            tasks: ids.iter().map(|id| BatchTask { id: (*id).into(), did: None }).collect(),
            notes: None,
            reported: true,
        }
    }

    #[test]
    fn a_damaged_batch_file_reads_as_no_account_rather_than_failing() {
        assert_eq!(parse_batch("{ not json").tasks.len(), 0);
        assert!(!parse_batch("{ not json").reported_ok);
    }

    #[test]
    fn a_batch_file_in_the_shape_the_prompt_asks_for_is_read_whole() {
        let parsed = parse_batch(
            r#"{"tasks":[{"id":"a-1","did":"reviewed and merged"}],"notes":"nothing odd"}"#,
        );
        assert!(parsed.reported_ok);
        assert_eq!(parsed.tasks[0].id, "a-1");
        assert_eq!(parsed.tasks[0].did.as_deref(), Some("reviewed and merged"));
        assert_eq!(parsed.notes.as_deref(), Some("nothing odd"));
    }

    #[test]
    fn a_batch_file_missing_its_optional_halves_is_still_an_account() {
        // `notes` is optional and so is `did`: a batch that says only which
        // ids it touched has still reported, and reading that as damage would
        // print "left no account of itself" over a file somebody wrote.
        let parsed = parse_batch(r#"{"tasks":[{"id":"a-1"}]}"#);
        assert!(parsed.reported_ok);
        assert!(parsed.tasks[0].did.is_none());
        assert!(parsed.notes.is_none());
    }

    #[test]
    fn duration_reads_in_hours_minutes_and_seconds() {
        assert_eq!(human(8040), "2h 14m");
        assert_eq!(human(840), "14m");
        assert_eq!(human(48), "48s");
        assert_eq!(human(0), "0s");
    }
}
