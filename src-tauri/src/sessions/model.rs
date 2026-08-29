//! The vocabulary of a session row, and every rule about a transcript that can
//! be decided from text alone.
//!
//! Everything here is pure: given a line, or a path, or a piece of message
//! content, it answers without touching a disk. That is what makes the rules
//! testable at all — the disk half is `read.rs`, and it carries its own tests
//! over temporary directories.

use std::path::{Component, Path};

use serde::{Deserialize, Serialize};

/// One row of the Sessions tab.
///
/// The field names are the front end's, and they are a contract rather than a
/// preference: `stores/mockBackend.js` answers this command with a hand-written
/// fixture under `npm run dev`, and nothing mechanical crosses between the two.
/// A field renamed here goes on being answered in its old shape by the browser
/// build, silently, which is the whole of that build's verification gone.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSummary {
    /// The transcript file's stem, which is the session id Claude Code resumes
    /// by. Taken from the name rather than from a `sessionId` field inside: the
    /// name is there before any record has been read, and the two agree.
    pub id: String,
    pub path: String,
    pub cwd: String,
    pub branch: Option<String>,
    pub title: Option<String>,
    /// `"user"` or `"assistant"` — whoever spoke last, for the card's prefix.
    pub last_role: Option<String>,
    pub last_text: Option<String>,
    pub messages: u32,
    /// Zero when the session ran none, which is what the card reads to leave
    /// the count off entirely.
    pub subagents: u32,
    pub model: Option<String>,
    pub modified_at: String,
    /// Whether the directory this session ran in is still on disk.
    ///
    /// A worktree is removed once its task is merged and the transcript stays
    /// behind, so on any machine that has done a few tasks this is `false` for
    /// a good number of rows — an ordinary state and not a fault. What reads it
    /// is the Resume verb: `claude --resume` resolves an id against the
    /// directory it is run in, so a session whose directory has gone cannot be
    /// picked up anywhere, and the row says so instead of offering a press that
    /// would be refused.
    ///
    /// Answered here rather than asked for when the menu opens, because the
    /// menu row has to be greyed at the moment it is drawn; and taken with the
    /// `stat` the list is already doing, one `is_dir` per row.
    ///
    /// It can be stale, and deliberately: the list is read when the tab is
    /// opened and never watched (see `mod.rs`), so a worktree removed since is
    /// a row still offering a resume. That press is refused by
    /// `terminal::service`'s own check, which is the one standing next to the
    /// spawn, and the refusal reaches the person as a sentence.
    pub cwd_exists: bool,
    /// The transcript's size in bytes, and the one field here that nothing on
    /// the row draws. It exists for the confirmation before a delete, which
    /// names what is about to go — the id, the path and how big it is. A
    /// dialog that asked the disk for it at the moment it opened would be a
    /// second read of a file the list has already `stat`ed, and one that could
    /// answer nothing at all if the file had gone in between.
    pub size: u64,
}

/// One line of a transcript, in as much detail as a row needs.
///
/// Deliberately loose. A transcript holds at least eleven record types —
/// `user`, `assistant`, `attachment`, `system`, `summary`, `queue-operation`,
/// `last-prompt`, `mode`, `permission-mode`, `file-history-snapshot`,
/// `atis-latch` — Claude Code adds to that list without asking anybody, and a
/// record in a shape this has never seen is an ordinary line to skip rather
/// than a failure. So every field is optional and unknown ones are ignored.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Record {
    #[serde(default, rename = "type")]
    pub kind: Option<String>,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub git_branch: Option<String>,
    #[serde(default)]
    pub is_sidechain: Option<bool>,
    /// Set on a `user` record the person did not type: the skill text and the
    /// hook output Claude Code injects into the conversation as if it came from
    /// them. See [`human_text`] for why that matters here.
    #[serde(default)]
    pub is_meta: Option<bool>,
    #[serde(default)]
    pub message: Option<Message>,
    /// Who the record came from, on a Claude Code new enough to say. See
    /// [`human_text`].
    #[serde(default)]
    pub origin: Option<Origin>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Origin {
    #[serde(default)]
    pub kind: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    /// Only an `assistant` record carries one, and it is the whole of what the
    /// card knows about the model.
    #[serde(default)]
    pub model: Option<String>,
    /// A string on a typed message, an array of blocks on everything else.
    #[serde(default)]
    pub content: Option<serde_json::Value>,
}

impl Record {
    pub fn is_user(&self) -> bool {
        self.kind.as_deref() == Some("user")
    }

    pub fn is_assistant(&self) -> bool {
        self.kind.as_deref() == Some("assistant")
    }
}

/// How many characters of a title or a last line travel to the front end.
///
/// A card shows one line of title and two of the last message, at a width
/// nobody here knows, so the cut is generous rather than exact — the ellipsis
/// is CSS's, and this only has to stop a 40 KB paste from crossing the IPC
/// boundary a few hundred times over.
pub const CLIP: usize = 240;

/// The wrappers Claude Code puts around text that is not the person talking,
/// inside records that are otherwise indistinguishable from what they typed.
///
/// `<system-reminder>` is context the harness injects mid-turn;
/// `<local-command-caveat>` and the `<command-*>` family are the echo of a
/// slash command — `/clear` on its own line, with its message and arguments —
/// `<local-command-stdout>` is what that command printed; and
/// `<task-notification>` is the harness reporting that a background subagent
/// has finished, which arrives as an ordinary `user` record with nothing else
/// to tell it apart. None of them is something a person would recognise as
/// having said, and a list of sessions titled "Caveat: The messages below were
/// generated by the user while running local commands" is a list of nothing.
const ENVELOPES: [&str; 8] = [
    "task-notification",
    "system-reminder",
    "local-command-caveat",
    "command-name",
    "command-message",
    "command-args",
    "command-contents",
    "local-command-stdout",
];

/// The text of a message's `content`, whatever shape it arrived in.
///
/// A string is the whole message. An array is blocks, and only `text` blocks
/// are words: `tool_use`, `tool_result`, `image` and `thinking` are the machine
/// talking to itself, and a card that showed the start of a tool result would
/// show a path and a diff where it promised a sentence.
pub fn message_text(content: &serde_json::Value) -> String {
    match content {
        serde_json::Value::String(text) => text.clone(),
        serde_json::Value::Array(blocks) => blocks
            .iter()
            .filter(|block| block.get("type").and_then(|t| t.as_str()) == Some("text"))
            .filter_map(|block| block.get("text").and_then(|t| t.as_str()))
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

/// The same text with every envelope block removed.
///
/// An envelope that was opened and never closed takes everything after it: a
/// truncated `<system-reminder>` is still not the person talking, and keeping
/// its tail would put the harness's own words in a title.
pub fn strip_envelopes(text: &str) -> String {
    let mut out = text.to_owned();
    for tag in ENVELOPES {
        let open = format!("<{tag}>");
        let close = format!("</{tag}>");
        while let Some(start) = out.find(&open) {
            let rest = &out[start + open.len()..];
            let end = match rest.find(&close) {
                Some(at) => start + open.len() + at + close.len(),
                None => out.len(),
            };
            out.replace_range(start..end, "");
        }
    }
    out
}

/// One line, clipped: every run of whitespace becomes a single space and the
/// result is cut to [`CLIP`] characters — characters and not bytes, so that a
/// cut never lands inside a multi-byte one.
pub fn one_line(text: &str) -> String {
    let collapsed = text.split_whitespace().collect::<Vec<_>>().join(" ");
    match collapsed.char_indices().nth(CLIP) {
        Some((at, _)) => collapsed[..at].trim_end().to_owned(),
        None => collapsed,
    }
}

/// What a person actually said in this record, or `None` if they said nothing.
///
/// Three ways a `user` record is not a person talking, and all three are
/// common enough to decide the title of most sessions on a real machine:
///
/// - `isSidechain` marks a subagent's turn, whose prompt was written by the
///   agent that spawned it.
/// - `isMeta` marks text Claude Code injected — a skill's body, a hook's
///   output — carried on a `user` record because that is where context goes.
///   Without this check a session started from a run is titled "Base directory
///   for this skill: …", which is true of a third of them here.
/// - The envelopes above, and blocks that are not text at all.
///
/// A fourth signal is used when it is there and never required: a recent
/// Claude Code stamps a record with `origin: {"kind": "human"}` when a person
/// typed it and something else — `task-notification`, an sdk prompt — when it
/// did not. That is a straight answer to this whole question, and it is only an
/// answer for files written by a version that has it, which is why the rules
/// above stay: an older transcript carries no `origin` at all and is read by
/// the shape of what is in it.
pub fn human_text(record: &Record) -> Option<String> {
    if !record.is_user() || record.is_sidechain == Some(true) || record.is_meta == Some(true) {
        return None;
    }
    if let Some(kind) = record.origin.as_ref().and_then(|origin| origin.kind.as_deref()) {
        if kind != "human" {
            return None;
        }
    }
    let content = record.message.as_ref()?.content.as_ref()?;
    let text = one_line(&strip_envelopes(&message_text(content)));
    (!text.is_empty()).then_some(text)
}

/// What the last speaker said, for the card's second line, or `None` when this
/// record is not somebody speaking.
///
/// A subagent's turn counts here, unlike in a title: it is still the last thing
/// that happened in the session, and the alternative — walking back past every
/// subagent turn to the parent's last word — would mean reading further than
/// the tail window this is called over.
pub fn spoken_text(record: &Record) -> Option<(String, String)> {
    let role = if record.is_user() {
        "user"
    } else if record.is_assistant() {
        "assistant"
    } else {
        return None;
    };
    let content = record.message.as_ref()?.content.as_ref()?;
    let text = one_line(&strip_envelopes(&message_text(content)));
    (!text.is_empty()).then(|| (role.to_owned(), text))
}

/// Whether a session with this working directory belongs to this project.
///
/// The project folder itself, a worktree under `.worktrees/`, `src-tauri` — all
/// of them are the project. Compared by path components rather than by string
/// prefix, so that a sibling project named `smetana-backend` is not swallowed
/// by `smetana`.
pub fn belongs_to(cwd: &str, project: &Path) -> bool {
    Path::new(cwd).starts_with(project)
}

/// Whether a folder under `~/.claude/projects` could hold sessions of this
/// project — a prefilter, and nothing more.
///
/// The folder is named after the working directory with every character that is
/// not a letter or a digit replaced by `-`, and **that transform is not
/// invertible**: a `-` in the name could have been a separator, a dot, or a `-`
/// somebody typed. So this cannot decide membership, and does not try to. What
/// it can do is rule a folder out, because the transform is character-wise and
/// therefore length- and prefix-preserving: if a working directory lies inside
/// the project, its folder name starts with the project's own encoding.
///
/// The match is deliberately tolerant on exactly one axis — a `-` in the folder
/// name matches any character of the project path. That covers this being wrong
/// about which characters Claude Code replaces, today or after an upgrade,
/// without ever letting the answer through on a name that is genuinely a
/// different path. Being wrong the other way would be silent: a session that
/// exists and is never listed.
pub fn folder_could_hold(folder: &str, project: &Path) -> bool {
    let project = project.to_string_lossy();
    let folder: Vec<char> = folder.chars().collect();
    let expected: Vec<char> = project.chars().collect();
    if folder.len() < expected.len() {
        return false;
    }
    if !folder.iter().zip(&expected).all(|(got, want)| got == want || *got == '-') {
        return false;
    }
    // A folder for a directory *inside* the project continues with a separator,
    // which the transform has already turned into a `-`. Without this, the
    // encoding of `/home/me/smetana` would accept the folder of
    // `/home/me/smetana-backend`.
    matches!(folder.get(expected.len()), None | Some('-'))
}

/// Whether a path names a transcript this app is allowed to act on.
///
/// The Sessions tab is the one place in this app that reaches outside the
/// project a person opened: a transcript lives under `~/.claude/projects`, it
/// is not in anybody's repository, and the menu on a session row opens it,
/// shows it and — with a confirmation — deletes it. The path travels from the
/// front end, so this is the whole of what stands between that menu and any
/// other file on the machine, and it is stated here, pure, where a test can
/// read it.
///
/// Three conditions, and each is load-bearing:
///
/// - the extension is `jsonl`, which is what Claude Code writes and the only
///   thing the list ever offers;
/// - the path lies under the projects root, compared by **components** rather
///   than by string prefix, so `~/.claude/projects-backup/x.jsonl` is not under
///   `~/.claude/projects`;
/// - no component is `..`. `Path::starts_with` is lexical, so without this
///   `<root>/../../etc/passwd.jsonl` would satisfy the clause above while
///   naming something entirely elsewhere. Canonicalising instead was the
///   rejected alternative: it resolves symlinks, and a transcript folder
///   somebody has symlinked onto another disk is an ordinary thing that would
///   then stop being deletable — the failure this guard exists for is a path
///   that walks *out*, and a `..` cannot be anything else here.
pub fn is_transcript(path: &Path, root: &Path) -> bool {
    path.extension().is_some_and(|ext| ext == "jsonl")
        && path.starts_with(root)
        && !path.components().any(|part| matches!(part, Component::ParentDir))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user(content: serde_json::Value) -> Record {
        Record {
            kind: Some("user".into()),
            message: Some(Message { model: None, content: Some(content) }),
            ..Record::default()
        }
    }

    #[test]
    fn a_typed_message_is_a_string_and_a_block_message_is_its_text_blocks() {
        assert_eq!(message_text(&serde_json::json!("just words")), "just words");
        assert_eq!(
            message_text(&serde_json::json!([
                {"type": "text", "text": "first"},
                {"type": "image", "source": {}},
                {"type": "text", "text": "second"}
            ])),
            "first\nsecond"
        );
    }

    #[test]
    fn a_message_of_tool_results_alone_has_no_words_in_it() {
        assert_eq!(
            message_text(&serde_json::json!([{"type": "tool_result", "content": "ok"}])),
            ""
        );
    }

    #[test]
    fn a_slash_command_echo_leaves_nothing_behind() {
        let echoed = "<command-name>/clear</command-name>\n  \
                      <command-message>clear</command-message>\n  \
                      <command-args></command-args>";
        assert_eq!(strip_envelopes(echoed).trim(), "");
    }

    #[test]
    fn a_reminder_around_a_sentence_is_removed_and_the_sentence_is_kept() {
        let text = "<system-reminder>Do not mention this</system-reminder>Move the card";
        assert_eq!(strip_envelopes(text), "Move the card");
    }

    #[test]
    fn an_envelope_that_was_never_closed_takes_the_rest_with_it() {
        let text = "Move the card<system-reminder>cut off here";
        assert_eq!(strip_envelopes(text), "Move the card");
    }

    #[test]
    fn a_title_is_one_line_and_no_longer_than_the_clip() {
        let text = "  a  sentence\nsplit over\tlines  ";
        assert_eq!(one_line(text), "a sentence split over lines");
        let long = "x".repeat(CLIP + 50);
        assert_eq!(one_line(&long).chars().count(), CLIP);
    }

    /// A multi-byte character, so that a cut made in bytes would land inside
    /// one and panic rather than merely shorten the string.
    #[test]
    fn clipping_counts_characters_rather_than_bytes() {
        let long = "é".repeat(CLIP + 10);
        assert_eq!(one_line(&long).chars().count(), CLIP);
    }

    #[test]
    fn injected_skill_text_is_not_something_the_person_said() {
        let mut record = user(serde_json::json!([{"type": "text", "text": "Base directory"}]));
        record.is_meta = Some(true);
        assert_eq!(human_text(&record), None);
    }

    #[test]
    fn a_subagents_prompt_is_not_something_the_person_said() {
        let mut record = user(serde_json::json!("Review this diff"));
        record.is_sidechain = Some(true);
        assert_eq!(human_text(&record), None);
    }

    #[test]
    fn the_first_thing_a_person_typed_survives_all_of_it() {
        let record = user(serde_json::json!("Talk to me in Russian:\n  everything"));
        assert_eq!(human_text(&record).as_deref(), Some("Talk to me in Russian: everything"));
    }

    #[test]
    fn a_subagents_last_word_is_still_the_last_word_of_the_session() {
        let mut record = user(serde_json::json!("Review this diff"));
        record.is_sidechain = Some(true);
        assert_eq!(
            spoken_text(&record),
            Some(("user".to_owned(), "Review this diff".to_owned()))
        );
    }

    #[test]
    fn a_notification_that_a_subagent_finished_is_not_the_person_talking() {
        let record = user(serde_json::json!(
            "<task-notification>\n<task-id>a18</task-id>\n<status>completed</status>\n</task-notification>"
        ));
        assert_eq!(human_text(&record), None);
        assert_eq!(spoken_text(&record), None);
    }

    #[test]
    fn a_record_stamped_with_a_non_human_origin_is_taken_at_its_word() {
        let mut record = user(serde_json::json!("Do the thing"));
        record.origin = Some(Origin { kind: Some("task-notification".into()) });
        assert_eq!(human_text(&record), None);
        record.origin = Some(Origin { kind: Some("human".into()) });
        assert_eq!(human_text(&record).as_deref(), Some("Do the thing"));
    }

    #[test]
    fn a_session_in_the_project_a_worktree_and_a_subdirectory_all_belong() {
        let project = Path::new("/home/me/smetana");
        assert!(belongs_to("/home/me/smetana", project));
        assert!(belongs_to("/home/me/smetana/src-tauri", project));
        assert!(belongs_to("/home/me/smetana/.worktrees/smetana-oln", project));
    }

    #[test]
    fn a_sibling_project_whose_name_merely_starts_the_same_does_not() {
        let project = Path::new("/home/me/smetana");
        assert!(!belongs_to("/home/me/smetana-backend", project));
        assert!(!belongs_to("/home/me/other", project));
        assert!(!belongs_to("/tmp/scratch/smetana", project));
    }

    #[test]
    fn the_folder_prefilter_keeps_the_project_its_worktrees_and_its_subdirectories() {
        let project = Path::new("/home/me/smetana");
        assert!(folder_could_hold("-home-me-smetana", project));
        assert!(folder_could_hold("-home-me-smetana-src-tauri", project));
        assert!(folder_could_hold("-home-me-smetana--worktrees-smetana-oln", project));
    }

    #[test]
    fn the_folder_prefilter_drops_another_project_and_a_longer_name() {
        let project = Path::new("/home/me/smetana");
        assert!(!folder_could_hold("-home-me-frontend", project));
        assert!(!folder_could_hold("-home-me-smetanaXbackend", project));
        assert!(!folder_could_hold("-home-me", project));
    }

    #[test]
    fn a_transcript_under_the_projects_root_is_one_this_app_may_act_on() {
        let root = Path::new("/home/me/.claude/projects");
        assert!(is_transcript(Path::new("/home/me/.claude/projects/-p/abc.jsonl"), root));
        assert!(is_transcript(Path::new("/home/me/.claude/projects/-p/abc/subagents/a.jsonl"), root));
    }

    #[test]
    fn nothing_outside_the_projects_root_is_a_transcript() {
        let root = Path::new("/home/me/.claude/projects");
        // A sibling folder whose name merely starts the same.
        assert!(!is_transcript(Path::new("/home/me/.claude/projects-backup/a.jsonl"), root));
        // Somewhere else entirely.
        assert!(!is_transcript(Path::new("/home/me/dev/smetana/src/main.js"), root));
        // Under the root, and not a transcript.
        assert!(!is_transcript(Path::new("/home/me/.claude/projects/-p/notes.md"), root));
        // No extension at all.
        assert!(!is_transcript(Path::new("/home/me/.claude/projects/-p"), root));
    }

    /// `Path::starts_with` is lexical, so a path that walks back out of the
    /// root passes it. This is the case the guard's third clause exists for and
    /// the one that would cost somebody a file they never named.
    #[test]
    fn a_path_that_walks_back_out_of_the_root_is_refused() {
        let root = Path::new("/home/me/.claude/projects");
        assert!(!is_transcript(
            Path::new("/home/me/.claude/projects/../../../tmp/anything.jsonl"),
            root
        ));
    }

    #[test]
    fn the_folder_prefilter_survives_a_transform_that_replaced_more_than_expected() {
        // The folder name is allowed to hold a `-` wherever the project path
        // holds anything at all: this is a prefilter and it must not be the
        // thing that loses a session when Claude Code changes what it escapes.
        let project = Path::new("/home/me/sme_tana");
        assert!(folder_could_hold("-home-me-sme-tana", project));
        assert!(folder_could_hold("-home-me-sme_tana", project));
    }
}
