//! Claude Code.
//!
//! Screen templates for specific CLIs. Data, not logic: moving them into
//! configuration when that becomes necessary should be a relocation, not a
//! rewrite.
//!
//! This reads someone else's interface, and an agent's major update breaks
//! it. It breaks softly: no match leaves layer A in place, and the app says
//! "someone is waiting" instead of "here is the question" — it does not lie.

use portable_pty::CommandBuilder;

use super::{prompt, Autonomy, Intent, Launch, Profile, SkillDelivery};
use crate::runs::model::RunMode;
use crate::runs::usage::Usage;
use crate::terminal::model::{Question, QuestionOption};

pub struct Claude;

impl Profile for Claude {
    fn id(&self) -> &'static str {
        "claude"
    }

    fn binary(&self) -> &'static str {
        "claude"
    }

    fn delivery(&self) -> SkillDelivery {
        SkillDelivery::PluginDir
    }

    /// `--plugin-dir` loads a plugin for this session only: nothing is
    /// installed and the person's own configuration is not touched.
    ///
    /// The vendored superpowers copy keeps its own name rather than being
    /// folded into ours, which is what lets the prompt say
    /// `superpowers:brainstorming` in both cases. It is withheld when the
    /// person has their own — two plugins of the same name is a choice the
    /// agent would make for us.
    fn command(&self, launch: &Launch) -> CommandBuilder {
        let mut cmd = CommandBuilder::new(self.binary());
        cmd.arg("--plugin-dir");
        cmd.arg(&launch.skills.smetana);
        if !launch.skills.superpowers_installed {
            cmd.arg("--plugin-dir");
            cmd.arg(&launch.skills.superpowers);
        }
        // Before the prompt, which is positional: a flag after it would rely
        // on the CLI's parser being relaxed about the order, and this app does
        // not get to assume that about somebody else's argument grammar.
        if let Intent::Run { settings } = &launch.intent {
            for arg in self.autonomy(settings.mode).args {
                cmd.arg(arg);
            }
        }
        // Nothing is read from disk here: both plugins are loaded, so the
        // prompt names the skills and Claude Code fetches them on demand.
        // Attached images are not on this command line either, and for a
        // harder reason: Claude Code has no flag for one. It opens an image
        // when the prompt names its path, which is what `ImageDelivery::InPrompt`
        // — the default this profile keeps — asks `prompt.rs` to write.
        let text =
            prompt::SkillText { filing: None, resolving: None, brainstorming: None, plans: None };
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

    /// `Auto` means nobody is there to answer a permission prompt, so the run
    /// would sit on the first one forever. The other two modes have a person,
    /// and taking their prompts away would take away the thing that makes them
    /// different.
    ///
    /// The environment variable goes on in every mode, and it is not about
    /// permissions at all: the CLI stops waiting on its own background tasks at
    /// a ten-minute default and carries on without them. That is fine for a
    /// person watching one answer arrive and wrong for a batch — the source
    /// this was ported from lost workers mid-task to it, with nothing in the
    /// output to say a task had been abandoned rather than finished. Zero means
    /// no ceiling.
    fn autonomy(&self, mode: RunMode) -> Autonomy {
        Autonomy {
            args: match mode {
                RunMode::Auto => vec!["--permission-mode", "bypassPermissions"],
                RunMode::Supervised | RunMode::Solo => vec![],
            },
            env: vec![("CLAUDE_CODE_PRINT_BG_WAIT_CEILING_MS", "0")],
        }
    }

    /// `/usage` is a slash command of the interactive interface, and `-p` runs
    /// one anyway and prints what it would have drawn. There is no
    /// machine-readable form of this and none has appeared: as of 2.1.174 the
    /// command grew a *more* interactive shape in the terminal — day and week
    /// views switched with `d` and `w` — while `-p` kept printing the same
    /// plain text it always did.
    fn usage_command(&self) -> Option<&'static [&'static str]> {
        Some(&["-p", "/usage"])
    }

    fn parse_usage(&self, output: &str) -> Option<Usage> {
        usage(output)
    }
}

/// The two lines that carry a plan limit, verbatim from `claude -p "/usage"`:
///
/// ```text
/// Current session: 10% used · resets Aug 7 at 8pm (Europe/Moscow)
/// Current week (all models): 20% used · resets Aug 11 at 5:59pm (Europe/Moscow)
/// Current week (Fable): 0% used
/// ```
///
/// The third is a per-model allowance and is deliberately not read. Claude
/// Code's own documentation answers an exhausted Opus or Fable limit with
/// "switch models", not "stop", and the CLI does that itself — pausing a run
/// for it would hold up work the harness was about to carry on with anyway.
/// The prefix below is exact for the same reason, so `(Fable)` cannot match
/// `(all models)`.
const SESSION: &str = "Current session:";
const WEEK: &str = "Current week (all models):";

fn usage(output: &str) -> Option<Usage> {
    let mut usage = Usage::default();
    let mut read_one = false;
    for line in output.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix(SESSION) {
            if let Some((pct, resets)) = used(rest) {
                usage.session_pct = pct;
                usage.session_reset = resets;
                read_one = true;
            }
        } else if let Some(rest) = line.strip_prefix(WEEK) {
            if let Some((pct, resets)) = used(rest) {
                usage.week_pct = pct;
                usage.week_reset = resets;
                read_one = true;
            }
        }
    }
    // One of the two is enough. A rename of either line loses that half and
    // leaves it reading zero, which is the source's own behaviour and the
    // benign direction to be wrong in: a run keeps going and finds the wall by
    // hitting it, rather than pausing forever on a limit nobody has reached.
    read_one.then_some(usage)
}

/// ` 10% used · resets Aug 7 at 8pm (Europe/Moscow)` → `(10, Some("Aug 7 at 8pm (Europe/Moscow)"))`.
///
/// The reset half is optional and its absence is not a failure: a fresh
/// allowance prints no reset at all, and a percentage with nothing to say about
/// when it clears is still the number the decision is made on.
fn used(rest: &str) -> Option<(u8, Option<String>)> {
    let rest = rest.trim_start();
    let percent = rest.find('%')?;
    let pct: u8 = rest[..percent].trim().parse().ok()?;
    const RESETS: &str = "resets ";
    let tail = &rest[percent + 1..];
    let resets = tail
        .find(RESETS)
        .map(|at| tail[at + RESETS.len()..].trim().to_owned())
        .filter(|text| !text.is_empty());
    Some((pct, resets))
}

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

/// One line of the dialog's own chrome and nothing else: a box edge, a full
/// width rule, the dashed rule that fences a diff preview off from the
/// question under it. Box drawing characters occupy one contiguous Unicode
/// block, which is the whole test — naming the handful in use today would
/// mean a silent miss the first time the CLI reaches for another.
fn is_rule(line: &str) -> bool {
    !line.is_empty()
        && line.chars().all(|c| c.is_whitespace() || ('\u{2500}'..='\u{257F}').contains(&c))
}

/// One screen line as this reader wants it: without the frame it used to
/// carry, so both shapes of the dialog are read the same way.
fn strip(line: &str) -> &str {
    line.trim_matches(['│', ' '])
}

/// The words of a run of lines, in reading order, as one line.
///
/// `split_whitespace` both drops the dialog's own padding and collapses any
/// run of whitespace to one space, so a question wrapped across rows comes
/// back joined.
fn joined(lines: &[&str]) -> String {
    lines.iter().flat_map(|l| l.split_whitespace()).collect::<Vec<_>>().join(" ")
}

/// The headings of the dialogs that do **not** put their question in the
/// paragraph directly above the options, one literal per dialog.
///
/// There is one so far: the folder-trust dialog, asked once the first time
/// Claude Code is started somewhere it has not been before. It numbers its
/// options from 1 and points at one of them like every other dialog, so it is
/// only the text that goes wrong — under the heading come the path, the
/// question, a sentence about what the agent will be able to do and a link
/// caption, and it is the caption, "Security guide", that sits directly above
/// the options (smetana-xh7).
///
/// Neither of the two properties that keep the permission dialog apart from
/// prose is relaxed to read it. Widening the search past a blank line for the
/// whole reader would drag a diff preview and a title into the permission
/// dialog's question; dropping the question mark for the whole reader would
/// leave the cursor as the only guard, and a loud row is budgeted at one or
/// two a screen. So the wider search is opened by a literal string this dialog
/// prints and ordinary output does not, and it stays fenced: the question is
/// looked for between that heading and the options, never above it and never
/// in what the agent itself wrote. A wording change on the other side loses
/// the reading and leaves layer A in place, which is how the rest of this
/// file already fails.
const HEADINGS: &[&str] = &["Accessing workspace:"];


/// Claude Code's permission dialog: a question line and numbered options.
///
/// It was a box until 2.1, and the frame around it was what told it apart
/// from any numbered list in the agent's own output. That frame is gone —
/// today the dialog is fenced off by full width rules and its lines are
/// bare — so two other properties carry that weight instead, and both are
/// things ordinary output does not do:
///
/// - the options number themselves 1, 2, 3 … and the **last** such run on
///   the screen is the dialog, since anything the agent merely printed sits
///   above it;
/// - exactly one of them carries the cursor. A list in a paragraph of prose
///   never does, and a live dialog always does.
///
/// The question is still the text directly above the options and still has
/// to end in a question mark — except for the handful of dialogs that print
/// a heading of their own and lay their text out some other way, which
/// `HEADINGS` names one by one.
fn question(screen: &[String]) -> Option<Question> {
    let lines: Vec<&str> = screen.iter().map(|l| strip(l)).collect();

    // Every numbered line on the screen, the agent's own among them.
    let marks: Vec<(usize, (usize, String, bool))> =
        lines.iter().enumerate().filter_map(|(i, l)| option_line(l).map(|o| (i, o))).collect();

    // The dialog is the last block that starts its numbering over at 1.
    let start = marks.iter().rposition(|(_, (index, _, _))| *index == 1)?;

    let mut options = Vec::new();
    let mut selected = None;
    let mut expected = 1;
    for (_, (index, label, is_selected)) in &marks[start..] {
        // A gap in the numbering is the end of this block, not a hole in it.
        if *index != expected {
            break;
        }
        if *is_selected {
            selected = Some(options.len());
        }
        options.push(QuestionOption { label: label.clone(), send: format!("{index}\r") });
        expected += 1;
    }

    // Nothing is waiting on a list nobody is pointing at.
    selected?;

    // Upwards from the options to the question. Some versions of the dialog
    // put a blank line between the two and some put none, so whatever
    // separates them is stepped over first; then the run of text directly
    // above is the question, and it ends where the dialog's own layout ends
    // it — at a blank line, or at the rule under a diff preview. Everything
    // further up is a title, a preview or the agent's output, and none of it
    // belongs in what a person is asked.
    let mut above = lines[..marks[start].0].iter().rev().peekable();
    while above.peek().is_some_and(|line| line.is_empty()) {
        above.next();
    }
    let mut text_lines: Vec<&str> = Vec::new();
    for line in above {
        if line.is_empty() || is_rule(line) {
            break;
        }
        text_lines.push(line);
    }
    text_lines.reverse();

    let text = joined(&text_lines);
    // The paragraph above the options is the question, or this is one of the
    // few dialogs that put theirs elsewhere and named itself in a heading.
    // Anything else is declined.
    let text = if text.ends_with('?') { text } else { headed_question(&lines[..marks[start].0])? };

    Some(Question { text, options, selected })
}

/// The question of a dialog that announced itself with one of `HEADINGS`,
/// searched only in the lines between that heading and the options.
///
/// Paragraphs are walked upwards from the options and the first one carrying a
/// question mark is the question, cut at that mark: the trust dialog's
/// question paragraph runs on past it into an aside and a piece of advice
/// ("(Like your own code…). If not, take a moment to review…"), and what a
/// person is being asked ends at the mark.
fn headed_question(above: &[&str]) -> Option<String> {
    let heading = above.iter().rposition(|line| HEADINGS.iter().any(|h| line.starts_with(h)))?;

    let mut paragraph: Vec<&str> = Vec::new();
    for line in above[heading + 1..].iter().rev() {
        if line.is_empty() || is_rule(line) {
            if let Some(text) = asked(&paragraph) {
                return Some(text);
            }
            paragraph.clear();
        } else {
            paragraph.insert(0, line);
        }
    }
    asked(&paragraph)
}

/// The question a paragraph opens with, if it holds one at all.
fn asked(paragraph: &[&str]) -> Option<String> {
    let text = joined(paragraph);
    let mark = text.find('?')?;
    Some(text[..=mark].to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::library::Skills;
    use crate::agents::{Intent, Launch, Profile, Stage, TaskDraft};
    use std::path::PathBuf;

    fn skills(superpowers_installed: bool) -> Skills {
        Skills {
            smetana: PathBuf::from("/app/resources/smetana"),
            superpowers: PathBuf::from("/app/resources/superpowers"),
            superpowers_installed,
        }
    }

    fn launch(intent: Intent, superpowers_installed: bool) -> Launch {
        Launch {
            profile: &Claude,
            cwd: PathBuf::from("/tmp/project"),
            intent,
            skills: skills(superpowers_installed),
            facts: None,
        }
    }

    fn argv(launch: &Launch) -> Vec<String> {
        Claude
            .command(launch)
            .get_argv()
            .iter()
            .map(|s| s.to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn our_own_skills_are_always_handed_over() {
        let args = argv(&launch(Intent::Bare, false));
        assert_eq!(args[0], "claude");
        assert!(args.windows(2).any(|w| w[0] == "--plugin-dir"
            && w[1] == "/app/resources/smetana"));
    }

    #[test]
    fn the_vendored_copy_goes_only_to_someone_without_their_own() {
        let without = argv(&launch(Intent::Bare, false));
        assert!(without.iter().any(|a| a == "/app/resources/superpowers"));

        let with = argv(&launch(Intent::Bare, true));
        assert!(
            !with.iter().any(|a| a == "/app/resources/superpowers"),
            "two plugins called superpowers must never be loaded at once"
        );
    }

    #[test]
    fn a_bare_session_gets_no_positional_prompt() {
        // claude, two --plugin-dir flags and their two paths: nothing else.
        assert_eq!(argv(&launch(Intent::Bare, false)).len(), 5);
        assert_eq!(argv(&launch(Intent::Bare, true)).len(), 3);
    }

    fn new_task(images: Vec<String>) -> Intent {
        Intent::NewTask {
            brainstorm: Stage::On,
            spec: Stage::On,
            plan: Stage::On,
            draft: TaskDraft {
                text: "Swap the red for green".into(),
                issue_type: Some("bug".into()),
                priority: Some(2),
                images,
            },
        }
    }

    #[test]
    fn a_new_task_rides_as_the_last_argument() {
        let args = argv(&launch(new_task(Vec::new()), false));
        let last = args.last().unwrap();
        assert!(last.contains("Swap the red for green"));
        assert!(last.contains("superpowers:brainstorming"));
    }

    #[test]
    fn an_attached_image_reaches_this_harness_by_path_and_never_by_flag() {
        // Claude Code has no flag for an image and reads one when the prompt
        // names it. A flag invented for it here would be an unknown argument
        // and the session would not start at all.
        let args = argv(&launch(new_task(vec!["/data/a.png".into()]), false));
        assert!(args.last().unwrap().contains("/data/a.png"), "{args:?}");
        assert!(!args.iter().any(|a| a == "-i" || a == "--image"), "{args:?}");
    }

    fn fixture(name: &str) -> Vec<String> {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name);
        std::fs::read_to_string(path).unwrap().lines().map(str::to_owned).collect()
    }

    #[test]
    fn recognises_the_permission_dialog() {
        let q = question(&fixture("claude-permission-dialog.txt")).expect("the dialog went unrecognised");
        assert_eq!(q.text, "Do you want to make this edit to tabs.js?");
        assert_eq!(q.options.len(), 3);
        assert_eq!(q.options[0].label, "Yes");
        assert_eq!(q.options[0].send, "1\r");
        assert_eq!(q.selected, Some(0));
    }

    // The two fixtures below are whole 120x30 screens, captured off a real
    // Claude Code 2.1.224 under a PTY and rendered through the same vt100 the
    // worker reads: banner, the agent's own output, and the dialog at the
    // bottom. That is the shape this reader actually has to survive, and a
    // hand-written excerpt of it would prove only that it survives an excerpt.

    #[test]
    fn recognises_the_unframed_dialog_of_claude_2_1() {
        // No frame anywhere on it any more: the dialog is fenced off by a
        // full width rule and its lines are bare. The box filter this used to
        // open with matched only the welcome banner, which has no options in
        // it, so every permission prompt went unseen (smetana-8hc).
        let q = question(&fixture("claude-2.1-permission-bash.txt")).expect("the dialog went unrecognised");
        assert_eq!(q.text, "Do you want to proceed?");
        assert_eq!(q.options.len(), 3);
        assert_eq!(q.options[0].label, "Yes");
        assert_eq!(q.options[2].label, "No");
        assert_eq!(q.selected, Some(0));
    }

    #[test]
    fn a_diff_preview_is_fenced_off_by_a_rule_rather_than_a_blank_line() {
        // The edit dialog puts its diff directly above the question with a
        // dashed rule between them and no blank line anywhere near, so
        // splitting on blanks alone drags the whole preview into the text.
        let q = question(&fixture("claude-2.1-permission-edit.txt")).expect("the dialog went unrecognised");
        assert_eq!(q.text, "Do you want to make this edit to tabs.js?");
        assert_eq!(q.options.len(), 3);
    }

    #[test]
    fn recognises_the_folder_trust_dialog() {
        // The one-off question Claude Code asks the first time it is started
        // somewhere it has not been — which is a new project's very first
        // agent, and the worst possible moment to stall silently. The
        // paragraph directly above the options here is "Security guide", a
        // link caption, so the ordinary reading declines it (smetana-xh7).
        //
        // The fixture is the dialog as captured under a PTY and rendered
        // through terminal/screen.rs, recorded in the task; unlike the two
        // fixtures above it is the dialog alone, since that is the whole of
        // what was captured — the surrounding screen would have to be
        // invented, and an invented screen proves nothing.
        let q = question(&fixture("claude-2.1-trust-folder.txt")).expect("the dialog went unrecognised");
        assert_eq!(q.text, "Quick safety check: Is this a project you created or one you trust?");
        assert_eq!(q.options.len(), 2);
        assert_eq!(q.options[0].label, "Yes, I trust this folder");
        assert_eq!(q.options[0].send, "1\r");
        assert_eq!(q.options[1].label, "No, exit");
        assert_eq!(q.selected, Some(0));
    }

    #[test]
    fn a_numbered_list_under_that_heading_still_needs_the_cursor() {
        // The heading opens a wider search for the text and nothing else: the
        // cursor rule is what keeps a numbered list in the agent's own prose
        // out, and it is untouched by it.
        let screen: Vec<String> = [
            "Accessing workspace:",
            "",
            "Quick safety check: Is this a project you created or one you trust?",
            "",
            "  1. Yes, I trust this folder",
            "  2. No, exit",
        ]
        .iter()
        .map(|s| (*s).to_owned())
        .collect();
        assert!(question(&screen).is_none(), "a list nobody is pointing at was taken for a dialog");
    }

    #[test]
    fn an_unheaded_dialog_still_has_to_end_in_a_question_mark() {
        // Cursor and numbering both present, and the text above the options is
        // not a question. Without a heading this reader knows, that is prose
        // with a menu in it and the answer stays no.
        let screen: Vec<String> = [
            "⏺ Here is what I would do next",
            " ❯ 1. Rename the module",
            "   2. Split the test file",
        ]
        .iter()
        .map(|s| (*s).to_owned())
        .collect();
        assert!(question(&screen).is_none(), "the question mark rule was dropped for everyone");
    }

    #[test]
    fn the_heading_fences_the_search_off_from_the_output_above_it() {
        // A question mark in what the agent printed is not the dialog's
        // question, however close it sits: the search runs between the heading
        // and the options, never above the heading.
        let screen: Vec<String> = [
            "⏺ Shall I carry on with the refactor?",
            "",
            "Accessing workspace:",
            "",
            "/tmp/project",
            "",
            "Security guide",
            "",
            "❯ 1. Yes, I trust this folder",
            "  2. No, exit",
        ]
        .iter()
        .map(|s| (*s).to_owned())
        .collect();
        assert!(
            question(&screen).is_none(),
            "the agent's own words above the heading were read as the dialog's question"
        );
    }

    #[test]
    fn ordinary_work_is_not_a_dialog() {
        let screen: Vec<String> = ["Reading tabs.js", "  1. checked", "Done"].iter().map(|s| (*s).to_owned()).collect();
        assert!(question(&screen).is_none(), "a numbered list was taken for a question");
    }

    #[test]
    fn a_numbered_list_the_agent_printed_is_not_a_dialog() {
        // Ends in a question mark and is numbered, and before 2.1 the frame
        // was the only thing keeping it out. Nothing points at it, which is
        // what a dialog always has and prose never does.
        let screen: Vec<String> = [
            "⏺ Which of these would you like me to do first?",
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
        // it replaces the input box at the foot of the screen, and whatever
        // the agent printed is by definition above it.
        let screen: Vec<String> = [
            "⏺ I considered three options:",
            "  1. Rename the module",
            "  2. Split the test file",
            "",
            "────────────────────────────────────────",
            " Bash command",
            "",
            " Do you want to proceed?",
            " ❯ 1. Yes",
            "   2. No",
        ]
        .iter()
        .map(|s| (*s).to_owned())
        .collect();
        let q = question(&screen).expect("the dialog was shadowed by the output above it");
        assert_eq!(q.text, "Do you want to proceed?");
        assert_eq!(q.options.len(), 2, "the printed list was swept in with the dialog's own options");
        assert_eq!(q.options[0].label, "Yes");
    }

    #[test]
    fn the_title_above_a_rule_is_not_part_of_the_question() {
        let screen: Vec<String> = [
            "────────────────────────────────────────",
            " Bash command",
            "",
            "   sw_vers -productVersion",
            "   Get macOS product version",
            "",
            " This command requires approval",
            "",
            " Do you want to proceed?",
            " ❯ 1. Yes",
            "   2. No",
        ]
        .iter()
        .map(|s| (*s).to_owned())
        .collect();
        let q = question(&screen).expect("the dialog went unrecognised");
        assert_eq!(q.text, "Do you want to proceed?");
    }

    #[test]
    fn a_half_drawn_frame_is_not_a_dialog() {
        let screen: Vec<String> = ["╭───────────╮", "│ Edit file │"].iter().map(|s| (*s).to_owned()).collect();
        assert!(question(&screen).is_none(), "a frame with no options was taken for a question");
    }

    #[test]
    fn a_wrapped_question_is_joined_into_one_line() {
        // Title plus a blank line above the wrapped question on purpose:
        // proves paragraphs and wrapping work together, not just wrapping
        // in isolation — the title lives in its own, earlier paragraph and
        // must not leak into `text`.
        let screen: Vec<String> = [
            "╭──────────────────────────────────────────────────────╮",
            "│ Edit file                                             │",
            "│                                                       │",
            "│ Do you want to make this edit to                      │",
            "│ some/very/long/path/to/file.js?                       │",
            "│                                                       │",
            "│ ❯ 1. Yes                                              │",
            "│   2. Yes, and don't ask again this session            │",
            "│   3. No, and tell Claude what to do differently       │",
            "╰──────────────────────────────────────────────────────╯",
        ]
        .iter()
        .map(|s| (*s).to_owned())
        .collect();
        let q = question(&screen).expect("the wrapped question went unrecognised");
        assert_eq!(q.text, "Do you want to make this edit to some/very/long/path/to/file.js?");
    }

    #[test]
    fn a_preview_above_the_question_does_not_reach_the_text() {
        // A real permission dialog carries a diff preview between the
        // title and the question, all inside the same frame. Only the
        // last paragraph before the options — the question — must survive.
        let screen: Vec<String> = [
            "╭──────────────────────────────────────────────────────╮",
            "│ Edit file                                             │",
            "│                                                       │",
            "│ + some code                                           │",
            "│ - other code                                          │",
            "│ + more code                                           │",
            "│                                                       │",
            "│ Do you want to make this edit to tabs.js?             │",
            "│                                                       │",
            "│ ❯ 1. Yes                                              │",
            "│   2. Yes, and don't ask again this session            │",
            "│   3. No, and tell Claude what to do differently       │",
            "╰──────────────────────────────────────────────────────╯",
        ]
        .iter()
        .map(|s| (*s).to_owned())
        .collect();
        let q = question(&screen).expect("the dialog went unrecognised");
        assert_eq!(q.text, "Do you want to make this edit to tabs.js?");
    }

    #[test]
    fn an_option_whose_label_holds_a_question_is_not_mistaken_for_the_question() {
        let screen: Vec<String> = [
            "╭──────────────────────────────────────────────────────╮",
            "│ Do you want to make this edit to tabs.js?             │",
            "│                                                       │",
            "│ ❯ 1. Yes                                              │",
            "│   2. Wait, are you sure about this?                   │",
            "│   3. No, and tell Claude what to do differently       │",
            "╰──────────────────────────────────────────────────────╯",
        ]
        .iter()
        .map(|s| (*s).to_owned())
        .collect();
        let q = question(&screen).expect("the dialog went unrecognised");
        assert_eq!(q.text, "Do you want to make this edit to tabs.js?");
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
    fn an_unattended_batch_gets_the_permission_switch_and_a_supervised_one_does_not() {
        use crate::runs::model::RunMode;
        let auto = argv(&launch(run(RunMode::Auto), false));
        assert!(
            auto.windows(2).any(|w| w[0] == "--permission-mode" && w[1] == "bypassPermissions"),
            "nobody is there to answer a prompt: {auto:?}"
        );
        for mode in [RunMode::Supervised, RunMode::Solo] {
            let args = argv(&launch(run(mode), false));
            assert!(
                !args.iter().any(|a| a == "--permission-mode"),
                "{mode:?} has a person, and their prompts are what it is"
            );
        }
    }

    #[test]
    fn the_switch_goes_in_front_of_the_positional_prompt() {
        // A flag after a positional relies on somebody else's parser being
        // relaxed about the order, which is not ours to assume.
        let args = argv(&launch(run(crate::runs::model::RunMode::Auto), false));
        let flag = args.iter().position(|a| a == "--permission-mode").expect("the flag is there");
        assert_eq!(flag, args.len() - 3, "only the flag's value and the prompt come after it");
    }

    #[test]
    fn nothing_a_person_started_is_silently_given_a_bypass() {
        // build_command applies autonomy for a Run and for nothing else; the
        // profile does the same with the arguments. Both halves are checked
        // here because either one alone would let a bare session through.
        for intent in [Intent::Bare, Intent::Setup] {
            let args = argv(&launch(intent, false));
            assert!(!args.iter().any(|a| a == "--permission-mode"), "{args:?}");
        }
    }

    /// Copied out of `claude -p "/usage"` on 2.1.224, whole and unedited. It is
    /// a fixture rather than a hand-written line for the same reason the
    /// `Intent` tests hold hand-copied JSON: a round trip through something we
    /// wrote would only agree with itself, and what breaks here is the other
    /// side changing its wording.
    const USAGE_OUTPUT: &str = "\
You are currently using your subscription to power your Claude Code usage

Current session: 10% used · resets Aug 7 at 8pm (Europe/Moscow)
Current week (all models): 20% used · resets Aug 11 at 5:59pm (Europe/Moscow)
Current week (Fable): 0% used

What's contributing to your limits usage?
Approximate, based on local sessions on this machine — does not include other devices or claude.ai.

Last 24h · 2269 requests · 24 sessions
  60% of your usage came from subagent-heavy sessions
  55% of your usage was at >150k context
";

    #[test]
    fn both_plan_limits_are_read_out_of_what_the_cli_prints() {
        let read = usage(USAGE_OUTPUT).expect("the two lines are there");
        assert_eq!(read.session_pct, 10);
        assert_eq!(read.session_reset.as_deref(), Some("Aug 7 at 8pm (Europe/Moscow)"));
        assert_eq!(read.week_pct, 20);
        assert_eq!(read.week_reset.as_deref(), Some("Aug 11 at 5:59pm (Europe/Moscow)"));
    }

    #[test]
    fn a_per_model_allowance_is_not_one_of_them() {
        // `Current week (Fable): 0% used` sits between the two lines that do
        // count, and reading it as the weekly figure would report 0% while the
        // week is nearly spent. The CLI answers an exhausted model limit by
        // switching models, so it is not a run's business at all.
        let read = usage(USAGE_OUTPUT).expect("the two lines are there");
        assert_eq!(read.week_pct, 20, "the per-model line must not overwrite the weekly one");
    }

    #[test]
    fn percentages_in_the_body_of_the_report_are_not_mistaken_for_limits() {
        // "60% of your usage came from subagent-heavy sessions" is a share of
        // what was spent, not a share of the allowance. Matching it would pause
        // a run at 8% of its actual limit.
        let read = usage(USAGE_OUTPUT).expect("the two lines are there");
        assert!(read.pct() < 60, "read {}%, which is a line from the breakdown", read.pct());
    }

    #[test]
    fn an_allowance_with_nothing_to_say_about_its_reset_is_still_a_reading() {
        // A fresh week prints no reset. Refusing the whole reading over the
        // missing half would leave the run blind to a session at 95%.
        let read = usage("Current session: 95% used\n").expect("a percentage is a reading");
        assert_eq!(read.session_pct, 95);
        assert_eq!(read.session_reset, None);
    }

    #[test]
    fn output_with_neither_line_in_it_is_no_reading_at_all() {
        // Not a zero: `decide` treats `None` as no reason to hold a run up,
        // while a zero would claim a fresh allowance nobody measured.
        assert_eq!(usage("Invalid API key · Please run /login"), None);
        assert_eq!(usage(""), None);
    }
}


