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
        // First of all, and before the plugins: this is what makes the batch
        // end by itself. See `agents::is_batch` for which sessions get it.
        if crate::agents::is_batch(&launch.intent) {
            for arg in self.batch_args() {
                cmd.arg(arg);
            }
        }
        // Then the resume, if this is one. In front of the plugins and of
        // everything else for the reason `batch_args` is in front of them: a
        // harness may answer this with a subcommand rather than a flag, and a
        // subcommand has one legal position. Claude Code's `--resume <id>`
        // resolves the id against the working directory, which
        // `terminal::service` has already checked is the one the transcript
        // recorded.
        //
        // Which of the two capabilities the profile is asked for is the whole
        // of the difference between the Sessions tab's two launching verbs, and
        // it is asked as one question either way: neither branch composes a
        // command line out of the other's answer plus a flag.
        if let Intent::ResumeSession { id, fork, .. } = &launch.intent {
            let resume = if *fork { self.fork_args(id) } else { self.resume_args(id) };
            for arg in resume.into_iter().flatten() {
                cmd.arg(arg);
            }
        }
        cmd.arg("--plugin-dir");
        cmd.arg(&launch.skills.smetana);
        if !launch.skills.superpowers_installed {
            cmd.arg("--plugin-dir");
            cmd.arg(&launch.skills.superpowers);
        }
        // Before the prompt, which is positional: a flag after it would rely
        // on the CLI's parser being relaxed about the order, and this app does
        // not get to assume that about somebody else's argument grammar.
        if let Intent::Run { settings, .. } = &launch.intent {
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
        let text = prompt::SkillText {
            filing: None,
            resolving: None,
            brainstorming: None,
            plans: None,
            reviewing_branch: None,
        };
        if let Some(built) = prompt::build(
            &launch.intent,
            self.delivery(),
            self.images(),
            &launch.skills,
            launch.facts.as_deref(),
            text,
            &launch.languages,
            &launch.caveman_level,
            &launch.agent_prompt,
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

    /// `-p` is Claude Code's own "print response and exit", and the shape the
    /// loop this subsystem was ported from has always used: `runClaude` in
    /// `holiday-curb`'s `scripts/lead-auto-loop.mjs` spawns exactly these four
    /// arguments per batch. The `CLAUDE_CODE_PRINT_BG_WAIT_CEILING_MS=0` beside
    /// `autonomy` came from that same script and is a **print-mode** variable —
    /// it has done nothing at all while this profile started an interactive
    /// session, and it starts working here.
    ///
    /// The stream format is not decoration. Measured against the installed CLI,
    /// `-p --verbose` prints a single line when the batch ends, so a pane would
    /// sit empty for the length of one; `--output-format stream-json` emits an
    /// event as each thing happens. What that JSONL is turned into is
    /// `transcript` below.
    fn batch_args(&self) -> &'static [&'static str] {
        &["-p", "--verbose", "--output-format", "stream-json"]
    }

    /// Claude Code's only streaming form is JSONL, so what a person reads has
    /// to be made from it here. See `transcript_line`.
    fn transcript(&self) -> Option<fn(&str) -> Vec<String>> {
        Some(transcript_line)
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

    /// The same `-p`, and nothing beside it: what a one-shot question wants is
    /// the answer on stdout, which is what print mode without a stream format
    /// already prints.
    fn oneshot_args(&self) -> Option<&'static [&'static str]> {
        Some(&["-p"])
    }

    /// `--resume <id>`, which is Claude Code's own way of opening a recorded
    /// conversation again. The id is the transcript file's stem — see
    /// `sessions::model::SessionSummary::id`, which is where it comes from —
    /// and it is resolved against the working directory, so the two travel
    /// together or neither is any use.
    ///
    /// **This is Claude Code's grammar and nobody else's.** `codex.rs` keeps
    /// the default `None` rather than a guess, for the reason `command` above
    /// records about argument order: this app does not get to assume anything
    /// about somebody else's command line, and a wrong flag here would start a
    /// fresh agent in a worktree under a card promising a conversation.
    fn resume_args(&self, session: &str) -> Option<Vec<String>> {
        Some(vec!["--resume".to_owned(), session.to_owned()])
    }

    /// The same `--resume <id>` with `--fork-session` behind it, which is
    /// Claude Code's own way of opening a recorded conversation into a **new**
    /// session: its help describes that flag as starting a new session id when
    /// resuming, so nothing here is guessed about somebody else's grammar. The
    /// original transcript is left as it was and a second one appears beside
    /// it, which is why the Sessions tab grows a card after this and not after
    /// a plain resume.
    ///
    /// Written out whole rather than as `resume_args` plus a flag: the two are
    /// separate answers to separate questions — see `Profile::fork_args` — and
    /// a caller that appended would be composing a command line out of halves.
    fn fork_args(&self, session: &str) -> Option<Vec<String>> {
        Some(vec!["--resume".to_owned(), session.to_owned(), "--fork-session".to_owned()])
    }

    fn parse_usage(&self, output: &str) -> Option<Usage> {
        usage(output)
    }
}

/// The agent's own words, and a tool's detail: the reference formatter's two
/// ceilings, kept because they were chosen against real output. A `Task` call
/// carries a whole briefing in its input, and unclipped it fills the pane.
const MAX_TEXT: usize = 200;
const MAX_DETAIL: usize = 140;

/// Whitespace collapsed and the whole thing on one line — a pane row is a row —
/// and every other control character dropped.
///
/// The second half is not tidiness. Everything that reaches this function is
/// **agent-authored**: an assistant paragraph, a `Bash` command quoting terminal
/// output, an error message from the API. Such a string routinely carries
/// `\u001b[31m` and `\u0007` in the JSON, and `serde_json` decodes those into
/// live bytes — which, before this translator existed, sat inert inside a JSON
/// line nobody rendered. Passed through, they would be colour and cursor
/// movement in a transcript specified as plain text, and a bell would set
/// `bell_pending`, turning the session's row `needs-you` and spending one of the
/// one or two loud rows the whole design budgets for a screen. `char::is_control`
/// covers C0, DEL and C1, and C1 is worth taking with them: U+009B is a CSI in
/// its own right.
fn one_line(text: &str) -> String {
    text.split_whitespace()
        .map(|word| word.chars().filter(|c| !c.is_control()).collect::<String>())
        // A word that was nothing but control bytes leaves no gap of its own.
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn clip(text: &str, max: usize) -> String {
    // Counted in characters rather than bytes: a Russian task title is two
    // bytes a letter, and slicing a string mid-character panics.
    if text.chars().count() <= max {
        return text.to_string();
    }
    let kept: String = text.chars().take(max.saturating_sub(3)).collect();
    format!("{kept}...")
}

/// Which field of a tool's input says what that call is actually doing. The
/// table is the reference formatter's; a tool it has never heard of still gets
/// its name shown, because the point of the line is that something happened.
fn tool_detail(name: &str, input: &serde_json::Value) -> String {
    let field = |key: &str| input.get(key).and_then(serde_json::Value::as_str).unwrap_or("");
    let detail = match name {
        "Bash" => field("command").to_string(),
        "Task" => {
            let kind = field("subagent_type");
            let what = if field("description").is_empty() { field("prompt") } else { field("description") };
            if kind.is_empty() { what.to_string() } else { format!("{kind}: {what}") }
        }
        "Read" | "Edit" | "Write" | "NotebookEdit" => field("file_path").to_string(),
        "Grep" | "Glob" => field("pattern").to_string(),
        "Skill" => {
            let skill = field("skill");
            if skill.is_empty() { field("command").to_string() } else { skill.to_string() }
        }
        "TaskCreate" | "TaskUpdate" => {
            let what = field("description");
            if what.is_empty() { field("status").to_string() } else { what.to_string() }
        }
        _ => String::new(),
    };
    clip(&one_line(&detail), MAX_DETAIL)
}

/// One line of `--output-format stream-json`, as the pane should show it.
///
/// Zero lines is an ordinary answer and the commonest one: hook chatter, a
/// rate-limit event, a tool's result — routinely a whole file — and every event
/// type this build has never heard of. Failing that way round is deliberate: a
/// missing row in a pane costs a person nothing they cannot get from the CLI's
/// own logs, while a wall of JSON costs them the pane.
///
/// A port of `lib/stream-format.mjs` in the project `runs/queue.rs` was ported
/// from, with its `⚙ ✓ ⚠` replaced by ASCII — this app composes the text, and
/// the constraint against emoji does not stop at the edge of a terminal pane.
pub fn transcript_line(line: &str) -> Vec<String> {
    let Ok(event) = serde_json::from_str::<serde_json::Value>(line) else {
        return Vec::new();
    };
    let str_at = |key: &str| event.get(key).and_then(serde_json::Value::as_str).unwrap_or("");
    match str_at("type") {
        "system" => match str_at("subtype") {
            "init" => {
                // Collapsed first and judged empty afterwards: a model field of
                // pure whitespace would otherwise render as `(model )`.
                let model = clip(&one_line(str_at("model")), MAX_DETAIL);
                Some(if model.is_empty() {
                    "-- session start".to_string()
                } else {
                    format!("-- session start (model {model})")
                })
            }
            "api_retry" => {
                // Somebody else's error message, and the same treatment the
                // agent's own prose gets: a newline in it is the diagonal
                // stepping the `\r\n` rule exists against, and an unbounded one
                // is a wall.
                let error = clip(&one_line(str_at("error")), MAX_DETAIL);
                let error = if error.is_empty() { "error".to_string() } else { error };
                let attempt = event
                    .get("attempt")
                    .and_then(serde_json::Value::as_i64)
                    .map_or("?".to_string(), |n| n.to_string());
                Some(format!("!! api retry ({error}), attempt {attempt}"))
            }
            _ => None,
        }
        .into_iter()
        .collect(),
        "assistant" => event
            .pointer("/message/content")
            .and_then(serde_json::Value::as_array)
            .map(|blocks| {
                blocks
                    .iter()
                    .filter_map(|block| {
                        match block.get("type").and_then(serde_json::Value::as_str)? {
                            "text" => {
                                let text = one_line(
                                    block.get("text").and_then(serde_json::Value::as_str)?,
                                );
                                (!text.is_empty()).then(|| format!("   {}", clip(&text, MAX_TEXT)))
                            }
                            "tool_use" => {
                                let name = block.get("name").and_then(serde_json::Value::as_str)?;
                                let detail = tool_detail(
                                    name,
                                    block.get("input").unwrap_or(&serde_json::Value::Null),
                                );
                                Some(if detail.is_empty() {
                                    format!("* {name}")
                                } else {
                                    format!("* {name} — {detail}")
                                })
                            }
                            _ => None,
                        }
                    })
                    .collect()
            })
            .unwrap_or_default(),
        "result" => {
            let tokens = |key: &str| {
                event
                    .pointer(&format!("/usage/{key}"))
                    .and_then(serde_json::Value::as_i64)
                    .map_or("?".to_string(), |n| n.to_string())
            };
            vec![format!(
                "-- batch result (in {} / out {} tok)",
                tokens("input_tokens"),
                tokens("output_tokens")
            )]
        }
        _ => Vec::new(),
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
    for line in output.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix(SESSION) {
            if let Some((pct, resets)) = used(rest) {
                usage.session_pct = Some(pct);
                usage.session_reset = resets;
            }
        } else if let Some(rest) = line.strip_prefix(WEEK) {
            if let Some((pct, resets)) = used(rest) {
                usage.week_pct = Some(pct);
                usage.week_reset = resets;
            }
        }
    }
    // One of the two is enough, and the half that was not read stays absent
    // rather than reading zero. The source's own behaviour was the zero, and it
    // is harmless in a run — `Usage::pct` takes the larger of the halves, so an
    // invented zero never pauses anything — but the same reading is drawn on
    // the settings window, where "This week: 0% used" under a real setting is
    // the app claiming a quota it never read (smetana-7rp).
    (usage.session_pct.is_some() || usage.week_pct.is_some()).then_some(usage)
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
            languages: crate::agents::Languages::default(),
            caveman_level: String::new(),
            agent_prompt: String::new(),
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
    fn a_bare_session_gets_the_language_sentence_and_nothing_more() {
        // claude, two --plugin-dir flags and their two paths, and one
        // positional prompt. That prompt is the whole of what a bare session is
        // told — the conversation language — and it exists because an English
        // default with no Auto position has to reach this session too; what it
        // says is `prompt.rs`'s business and is pinned there.
        let args = argv(&launch(Intent::Bare, false));
        assert_eq!(args.len(), 6);
        assert!(args.last().unwrap().contains("Talk to me in English"), "{args:?}");
        assert_eq!(argv(&launch(Intent::Bare, true)).len(), 4);
    }

    fn resume(id: &str) -> Intent {
        resuming(id, false)
    }

    fn resuming(id: &str, fork: bool) -> Intent {
        Intent::ResumeSession {
            id: id.to_owned(),
            cwd: "/tmp/project/.worktrees/smetana-0cj".into(),
            title: Some("Move the card to done".into()),
            fork,
        }
    }

    #[test]
    fn a_resumed_session_carries_the_id_behind_resume_and_no_prompt_at_all() {
        // The whole of what this feature adds to a command line, and the two
        // halves are one assertion apiece. `--resume <id>` is Claude Code's own
        // grammar and the id is the transcript's stem, immediately after it and
        // not somewhere else on the line.
        //
        // And nothing positional: a prompt is submitted as the session's first
        // message, so one here would be this app talking into a conversation
        // that already has somebody's words in it. `prompt::build` refuses it,
        // and this is the check standing where the argument would appear.
        let args = argv(&launch(resume("9f1c0a2e-6d4b-4f77-8f1a-0c2b3d4e5f60"), true));
        assert_eq!(
            args,
            vec![
                "claude",
                "--resume",
                "9f1c0a2e-6d4b-4f77-8f1a-0c2b3d4e5f60",
                "--plugin-dir",
                "/app/resources/smetana",
            ],
            "a resumed session is the ordinary command line plus --resume <id>"
        );
    }

    #[test]
    fn the_resume_flag_goes_in_front_of_the_plugins() {
        // In front of everything else for the reason `batch_args` is: a harness
        // may answer this with a subcommand rather than a flag, and a
        // subcommand has one legal position. The plugins still travel, so a
        // resumed session can reach the skill library exactly as any other can.
        let args = argv(&launch(resume("abc"), false));
        let resume_at = args.iter().position(|a| a == "--resume").expect("the flag is on the line");
        let first_plugin =
            args.iter().position(|a| a == "--plugin-dir").expect("the plugins still travel");
        assert!(resume_at < first_plugin, "{args:?}");
    }

    #[test]
    fn nothing_but_a_resume_puts_that_flag_on_a_command_line() {
        // The guard against the flag leaking into every other session, which
        // would resume some other conversation under a person who asked for a
        // new agent.
        for intent in [Intent::Bare, Intent::Setup, new_task(Vec::new())] {
            let args = argv(&launch(intent, false));
            assert!(!args.iter().any(|a| a == "--resume"), "{args:?}");
        }
    }

    #[test]
    fn a_forked_session_carries_the_same_id_and_the_flag_that_branches_it() {
        // Continue in a new session: the resume's own command line with
        // `--fork-session` behind it, so the history is read and what is
        // written goes somewhere new. Leaving the original transcript alone is
        // what that flag is for, and it is why the Sessions tab grows a second
        // card after this.
        let args = argv(&launch(resuming("9f1c0a2e-6d4b-4f77-8f1a-0c2b3d4e5f60", true), true));
        assert_eq!(
            args,
            vec![
                "claude",
                "--resume",
                "9f1c0a2e-6d4b-4f77-8f1a-0c2b3d4e5f60",
                "--fork-session",
                "--plugin-dir",
                "/app/resources/smetana",
            ],
            "a forked session is the resumed command line plus --fork-session"
        );
    }

    #[test]
    fn only_a_fork_branches_the_transcript() {
        // The flag that decides whether somebody's transcript is written into
        // or left alone, so its absence is worth an assertion of its own rather
        // than only the whole-line comparison above: a fork leaking into Resume
        // in worktree would answer a person who asked to carry on in the same
        // conversation with a second one.
        let plain = argv(&launch(resume("abc"), false));
        assert!(!plain.iter().any(|a| a == "--fork-session"), "{plain:?}");
        for intent in [Intent::Bare, Intent::Setup, new_task(Vec::new())] {
            let args = argv(&launch(intent, false));
            assert!(!args.iter().any(|a| a == "--fork-session"), "{args:?}");
        }
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
                parent: None,
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
            reports: std::path::PathBuf::from("/p/.smetana/runs/1"),
            batch: 1,
            remove_worktrees: true,
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

    fn run_intent(mode: crate::runs::model::RunMode) -> Intent {
        Intent::Run {
            settings: crate::runs::model::RunSettings {
                scope: crate::runs::model::RunScope::Queue,
                mode,
                target_branch: "main".into(),
                create_target: false,
                min_priority: Some(2),
                // None in Solo, the way `RunSettings::validate` requires — the
                // same form the copy in `agents/mod.rs` keeps. A flat `Some(3)`
                // makes the Solo assertion below one about settings that can
                // never reach a profile at all.
                max_parallel_tasks: (!matches!(mode, crate::runs::model::RunMode::Solo))
                    .then_some(3),
                live_check: true,
                file_findings: true,
            },
            reports: PathBuf::from("/p/.smetana/runs/7"),
            batch: 1,
            remove_worktrees: true,
        }
    }

    #[test]
    fn an_unattended_batch_prints_its_stream_and_exits() {
        // `-p` is what makes the process end when the work does, which is the
        // whole of why the run's loop ever comes round. The stream format is
        // the other half: measured against the CLI, `-p` alone prints one line
        // at the very end, so the pane would sit empty for the length of a
        // batch.
        let args = argv(&launch(run_intent(crate::runs::model::RunMode::Auto), true));
        assert_eq!(args[0], "claude");
        assert_eq!(args[1..4], ["-p", "--verbose", "--output-format"], "{args:?}");
        assert_eq!(args[4], "stream-json", "{args:?}");
        assert!(
            args.iter().any(|a| a == "bypassPermissions"),
            "the autonomy switches are unaffected: {args:?}"
        );
        assert!(
            args.windows(2).any(|w| w[0] == "--plugin-dir" && w[1] == "/app/resources/smetana"),
            "and so are the plugins: {args:?}"
        );
    }

    #[test]
    fn a_supervised_or_solo_batch_keeps_its_interface() {
        // Both modes have a person answering in the terminal, and print mode
        // has no terminal to answer in.
        for mode in [crate::runs::model::RunMode::Supervised, crate::runs::model::RunMode::Solo] {
            let args = argv(&launch(run_intent(mode), true));
            assert!(!args.iter().any(|a| a == "-p"), "{mode:?}: {args:?}");
        }
    }

    #[test]
    fn a_persons_own_session_is_never_printed() {
        for intent in [Intent::Bare, Intent::EditTask { id: "a-1".into(), title: "t".into() }] {
            let args = argv(&launch(intent, true));
            assert!(!args.iter().any(|a| a == "-p"), "{args:?}");
        }
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

    #[test]
    fn a_tool_call_becomes_one_line_naming_the_tool_and_its_point() {
        let line = r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","id":"t1","name":"Bash","input":{"command":"git worktree add ../wt-1 -b task/x"}}]}}"#;
        assert_eq!(
            transcript_line(line),
            vec!["* Bash — git worktree add ../wt-1 -b task/x".to_string()]
        );
    }

    #[test]
    fn each_tool_is_summarized_by_the_field_that_says_what_it_is_doing() {
        let call = |name: &str, input: &str| {
            transcript_line(&format!(
                r#"{{"type":"assistant","message":{{"content":[{{"type":"tool_use","name":"{name}","input":{input}}}]}}}}"#
            ))
        };
        assert_eq!(call("Read", r#"{"file_path":"/p/src/main.rs"}"#), vec!["* Read — /p/src/main.rs"]);
        assert_eq!(call("Grep", r#"{"pattern":"fn main"}"#), vec!["* Grep — fn main"]);
        assert_eq!(
            call("Task", r#"{"subagent_type":"worker","description":"implement smetana-1"}"#),
            vec!["* Task — worker: implement smetana-1"]
        );
        assert_eq!(call("Skill", r#"{"skill":"smetana:merging"}"#), vec!["* Skill — smetana:merging"]);
        // A tool this table has never heard of still says which tool ran: the
        // point of the line is that something happened, and the detail is a
        // bonus.
        assert_eq!(call("Whatever", r#"{}"#), vec!["* Whatever"]);
    }

    #[test]
    fn the_agents_own_words_are_shown_and_long_ones_are_clipped() {
        let text = "x".repeat(400);
        let line = format!(
            r#"{{"type":"assistant","message":{{"content":[{{"type":"text","text":"{text}"}}]}}}}"#
        );
        let out = transcript_line(&line);
        assert_eq!(out.len(), 1);
        assert!(out[0].len() <= 204, "clipped to the reference's 200 plus the indent: {}", out[0].len());
        assert!(out[0].ends_with("..."), "{}", out[0]);
    }

    #[test]
    fn the_start_and_the_end_of_a_batch_are_each_one_line() {
        assert_eq!(
            transcript_line(r#"{"type":"system","subtype":"init","model":"claude-opus-5"}"#),
            vec!["-- session start (model claude-opus-5)"]
        );
        assert_eq!(
            transcript_line(
                r#"{"type":"result","is_error":false,"num_turns":3,"usage":{"input_tokens":6,"output_tokens":91}}"#
            ),
            vec!["-- batch result (in 6 / out 91 tok)"]
        );
    }

    #[test]
    fn noise_and_nonsense_produce_nothing() {
        // Hook chatter, the rate-limit event, a tool's result (routinely a whole
        // file), an event type this build has never heard of, a line that is not
        // JSON at all, and an empty line. Every one of them is an ordinary thing
        // to meet in that stream, and none of them is worth a row in the pane.
        for line in [
            r#"{"type":"system","subtype":"hook_started","hook_name":"SessionStart:startup"}"#,
            r#"{"type":"rate_limit_event","rate_limit_info":{"status":"allowed"}}"#,
            r#"{"type":"user","message":{"role":"user","content":[{"type":"tool_result","content":"one"}]}}"#,
            r#"{"type":"something_new_in_2027"}"#,
            "not json at all",
            "",
        ] {
            assert!(transcript_line(line).is_empty(), "{line}");
        }
    }

    #[test]
    fn nothing_agent_authored_reaches_the_pane_carrying_a_control_byte() {
        // The text and a tool's input are written by the agent, and an
        // assistant paragraph or a `Bash` command quoting terminal output
        // carries an escape sequence and a bell in the JSON, which serde
        // decodes into live bytes. Through the pane they would be colour and
        // cursor movement in a transcript specified as plain text, and a ring
        // that turns the row `needs-you` -- one of the one or two loud rows the
        // whole design budgets for a screen. The escapes below are written the
        // way the CLI writes them, as JSON `\u001b`, so this source file holds
        // no control byte of its own.
        let text = transcript_line(
            r#"{"type":"assistant","message":{"content":[{"type":"text","text":"red \u001b[31mhere\u001b[0m and a bell \u0007done"}]}}"#,
        );
        assert_eq!(text, vec!["   red [31mhere[0m and a bell done"]);
        assert!(!text[0].contains('\u{1b}') && !text[0].contains('\u{7}'), "{text:?}");

        let call = transcript_line(
            r#"{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Bash","input":{"command":"echo \u001b[1mhi\u001b[0m"}}]}}"#,
        );
        assert_eq!(call, vec!["* Bash — echo [1mhi[0m"]);
    }

    #[test]
    fn the_fields_of_a_system_event_are_collapsed_and_clipped_like_every_other() {
        // The model name and an API error message are interpolated the same way
        // the agent's own text is, and were the only two that reached the pane
        // raw: a newline in an error message is the very diagonal stepping the
        // `\r\n` rule exists against, and an unbounded one is a wall. What is
        // left of the escape below is its printable tail, which is text like any
        // other -- only the control byte itself is a thing this pane must not
        // be handed.
        assert_eq!(
            transcript_line(
                r#"{"type":"system","subtype":"api_retry","error":"overloaded\nupstream said \u001b[0m no","attempt":2}"#
            ),
            vec!["!! api retry (overloaded upstream said [0m no), attempt 2"]
        );
        assert_eq!(
            transcript_line(&format!(
                r#"{{"type":"system","subtype":"api_retry","error":"{}","attempt":1}}"#,
                "e".repeat(400)
            ))[0]
                .chars()
                .count(),
            // "!! api retry (" + 140 clipped characters + "), attempt 1"
            14 + MAX_DETAIL + 12
        );
        // Whitespace is collapsed *before* the emptiness is judged, or a model
        // field holding nothing but spaces renders as `(model )`.
        assert_eq!(
            transcript_line(r#"{"type":"system","subtype":"init","model":"   "}"#),
            vec!["-- session start"]
        );
    }

    #[test]
    fn an_api_retry_is_worth_saying_out_loud() {
        // The one piece of trouble this stream reports that a person watching
        // an overnight run would want to see: it is why nothing is happening.
        assert_eq!(
            transcript_line(r#"{"type":"system","subtype":"api_retry","error":"overloaded","attempt":2}"#),
            vec!["!! api retry (overloaded), attempt 2"]
        );
    }

    #[test]
    fn the_profile_hands_the_translator_over_and_only_claude_code_has_one() {
        assert!(Claude.transcript().is_some());
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
        assert_eq!(read.session_pct, Some(10));
        assert_eq!(read.session_reset.as_deref(), Some("Aug 7 at 8pm (Europe/Moscow)"));
        assert_eq!(read.week_pct, Some(20));
        assert_eq!(read.week_reset.as_deref(), Some("Aug 11 at 5:59pm (Europe/Moscow)"));
    }

    #[test]
    fn a_per_model_allowance_is_not_one_of_them() {
        // `Current week (Fable): 0% used` sits between the two lines that do
        // count, and reading it as the weekly figure would report 0% while the
        // week is nearly spent. The CLI answers an exhausted model limit by
        // switching models, so it is not a run's business at all.
        let read = usage(USAGE_OUTPUT).expect("the two lines are there");
        assert_eq!(read.week_pct, Some(20), "the per-model line must not overwrite the weekly one");
    }

    #[test]
    fn percentages_in_the_body_of_the_report_are_not_mistaken_for_limits() {
        // "60% of your usage came from subagent-heavy sessions" is a share of
        // what was spent, not a share of the allowance. Matching it would pause
        // a run at 8% of its actual limit.
        let pct = usage(USAGE_OUTPUT).and_then(|read| read.pct()).expect("the two lines are there");
        assert!(pct < 60, "read {pct}%, which is a line from the breakdown");
    }

    #[test]
    fn an_allowance_with_nothing_to_say_about_its_reset_is_still_a_reading() {
        // A fresh week prints no reset. Refusing the whole reading over the
        // missing half would leave the run blind to a session at 95%.
        let read = usage("Current session: 95% used\n").expect("a percentage is a reading");
        assert_eq!(read.session_pct, Some(95));
        assert_eq!(read.session_reset, None);
    }

    #[test]
    fn a_line_that_was_not_read_leaves_its_half_absent_rather_than_zero() {
        // Half a reading is a real answer and the half that arrived is shown,
        // but the other half is not invented: `Some(0)` here would be drawn on
        // the settings window as "This week: 0% used", a quota nobody read.
        let renamed = USAGE_OUTPUT.replace("Current week (all models):", "Weekly limit:");
        let read = usage(&renamed).expect("the session line is still there");
        assert_eq!(read.session_pct, Some(10));
        assert_eq!(read.week_pct, None);
        assert_eq!(read.week_reset, None);
        assert_eq!(read.pct(), Some(10), "the half that arrived is the whole of the reading");

        // And the same the other way round, since either line can be the one
        // that is reworded.
        let renamed = USAGE_OUTPUT.replace("Current session:", "This session:");
        let read = usage(&renamed).expect("the weekly line is still there");
        assert_eq!(read.session_pct, None);
        assert_eq!(read.session_reset, None);
        assert_eq!(read.week_pct, Some(20));
    }

    #[test]
    fn a_zero_the_cli_printed_is_a_reading_and_not_an_absence() {
        // The point of the distinction cuts both ways: a fresh week really does
        // print 0%, and hiding it would be the same class of lie as inventing
        // it. `Some(0)` is what puts "This week: 0% used" on screen honestly.
        let fresh = USAGE_OUTPUT.replace(
            "Current week (all models): 20% used · resets Aug 11 at 5:59pm (Europe/Moscow)",
            "Current week (all models): 0% used",
        );
        let read = usage(&fresh).expect("both lines are there");
        assert_eq!(read.week_pct, Some(0));
        assert_eq!(read.week_reset, None);
    }

    #[test]
    fn output_with_neither_line_in_it_is_no_reading_at_all() {
        // Not an empty reading: `decide` treats `None` as no reason to hold a
        // run up, and a `Usage` with both halves absent would be the same
        // answer written twice — this one keeps `report` able to call it
        // unreadable, which is a sentence a person can act on.
        assert_eq!(usage("Invalid API key · Please run /login"), None);
        assert_eq!(usage(""), None);
    }
}


