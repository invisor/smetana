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
        let text = prompt::SkillText { filing: None, brainstorming: None };
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
/// to end in a question mark.
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

    // split_whitespace both drops the dialog's own padding and collapses any
    // run of whitespace to one space, so a question wrapped across rows comes
    // back as one line.
    let text = text_lines.iter().flat_map(|l| l.split_whitespace()).collect::<Vec<_>>().join(" ");
    if !text.ends_with('?') {
        return None;
    }

    Some(Question { text, options, selected })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::library::Skills;
    use crate::agents::{Brainstorm, Intent, Launch, Profile, TaskDraft};
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
            brainstorm: Brainstorm::On,
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
}


