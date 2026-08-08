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
use super::{prompt, Autonomy, Brainstorm, ImageDelivery, Intent, Launch, Profile, SkillDelivery};
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
            Intent::NewTask { brainstorm: Brainstorm::On, .. }
        );
        let filing =
            filing_a_task.then(|| read_skill(&launch.skills.smetana, "filing-a-task")).flatten();
        let brainstorming_text =
            discussing.then(|| read_skill(&launch.skills.superpowers, "brainstorming")).flatten();
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
            brainstorming: brainstorming_text.as_deref(),
        };
        if let Some(built) = prompt::build(
            &launch.intent,
            self.delivery(),
            self.images(),
            &launch.skills,
            launch.facts.as_deref(),
            text,
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
/// The label keeps the shortcut hint Codex prints after it — `(y)`, `(p)`,
/// `(esc)`. It is what a person reads on the screen, and trimming it would mean
/// this app deciding which part of somebody else's wording is the real label.
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

/// A dialog's own body line: the question, the command preview, the diff under
/// it. Codex indents everything it draws inside a dialog and starts every
/// transcript entry — the person's prompt, the agent's bullets — hard against
/// column 0, which is the only structural boundary between the two on a screen
/// with no frame anywhere on it.
///
/// A blank line counts, because the dialog is several paragraphs.
fn is_body(line: &str) -> bool {
    line.trim().is_empty() || line.starts_with(char::is_whitespace)
}

/// The question inside one paragraph of a dialog, or `None` if there is none.
///
/// Two shapes, both taken from real screens. `Would you like to run the
/// following command?` is the whole paragraph. The trust prompt is a question
/// followed by two sentences explaining it, and only the first is what a person
/// is being asked — so a question mark with whitespace after it ends the
/// question too.
///
/// That "with whitespace after it" is not decoration: the paragraph above the
/// options can be a shell command Codex is asking to run, and `grep "what?"
/// file` must not be read as a question. A question mark that ends a sentence
/// is followed by a space or by the end of the line; one inside a quoted
/// argument is followed by the quote.
fn sentence(text: &str) -> Option<String> {
    if text.ends_with('?') {
        return Some(text.to_owned());
    }
    let mut chars = text.char_indices().peekable();
    while let Some((at, c)) = chars.next() {
        if c == '?' && chars.peek().is_some_and(|(_, next)| next.is_whitespace()) {
            return Some(text[..=at].to_owned());
        }
    }
    None
}

/// Codex's approval dialog: a paragraph of text and numbered options, drawn
/// straight onto the transcript with no frame around it.
///
/// Read off Codex CLI 0.146.0 — the command dialog, the edit dialog and the
/// trust-this-directory prompt are all in `tests/fixtures/`, captured under a
/// PTY at 120x30 and rendered through the same vt100 the worker reads. Every
/// discriminator below is a property of those screens, and every one of them
/// costs a **miss** if that CLI changes it — never a false match, which is the
/// direction that matters: a session wrongly turned `needs-you` would put a
/// loud row in a panel budgeted at one or two, and would make
/// `terminal_run_capture` refuse to write to a session with nothing open on it.
///
/// - the options number themselves 1, 2, 3 … and the **last** such run on the
///   screen is the dialog, since anything the agent merely printed sits above
///   it. Same reasoning as the Claude reader, and it holds for the same reason:
///   the dialog replaces the composer at the foot of the screen;
/// - **exactly one** option carries the cursor. Prose never does, and a live
///   dialog always does. More than one would not be this dialog at all;
/// - there are **at least two** options. Every dialog Codex draws offers a way
///   out as well as a way on, and the floor is what keeps a person's own
///   prompt out: `› 1. Fix the bug` typed into the composer is a numbered line
///   with the cursor on it, and it is alone;
/// - the question is in the block directly above the options, and that block is
///   bounded by indentation — Codex indents a dialog's own lines and starts
///   every transcript entry at column 0;
/// - and something in that block has to read as a question. Bottom-most
///   paragraph first, because a command preview sits *below* the question in
///   the command dialog and a diff sits *above* it in the edit one, so the
///   nearest one that reads as a question is the right answer for both.
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
    for (_, (index, label, is_selected)) in &marks[start..] {
        // A gap in the numbering is the end of this block, not a hole in it.
        // Rows are not required to be adjacent: a long label wraps, and the
        // continuation line is neither an option nor the end of the list.
        if *index != expected {
            break;
        }
        if *is_selected {
            cursors += 1;
            selected = Some(options.len());
        }
        // Verified against 0.146.0 by answering a live dialog with these very
        // bytes: the digit selects and the return confirms, the same as Claude
        // Code, which is what the footer means by "press enter to confirm".
        options.push(QuestionOption { label: label.clone(), send: format!("{index}\r") });
        expected += 1;
    }

    if cursors != 1 || options.len() < 2 {
        return None;
    }

    // Upwards from the options over the dialog's own indented lines, stopping
    // at the first thing drawn hard against column 0 — the agent's last bullet,
    // the person's prompt, or the banner at the top of the session.
    let first = marks[start].0;
    let mut top = first;
    while top > 0 && is_body(&screen[top - 1]) {
        top -= 1;
    }

    // Bottom-most paragraph first. `split_whitespace` drops the dialog's own
    // indent and collapses any run of whitespace to one space, so a question
    // wrapped across rows comes back as one line.
    let mut paragraphs: Vec<String> = Vec::new();
    for line in &screen[top..first] {
        if line.trim().is_empty() {
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

    let text = paragraphs.iter().rev().find_map(|paragraph| sentence(paragraph))?;
    Some(Question { text, options, selected })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::library::Skills;
    use crate::agents::{Brainstorm, Intent, Launch, TaskDraft};
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
        }
    }

    /// A line from the middle of each shipped `SKILL.md`, far enough in to be
    /// the body rather than the front matter: finding it in the prompt is the
    /// only proof that the file was read and pasted, not merely named.
    const FILING_BODY: &str = "The title says what needs doing";
    const BRAINSTORMING_BODY: &str = "ask questions one at a time";

    fn argv(launch: &Launch) -> Vec<String> {
        Codex
            .command(launch)
            .get_argv()
            .iter()
            .map(|s| s.to_string_lossy().into_owned())
            .collect()
    }

    fn new_task(brainstorm: Brainstorm) -> Intent {
        with_images(brainstorm, Vec::new())
    }

    fn with_images(brainstorm: Brainstorm, images: Vec<String>) -> Intent {
        Intent::NewTask {
            brainstorm,
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
        let args = argv(&launch(new_task(Brainstorm::Off)));
        assert_eq!(args.len(), 2);
        assert_eq!(args[0], "codex");
    }

    #[test]
    fn a_bare_session_is_just_the_binary() {
        assert_eq!(argv(&launch(Intent::Bare)), vec!["codex".to_string()]);
    }

    #[test]
    fn it_never_names_a_skill_registry_it_does_not_have() {
        let args = argv(&launch(new_task(Brainstorm::On)));
        assert!(!args.last().unwrap().contains("superpowers:"));
    }

    #[test]
    fn auto_points_at_the_file_instead_of_pasting_it() {
        let args = argv(&launch(new_task(Brainstorm::Auto)));
        let pointer = resources("superpowers").join("skills/brainstorming/SKILL.md");
        assert!(args.last().unwrap().contains(&pointer.display().to_string()));
    }

    #[test]
    fn the_filing_skill_is_pasted_in_whatever_the_switch_says() {
        // The branch's headline behaviour for a harness with no skill registry:
        // how this project wants a task worded is not part of the brainstorming
        // question, so the text travels in all three positions of the switch.
        // Reading it under `Brainstorm::On` alone would still satisfy every
        // other test in this file.
        for mode in [Brainstorm::Off, Brainstorm::Auto, Brainstorm::On] {
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
        for mode in [Brainstorm::Off, Brainstorm::Auto] {
            let args = argv(&launch(new_task(mode)));
            assert!(
                !args.last().unwrap().contains(BRAINSTORMING_BODY),
                "{mode:?}: the whole process was pasted in unasked"
            );
        }
        let args = argv(&launch(new_task(Brainstorm::On)));
        assert!(
            args.last().unwrap().contains(BRAINSTORMING_BODY),
            "on must carry the process itself, there being no registry to name it in"
        );
    }

    #[test]
    fn every_attached_image_rides_as_its_own_flag_in_front_of_the_prompt() {
        let args = argv(&launch(with_images(
            Brainstorm::Off,
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
        let args = argv(&launch(with_images(Brainstorm::Off, vec!["/data/a.png".into()])));
        assert!(args.last().unwrap().contains("/data/a.png"), "{args:?}");
    }

    #[test]
    fn a_task_with_no_images_gets_no_image_flag() {
        let args = argv(&launch(new_task(Brainstorm::Off)));
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
        // in front of it and a question directly above. The floor of two
        // options is the whole of what keeps it out.
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
        assert_eq!(q.options[2].label, "No, and tell Codex what to do differently (esc)");
    }

    #[test]
    fn a_question_the_agent_asked_in_prose_above_the_dialog_is_not_the_dialogs() {
        // The transcript entry directly above ends in a question mark and is
        // indented in its continuation rows. Column 0 is the boundary between
        // the two, and without it the question of the dialog would be whatever
        // the agent last said.
        let screen: Vec<String> = [
            "• Should I also rename the test file?",
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
