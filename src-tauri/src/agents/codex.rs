//! Codex.
//!
//! Codex has a skills system of its own — `~/.codex/skills/`, `skills.config`,
//! `skills.bundled.enabled` — and `-c key=value` overrides any config value on
//! the command line. What it has no way to do is name an extra skills
//! directory: `SkillsConfig` carries no filesystem roots, and the only
//! mechanism that does, `skills/extraRoots/set`, belongs to the app-server, a
//! different process from the TUI this app runs in a PTY. Writing into
//! `~/.codex/skills/` or repointing `CODEX_HOME` would reach into the person's
//! own setup, so neither is done. The skill text rides in the prompt instead.
//!
//! Layer B — Codex's own approval dialog, read off the vt100 grid — is at the
//! bottom of this file. It reads someone else's interface and an update to that
//! CLI can break it; when it does it breaks softly, leaving layer A in place.

use portable_pty::CommandBuilder;

use super::library::read_skill;
use super::{
    cascade, prompt, Autonomy, ImageDelivery, Intent, Launch, Profile, SkillDelivery, Stage,
};
use crate::runs::model::RunMode;
use crate::terminal::model::{Question, QuestionOption};

pub struct Codex;

impl Profile for Codex {
    fn id(&self) -> &'static str {
        "codex"
    }

    fn binary(&self) -> &'static str {
        "codex"
    }

    fn delivery(&self) -> SkillDelivery {
        SkillDelivery::Inline
    }

    /// `-i, --image <FILE>...` — verified against the installed CLI's own help.
    /// Repeated once per file rather than given a list: a repeated flag is the
    /// one form every argument parser agrees on, and this app does not get to
    /// assume anything about somebody else's separator.
    fn images(&self) -> ImageDelivery {
        ImageDelivery::Flag("-i")
    }

    fn command(&self, launch: &Launch) -> CommandBuilder {
        let mut cmd = CommandBuilder::new(self.binary());
        let filing_a_task = matches!(launch.intent, Intent::NewTask { .. });
        // Only the mode that actually uses the whole process pays for reading
        // it: `Auto` is handed the path and decides for itself.
        let discussing = matches!(
            launch.intent,
            Intent::NewTask { brainstorm: Stage::On, .. }
        );
        // The plan's own process, and only where a plan was actually asked
        // for: the cascade decides that, never the raw switch, so an `On`
        // sitting under a discussion nobody wanted costs nothing here either.
        let planning = matches!(
            &launch.intent,
            Intent::NewTask { brainstorm, spec, plan, .. }
                if cascade(*brainstorm, *spec, *plan).1 == Stage::On
        );
        let filing =
            filing_a_task.then(|| read_skill(&launch.skills.smetana, "filing-a-task")).flatten();
        // The whole of what a resolving session does, so it is read whenever
        // one is being started and never otherwise.
        let resolving = matches!(launch.intent, Intent::ResolveTask { .. })
            .then(|| read_skill(&launch.skills.smetana, "resolving-questions"))
            .flatten();
        let brainstorming_text =
            discussing.then(|| read_skill(&launch.skills.superpowers, "brainstorming")).flatten();
        let plans_text =
            planning.then(|| read_skill(&launch.skills.superpowers, "writing-plans")).flatten();
        // Before the prompt, which is positional — see the same line in
        // `claude.rs` for why the order is not left to the parser.
        if let Intent::Run { settings } = &launch.intent {
            for arg in self.autonomy(settings.mode).args {
                cmd.arg(arg);
            }
        }
        // The pixels, on the command line. The paths are in the prompt as well,
        // and that is not a duplicate: what rides here is what Codex looks at,
        // and what rides there is what has to end up in the issue description.
        // Read back through `self.images()` rather than written out again, so
        // the flag and the profile's answer about it cannot drift apart.
        if let (Intent::NewTask { draft, .. }, ImageDelivery::Flag(flag)) =
            (&launch.intent, self.images())
        {
            for image in &draft.images {
                cmd.arg(flag);
                cmd.arg(image);
            }
        }
        let text = prompt::SkillText {
            filing: filing.as_deref(),
            resolving: resolving.as_deref(),
            brainstorming: brainstorming_text.as_deref(),
            plans: plans_text.as_deref(),
        };
        if let Some(built) = prompt::build(
            &launch.intent,
            self.delivery(),
            self.images(),
            &launch.skills,
            launch.facts.as_deref(),
            text,
            &launch.languages,
        ) {
            cmd.arg(built);
        }
        cmd
    }

    fn question(&self, screen: &[String]) -> Option<Question> {
        question(screen)
    }

    /// `--dangerously-bypass-approvals-and-sandbox` rather than the gentler
    /// `--ask-for-approval never --sandbox workspace-write`, and the difference
    /// is not caution but reach: a run cuts worktrees in sibling repositories
    /// and installs dependencies, so it writes outside any one workspace and
    /// needs the network. Under `workspace-write` both are refused, and what
    /// the person would see is every task parked for a reason that is about
    /// the sandbox rather than about their code.
    ///
    /// `--full-auto` is not used: Codex removed it, and a build that still
    /// passed it would get a deprecation warning today and an unknown argument
    /// tomorrow.
    ///
    /// Nothing in `Supervised` or `Solo`, the same as Claude Code — a person is
    /// there, and taking their prompts away is taking away what those modes are.
    fn autonomy(&self, mode: RunMode) -> Autonomy {
        Autonomy {
            args: match mode {
                RunMode::Auto => vec!["--dangerously-bypass-approvals-and-sandbox"],
                RunMode::Supervised | RunMode::Solo => vec![],
            },
            env: vec![],
        }
    }
}

/// The glyph Codex draws against the option the cursor is on: U+203A, a single
/// right-pointing angle quotation mark. Not Claude Code's U+276F, and the two
/// readers deliberately do not share one: a glyph one harness happens to use
/// today is exactly the kind of thing that would drift.
///
/// Codex uses the same glyph in two other places, and both are why this reader
/// insists on the numbering as well: it prefixes the person's own submitted
/// prompt in the transcript, and it prefixes the placeholder inside the empty
/// composer at the foot of the screen. Neither is numbered.
const CURSOR: char = '\u{203A}';

/// One option of a dialog: `› 1. Yes, proceed (y)` or
/// `  3. No, and tell Codex what to do differently (esc)`.
/// Returns (index, label, whether the cursor is on it).
///
/// The cursor has to be the first non-blank character of the line rather than
/// merely present in it, which is where this parts company with the Claude
/// reader: `›` is a character Codex writes into ordinary lines, so a label
/// containing one would otherwise read as the selected option and the app would
/// point a person at the wrong answer.
///
/// The label is only the head row; a label long enough to wrap continues on the
/// rows beneath, and `wrapped` below puts it back together. Whatever comes back
/// keeps the shortcut hint Codex prints after it — `(y)`, `(p)`, `(esc)`. It is
/// what a person reads on the screen, and trimming it would mean this app
/// deciding which part of somebody else's wording is the real label.
fn option_line(line: &str) -> Option<(usize, String, bool)> {
    let rest = line.trim_start();
    let selected = rest.starts_with(CURSOR);
    let rest = if selected { rest[CURSOR.len_utf8()..].trim_start() } else { rest };
    let dot = rest.find(". ")?;
    let index: usize = rest[..dot].parse().ok()?;
    let label = rest[dot + 2..].trim_end().to_owned();
    if label.is_empty() {
        return None;
    }
    Some((index, label, selected))
}

fn is_blank(line: &str) -> bool {
    line.trim().is_empty()
}

/// The first row of a transcript entry: the person's prompt behind `›`, one of
/// the agent's `•` bullets, a rule, the session banner. Codex draws every one
/// of them hard against column 0 and indents everything that continues one, and
/// on a screen with no frame anywhere on it that is the only structural
/// boundary there is between the transcript and a dialog.
fn is_entry(line: &str) -> bool {
    !is_blank(line) && !line.starts_with(char::is_whitespace)
}

/// A transcript entry specifically — a turn of the conversation, rather than
/// any old thing at column 0.
///
/// The distinction earns its keep in exactly one place, and without it that
/// place cannot work at all. An agent's entry and the trust prompt are the same
/// shape to the row: something at column 0, a blank, indented prose ending in a
/// question mark, a blank, the options. Rejecting everything that hangs off a
/// column-0 line would take the trust prompt with it; rejecting only what hangs
/// off a turn of the conversation keeps it, because the trust prompt's anchor is
/// `> You are in …` and a turn's is `•` or `›`.
///
/// `◦` is the same bullet in the hollow state Codex alternates it into while a
/// turn is still running — `codex-0.146-working.txt` caught the filled form of
/// the very same line. Naming a glyph that never appears would cost nothing
/// here anyway: everything in this set only ever adds refusals.
fn is_turn(line: &str) -> bool {
    is_entry(line) && line.starts_with(['\u{2022}', '\u{25E6}', CURSOR])
}

/// The whole of an option's label, head row and any rows it wrapped onto.
///
/// A continuation row is anything directly under the option that is not blank,
/// not a transcript entry and not an option of its own — measured on the update
/// prompt at 60 columns, which wraps its first option over three rows and
/// numbers none of them. Gathering it is not cosmetic: skipping it, which is
/// what the first version of this did, cut every long label at whatever column
/// the pane happened to wrap, so `Update now (runs \`sh -c 'curl -fsSL` was the
/// whole of what a person would have been offered.
///
/// The blank row is what ends the list, and it is also all that stands between
/// the last option and the footer under it. A dialog that ever drew the two
/// without one between them would fold the footer into the last label — a
/// cosmetic cost, and the direction to be wrong in, since the alternative is
/// guessing at wording.
fn wrapped(screen: &[String], row: usize, head: &str) -> String {
    let mut words: Vec<&str> = head.split_whitespace().collect();
    for line in &screen[row + 1..] {
        if is_blank(line) || is_entry(line) || option_line(line).is_some() {
            break;
        }
        words.extend(line.split_whitespace());
    }
    words.join(" ")
}

/// Is there a break in the layout strictly between these two rows — a transcript
/// entry of its own, or the double blank Codex puts between one top-level block
/// and the next?
///
/// Measured, on real screens at 60 and 120 columns: **one** blank row separates
/// two paragraphs of the same block, **two** separate one block from another.
/// That is the difference between the second paragraph of an agent's answer and
/// a dialog drawn under it, and there is nothing else to tell them apart —
/// both are prose indented by two under a `•` at column 0.
fn parted(screen: &[String], from: usize, to: usize) -> bool {
    let mut blanks = 0;
    for line in &screen[from + 1..to] {
        if is_entry(line) {
            return true;
        }
        if is_blank(line) {
            blanks += 1;
            if blanks >= 2 {
                return true;
            }
        } else {
            blanks = 0;
        }
    }
    false
}

/// The question inside one paragraph of a dialog, or `None` if there is none.
///
/// Two shapes, both taken from real screens. `Would you like to run the
/// following command?` is the whole paragraph — that is `whole`. The trust
/// prompt is a question followed by two sentences explaining it, and only the
/// first is what a person is being asked, so a question mark with whitespace
/// after it ends the question too — that is `opening`.
///
/// They are two functions and the caller tries every paragraph against `whole`
/// before it tries any against `opening`, which is not a detail: in the command
/// dialog the preview sits *below* the question, so the nearest paragraph is
/// the command itself, and a command carrying `x ? 1 : 2` would otherwise hand
/// back `$ node -e "console.log(x ?` as what a person is being asked. Preferring
/// a paragraph that is a question outright leaves `opening` doing only the job
/// it was written for.
fn whole(text: &str) -> Option<String> {
    text.ends_with('?').then(|| text.to_owned())
}

/// The first sentence of a paragraph, if it is a question.
///
/// The "followed by whitespace" is not decoration: the paragraph above the
/// options can be a shell command Codex is asking to run, and `grep "what?"
/// file` must not be read as a question. A question mark that ends a sentence
/// is followed by a space; one inside a quoted argument is followed by the
/// quote.
fn opening(text: &str) -> Option<String> {
    let mut chars = text.char_indices().peekable();
    while let Some((at, c)) = chars.next() {
        if c == '?' && chars.peek().is_some_and(|(_, next)| next.is_whitespace()) {
            return Some(text[..=at].to_owned());
        }
    }
    None
}

/// The dialogs that name themselves and ask nothing, one literal per dialog.
///
/// There is one so far: the update prompt Codex draws on the day a newer
/// version is out, before the session has started at all. It has everything a
/// dialog has — numbered options from 1, a cursor on one of them, a block of
/// its own at the top of the screen — except a question. There is no question
/// mark anywhere on it: the banner states, the options act
/// (`codex-0.146-update-prompt.txt`). So the ordinary reading declined it, and
/// a session that opened on that day simply stood there with nothing on screen
/// to say a person was needed (smetana-9qp).
///
/// This is the same shape `claude.rs` carries for the folder-trust dialog and
/// it keeps the same two rules, because the thing being protected is the same:
/// a loud row is budgeted at one or two a screen, and dropping the
/// question-mark requirement for **everything** would leave the cursor as the
/// only guard against a numbered list in the agent's own prose. So the
/// requirement stays in force for every dialog that does not name itself, and
/// the narrower reading opens only after the ordinary one has declined, fenced
/// on a literal string this dialog prints and ordinary output does not.
///
/// The literal is a substring rather than a prefix, which is where this parts
/// company with the Claude table: the banner opens with a decorative glyph
/// (U+2728 and a hair space) and closes with the two version numbers, so the
/// only stable part is the middle. Matching the glyph would pin a decoration,
/// and matching the versions would pin the one thing guaranteed to change.
const HEADINGS: &[&str] = &["Update available!"];

/// The question of a dialog that named itself with one of `HEADINGS`, when
/// nothing in its block reads as a question at all.
///
/// What comes back is the heading's own paragraph, as the screen draws it —
/// glyph, versions and all. Nothing else on that screen is a candidate: the
/// only other line is a link to the release notes, and the two things a person
/// needs in order to answer are which version is on offer and what the options
/// do, which is exactly the banner and the labels. Composing a question of our
/// own ("Update Codex now?") would put words on the screen that Codex never
/// wrote, and trimming the glyph would be this app deciding which part of
/// somebody else's wording is the real one — the same reason `option_line`
/// keeps the `(y)` and `(esc)` hints.
///
/// The search is fenced by its caller: `paragraphs` is the dialog's own block,
/// already cut away from the transcript above it, so nothing the agent wrote
/// can be handed back here.
fn headed(paragraphs: &[String]) -> Option<String> {
    let heading =
        paragraphs.iter().rposition(|p| HEADINGS.iter().any(|heading| p.contains(heading)))?;
    Some(paragraphs[heading].clone())
}

/// Codex's approval dialog: a paragraph of text and numbered options, drawn
/// straight onto the transcript with no frame around it.
///
/// Read off Codex CLI 0.146.0 — the command dialog, the edit dialog, the
/// trust-this-directory prompt, the update prompt, a session at work and a
/// draft left in the composer are all in `tests/fixtures/`, captured under a
/// PTY at 60 and 120 columns and rendered through the same vt100 the worker
/// reads. Every discriminator below is a property of those screens.
///
/// **What this aims at, and what it does not promise.** The direction that
/// matters is missing rather than matching: a session wrongly turned
/// `needs-you` claims one of the one or two loud rows the design budgets, and
/// makes `terminal_run_capture` refuse to write to a session with nothing open
/// on it. So every rule here is written to fail closed, and a change to that
/// CLI should cost a miss.
///
/// That is an aim and not a guarantee, and saying otherwise here would be worse
/// than saying nothing — the next person would read it as a contract and stop
/// checking. It has been untrue twice already. Both times the shape was the
/// same: something the app had never seen on a screen turned out to be built
/// out of the same rows a dialog is. The known open case is written out under
/// the rules below; the unknown ones are why this is a list of properties of
/// six screens rather than a specification of an interface nobody here owns.
///
/// - the options number themselves 1, 2, 3 … and the **last** such run on the
///   screen is the dialog, since anything the agent merely printed sits above
///   it. Same reasoning as the Claude reader, and it holds for the same reason:
///   the dialog replaces the composer at the foot of the screen;
/// - **exactly one** option carries the cursor. Prose never does, and a live
///   dialog always does. More than one would not be this dialog at all;
/// - there are **at least two** options, and nothing parts them from one
///   another — a wrapped label may sit between two of them, a blank pair or
///   another transcript entry may not;
/// - the question is in the block directly above the options, and that block is
///   what is left after the transcript is cut away: the walk up stops at a
///   line drawn at column 0 or at the double blank between two top-level
///   blocks, and if it stopped on a **turn of the conversation** — a `•` or a
///   `›` rather than any old column-0 line — then everything it gathered is
///   that turn's own continuation and is thrown away whole;
/// - and something in that block has to read as a question — a paragraph that
///   is one outright for preference, and only failing that a paragraph that
///   opens with one. The one exception is a block that names itself as one of
///   the dialogs in `HEADINGS`, which are the ones that ask nothing at all;
///   that reading opens only once this one has declined.
///
/// The block rules are the third version of this, and both earlier ones were
/// wrong in the same direction: they matched a screen with nothing on it to
/// answer.
///
/// The first bounded the block by indentation alone, which holds while the turn
/// above fits on one row and fails the moment it wraps — at a narrow centre
/// pane, the common case. `codex-0.146-draft-in-the-composer.txt` is that
/// failure captured: an agent still working, its answer's last paragraph ending
/// in a question mark on an indented row, and a two-line numbered draft sitting
/// unsent in the composer below.
///
/// The second threw the block away only when its first row was glued to the
/// turn above with no blank between, which misses whenever that turn's first
/// paragraph is a single row — the row under it is then the blank before its
/// *second* paragraph, and the block looks unanchored. `• Done — tests pass.`
/// followed by a blank, a question and a numbered draft is the whole of it.
/// Hence a turn, not a gap: the block is refused for what it hangs off, rather
/// than for how closely.
///
/// **Known open: a scrolled screen with no anchor on it at all.** When the turn
/// that owns the prose has scrolled off the top, the walk reaches row 0 without
/// meeting anything, and an indented `is that alright with you?` above a
/// numbered draft still reads as a dialog —
/// `a_scrolled_screen_with_nothing_to_anchor_it_is_a_known_gap` pins it.
/// Closing it means requiring a block to be anchored at all, which would then
/// refuse a dialog drawn at the top of an unscrolled screen — the update prompt
/// is one, and it starts at row 0. That is a trade between a false match in a
/// rare scroll position and a miss in an ordinary one, with no measurement
/// available to settle it, so it is recorded rather than guessed at.
fn question(screen: &[String]) -> Option<Question> {
    // Every numbered line on the screen, the agent's own among them.
    let marks: Vec<(usize, (usize, String, bool))> = screen
        .iter()
        .enumerate()
        .filter_map(|(i, line)| option_line(line).map(|parsed| (i, parsed)))
        .collect();

    // The dialog is the last block that starts its numbering over at 1.
    let start = marks.iter().rposition(|(_, (index, _, _))| *index == 1)?;

    let mut options = Vec::new();
    let mut selected = None;
    let mut cursors = 0;
    let mut expected = 1;
    let mut previous = None;
    for (row, (index, label, is_selected)) in &marks[start..] {
        // A gap in the numbering is the end of this block, not a hole in it.
        // Rows are not required to be adjacent: a long label wraps, and the
        // continuation line is neither an option nor the end of the list — the
        // update prompt at 60 columns wraps its first option over three rows.
        // What may not sit between two options is a break in the layout, which
        // is what stops a stray `4.` further down the screen from joining a
        // list it has nothing to do with.
        if *index != expected || previous.is_some_and(|last| parted(screen, last, *row)) {
            break;
        }
        if *is_selected {
            cursors += 1;
            selected = Some(options.len());
        }
        // Verified against 0.146.0 by answering a live dialog with these very
        // bytes: the digit selects and the return confirms, the same as Claude
        // Code, which is what the footer means by "press enter to confirm".
        options.push(QuestionOption {
            label: wrapped(screen, *row, label),
            send: format!("{index}\r"),
        });
        expected += 1;
        previous = Some(*row);
    }

    if cursors != 1 || options.len() < 2 {
        return None;
    }

    // Upwards from the options: first over the blank rows the dialog leaves
    // between its text and its options, then over its own rows, stopping at a
    // transcript entry at column 0 or at the double blank that separates one
    // top-level block from the next.
    let first = marks[start].0;
    let mut top = first;
    while top > 0 && is_blank(&screen[top - 1]) {
        top -= 1;
    }
    while top > 0 {
        if is_entry(&screen[top - 1]) {
            break;
        }
        if is_blank(&screen[top - 1]) && top >= 2 && is_blank(&screen[top - 2]) {
            top -= 1;
            break;
        }
        top -= 1;
    }

    // What is left may still be the tail of the transcript rather than a block
    // of its own: a turn's continuation rows are indented exactly as a dialog's
    // are, and its later paragraphs are parted from its first by the same
    // single blank a dialog uses inside itself. Hanging off that turn at all is
    // what says so — not how closely, which was the second version of this rule
    // and missed every turn whose first paragraph was one row. It throws the
    // whole run away rather than trimming it, because a dialog hangs off
    // nothing.
    //
    // A dialog reached its own block over the double blank between top-level
    // blocks, so `top` sits on that blank and this never fires; the trust
    // prompt reaches it over `> You are in …`, which is not a turn.
    if top > 0 && is_turn(&screen[top - 1]) {
        return None;
    }

    // `split_whitespace` drops the dialog's own indent and collapses any run of
    // whitespace to one space, so a question wrapped across rows comes back as
    // one line.
    let mut paragraphs: Vec<String> = Vec::new();
    for line in &screen[top..first] {
        if is_blank(line) {
            paragraphs.push(String::new());
        } else {
            let joined = line.split_whitespace().collect::<Vec<_>>().join(" ");
            match paragraphs.last_mut() {
                Some(last) if !last.is_empty() => {
                    last.push(' ');
                    last.push_str(&joined);
                }
                _ => paragraphs.push(joined),
            }
        }
    }

    // Bottom-most paragraph first, because a command preview sits *below* the
    // question in the command dialog and a diff sits *above* it in the edit
    // one, so the nearest one that reads as a question is the right answer for
    // both. A whole question beats an opening one anywhere on the screen.
    //
    // Only when neither finds anything does `headed` look, and only for a block
    // that named itself: the dialogs in `HEADINGS` ask nothing, and everything
    // else on this screen still has to.
    let text = paragraphs
        .iter()
        .rev()
        .find_map(|paragraph| whole(paragraph))
        .or_else(|| paragraphs.iter().rev().find_map(|paragraph| opening(paragraph)))
        .or_else(|| headed(&paragraphs))?;
    Some(Question { text, options, selected })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::library::Skills;
    use crate::agents::{Intent, Launch, Stage, TaskDraft};
    use std::path::PathBuf;

    /// The real bundle resources, the way `claude.rs` reaches its own screen
    /// fixtures. A made-up path would leave `read_skill` answering `None` in
    /// every test here, and the gating in `command` — which decides that filing
    /// guidance reaches an `Inline` harness whatever the Brainstorming switch
    /// says — would never be observed at all.
    fn resources(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources").join(name)
    }

    fn launch(intent: Intent) -> Launch {
        Launch {
            profile: &Codex,
            cwd: PathBuf::from("/tmp/project"),
            intent,
            skills: Skills {
                smetana: resources("smetana"),
                superpowers: resources("superpowers"),
                superpowers_installed: false,
            },
            facts: None,
            languages: crate::agents::Languages::default(),
        }
    }

    /// A line from the middle of each shipped `SKILL.md`, far enough in to be
    /// the body rather than the front matter: finding it in the prompt is the
    /// only proof that the file was read and pasted, not merely named.
    /// A sentinel from the shipped file, and deliberately its heading rather
    /// than a sentence out of the body: the prose gets rewritten, and a test
    /// that fails on wording says nothing about what it is actually guarding —
    /// that the body travelled at all. It must also be a string the prompt
    /// itself never writes, or the test would pass with the skill missing.
    const FILING_BODY: &str = "# Filing a task in bd";
    const BRAINSTORMING_BODY: &str = "ask questions one at a time";
    const PLANS_BODY: &str = "# Writing Plans";

    fn argv(launch: &Launch) -> Vec<String> {
        Codex
            .command(launch)
            .get_argv()
            .iter()
            .map(|s| s.to_string_lossy().into_owned())
            .collect()
    }

    fn new_task(brainstorm: Stage) -> Intent {
        with_images(brainstorm, Vec::new())
    }

    fn with_images(brainstorm: Stage, images: Vec<String>) -> Intent {
        staged(brainstorm, Stage::Off, Stage::Off, images)
    }

    fn staged(brainstorm: Stage, spec: Stage, plan: Stage, images: Vec<String>) -> Intent {
        Intent::NewTask {
            brainstorm,
            spec,
            plan,
            draft: TaskDraft {
                text: "Swap the red for green".into(),
                issue_type: Some("bug".into()),
                priority: Some(2),
                images,
            },
        }
    }

    #[test]
    fn nothing_but_the_binary_and_the_prompt() {
        // Codex has no per-session flag for a skill library — verified against
        // 0.146.0 — so anything else on this command line would be a mistake.
        let args = argv(&launch(new_task(Stage::Off)));
        assert_eq!(args.len(), 2);
        assert_eq!(args[0], "codex");
    }

    #[test]
    fn a_bare_session_is_the_binary_and_the_language_sentence() {
        // It was the binary alone until the conversation language reached every
        // intent. Still nothing about the work — a bare session has none — and
        // still no flags, since Codex has no per-session skill mechanism.
        let args = argv(&launch(Intent::Bare));
        assert_eq!(args.len(), 2);
        assert_eq!(args[0], "codex");
        assert!(args[1].contains("Talk to me in English"), "{args:?}");
    }

    #[test]
    fn it_never_names_a_skill_registry_it_does_not_have() {
        let args = argv(&launch(new_task(Stage::On)));
        assert!(!args.last().unwrap().contains("superpowers:"));
    }

    #[test]
    fn auto_points_at_the_file_instead_of_pasting_it() {
        let args = argv(&launch(new_task(Stage::Auto)));
        let pointer = resources("superpowers").join("skills/brainstorming/SKILL.md");
        assert!(args.last().unwrap().contains(&pointer.display().to_string()));
    }

    #[test]
    fn the_filing_skill_is_pasted_in_whatever_the_switch_says() {
        // The branch's headline behaviour for a harness with no skill registry:
        // how this project wants a task worded is not part of the brainstorming
        // question, so the text travels in all three positions of the switch.
        // Reading it under `Stage::On` alone would still satisfy every
        // other test in this file.
        for mode in [Stage::Off, Stage::Auto, Stage::On] {
            let args = argv(&launch(new_task(mode)));
            assert!(
                args.last().unwrap().contains(FILING_BODY),
                "{mode:?}: the shipped filing skill never reached the prompt"
            );
        }
    }

    #[test]
    fn the_brainstorming_process_is_pasted_in_only_when_it_is_switched_on() {
        // 10 KB the agent may never use: `Off` has no business with it at all,
        // and `Auto` is handed the path instead so it pays only if it decides
        // the task warrants a conversation.
        for mode in [Stage::Off, Stage::Auto] {
            let args = argv(&launch(new_task(mode)));
            assert!(
                !args.last().unwrap().contains(BRAINSTORMING_BODY),
                "{mode:?}: the whole process was pasted in unasked"
            );
        }
        let args = argv(&launch(new_task(Stage::On)));
        assert!(
            args.last().unwrap().contains(BRAINSTORMING_BODY),
            "on must carry the process itself, there being no registry to name it in"
        );
    }

    #[test]
    fn the_plan_process_is_pasted_in_only_when_the_cascade_leaves_it_on() {
        // 7 KB, and the same rule the brainstorming body follows: `On` gets
        // the process, `Auto` gets the path, `Off` gets nothing — and none of
        // it is read from disk at all unless the two stages above it left the
        // plan standing.
        let asked = argv(&launch(staged(Stage::On, Stage::On, Stage::On, Vec::new())));
        assert!(
            asked.last().unwrap().contains(PLANS_BODY),
            "on must carry the process itself, there being no registry to name it in"
        );

        for intent in [
            staged(Stage::On, Stage::On, Stage::Auto, Vec::new()),
            staged(Stage::On, Stage::On, Stage::Off, Vec::new()),
            // The cascade's own cases: a plan asked for under a spec, or a
            // discussion, that is not On costs nothing either.
            staged(Stage::On, Stage::Off, Stage::On, Vec::new()),
            staged(Stage::Off, Stage::On, Stage::On, Vec::new()),
            staged(Stage::Auto, Stage::On, Stage::On, Vec::new()),
        ] {
            let args = argv(&launch(intent));
            assert!(
                !args.last().unwrap().contains(PLANS_BODY),
                "the whole process was pasted in unasked"
            );
        }
    }

    #[test]
    fn an_auto_plan_is_pointed_at_the_shipped_file() {
        let args = argv(&launch(staged(Stage::On, Stage::On, Stage::Auto, Vec::new())));
        let pointer = resources("superpowers").join("skills/writing-plans/SKILL.md");
        assert!(args.last().unwrap().contains(&pointer.display().to_string()), "{args:?}");
    }

    #[test]
    fn every_attached_image_rides_as_its_own_flag_in_front_of_the_prompt() {
        let args = argv(&launch(with_images(
            Stage::Off,
            vec!["/data/a.png".into(), "/data/b.png".into()],
        )));

        assert_eq!(
            args,
            vec!["codex", "-i", "/data/a.png", "-i", "/data/b.png", args.last().unwrap()],
            "the prompt is positional and everything else goes in front of it: {args:?}"
        );
    }

    #[test]
    fn the_paths_are_in_the_prompt_as_well_as_on_the_command_line() {
        // Not a duplicate: the flag is what Codex looks at, the path in the
        // prompt is what has to reach the issue description.
        let args = argv(&launch(with_images(Stage::Off, vec!["/data/a.png".into()])));
        assert!(args.last().unwrap().contains("/data/a.png"), "{args:?}");
    }

    #[test]
    fn a_task_with_no_images_gets_no_image_flag() {
        let args = argv(&launch(new_task(Stage::Off)));
        assert!(!args.iter().any(|a| a == "-i"), "{args:?}");
    }

    // The four fixtures below are whole 120x30 screens captured off a real
    // Codex CLI 0.146.0 under a PTY and rendered through the same vt100 the
    // worker reads: the update banner, the session header, the person's own
    // prompt, the agent's transcript and the dialog at the bottom. That is the
    // shape this reader actually has to survive, and a hand-written excerpt of
    // it would prove only that it survives an excerpt. Only the working
    // directory in them is edited, to keep somebody's home directory out of the
    // repository.
    fn fixture(name: &str) -> Vec<String> {
        let path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name);
        std::fs::read_to_string(path).unwrap().lines().map(str::to_owned).collect()
    }

    #[test]
    fn recognises_the_command_approval_dialog() {
        let q = question(&fixture("codex-0.146-approval-command.txt"))
            .expect("the dialog went unrecognised");
        assert_eq!(q.text, "Would you like to run the following command?");
        assert_eq!(q.options.len(), 3);
        assert_eq!(q.options[0].label, "Yes, proceed (y)");
        assert_eq!(q.options[0].send, "1\r");
        assert_eq!(q.options[2].label, "No, and tell Codex what to do differently (esc)");
        assert_eq!(q.selected, Some(0));
    }

    #[test]
    fn the_command_preview_under_the_question_does_not_become_the_question() {
        // The command dialog puts `Environment: local` and `$ touch probe.txt`
        // *between* the question and the options, which is the opposite of
        // Claude Code's layout. Reading the run of text directly above the
        // options — which is exactly what the Claude reader does — would hand
        // back the shell command and then reject the whole dialog for not
        // ending in a question mark.
        let q = question(&fixture("codex-0.146-approval-command.txt"))
            .expect("the dialog went unrecognised");
        assert!(!q.text.contains("touch probe.txt"), "{}", q.text);
    }

    #[test]
    fn recognises_the_edit_approval_dialog() {
        // Here the preview is a diff and it sits *above* the question, inside
        // the agent's own transcript entry.
        let q = question(&fixture("codex-0.146-approval-patch.txt"))
            .expect("the dialog went unrecognised");
        assert_eq!(q.text, "Would you like to make the following edits?");
        assert_eq!(q.options.len(), 3);
        assert_eq!(q.options[1].label, "Yes, and don't ask again for these files (a)");
        assert_eq!(q.selected, Some(0));
    }

    #[test]
    fn a_diff_above_the_question_does_not_reach_the_text() {
        let q = question(&fixture("codex-0.146-approval-patch.txt"))
            .expect("the dialog went unrecognised");
        assert!(!q.text.contains("hello"), "{}", q.text);
        assert!(!q.text.contains("bye"), "{}", q.text);
    }

    #[test]
    fn recognises_the_trust_prompt_a_session_opens_on() {
        // The first thing Codex draws in a directory it has not been trusted
        // in, before the agent has done anything at all. It is a question with
        // a person's answer required, so it is one of these — and reading it is
        // worth as much as reading an approval, since a session sat on this one
        // never starts.
        let q = question(&fixture("codex-0.146-trust-directory.txt"))
            .expect("the trust prompt went unrecognised");
        assert_eq!(q.text, "Do you trust the contents of this directory?");
        assert_eq!(q.options.len(), 2);
        assert_eq!(q.options[1].label, "No, quit");
        assert_eq!(q.selected, Some(0));
    }

    #[test]
    fn the_explanation_under_the_trust_prompt_is_not_part_of_the_question() {
        // Two sentences of prose follow the question mark in the same
        // paragraph, wrapped across two rows. What a person is being asked is
        // the first sentence.
        let q = question(&fixture("codex-0.146-trust-directory.txt"))
            .expect("the trust prompt went unrecognised");
        assert!(!q.text.contains("prompt injection"), "{}", q.text);
    }

    #[test]
    fn an_agent_at_work_is_not_waiting_on_anybody() {
        // A whole real screen mid-turn: the banner, the person's prompt behind
        // the cursor glyph, the spinner, and the composer's own placeholder
        // behind that same glyph at the foot. Two `›` on it and nothing to
        // answer.
        assert!(
            question(&fixture("codex-0.146-working.txt")).is_none(),
            "an agent at work was read as waiting on a person"
        );
    }

    #[test]
    fn a_numbered_list_the_agent_printed_is_not_a_dialog() {
        // Ends in a question mark and is numbered. Nothing points at it, which
        // is what a dialog always has and prose never does.
        let screen: Vec<String> = [
            "• Which of these would you like me to do first?",
            "  1. Rename the module",
            "  2. Split the test file",
            "  3. Neither, stop here",
        ]
        .iter()
        .map(|s| (*s).to_owned())
        .collect();
        assert!(question(&screen).is_none(), "prose with a numbered list was taken for a dialog");
    }

    #[test]
    fn a_list_in_the_output_above_does_not_shadow_the_dialog() {
        // Both blocks number themselves from 1. The dialog is the lower one —
        // it replaces the composer at the foot of the screen, and whatever the
        // agent printed is by definition above it.
        let screen: Vec<String> = [
            "• I considered three options:",
            "  1. Rename the module",
            "  2. Split the test file",
            "",
            "",
            "  Would you like to run the following command?",
            "",
            "  $ ls",
            "",
            "\u{203a} 1. Yes, proceed (y)",
            "  2. No, and tell Codex what to do differently (esc)",
        ]
        .iter()
        .map(|s| (*s).to_owned())
        .collect();
        let q = question(&screen).expect("the dialog was shadowed by the output above it");
        assert_eq!(q.text, "Would you like to run the following command?");
        assert_eq!(q.options.len(), 2, "the printed list was swept in with the dialog's own");
        assert_eq!(q.options[0].label, "Yes, proceed (y)");
    }

    #[test]
    fn a_persons_own_numbered_prompt_is_not_a_dialog() {
        // Typed into the composer, it is a numbered line with the cursor glyph
        // in front of it and a question directly above. One option is not a
        // dialog: every one Codex draws offers a way out as well as a way on.
        let screen: Vec<String> = [
            "  Which module should this live in?",
            "",
            "\u{203a} 1. the one you suggested",
        ]
        .iter()
        .map(|s| (*s).to_owned())
        .collect();
        assert!(question(&screen).is_none(), "a person's own prompt was taken for a dialog");
    }

    #[test]
    fn a_numbered_draft_left_in_the_composer_is_not_a_dialog() {
        // The false positive this reader's first version had, captured whole
        // rather than argued about: a real session at 60 columns with a
        // two-line numbered draft sitting unsent in the composer, and the
        // agent's own answer above it ending — on an indented continuation row
        // — in a question mark. Two options, the cursor on the first, prose
        // above indented exactly as a dialog's is. Everything a dialog has
        // except being one.
        //
        // The agent was asked for two paragraphs each ending in a question
        // mark, so the shape was provoked rather than stumbled on; every row of
        // it is Codex's own rendering, which is the part that had to be real.
        //
        // What tells them apart is that those rows hang off the `•` at column 0
        // that owns them, and a dialog hangs off nothing.
        assert!(
            question(&fixture("codex-0.146-draft-in-the-composer.txt")).is_none(),
            "a busy agent with a draft in its composer was read as waiting on a person"
        );
    }

    #[test]
    fn a_later_paragraph_of_the_agents_own_answer_is_not_the_dialogs_question() {
        // The same shape reduced to its bones. Measured on real screens at 60
        // and 120 columns: one blank row parts two paragraphs of the same
        // entry, two part one block from the next — so the blank above the
        // second paragraph says nothing, and the glue to the bullet says
        // everything.
        let screen: Vec<String> = [
            "• I can rename it, but the test file carries the old",
            "  name too. Should I rename that one as well?",
            "",
            "  A second paragraph of the same answer, which also",
            "  happens to end in a question mark, correct?",
            "",
            "",
            "\u{203a} 1. rename the module",
            "  2. split the test file",
        ]
        .iter()
        .map(|s| (*s).to_owned())
        .collect();
        assert!(question(&screen).is_none(), "the agent's own prose was read as a dialog");
    }

    #[test]
    fn a_question_mark_used_as_an_operator_is_not_the_question() {
        // The command preview sits below the question in this dialog, so it is
        // the paragraph reached first, and a ternary in it carries a question
        // mark followed by a space. Detection would be right either way; the
        // text handed over would be a shell fragment. A paragraph that is a
        // question outright wins over one that merely opens with something
        // shaped like one.
        let screen: Vec<String> = [
            "  Would you like to run the following command?",
            "",
            "  $ node -e \"console.log(x ? 1 : 2)\"",
            "",
            "\u{203a} 1. Yes, proceed (y)",
            "  2. No, and tell Codex what to do differently (esc)",
        ]
        .iter()
        .map(|s| (*s).to_owned())
        .collect();
        let q = question(&screen).expect("the dialog went unrecognised");
        assert_eq!(q.text, "Would you like to run the following command?");
    }

    #[test]
    fn a_stray_number_further_down_the_screen_does_not_join_the_list() {
        // Nothing consumes `options` today, but a list that quietly swallows
        // whatever is below it would hand a person an answer that is not on
        // their screen the day something does.
        let screen: Vec<String> = [
            "  Would you like to run the following command?",
            "",
            "\u{203a} 1. Yes, proceed (y)",
            "  2. No, and tell Codex what to do differently (esc)",
            "",
            "",
            "• 3. something the agent printed underneath",
        ]
        .iter()
        .map(|s| (*s).to_owned())
        .collect();
        let q = question(&screen).expect("the dialog went unrecognised");
        assert_eq!(q.options.len(), 2, "a line from below the dialog joined its options");
    }

    #[test]
    fn the_update_prompt_names_itself_and_is_read_without_asking_anything() {
        // A real screen at 60 columns, and the named exception in one place.
        // This prompt states rather than asks — there is no question mark
        // anywhere on it — so the ordinary reading declines, and what answers
        // is the `HEADINGS` table: the banner is a string this dialog prints
        // and ordinary output does not. Before that a session started on the
        // day an update was out simply stood here, read as idle, with nothing
        // on screen to say a person was needed (smetana-9qp).
        //
        // The text is the banner as Codex draws it, glyph and versions and
        // all — see `headed` for why nothing is composed and nothing trimmed.
        let q = question(&fixture("codex-0.146-update-prompt.txt"))
            .expect("the update prompt went unrecognised");
        assert_eq!(q.text, "\u{2728} Update available! 0.146.0 -> 0.147.0");
        assert_eq!(q.options.len(), 3);
        assert_eq!(
            q.options[0].label,
            "Update now (runs `sh -c 'curl -fsSL https://chatgpt.com/codex/install.sh | \
             CODEX_NON_INTERACTIVE=1 sh'`)",
            "the wrapped first option was cut at the column the pane happened to wrap"
        );
        assert_eq!(q.options[1].label, "Skip");
        assert_eq!(q.options[2].label, "Skip until next version");
        assert_eq!(q.selected, Some(0));
    }

    #[test]
    fn the_same_screen_without_its_banner_is_still_refused() {
        // The general rule did not move. This is the update prompt with the one
        // line that names it swapped for ordinary prose: same options, same
        // cursor, same block, and still no question mark anywhere. A dialog
        // that does not name itself has to ask, and this one does not, so it is
        // declined exactly as it was before the exception existed.
        let screen: Vec<String> = [
            "",
            "  Release notes: https://github.com/openai/codex/releases/la",
            "",
            "\u{203a} 1. Update now",
            "  2. Skip",
            "  3. Skip until next version",
        ]
        .iter()
        .map(|s| (*s).to_owned())
        .collect();
        assert!(
            question(&screen).is_none(),
            "the named exception was widened into the general rule"
        );
    }

    #[test]
    fn a_banner_does_not_excuse_a_list_nobody_is_pointing_at() {
        // The heading opens one narrower reading of the *text* and nothing
        // else. Every structural guard is still in front of it, and the cursor
        // is the one that keeps a numbered list in the agent's own output from
        // reading as a dialog — a heading must not buy its way past that.
        let screen: Vec<String> = [
            "",
            "  \u{2728} Update available! 0.146.0 -> 0.147.0",
            "",
            "  1. Update now",
            "  2. Skip",
        ]
        .iter()
        .map(|s| (*s).to_owned())
        .collect();
        assert!(
            question(&screen).is_none(),
            "an unpointed list under a banner was read as a dialog"
        );
    }

    #[test]
    fn a_question_mark_inside_a_command_preview_is_not_a_question() {
        // The paragraph directly above the options is the command Codex is
        // asking to run, and it happens to carry a question mark inside a
        // quoted argument. The real question is the paragraph above that.
        let screen: Vec<String> = [
            "  Would you like to run the following command?",
            "",
            "  $ grep -n \"what?\" README.md",
            "",
            "\u{203a} 1. Yes, proceed (y)",
            "  2. No, and tell Codex what to do differently (esc)",
        ]
        .iter()
        .map(|s| (*s).to_owned())
        .collect();
        let q = question(&screen).expect("the dialog went unrecognised");
        assert_eq!(q.text, "Would you like to run the following command?");
    }

    #[test]
    fn a_dialog_with_nothing_pointing_at_it_is_not_read() {
        // The same screen as the one that matches, with the cursor taken off
        // it. Nothing is waiting on a list nobody is pointing at, and this is
        // what a half-drawn dialog looks like on the way in.
        let screen: Vec<String> = [
            "  Would you like to run the following command?",
            "",
            "  1. Yes, proceed (y)",
            "  2. No, and tell Codex what to do differently (esc)",
        ]
        .iter()
        .map(|s| (*s).to_owned())
        .collect();
        assert!(question(&screen).is_none(), "an unpointed list was taken for a dialog");
    }

    #[test]
    fn a_turn_whose_first_paragraph_is_one_row_is_still_a_turn() {
        // The second version of the block rules refused a block only when its
        // first row was glued to the turn above with no blank between. When the
        // turn's first paragraph is a single row, the row under it is the blank
        // before the turn's *second* paragraph, so nothing looked glued and
        // this read as a dialog with nothing to answer.
        let screen: Vec<String> = [
            "• Done — tests pass.",
            "",
            "  Do you want me to also update the changelog?",
            "",
            "",
            "\u{203a} 1. yes update it",
            "  2. no leave it",
        ]
        .iter()
        .map(|s| (*s).to_owned())
        .collect();
        assert!(question(&screen).is_none(), "the agent's own one-line turn was read as a dialog");
    }

    #[test]
    fn a_scrolled_screen_with_nothing_to_anchor_it_is_a_known_gap() {
        // Pinned as it behaves, not as it should: when the turn that owns the
        // prose has scrolled off the top there is no anchor on the screen at
        // all, and this reads as a dialog. Closing it means requiring a block
        // to hang off something, which would refuse a dialog drawn at the top
        // of an unscrolled screen — `codex-0.146-update-prompt.txt` is one, and
        // it starts at row 0. Recorded rather than guessed at; if this
        // assertion ever needs inverting, that is the trade being made.
        let screen: Vec<String> =
            ["  is that alright with you?", "", "", "\u{203a} 1. yes", "  2. no"]
                .iter()
                .map(|s| (*s).to_owned())
                .collect();
        assert!(question(&screen).is_some(), "the known gap closed on its own — read the doc");
    }

    #[test]
    fn a_wrapped_option_label_is_gathered_whole() {
        // A real wrapped label, three rows of it, off the update prompt at 60
        // columns. Skipping the continuations — which the first version of this
        // did — left a person being offered
        // `Update now (runs \`sh -c 'curl -fsSL`, cut at whatever column the
        // pane happened to wrap. The two pieces are called directly here, so
        // that a regression in the gathering is told apart from one in the
        // reading around it.
        let screen = fixture("codex-0.146-update-prompt.txt");
        let row = screen.iter().position(|l| option_line(l).is_some()).expect("an option is there");
        let (index, head, selected) = option_line(&screen[row]).expect("an option is there");
        assert_eq!(index, 1);
        assert!(selected, "the cursor is on the first option");
        assert_eq!(
            wrapped(&screen, row, &head),
            "Update now (runs `sh -c 'curl -fsSL https://chatgpt.com/codex/install.sh | \
             CODEX_NON_INTERACTIVE=1 sh'`)"
        );
    }

    #[test]
    fn a_wrapped_option_label_does_not_end_the_list() {
        // A narrow pane wraps the long second option, and the continuation row
        // is neither an option nor the end of the block. Losing option 3 would
        // hide the only way out of the dialog.
        let screen: Vec<String> = [
            "  Would you like to run the following command?",
            "",
            "\u{203a} 1. Yes, proceed (y)",
            "  2. Yes, and don't ask again for commands that start",
            "     with `touch probe.txt` (p)",
            "  3. No, and tell Codex what to do differently (esc)",
        ]
        .iter()
        .map(|s| (*s).to_owned())
        .collect();
        let q = question(&screen).expect("the dialog went unrecognised");
        assert_eq!(q.options.len(), 3);
        assert_eq!(
            q.options[1].label,
            "Yes, and don't ask again for commands that start with `touch probe.txt` (p)",
            "the label was cut at the column the pane happened to wrap"
        );
        assert_eq!(q.options[2].label, "No, and tell Codex what to do differently (esc)");
    }

    #[test]
    fn a_question_the_agent_asked_in_prose_above_the_dialog_is_not_the_dialogs() {
        // The turn directly above ends in a question mark of its own. What
        // separates it from the dialog here is the double blank Codex puts
        // between one top-level block and the next — the walk stops on that and
        // never reaches the `•`, so this pins the blank-pair rule rather than
        // the column-0 one. Without it the dialog's question would be whatever
        // the agent last said.
        let screen: Vec<String> = [
            "• Should I also rename the test file?",
            "",
            "",
            "  Would you like to run the following command?",
            "",
            "\u{203a} 1. Yes, proceed (y)",
            "  2. No, and tell Codex what to do differently (esc)",
        ]
        .iter()
        .map(|s| (*s).to_owned())
        .collect();
        let q = question(&screen).expect("the dialog went unrecognised");
        assert_eq!(q.text, "Would you like to run the following command?");
    }

    #[test]
    fn a_dialog_with_no_question_in_it_is_not_read() {
        // Options and a cursor, and nothing above them that reads as a
        // question. Layer A still says somebody is waiting; this reader says
        // nothing rather than handing over a line of preview as the question.
        let screen: Vec<String> = [
            "  Environment: local",
            "",
            "\u{203a} 1. Yes, proceed (y)",
            "  2. No, and tell Codex what to do differently (esc)",
        ]
        .iter()
        .map(|s| (*s).to_owned())
        .collect();
        assert!(question(&screen).is_none());
    }

    #[test]
    fn it_does_not_read_a_dialog_drawn_by_another_cli() {
        // Claude Code's boxed dialog, frame and all. Reading it would be a lie
        // about which agent is running, and `detect` picks the reader by the
        // session's own profile precisely so that it cannot happen.
        let q = question(&fixture("claude-permission-dialog.txt"));
        assert!(q.is_none(), "codex read a dialog drawn by another CLI");
    }

    fn run(mode: crate::runs::model::RunMode) -> Intent {
        Intent::Run {
            settings: crate::runs::model::RunSettings {
                scope: crate::runs::model::RunScope::Queue,
                mode,
                target_branch: "staging".into(),
                create_target: false,
                min_priority: Some(2),
                // None in Solo, the way `RunSettings::validate` requires.
                max_parallel_tasks: (!matches!(mode, crate::runs::model::RunMode::Solo)).then_some(3),
                live_check: true,
                file_findings: true,
            },
        }
    }

    #[test]
    fn an_unattended_batch_gets_the_current_flag_and_never_the_removed_one() {
        use crate::runs::model::RunMode;
        let args = argv(&launch(run(RunMode::Auto)));
        assert!(args.iter().any(|a| a == "--dangerously-bypass-approvals-and-sandbox"), "{args:?}");
        assert!(!args.iter().any(|a| a == "--full-auto"), "Codex removed it");

        for mode in [RunMode::Supervised, RunMode::Solo] {
            let args = argv(&launch(run(mode)));
            assert!(args.len() == 2, "{mode:?}: just the binary and the prompt, {args:?}");
        }
    }

    #[test]
    fn the_flag_goes_in_front_of_the_positional_prompt() {
        let args = argv(&launch(run(crate::runs::model::RunMode::Auto)));
        assert_eq!(args[1], "--dangerously-bypass-approvals-and-sandbox");
        assert_eq!(args.len(), 3, "binary, flag, prompt: {args:?}");
    }
}
