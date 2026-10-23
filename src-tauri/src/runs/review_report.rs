//! The branch review's document: `{report}.json` in, `{report}.html` out.
//!
//! The agent used to lay the HTML out itself, and every review looked
//! different. It now writes data — `docs/design/reports/code-review/schema.json`,
//! pasted whole into the `reviewing-branch-changes` skill — and this module
//! draws it, the way `report.rs` draws a run. The two documents are one system:
//! the same `<style>` byte for byte (`report::REPORTS_CSS`, taken, never
//! copied), the same header, strip, cards and footer.
//!
//! The split is `report.rs`'s as well: [`render`] and [`render_fallback`] are
//! pure — a value or a string in, a string out — and the disk is left to
//! [`write_html`], which the terminal worker calls when a review session ends.
//! The styling exception `report.rs` documents holds here word for word: no
//! external stylesheet, no network font, no script and no image.
//!
//! **Parsing is lax in one direction.** Fields the schema does not name are
//! ignored (serde's default, deliberately not `deny_unknown_fields`: an agent
//! that adds a note is not worth a failed report), and a field the schema
//! requires that is missing is an error. An error is not a missing tab: the
//! caller draws [`render_fallback`] instead, because an empty tab or a read
//! error is worse than a page that says what happened.

use std::fmt::Write as _;
use std::path::{Component, Path};

use crate::terminal::model::SessionWork;

use serde::Deserialize;

use crate::runs::report::{escape, prose, REPORTS_CSS};

#[derive(Debug, Clone, Deserialize)]
pub struct Review {
    pub score: u32,
    pub pairs: Vec<Pair>,
    pub findings: Vec<Finding>,
    pub reviewer: String,
    pub date: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Pair {
    pub repo: String,
    pub base: String,
    pub head: String,
    pub files: u32,
    pub added: u32,
    pub removed: u32,
    pub hazards: bool,
    #[serde(default)]
    pub rules_source: Option<String>,
    /// Drawn only by the zero-findings state.
    #[serde(default)]
    pub reviewed_files: Vec<ReviewedFile>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ReviewedFile {
    pub path: String,
    pub added: u32,
    pub removed: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    Blocking,
    Serious,
    Minor,
}

impl Level {
    const ALL: [Level; 3] = [Level::Blocking, Level::Serious, Level::Minor];

    fn word(self) -> &'static str {
        match self {
            Level::Blocking => "Blocking",
            Level::Serious => "Serious",
            Level::Minor => "Minor",
        }
    }

    fn slug(self) -> &'static str {
        match self {
            Level::Blocking => "blocking",
            Level::Serious => "serious",
            Level::Minor => "minor",
        }
    }

    /// The prefix of a finding's id: `B1`, `S2`, `M3`.
    fn letter(self) -> char {
        match self {
            Level::Blocking => 'B',
            Level::Serious => 'S',
            Level::Minor => 'M',
        }
    }

    fn glyph(self) -> &'static str {
        match self {
            Level::Blocking => "\u{25A0}",
            Level::Serious => "\u{25B2}",
            Level::Minor => "\u{25CB}",
        }
    }

    /// The strip cell's tone, which is the status tone the level borrows.
    fn tone(self) -> &'static str {
        match self {
            Level::Blocking => "tone-failed",
            Level::Serious => "tone-needs-you",
            Level::Minor => "tone-neutral",
        }
    }

    fn hint(self) -> &'static str {
        match self {
            Level::Blocking => "Must be fixed before merge",
            Level::Serious => "Fix before merge or file a follow-up",
            Level::Minor => "Optional",
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Finding {
    pub level: Level,
    pub title: String,
    pub repo: String,
    pub path: String,
    pub line: u32,
    pub defect: String,
    pub scenario: String,
    pub fix: String,
    #[serde(default)]
    pub code: Option<Code>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Code {
    /// The caption; the finding's own path when absent.
    #[serde(default)]
    pub path: Option<String>,
    pub start_line: u32,
    pub lines: Vec<String>,
    /// Absolute line numbers.
    #[serde(default)]
    pub highlight: Vec<u32>,
}

/// Where a readiness percentage falls on the scale `reviewing-branch-changes`
/// §7 states: 90 and up, 70 to 89, below 70.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Range {
    High,
    Mid,
    Low,
}

impl Range {
    fn of(score: u32) -> Range {
        match score {
            90.. => Range::High,
            70..=89 => Range::Mid,
            _ => Range::Low,
        }
    }

    fn glyph(self) -> &'static str {
        match self {
            Range::High => "\u{25CF}",
            Range::Mid => "\u{25B2}",
            Range::Low => "\u{25A0}",
        }
    }

    /// The text colour class of the glyph and the label.
    fn colour(self) -> &'static str {
        match self {
            Range::High => "c-ok",
            Range::Mid => "c-serious",
            Range::Low => "c-blocking",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Range::High => "Ready as is",
            Range::Mid => "Serious issues, nothing blocking",
            Range::Low => "Blocking issue \u{2014} returned for rework",
        }
    }

    /// The legend's range text, already escaped (`<70` carries a `<`).
    fn span(self) -> &'static str {
        match self {
            Range::High => "90\u{2013}100",
            Range::Mid => "70\u{2013}89",
            Range::Low => "&lt;70",
        }
    }

    /// The same range in words, for the bar's `aria-label`.
    fn spoken(self) -> &'static str {
        match self {
            Range::High => "90 to 100",
            Range::Mid => "70 to 89",
            Range::Low => "0 to 69",
        }
    }

    fn slug(self) -> &'static str {
        match self {
            Range::High => "high",
            Range::Mid => "mid",
            Range::Low => "low",
        }
    }
}

/// Parse the agent's file. The error is serde's own sentence, which is what the
/// fallback page prints: it names the missing field.
pub fn parse(text: &str) -> Result<Review, String> {
    let review: Review = serde_json::from_str(text).map_err(|e| e.to_string())?;
    // `minItems: 1` in the schema: a review of nothing has no header to draw.
    if review.pairs.is_empty() {
        return Err("`pairs` must hold at least one entry".to_owned());
    }
    Ok(review)
}

/// The footer's date: the schema asks for local time already
/// formatted, and an agent that writes an ISO stamp anyway gets the design's
/// `YYYY-MM-DD HH:mm` rather than a `T` and a zone.
fn date_text(date: &str) -> String {
    let d = date.trim();
    let bytes = d.as_bytes();
    let iso = d.len() >= 16
        && d.is_char_boundary(16)
        && bytes[10] == b'T'
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes[13] == b':';
    if iso {
        format!("{} {}", &d[..10], &d[11..16])
    } else {
        d.to_owned()
    }
}

/// Whether the score contradicts the findings in the one way the README names:
/// something blocking and a readiness that says it can land.
fn contradicts(review: &Review) -> bool {
    review.score >= 70 && review.findings.iter().any(|f| f.level == Level::Blocking)
}

/// The finding's `Fails when` text: backtick spans become `.io` rather than
/// `<code>`. Derived from [`prose`] instead of repeating its loop — everything
/// else in the output was escaped first, so the only `<code>` tags there are the
/// ones `prose` wrote.
fn io_prose(text: &str) -> String {
    prose(text).replace("<code>", "<span class=\"io\">").replace("</code>", "</span>")
}

fn plural_files(n: u32) -> String {
    if n == 1 {
        "1 file".to_owned()
    } else {
        format!("{n} files")
    }
}

pub fn render(review: &Review) -> String {
    if contradicts(review) {
        // A warning and not a refusal: the score is the reviewer's judgement
        // and the document still says exactly what it was given.
        log::warn!(
            "[review-report] score {} contradicts {} blocking finding(s); drawn as written",
            review.score,
            review.findings.iter().filter(|f| f.level == Level::Blocking).count()
        );
    }
    let score = review.score.min(100);
    let repos = review.pairs.iter().map(|p| p.repo.as_str()).collect::<Vec<_>>().join(", ");

    let mut out = String::new();
    document_open(&mut out, &format!("Code review \u{00B7} {repos} \u{00B7} {score}%"));

    out.push_str("<header class=\"head\">\n<div class=\"eyebrow\">smetana \u{00B7} code review</div>\n<h1>");
    out.push_str(&escape(&repos));
    out.push_str("</h1>\n<ul class=\"refs card\" aria-label=\"Compared refs\">\n");
    for pair in &review.pairs {
        out.push_str("<li class=\"ref\">\n<span class=\"ref-repo\">");
        out.push_str(&escape(&pair.repo));
        out.push_str("</span>\n<span class=\"ref-pair\">");
        out.push_str(&escape(&pair.base));
        out.push_str("<span class=\"arrow\" aria-label=\"from\">\u{2190}</span>");
        out.push_str(&escape(&pair.head));
        let _ = write!(
            out,
            "</span>\n<span class=\"ref-stat\">{} \u{00B7} <span class=\"add\">+{}</span> <span class=\"del\">\u{2212}{}</span></span>\n</li>\n",
            plural_files(pair.files),
            pair.added,
            pair.removed
        );
    }
    out.push_str("</ul>\n</header>\n");

    verdict(&mut out, score);
    strip(&mut out, &review.findings);

    if review.findings.is_empty() {
        clean(&mut out, &review.pairs);
    } else {
        for level in Level::ALL {
            group(&mut out, level, &review.findings);
        }
    }

    footer(&mut out, review);
    out.push_str("</main>\n</body>\n</html>\n");
    out
}

/// Everything up to and including `<main class="page">`, shared by the report
/// and the fallback page. `title` is plain text and is escaped here.
fn document_open(out: &mut String, title: &str) {
    out.push_str("<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n");
    out.push_str("<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n<title>");
    out.push_str(&escape(title));
    out.push_str("</title>\n<style>\n");
    out.push_str(REPORTS_CSS);
    out.push_str("</style>\n</head>\n<body>\n<main class=\"page\">\n");
}

fn verdict(out: &mut String, score: u32) {
    let range = Range::of(score);
    out.push_str("<section class=\"verdict card\" aria-labelledby=\"verdict-h\">\n<div class=\"score\">\n");
    out.push_str("<div class=\"eyebrow\" id=\"verdict-h\">Readiness</div>\n");
    let _ = write!(out, "<div class=\"score-num\">{score}<small>%</small></div>\n");
    let _ = write!(
        out,
        "<div class=\"score-label {}\"><span class=\"g\" aria-hidden=\"true\">{}</span>{}</div>\n</div>\n",
        range.colour(),
        range.glyph(),
        range.label()
    );
    out.push_str("<div class=\"scale\">\n");
    let _ = write!(
        out,
        "<div class=\"bar\" role=\"img\" aria-label=\"Readiness {score} percent, range {}\">\n",
        range.spoken()
    );
    for r in [Range::Low, Range::Mid, Range::High] {
        let current = if r == range { " is-current" } else { "" };
        let _ = write!(out, "<div class=\"band b-{}{}\"></div>\n", r.slug(), current);
    }
    // Pinned inside the bar at the far end, where a marker centred on 100%
    // would hang out of the card.
    if score >= 98 {
        let _ = write!(
            out,
            "<div class=\"marker\" style=\"left:calc(100% - 2px)\"><span style=\"transform:translateX(-90%)\">{score}</span></div>\n"
        );
    } else {
        let _ = write!(out, "<div class=\"marker\" style=\"left:{score}%\"><span>{score}</span></div>\n");
    }
    out.push_str("</div>\n<div class=\"ticks\" aria-hidden=\"true\"><span style=\"left:0\">0</span><span style=\"left:70%\">70</span><span style=\"left:90%\">90</span><span style=\"left:100%\">100</span></div>\n<ul class=\"legend\">\n");
    for r in [Range::High, Range::Mid, Range::Low] {
        let current = r == range;
        let _ = write!(
            out,
            "<li class=\"l-{}{}\"><span class=\"g {}\" aria-hidden=\"true\">{}</span><span class=\"range\">{}</span><span>{}</span>",
            r.slug(),
            if current { " is-current" } else { "" },
            r.colour(),
            r.glyph(),
            r.span(),
            r.label()
        );
        if current {
            let _ = write!(out, "<span class=\"now {}\">Current</span></li>\n", r.colour());
        } else {
            out.push_str("<span class=\"now\"></span></li>\n");
        }
    }
    out.push_str("</ul>\n</div>\n</section>\n");
}

fn count(findings: &[Finding], level: Level) -> usize {
    findings.iter().filter(|f| f.level == level).count()
}

fn strip(out: &mut String, findings: &[Finding]) {
    let total = findings.len();
    // `<nav>` only when a cell has somewhere to go; an all-zero strip is a
    // plain `<div>`.
    let tag = if total > 0 { "nav" } else { "div" };
    let _ = write!(out, "<{tag} class=\"strip\" aria-label=\"Findings by level\">\n");
    for level in Level::ALL {
        let n = count(findings, level);
        let label = format!(
            "<span class=\"cell-label\"><span class=\"g\" aria-hidden=\"true\">{}</span>{}</span><span class=\"cell-n\">{n}</span>",
            level.glyph(),
            level.word()
        );
        if n == 0 {
            let _ = write!(out, "<div class=\"cell is-zero\">{label}</div>\n");
        } else {
            // Blocking is the one loud cell of the document.
            let loud = if level == Level::Blocking { " is-loud" } else { "" };
            let _ = write!(
                out,
                "<a class=\"cell {}{loud}\" href=\"#{}\">{label}</a>\n",
                level.tone(),
                level.slug()
            );
        }
    }
    let zero = if total == 0 { " is-zero" } else { "" };
    let _ = write!(
        out,
        "<div class=\"cell{zero}\"><span class=\"cell-label\">Findings</span><span class=\"cell-n\">{total}</span></div>\n</{tag}>\n"
    );
}

fn group(out: &mut String, level: Level, findings: &[Finding]) {
    let of_level: Vec<&Finding> = findings.iter().filter(|f| f.level == level).collect();
    if of_level.is_empty() {
        return;
    }
    let slug = level.slug();
    let class = format!("c-{slug}");
    let _ = write!(
        out,
        "<section class=\"group group-{slug}\" id=\"{slug}\" aria-labelledby=\"{slug}-h\">\n<div class=\"group-head\">\n<span class=\"g {class}\" aria-hidden=\"true\">{}</span>\n<h2 id=\"{slug}-h\">{}</h2>\n<span class=\"count\">{}</span>\n<span class=\"hint\">{}</span>\n</div>\n",
        level.glyph(),
        level.word(),
        of_level.len(),
        level.hint()
    );
    for (i, finding) in of_level.iter().enumerate() {
        let id = format!("{}{}", level.letter(), i + 1);
        finding_card(out, finding, &id);
    }
    out.push_str("</section>\n");
}

fn finding_card(out: &mut String, f: &Finding, id: &str) {
    let class = format!("c-{}", f.level.slug());
    let _ = write!(
        out,
        "<article class=\"finding card\" id=\"{id}\">\n<div class=\"f-top\">\n<div class=\"f-title\"><span class=\"f-id {class}\">{} {id}</span><h3>",
        f.level.glyph()
    );
    out.push_str(&prose(&f.title));
    out.push_str("</h3></div>\n<div class=\"f-loc\"><span class=\"repo\">");
    out.push_str(&escape(&f.repo));
    out.push_str("/</span>");
    out.push_str(&escape(&f.path));
    let _ = write!(out, ":<span class=\"line\">{}</span></div>\n</div>\n<dl class=\"f-body\">\n", f.line);
    out.push_str("<dt>Defect</dt>\n<dd class=\"defect\">");
    out.push_str(&prose(&f.defect));
    out.push_str("</dd>\n<dt>Fails when</dt>\n<dd>");
    out.push_str(&io_prose(&f.scenario));
    out.push_str("</dd>\n<dt>Fix</dt>\n<dd>");
    out.push_str(&prose(&f.fix));
    out.push_str("</dd>\n");
    if let Some(code) = &f.code {
        code_quote(out, code, &f.path);
    }
    out.push_str("</dl>\n</article>\n");
}

/// The quoted lines. **No whitespace between the `span.l` elements**: the `pre`
/// would draw it as blank lines.
fn code_quote(out: &mut String, code: &Code, finding_path: &str) {
    out.push_str("<figure class=\"code\">\n<figcaption>");
    out.push_str(&escape(code.path.as_deref().unwrap_or(finding_path)));
    out.push_str("</figcaption>\n<pre>");
    for (i, text) in code.lines.iter().enumerate() {
        let n = code.start_line.saturating_add(i as u32);
        let hl = if code.highlight.contains(&n) { " hl" } else { "" };
        let _ = write!(out, "<span class=\"l{hl}\"><span class=\"n\">{n}</span>");
        out.push_str(&escape(text));
        out.push_str("</span>");
    }
    out.push_str("</pre>\n</figure>\n");
}

fn clean(out: &mut String, pairs: &[Pair]) {
    out.push_str("<section class=\"clean card\" aria-labelledby=\"clean-h\">\n<div class=\"clean-head\">\n<span class=\"g c-ok\" aria-hidden=\"true\">\u{25CF}</span>\n<h2 id=\"clean-h\">No findings</h2>\n</div>\n");
    // "against the project rules" is a claim about hazards, so it is only made
    // when some pair was reviewed with them.
    let against = if pairs.iter().any(|p| p.hazards) { " against the project rules" } else { "" };
    let _ = write!(
        out,
        "<p>Every changed file was read{against}. Nothing blocking, serious or minor.</p>\n"
    );
    let reviewed: Vec<(&Pair, &ReviewedFile)> =
        pairs.iter().flat_map(|p| p.reviewed_files.iter().map(move |f| (p, f))).collect();
    if !reviewed.is_empty() {
        let several = pairs.len() > 1;
        out.push_str("<div class=\"eyebrow\">Files reviewed</div>\n<ul class=\"files\">\n");
        for (pair, file) in reviewed {
            out.push_str("<li><span>");
            if several {
                out.push_str(&escape(&pair.repo));
                out.push('/');
            }
            out.push_str(&escape(&file.path));
            let _ = write!(
                out,
                "</span><span><span class=\"add\">+{}</span> <span class=\"del\">\u{2212}{}</span></span></li>\n",
                file.added, file.removed
            );
        }
        out.push_str("</ul>\n");
    }
    out.push_str("</section>\n");
}

fn footer(out: &mut String, review: &Review) {
    out.push_str("<footer class=\"foot\">\n<dl>\n<dt>Project rules</dt>\n<dd><span class=\"mono\">[merge].hazards</span>");
    let several = review.pairs.len() > 1;
    for pair in &review.pairs {
        let source = pair.rules_source.as_deref().filter(|s| !s.trim().is_empty());
        let verdict = if pair.hazards { "<span class=\"yes\">yes</span>" } else { "<span class=\"no\">no</span>" };
        if several {
            // `· repo yes`, then the source when there is one.
            out.push_str(" \u{00B7} ");
            out.push_str(&escape(&pair.repo));
            out.push(' ');
            out.push_str(verdict);
            if let Some(source) = source {
                out.push_str(" <span class=\"mono\">");
                out.push_str(&escape(source));
                out.push_str("</span>");
            }
        } else {
            // The reference's single-repo shape: `hazards yes · source`.
            out.push(' ');
            out.push_str(verdict);
            if let Some(source) = source {
                out.push_str(" \u{00B7} <span class=\"mono\">");
                out.push_str(&escape(source));
                out.push_str("</span>");
            }
        }
    }
    out.push_str("</dd>\n<dt>Reviewer</dt>\n<dd><span class=\"mono\">");
    out.push_str(&escape(&review.reviewer));
    out.push_str("</span></dd>\n<dt>Date</dt>\n<dd><span class=\"mono\">");
    out.push_str(&escape(&date_text(&review.date)));
    out.push_str("</span></dd>\n<dt>Refs</dt>\n<dd>\n");
    for pair in &review.pairs {
        out.push_str("<div class=\"mono\">");
        out.push_str(&escape(&pair.repo));
        out.push_str(" \u{00B7} ");
        out.push_str(&escape(&pair.base));
        out.push_str(" \u{2190} ");
        out.push_str(&escape(&pair.head));
        out.push_str("</div>\n");
    }
    out.push_str("</dd>\n</dl>\n</footer>\n");
}

/// The page that stands in when there is no report to draw: the agent wrote no
/// JSON, or wrote something that does not parse. Same document, same
/// stylesheet, and it names the reason and both files.
pub fn render_fallback(reason: &str, json_path: &str, md_path: &str) -> String {
    let mut out = String::new();
    document_open(&mut out, "Code review \u{00B7} report could not be built");
    out.push_str("<header class=\"head\">\n<div class=\"eyebrow\">smetana \u{00B7} code review</div>\n<h1>Report could not be built</h1>\n</header>\n");
    out.push_str("<section class=\"clean card\" aria-labelledby=\"failed-h\">\n<div class=\"clean-head\">\n<span class=\"g c-blocking\" aria-hidden=\"true\">\u{25A0}</span>\n<h2 id=\"failed-h\">No report to show</h2>\n</div>\n");
    out.push_str("<p>The review session ended, but the report it was meant to leave could not be turned into a page.</p>\n<dl class=\"f-body\">\n<dt>Reason</dt>\n<dd>");
    out.push_str(&escape(reason));
    out.push_str("</dd>\n<dt>Data</dt>\n<dd><span class=\"mono\">");
    out.push_str(&escape(json_path));
    out.push_str("</span></dd>\n<dt>Text</dt>\n<dd><span class=\"mono\">");
    out.push_str(&escape(md_path));
    out.push_str("</span></dd>\n</dl>\n</section>\n</main>\n</body>\n</html>\n");
    out
}

/// The hook both session workers call when a session's output has ended,
/// before the exited state is emitted: a `ReviewBranch` gets its page drawn,
/// anything else is left alone. A failure is a log line and nothing more.
pub fn on_session_end(work: &SessionWork, project: &str) {
    if let SessionWork::ReviewBranch { report } = work {
        if let Err(e) = write_html(Path::new(project), report) {
            log::warn!("[review-report] {report}: {e}");
        }
    }
}

/// Draw the page for a finished review: read `{report}.json` under `project`,
/// and write `{report}.html` beside it — the report if it parsed, the fallback
/// page if it did not, so that a file is on disk whatever the agent did.
///
/// `report` is the project-relative path the app composed, without extension.
/// The only error is the one that leaves nothing on disk: a path that leaves
/// the project, or a write that failed.
pub fn write_html(project: &Path, report: &str) -> Result<(), String> {
    let relative = Path::new(report);
    if relative.is_absolute() || relative.has_root() || relative.components().any(|c| matches!(c, Component::ParentDir | Component::Prefix(_) | Component::RootDir)) {
        return Err(format!("report path {report:?} is not inside the project"));
    }
    let json = project.join(format!("{report}.json"));
    let html = project.join(format!("{report}.html"));

    let page = match std::fs::read_to_string(&json) {
        Ok(text) => match parse(&text) {
            Ok(review) => render(&review),
            Err(e) => render_fallback(&format!("The JSON could not be read: {e}"), &format!("{report}.json"), &format!("{report}.md")),
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            render_fallback("The JSON file was not found.", &format!("{report}.json"), &format!("{report}.md"))
        }
        Err(e) => render_fallback(&format!("The JSON file could not be read: {e}"), &format!("{report}.json"), &format!("{report}.md")),
    };
    if let Some(dir) = html.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    std::fs::write(&html, page).map_err(|e| format!("{}: {e}", html.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const TYPICAL: &str = include_str!("../../../docs/design/reports/code-review/examples/typical.json");
    const CLEAN: &str = include_str!("../../../docs/design/reports/code-review/examples/no-findings.json");

    fn typical() -> Review {
        parse(TYPICAL).expect("the typical fixture parses")
    }

    fn pair(repo: &str) -> Pair {
        Pair {
            repo: repo.into(),
            base: "main".into(),
            head: "feature/x".into(),
            files: 2,
            added: 5,
            removed: 1,
            hazards: true,
            rules_source: None,
            reviewed_files: vec![],
        }
    }

    fn finding(level: Level) -> Finding {
        Finding {
            level,
            title: "A title".into(),
            repo: "r".into(),
            path: "src/a.rs".into(),
            line: 3,
            defect: "It is wrong.".into(),
            scenario: "input \u{2192} result".into(),
            fix: "Fix it.".into(),
            code: None,
        }
    }

    fn review(pairs: Vec<Pair>, findings: Vec<Finding>, score: u32) -> Review {
        Review { score, pairs, findings, reviewer: "claude-2".into(), date: "2026-10-23 14:02".into() }
    }

    fn style_of(html: &str) -> &str {
        let open = html.find("<style>").expect("a style element") + "<style>".len();
        let close = html[open..].find("</style>").expect("the end of the style element");
        &html[open..open + close]
    }

    fn body_of(html: &str) -> &str {
        &html[html.find("<main").expect("the page")..]
    }

    // -- the shared document -----------------------------------------------------

    #[test]
    fn the_style_element_is_the_shared_stylesheet_byte_for_byte() {
        for html in [
            render(&typical()),
            render(&parse(CLEAN).unwrap()),
            render_fallback("x", "a.json", "a.md"),
        ] {
            assert_eq!(style_of(&html).trim_matches('\n'), REPORTS_CSS.trim_matches('\n'));
            assert_eq!(html.matches("<style>\n").count(), 1, "one style element");
        }
    }

    #[test]
    fn the_style_is_the_run_reports_style_too() {
        use crate::runs::report::{render as render_run, RunReport};
        use crate::runs::model::StopReason;
        let run = render_run(&RunReport {
            title: "t",
            project: "p",
            scope: "s",
            finished: "f",
            seconds: 1,
            stop_reason: &StopReason::QueueEmpty,
            tasks: None,
            batches: &[],
            journal: None,
        });
        assert_eq!(style_of(&render(&typical())), style_of(&run));
    }

    #[test]
    fn the_document_is_a_whole_page_the_tab_can_stamp_a_theme_on() {
        let html = render(&typical());
        assert!(html.starts_with("<!doctype html>"));
        assert!(html.contains("<html lang=\"en\">"));
        assert!(html.contains("<main class=\"page\">"));
        assert!(html.trim_end().ends_with("</html>"));
    }

    #[test]
    fn the_document_reaches_no_network() {
        for html in [render(&typical()), render(&parse(CLEAN).unwrap()), render_fallback("x", "a", "b")] {
            let body = body_of(&html);
            assert!(!html.contains("<script") && !html.contains("<img") && !html.contains("<link"));
            assert!(!body.contains("http://") && !body.contains("https://"));
            assert!(!html.contains("@import") && !html.contains("@font-face"));
        }
    }

    // -- the fixtures ------------------------------------------------------------

    #[test]
    fn the_typical_fixture_draws_every_part_of_the_design() {
        let html = render(&typical());
        assert!(html.contains("<title>Code review \u{00B7} smetana, bd-sync \u{00B7} 74%</title>"), "{html}");
        assert!(html.contains("<h1>smetana, bd-sync</h1>"));
        assert!(html.contains(
            "<span class=\"ref-pair\">main<span class=\"arrow\" aria-label=\"from\">\u{2190}</span>feature/smetana-6tev-worktree-gc-skip-live-agents</span>"
        ));
        assert!(html.contains("9 files \u{00B7} <span class=\"add\">+318</span> <span class=\"del\">\u{2212}42</span>"));
        assert!(html.contains("<div class=\"score-num\">74<small>%</small></div>"));
        assert!(html.contains("<div class=\"score-label c-serious\">"));
        assert!(html.contains("class=\"band b-mid is-current\""));
        assert!(html.contains("<div class=\"marker\" style=\"left:74%\"><span>74</span></div>"));
        assert!(html.contains("aria-label=\"Readiness 74 percent, range 70 to 89\""));
        assert!(html.contains("<nav class=\"strip\""));
        assert!(html.contains("<a class=\"cell tone-failed is-loud\" href=\"#blocking\">"));
        assert!(html.contains("<a class=\"cell tone-needs-you\" href=\"#serious\">"));
        assert!(html.contains("<a class=\"cell tone-neutral\" href=\"#minor\">"));
        let body = body_of(&html);
        let blocking = body.find("id=\"blocking\"").unwrap();
        let serious = body.find("id=\"serious\"").unwrap();
        let minor = body.find("id=\"minor\"").unwrap();
        assert!(blocking < serious && serious < minor, "blocking, then serious, then minor");
        for id in ["B1", "S1", "S2", "S3", "M1", "M2", "M3", "M4", "M5"] {
            assert!(html.contains(&format!("<article class=\"finding card\" id=\"{id}\">")), "{id}");
        }
        assert!(!html.contains("id=\"B2\"") && !html.contains("id=\"S4\""));
        assert!(html.contains("<span class=\"repo\">bd-sync/</span>crates/bd-sync-store/"));
        assert!(html.contains("<footer class=\"foot\">"));
        assert!(html.contains("<div class=\"mono\">bd-sync \u{00B7} main \u{2190} feature/smetana-6tev-spawned-from-index</div>"));
        assert!(html.contains("<span class=\"mono\">2026-10-23 14:02</span>"));
        assert!(html.contains("<span class=\"mono\">claude-2</span>"));
        assert!(html.contains("<span class=\"mono\">[merge].hazards</span> \u{00B7} smetana <span class=\"yes\">yes</span>"));
        assert!(html.contains("bd-sync <span class=\"no\">no</span>"));
    }

    #[test]
    fn a_finding_draws_defect_scenario_fix_and_its_code_quote() {
        let html = render(&typical());
        // Backticks: code in defect and fix, `.io` in the scenario.
        assert!(html.contains("<dd class=\"defect\"><code>collect_stale</code> decides staleness"));
        assert!(html.contains("<span class=\"io\">remove_dir_all(&quot;wt/bd-a1b2&quot;)</span> runs under the live agent"));
        assert!(html.contains("read <code>.smetana/agent.lock</code> and skip"));
        // The quote: a caption, one span per line with no whitespace between, the defect line marked.
        assert!(html.contains("<figcaption>src-tauri/src/worktree/gc.rs</figcaption>"));
        assert!(html.contains("<span class=\"l\"><span class=\"n\">138</span>for wt in list_worktrees(root)? {</span><span class=\"l\"><span class=\"n\">139</span>"));
        assert!(html.contains("<span class=\"l hl\"><span class=\"n\">142</span>"));
        assert_eq!(html.matches("<span class=\"l hl\">").count(), 3, "one highlight per quote that has one");
        // A title with backticks.
        assert!(html.contains("<h3><code>gc.stale_after</code> setting is ignored</h3>"));
    }

    #[test]
    fn the_no_findings_fixture_draws_the_clean_state_and_a_div_strip() {
        let html = render(&parse(CLEAN).unwrap());
        assert!(html.contains("<div class=\"strip\""), "{html}");
        assert!(!html.contains("<nav"), "nothing to link to, so no nav");
        assert_eq!(html.matches("cell is-zero").count(), 4);
        assert!(!html.contains("<a class=\"cell"));
        assert!(html.contains("<section class=\"clean card\""));
        assert!(html.contains("<h2 id=\"clean-h\">No findings</h2>"));
        assert!(html.contains(
            "Every changed file was read against the project rules. Nothing blocking, serious or minor."
        ));
        assert!(html.contains("<li><span>app/src/components/agent/LogView.vue</span><span><span class=\"add\">+31</span> <span class=\"del\">\u{2212}6</span></span></li>"));
        assert!(!html.contains("class=\"group "), "no groups");
        assert!(html.contains("<div class=\"score-label c-ok\">"));
        assert!(html.contains("<div class=\"marker\" style=\"left:calc(100% - 2px)\"><span style=\"transform:translateX(-90%)\">100</span></div>"));
        assert!(html.contains("class=\"band b-high is-current\""));
        assert!(html.contains("<span class=\"mono\">[merge].hazards</span> <span class=\"yes\">yes</span> \u{00B7} <span class=\"mono\">.smetana/review.toml</span>"), "{html}");
    }

    // -- states from the README --------------------------------------------------

    #[test]
    fn four_or_more_repositories_are_all_drawn_everywhere() {
        let pairs: Vec<Pair> = ["alpha", "beta", "gamma", "delta", "epsilon"].iter().map(|r| pair(r)).collect();
        let html = render(&review(pairs, vec![finding(Level::Minor)], 95));
        assert!(html.contains("<h1>alpha, beta, gamma, delta, epsilon</h1>"));
        assert!(html.contains("<title>Code review \u{00B7} alpha, beta, gamma, delta, epsilon \u{00B7} 95%</title>"));
        assert_eq!(html.matches("<li class=\"ref\">").count(), 5);
        assert_eq!(html.matches("<div class=\"mono\">").count(), 5, "five refs lines in the footer");
        for repo in ["alpha", "beta", "gamma", "delta", "epsilon"] {
            assert!(html.contains(&format!("<span class=\"ref-repo\">{repo}</span>")), "{repo}");
        }
    }

    #[test]
    fn an_empty_level_in_a_non_empty_report_draws_no_group_and_a_zero_cell() {
        let html = render(&review(vec![pair("r")], vec![finding(Level::Blocking), finding(Level::Minor)], 40));
        assert!(html.contains("<nav class=\"strip\""), "something to link to");
        assert!(html.contains("<div class=\"cell is-zero\"><span class=\"cell-label\"><span class=\"g\" aria-hidden=\"true\">\u{25B2}</span>Serious</span><span class=\"cell-n\">0</span></div>"));
        let body = body_of(&html);
        assert!(!body.contains("id=\"serious\""), "no serious group");
        assert!(!body.contains("href=\"#serious\""), "and no link to it");
        assert!(body.contains("id=\"blocking\"") && body.contains("id=\"minor\""));
        assert!(!body.contains("class=\"clean"), "the clean state is for no findings at all");
        assert_eq!(body.matches("is-loud").count(), 1);
    }

    #[test]
    fn the_scale_picks_its_range_at_the_edges() {
        for (score, range, label) in [
            (100, "b-high", "Ready as is"),
            (90, "b-high", "Ready as is"),
            (89, "b-mid", "Serious issues, nothing blocking"),
            (70, "b-mid", "Serious issues, nothing blocking"),
            (69, "b-low", "Blocking issue \u{2014} returned for rework"),
            (0, "b-low", "Blocking issue \u{2014} returned for rework"),
        ] {
            let html = render(&review(vec![pair("r")], vec![], score));
            assert!(html.contains(&format!("class=\"band {range} is-current\"")), "{score}");
            assert_eq!(body_of(&html).matches("is-current").count(), 2, "the band and the legend row: {score}");
            assert!(html.contains(&format!("</span>{label}</div>")), "{score}");
        }
        let html = render(&review(vec![pair("r")], vec![], 97));
        assert!(html.contains("left:97%"), "the pinned marker starts at 98");
        let html = render(&review(vec![pair("r")], vec![], 98));
        assert!(html.contains("left:calc(100% - 2px)"));
    }

    #[test]
    fn a_long_path_and_a_long_code_line_are_drawn_whole() {
        let path = format!("crates/{}/migration.rs", "very-long-directory-name/".repeat(12));
        let line = format!("let x = \"{}\";", "0123456789".repeat(40));
        let mut f = finding(Level::Serious);
        f.path = path.clone();
        f.code = Some(Code { path: None, start_line: 1, lines: vec![line.clone()], highlight: vec![1] });
        let html = render(&review(vec![pair("r")], vec![f], 80));
        assert!(html.contains(&escape(&path)), "the path is not cut");
        assert!(html.contains(&format!("<figcaption>{}</figcaption>", escape(&path))), "the caption defaults to the finding's path");
        assert!(html.contains(&escape(&line)), "the line is not cut");
        assert!(html.contains("<span class=\"l hl\">"));
    }

    #[test]
    fn a_caption_of_its_own_wins_over_the_findings_path() {
        let mut f = finding(Level::Minor);
        f.code = Some(Code { path: Some("other/file.ts".into()), start_line: 10, lines: vec!["a".into(), "b".into()], highlight: vec![11] });
        let html = render(&review(vec![pair("r")], vec![f], 95));
        assert!(html.contains("<figcaption>other/file.ts</figcaption>"));
        assert!(html.contains("<span class=\"l\"><span class=\"n\">10</span>a</span><span class=\"l hl\"><span class=\"n\">11</span>b</span>"));
    }

    #[test]
    fn without_hazards_the_clean_sentence_does_not_claim_the_project_rules() {
        let mut p = pair("r");
        p.hazards = false;
        let html = render(&review(vec![p], vec![], 100));
        assert!(html.contains("<p>Every changed file was read. Nothing blocking, serious or minor.</p>"), "{html}");
        assert!(!html.contains("against the project rules"));
        assert!(html.contains("<span class=\"no\">no</span>"));
    }

    #[test]
    fn one_pair_with_hazards_is_enough_to_claim_the_project_rules() {
        let mut a = pair("a");
        a.hazards = false;
        let html = render(&review(vec![a, pair("b")], vec![], 100));
        assert!(html.contains("read against the project rules."));
    }

    #[test]
    fn reviewed_files_carry_their_repo_only_when_there_are_several() {
        let mut one = pair("solo");
        one.reviewed_files = vec![ReviewedFile { path: "a.rs".into(), added: 1, removed: 0 }];
        let html = render(&review(vec![one.clone()], vec![], 100));
        assert!(html.contains("<li><span>a.rs</span>"));
        let mut two = pair("other");
        two.reviewed_files = vec![ReviewedFile { path: "b.rs".into(), added: 2, removed: 2 }];
        let html = render(&review(vec![one, two], vec![], 100));
        assert!(html.contains("<li><span>solo/a.rs</span>") && html.contains("<li><span>other/b.rs</span>"));
    }

    #[test]
    fn a_single_file_is_not_called_files() {
        let mut p = pair("r");
        p.files = 1;
        let html = render(&review(vec![p], vec![], 100));
        assert!(html.contains("1 file \u{00B7} <span class=\"add\">"));
        assert!(!html.contains("1 files"));
    }

    // -- escaping ----------------------------------------------------------------

    #[test]
    fn markup_in_every_field_comes_out_as_text() {
        let evil = "<script>alert(1)</script> & co";
        let mut p = pair(evil);
        p.head = evil.into();
        p.base = evil.into();
        p.rules_source = Some(evil.into());
        let mut f = finding(Level::Blocking);
        f.title = evil.into();
        f.repo = evil.into();
        f.path = evil.into();
        f.defect = evil.into();
        f.scenario = evil.into();
        f.fix = evil.into();
        f.code = Some(Code { path: Some(evil.into()), start_line: 1, lines: vec![evil.into()], highlight: vec![] });
        let mut r = review(vec![p], vec![f], 10);
        r.reviewer = evil.into();
        r.date = evil.into();
        let html = render(&r);
        assert!(!html.contains("<script"), "{html}");
        assert!(html.contains("&lt;script&gt;alert(1)&lt;/script&gt; &amp; co"));
        // The code line, in the place that has no backtick handling.
        assert!(html.contains("<span class=\"n\">1</span>&lt;script&gt;"));
    }

    #[test]
    fn markup_inside_a_backtick_span_is_still_escaped() {
        let mut f = finding(Level::Minor);
        f.defect = "see `<b>x</b>` here".into();
        f.scenario = "got `<i>&</i>` back".into();
        let html = render(&review(vec![pair("r")], vec![f], 95));
        assert!(html.contains("<code>&lt;b&gt;x&lt;/b&gt;</code>"));
        assert!(html.contains("<span class=\"io\">&lt;i&gt;&amp;&lt;/i&gt;</span>"));
        assert!(!html.contains("<b>x") && !html.contains("<i>"));
    }

    #[test]
    fn an_unpaired_backtick_is_left_as_written() {
        let mut f = finding(Level::Minor);
        f.defect = "a ` alone".into();
        let html = render(&review(vec![pair("r")], vec![f], 95));
        assert!(html.contains("<dd class=\"defect\">a ` alone</dd>"));
    }

    #[test]
    fn an_iso_date_is_shown_the_way_the_design_has_it() {
        assert_eq!(date_text("2026-10-23T16:40:12+03:00"), "2026-10-23 16:40");
        assert_eq!(date_text("2026-10-23 16:40"), "2026-10-23 16:40");
        assert_eq!(date_text("yesterday"), "yesterday");
        assert_eq!(date_text("\u{0436}".repeat(10).as_str()), "\u{0436}".repeat(10), "no slicing inside a character");
    }

    // -- parsing -----------------------------------------------------------------

    #[test]
    fn unknown_fields_are_ignored() {
        let json = r#"{"score":100,"pairs":[{"repo":"r","base":"a","head":"b","files":1,"added":1,"removed":0,"hazards":false,"extra":1}],
            "findings":[],"reviewer":"x","date":"d","note":"hello"}"#;
        assert!(parse(json).is_ok());
    }

    #[test]
    fn a_missing_required_field_is_an_error_naming_it() {
        let json = r#"{"score":100,"pairs":[],"findings":[],"reviewer":"x"}"#;
        let err = parse(json).unwrap_err();
        assert!(err.contains("date"), "{err}");
        let json = r#"{"score":1,"pairs":[{"repo":"r","base":"a","head":"b","files":1,"added":1,"removed":0}],"findings":[],"reviewer":"x","date":"d"}"#;
        assert!(parse(json).unwrap_err().contains("hazards"));
    }

    #[test]
    fn an_empty_pairs_list_is_an_error_naming_the_field() {
        let json = r#"{"score":1,"pairs":[],"findings":[],"reviewer":"x","date":"d"}"#;
        assert!(parse(json).unwrap_err().contains("pairs"));
        let dir = scratch("nopairs");
        std::fs::write(dir.join("r.json"), json).unwrap();
        write_html(&dir, "r").unwrap();
        let html = std::fs::read_to_string(dir.join("r.html")).unwrap();
        assert!(html.contains("Report could not be built") && html.contains("pairs"), "{html}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_unknown_level_is_an_error_rather_than_a_guess() {
        let json = r#"{"score":1,"pairs":[],"reviewer":"x","date":"d","findings":[{"level":"huge","title":"t","repo":"r","path":"p","line":1,"defect":"d","scenario":"s","fix":"f"}]}"#;
        assert!(parse(json).is_err());
    }

    #[test]
    fn a_score_that_contradicts_the_findings_is_drawn_as_written() {
        let r = review(vec![pair("r")], vec![finding(Level::Blocking)], 95);
        assert!(contradicts(&r));
        let html = render(&r);
        assert!(html.contains("<div class=\"score-num\">95<small>%</small></div>"), "no refusal, no correction");
        assert!(!contradicts(&review(vec![pair("r")], vec![finding(Level::Blocking)], 60)));
        assert!(!contradicts(&review(vec![pair("r")], vec![finding(Level::Serious)], 80)));
    }

    // -- the fallback page and the disk -------------------------------------------

    fn scratch(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("smetana-review-report-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_missing_json_still_leaves_a_page_that_says_so() {
        let dir = scratch("missing");
        write_html(&dir, ".smetana/reviews/r1").unwrap();
        let html = std::fs::read_to_string(dir.join(".smetana/reviews/r1.html")).unwrap();
        assert!(html.contains("Report could not be built"));
        assert!(html.contains("The JSON file was not found."));
        assert!(html.contains(".smetana/reviews/r1.json") && html.contains(".smetana/reviews/r1.md"));
        assert_eq!(style_of(&html).trim_matches('\n'), REPORTS_CSS.trim_matches('\n'));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_broken_json_still_leaves_a_page_that_names_the_parse_error() {
        let dir = scratch("broken");
        std::fs::create_dir_all(dir.join("rv")).unwrap();
        std::fs::write(dir.join("rv/r.json"), "{ not json <b>").unwrap();
        write_html(&dir, "rv/r").unwrap();
        let html = std::fs::read_to_string(dir.join("rv/r.html")).unwrap();
        assert!(html.contains("Report could not be built"));
        assert!(html.contains("The JSON could not be read:"), "{html}");
        assert!(html.contains("rv/r.json") && html.contains("rv/r.md"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_json_without_a_required_field_leaves_a_page_naming_the_field() {
        let dir = scratch("field");
        std::fs::write(dir.join("r.json"), r#"{"score":90,"pairs":[],"findings":[],"reviewer":"x"}"#).unwrap();
        write_html(&dir, "r").unwrap();
        let html = std::fs::read_to_string(dir.join("r.html")).unwrap();
        assert!(html.contains("Report could not be built"));
        assert!(html.contains("missing field"), "{html}");
        assert!(html.contains("date"), "{html}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_good_json_leaves_the_report_and_replaces_whatever_html_was_there() {
        let dir = scratch("good");
        std::fs::write(dir.join("r.json"), TYPICAL).unwrap();
        std::fs::write(dir.join("r.html"), "<p>the agent's own layout</p>").unwrap();
        write_html(&dir, "r").unwrap();
        let html = std::fs::read_to_string(dir.join("r.html")).unwrap();
        assert!(html.contains("<div class=\"score-num\">74<small>%</small></div>"));
        assert!(!html.contains("the agent's own layout"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_fallback_escapes_the_reason_and_the_paths() {
        let html = render_fallback("<script>x</script> & y", "a<b>.json", "a&b.md");
        assert!(!html.contains("<script"));
        assert!(html.contains("&lt;script&gt;x&lt;/script&gt; &amp; y"));
        assert!(html.contains("a&lt;b&gt;.json") && html.contains("a&amp;b.md"));
    }

    #[test]
    fn a_report_path_that_leaves_the_project_writes_nothing() {
        let dir = scratch("escape");
        assert!(write_html(&dir, "../outside").is_err());
        assert!(write_html(&dir, "/abs/outside").is_err());
        assert!(write_html(&dir, "\\rooted").is_err() || cfg!(unix), "a backslash root is a root only on Windows");
        assert!(!dir.parent().unwrap().join("outside.html").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_schema_pasted_into_the_skill_is_the_design_sources_schema() {
        const SKILL: &str = include_str!("../../resources/smetana/skills/reviewing-branch-changes/SKILL.md");
        const SCHEMA: &str = include_str!("../../../docs/design/reports/code-review/schema.json");
        let section = &SKILL[SKILL.find("## 8.").expect("section 8")..];
        let open = section.find("```json\n").expect("a json fence in section 8") + "```json\n".len();
        let close = section[open..].find("\n```").expect("the end of the fence");
        let pasted: serde_json::Value = serde_json::from_str(&section[open..open + close]).expect("the pasted schema parses");
        let source: serde_json::Value = serde_json::from_str(SCHEMA).unwrap();
        assert_eq!(pasted, source, "SKILL.md section 8 must carry docs/design/reports/code-review/schema.json whole");
    }
}
