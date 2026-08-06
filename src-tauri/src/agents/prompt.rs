//! An intent becomes the text the agent opens on. Pure: the skill text, when
//! one is needed, is read by the caller and passed in.

use std::fmt::Write;
use std::path::Path;

use super::library::Skills;
use super::{Brainstorm, Intent, SkillDelivery, TaskDraft};
use crate::runs::model::{RunMode, RunScope, RunSettings};

/// The sentence that makes the agent talk the task through. It has to stand on
/// its own: `Inline` may find no skill text to attach, and `Auto` deliberately
/// attaches none.
const DISCUSS: &str =
    "Before creating anything, agree the design with me first — ask one question at a time — \
     and only then file the task, or tasks, the discussion produces.";

/// The test the agent applies in `Auto`. Nothing in the app has read the text
/// of the task, so the judgement is the agent's, and the rule has to be sharp
/// enough to be applied by someone who has just read it once.
const JUDGE: &str =
    "Judge first. If this touches more than one place, or the wording admits more than one \
     reading, discuss it with me before creating anything. If it is a single obvious change, \
     just file it.";

/// The skills a harness cannot look up for itself, already read. Both are
/// `None` for a `PluginDir` harness — it has the plugins loaded and is told
/// the skills by name — and either may be `None` for an `Inline` one when the
/// file could not be read, which is an ordinary outcome, not an error.
pub struct SkillText<'a> {
    /// The app's own filing-a-task skill.
    pub filing: Option<&'a str>,
    /// superpowers' brainstorming skill. Read only when the switch is `On`.
    pub brainstorming: Option<&'a str>,
}

/// What the agent is told to produce when a project has no configuration yet.
/// The file's path is named here rather than left to the skill: a session that
/// could not read the skill must still write to the right place.
const SETUP: &str = "Work out what this project is made of and write .smetana/project.toml — \
     the file Smetana reads before it runs anything here. Check the commands before you write \
     them in, and ask me about anything the folder does not answer.";

/// What the session opens on. `None` means nothing is imposed and the agent
/// starts on an empty prompt.
pub fn build(
    intent: &Intent,
    delivery: SkillDelivery,
    skills: &Skills,
    facts: Option<&str>,
    text: SkillText,
) -> Option<String> {
    let brainstorming = skills.superpowers.join("skills/brainstorming");
    match intent {
        Intent::Bare => None,
        // Deliberately unfinished: the agent is being told what to work on,
        // not what to change, and only the person knows the second half.
        Intent::EditTask { id, title } => Some(format!("Update bd issue {id} (\"{title}\"): ")),
        Intent::NewTask { brainstorm, draft } => {
            Some(new_task(*brainstorm, draft, delivery, &brainstorming, text))
        }
        Intent::Setup => Some(setup(delivery, skills, facts)),
        Intent::Run { settings } => Some(run(settings, delivery, skills)),
    }
}

/// What one batch of a run opens on.
///
/// Everything variable is stated here, and everything fixed is left to the
/// skill: the settings are this run's and are not in any file, while how to
/// carry work through is the same every time and is 300 lines nobody should
/// pay for in a prompt. The rest — repositories, gates, hazards — the skill
/// reads out of `.smetana/project.toml` itself, which is also what keeps a
/// batch reading the config as it is now rather than as it was when the run
/// started.
fn run(settings: &RunSettings, delivery: SkillDelivery, skills: &Skills) -> String {
    let mut out = String::from("Work this project's bd tracker. ");

    match &settings.scope {
        RunScope::Queue => out.push_str("Take ready tasks from the board."),
        RunScope::Task { id } => {
            let _ = write!(out, "Work only on issue {id}, and nothing else.");
        }
        /* "the children of", not "the children of epic": bd's parent-child is
           the relation, and the parent's own type has nothing to do with it —
           a `feature` with children is how this very tracker is written. */
        RunScope::Epic { id } => {
            let _ = write!(out, "Work only on the children of {id}, and nothing else.");
        }
    }

    out.push_str("\n\nThis run:\n");
    let _ = writeln!(
        out,
        "- merge finished work into `{}`{}",
        settings.target_branch,
        if settings.create_target {
            " — it does not exist yet, so cut it from the current branch before the first merge"
        } else {
            ""
        }
    );
    // Only where there is something to choose between, which is the queue and
    // nothing else — `RunSettings::validate` is what makes it `None` elsewhere.
    // Beside "Work only on issue X, and nothing else" a floor is a second
    // instruction contradicting the first.
    if let Some(floor) = settings.min_priority {
        let _ = writeln!(out, "- take nothing worse than priority P{floor} automatically");
    }
    let _ = writeln!(
        out,
        "- {}",
        match settings.mode {
            RunMode::Auto =>
                "you are on your own — there is no one to ask. Park anything you cannot resolve, \
                 note why, and carry on with the rest",
            RunMode::Supervised =>
                "ask me when something genuinely needs a decision, and keep going otherwise",
            RunMode::Solo =>
                "do the work yourself rather than delegating it, and ask me freely",
        }
    );
    let _ = writeln!(
        out,
        "- {}",
        if settings.live_check {
            "verify each merged task for real before closing it"
        } else {
            "close a task on a green merge; there is no live check this run"
        }
    );
    let _ = writeln!(
        out,
        "- {}",
        if settings.file_findings {
            "findings that are out of scope may be filed as `deferred`, within the budget"
        } else {
            "file nothing new: every out-of-scope finding goes to the digest and nowhere else"
        }
    );

    out.push('\n');
    match delivery {
        SkillDelivery::PluginDir => out.push_str(
            "Follow the smetana:running-tasks skill — it is the process, and it names the \
             others it needs.",
        ),
        SkillDelivery::Inline => {
            let _ = write!(
                out,
                "The process is at {} — read it first, and read the skills it names beside it.",
                skills.smetana.join("skills/running-tasks/SKILL.md").display()
            );
        }
    }
    out
}

fn setup(delivery: SkillDelivery, skills: &Skills, facts: Option<&str>) -> String {
    let mut out = String::from(SETUP);
    out.push_str("\n\n");
    match delivery {
        SkillDelivery::PluginDir => {
            out.push_str("Use the smetana:project-setup skill — it says what the file holds.");
        }
        SkillDelivery::Inline => {
            let skill = skills.smetana.join("skills/project-setup/SKILL.md");
            let _ = write!(
                out,
                "What the file holds is described at {} — read it first.",
                skill.display()
            );
        }
    }
    if let Some(facts) = facts {
        out.push_str("\n\n");
        out.push_str(facts.trim_end());
    }
    out
}

/// What the person pinned, and what they left on Auto. Auto is said out loud
/// rather than left to silence: an agent told nothing about the type would
/// have to invent one anyway, but would not know that inventing it was its
/// job rather than a gap in what it was told.
fn fields(draft: &TaskDraft) -> String {
    let mut given: Vec<String> = Vec::new();
    let mut auto: Vec<&str> = Vec::new();
    match &draft.issue_type {
        Some(kind) => given.push(format!("type {kind}")),
        None => auto.push("type"),
    }
    match draft.priority {
        Some(priority) => given.push(format!("priority P{priority}")),
        None => auto.push("priority"),
    }

    let mut out = String::new();
    if !given.is_empty() {
        let _ = write!(out, "File it with {}.", given.join(" and "));
    }
    if !auto.is_empty() {
        if !out.is_empty() {
            out.push(' ');
        }
        let _ = write!(
            out,
            "Decide the {} yourself, from what is written above.",
            auto.join(" and the ")
        );
    }
    out
}

fn new_task(
    brainstorm: Brainstorm,
    draft: &TaskDraft,
    delivery: SkillDelivery,
    brainstorming: &Path,
    text: SkillText,
) -> String {
    let mut out = String::new();
    out.push_str("File a new task in this project's bd tracker. This is what needs doing:\n\n");
    out.push_str(draft.text.trim());
    out.push_str("\n\n");
    out.push_str(&fields(draft));
    out.push_str("\n\n");

    // How to file one properly is not part of the brainstorming question: an
    // agent that files without any discussion still has to file it well. A
    // harness with a registry is told the name; one without gets the text.
    match delivery {
        SkillDelivery::PluginDir => {
            out.push_str("Use the smetana:filing-a-task skill for how this project wants it worded.\n\n");
        }
        SkillDelivery::Inline => {
            if let Some(filing) = text.filing {
                out.push_str("How this project wants a task filed:\n\n");
                out.push_str(filing);
                out.push_str("\n\n");
            }
        }
    }

    match (brainstorm, delivery) {
        (Brainstorm::Off, _) => {
            out.push_str("File it now. No design discussion is wanted for this one.");
        }
        (Brainstorm::On, SkillDelivery::PluginDir) => {
            out.push_str("Use the superpowers:brainstorming skill. ");
            out.push_str(DISCUSS);
        }
        (Brainstorm::On, SkillDelivery::Inline) => {
            out.push_str(DISCUSS);
            if let Some(process) = text.brainstorming {
                out.push_str("\n\nFollow this process:\n\n");
                out.push_str(process);
            }
        }
        (Brainstorm::Auto, SkillDelivery::PluginDir) => {
            out.push_str(JUDGE);
            out.push_str(" If you decide to discuss it, use the superpowers:brainstorming skill.");
        }
        (Brainstorm::Auto, SkillDelivery::Inline) => {
            out.push_str(JUDGE);
            let _ = write!(
                out,
                " If you decide to discuss it, the process is at {} — read it first.",
                brainstorming.join("SKILL.md").display()
            );
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::{Brainstorm, Intent, TaskDraft};
    use std::path::PathBuf;

    fn draft() -> TaskDraft {
        TaskDraft {
            text: "Swap the red for green".into(),
            issue_type: Some("bug".into()),
            priority: Some(2),
        }
    }

    fn new_task(brainstorm: Brainstorm) -> Intent {
        Intent::NewTask { brainstorm, draft: draft() }
    }

    fn skills() -> crate::agents::library::Skills {
        crate::agents::library::Skills {
            smetana: PathBuf::from("/app/resources/smetana"),
            superpowers: PathBuf::from("/app/resources/superpowers"),
            superpowers_installed: false,
        }
    }

    const BRAINSTORMING: &str = "# Brainstorming\n\nAsk one question at a time.";
    const FILING: &str = "# Filing a task\n\nThe title says what needs doing.";

    /// Nothing read: what a PluginDir harness always gets, and what an Inline
    /// harness gets when the files cannot be read.
    fn nothing() -> SkillText<'static> {
        SkillText { filing: None, brainstorming: None }
    }

    fn both() -> SkillText<'static> {
        SkillText { filing: Some(FILING), brainstorming: Some(BRAINSTORMING) }
    }

    /// A floor only where the scope allows one — `RunSettings::validate` is
    /// what refuses the rest, and a fixture that could not be started is not
    /// worth writing a prompt for.
    fn run_settings(mode: RunMode, scope: RunScope) -> RunSettings {
        let min_priority = matches!(scope, RunScope::Queue).then_some(2);
        RunSettings {
            scope,
            mode,
            target_branch: "staging".into(),
            create_target: false,
            min_priority,
            live_check: true,
            file_findings: true,
        }
    }

    fn run_prompt(settings: RunSettings, delivery: SkillDelivery) -> String {
        build(&Intent::Run { settings }, delivery, &skills(), None, nothing()).unwrap()
    }

    #[test]
    fn a_run_names_the_process_skill_in_both_deliveries() {
        let named = run_prompt(run_settings(RunMode::Auto, RunScope::Queue), SkillDelivery::PluginDir);
        assert!(named.contains("smetana:running-tasks"), "{named}");

        let pointed = run_prompt(run_settings(RunMode::Auto, RunScope::Queue), SkillDelivery::Inline);
        assert!(
            pointed.contains("/app/resources/smetana/skills/running-tasks/SKILL.md"),
            "a harness with no registry gets the path: {pointed}"
        );
        assert!(!pointed.contains("smetana:running-tasks"), "it has no registry to name");
    }

    #[test]
    fn every_setting_the_person_chose_reaches_the_prompt() {
        // The config is read by the skill; these are the ones that exist only
        // in this run and are in no file for it to find.
        let text = run_prompt(
            RunSettings {
                target_branch: "release/7".into(),
                min_priority: Some(1),
                ..run_settings(RunMode::Auto, RunScope::Queue)
            },
            SkillDelivery::PluginDir,
        );
        assert!(text.contains("release/7"), "{text}");
        assert!(text.contains("P1"), "{text}");
    }

    #[test]
    fn only_a_queue_is_told_about_a_priority_floor() {
        // "Work only on issue X, and nothing else" beside "take nothing worse
        // than P2" is two instructions that contradict each other, and the
        // work is already named — there is nothing left for a floor to pick.
        let queue = run_prompt(run_settings(RunMode::Auto, RunScope::Queue), SkillDelivery::PluginDir);
        assert!(queue.contains("nothing worse than priority P2"), "{queue}");

        let one = run_prompt(
            run_settings(RunMode::Auto, RunScope::Task { id: "smetana-9".into() }),
            SkillDelivery::PluginDir,
        );
        assert!(!one.contains("nothing worse than priority"), "{one}");

        let epic = run_prompt(
            run_settings(RunMode::Auto, RunScope::Epic { id: "smetana-4".into() }),
            SkillDelivery::PluginDir,
        );
        assert!(!epic.contains("nothing worse than priority"), "{epic}");
    }

    #[test]
    fn a_branch_that_does_not_exist_yet_is_named_as_one_to_cut() {
        // The dialog is the only place that knows the branch list, so the fact
        // travels in the settings. Without this line the first merge is into a
        // branch nothing created, and every task parks on the same error.
        let settings = RunSettings {
            target_branch: "release/8".into(),
            create_target: true,
            ..run_settings(RunMode::Auto, RunScope::Queue)
        };
        let text = run_prompt(settings, SkillDelivery::PluginDir);
        assert!(text.contains("release/8"), "{text}");
        assert!(text.contains("does not exist yet"), "{text}");
        assert!(text.contains("cut it from the current branch"), "{text}");

        // And an existing branch says nothing of the kind: an agent told to
        // create a branch that is already there fails on its first command.
        let plain = run_prompt(run_settings(RunMode::Auto, RunScope::Queue), SkillDelivery::PluginDir);
        assert!(!plain.contains("does not exist yet"), "{plain}");
    }

    #[test]
    fn the_scope_says_what_may_be_touched() {
        let queue = run_prompt(run_settings(RunMode::Auto, RunScope::Queue), SkillDelivery::PluginDir);
        assert!(queue.contains("Take ready tasks"), "{queue}");

        let one = run_prompt(
            run_settings(RunMode::Auto, RunScope::Task { id: "smetana-9".into() }),
            SkillDelivery::PluginDir,
        );
        assert!(one.contains("only on issue smetana-9"), "{one}");
        assert!(one.contains("and nothing else"), "{one}");

        let epic = run_prompt(
            run_settings(RunMode::Auto, RunScope::Epic { id: "smetana-4".into() }),
            SkillDelivery::PluginDir,
        );
        assert!(epic.contains("children of smetana-4"), "{epic}");
    }

    #[test]
    fn each_mode_says_what_to_do_when_something_is_unclear() {
        // The whole difference between the three modes is this one line, and an
        // agent that never reads it would ask a question nobody can answer.
        let auto = run_prompt(run_settings(RunMode::Auto, RunScope::Queue), SkillDelivery::PluginDir);
        assert!(auto.contains("no one to ask"), "{auto}");
        assert!(auto.contains("Park"), "{auto}");

        let supervised =
            run_prompt(run_settings(RunMode::Supervised, RunScope::Queue), SkillDelivery::PluginDir);
        assert!(supervised.contains("ask me"), "{supervised}");
        assert!(!supervised.contains("no one to ask"), "{supervised}");

        let solo = run_prompt(
            run_settings(RunMode::Solo, RunScope::Task { id: "smetana-9".into() }),
            SkillDelivery::PluginDir,
        );
        assert!(solo.contains("yourself rather than delegating"), "{solo}");
    }

    #[test]
    fn the_two_switches_are_stated_in_both_positions() {
        // Silence would be read as the default, and the defaults differ from
        // what a person may have just turned off.
        let on = run_prompt(run_settings(RunMode::Auto, RunScope::Queue), SkillDelivery::PluginDir);
        assert!(on.contains("verify each merged task"), "{on}");
        assert!(on.contains("may be filed as `deferred`"), "{on}");

        let off = run_prompt(
            RunSettings {
                live_check: false,
                file_findings: false,
                ..run_settings(RunMode::Auto, RunScope::Queue)
            },
            SkillDelivery::PluginDir,
        );
        assert!(off.contains("no live check this run"), "{off}");
        assert!(off.contains("file nothing new"), "{off}");
    }

    #[test]
    fn a_bare_session_opens_on_nothing() {
        assert!(build(&Intent::Bare, SkillDelivery::PluginDir, &skills(), None, nothing()).is_none());
    }

    #[test]
    fn editing_an_issue_names_it_and_stops_mid_sentence() {
        let intent = Intent::EditTask { id: "smetana-7".into(), title: "x y".into() };
        let text = build(&intent, SkillDelivery::PluginDir, &skills(), None, nothing()).unwrap();
        assert_eq!(text, "Update bd issue smetana-7 (\"x y\"): ");
    }

    #[test]
    fn editing_an_issue_is_never_given_a_filing_skill() {
        let intent = Intent::EditTask { id: "smetana-7".into(), title: "x y".into() };
        let text = build(&intent, SkillDelivery::Inline, &skills(), None, both()).unwrap();
        assert!(!text.contains("The title says what needs doing"), "nothing is filed here");
    }

    fn drafted(draft: TaskDraft) -> String {
        let intent = Intent::NewTask { brainstorm: Brainstorm::Off, draft };
        build(&intent, SkillDelivery::PluginDir, &skills(), None, nothing()).unwrap()
    }

    #[test]
    fn the_persons_own_words_reach_the_agent_whole() {
        let text = drafted(TaskDraft {
            text: "  The board flashes twice.\n\nIt should flash once.  ".into(),
            ..draft()
        });
        // Whole, including the blank line: what a person typed into a
        // multi-line field is one piece of prose, not a title with an
        // afterthought, and only the trailing whitespace is ours to drop.
        assert!(text.contains("The board flashes twice.\n\nIt should flash once.\n\n"));
        assert!(!text.contains("  The board"), "the leading padding is not the person's text");
    }

    #[test]
    fn a_pinned_type_and_priority_are_stated_as_settled() {
        let text = drafted(draft());
        assert!(text.contains("File it with type bug and priority P2."), "{text}");
        assert!(!text.contains("Decide the"), "nothing is left to the agent here");
    }

    #[test]
    fn auto_hands_the_field_to_the_agent_by_name() {
        // Both on Auto, then one of each: an agent told nothing about a field
        // would still have to choose, and would not know that choosing was
        // its job — so every combination says which fields are its to decide.
        let both = drafted(TaskDraft { issue_type: None, priority: None, ..draft() });
        assert!(both.contains("Decide the type and the priority yourself"), "{both}");
        assert!(!both.contains("File it with"), "nothing was pinned");

        let typed = drafted(TaskDraft { priority: None, ..draft() });
        assert!(typed.contains("File it with type bug."), "{typed}");
        assert!(typed.contains("Decide the priority yourself"), "{typed}");

        let prioritised = drafted(TaskDraft { issue_type: None, ..draft() });
        assert!(prioritised.contains("File it with priority P2."), "{prioritised}");
        assert!(prioritised.contains("Decide the type yourself"), "{prioritised}");
    }

    #[test]
    fn switched_off_it_asks_for_no_discussion() {
        // Checking against the constants themselves, not a retyped substring,
        // is what keeps this test from drifting away from the prose it
        // guards: neither DISCUSS nor JUDGE contains the word "brainstorm",
        // so a leak of either into the Off arm would say nothing about the
        // process and still pass a substring check on that word alone.
        for delivery in [SkillDelivery::PluginDir, SkillDelivery::Inline] {
            let text = build(&new_task(Brainstorm::Off), delivery, &skills(), None, both()).unwrap();
            assert!(!text.contains(DISCUSS), "{delivery:?}: off must not carry the discussion prose");
            assert!(!text.contains(JUDGE), "{delivery:?}: off must not carry the judgement prose");
        }
    }

    #[test]
    fn a_plugin_dir_harness_is_told_the_filing_skill_by_name() {
        // Mirrors an_inline_harness_carries_the_filing_skill_whatever_the_switch_says
        // from the PluginDir side of the same guarantee: filing applies to
        // every NewTask whatever the switch says.
        for mode in [Brainstorm::Off, Brainstorm::Auto, Brainstorm::On] {
            let text = build(&new_task(mode), SkillDelivery::PluginDir, &skills(), None, both()).unwrap();
            assert!(text.contains("smetana:filing-a-task"), "{mode:?}");
            assert!(!text.contains(FILING), "{mode:?}: no registry should carry the skill body");
        }
    }

    #[test]
    fn an_inline_harness_carries_the_filing_skill_whatever_the_switch_says() {
        // The rules for filing a task are not part of the brainstorming
        // question: an agent that files without discussion still has to file
        // it properly.
        for mode in [Brainstorm::Off, Brainstorm::Auto, Brainstorm::On] {
            let text = build(&new_task(mode), SkillDelivery::Inline, &skills(), None, both()).unwrap();
            assert!(text.contains("The title says what needs doing"), "{mode:?}");
            assert!(!text.contains("smetana:filing-a-task"), "{mode:?}: no registry to name");
        }
    }

    #[test]
    fn switched_on_a_plugin_dir_harness_is_told_the_skill_name() {
        let text =
            build(&new_task(Brainstorm::On), SkillDelivery::PluginDir, &skills(), None, nothing()).unwrap();
        assert!(text.contains("superpowers:brainstorming"));
    }

    #[test]
    fn switched_on_an_inline_harness_carries_the_whole_process() {
        let text = build(&new_task(Brainstorm::On), SkillDelivery::Inline, &skills(), None, both()).unwrap();
        assert!(text.contains("Ask one question at a time."));
        assert!(
            !text.contains("superpowers:brainstorming"),
            "an inline harness has no skill registry"
        );
    }

    #[test]
    fn on_inline_degrades_to_the_rule_when_the_skill_cannot_be_read() {
        let text =
            build(&new_task(Brainstorm::On), SkillDelivery::Inline, &skills(), None, nothing()).unwrap();
        assert!(text.contains("agree the design"), "the instruction survives a missing file");
    }

    #[test]
    fn auto_leaves_the_judgement_to_the_agent() {
        let text =
            build(&new_task(Brainstorm::Auto), SkillDelivery::PluginDir, &skills(), None, nothing()).unwrap();
        assert!(text.contains("more than one"), "auto states the test the agent applies");
    }

    #[test]
    fn auto_on_an_inline_harness_points_at_the_file_rather_than_pasting_it() {
        let text = build(&new_task(Brainstorm::Auto), SkillDelivery::Inline, &skills(), None, both()).unwrap();
        assert!(text.contains("/app/resources/superpowers/skills/brainstorming/SKILL.md"));
        assert!(
            !text.contains("Ask one question at a time."),
            "auto must not pay for 10 KB the agent may not use"
        );
    }

    const FACTS: &str = "- backend — npm\n    npm run test\n";

    #[test]
    fn setting_a_project_up_carries_the_survey_and_names_the_file_to_write() {
        let text =
            build(&Intent::Setup, SkillDelivery::PluginDir, &skills(), Some(FACTS), nothing())
                .expect("a setup session opens on something");
        assert!(text.contains(".smetana/project.toml"), "{text}");
        assert!(text.contains("npm run test"), "the survey reaches the agent: {text}");
    }

    #[test]
    fn a_plugin_dir_harness_is_told_the_setup_skill_by_name() {
        let text =
            build(&Intent::Setup, SkillDelivery::PluginDir, &skills(), Some(FACTS), nothing())
                .expect("builds");
        assert!(text.contains("smetana:project-setup"), "{text}");
    }

    #[test]
    fn an_inline_harness_is_given_the_setup_skill_s_path_rather_than_its_body() {
        // The same choice `Auto` already makes for brainstorming: a path costs
        // one line, the body costs kilobytes the session may never need.
        let text = build(
            &Intent::Setup,
            SkillDelivery::Inline,
            &skills(),
            Some(FACTS),
            SkillText { filing: Some(FILING), brainstorming: Some(BRAINSTORMING) },
        )
        .expect("builds");
        assert!(text.contains("/app/resources/smetana/skills/project-setup/SKILL.md"), "{text}");
        assert!(!text.contains("The title says what needs doing"), "nothing is filed here");
    }

    #[test]
    fn a_setup_session_survives_a_survey_that_found_nothing() {
        // `render` always produces text, but a caller that could not run the
        // survey at all passes None, and the instruction still has to stand.
        let text = build(&Intent::Setup, SkillDelivery::PluginDir, &skills(), None, nothing())
            .expect("builds");
        assert!(text.contains(".smetana/project.toml"), "{text}");
    }
}
