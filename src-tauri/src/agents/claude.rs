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

/// Claude Code's permission dialog: a frame, a question line, numbered
/// options. What separates it from any numbered list in ordinary output is
/// the frame around it and a question mark in the text.
fn question(screen: &[String]) -> Option<Question> {
    let framed: Vec<&String> = screen.iter().filter(|l| l.contains('│')).collect();
    if framed.is_empty() {
        return None;
    }

    // The first option line marks where the question ends and the option
    // list begins: everything above it is candidate question text,
    // everything from it on is never text, so an option label that itself
    // ends in '?' cannot be mistaken for the question. This is also the
    // sole "no options, not a dialog" guard — nothing downstream repeats it.
    let first_option = framed.iter().position(|l| option_line(l).is_some())?;

    let mut options = Vec::new();
    let mut selected = None;
    for line in &framed[first_option..] {
        if let Some((index, label, is_selected)) = option_line(line) {
            if is_selected {
                selected = Some(options.len());
            }
            options.push(QuestionOption { label, send: format!("{index}\r") });
        }
    }

    // The title, an optional diff preview and the question all live above
    // the options, in the same frame, separated from each other by blank
    // lines — that is how the dialog itself lays them out. Split what is
    // above the first option into paragraphs on those blanks and take the
    // last one: that is the question and nothing else, however much other
    // text the frame carries above it. A question wrapped across more than
    // one row still lives in a single paragraph and still reassembles.
    let mut paragraphs: Vec<Vec<&str>> = vec![Vec::new()];
    for line in &framed[..first_option] {
        let stripped = line.trim_matches(['│', ' ']);
        if stripped.is_empty() {
            if !paragraphs.last().unwrap().is_empty() {
                paragraphs.push(Vec::new());
            }
        } else {
            paragraphs.last_mut().unwrap().push(stripped);
        }
    }
    let question_paragraph = paragraphs.into_iter().filter(|p| !p.is_empty()).last()?;

    // split_whitespace both drops the frame's own padding and collapses any
    // run of whitespace to one space.
    let text = question_paragraph.iter().flat_map(|l| l.split_whitespace()).collect::<Vec<_>>().join(" ");
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

    #[test]
    fn ordinary_work_is_not_a_dialog() {
        let screen: Vec<String> = ["Reading tabs.js", "  1. checked", "Done"].iter().map(|s| (*s).to_owned()).collect();
        assert!(question(&screen).is_none(), "a numbered list was taken for a question");
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
