//! The run's document: a self-contained HTML file under `.smetana/reports/`.
//!
//! Pure — values in, a string out — with the disk left to `service.rs`, the
//! same split `attachments/cleanup.rs` keeps.
//!
//! **The styling exception.** Everywhere else in this repository a visual value
//! is a `var(--token)` reference and never a literal, and this file is the
//! third documented exception beside `files/editor/theme.js` and
//! `terminal/theme.js`: there are no tokens around this document at all,
//! because its whole purpose is to be readable in a browser with no app in the
//! picture. The stylesheet is not written here any more — it is the design
//! handoff's own `reports.css`, carried whole as `REPORTS_CSS`, and pinned to `docs/design/reports/reports.css` by a
//! byte-equality test. What replaces the rule is narrower than "anything
//! goes": no external stylesheet, no font off a network, no script, no image.
//! The document reaches nowhere at all.
//!
//! The other rule with no exception to it: `did` is text an agent wrote, going
//! into a document a person opens, so every value that reaches the output goes
//! through `escape` at its own call site.

use serde::Deserialize;

use crate::runs::model::StopReason;
use crate::runs::queue::Leftover;
use crate::runs::summary::{TaskLine, Tasks};

/// One task as the batch's own file describes it. `did` is free text an agent
/// wrote, and it is escaped on the way into the document without exception.
#[derive(Debug, Clone, Deserialize)]
pub struct BatchTask {
    pub id: String,
    #[serde(default)]
    pub did: Option<String>,
}

/// How a batch ended **as the run itself saw it** — the half of the record that
/// does not depend on an agent having written anything, and the half that was
/// missing until smetana-pmj.
///
/// The vocabulary is not this file's invention and deliberately adds nothing to
/// what the loop already holds: `service::Batch` and `terminal::model::Exit`,
/// read out. `service::outcome_of` is the one place the two are translated, and
/// the split a person wants first is the one between "it fell over on its own"
/// and "the run ended it", which no code path said out loud before.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BatchOutcome {
    /// `Exit::Code(0)`: the session's process ended of its own accord, well.
    Exited,
    /// `Exit::Code(n)`, `n != 0`. The number is carried: a 137 and a 1 send
    /// somebody to different places.
    Failed { code: i32 },
    /// `Exit::NoCode`: the process is gone and no code ever arrived, which in
    /// practice means it was signalled. This is the shape of the night
    /// smetana-pmj was filed about.
    NoCode,
    /// `Exit::Removed`: somebody took the session out of the agents panel while
    /// the run was waiting on it. A person's doing, not a failure.
    Removed,
    /// `Batch::HandedBack`: the lead wrote its account and gave the work back,
    /// in a mode whose session stays alive with a person in it.
    HandedBack,
    /// `Batch::Unanswered`: the session stopped to ask, in a run with nobody in
    /// it to answer, and the run ended the batch. The question travels with it —
    /// it is the whole of what the run knows about why.
    Unanswered { question: String },
}

/// What one batch contributed, from both sides.
///
/// `reported` is false when the batch left no file or left one that could not be
/// read — a batch that was killed, crashed or cancelled. The distinction is
/// drawn rather than smoothed over: an empty row reads as "nothing was done",
/// which is a different claim.
///
/// `outcome` is the other side of the same record and is always known, which is
/// the point of it: a killed agent writes no file by definition, so a document
/// resting on the file alone goes silent in exactly the case somebody opens it
/// for. `left_behind` is filled only for a batch that left no account — see
/// `service::held_by` — because that is the only case where nobody has said what
/// state the board was left in.
///
/// `lock_released` is the other way round and is filled whatever the batch
/// said, because it is not a reading of the board but a thing the app *did* to
/// it (smetana-rxzd), and a write nobody wrote down is a write nobody can
/// question afterwards.
#[derive(Debug, Clone)]
pub struct BatchLine {
    pub n: u32,
    /// An attempt shares `n` with the logical batch it resumes.
    pub attempt: u32,
    pub agent: String,
    pub seconds: u64,
    pub tasks: Vec<BatchTask>,
    pub notes: Option<String>,
    /// The lead's own plain-language account of the batch, asked for in
    /// `agents::prompt` as the batch file's `summary` — two or three sentences
    /// for somebody who does not read code, with no identifiers in it. It is
    /// escaped and never `prose()`d, because it was asked to hold none: there
    /// is nothing here for that rule to mark as a backtick span.
    pub summary: Option<String>,
    pub reported: bool,
    pub outcome: BatchOutcome,
    pub left_behind: Vec<Leftover>,
    pub lock_released: Option<LockRelease>,
}

/// The merge lock this batch's actor was holding when it ended, and which the
/// run took back off it (smetana-rxzd).
///
/// The whole of what the document has to say about a release is who it was
/// taken from and on what evidence, and only one of those is data: the evidence
/// is always the same one — the batch's own process group read as gone — and is
/// a phrase in `released` rather than a field, because a second answer here
/// would mean a second rule in `queue::release`, and there is exactly one.
///
/// Nothing is written to the lock's own issue, deliberately: any note there
/// moves `updated_at` and makes a dead claim look fresh to every lead waiting
/// on `merging`'s staleness rule. So this line is the only record of the
/// release there is.
#[derive(Debug, Clone)]
pub struct LockRelease {
    pub id: String,
    /// `smetana-run-<session-id>`, the batch's own bd actor — the name the
    /// claim was sitting under.
    pub actor: String,
}

/// What `parse_batch` answers: the file's contents, and whether it was readable
/// at all.
pub struct ParsedBatch {
    pub tasks: Vec<BatchTask>,
    pub notes: Option<String>,
    pub summary: Option<String>,
    pub reported_ok: bool,
}

#[derive(Deserialize)]
struct BatchFile {
    #[serde(default)]
    tasks: Vec<BatchTask>,
    #[serde(default)]
    notes: Option<String>,
    /// Two or three plain sentences for the Reports tab's row, asked for in
    /// `agents::prompt`. Optional, like `notes`: an older account has none.
    #[serde(default)]
    summary: Option<String>,
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
    /// Why the run is not running any more — `finish`'s own `StopReason`, the
    /// same value the journal writes as `{reason:?}` and `stopReason.js` turns
    /// into a sentence of its own on the run bar. Not an `Option`: `finish` is
    /// the only writer of this document and it never runs before a run has a
    /// reason, so a placeholder here would only be a state nothing produces.
    pub stop_reason: &'a StopReason,
    pub tasks: Option<&'a Tasks>,
    pub batches: &'a [BatchLine],
    /// Where this run's journal is — the file `journal.rs` wrote a line to at
    /// every decision the loop made (smetana-7di). `None` when the journal
    /// could not be opened at all, and the document then says nothing rather
    /// than naming a path that is not there.
    ///
    /// A path and not a link: this document reaches nowhere — no stylesheet, no
    /// font, no script, no image — which is what makes it safe to hand to a
    /// sandboxed frame, and a `file:` anchor would be the first exception.
    pub journal: Option<&'a str>,
}

/// Malformed is an ordinary outcome, not an error: the batch's tasks still
/// appear from the board, and the document says the batch left no account of
/// itself.
pub fn parse_batch(text: &str) -> ParsedBatch {
    match serde_json::from_str::<BatchFile>(text) {
        Ok(file) => ParsedBatch {
            tasks: file.tasks,
            notes: file.notes,
            summary: file.summary,
            reported_ok: true,
        },
        Err(_) => ParsedBatch { tasks: vec![], notes: None, summary: None, reported_ok: false },
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

/// Several attempts can describe one logical batch after a limit handoff.
/// Reports retain every attempt but never present them as extra work batches.
fn logical_batches(batches: &[BatchLine]) -> usize {
    let mut seen = std::collections::BTreeSet::new();
    seen.extend(batches.iter().map(|batch| batch.n));
    seen.len()
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

/// A duration as the handoff writes it: under a minute is `<1m`, under an hour
/// is `{m}m`, otherwise `{h}h {m}m` (`1h 0m` stays as it is). Minutes are
/// floored, so the number is never an overstatement, and there is no bare count
/// of seconds — nobody reads a night as 8040.
///
/// The result may hold a `<`, so it is escaped where it is written, and
/// `reports::parse_seconds` reads the same shapes back.
pub fn duration(seconds: u64) -> String {
    let (hours, minutes) = (seconds / 3600, (seconds % 3600) / 60);
    if hours > 0 {
        format!("{hours}h {minutes}m")
    } else if minutes > 0 {
        format!("{minutes}m")
    } else {
        "<1m".to_string()
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

/// Agent prose on its way into the document: escaped, and then with each pair of
/// backticks turned into a `<code>` span.
///
/// **The two halves happen in this order and cannot be swapped.** `escape` runs
/// over the raw text; the substitution runs over its output and writes the only
/// tags in the result. Escaping second would turn these tags into
/// `&lt;code&gt;` and the document would show its own markup.
///
/// Backticks and nothing else. A shape heuristic — paths, dotted symbols, hex
/// shas — was the alternative, and it was refused because its false positives
/// land in ordinary prose and cannot be reviewed: a rule nobody can predict is a
/// rule nobody can check. The lead is asked for the backticks instead, in
/// `agents/prompt.rs` and in the `running-tasks` skill.
///
/// An unpaired trailing backtick stays a backtick. Text somebody wrote is never
/// rewritten beyond escaping and this one substitution, and guessing where a
/// half-open span ends would be exactly that.
pub fn prose(text: &str) -> String {
    let escaped = escape(text);
    let mut out = String::with_capacity(escaped.len());
    let mut rest = escaped.as_str();
    loop {
        let Some(open) = rest.find('`') else {
            out.push_str(rest);
            return out;
        };
        // The closing backtick is searched for past the opening one, so an
        // adjacent pair (an empty span) still closes rather than reading the
        // same character twice.
        let Some(close) = rest[open + 1..].find('`') else {
            out.push_str(rest);
            return out;
        };
        out.push_str(&rest[..open]);
        out.push_str("<code>");
        out.push_str(&rest[open + 1..open + 1 + close]);
        out.push_str("</code>");
        rest = &rest[open + 1 + close + 1..];
    }
}

/// The run report's stylesheet, exactly as the design handoff ships it. The
/// handoff's code-review half is unused by the app: a branch review's agent
/// writes its own page.
///
/// **One file, and it is never edited here.** `docs/design/reports/reports.css`
/// is the handoff's own copy and this is the same bytes; the test
/// `the_stylesheet_is_the_handoffs_own_bytes` is what keeps the two from
/// drifting. A change to the look is made in the design source first and
/// carried over whole.
///
/// The file carries the full token set itself, in the four-block shape the
/// app's tab relies on (a bare `:root`, `prefers-color-scheme` guarded with
/// `:not([data-theme="light"])`, then `[data-theme]` both ways), so there is
/// nothing for this module to add around it.
pub const REPORTS_CSS: &str = include_str!("reports.css");

/// The three tones of a status line and the one neutral. A tone is a class and
/// a glyph together, never colour alone — the design's own rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tone {
    Done,
    NeedsYou,
    Failed,
    Neutral,
}

impl Tone {
    fn class(self) -> &'static str {
        match self {
            Tone::Done => "tone-done",
            Tone::NeedsYou => "tone-needs-you",
            Tone::Failed => "tone-failed",
            Tone::Neutral => "tone-neutral",
        }
    }

    fn glyph(self) -> &'static str {
        match self {
            Tone::Done => "\u{25CF}",
            Tone::NeedsYou => "\u{25B2}",
            Tone::Failed => "\u{25A0}",
            Tone::Neutral => "\u{25CB}",
        }
    }
}

/// How a stop reason is drawn. Only a clean finish is `done`, a crash is
/// `failed`, and everything else — a person pressing stop, a limit, an
/// unreadable board, and any reason added later — is neutral, so a new variant
/// is never louder than anybody decided it should be.
fn stop_tone(reason: &StopReason) -> Tone {
    match reason {
        StopReason::QueueEmpty | StopReason::BatchDone => Tone::Done,
        StopReason::Crashed { .. } => Tone::Failed,
        StopReason::Cancelled => Tone::Neutral,
        _ => Tone::Neutral,
    }
}

pub fn render(report: &RunReport) -> String {
    // Built with `push_str` rather than one big `format!`, so every escaped
    // value is visibly escaped at its own call site.
    let mut out = String::new();
    out.push_str("<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n");
    out.push_str("<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n");
    // `Run report · {title} · smetana` is the handoff's own shape, and
    // `reports::parse_title` strips it back off again.
    out.push_str("<title>Run report \u{00B7} ");
    out.push_str(&escape(report.title));
    out.push_str(" \u{00B7} smetana</title>\n<style>\n");
    out.push_str(REPORTS_CSS);
    out.push_str("</style>\n</head>\n<body>\n<main class=\"page\">\n");

    // Header. The heading's words are `RunMode::report_title`'s — this file
    // places them and owns none of them.
    out.push_str("<header class=\"head\">\n<div class=\"eyebrow\">smetana \u{00B7} run report</div>\n<h1>");
    out.push_str(&escape(report.title));
    out.push_str("</h1>\n<p class=\"meta\">");
    out.push_str(&escape(report.project));
    out.push_str(" \u{00B7} ");
    out.push_str(&escape(report.scope));
    out.push_str(" \u{00B7} finished ");
    out.push_str(&escape(report.finished));
    out.push_str("</p>\n</header>\n");

    strip(&mut out, report);

    // How the run itself ended, unconditionally: a run that stopped before its
    // first batch still owes a person the reason, and it is written whether or
    // not the board below can say anything at all.
    status_line(
        &mut out,
        "stop",
        stop_tone(report.stop_reason),
        &stop_reason_line(report.stop_reason),
    );

    // The lead's own plain-language account of the run, one paragraph per
    // batch that gave one, in batch order. It comes from the batches rather
    // than the board, so an unread board does not silence it. No section over
    // nothing: `reports::parse_summary` falls back to the closed titles.
    //
    // Filtered on content rather than on `Option`: the prompt asks for the
    // key by name, so `"summary": ""` is a shape a lead can plausibly write,
    // and `Some("")` must not count as "gave one".
    let summaries: Vec<&str> = report
        .batches
        .iter()
        .filter_map(|b| b.summary.as_deref())
        .filter(|s| !s.trim().is_empty())
        .collect();
    if !summaries.is_empty() {
        out.push_str(
            "<section class=\"group\" aria-labelledby=\"summary-h\">\n\
             <div class=\"group-head\"><h2 id=\"summary-h\">Summary</h2></div>\n<div class=\"prose\">\n",
        );
        for text in summaries {
            out.push_str("<p>");
            // Trimmed on the way out, not only on the way to the emptiness
            // test above, so a stray space never sits inside the sentence.
            out.push_str(&escape(text.trim()));
            out.push_str("</p>\n");
        }
        out.push_str("</div>\n</section>\n");
    }

    // An unread board draws neither section: the strip's two `is-none` cells
    // are what say so, and a heading over nothing would read as an empty list.
    if let Some(tasks) = report.tasks {
        task_section(&mut out, Section::Closed, &tasks.closed, report.batches);
        task_section(&mut out, Section::Parked, &tasks.parked, report.batches);
    }

    if !report.batches.is_empty() {
        out.push_str("<section class=\"group\" id=\"batches\" aria-labelledby=\"batches-h\">\n");
        out.push_str("<div class=\"group-head\"><h2 id=\"batches-h\">Batches</h2><span class=\"count\">");
        out.push_str(&logical_batches(report.batches).to_string());
        out.push_str("</span></div>\n");
        for b in report.batches {
            batch_card(&mut out, b);
        }
        out.push_str("</section>\n");
    }

    // Footer. The journal is the one direction the pair cannot go by itself:
    // the journal names the report at its last line, and without this a person
    // holding the document has no way back to the mechanics. A path and not a
    // link — this document reaches nowhere.
    out.push_str("<footer class=\"foot\">\n<dl>\n<dt>Total</dt>\n<dd><span class=\"mono\">");
    out.push_str(&escape(&duration(report.seconds)));
    out.push_str("</span></dd>\n");
    if let Some(path) = report.journal {
        out.push_str("<dt>Journal</dt>\n<dd><span class=\"mono\">");
        out.push_str(&escape(path));
        out.push_str("</span></dd>\n");
    }
    out.push_str("</dl>\n</footer>\n</main>\n</body>\n</html>\n");
    out
}

/// A one-line status — the stop reason or a batch's ending. `text` is already
/// markup (it may carry `<code>` or an entity), so it is wrapped in one `<span>`:
/// the line is a flex row and loose text beside inline markup would be split
/// into separate flex items.
fn status_line(out: &mut String, class: &str, tone: Tone, text: &str) {
    out.push_str("<p class=\"");
    out.push_str(class);
    out.push(' ');
    out.push_str(tone.class());
    out.push_str("\"><span class=\"g\" aria-hidden=\"true\">");
    out.push_str(tone.glyph());
    out.push_str("</span><span>");
    out.push_str(text);
    out.push_str("</span></p>\n");
}

/// The summary strip: four cells, and the one place in the document where an
/// unreadable board could quietly become a zero. It does not: no task data is
/// an `is-none` cell with a dash.
///
/// A cell with a target is an `<a>`, and the strip is a `<nav>` only when at
/// least one cell is; otherwise it is a `<div>`, as the handoff says. A count of
/// zero is muted, carries no tone and links nowhere — and the parked cell is the
/// only one that is ever `is-loud`, so the document has at most one.
fn strip(out: &mut String, report: &RunReport) {
    let (closed, parked) = match report.tasks {
        Some(t) => (Some(t.closed.len()), Some(t.parked.len())),
        None => (None, None),
    };
    let has_link = closed.is_some_and(|n| n > 0)
        || parked.is_some_and(|n| n > 0)
        || !report.batches.is_empty();
    let tag = if has_link { "nav" } else { "div" };
    out.push_str("<");
    out.push_str(tag);
    out.push_str(" class=\"strip\" aria-label=\"Run totals\">\n");
    count_cell(out, Some(Tone::Done), "Closed", "Closed", "closed", closed, false);
    count_cell(out, Some(Tone::NeedsYou), "Parked \u{00B7} needs you", "Parked", "parked", parked, true);
    count_cell(out, None, "Batches", "Batches", "batches", Some(logical_batches(report.batches)), false);
    out.push_str("<div class=\"cell\"><span class=\"cell-label\">Total</span><span class=\"cell-n\">");
    out.push_str(&escape(&duration(report.seconds)));
    out.push_str("</span></div>\n</");
    out.push_str(tag);
    out.push_str(">\n");
}

/// One counted cell. `tone` is `None` for the batches cell, which has a link and
/// no hue. `label` is what a non-zero count says and `zero_label` what a zero or
/// missing one does (`Parked`, without "needs you" — nobody is being asked
/// anything).
fn count_cell(
    out: &mut String,
    tone: Option<Tone>,
    label: &str,
    zero_label: &str,
    target: &str,
    count: Option<usize>,
    loud: bool,
) {
    let glyph_of = |t: Tone| {
        format!("<span class=\"g\" aria-hidden=\"true\">{}</span>", t.glyph())
    };
    match count {
        None => {
            // No data at all: no tone, no link, and `aria-label` so a screen
            // reader says what the dash means.
            out.push_str("<div class=\"cell is-none\"><span class=\"cell-label\">");
            if let Some(t) = tone {
                out.push_str(&glyph_of(t));
            }
            out.push_str(zero_label);
            out.push_str("</span><span class=\"cell-n\" aria-label=\"no data\">\u{2014}</span></div>\n");
        }
        Some(0) => {
            out.push_str("<div class=\"cell is-zero\"><span class=\"cell-label\">");
            if let Some(t) = tone {
                out.push_str(&glyph_of(t));
            }
            out.push_str(zero_label);
            out.push_str("</span><span class=\"cell-n\">0</span></div>\n");
        }
        Some(n) => {
            out.push_str("<a class=\"cell");
            if let Some(t) = tone {
                out.push(' ');
                out.push_str(t.class());
            }
            if loud {
                out.push_str(" is-loud");
            }
            out.push_str("\" href=\"#");
            out.push_str(target);
            out.push_str("\"><span class=\"cell-label\">");
            if let Some(t) = tone {
                out.push_str(&glyph_of(t));
            }
            out.push_str(label);
            out.push_str("</span><span class=\"cell-n\">");
            out.push_str(&n.to_string());
            out.push_str("</span></a>\n");
        }
    }
}

/// One sentence naming how the run itself ended — Rust's one place for the
/// words, where before this change the whole vocabulary lived only in
/// `stopReason.js`, and a run that failed its preflight shipped a document
/// with nothing in it about why.
///
/// Every variant gets its own sentence, and a test holds both halves of that:
/// none is empty and none repeats another's words. `Preflight`'s `detail` and
/// `NeedsAnswer`'s `question` are text the app or the harness wrote, going into
/// a document a person opens, so both go through `escape` the same as every
/// other borrowed string that reaches this file. The result is markup (it may
/// carry an entity), which is why the caller wraps it in a `<span>`.
fn stop_reason_line(reason: &StopReason) -> String {
    match reason {
        StopReason::QueueEmpty => {
            "The run ended with nothing ready and nothing left unfinished.".into()
        }
        StopReason::BatchDone => "The run took its one batch and finished it.".into(),
        StopReason::NoProgress => "The run stopped: a whole batch ran and moved neither what \
                                    was ready nor what was unfinished."
            .into(),
        StopReason::MaxIterations => {
            "The run stopped after reaching its limit on how many batches it may run.".into()
        }
        StopReason::Crashed { attempts } => format!(
            "The run stopped: its session exited non-zero {attempts} time{} in a row.",
            if *attempts == 1 { "" } else { "s" }
        ),
        StopReason::Unreadable => {
            "The run stopped: the tracker could not be read twice running.".into()
        }
        StopReason::Cancelled => {
            "Somebody pressed stop, and the batch already in flight was allowed to finish.".into()
        }
        StopReason::SessionRemoved => "The run stopped mid-batch: its agent session was removed \
                                        from the agents panel."
            .into(),
        // The `&ldquo;`/`&rdquo;` pair matches `outcome`'s own `Unanswered` arm
        // below, so the same ending is quoted the same way wherever it appears
        // in this document.
        StopReason::NeedsAnswer { question } => format!(
            "The run stopped at a question it had nobody to answer: &ldquo;{}&rdquo;",
            escape(question)
        ),
        StopReason::NothingDone { batches } => format!(
            "The run stopped: {} in a row came back having done nothing at all.",
            if *batches == 1 { "one batch".to_string() } else { format!("{batches} batches") }
        ),
        StopReason::Preflight { detail } => {
            format!("The run could not start: {}.", escape(detail))
        }
    }
}

/// What the run saw end the batch, as the tone and the markup of one line, under
/// every batch card and whether or not the agent left a file.
///
/// The five kinds are the handoff's own and their words are its own. `Unanswered`
/// is the one ending the handoff has no kind for — a session that stopped to ask
/// in a run with nobody to answer — and it keeps the sentence this document
/// already had for it, in the `needs-you` tone: a person has to come back to it,
/// the same as to work handed back.
///
/// The line says what the run *saw*, and that is the honest form: this is one
/// party's account of a process it was watching from outside, and why the
/// process went is beyond it.
fn outcome_line(outcome: &BatchOutcome) -> (Tone, String) {
    match outcome {
        BatchOutcome::Exited => (Tone::Done, "The run saw its session exit cleanly.".into()),
        BatchOutcome::Failed { code } => {
            (Tone::Failed, format!("The session crashed with exit code <code>{code}</code>."))
        }
        BatchOutcome::NoCode => {
            (Tone::Failed, "The session process disappeared without an exit code.".into())
        }
        BatchOutcome::Removed => (Tone::Neutral, "A person removed the session.".into()),
        BatchOutcome::HandedBack => {
            (Tone::NeedsYou, "The lead handed the work back to a person.".into())
        }
        // The question is a harness's own text reaching a document a person
        // opens, so it goes through `escape` like everything else. `prose` is
        // not used: nobody was asked to type backticks into a dialog.
        BatchOutcome::Unanswered { question } => (
            Tone::NeedsYou,
            format!(
                "The run ended this batch at a question it had nobody to answer: &ldquo;{}&rdquo;",
                escape(question)
            ),
        ),
    }
}

/// One batch card: its label and time, the agent's own half, what the board held
/// and what the run released, and last the run's own ending.
fn batch_card(out: &mut String, b: &BatchLine) {
    out.push_str("<article class=\"batch card\">\n<div class=\"batch-top\">\n<span class=\"batch-label\">batch ");
    out.push_str(&b.n.to_string());
    if b.attempt > 1 {
        out.push_str(" \u{00B7} attempt ");
        out.push_str(&b.attempt.to_string());
    }
    out.push_str(" <span class=\"agent\">(");
    out.push_str(&escape(&b.agent));
    out.push_str(")</span></span>\n<span class=\"dur\">");
    out.push_str(&escape(&duration(b.seconds)));
    out.push_str("</span>\n</div>\n");

    // The agent's own half first, and then the run's, always. The phrase is
    // about the *account* — a killed agent really did write nothing — and it is
    // not the whole record of the batch.
    let notes = b.notes.as_deref().filter(|n| !n.trim().is_empty());
    match (b.reported, notes) {
        (true, Some(notes)) => {
            out.push_str("<p class=\"task-body\">");
            out.push_str(&prose(notes));
            out.push_str("</p>\n");
        }
        (false, _) => {
            out.push_str("<p class=\"no-account\">This batch left no account of itself.</p>\n");
        }
        // A file somebody wrote, with no `notes` in it: saying the batch "left
        // no account" over a file would be false, so this one says what is true.
        (true, None) => {
            out.push_str("<p class=\"no-account\">No notes on the batch as a whole.</p>\n");
        }
    }

    left_and_released(out, &b.left_behind, &b.lock_released);

    let (tone, text) = outcome_line(&b.outcome);
    status_line(out, "outcome", tone, &text);
    out.push_str("</article>\n");
}

/// What this batch's actor was still holding on the board when the batch ended
/// ("Left unfinished"), and the merge lock the run gave back ("Lock released") —
/// one `dl`, drawn only when either has something to say, since a row saying an
/// actor held nothing would stand under every batch of every report.
///
/// **The first is a reading of the board, never the record of an act**: what the
/// board said a moment after the batch ended, filled only for a batch that left
/// no account. The second is the opposite — a **write the app made**, filled
/// whatever the batch said, and the only record of the release there will ever
/// be, because nothing is written to the lock's own issue (a note there would
/// move `updated_at` and make a dead claim look fresh to every lead waiting on
/// it). It names the actor the claim was taken from; the evidence — the batch's
/// own process group read as gone — is the same every time and is the licence
/// for the release rather than a field.
///
/// The design's row is an id and nothing more, so the status each leftover had
/// and whether one was the lock are not drawn; the lock, when the app took it
/// back, is named by the second row.
fn left_and_released(out: &mut String, held: &[Leftover], released: &Option<LockRelease>) {
    if held.is_empty() && released.is_none() {
        return;
    }
    out.push_str("<dl class=\"f-body\">\n");
    if !held.is_empty() {
        out.push_str("<dt>Left unfinished</dt>\n<dd class=\"ids\">");
        for item in held {
            out.push_str("<code>");
            out.push_str(&escape(&item.id));
            out.push_str("</code>");
        }
        out.push_str("</dd>\n");
    }
    if let Some(release) = released {
        out.push_str("<dt>Lock released</dt>\n<dd><code>");
        out.push_str(&escape(&release.id));
        out.push_str("</code> \u{00B7} by <code>");
        out.push_str(&escape(&release.actor));
        out.push_str("</code></dd>\n");
    }
    out.push_str("</dl>\n");
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Section {
    Closed,
    Parked,
}

/// One list of tasks, each with whatever its batch said it did and — where the
/// batch held it alone — how long that batch took. An empty list draws nothing:
/// the strip is what carries the zero.
///
/// Which section a card sits in decides its look, since the board's own status
/// is not carried this far — `TaskLine` is an id and a title. A parked card is
/// told apart by more than its border: a flag with a glyph and the words
/// "Waiting on you".
fn task_section(out: &mut String, which: Section, lines: &[TaskLine], batches: &[BatchLine]) {
    if lines.is_empty() {
        return;
    }
    let (id, name, glyph, glyph_class) = match which {
        Section::Closed => ("closed", "Closed", Tone::Done.glyph(), "c-ok"),
        Section::Parked => ("parked", "Parked", Tone::NeedsYou.glyph(), "c-serious"),
    };
    out.push_str("<section class=\"group\" id=\"");
    out.push_str(id);
    out.push_str("\" aria-labelledby=\"");
    out.push_str(id);
    out.push_str("-h\">\n<div class=\"group-head\">\n<span class=\"g ");
    out.push_str(glyph_class);
    out.push_str("\" aria-hidden=\"true\">");
    out.push_str(glyph);
    out.push_str("</span>\n<h2 id=\"");
    out.push_str(id);
    out.push_str("-h\">");
    out.push_str(name);
    out.push_str("</h2>\n<span class=\"count\">");
    out.push_str(&lines.len().to_string());
    out.push_str("</span>\n");
    if which == Section::Parked {
        out.push_str("<span class=\"hint\">Waiting on you</span>\n");
    }
    out.push_str("</div>\n");
    for line in lines {
        // One rule answers the whole card — both what was done and how long it
        // took — because `task_duration` is `last_naming` too. Two *rules* is
        // what let the columns name different batches.
        let batch = last_naming(batches, &line.id);
        out.push_str("<article class=\"task card");
        if which == Section::Parked {
            out.push_str(" is-parked");
        }
        out.push_str("\" id=\"");
        out.push_str(&escape(&line.id));
        out.push_str("\">\n<div class=\"task-top\">\n<div class=\"task-meta\"><span class=\"task-id\">");
        out.push_str(&escape(&line.id));
        out.push_str("</span>");
        if which == Section::Parked {
            out.push_str(
                "<span class=\"flag tone-needs-you\"><span aria-hidden=\"true\">\u{25B2}</span>Waiting on you</span>",
            );
        }
        out.push_str("</div>\n");
        // The public rule rather than `duration_of` on the batch above, so the
        // number drawn here is the one the tests exercise.
        match task_duration(batches, &line.id) {
            Some(seconds) => {
                out.push_str("<span class=\"dur\">");
                out.push_str(&escape(&duration(seconds)));
                out.push_str("</span>\n");
            }
            None => out.push_str(
                "<span class=\"dur is-none\" aria-label=\"duration unknown\">\u{2014}</span>\n",
            ),
        }
        out.push_str("<h3>");
        out.push_str(&escape(&line.title));
        out.push_str("</h3>\n</div>\n");
        let did = batch
            .and_then(|b| b.tasks.iter().find(|t| t.id == line.id))
            .and_then(|t| t.did.as_deref())
            .filter(|d| !d.trim().is_empty());
        match did {
            Some(text) => {
                out.push_str("<p class=\"task-body\">");
                out.push_str(&prose(text));
                out.push_str("</p>\n");
            }
            // A dash rather than nothing: the batch either said nothing about
            // this task or left no account at all, and both are already named
            // under Batches.
            None => out.push_str("<p class=\"no-account\">\u{2014}</p>\n"),
        }
        out.push_str("</article>\n");
    }
    out.push_str("</section>\n");
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::runs::summary::{TaskLine, Tasks};

    fn line(id: &str) -> TaskLine {
        TaskLine { id: id.into(), title: format!("{id} title") }
    }

    fn batch(n: u32) -> BatchLine {
        BatchLine {
            n,
            attempt: 1,
            agent: "claude".into(),
            seconds: 600,
            tasks: vec![],
            notes: None,
            summary: None,
            reported: true,
            // The ordinary ending, so a test about anything else is not also a
            // test about a crash.
            outcome: BatchOutcome::Exited,
            left_behind: vec![],
            lock_released: None,
        }
    }

    /// The ordinary ending, so a test about anything else is not also a test
    /// about how a run stopped.
    const DEFAULT_REASON: StopReason = StopReason::QueueEmpty;

    fn report<'a>(seconds: u64, tasks: Option<&'a Tasks>, batches: &'a [BatchLine]) -> RunReport<'a> {
        RunReport {
            title: "Run report",
            project: "/p",
            scope: "the queue",
            finished: "2026-08-12 14:31",
            seconds,
            stop_reason: &DEFAULT_REASON,
            tasks,
            batches,
            journal: None,
        }
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
    fn a_batch_that_left_no_file_is_named_rather_than_drawn_empty() {
        let mut quiet = batch(2);
        quiet.reported = false;
        let tasks = Tasks { closed: vec![line("a-1")], parked: vec![] };
        let html = render(&report(600, Some(&tasks), &[quiet]));
        assert!(html.contains("left no account of itself"));
        assert!(html.contains("a-1"), "and the board still says which task moved");
    }

    /// The whole `StopReason` vocabulary, one representative value per
    /// variant — `Crashed`, `NeedsAnswer`, `NothingDone` and `Preflight` all
    /// carry a payload, so each gets a value rather than standing for a shape.
    fn stop_reasons() -> Vec<StopReason> {
        vec![
            StopReason::QueueEmpty,
            StopReason::BatchDone,
            StopReason::NoProgress,
            StopReason::MaxIterations,
            StopReason::Crashed { attempts: 3 },
            StopReason::Unreadable,
            StopReason::Cancelled,
            StopReason::SessionRemoved,
            StopReason::NeedsAnswer { question: "Trust this folder?".into() },
            StopReason::NothingDone { batches: 2 },
            StopReason::Preflight { detail: "npm install exited 127".into() },
        ]
    }

    #[test]
    fn a_run_that_never_reached_a_batch_still_says_why_it_stopped() {
        // smetana-2s3r: a run that fell over in the preflight left a document
        // of four zeros and a shrug — the reason was known, sitting in
        // `journal::ended` and in the run bar, and said nowhere in the one
        // document meant to outlive both. `tasks: None` is the unreadable
        // board this ending always carries, and the sentence is still there.
        let reason = StopReason::Preflight {
            detail: "the session could not be started: <runtime> did not \"init\" & exit".into(),
        };
        let html = render(&RunReport { stop_reason: &reason, ..report(22, None, &[]) });

        assert!(html.contains("could not start"), "{html}");
        // The detail travels word for word, escaped like every other borrowed
        // string that reaches this document.
        assert!(
            html.contains("&lt;runtime&gt; did not &quot;init&quot; &amp; exit"),
            "the detail is carried verbatim, escaped rather than dropped: {html}"
        );
        assert!(!html.contains("<runtime>"), "an unescaped angle bracket would not be a document");
    }

    #[test]
    fn every_stop_reason_gets_its_own_sentence_and_none_is_empty() {
        let mut seen = std::collections::HashSet::new();
        for reason in stop_reasons() {
            let sentence = stop_reason_line(&reason);
            assert!(!sentence.trim().is_empty(), "no ending goes unsaid: {reason:?}");
            assert!(
                seen.insert(sentence.clone()),
                "two endings sharing one sentence read as one ending: {reason:?} -> {sentence}"
            );
        }
    }

    #[test]
    fn the_stop_reason_prints_even_when_the_board_could_not_be_read() {
        // The board's own dashes must never be the whole of what a person
        // reads — see `a_report_with_no_diff_says_so_rather_than_showing_zero`
        // — and neither must an unreadable board silence the one sentence that
        // never depended on it in the first place.
        let reason = StopReason::Unreadable;
        let html = render(&RunReport { stop_reason: &reason, ..report(600, None, &[]) });
        assert!(html.contains("could not be read twice running"), "{html}");
    }

    #[test]
    fn the_question_that_ended_a_batch_is_escaped_like_every_other_borrowed_text() {
        // A harness wrote this and a person opens it. It goes through `escape`
        // rather than `prose`: nobody was asked to type backticks into a dialog.
        let mut killed = batch(1);
        killed.reported = false;
        killed.outcome =
            BatchOutcome::Unanswered { question: "Trust <script>alert(1)</script>?".into() };
        let html = render(&report(600, None, &[killed]));

        assert!(!html.contains("<script"), "{html}");
        assert!(html.contains("Trust &lt;script&gt;"), "{html}");
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
            attempt: 1,
            agent: "claude".into(),
            seconds,
            tasks: ids.iter().map(|id| BatchTask { id: (*id).into(), did: None }).collect(),
            notes: None,
            summary: None,
            reported: true,
            outcome: BatchOutcome::Exited,
            left_behind: vec![],
            lock_released: None,
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
    fn a_batch_file_with_a_summary_parses_it_and_one_without_reads_none() {
        let with = parse_batch(r#"{"tasks":[],"summary":"Two sentences."}"#);
        assert_eq!(with.summary.as_deref(), Some("Two sentences."));
        let without = parse_batch(r#"{"tasks":[]}"#);
        assert_eq!(without.summary, None);
        assert!(without.reported_ok);
    }

    #[test]
    fn a_backtick_span_becomes_code_and_the_escape_runs_first() {
        // The order is load-bearing. `escape` runs over the raw text and the
        // backtick pass runs over the escaped string; the other way round, `escape`
        // would turn the tags this pass just wrote into `&lt;code&gt;`.
        assert_eq!(prose("touched `src/main.rs` twice"), "touched <code>src/main.rs</code> twice");
        assert_eq!(
            prose("wrapped `<Foo>` in a guard"),
            "wrapped <code>&lt;Foo&gt;</code> in a guard",
            "the identifier is escaped inside its own tags"
        );
        assert_eq!(
            prose("ran `a && b` once"),
            "ran <code>a &amp;&amp; b</code> once",
            "the ampersand is escaped once, not twice"
        );
    }

    #[test]
    fn an_unpaired_backtick_is_left_as_it_was_written() {
        // Text an agent wrote is never rewritten beyond escaping and this one
        // substitution: a half-open span is prose with a backtick in it, not a
        // marker to guess the end of.
        assert_eq!(prose("it uses `serde but stopped"), "it uses `serde but stopped");
        assert_eq!(
            prose("`one` and `two` and `three"),
            "<code>one</code> and <code>two</code> and `three",
            "the closed pairs still close"
        );
    }

    #[test]
    fn prose_marks_nothing_that_is_not_between_backticks() {
        // The rule is deterministic and never guesses: a path that arrives without
        // backticks stays prose, because a heuristic nobody can predict is a rule
        // nobody can review.
        assert_eq!(prose("touched src/main.rs twice"), "touched src/main.rs twice");
        assert_eq!(prose("closed smetana-t9o"), "closed smetana-t9o");
    }

    #[test]
    fn agent_prose_with_a_script_in_it_is_still_escaped_inside_a_code_span() {
        // The whole point of `escape` reaching every call site, restated for the
        // one function that writes tags of its own.
        let out = prose("ran `<script>alert(1)</script>` in a page");
        assert!(!out.contains("<script"), "an agent wrote this and a person opens it");
        assert!(out.contains("<code>&lt;script&gt;"), "{out}");
    }

    // -- the stylesheet --------------------------------------------------------

    #[test]
    fn the_stylesheet_is_the_handoffs_own_bytes() {
        // The design source and the copy this crate compiles in are one file in
        // two places, and nothing but this test says so. A drift here is a
        // report drawn in a stylesheet nobody reviewed.
        assert_eq!(
            REPORTS_CSS,
            include_str!("../../../../docs/design/reports/reports.css"),
            "crates/core/src/runs/reports.css must equal docs/design/reports/reports.css"
        );
    }

    /// The markup after the stylesheet: the stylesheet itself names every class
    /// the design has, so a test asking whether a class was *drawn* has to look
    /// past it.
    fn body_of(html: &str) -> &str {
        &html[html.find("<main").expect("the page")..]
    }

    /// The text between the document's one `<style>` and its `</style>`.
    fn style_of(html: &str) -> &str {
        let open = html.find("<style>").expect("a style element") + "<style>".len();
        let close = html[open..].find("</style>").expect("the end of the style element");
        &html[open..open + close]
    }

    #[test]
    fn the_style_element_is_the_shared_stylesheet_byte_for_byte() {
        let html = render(&report(90, None, &[]));
        assert_eq!(
            style_of(&html).trim_matches('\n'),
            REPORTS_CSS.trim_matches('\n'),
            "the whole of reports.css and nothing added or dropped"
        );
        assert_eq!(html.matches("<style>\n").count(), 1, "one style element");
    }

    #[test]
    fn the_document_is_a_whole_page_the_apps_tab_can_stamp_a_theme_on() {
        // `reportTheme.js` returns a rootless document untouched and otherwise
        // writes `data-theme` onto the `<html>` tag, so both have to be there.
        let html = render(&report(90, None, &[]));
        assert!(html.starts_with("<!doctype html>"), "{html}");
        assert!(html.contains("<html lang=\"en\">"), "{html}");
        assert!(html.contains("<main class=\"page\">"), "{html}");
        assert!(html.trim_end().ends_with("</html>"), "{html}");
    }

    #[test]
    fn the_stylesheet_carries_the_four_palette_blocks_in_the_order_the_tab_needs() {
        // Light on a bare `:root`; dark under the media query, guarded against an
        // app that has said light; then the attribute blocks after it, so that at
        // equal specificity source order lets the attribute win in both directions.
        let css = REPORTS_CSS;
        let bare = css.find("\n:root{").expect("a bare :root");
        let media = css.find("@media (prefers-color-scheme: dark){").expect("the media query");
        let guard = css.find(":root:not([data-theme=\"light\"]){").expect("the guard");
        let dark = css.find(":root[data-theme=\"dark\"]{").expect("the dark attribute block");
        let light = css.find(":root[data-theme=\"light\"]{").expect("the light attribute block");
        assert!(bare < media && media < guard, "the guard sits inside the media query");
        assert!(guard < dark && dark < light, "the attribute blocks follow the query");
        assert!(css.contains("color-scheme:light") && css.contains("color-scheme:dark"));
    }

    #[test]
    fn the_document_reaches_no_network() {
        let tasks = Tasks { closed: vec![line("a-1")], parked: vec![line("a-2")] };
        let html = render(&report(60, Some(&tasks), &[batch(1)]));
        assert!(html.contains("<style>"), "self-contained: it opens with no app around it");
        assert!(!html.contains("http://") && !html.contains("https://"));
        assert!(!html.contains("<script"), "no script, in the document or out of it");
        assert!(!html.contains("<img"), "and no image");
        assert!(!html.contains("@import"), "and no stylesheet fetched from anywhere");
        assert!(!html.contains("@font-face"), "and no font file either");
        assert!(!html.contains("url("), "and nothing the stylesheet could fetch");
        assert!(!html.contains("<link"), "and no linked file");
        assert!(!html.contains("fonts.googleapis"));
        assert!(html.contains("ui-monospace"), "the mono stack falls back to what every reader has");
    }

    #[test]
    fn the_document_carries_no_density_and_no_app_wide_scale() {
        // Those are about fitting rows into panels, while a document has a
        // measure of its own. Nothing here may quietly reintroduce either.
        let html = render(&report(90, None, &[]));
        assert!(!html.contains("data-density"), "{html}");
        assert!(!html.contains("--ui-scale"), "{html}");
    }

    // -- the header and the strip ----------------------------------------------

    #[test]
    fn the_document_is_named_for_what_it_covers() {
        // A run is one task in Solo, one batch in Crew and a night of batches in
        // Autopilot, and somebody opening the document should read which of
        // those they are looking at. The words are `RunMode::report_title`'s and
        // this file only places them, in the tab's title and in the heading.
        let html = render(&RunReport { title: "Task report", ..report(90, None, &[]) });

        assert!(html.contains("<title>Run report \u{00B7} Task report \u{00B7} smetana</title>"), "{html}");
        assert!(html.contains("<h1>Task report</h1>"), "{html}");
        assert!(html.contains("<div class=\"eyebrow\">smetana \u{00B7} run report</div>"), "{html}");
    }

    #[test]
    fn the_meta_line_joins_project_scope_and_finish() {
        let html = render(&report(90, None, &[]));
        assert!(
            html.contains("<p class=\"meta\">/p \u{00B7} the queue \u{00B7} finished 2026-08-12 14:31</p>"),
            "{html}"
        );
    }

    /// An ordinary run: five closed, one parked, three batches of which one is a
    /// second attempt, a summary, and a journal.
    fn ordinary_run() -> (Tasks, Vec<BatchLine>) {
        let tasks = Tasks {
            closed: ["a-1", "a-2", "a-3", "a-4", "a-5"].iter().map(|i| line(i)).collect(),
            parked: vec![line("a-6")],
        };
        let mut b1 = batch_naming(1, 1680, &["a-1"]);
        b1.notes = Some("Nothing odd; `bd` was slow.".into());
        b1.summary = Some("Fixed the panel.".into());
        let mut b2 = batch_naming(2, 1140, &["a-6", "a-2"]);
        b2.attempt = 2;
        b2.notes = Some("Took `a-6` and `a-2`.".into());
        b2.outcome = BatchOutcome::HandedBack;
        b2.left_behind =
            vec![Leftover { id: "a-6".into(), status: "in_progress".into(), lock: false }];
        b2.lock_released = Some(LockRelease { id: "a-6".into(), actor: "smetana-run-7".into() });
        let b3 = batch_naming(3, 1320, &["a-3", "a-4", "a-5"]);
        // The first attempt of batch 2 is a line of its own that shares `n`.
        let mut b2_first = batch_naming(2, 60, &[]);
        b2_first.reported = false;
        b2_first.outcome = BatchOutcome::NoCode;
        (tasks, vec![b1, b2_first, b2, b3])
    }

    #[test]
    fn an_ordinary_run_draws_every_part_of_the_design_in_order() {
        let (tasks, batches) = ordinary_run();
        let html = render(&RunReport {
            journal: Some("/p/.smetana/runs/3/journal.log"),
            ..report(4320, Some(&tasks), &batches)
        });

        // The strip: four cells, each with its tone and target.
        assert!(html.contains("<nav class=\"strip\" aria-label=\"Run totals\">"), "{html}");
        assert!(
            html.contains(
                "<a class=\"cell tone-done\" href=\"#closed\"><span class=\"cell-label\"><span class=\"g\" aria-hidden=\"true\">\u{25CF}</span>Closed</span><span class=\"cell-n\">5</span></a>"
            ),
            "{html}"
        );
        assert!(
            html.contains(
                "<a class=\"cell tone-needs-you is-loud\" href=\"#parked\"><span class=\"cell-label\"><span class=\"g\" aria-hidden=\"true\">\u{25B2}</span>Parked \u{00B7} needs you</span><span class=\"cell-n\">1</span></a>"
            ),
            "{html}"
        );
        assert!(
            html.contains("<a class=\"cell\" href=\"#batches\"><span class=\"cell-label\">Batches</span><span class=\"cell-n\">3</span></a>"),
            "{html}"
        );
        assert!(
            html.contains("<div class=\"cell\"><span class=\"cell-label\">Total</span><span class=\"cell-n\">1h 12m</span></div>"),
            "{html}"
        );

        // Then, in the order of the reference: stop line, summary, closed,
        // parked, batches, footer.
        let at = |needle: &str| html.find(needle).unwrap_or_else(|| panic!("missing {needle}: {html}"));
        let order = [
            at("<p class=\"stop tone-done\">"),
            at("<h2 id=\"summary-h\">Summary</h2>"),
            at("<section class=\"group\" id=\"closed\""),
            at("<section class=\"group\" id=\"parked\""),
            at("<section class=\"group\" id=\"batches\""),
            at("<footer class=\"foot\">"),
        ];
        assert!(order.windows(2).all(|w| w[0] < w[1]), "{order:?}");

        // A parked card is told apart by more than a border.
        assert!(html.contains("<article class=\"task card is-parked\" id=\"a-6\">"), "{html}");
        assert!(
            html.contains("<span class=\"flag tone-needs-you\"><span aria-hidden=\"true\">\u{25B2}</span>Waiting on you</span>"),
            "{html}"
        );
        assert!(html.contains("<span class=\"hint\">Waiting on you</span>"), "{html}");
        assert!(html.contains("<article class=\"task card\" id=\"a-1\">"), "{html}");

        // A solo batch lends its time to the task; a shared one does not.
        assert!(
            html.contains("<span class=\"dur\">28m</span>"),
            "a-1 was alone in its batch: {html}"
        );
        assert!(html.contains("<span class=\"dur is-none\" aria-label=\"duration unknown\">\u{2014}</span>"), "{html}");

        // Batch labels: an attempt past the first is named, and the agent is
        // always in parentheses.
        assert!(html.contains("<span class=\"batch-label\">batch 1 <span class=\"agent\">(claude)</span></span>"), "{html}");
        assert!(
            html.contains("<span class=\"batch-label\">batch 2 \u{00B7} attempt 2 <span class=\"agent\">(claude)</span></span>"),
            "{html}"
        );

        // The footer.
        assert!(html.contains("<dt>Total</dt>\n<dd><span class=\"mono\">1h 12m</span></dd>"), "{html}");
        assert!(
            html.contains("<dt>Journal</dt>\n<dd><span class=\"mono\">/p/.smetana/runs/3/journal.log</span></dd>"),
            "{html}"
        );
    }

    #[test]
    fn a_second_attempt_is_not_counted_as_another_batch_in_the_strip_or_the_group() {
        // An attempt shares `n` with the batch it resumes, so the strip and the
        // group head count logical batches and agree — the handoff's own
        // reference shows one number for both.
        let (tasks, batches) = ordinary_run();
        assert_eq!(batches.len(), 4, "four lines, three batches");
        let html = render(&report(4320, Some(&tasks), &batches));
        assert!(html.contains("<span class=\"cell-label\">Batches</span><span class=\"cell-n\">3</span>"), "{html}");
        assert!(html.contains("<h2 id=\"batches-h\">Batches</h2><span class=\"count\">3</span>"), "{html}");
    }

    #[test]
    fn a_run_that_crashed_on_its_first_batch_has_no_task_data_and_says_so() {
        // The handoff's crashed fixture: no summary, no task data. Closed and
        // parked are `is-none` with a dash, carry no tone and link nowhere, and
        // neither section is drawn.
        let mut crashed = batch(1);
        crashed.reported = false;
        crashed.outcome = BatchOutcome::Failed { code: 137 };
        let reason = StopReason::Crashed { attempts: 1 };
        let html = render(&RunReport { stop_reason: &reason, ..report(360, None, &[crashed]) });
        let html = body_of(&html);

        assert!(
            html.contains("<div class=\"cell is-none\"><span class=\"cell-label\"><span class=\"g\" aria-hidden=\"true\">\u{25CF}</span>Closed</span><span class=\"cell-n\" aria-label=\"no data\">\u{2014}</span></div>"),
            "{html}"
        );
        assert!(
            html.contains("<div class=\"cell is-none\"><span class=\"cell-label\"><span class=\"g\" aria-hidden=\"true\">\u{25B2}</span>Parked</span><span class=\"cell-n\" aria-label=\"no data\">\u{2014}</span></div>"),
            "{html}"
        );
        assert!(!html.contains("href=\"#closed\"") && !html.contains("href=\"#parked\""), "{html}");
        assert!(!html.contains("id=\"closed\"") && !html.contains("id=\"parked\""), "{html}");
        assert!(!html.contains("Summary"), "{html}");
        assert!(!html.contains("is-loud"), "no data is never loud: {html}");
        // Nothing turned into a confident zero.
        assert!(!html.contains("is-zero"), "{html}");

        assert!(html.contains("<p class=\"stop tone-failed\"><span class=\"g\" aria-hidden=\"true\">\u{25A0}</span>"), "{html}");
        assert!(html.contains("<p class=\"no-account\">This batch left no account of itself.</p>"), "{html}");
        assert!(
            html.contains("<p class=\"outcome tone-failed\"><span class=\"g\" aria-hidden=\"true\">\u{25A0}</span><span>The session crashed with exit code <code>137</code>.</span></p>"),
            "{html}"
        );
    }

    #[test]
    fn nothing_parked_is_a_zero_cell_with_no_loud_and_no_link() {
        let tasks = Tasks { closed: vec![line("a-1")], parked: vec![] };
        let html = render(&report(120, Some(&tasks), &[batch(1)]));
        let html = body_of(&html);

        assert!(
            html.contains("<div class=\"cell is-zero\"><span class=\"cell-label\"><span class=\"g\" aria-hidden=\"true\">\u{25B2}</span>Parked</span><span class=\"cell-n\">0</span></div>"),
            "the zero cell says Parked, without asking anything of anybody: {html}"
        );
        assert!(!html.contains("is-loud"), "{html}");
        assert!(!html.contains("href=\"#parked\""), "{html}");
        assert!(!html.contains("id=\"parked\""), "an empty section is not drawn at all: {html}");
        assert!(html.contains("id=\"closed\""), "{html}");
    }

    #[test]
    fn a_strip_with_nothing_to_link_to_is_a_div_and_not_a_nav() {
        let tasks = Tasks { closed: vec![], parked: vec![] };
        let html = render(&report(5, Some(&tasks), &[]));
        assert!(html.contains("<div class=\"strip\" aria-label=\"Run totals\">"), "{html}");
        assert!(!html.contains("<nav"), "{html}");
        assert!(html.contains("<span class=\"cell-n\">0</span>"), "{html}");
    }

    #[test]
    fn at_most_one_cell_is_loud_whatever_the_run() {
        // The design's own budget: shouting is for the one thing that needs a
        // person, so a document never has two loud cells, and only the parked
        // cell can be one.
        let (tasks, batches) = ordinary_run();
        let crashed = StopReason::Crashed { attempts: 2 };
        let cases: Vec<String> = vec![
            render(&report(1, Some(&tasks), &batches)),
            render(&report(1, None, &batches)),
            render(&RunReport { stop_reason: &crashed, ..report(1, Some(&tasks), &batches) }),
            render(&report(1, Some(&Tasks { closed: vec![], parked: vec![line("x")] }), &[])),
        ];
        for html in cases {
            // The stylesheet names `.is-loud` too, so count real uses only.
            let body = &html[html.find("<main").unwrap()..];
            assert!(body.matches("is-loud").count() <= 1, "{body}");
        }
        let (tasks, batches) = ordinary_run();
        let html = render(&report(1, Some(&tasks), &batches));
        let body = &html[html.find("<main").unwrap()..];
        assert_eq!(body.matches("is-loud").count(), 1, "the parked cell is the loud one");
    }

    // -- the stop line ----------------------------------------------------------

    #[test]
    fn the_stop_reason_picks_a_tone_and_a_glyph_and_keeps_its_words() {
        let cases = [
            (StopReason::QueueEmpty, "tone-done", "\u{25CF}"),
            (StopReason::BatchDone, "tone-done", "\u{25CF}"),
            (StopReason::Cancelled, "tone-neutral", "\u{25CB}"),
            (StopReason::Crashed { attempts: 2 }, "tone-failed", "\u{25A0}"),
            (StopReason::NoProgress, "tone-neutral", "\u{25CB}"),
            (StopReason::MaxIterations, "tone-neutral", "\u{25CB}"),
            (StopReason::Unreadable, "tone-neutral", "\u{25CB}"),
            (StopReason::SessionRemoved, "tone-neutral", "\u{25CB}"),
            (StopReason::NothingDone { batches: 2 }, "tone-neutral", "\u{25CB}"),
            (StopReason::Preflight { detail: "x".into() }, "tone-neutral", "\u{25CB}"),
            (StopReason::NeedsAnswer { question: "q".into() }, "tone-neutral", "\u{25CB}"),
        ];
        for (reason, tone, glyph) in cases {
            let html = render(&RunReport { stop_reason: &reason, ..report(60, None, &[]) });
            let expected = format!(
                "<p class=\"stop {tone}\"><span class=\"g\" aria-hidden=\"true\">{glyph}</span><span>{}</span></p>",
                stop_reason_line(&reason)
            );
            assert!(html.contains(&expected), "{reason:?}: {html}");
        }
    }

    #[test]
    fn the_stop_line_stands_after_the_strip_and_before_the_sections() {
        let tasks = Tasks { closed: vec![line("a-1")], parked: vec![] };
        let reason = StopReason::NoProgress;
        let html = render(&RunReport {
            stop_reason: &reason,
            ..report(600, Some(&tasks), &[batch(1)])
        });

        let strip_total = html.find(">Total</span>").expect("the strip's last cell");
        let reason_at = html.find("moved neither what was ready").expect("the reason line");
        let closed_at = html.find("id=\"closed\"").expect("closed");
        assert!(strip_total < reason_at, "the reason follows the counters: {html}");
        assert!(reason_at < closed_at, "and stands before the task sections: {html}");
    }

    // -- batches ----------------------------------------------------------------

    /// The five kinds of the design, and the one this document has beside them.
    /// Written out once, so "every ending is named" cannot quietly become "the
    /// endings somebody remembered to list twice".
    fn vocabulary() -> Vec<(BatchOutcome, &'static str, &'static str)> {
        vec![
            (BatchOutcome::Exited, "outcome tone-done", "The run saw its session exit cleanly."),
            (
                BatchOutcome::Failed { code: 137 },
                "outcome tone-failed",
                "The session crashed with exit code <code>137</code>.",
            ),
            (
                BatchOutcome::NoCode,
                "outcome tone-failed",
                "The session process disappeared without an exit code.",
            ),
            (BatchOutcome::Removed, "outcome tone-neutral", "A person removed the session."),
            (
                BatchOutcome::HandedBack,
                "outcome tone-needs-you",
                "The lead handed the work back to a person.",
            ),
            (
                BatchOutcome::Unanswered { question: "Do you trust this folder?".into() },
                "outcome tone-needs-you",
                "&ldquo;Do you trust this folder?&rdquo;",
            ),
        ]
    }

    #[test]
    fn every_batch_outcome_draws_its_own_tone_glyph_and_words() {
        for (outcome, class, words) in vocabulary() {
            let mut b = batch(1);
            b.outcome = outcome.clone();
            let html = render(&report(600, None, &[b]));
            let line_start = html.find(&format!("<p class=\"{class}\">")).unwrap_or_else(|| {
                panic!("no `{class}` line for {outcome:?}: {html}")
            });
            let line = &html[line_start..html[line_start..].find("</p>").unwrap() + line_start];
            assert!(line.contains(words), "{outcome:?}: {line}");
            assert!(line.contains("<span class=\"g\" aria-hidden=\"true\">"), "{line}");
        }
    }

    #[test]
    fn the_glyph_of_each_tone_is_the_designs() {
        let glyph = |outcome: BatchOutcome| {
            let mut b = batch(1);
            b.outcome = outcome;
            let html = render(&report(600, None, &[b]));
            let at = html.find("<p class=\"outcome ").unwrap();
            let rest = &html[at..];
            let g = rest.find("aria-hidden=\"true\">").unwrap() + "aria-hidden=\"true\">".len();
            rest[g..g + rest[g..].find('<').unwrap()].to_string()
        };
        assert_eq!(glyph(BatchOutcome::Exited), "\u{25CF}");
        assert_eq!(glyph(BatchOutcome::Failed { code: 1 }), "\u{25A0}");
        assert_eq!(glyph(BatchOutcome::NoCode), "\u{25A0}");
        assert_eq!(glyph(BatchOutcome::Removed), "\u{25CB}");
        assert_eq!(glyph(BatchOutcome::HandedBack), "\u{25B2}");
    }

    #[test]
    fn a_batch_that_left_no_account_still_says_how_the_run_saw_it_end() {
        // smetana-pmj, and the case the whole change exists for: a batch whose
        // agent was killed before it could write a word. The run knew what it
        // saw, and the phrase about the *agent's* silence was the entire record
        // of a batch that died holding the merge lock.
        for (outcome, class, words) in vocabulary() {
            let mut quiet = batch(1);
            quiet.reported = false;
            quiet.outcome = outcome.clone();
            let html = render(&report(600, None, &[quiet]));

            assert!(
                html.contains("<p class=\"no-account\">This batch left no account of itself.</p>"),
                "the phrase stays, and stays about the agent: {outcome:?}"
            );
            assert!(html.contains(class) && html.contains(words), "{outcome:?}: {html}");
        }
    }

    #[test]
    fn every_batch_carries_the_runs_own_ending_whether_it_wrote_a_file_or_not() {
        // The two halves are a pair, not a fallback, and a reader comparing "the
        // lead says it merged three" against "the session exited with code 1" is
        // exactly the reading this is for.
        for (outcome, _, words) in vocabulary() {
            let mut spoke = batch(1);
            spoke.notes = Some("nothing odd".into());
            spoke.outcome = outcome.clone();
            let html = render(&report(600, None, &[spoke]));

            assert!(html.contains("nothing odd"), "the lead's own words stay: {outcome:?}");
            assert!(html.contains(words), "beside the run's: {html}");
            assert!(!html.contains("left no account of itself"), "{html}");
        }
    }

    #[test]
    fn a_batch_file_without_notes_is_not_called_silent() {
        // A file somebody wrote with no `notes` in it is still an account; the
        // line says what is true of it, in the muted style.
        let html = render(&report(600, None, &[batch(1)]));
        assert!(html.contains("<p class=\"no-account\">No notes on the batch as a whole.</p>"), "{html}");
        assert!(!html.contains("left no account of itself"), "{html}");
    }

    #[test]
    fn a_batch_that_died_holding_the_board_says_what_it_was_left_holding() {
        // The row that would have saved the night: a batch killed mid-merge
        // leaves a claimed lock behind, and the next run discovers it by failing
        // to take it. Ids, because a person has to be able to go and look.
        let mut killed = batch(1);
        killed.reported = false;
        killed.outcome = BatchOutcome::NoCode;
        killed.left_behind = vec![
            Leftover { id: "smetana-js4".into(), status: "in_progress".into(), lock: true },
            Leftover { id: "smetana-42v".into(), status: "ready_to_merge".into(), lock: false },
        ];
        let html = render(&report(600, None, &[killed]));

        assert!(
            html.contains(
                "<dl class=\"f-body\">\n<dt>Left unfinished</dt>\n<dd class=\"ids\"><code>smetana-js4</code><code>smetana-42v</code></dd>\n</dl>"
            ),
            "{html}"
        );
        assert!(!html.contains("Lock released"), "nothing was taken back: {html}");
    }

    #[test]
    fn a_lock_the_run_took_back_is_named_with_the_actor() {
        // The release is a write the app made, and this row is the only record of
        // it there will ever be: nothing goes onto the lock's own issue, because a
        // note there would move `updated_at` and make a dead claim look fresh to
        // every lead waiting on it.
        let mut killed = batch(1);
        killed.reported = false;
        killed.outcome = BatchOutcome::NoCode;
        killed.lock_released =
            Some(LockRelease { id: "smetana-lock".into(), actor: "smetana-run-7".into() });
        let html = render(&report(600, None, &[killed]));

        assert!(
            html.contains(
                "<dt>Lock released</dt>\n<dd><code>smetana-lock</code> \u{00B7} by <code>smetana-run-7</code></dd>"
            ),
            "{html}"
        );
        assert!(!html.contains("Left unfinished"), "{html}");
    }

    #[test]
    fn a_batch_that_left_the_board_clean_draws_no_row_about_it() {
        let html = render(&report(600, None, &[batch(1)]));
        let html = body_of(&html);
        assert!(!html.contains("Left unfinished") && !html.contains("Lock released"), "{html}");
        assert!(!html.contains("f-body"), "{html}");
    }

    #[test]
    fn a_batch_label_names_an_attempt_only_past_the_first() {
        let mut second = batch(2);
        second.attempt = 3;
        let html = render(&report(600, None, &[batch(1), second]));
        assert!(html.contains("batch 1 <span class=\"agent\">"), "{html}");
        assert!(html.contains("batch 2 \u{00B7} attempt 3 <span class=\"agent\">"), "{html}");
        assert!(!html.contains("attempt 1"), "{html}");
    }

    // -- text from agents --------------------------------------------------------

    #[test]
    fn a_long_account_with_paths_and_backticks_comes_out_as_prose_and_code() {
        let did = "Added `denial` to `EventKind::ToolResult` and a detector in \
                   `src-tauri/src/session/claude_driver.rs`; merged as `4e86a5bd`, \
                   see also /Users/someone/a/very/long/path/that/must/wrap/by/itself.rs \
                   and a stray ` backtick.";
        let mut b = batch(1);
        b.tasks = vec![BatchTask { id: "smetana-6tev".into(), did: Some(did.into()) }];
        let tasks = Tasks { closed: vec![line("smetana-6tev")], parked: vec![] };
        let html = render(&report(600, Some(&tasks), &[b]));

        assert!(
            html.contains("<p class=\"task-body\">Added <code>denial</code> to <code>EventKind::ToolResult</code> and a detector in <code>src-tauri/src/session/claude_driver.rs</code>; merged as <code>4e86a5bd</code>,"),
            "{html}"
        );
        assert!(html.contains("/Users/someone/a/very/long/path/that/must/wrap/by/itself.rs"), "a bare path stays prose: {html}");
        assert!(html.contains("and a stray ` backtick.</p>"), "an unpaired backtick is left as written: {html}");
    }

    #[test]
    fn markup_in_did_and_in_notes_comes_out_as_text_and_not_as_a_tag() {
        let mut b = batch(1);
        b.tasks = vec![BatchTask {
            id: "a-1".into(),
            did: Some("done <script>alert(1)</script> and `<script>x</script>`".into()),
        }];
        b.notes = Some("notes <script>alert(2)</script> & <b>bold</b>".into());
        b.summary = Some("summary <script>alert(3)</script>".into());
        let tasks = Tasks {
            closed: vec![TaskLine { id: "a-1".into(), title: "title <script>t</script>".into() }],
            parked: vec![],
        };
        let html = render(&report(600, Some(&tasks), &[b]));
        let body = &html[html.find("<main").unwrap()..];

        assert!(!body.contains("<script"), "an agent wrote this and a person opens it: {body}");
        assert!(!body.contains("<b>"), "{body}");
        assert!(body.contains("done &lt;script&gt;alert(1)&lt;/script&gt;"), "{body}");
        assert!(body.contains("notes &lt;script&gt;alert(2)&lt;/script&gt; &amp; &lt;b&gt;bold&lt;/b&gt;"), "{body}");
        assert!(body.contains("summary &lt;script&gt;alert(3)&lt;/script&gt;"), "{body}");
        assert!(body.contains("<h3>title &lt;script&gt;t&lt;/script&gt;</h3>"), "{body}");
    }

    #[test]
    fn identifiers_and_names_are_escaped_where_they_are_written() {
        let mut b = batch(1);
        b.agent = "cl<aude>".into();
        b.lock_released = Some(LockRelease { id: "<i>".into(), actor: "<a>".into() });
        b.left_behind = vec![Leftover { id: "<l>".into(), status: "s".into(), lock: false }];
        let tasks = Tasks { closed: vec![line("<t>")], parked: vec![] };
        let batches = [b];
        let run = RunReport {
            project: "/p<q>",
            scope: "s<c>",
            journal: Some("/j<k>"),
            ..report(1, Some(&tasks), &batches)
        };
        let html = render(&run);
        let body = &html[html.find("<main").unwrap()..];
        for raw in ["<aude>", "<i>", "<a>", "<l>", "<t>", "<q>", "<c>", "<k>"] {
            assert!(!body.contains(raw), "{raw} reached the document unescaped: {body}");
        }
    }

    // -- sections, summary, durations -------------------------------------------

    #[test]
    fn the_summary_section_stands_before_closed_and_carries_one_paragraph_per_batch() {
        let tasks = Tasks { closed: vec![line("a-1")], parked: vec![] };
        let mut first = batch(1);
        first.summary = Some("Fixed the login form & tidied the tests.".into());
        let mut second = batch(2);
        second.summary = Some("Added the export button.".into());
        let batches = [first, second];
        let html = render(&report(3600, Some(&tasks), &batches));

        let sec = html.find("<h2 id=\"summary-h\">Summary</h2>").expect("the summary section");
        let closed = html.find("id=\"closed\"").expect("the closed section");
        assert!(sec < closed, "summary comes before closed: {html}");
        assert!(
            html.contains("<div class=\"prose\">\n<p>Fixed the login form &amp; tidied the tests.</p>\n<p>Added the export button.</p>\n</div>"),
            "{html}"
        );
    }

    #[test]
    fn no_summary_at_all_draws_no_section_and_a_blank_one_counts_as_none() {
        let tasks = Tasks { closed: vec![line("a-1")], parked: vec![] };
        let mut blank = batch(1);
        blank.summary = Some("".into());
        let mut whitespace = batch(2);
        whitespace.summary = Some("   ".into());
        let html = render(&report(3600, Some(&tasks), &[blank, whitespace, batch(3)]));
        assert!(!body_of(&html).contains("Summary"), "no section over nothing: {html}");

        // A real one beside a blank one draws only the real one, trimmed.
        let mut blank = batch(1);
        blank.summary = Some("".into());
        let mut real = batch(2);
        real.summary = Some("  Did a real thing.  ".into());
        let html = render(&report(3600, Some(&tasks), &[blank, real]));
        assert!(html.contains("<div class=\"prose\">\n<p>Did a real thing.</p>\n</div>"), "{html}");
    }

    #[test]
    fn the_summary_is_drawn_even_when_the_board_could_not_be_read() {
        // The summary comes from the batches, not from the board.
        let mut b = batch(1);
        b.summary = Some("Did a thing.".into());
        let html = render(&report(3600, None, &[b]));
        assert!(html.contains("<h2 id=\"summary-h\">Summary</h2>"), "{html}");
    }

    #[test]
    fn a_task_with_no_account_draws_a_dash() {
        let tasks = Tasks { closed: vec![line("a-1")], parked: vec![] };
        let html = render(&report(600, Some(&tasks), &[]));
        assert!(html.contains("<p class=\"no-account\">\u{2014}</p>"), "{html}");
        assert!(html.contains("aria-label=\"duration unknown\""), "{html}");
    }

    #[test]
    fn a_task_alone_in_its_batch_shows_a_time_and_one_sharing_a_batch_shows_none() {
        let alone = batch_naming(1, 3600, &["a-1"]);
        let shared = batch_naming(2, 7260, &["a-2", "a-3"]);
        let tasks = Tasks { closed: vec![line("a-1"), line("a-2")], parked: vec![] };
        let html = render(&report(11000, Some(&tasks), &[alone, shared]));
        // The cards themselves, not the whole document: every batch prints its
        // own duration under Batches, which says nothing about a task.
        let rows = &html[html.find("id=\"closed\"").unwrap()..html.find("id=\"batches\"").unwrap()];
        assert!(rows.contains("1h 0m"), "the batch that held one task lends it its own number");
        assert!(!rows.contains("2h 1m"), "and the batch that held two lends nothing");
    }

    #[test]
    fn a_task_handed_on_to_a_shared_batch_shows_no_duration_at_all() {
        // Batch 1 held `a-1` alone, batch 2 took it on beside `a-2`. The row
        // carries batch 2's account, so the rule is asked of batch 2 — which held
        // two tasks and lends none of them a number. Borrowing batch 1's hour
        // would be a figure that looks measured and belongs to different work.
        let alone = batch_naming(1, 3600, &["a-1"]);
        let shared = batch_naming(2, 7260, &["a-1", "a-2"]);
        assert_eq!(task_duration(&[alone.clone(), shared.clone()], "a-1"), None);

        let tasks = Tasks { closed: vec![line("a-1")], parked: vec![] };
        let html = render(&report(11000, Some(&tasks), &[alone, shared]));
        let rows = &html[html.find("id=\"closed\"").unwrap()..html.find("id=\"batches\"").unwrap()];
        assert!(!rows.contains("1h 0m"), "an hour measured against other work: {rows}");
        assert!(!rows.contains("2h 1m"), "and the shared batch lends nothing either: {rows}");
    }

    #[test]
    fn a_duration_reads_by_the_handoffs_rule() {
        assert_eq!(duration(0), "<1m");
        assert_eq!(duration(59), "<1m");
        assert_eq!(duration(60), "1m");
        assert_eq!(duration(119), "1m", "minutes are floored");
        assert_eq!(duration(840), "14m");
        assert_eq!(duration(3599), "59m");
        assert_eq!(duration(3600), "1h 0m", "`1h 0m` stays as it is");
        assert_eq!(duration(8040), "2h 14m");
    }

    #[test]
    fn a_duration_under_a_minute_is_escaped_in_the_document() {
        let html = render(&report(20, None, &[]));
        assert!(html.contains("<span class=\"cell-n\">&lt;1m</span>"), "{html}");
        assert!(!html.contains("<1m"), "{html}");
    }

    #[test]
    fn the_footer_names_the_journal_this_run_wrote_and_is_quiet_without_one() {
        let path = "/p/.smetana/runs/3/journal-2026-08-29-220109.log";
        let html = render(&RunReport { journal: Some(path), ..report(8040, None, &[]) });
        assert!(html.contains(&format!("<dt>Journal</dt>\n<dd><span class=\"mono\">{path}</span></dd>")), "{html}");
        assert!(!html.contains("<a "), "a path, never a link: this document reaches nowhere");

        // Naming a path that is not there sends somebody to a file that does not
        // exist, which is worse than the document being quiet about it.
        let html = render(&report(8040, None, &[]));
        assert!(!html.contains("Journal"), "{html}");
    }

    #[test]
    fn the_document_sets_no_fixed_width_of_its_own() {
        // Everything that could force a horizontal scroll at 360px is the
        // stylesheet's to handle, and it is not touched here: the markup carries
        // no `style` attribute and no width of its own.
        let (tasks, batches) = ordinary_run();
        let html = render(&report(4320, Some(&tasks), &batches));
        let body = &html[html.find("<main").unwrap()..];
        assert!(!body.contains("style="), "{body}");
        assert!(!body.contains("width"), "{body}");
        assert!(!body.contains("<table"), "{body}");
    }
}
