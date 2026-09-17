//! The Reports tab's list: turning `.smetana/reports/` into one row per
//! document, without ever asking anything but the filesystem and the
//! documents themselves.
//!
//! **Pure over text, then a walk of a directory.** `parse_head` reads a whole
//! HTML document back into the handful of facts its own row needs and never
//! touches a disk; `list` is the thin, fallible half that finds the files and
//! hands each one's text to it. The split is `report.rs`'s own — that file
//! renders, this one reads back — and the two are joined by nothing but the
//! round-trip test at the bottom of this file: change what `report::render`
//! writes and this is the one place that notices.
//!
//! **Every field of a row is `Option`, and a dash is `None` rather than a
//! zero**, for the reason `report::render`'s own strip carries: an unreadable
//! board draws `&mdash;` in `closed` and `parked` so that it is never read as
//! "nothing happened", and a row here must not turn that dash back into a
//! quiet zero. The same widens to every field this parser could simply fail to
//! find — a document written by a version of this app that named something
//! differently, or a page with none of this shape at all — because "the
//! parser did not recognise this" and "the board could not be read" are both
//! answered the same way on a row: a dash, and never a number that was never
//! measured.
//!
//! **A file that will not read, or a name that does not fit, is one row
//! fewer — never a refusal.** `list` returns a plain `Vec`, the shape
//! `sessions::read::list` already settled on for the same reason: a folder
//! that is not there yet, a journal sitting beside the reports it describes, a
//! file somebody else dropped into that folder — none of them is a fact worth
//! failing a tab over, and the fewer rows say the whole of it.

use std::path::Path;

use serde::Serialize;

/// One report's own header, read back out of the document `report::render`
/// wrote — `project` is deliberately not carried past this parse: the row
/// this feeds is already scoped to one project by the folder it was read
/// from, and repeating it on every row would say nothing a person does not
/// already know from the tab they are looking at.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Head {
    pub title: Option<String>,
    pub scope: Option<String>,
    /// As `write_report` stamped it into the document's own meta line —
    /// `%Y-%m-%d %H:%M`, local time, and never reparsed into anything else.
    pub finished: Option<String>,
    pub closed: Option<u32>,
    pub parked: Option<u32>,
    pub batches: Option<u32>,
    /// A human duration exactly as `report::human` wrote it — `"1h 12m"`,
    /// `"45m"`, `"30s"` — and never turned back into seconds: nothing this row
    /// does needs to compare two of them, and the string is what a person
    /// reads.
    pub total: Option<String>,
}

/// One file's row: the head above, flattened onto the wire so the front end
/// sees one flat object, plus what the filename itself carries and the
/// document repeats nowhere — where it is, what it is called, and when it was
/// written, read off the name rather than off `finished` so a row still sorts
/// correctly even for a document whose meta line this parser could not read
/// at all.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportEntry {
    /// Absolute.
    pub path: String,
    /// The bare filename — what tells two reports of the same second apart,
    /// and the tiebreaker `reportsPage.js`'s sort uses under an equal `stamp`.
    pub file: String,
    /// `"YYYY-MM-DDTHH:MM:SS"`, local time, parsed out of the filename rather
    /// than out of the document: `claim_report` names every file this way
    /// whether or not the render inside it ever reaches a person, so this is
    /// the one date a row can always have.
    pub stamp: String,
    #[serde(flatten)]
    pub head: Head,
}

/// The inverse of `report::escape`, applied to whatever this file lifts back
/// out of a document. `&amp;` is undone last for the reason `escape`'s own
/// header gives in the other direction: undoing it first would turn a
/// document's `&lt;` — itself the escaped form of a real `<` — back into
/// `&lt;`'s own three letters, rather than into the character it stood for.
fn unescape(text: &str) -> String {
    text.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&amp;", "&")
}

/// The text strictly between the first `start` and the next `end` after it,
/// or `None` when either is missing. Every reader below is one call of this
/// against one piece of `report.rs`'s own markup — there is no HTML parser
/// here, on the same argument `reportTheme.js`'s header makes for the front
/// end's own scanner: this reads one writer's output, not arbitrary HTML.
fn between<'a>(html: &'a str, start: &str, end: &str) -> Option<&'a str> {
    let after = html.find(start).map(|at| &html[at + start.len()..])?;
    let stop = after.find(end)?;
    Some(&after[..stop])
}

/// `<title>` and `<h1>` carry the same words in `report::render`, so either
/// would do; the title is read because it is first in the document and
/// therefore cheapest to find in a large one.
fn parse_title(html: &str) -> Option<String> {
    between(html, "<title>", "</title>").map(unescape)
}

/// The one line the header writes as `PROJECT &middot; SCOPE &middot; finished
/// FINISHED` — `render`'s own three `push_str` calls, joined back apart.
/// `project` is read only to be discarded: see `Head`'s own header for why the
/// row has no field for it. A meta line missing, or not shaped as exactly
/// three parts the middle one of which is not a project this parser has no
/// business examining, answers both fields `None` together rather than one at
/// a time — a document that has been through anything other than
/// `report::render` is not a report to guess the rest of.
fn parse_meta(html: &str) -> (Option<String>, Option<String>) {
    let Some(meta) = between(html, "<p class=\"meta\">", "</p>") else {
        return (None, None);
    };
    let parts: Vec<&str> = meta.split(" &middot; ").collect();
    let [_project, scope, finished] = parts[..] else {
        return (None, None);
    };
    let Some(finished) = finished.strip_prefix("finished ") else {
        return (None, None);
    };
    (Some(unescape(scope)), Some(unescape(finished)))
}

/// One cell of the strip, by the label `cell` wrote beside it — the raw text
/// between the value span's opening `>` and its own closing `<`, untouched:
/// `&mdash;` for an unread board, digits for a count, a duration's own letters
/// for `total`. `None` when this document carries no cell of that name at
/// all, which is the whole of the answer for a page that is not a report.
fn cell_text<'a>(html: &'a str, label: &str) -> Option<&'a str> {
    let marker = format!("<span class=\"cell-label\">{label}</span><span class=\"cell-n");
    let after = html.find(&marker).map(|at| &html[at + marker.len()..])?;
    // The opening span's own classes run up to the next `>`; `cell`
    // (`report.rs`) writes zero or more of them before it, and this is what
    // steps past whichever it wrote without naming them here too.
    let at_gt = after.find('>')?;
    let value = &after[at_gt + 1..];
    let at_lt = value.find('<')?;
    Some(&value[..at_lt])
}

/// `&mdash;` reads as `None`, matching `cell`'s own reason for writing it: a
/// board that could not be read is not a zero. Anything that does not parse
/// as a plain non-negative count is also `None`, since a strip this parser
/// half-recognises is not one to guess a number for.
fn cell_count(html: &str, label: &str) -> Option<u32> {
    match cell_text(html, label)? {
        "&mdash;" => None,
        text => text.parse().ok(),
    }
}

/// The `total` cell, which is prose rather than a count and is never `&mdash;`
/// in a document `report::render` wrote — `human()` always has an answer,
/// even for zero seconds. Read the same way regardless, since a document this
/// parser only partly recognises is exactly where that assumption should not
/// be leaned on.
fn cell_duration(html: &str) -> Option<String> {
    match cell_text(html, "total")? {
        "&mdash;" => None,
        text => Some(unescape(text)),
    }
}

/// A document's header, read back into the shape a row wants. Every one of
/// the seven fields is found on its own, so a document missing one piece of
/// this shape — an older render, a stray page, one field this parser has not
/// caught up with — loses only that field rather than the whole row.
pub fn parse_head(html: &str) -> Head {
    let title = parse_title(html);
    let (scope, finished) = parse_meta(html);
    Head {
        title,
        scope,
        finished,
        closed: cell_count(html, "closed"),
        parked: cell_count(html, "parked"),
        batches: cell_count(html, "batches"),
        total: cell_duration(html),
    }
}

/// The stamp's own seventeen characters — `YYYY-MM-DD-HHMMSS` — out of a
/// report's filename, and `None` for anything that is not exactly that
/// followed by `.html` or by `-<n>.html`. Checked byte by byte rather than
/// parsed as a date: this is a filename shape `claim_report` chose, and a
/// clock that had drifted still wrote a report worth listing.
fn date_stamp(file_name: &str) -> Option<&str> {
    let stem = file_name.strip_suffix(".html")?;
    let head = stem.get(..17)?;
    let rest = stem.get(17..)?;
    if !rest.is_empty() {
        let suffix = rest.strip_prefix('-')?;
        if suffix.is_empty() || !suffix.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
    }
    is_stamp_shape(head).then_some(head)
}

/// `dddd-dd-dd-dddddd`, and nothing else — the shape `write_report`'s two
/// `strftime` calls always produce together.
fn is_stamp_shape(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 17
        && b[..4].iter().all(u8::is_ascii_digit)
        && b[4] == b'-'
        && b[5..7].iter().all(u8::is_ascii_digit)
        && b[7] == b'-'
        && b[8..10].iter().all(u8::is_ascii_digit)
        && b[10] == b'-'
        && b[11..17].iter().all(u8::is_ascii_digit)
}

/// The filename's own stamp, joined into the `T`-separated shape the row
/// travels as. `date` is exactly the seventeen bytes `date_stamp` validated,
/// so every index below lands inside it.
fn stamp_field(date: &str) -> String {
    format!("{}T{}:{}:{}", &date[..10], &date[11..13], &date[13..15], &date[15..17])
}

/// Every report under `<project>/.smetana/reports/`, one row per file whose
/// name fits the shape `claim_report` writes. A missing folder, a folder that
/// cannot be read, a `journal-*.log` beside the reports it describes, a file
/// this parser cannot open — every one of them is fewer rows, never a
/// refusal, for the reason this file's own header gives. The order is not
/// this function's to decide: `reportsPage.js` sorts what it is handed.
pub fn list(root: &Path) -> Vec<ReportEntry> {
    let dir = root.join(".smetana").join("reports");
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut rows = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let Some(date) = date_stamp(file_name) else {
            continue;
        };
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        rows.push(ReportEntry {
            path: path.to_string_lossy().into_owned(),
            file: file_name.to_owned(),
            stamp: stamp_field(date),
            head: parse_head(&text),
        });
    }
    rows
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::runs::report::{render, BatchLine, BatchOutcome, BatchTask, RunReport};
    use crate::runs::summary::{TaskLine, Tasks};

    fn line(id: &str) -> TaskLine {
        TaskLine { id: id.into(), title: format!("{id} title") }
    }

    fn batch(n: u32) -> BatchLine {
        BatchLine {
            n,
            seconds: 8040,
            tasks: vec![BatchTask { id: "a-1".into(), did: None }],
            notes: None,
            reported: true,
            outcome: BatchOutcome::Exited,
            left_behind: vec![],
            lock_released: None,
        }
    }

    fn report<'a>(tasks: Option<&'a Tasks>, batches: &'a [BatchLine]) -> RunReport<'a> {
        RunReport {
            title: "Task report",
            project: "my-project",
            scope: "the queue",
            finished: "2026-08-12 14:31",
            seconds: 8040,
            tasks,
            batches,
            journal: None,
        }
    }

    #[test]
    fn a_rendered_document_reads_back_the_same_head_it_was_given() {
        let tasks = Tasks { closed: vec![line("a-1"), line("a-2")], parked: vec![line("a-3")] };
        let batches = [batch(1)];
        let html = render(&report(Some(&tasks), &batches));

        let head = parse_head(&html);
        assert_eq!(head.title.as_deref(), Some("Task report"));
        assert_eq!(head.scope.as_deref(), Some("the queue"));
        assert_eq!(head.finished.as_deref(), Some("2026-08-12 14:31"));
        assert_eq!(head.closed, Some(2));
        assert_eq!(head.parked, Some(1));
        assert_eq!(head.batches, Some(1));
        assert_eq!(head.total.as_deref(), Some("2h 14m"));
    }

    #[test]
    fn a_scope_with_an_ampersand_comes_back_unescaped() {
        let batches = [batch(1)];
        let with_amp = RunReport { scope: "backend & frontend", ..report(None, &batches) };
        let html = render(&with_amp);

        let head = parse_head(&html);
        assert_eq!(head.scope.as_deref(), Some("backend & frontend"));
    }

    #[test]
    fn an_unread_board_gives_a_dash_and_the_dash_reads_back_as_none() {
        let batches = [batch(1)];
        let html = render(&report(None, &batches));

        let head = parse_head(&html);
        assert_eq!(head.closed, None, "the strip's dash must not become a zero");
        assert_eq!(head.parked, None);
        // The board being unreadable says nothing about the batches, which
        // are counted independently of it.
        assert_eq!(head.batches, Some(1));
    }

    #[test]
    fn a_page_that_is_not_a_report_answers_every_field_none() {
        let html = "<!doctype html><html><body><h1>Hello</h1></body></html>";

        let head = parse_head(html);
        assert_eq!(head, Head::default());
    }

    #[test]
    fn a_plain_stamp_reads_as_the_t_joined_shape() {
        assert_eq!(date_stamp("2026-09-17-143205.html"), Some("2026-09-17-143205"));
        assert_eq!(stamp_field("2026-09-17-143205"), "2026-09-17T14:32:05");
    }

    #[test]
    fn a_second_report_in_the_same_second_still_reads_its_stamp() {
        assert_eq!(date_stamp("2026-09-17-143205-2.html"), Some("2026-09-17-143205"));
    }

    #[test]
    fn a_journal_and_a_stray_name_are_not_a_stamp() {
        assert_eq!(date_stamp("journal-2026-09-17-143205.log"), None);
        assert_eq!(date_stamp("notes.html"), None);
        assert_eq!(date_stamp("2026-09-17-143205-abc.html"), None);
    }

    #[test]
    fn listing_skips_journals_and_survives_a_missing_folder() {
        let dir = tempdir();
        let reports = dir.join(".smetana").join("reports");
        std::fs::create_dir_all(&reports).unwrap();
        std::fs::write(reports.join("2026-09-17-143205.html"), "<title>Task report</title>").unwrap();
        std::fs::write(reports.join("journal-2026-09-17-143205.log"), "not a report").unwrap();

        let rows = list(&dir);
        assert_eq!(rows.len(), 1, "the journal must not turn into a row");
        assert_eq!(rows[0].file, "2026-09-17-143205.html");
        assert_eq!(rows[0].stamp, "2026-09-17T14:32:05");
        assert_eq!(rows[0].head.title.as_deref(), Some("Task report"));

        let missing = dir.join("nowhere");
        assert_eq!(list(&missing), Vec::new(), "no folder is fewer rows, not a refusal");

        std::fs::remove_dir_all(&dir).ok();
    }

    /// A private temp folder under the target directory, named for the test
    /// process so two runs of the suite in parallel cannot collide.
    fn tempdir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("smetana-reports-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }
}
