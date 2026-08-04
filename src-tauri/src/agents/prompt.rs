//! An intent becomes the text the agent opens on. Pure: the skill text, when
//! one is needed, is read by the caller and passed in.

use std::fmt::Write;
use std::path::Path;

use super::{Brainstorm, Intent, SkillDelivery, TaskDraft};

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

/// What the session opens on. `None` means nothing is imposed and the agent
/// starts on an empty prompt.
pub fn build(
    intent: &Intent,
    delivery: SkillDelivery,
    brainstorming: &Path,
    text: SkillText,
) -> Option<String> {
    match intent {
        Intent::Bare => None,
        // Deliberately unfinished: the agent is being told what to work on,
        // not what to change, and only the person knows the second half.
        Intent::EditTask { id, title } => Some(format!("Update bd issue {id} (\"{title}\"): ")),
        Intent::NewTask { brainstorm, draft } => {
            Some(new_task(*brainstorm, draft, delivery, brainstorming, text))
        }
    }
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

    fn path() -> PathBuf {
        PathBuf::from("/app/resources/superpowers/skills/brainstorming")
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

    #[test]
    fn a_bare_session_opens_on_nothing() {
        assert!(build(&Intent::Bare, SkillDelivery::PluginDir, &path(), nothing()).is_none());
    }

    #[test]
    fn editing_an_issue_names_it_and_stops_mid_sentence() {
        let intent = Intent::EditTask { id: "smetana-7".into(), title: "x y".into() };
        let text = build(&intent, SkillDelivery::PluginDir, &path(), nothing()).unwrap();
        assert_eq!(text, "Update bd issue smetana-7 (\"x y\"): ");
    }

    #[test]
    fn editing_an_issue_is_never_given_a_filing_skill() {
        let intent = Intent::EditTask { id: "smetana-7".into(), title: "x y".into() };
        let text = build(&intent, SkillDelivery::Inline, &path(), both()).unwrap();
        assert!(!text.contains("The title says what needs doing"), "nothing is filed here");
    }

    fn drafted(draft: TaskDraft) -> String {
        let intent = Intent::NewTask { brainstorm: Brainstorm::Off, draft };
        build(&intent, SkillDelivery::PluginDir, &path(), nothing()).unwrap()
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
            let text = build(&new_task(Brainstorm::Off), delivery, &path(), both()).unwrap();
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
            let text = build(&new_task(mode), SkillDelivery::PluginDir, &path(), both()).unwrap();
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
            let text = build(&new_task(mode), SkillDelivery::Inline, &path(), both()).unwrap();
            assert!(text.contains("The title says what needs doing"), "{mode:?}");
            assert!(!text.contains("smetana:filing-a-task"), "{mode:?}: no registry to name");
        }
    }

    #[test]
    fn switched_on_a_plugin_dir_harness_is_told_the_skill_name() {
        let text =
            build(&new_task(Brainstorm::On), SkillDelivery::PluginDir, &path(), nothing()).unwrap();
        assert!(text.contains("superpowers:brainstorming"));
    }

    #[test]
    fn switched_on_an_inline_harness_carries_the_whole_process() {
        let text = build(&new_task(Brainstorm::On), SkillDelivery::Inline, &path(), both()).unwrap();
        assert!(text.contains("Ask one question at a time."));
        assert!(
            !text.contains("superpowers:brainstorming"),
            "an inline harness has no skill registry"
        );
    }

    #[test]
    fn on_inline_degrades_to_the_rule_when_the_skill_cannot_be_read() {
        let text =
            build(&new_task(Brainstorm::On), SkillDelivery::Inline, &path(), nothing()).unwrap();
        assert!(text.contains("agree the design"), "the instruction survives a missing file");
    }

    #[test]
    fn auto_leaves_the_judgement_to_the_agent() {
        let text =
            build(&new_task(Brainstorm::Auto), SkillDelivery::PluginDir, &path(), nothing()).unwrap();
        assert!(text.contains("more than one"), "auto states the test the agent applies");
    }

    #[test]
    fn auto_on_an_inline_harness_points_at_the_file_rather_than_pasting_it() {
        let text = build(&new_task(Brainstorm::Auto), SkillDelivery::Inline, &path(), both()).unwrap();
        assert!(text.contains("/app/resources/superpowers/skills/brainstorming/SKILL.md"));
        assert!(
            !text.contains("Ask one question at a time."),
            "auto must not pay for 10 KB the agent may not use"
        );
    }
}
