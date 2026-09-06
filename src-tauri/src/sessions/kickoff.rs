//! The first message of a transcript, read back: whose words are in it, and
//! which of them.
//!
//! Smetana starts an agent by handing it a prompt of its own as the session's
//! positional argument, and both harnesses **submit** that argument rather than
//! leaving it in the composer — so Claude Code writes it into the transcript as
//! an ordinary `user` record stamped `origin.kind: "human"`. Nothing about the
//! record tells it apart from a sentence somebody typed, which is why the
//! Sessions tab drew the same paragraph of ours under `First prompt` on nearly
//! every row (smetana-w4i6): what it was showing was the opening of our own
//! prompt, and the person's own words were buried several hundred lines into
//! it.
//!
//! So the words are cut back out here. There are exactly three answers, and the
//! middle one is the only one that does any cutting:
//!
//! - the message is **not ours** — a session somebody started themselves, with
//!   `claude` in a terminal — and travels on untouched;
//! - it is ours and carries a person's words, which is `Intent::NewTask` and
//!   nothing else: what they wrote in the new-task dialog comes back;
//! - it is ours and carries nobody's words — a run's batch, a setup session, a
//!   conflict, a repair, a branch review, "+ New agent" — and the answer is
//!   nothing at all, which the card draws as its own sentence rather than as an
//!   empty frame.
//!
//! **Pure, and a module of its own for that reason.** `read.rs` is the disk and
//! nothing here touches one, the split `model.rs` already keeps; a branch
//! inside `summarise` would put this rule where no test in this repository can
//! reach it, and the one test that matters — building a real prompt through
//! `agents::prompt::build` and requiring exactly the draft's text back out — is
//! the only thing holding the two sides of the app together mechanically.
//!
//! **Every phrase this matches on is `agents::prompt`'s own constant**, read
//! from there and never copied. A second copy is the whole hazard: the prompt
//! is prose somebody will reword, nothing fails when a copy stops matching, and
//! the visible cost is this defect coming back exactly as it was.

use crate::agents::prompt::{
    CONVERSATION_TAIL, FIELDS_AUTO, FIELDS_AUTO_TAIL, FIELDS_GIVEN, FOLLOW_UP, IMAGES_MANY,
    IMAGES_ONE, NEW_TASK_OPENING, STANDARD,
};

/// What the first human record of a transcript turns out to be.
///
/// Borrowed rather than owned: both text answers are slices of the message that
/// was handed in, and the caller clips them ([`crate::sessions::model::CLIP`])
/// on its way to the front end, exactly as it did when the whole message went
/// there.
#[derive(Debug, PartialEq)]
pub enum Kickoff<'a> {
    /// Not one of ours. Whatever it is, somebody typed it, and it is handed
    /// back whole — a session started from a terminal is not this module's
    /// business and must read exactly as it always has.
    Typed(&'a str),
    /// Ours, and these are the words the person wrote into the new-task dialog,
    /// with the prompt built around them taken away.
    Words(&'a str),
    /// Ours, and nothing in it was typed by anybody.
    Ours,
}

/// Read one first message.
///
/// The message is the *whole* record, uncut: the person's words sit past the
/// language paragraphs and the standing instruction, several hundred characters
/// in, so anything already clipped to `CLIP` is nothing but our own opening and
/// there would be nothing left in it to find.
pub fn of(message: &str) -> Kickoff<'_> {
    if !is_ours(message) {
        return Kickoff::Typed(message);
    }
    // Ours, and only one intent puts a person's words in one. Everything else
    // — a run's batch, a setup, a conflict, a bare session — has no opening
    // line to look for and stops here.
    let Some(at) = message.find(NEW_TASK_OPENING) else {
        return Kickoff::Ours;
    };
    let rest = &message[at + NEW_TASK_OPENING.len()..];
    let words = rest[..end_of_words(rest)].trim();
    if words.is_empty() {
        return Kickoff::Ours;
    }
    Kickoff::Words(words)
}

/// Whether this message is a prompt this app composed.
///
/// The mark is the invariant tail of `prompt::conversation`, the paragraph
/// every intent but a resumed session opens on — and the tail rather than the
/// whole sentence because the language name is substituted into the head of it,
/// so a person working in Russian and a person working in English have to be
/// recognised by the same string.
///
/// It has to be in the **first** paragraph, which is where that sentence always
/// is. Searching the whole message would read somebody quoting our own prompt
/// back at an agent as a prompt of ours, and then cut their message up.
fn is_ours(message: &str) -> bool {
    let text = message.trim_start();
    text.split("\n\n").next().is_some_and(|opening| opening.contains(CONVERSATION_TAIL))
}

/// Where the person's words stop: whichever of the blocks `prompt::new_task`
/// writes after them comes first.
///
/// All of them are looked for and the earliest wins, rather than one being
/// relied on: the images block is there only when something was attached, and
/// which half of the fields block is printed depends on what was left on Auto.
///
/// Nothing matching at all means `prompt.rs` has moved and this has not. The
/// answer then is the whole of the rest, which is this defect back as it was —
/// a first prompt showing our own prose — and deliberately not `Ours`, which
/// would throw away words somebody typed on the strength of a phrase this file
/// failed to recognise. The test that builds a real prompt is what is supposed
/// to fail first.
fn end_of_words(rest: &str) -> usize {
    // Everything in front of the placeholder: the only invariant part of the
    // follow-up block, and derived from the constant rather than written out
    // again beside it.
    let follow_up = FOLLOW_UP.split("{id}").next().unwrap_or(FOLLOW_UP);
    // The second half is required only where the first is too short to stand
    // for a block on its own — see [`paragraph_start`], and `FIELDS_AUTO_TAIL`
    // in `prompt.rs`, for what three words would otherwise cost somebody.
    [
        (IMAGES_ONE, None),
        (IMAGES_MANY, None),
        (FIELDS_GIVEN, None),
        (FIELDS_AUTO, Some(FIELDS_AUTO_TAIL)),
        (follow_up, None),
        (STANDARD, None),
    ]
    .into_iter()
    .filter_map(|(opening, tail)| paragraph_start(rest, opening, tail))
    .min()
    .unwrap_or(rest.len())
}

/// Where `opening` begins a paragraph of `text` — and, where a `tail` is given,
/// where that paragraph also carries it.
///
/// A paragraph and not merely an occurrence, because what is being cut out is
/// somebody's own prose and these phrases are ordinary English. Every block
/// `new_task` writes is preceded by a blank line, so requiring one costs
/// nothing and takes a matching sentence in the middle of a paragraph out of
/// the running.
fn paragraph_start(text: &str, opening: &str, tail: Option<&str>) -> Option<usize> {
    let mut from = 0usize;
    while let Some(found) = text[from..].find(opening) {
        let at = from + found;
        let begins = at == 0 || text[..at].ends_with("\n\n");
        if begins && tail.is_none_or(|tail| paragraph_at(text, at).contains(tail)) {
            return Some(at);
        }
        from = at + opening.len();
    }
    None
}

/// The paragraph of `text` starting at `at`: up to the next blank line, or to
/// the end.
fn paragraph_at(text: &str, at: usize) -> &str {
    let rest = &text[at..];
    match rest.find("\n\n") {
        Some(end) => &rest[..end],
        None => rest,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::library::Skills;
    use crate::agents::prompt::{self, SkillText};
    use crate::agents::{ImageDelivery, Intent, Languages, SkillDelivery, Stage, TaskDraft};
    use crate::runs::model::{RunMode, RunScope, RunSettings};
    use std::path::PathBuf;

    /// What the person wrote, and it is deliberately more than one line and
    /// deliberately opens a paragraph with a word one of the blocks opens with:
    /// prose is what this field holds, and the cut has to survive it.
    const TYPED: &str = "The scope bar counts dirty files it cannot see.\n\n\
                         Decide the right count with me before changing anything.";

    fn skills() -> Skills {
        Skills {
            smetana: PathBuf::from("/app/resources/smetana"),
            superpowers: PathBuf::from("/app/resources/superpowers"),
            superpowers_installed: false,
        }
    }

    /// Nothing read from disk, which is what a `PluginDir` harness always gets
    /// and an `Inline` one gets when the files cannot be read. What a skill
    /// body would add is text after the words, and the cut is in front of it.
    fn nothing() -> SkillText<'static> {
        SkillText {
            filing: None,
            resolving: None,
            brainstorming: None,
            plans: None,
            reviewing_branch: None,
        }
    }

    fn languages(name: &str) -> Languages {
        Languages {
            agent: name.into(),
            task: name.into(),
            commit: name.into(),
            report: name.into(),
        }
    }

    fn built(
        intent: &Intent,
        delivery: SkillDelivery,
        images: ImageDelivery,
        languages: &Languages,
        agent_prompt: &str,
    ) -> String {
        prompt::build(
            intent,
            delivery,
            images,
            &skills(),
            None,
            nothing(),
            languages,
            agent_prompt,
        )
        .expect("every intent here opens on a prompt")
    }

    fn draft(images: &[&str], issue_type: Option<&str>, priority: Option<u8>) -> TaskDraft {
        TaskDraft {
            text: TYPED.into(),
            issue_type: issue_type.map(str::to_owned),
            priority,
            images: images.iter().map(|path| (*path).to_owned()).collect(),
            parent: None,
        }
    }

    /// The one test the whole design rests on: a real prompt, built by the code
    /// that builds the ones agents actually get, handed to the extractor, and
    /// required to give back exactly what the person typed. Every combination
    /// that changes what is written around those words is walked, because the
    /// cut is decided by whichever block comes first and that differs by
    /// combination.
    #[test]
    fn a_real_new_task_prompt_gives_back_exactly_what_the_person_typed() {
        let picture = "/data/attachments/task/shot.png";
        for images in [Vec::new(), vec![picture], vec![picture, picture]] {
            for (issue_type, priority) in
                [(Some("bug"), Some(2)), (None, None), (Some("chore"), None), (None, Some(1))]
            {
                for parent in [None, Some("smetana-a1d")] {
                    for standing in ["", "Answer briefly, and never guess at a path."] {
                        for stages in
                            [(Stage::Off, Stage::Off, Stage::Off), (Stage::On, Stage::On, Stage::On)]
                        {
                            let mut draft = draft(&images, issue_type, priority);
                            draft.parent = parent.map(str::to_owned);
                            let (brainstorm, spec, plan) = stages;
                            let intent = Intent::NewTask { brainstorm, spec, plan, draft };
                            for delivery in [SkillDelivery::PluginDir, SkillDelivery::Inline] {
                                for how in [ImageDelivery::InPrompt, ImageDelivery::Flag("-i")] {
                                    let text =
                                        built(&intent, delivery, how, &languages("en"), standing);
                                    assert_eq!(
                                        of(&text),
                                        Kickoff::Words(TYPED),
                                        "images {}, type {issue_type:?}, priority {priority:?}, \
                                         parent {parent:?}, standing {}, {delivery:?}, {how:?}",
                                        images.len(),
                                        standing.len()
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    /// The recognition must not depend on the `agentLanguage` setting, which is
    /// the whole reason the mark is the tail of that sentence rather than the
    /// sentence. Two languages, and the twelfth would do as well.
    #[test]
    fn the_language_the_person_works_in_changes_nothing_about_the_reading() {
        let intent = Intent::NewTask {
            brainstorm: Stage::Auto,
            spec: Stage::Auto,
            plan: Stage::Auto,
            draft: draft(&[], Some("bug"), Some(2)),
        };
        for name in ["en", "ru", "zh-Hans"] {
            let text = built(
                &intent,
                SkillDelivery::PluginDir,
                ImageDelivery::InPrompt,
                &languages(name),
                "",
            );
            assert_eq!(of(&text), Kickoff::Words(TYPED), "{name}");
        }
    }

    /// Every other intent is a prompt of ours with nobody's words in it, and
    /// the card says so rather than showing our own prose. A run's batch is the
    /// commonest of them on a real machine.
    #[test]
    fn a_prompt_nobody_typed_a_word_into_carries_no_first_prompt() {
        let run = Intent::Run {
            settings: RunSettings {
                scope: RunScope::Queue,
                mode: RunMode::Auto,
                target_branch: "staging".into(),
                create_target: false,
                min_priority: Some(2),
                max_parallel_tasks: Some(3),
                live_check: true,
                file_findings: true,
            },
            reports: PathBuf::from("/p/.smetana/runs/7"),
            batch: 2,
            remove_worktrees: true,
        };
        let conflict = Intent::ResolveConflict {
            repo: "/p".into(),
            op: crate::vcs::model::OpKind::Merge,
            ours: "main".into(),
            theirs: "fix/smetana-w4i6".into(),
            files: vec!["src-tauri/src/sessions/read.rs".into()],
        };
        for intent in [run, Intent::Setup, conflict, Intent::Bare] {
            for delivery in [SkillDelivery::PluginDir, SkillDelivery::Inline] {
                let text =
                    built(&intent, delivery, ImageDelivery::InPrompt, &languages("ru"), "");
                assert_eq!(of(&text), Kickoff::Ours, "{intent:?} {delivery:?}");
            }
        }
    }

    /// A session somebody started themselves, which this must not touch at all.
    /// The message even opens the way ours does, because the two are told apart
    /// by the whole tail of that sentence and not by its first words.
    #[test]
    fn a_message_that_is_not_our_prompt_comes_back_exactly_as_it_arrived() {
        for message in [
            "Move the card to done",
            "Talk to me in Russian. Then read src-tauri/src/sessions/read.rs.",
            "File a new task in this project's bd tracker. This is what needs doing:\n\nthe board",
            "",
        ] {
            assert_eq!(of(message), Kickoff::Typed(message));
        }
    }

    /// Somebody quoting our own opening at an agent, in the middle of their own
    /// message. The mark is looked for in the first paragraph only, so this is
    /// their message and not ours.
    #[test]
    fn our_own_paragraph_quoted_inside_somebody_s_message_is_still_their_message() {
        let message = format!(
            "Why does every session in the tab say this?\n\nTalk to me in Russian: \
             {CONVERSATION_TAIL}"
        );
        assert_eq!(of(&message), Kickoff::Typed(message.as_str()));
    }

    /// A dialog submitted with nothing in the box. `TaskDraft.text` is trimmed
    /// into the prompt, so what is left between the opening and the next block
    /// is whitespace — and an empty first prompt is nothing typed rather than
    /// an empty string handed to the card.
    #[test]
    fn a_draft_with_no_words_in_it_is_nothing_typed_rather_than_an_empty_line() {
        let intent = Intent::NewTask {
            brainstorm: Stage::Off,
            spec: Stage::Off,
            plan: Stage::Off,
            draft: TaskDraft {
                text: "   \n\n  ".into(),
                issue_type: None,
                priority: None,
                images: Vec::new(),
                parent: None,
            },
        };
        let text = built(
            &intent,
            SkillDelivery::PluginDir,
            ImageDelivery::InPrompt,
            &languages("en"),
            "",
        );
        assert_eq!(of(&text), Kickoff::Ours);
    }
}
