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
#[derive(Debug, Clone)]
pub struct BatchLine {
    pub n: u32,
    pub seconds: u64,
    pub tasks: Vec<BatchTask>,
    pub notes: Option<String>,
    pub reported: bool,
    pub outcome: BatchOutcome,
    pub left_behind: Vec<Leftover>,
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
    out.push_str("</style></head><body><div class=\"doc\">");

    // Header. The eyebrow is the only place the product names itself, and the
    // heading's words are `RunMode::report_title`'s — this file places them and
    // owns none of them.
    out.push_str("<header><p class=\"eyebrow\">smetana &middot; run report</p><h1>");
    out.push_str(&escape(report.title));
    out.push_str("</h1><p class=\"meta\">");
    out.push_str(&escape(report.project));
    out.push_str(" &middot; ");
    out.push_str(&escape(report.scope));
    out.push_str(" &middot; finished ");
    out.push_str(&escape(report.finished));
    out.push_str("</p></header>");

    // The strip, and the one place in the document where an unreadable board
    // could quietly become a zero. It does not: `None` draws a dash.
    out.push_str("<div class=\"strip\">");
    match report.tasks {
        Some(tasks) => {
            cell(&mut out, "closed", &tasks.closed.len().to_string(), "cell-done");
            cell(&mut out, "parked", &tasks.parked.len().to_string(), "cell-loud");
        }
        None => {
            cell(&mut out, "closed", "&mdash;", "cell-done");
            cell(&mut out, "parked", "&mdash;", "cell-loud");
        }
    }
    cell(&mut out, "batches", &report.batches.len().to_string(), "");
    cell(&mut out, "total", &human(report.seconds), "");
    out.push_str("</div>");

    match report.tasks {
        // Not a zero, and not a silence either: the reason the lists are
        // missing is itself the thing worth saying.
        None => out.push_str(
            "<p class=\"notice\">The board could not be read, so what moved on it is unknown. \
             Nothing below is a count of zero.</p>",
        ),
        Some(tasks) => {
            section(&mut out, "closed", "done", "", &tasks.closed, report.batches);
            section(&mut out, "parked", "needs you", " card-parked", &tasks.parked, report.batches);
        }
    }

    if !report.batches.is_empty() {
        header(&mut out, "batches", report.batches.len());
        out.push_str("<div class=\"list\">");
        for b in report.batches {
            out.push_str("<div class=\"card card-batch\"><div class=\"head\">");
            out.push_str("<span class=\"batch-label\">batch ");
            out.push_str(&b.n.to_string());
            out.push_str("</span><span class=\"right\">");
            out.push_str(&human(b.seconds));
            out.push_str("</span></div>");
            // The agent's own half first, and then the run's, always. The
            // phrase below is still about the *account* — a killed agent really
            // did write nothing — and it is no longer the whole record of the
            // batch, which is what left the night of smetana-pmj to `log show`.
            if !b.reported {
                out.push_str("<p class=\"unknown\">This batch left no account of itself.</p>");
            } else {
                match &b.notes {
                    Some(notes) => {
                        out.push_str("<p class=\"body\">");
                        out.push_str(&prose(notes));
                        out.push_str("</p>");
                    }
                    None => {
                        out.push_str("<p class=\"unknown\">No notes on the batch as a whole.</p>");
                    }
                }
            }
            outcome(&mut out, &b.outcome);
            held(&mut out, &b.left_behind);
            out.push_str("</div>");
        }
        out.push_str("</div>");
    }

    out.push_str("<div class=\"total\"><span class=\"total-label\">total</span>");
    out.push_str("<span class=\"total-n\">");
    out.push_str(&human(report.seconds));
    out.push_str("</span></div></div></body></html>");
    out
}

/// What the run saw end the batch, in a sentence, under every batch card and
/// whether or not the agent left a file.
///
/// It says "the run saw" rather than stating the ending flat, and that is the
/// honest form: this is one party's account of a process it was watching from
/// outside, and *why* the process went is beyond it — the night this was written
/// for had a session ending tidily, atexit handlers and all, for a reason
/// nothing in this app could have named. What the app can promise is that it
/// wrote down what it saw.
fn outcome(out: &mut String, outcome: &BatchOutcome) {
    out.push_str("<p class=\"outcome\">");
    match outcome {
        BatchOutcome::Exited => out.push_str("The run saw its session exit cleanly."),
        BatchOutcome::Failed { code } => {
            out.push_str("The run saw its session exit with code ");
            out.push_str(&code.to_string());
            out.push('.');
        }
        // Said in words rather than as an absent number: "no exit code" alone
        // reads as something the app failed to look up, when it is a fact about
        // the process.
        BatchOutcome::NoCode => out.push_str(
            "The run saw its session end with no exit code at all, which is what a \
             signalled process leaves.",
        ),
        BatchOutcome::Removed => {
            out.push_str("Somebody removed this session from the agents panel while the run was \
                          waiting on it.");
        }
        BatchOutcome::HandedBack => out.push_str(
            "The lead handed the work back, and its session was left running with the \
             conversation open.",
        ),
        // The question is a harness's own text reaching a document a person
        // opens, so it goes through `escape` like everything else. `prose` is
        // not used: nobody was asked to type backticks into a dialog.
        BatchOutcome::Unanswered { question } => {
            out.push_str(
                "The run ended this batch at a question it had nobody to answer: &ldquo;",
            );
            out.push_str(&escape(question));
            out.push_str("&rdquo;");
        }
    }
    out.push_str("</p>");
}

/// What this batch's actor was still holding on the board when the batch ended —
/// a claimed merge lock, work left `in_progress`, work left `ready_to_merge`.
///
/// Drawn only when there is something to draw: a line saying an actor held
/// nothing would stand under every batch of every report, and a line that is
/// always there is one nobody reads.
///
/// **Named, never acted on.** The app writes to the tracker nowhere as part of
/// recovery (`recovery.rs`), so nothing here releases a lock or parks a claim;
/// `running-tasks` Phase R does that with the worktrees in front of it. Without
/// the line, a lock a dead batch is still holding is discovered by the *next*
/// run failing to take it.
fn held(out: &mut String, held: &[Leftover]) {
    if held.is_empty() {
        return;
    }
    out.push_str("<p class=\"held\">When this batch ended, its actor still held on the board: ");
    for (nth, item) in held.iter().enumerate() {
        if nth > 0 {
            out.push_str(", ");
        }
        out.push_str("<code>");
        out.push_str(&escape(&item.id));
        out.push_str("</code>");
        if item.lock {
            out.push_str(" the merge lock");
        }
        out.push_str(" (");
        out.push_str(&escape(&item.status));
        out.push(')');
    }
    // The list and nothing after it. A closing sentence about the app having
    // cleared none of it was written and taken out again: the run *does* park a
    // stuck batch's claims after an unanswered question, moments after this
    // reading, so the reassurance would have been false in one of the six
    // endings and there is nothing in the document to tell the reader which.
    out.push_str(".</p>");
}

/// One cell of the summary strip. `extra` carries the hue that says what the
/// number is about, and `cell-none` is added for a value that is not a count —
/// a dash over an unreadable board, or a zero, both of which are muted so that
/// only a real quantity is drawn in a status colour.
fn cell(out: &mut String, label: &str, value: &str, extra: &str) {
    out.push_str("<div class=\"cell\"><span class=\"cell-label\">");
    out.push_str(label);
    out.push_str("</span><span class=\"cell-n");
    if !extra.is_empty() {
        out.push(' ');
        out.push_str(extra);
    }
    if value == "0" || value == "&mdash;" {
        out.push_str(" cell-none");
    }
    out.push_str("\">");
    out.push_str(value);
    out.push_str("</span></div>");
}

/// A section's rule and its count. A section with no items never reaches this —
/// the strip is what carries the zero, and an empty heading over nothing is a
/// line asking to be read for no reason.
fn header(out: &mut String, label: &str, count: usize) {
    out.push_str("<div class=\"sec\"><span>");
    out.push_str(label);
    out.push_str("</span><span class=\"sec-n\">");
    out.push_str(&count.to_string());
    out.push_str("</span></div>");
}

/// One list of tasks, each with whatever its batch said it did and — where the
/// batch held it alone — how long that batch took.
///
/// The badge is decided by the section rather than read off the task, because
/// the board's own status is not carried this far: `TaskLine` is an id and a
/// title. Both words here are `RESERVED` statuses with fixed tokens, so nothing
/// is hashed and nothing needs a two-letter code.
fn section(
    out: &mut String,
    label: &str,
    badge: &str,
    card_extra: &str,
    lines: &[TaskLine],
    batches: &[BatchLine],
) {
    if lines.is_empty() {
        return;
    }
    header(out, label, lines.len());
    out.push_str("<div class=\"list\">");
    let badge_class = if card_extra.is_empty() { "badge-done" } else { "badge-parked" };
    for line in lines {
        // One rule answers the whole card — both what was done and how long it
        // took — because `task_duration` is `last_naming` too. Two *rules* is
        // what let the columns name different batches, and a duration belonging
        // to a different piece of work is the same lie as an account belonging
        // to one.
        let batch = last_naming(batches, &line.id);
        out.push_str("<div class=\"card");
        out.push_str(card_extra);
        out.push_str("\"><div class=\"head\"><span class=\"chip\">");
        out.push_str(&escape(&line.id));
        out.push_str("</span><span class=\"badge ");
        out.push_str(badge_class);
        out.push_str("\">");
        out.push_str(badge);
        out.push_str("</span><span class=\"right\">");
        // The public rule rather than `duration_of` on the batch above, so the
        // number drawn here is the one the tests exercise. It resolves to the
        // very same batch — both go through `last_naming` — so this cannot
        // disagree with the account beside it.
        match task_duration(batches, &line.id) {
            Some(seconds) => out.push_str(&human(seconds)),
            None => out.push_str("&mdash;"),
        }
        out.push_str("</span></div><h3>");
        out.push_str(&escape(&line.title));
        out.push_str("</h3>");
        let did = batch
            .and_then(|b| b.tasks.iter().find(|t| t.id == line.id))
            .and_then(|t| t.did.as_deref());
        match did {
            Some(text) => {
                out.push_str("<p class=\"body\">");
                out.push_str(&prose(text));
                out.push_str("</p>");
            }
            // A dash rather than nothing: the batch either said nothing about
            // this task or left no account at all, and both are already named
            // under Batches.
            None => out.push_str("<p class=\"unknown\">&mdash;</p>"),
        }
        out.push_str("</div>");
    }
    out.push_str("</div>");
}

/// The document's palette, light — and the complete one, because it stands on a
/// bare `:root` and every dark block below only redefines these names. A colour
/// whose sole definition sat inside a media query or an attribute block would
/// simply be absent for the reader that block does not match, and nothing on
/// screen would say a colour had gone missing.
///
/// The names are the app's token names and the values are the app's values,
/// copied from `src/styles/tokens/`. That is a change: they used to be `--doc-*`
/// precisely because the values were not the app's, and a shared name over a
/// different value would have drifted in silence. Now the name is a true
/// statement. See the module comment for why literals live here and nowhere else
/// in this repository — there is no stylesheet of ours around this document, so
/// there is nothing for a `var()` to resolve against.
///
/// Only the status families this document draws are here. A closed card renders
/// `done` and a parked one renders `needs-you`; the other four families and
/// `--attn-live` are absent because nothing draws them, and every generated
/// document would otherwise carry them.
///
/// `color-scheme` is the one declaration that is not a colour, and it earns its
/// place: the frame's scrollbar and every default the user agent paints —
/// selection among them — follow it rather than the rules below, so without it a
/// dark report comes with a light scrollbar down its side.
const LIGHT: &str = "color-scheme:light;\
--canvas:#eaeeef;--surface-sunken:#e1e6e7;--surface:#f4f7f7;--surface-raised:#ffffff;\
--border-subtle:#dde3e3;--border:#c9d1d2;--border-strong:#a9b4b6;\
--text-primary:#16201f;--text-secondary:#4a565a;--text-muted:#6b777c;\
--text-link:#1f5d8f;--text-link-hover:#123f63;\
--focus-ring:#1c6fd0;--selection-bg:#c6dcf0;--scrollbar-thumb:#c2caca;\
--status-done-fg:#3f6b54;--status-done-bg:#e6eee9;--status-done-border:#c0d3c8;\
--status-needs-you-fg:#8a5405;--status-needs-you-bg:#fbf0da;--status-needs-you-border:#e8ce94;\
--attn-loud:#b96a06;--shadow-raised:0 1px 2px rgba(22,32,31,.08)";

/// The same names again, dark. Written once and placed twice, because the
/// document has two readers that ask differently — see `style`. `--shadow-raised`
/// collapses to `none`: a raised surface in the dark theme is told apart by its
/// own lightness, and a shadow under it only smears the edge.
const DARK: &str = "color-scheme:dark;\
--canvas:#10151a;--surface-sunken:#0c1116;--surface:#161b21;--surface-raised:#1b2229;\
--border-subtle:#232b33;--border:#2e3841;--border-strong:#3d4954;\
--text-primary:#e3e8ed;--text-secondary:#a8b3bd;--text-muted:#7c8b97;\
--text-link:#8fb6e8;--text-link-hover:#b3cef2;\
--focus-ring:#5fa8ff;--selection-bg:#2b4560;--scrollbar-thumb:#333e48;\
--status-done-fg:#7fa792;--status-done-bg:#16211c;--status-done-border:#2c4136;\
--status-needs-you-fg:#f2b03d;--status-needs-you-bg:#2b2010;--status-needs-you-border:#6a4e1b;\
--attn-loud:#f2b03d;--shadow-raised:none";

/// What the document actually draws, through the names above and no colour of its
/// own.
///
/// The scale is written out rather than referenced: these are the design system's
/// space, type and radius steps, and there is nothing here for a `var()` to
/// resolve them against. `box-sizing:border-box` on everything is the same first
/// line `tokens/base.css` opens with, and everything below declares a size and
/// adds padding and a border on top of it.
///
/// **The mono stack names no IBM Plex Mono, and that is a deliberate divergence
/// from `tokens/fonts.css` rather than an omission.** The app's `--font-mono`
/// opens with it; this document may fetch nothing, and the app's own `@font-face`
/// does not reach across into an `<iframe sandbox="" srcdoc>`, so naming a face
/// neither reader can load would only be a line of CSS that never applies. The
/// handoff contradicts itself here — its first constraint says the stack falls
/// past Plex, its typography block says to copy `--font-mono` literally — and the
/// spec and `[merge].hazards` both resolve it as "drop Plex". Do not put it back
/// on the grounds that the tokens have it.
///
/// The stack is repeated in full at every rule that needs it rather than
/// inherited from one class: a document with no stylesheet of ours around it has
/// no `--font-mono` to reach for, and a single shared class would have to be put
/// on every one of these elements by hand in the markup, where forgetting it
/// shows up as one line of prose set in sans among identifiers.
const RULES: &str = "\
*,*::before,*::after{box-sizing:border-box}\
::selection{background:var(--selection-bg)}\
::-webkit-scrollbar{width:10px;height:10px}\
::-webkit-scrollbar-track{background:transparent}\
::-webkit-scrollbar-thumb{background:var(--scrollbar-thumb);border-radius:5px}\
a{color:var(--text-link);text-decoration:none}\
a:hover{color:var(--text-link-hover);text-decoration:underline}\
body{margin:0;padding:32px 16px 40px;background:var(--canvas);color:var(--text-primary);\
font-family:system-ui,-apple-system,\"Segoe UI\",\"Noto Sans\",Roboto,sans-serif;\
font-size:13px;line-height:1.5}\
.doc{max-width:52rem;margin:0 auto;display:flex;flex-direction:column;gap:24px}\
code{font-family:ui-monospace,\"SF Mono\",Menlo,Consolas,\
\"DejaVu Sans Mono\",monospace}\
.eyebrow{font-family:ui-monospace,\"SF Mono\",Menlo,Consolas,\
\"DejaVu Sans Mono\",monospace;font-size:10px;letter-spacing:.07em;text-transform:uppercase;\
color:var(--text-muted);margin:0 0 8px}\
h1{font-size:22px;font-weight:600;letter-spacing:-.006em;line-height:1.2;margin:0}\
.meta{font-family:ui-monospace,\"SF Mono\",Menlo,Consolas,\
\"DejaVu Sans Mono\",monospace;font-size:12px;color:var(--text-secondary);\
word-break:break-all;margin:8px 0 0}\
.strip{display:grid;grid-template-columns:repeat(auto-fit,minmax(150px,1fr));gap:8px}\
.cell{background:var(--surface-raised);border:1px solid var(--border-subtle);border-radius:4px;\
box-shadow:var(--shadow-raised);padding:10px;display:flex;flex-direction:column;gap:4px}\
.cell-label{font-family:ui-monospace,\"SF Mono\",Menlo,Consolas,\
\"DejaVu Sans Mono\",monospace;font-size:10px;letter-spacing:.07em;text-transform:uppercase;\
color:var(--text-muted)}\
.cell-n{font-family:ui-monospace,\"SF Mono\",Menlo,Consolas,\
\"DejaVu Sans Mono\",monospace;font-size:22px;font-weight:500;line-height:1.2;\
color:var(--text-primary)}\
.cell-done{color:var(--status-done-fg)}\
.cell-loud{color:var(--attn-loud)}\
.cell-none{color:var(--text-muted)}\
.sec{display:flex;align-items:baseline;gap:8px;border-bottom:1px solid var(--border);\
padding-bottom:6px;margin:0 0 -8px;font-family:ui-monospace,\"SF Mono\",Menlo,\
Consolas,\"DejaVu Sans Mono\",monospace;font-size:10px;letter-spacing:.07em;\
text-transform:uppercase;font-weight:400;color:var(--text-secondary)}\
.sec-n{color:var(--text-muted);letter-spacing:0}\
.list{display:flex;flex-direction:column;gap:8px}\
.card{background:var(--surface-raised);border:1px solid var(--border-subtle);border-radius:4px;\
box-shadow:var(--shadow-raised);padding:16px;display:flex;flex-direction:column;gap:8px}\
.card-parked{border-color:var(--status-needs-you-border)}\
.card-batch{background:var(--surface);box-shadow:none}\
.head{display:flex;align-items:center;gap:8px;flex-wrap:wrap}\
.chip{font-family:ui-monospace,\"SF Mono\",Menlo,Consolas,\
\"DejaVu Sans Mono\",monospace;font-size:12px;font-weight:500;background:var(--surface-sunken);\
border:1px solid var(--border-subtle);border-radius:3px;padding:1px 6px;white-space:nowrap}\
.badge{font-family:ui-monospace,\"SF Mono\",Menlo,Consolas,\
\"DejaVu Sans Mono\",monospace;font-size:11px;border-radius:3px;padding:1px 6px;\
white-space:nowrap;border:1px solid}\
.badge-done{background:var(--status-done-bg);color:var(--status-done-fg);\
border-color:var(--status-done-border)}\
.badge-parked{background:var(--status-needs-you-bg);color:var(--status-needs-you-fg);\
border-color:var(--status-needs-you-border)}\
.batch-label{font-family:ui-monospace,\"SF Mono\",Menlo,Consolas,\
\"DejaVu Sans Mono\",monospace;font-size:10px;letter-spacing:.07em;text-transform:uppercase;\
color:var(--text-secondary)}\
.right{margin-left:auto;font-family:ui-monospace,\"SF Mono\",Menlo,Consolas,\
\"DejaVu Sans Mono\",monospace;font-size:11px;color:var(--text-muted)}\
h3{margin:0;font-size:15px;font-weight:600;line-height:1.35}\
.body{margin:0;color:var(--text-secondary)}\
.body code{font-size:12px;color:var(--text-primary)}\
.unknown{margin:0;color:var(--text-muted)}\
.outcome{margin:0;color:var(--text-secondary)}\
.held{margin:0;color:var(--attn-loud)}\
.held code{font-size:12px}\
.notice{background:var(--surface);border:1px solid var(--border-subtle);border-radius:4px;\
padding:16px;color:var(--text-muted);margin:0}\
.total{border-top:1px solid var(--border-strong);padding-top:12px;display:flex;\
align-items:baseline;gap:8px}\
.total-label{font-family:ui-monospace,\"SF Mono\",Menlo,Consolas,\
\"DejaVu Sans Mono\",monospace;font-size:10px;letter-spacing:.07em;text-transform:uppercase;\
color:var(--text-secondary)}\
.total-n{margin-left:auto;font-family:ui-monospace,\"SF Mono\",Menlo,Consolas,\
\"DejaVu Sans Mono\",monospace;font-size:18px;font-weight:500;color:var(--text-primary)}";

/// The document's whole appearance.
///
/// **The palette is placed four times because the document has two readers that
/// ask differently.** Light on a bare `:root` and dark under the media query is
/// the browser's pair; dark and light again under `[data-theme]` is this app's.
/// Opened in a browser, with nothing of ours loaded, the document has
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
        BatchLine {
            n,
            seconds: 600,
            tasks: vec![],
            notes: None,
            reported: true,
            // The ordinary ending, so a test about anything else is not also a
            // test about a crash.
            outcome: BatchOutcome::Exited,
            left_behind: vec![],
        }
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
        // The strip's closed and parked cells are the two the board would have
        // filled, and a dash is what stands there instead. The batches cell is
        // a number the app measured itself and may honestly read zero, which is
        // why the assertion names the cells rather than the digit — and names
        // them as `cell` writes them, hue first and then `cell-none`. Forbidding
        // `cell-done">0<` would forbid a string this file cannot produce, and an
        // assertion that cannot fail is worse than none.
        assert!(
            !html.contains("cell-done cell-none\">0<") && !html.contains("cell-loud cell-none\">0<"),
            "no confident zero over a number nobody measured: {html}"
        );
        assert!(html.contains("cell-none\">&mdash;<"), "a dash stands where the counts would: {html}");
        assert!(
            !html.contains("<div class=\"sec\">"),
            "and no empty section standing in for one: {html}"
        );
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

    /// The whole outcome vocabulary, each with the words the document is
    /// expected to carry for it. Written out once and used by both tests below,
    /// so "every ending is named" cannot quietly become "the endings somebody
    /// remembered to list twice".
    fn vocabulary() -> Vec<(BatchOutcome, &'static str)> {
        vec![
            (BatchOutcome::Exited, "exit cleanly"),
            (BatchOutcome::Failed { code: 137 }, "exit with code 137"),
            (BatchOutcome::NoCode, "no exit code at all"),
            (BatchOutcome::Removed, "removed this session from the agents panel"),
            (BatchOutcome::HandedBack, "handed the work back"),
            (
                BatchOutcome::Unanswered { question: "Do you trust this folder?".into() },
                "Do you trust this folder?",
            ),
        ]
    }

    #[test]
    fn a_batch_that_left_no_account_still_says_how_the_run_saw_it_end() {
        // smetana-pmj, and the case the whole change exists for: a batch whose
        // agent was killed before it could write a word. The run knew what it
        // saw and wrote none of it, so one sentence — about the *agent's*
        // silence — was the entire record of a batch that died holding the
        // merge lock.
        for (outcome, words) in vocabulary() {
            let mut quiet = batch(1);
            quiet.reported = false;
            quiet.outcome = outcome.clone();
            let html = render(&report(600, None, &[quiet]));

            assert!(
                html.contains("left no account of itself"),
                "the phrase stays, and stays about the agent: {outcome:?}"
            );
            assert!(
                html.contains(words),
                "and the run's own half stands beside it for {outcome:?}: {html}"
            );
        }
    }

    #[test]
    fn every_batch_carries_the_runs_own_ending_whether_it_wrote_a_file_or_not() {
        // Independent of the account file in both directions. A batch that did
        // leave one is told the same thing about itself — the two halves are a
        // pair, not a fallback, and a reader comparing "the lead says it merged
        // three" against "the session exited with code 1" is exactly the reading
        // this is for.
        for (outcome, words) in vocabulary() {
            let mut spoke = batch(1);
            spoke.notes = Some("nothing odd".into());
            spoke.outcome = outcome.clone();
            let html = render(&report(600, None, &[spoke]));

            assert!(html.contains("nothing odd"), "the lead's own words stay: {outcome:?}");
            assert!(html.contains(words), "beside the run's: {html}");
            assert!(
                !html.contains("left no account of itself"),
                "and nothing says it was silent when it was not"
            );
        }
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
    fn a_batch_that_died_holding_the_board_says_what_it_was_holding() {
        // The line that would have saved the night: a batch killed mid-merge
        // leaves a claimed lock behind, and the next run discovers it by
        // failing to take it. Ids, because a person has to be able to go and
        // look — and the lock is called what it is, since it is the one leftover
        // that costs somebody else their run rather than this one.
        let mut killed = batch(1);
        killed.reported = false;
        killed.outcome = BatchOutcome::NoCode;
        killed.left_behind = vec![
            Leftover { id: "smetana-js4".into(), status: "in_progress".into(), lock: true },
            Leftover { id: "smetana-42v".into(), status: "ready_to_merge".into(), lock: false },
        ];
        let html = render(&report(600, None, &[killed]));

        assert!(html.contains("still held on the board"), "{html}");
        assert!(html.contains("<code>smetana-js4</code> the merge lock (in_progress)"), "{html}");
        assert!(html.contains("<code>smetana-42v</code> (ready_to_merge)"), "{html}");
        assert!(html.contains(".held{margin:0;color:var(--attn-loud)}"), "drawn loud: {html}");
    }

    #[test]
    fn a_batch_that_left_the_board_clean_draws_no_line_about_it() {
        // A line under every batch saying an actor held nothing is a line
        // nobody reads, and the one that matters would then be a line in a
        // column of them.
        let html = render(&report(600, None, &[batch(1)]));
        assert!(!html.contains("still held on the board"), "{html}");
    }

    #[test]
    fn the_document_carries_its_own_styles_and_reaches_no_network() {
        let tasks = Tasks { closed: vec![line("a-1")], parked: vec![line("a-2")] };
        let html = render(&report(60, Some(&tasks), &[batch(1)]));
        assert!(html.contains("<style>"), "self-contained: it opens with no app around it");
        assert!(!html.contains("http://") && !html.contains("https://"));
        assert!(!html.contains("<script"), "no script, in the document or out of it");
        assert!(!html.contains("<img"), "and no image");
        assert!(!html.contains("@import"), "and no stylesheet fetched from anywhere");
        assert!(!html.contains("@font-face"), "and no font file either");
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
        let rules = &html[html.find("*,*::before").expect("the first rule")..];
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
        let tasks = Tasks { closed: vec![line("a-1")], parked: vec![] };
        let html = render(&report(8040, Some(&tasks), &[]));
        assert!(html.contains("class=\"total-n\">2h 14m<"), "{html}");
        assert!(html.trim_end().ends_with("</body></html>"));
        let total = html.find("class=\"total\"").expect("the footer");
        assert!(
            html.find("<div class=\"sec\"><span>closed</span>").expect("the closed section") < total,
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
        // The cards themselves, not the whole document: every batch prints its
        // own duration under Batches, which says nothing about a task.
        let start = html.find("<div class=\"list\">").expect("the closed list");
        let end = html.find("<div class=\"sec\"><span>batches</span>").expect("the batches section");
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
        let start = html.find("<div class=\"list\">").expect("the closed list");
        let end = html.find("<div class=\"sec\"><span>batches</span>").expect("the batches section");
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
            outcome: BatchOutcome::Exited,
            left_behind: vec![],
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

    #[test]
    fn the_palette_is_the_design_systems_and_is_placed_four_times() {
        // The document has two readers that ask differently: a browser with
        // nothing of ours loaded reads `prefers-color-scheme`, and this app hands
        // it `data-theme`. The attribute has to win in both directions, so the
        // guarded media query and both attribute blocks are all three required.
        let html = render(&report(90, None, &[]));

        assert!(html.contains("--canvas:#eaeeef"), "light canvas is the design system's: {html}");
        assert!(html.contains("--canvas:#10151a"), "and so is the dark one");
        assert!(
            html.contains("@media(prefers-color-scheme:dark){:root:not([data-theme=\"light\"])"),
            "the media query is guarded, so a light app on a dark machine is not overruled"
        );
        assert!(html.contains(":root[data-theme=\"dark\"]"));
        assert!(html.contains(":root[data-theme=\"light\"]"));
        assert!(
            html.contains("color-scheme:light"),
            "the user agent's own defaults follow the document"
        );
        assert!(html.contains("color-scheme:dark"));
    }

    #[test]
    fn only_the_status_families_the_document_draws_are_emitted() {
        // Every generated document carries this stylesheet, so a family nothing
        // draws is dead weight in every one of them. Closed cards render `done`
        // and parked cards render `needs-you`; nothing here renders the other four.
        let html = render(&report(90, None, &[]));

        assert!(html.contains("--status-done-fg"));
        assert!(html.contains("--status-needs-you-fg"));
        assert!(html.contains("--attn-loud"));
        assert!(!html.contains("--status-running-"), "nothing in a finished report is running");
        assert!(!html.contains("--status-blocked-"));
        assert!(!html.contains("--status-ready-"));
        assert!(!html.contains("--status-failed-"));
        assert!(!html.contains("--attn-live"), "the batch takeaway that would have used it is not built");
    }

    #[test]
    fn the_document_carries_no_density_and_no_app_wide_scale() {
        // `ReportView.vue` refuses to carry density and the app's font size
        // across, and the reason is in its header: those are about fitting rows
        // into panels, while a document has a measure of its own. Nothing here
        // may quietly reintroduce either.
        let html = render(&report(90, None, &[]));

        assert!(!html.contains("data-density"), "{html}");
        assert!(!html.contains("--ui-scale"), "{html}");
    }

    #[test]
    fn the_document_fetches_no_font_and_names_a_fallback_stack() {
        let html = render(&report(90, None, &[]));

        assert!(!html.contains("@import"), "no stylesheet is fetched");
        assert!(!html.contains("@font-face"), "and no font file either");
        assert!(!html.contains("fonts.googleapis"));
        assert!(html.contains("ui-monospace"), "the stack opens with what every reader has");
        assert!(
            !html.contains("IBM Plex Mono"),
            "the document may fetch nothing, so it does not name a face it cannot load: {html}"
        );
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

    #[test]
    fn the_summary_strip_carries_the_four_numbers() {
        let tasks = Tasks { closed: vec![line("a-1"), line("a-2")], parked: vec![line("a-3")] };
        let html = render(&report(8040, Some(&tasks), &[batch(1)]));

        assert!(html.contains("<div class=\"strip\">"), "{html}");
        assert!(html.contains("class=\"cell-n cell-done\">2<"), "closed carries the done hue: {html}");
        assert!(html.contains("class=\"cell-n cell-loud\">1<"), "parked is the loud one: {html}");
        assert!(html.contains("class=\"cell-n\">1<"), "batches and total are plain: {html}");
        assert!(html.contains("2h 14m"), "the total is a duration, not a count");
    }

    #[test]
    fn an_unreadable_board_shows_a_dash_in_the_strip_and_never_a_zero() {
        // The rule `RunSummary.tasks` is an `Option` for, restated at the top of the
        // document where a person's eye lands first: an unreadable board and an
        // empty board are opposite facts.
        let html = render(&report(120, None, &[batch(1)]));

        assert!(html.contains("class=\"cell-n cell-done cell-none\">&mdash;<"), "{html}");
        assert!(html.contains("class=\"cell-n cell-loud cell-none\">&mdash;<"), "{html}");
        // Against the class string as `cell` writes it — hue first, then
        // `cell-none` — or the assertion forbids something unreachable and holds
        // no matter what the code does.
        assert!(
            !html.contains("cell-done cell-none\">0<"),
            "no confident zero over a number nobody measured"
        );
        assert!(!html.contains("cell-loud cell-none\">0<"));
        assert!(html.contains("could not be read"), "and the reason is still said in words");
        assert!(html.contains("class=\"notice\""), "drawn as a notice rather than as loose prose");
    }

    #[test]
    fn an_empty_section_is_omitted_and_the_strip_carries_its_zero() {
        let tasks = Tasks { closed: vec![line("a-1")], parked: vec![] };
        let html = render(&report(120, Some(&tasks), &[]));

        // The section headers, not the words: the strip labels its cells with
        // these same four nouns, and it stands above every section.
        assert!(html.contains("<div class=\"sec\"><span>closed</span>"), "{html}");
        assert!(
            !html.contains("<div class=\"sec\"><span>parked</span>"),
            "an empty section is not drawn at all: {html}"
        );
        assert!(html.contains("class=\"cell-n cell-loud cell-none\">0<"), "the strip carries it: {html}");
        assert!(!html.contains("None."), "and the old placeholder line is gone");
    }

    #[test]
    fn a_closed_card_and_a_parked_card_are_told_apart_by_more_than_colour() {
        // Status is never colour alone in this design system. The section a card
        // sits in decides its badge, since the board's own status is not carried
        // this far — `TaskLine` is an id and a title.
        let tasks = Tasks { closed: vec![line("a-1")], parked: vec![line("a-2")] };
        let html = render(&report(120, Some(&tasks), &[]));

        assert!(html.contains("class=\"badge badge-done\">done<"), "{html}");
        assert!(html.contains("class=\"badge badge-parked\">needs you<"), "{html}");
        assert!(html.contains("class=\"card card-parked\""), "the parked card's own border: {html}");
        assert!(
            html.contains(".card-parked{border-color:var(--status-needs-you-border)}"),
            "and the class is what draws it: {html}"
        );
    }

    #[test]
    fn a_tasks_own_account_is_marked_up_as_prose_and_its_id_as_a_chip() {
        let mut b = batch(1);
        b.tasks = vec![BatchTask { id: "a-1".into(), did: Some("moved `src/main.rs`".into()) }];
        let tasks = Tasks { closed: vec![line("a-1")], parked: vec![] };
        let html = render(&report(600, Some(&tasks), &[b]));

        assert!(html.contains("<span class=\"chip\">a-1</span>"), "{html}");
        assert!(html.contains("<h3>a-1 title</h3>"), "{html}");
        assert!(
            html.contains("<p class=\"body\">moved <code>src/main.rs</code></p>"),
            "the backtick rule reaches the document: {html}"
        );
    }

    #[test]
    fn a_batch_card_is_ranked_below_a_task_card_and_says_when_it_left_no_account() {
        let mut silent = batch(2);
        silent.reported = false;
        let mut spoke = batch(1);
        spoke.notes = Some("nothing odd, though `bd` was slow".into());
        let html = render(&report(1200, None, &[spoke, silent]));

        assert!(html.contains("class=\"card card-batch\""), "{html}");
        assert!(html.contains("batch 1"), "{html}");
        assert!(
            html.contains("nothing odd, though <code>bd</code> was slow"),
            "the notes go through the same rule as a task's account: {html}"
        );
        assert!(html.contains("left no account of itself"), "{html}");
    }


    #[test]
    fn the_header_and_footer_are_the_documents_only_uppercase_labels() {
        let html = render(&report(8040, None, &[]));

        assert!(html.contains("class=\"eyebrow\">smetana &middot; run report<"), "{html}");
        assert!(html.contains("class=\"meta\">"), "the run's own line: {html}");
        assert!(html.contains("class=\"total-label\">total<"), "{html}");
        assert!(html.contains("class=\"total-n\">2h 14m<"), "{html}");
    }
}
