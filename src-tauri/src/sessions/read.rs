//! The disk half: turning a folder of transcripts into a list of rows.
//!
//! **A transcript is streamed, never loaded.** The ceiling on what one file
//! costs in memory is [`MAX_LINE`] for the line being looked at plus
//! [`TAIL_WINDOW`] for the window read back from the end — 320 KiB, for a file
//! of any size, and the largest one on the machine this was written against is
//! 16 MB. Files are summarised one at a time, so that is the ceiling for the
//! whole command as well.
//!
//! **One pass forward, one window back.** Everything a row needs is in one of
//! three places: at the head (`cwd`, `gitBranch`, the session's title), at the
//! tail (the last thing anybody said, the model), or is a count over the
//! lines. The forward pass does the head and the counts together and is the
//! only thing here that touches every byte; the tail is a seek.
//!
//! **What that costs, measured rather than guessed** (Apple Silicon, macOS,
//! release build, warm cache, against this project's own history): 299
//! sessions across three folders, 301 MB of transcript, in **330–355 ms** —
//! one linear read of every file that belongs plus a seek per file. The counts
//! are what make it linear, and they are the reason `commands.rs` puts the
//! whole thing on the blocking pool. A file that turns out to belong to
//! another project costs only its first few records: the forward pass gives up
//! at the line that says so. `bench_listing_the_real_projects_folder` at the
//! bottom of this file is where those numbers come from, and how to take them
//! again.
//!
//! Taking the title from the `ai-title` record rather than from the person's
//! first words cost **nothing measurable**: 332–354 ms before, 338–355 ms
//! after, over five timed runs each side of the change, which is the same
//! number twice. It could not cost much — the record is inside the head budget
//! the pass was reading anyway, and [`HEAD_LINES`] carries the measurement
//! that says so. What it bought, on the same 299 sessions: **122 distinct
//! titles became 214**, and the one phrase that titled **142** of them now
//! titles 60.

use std::fs::File;
use std::io::{BufRead, BufReader, ErrorKind, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use super::model::{
    belongs_to, folder_could_hold, generated_title, human_text, spoken_text, Record,
    SessionSummary,
};

/// The most of one line that is ever held. A tool result carrying a file is a
/// single line of megabytes, and none of what this reads is ever that far into
/// one: `type`, `cwd`, `isSidechain` and the start of a message all sit in the
/// first few hundred bytes of the record that carries them.
const MAX_LINE: usize = 64 * 1024;

/// How far back from the end the last spoken line is looked for. Measured
/// rather than picked: across this project's 276 transcripts the last message
/// with words in it sits within a few records of the end, and the tail is
/// otherwise `system`, `attachment` and `last-prompt` bookkeeping.
const TAIL_WINDOW: u64 = 256 * 1024;

/// How many lines at the head are parsed as JSON, at most.
///
/// The rest of the pass is substring counting. On this machine the first thing
/// a person typed is within the first 61 KB of every transcript and well inside
/// the first hundred records; the budget is generous against a session that
/// opens with an unusual amount of injected context, and it is what stops the
/// head half of the pass from turning into a parse of the whole file.
///
/// It is also the budget the generated title has to fall inside, and that was
/// measured rather than hoped for. The `ai-title` record sits deep in *bytes* —
/// 29 to 47 KB into a file for the middle nine tenths of them, and 699 KB into
/// the worst — which sounds far and is not, because those bytes are a handful
/// of enormous injected records. Counted in **lines**, which is what this
/// budget is in, the record is at index 7 to 88 across the 211 transcripts here
/// that have one, median 15, and **not one of them is past line 500**: the
/// furthest is inside a fifth of the budget. So the title costs no extra
/// reading at all — it is found by the pass that was already going to visit
/// those lines. A file that ever did carry one past this line falls back to the
/// person's first words, which is the answer this gave for every file before
/// the record existed.
const HEAD_LINES: usize = 500;

/// The record that carries the generated title, as a substring, so that the
/// head pass can skip parsing every line that is not one.
const AI_TITLE: &str = "\"type\":\"ai-title\"";

/// The substrings the counting pass looks for, before any JSON parse.
///
/// They are exact enough to count with, and that was checked against the real
/// files rather than assumed: a nested occurrence of the same text inside a
/// message would arrive escaped (`\"type\":\"user\"`) and so cannot match, and
/// over the four largest transcripts here — 3250 lines, 17 MB — the substring
/// count and a full `serde_json` parse of every line agree exactly.
const USER: &str = "\"type\":\"user\"";
const ASSISTANT: &str = "\"type\":\"assistant\"";
const SIDECHAIN: &str = "\"isSidechain\":true";

/// One line of a transcript, and what it cost to get here.
struct Line {
    /// The line was longer than [`MAX_LINE`] and only its start was kept. It is
    /// still counted — the record type is at the front of the record — but it
    /// is not offered to the JSON parser, which would fail on half an object.
    truncated: bool,
}

/// The next line, into a buffer that never grows past [`MAX_LINE`].
///
/// `BufRead::read_until` would do this in one call and is not usable here for
/// exactly one reason: it grows the buffer to the length of the line, so a
/// transcript with a 10 MB tool result in it would decide this command's peak
/// memory. Everything past the cap is read and dropped rather than skipped, so
/// the reader still ends up on the next line.
fn next_line(reader: &mut impl BufRead, line: &mut Vec<u8>) -> std::io::Result<Option<Line>> {
    line.clear();
    let mut seen = 0usize;
    let mut truncated = false;
    loop {
        let (used, done) = {
            let available = match reader.fill_buf() {
                Ok(bytes) => bytes,
                Err(err) if err.kind() == ErrorKind::Interrupted => continue,
                Err(err) => return Err(err),
            };
            if available.is_empty() {
                return Ok((seen > 0).then_some(Line { truncated }));
            }
            let (used, done) = match available.iter().position(|byte| *byte == b'\n') {
                Some(at) => (at + 1, true),
                None => (available.len(), false),
            };
            let chunk = &available[..used];
            let room = MAX_LINE.saturating_sub(line.len()).min(chunk.len());
            if room < chunk.len() {
                truncated = true;
            }
            line.extend_from_slice(&chunk[..room]);
            (used, done)
        };
        reader.consume(used);
        seen += used;
        if done {
            return Ok(Some(Line { truncated }));
        }
    }
}

/// What the forward pass learns.
#[derive(Default)]
struct Facts {
    cwd: Option<String>,
    branch: Option<String>,
    /// Claude Code's own one-line title for the session, when the transcript
    /// carries one. It wins over `human_title`; see [`summarise`].
    generated_title: Option<String>,
    /// The first thing the person actually typed — the title of a transcript
    /// with no generated one, and the only title there was before that record
    /// existed.
    human_title: Option<String>,
    model: Option<String>,
    messages: u32,
    sidechains: u32,
}

/// The forward pass: the head's facts and the counts, in one read.
///
/// `None` means this file is not a session of this project — either its `cwd`
/// says so, in which case the pass stops at that line and the rest of the file
/// is never read, or no record in the head carries a `cwd` at all and there is
/// nothing to place it by.
fn scan_forward(file: File, project: &Path, also: Option<&Path>) -> Option<Facts> {
    let mut reader = BufReader::with_capacity(64 * 1024, file);
    let mut buf: Vec<u8> = Vec::with_capacity(8 * 1024);
    let mut facts = Facts::default();
    let mut index = 0usize;
    while let Ok(Some(line)) = next_line(&mut reader, &mut buf) {
        let text = String::from_utf8_lossy(&buf);
        let is_user = text.contains(USER);
        let is_assistant = text.contains(ASSISTANT);
        if is_user || is_assistant {
            facts.messages += 1;
        }
        if text.contains(SIDECHAIN) {
            facts.sidechains += 1;
        }
        // The head half. A line is parsed only while something it could carry
        // is still missing, and only when a substring says it might carry it:
        // parsing every line of the head would be the thing this file exists to
        // avoid, on a smaller scale. The question is asked inside the budget
        // and not before it, so that a file of ten thousand lines is scanned
        // for these substrings over its first five hundred and no further.
        if index < HEAD_LINES && !line.truncated {
            let wants = (facts.cwd.is_none() && text.contains("\"cwd\""))
                || (facts.generated_title.is_none() && text.contains(AI_TITLE))
                || (facts.human_title.is_none() && is_user)
                || (facts.model.is_none() && is_assistant);
            if wants {
                if let Ok(record) = serde_json::from_str::<Record>(&text) {
                    if facts.cwd.is_none() {
                        if let Some(cwd) = record.cwd.as_deref().filter(|cwd| !cwd.is_empty()) {
                            let ours = belongs_to(cwd, project)
                                || also.is_some_and(|other| belongs_to(cwd, other));
                            if !ours {
                                return None;
                            }
                            facts.cwd = Some(cwd.to_owned());
                            facts.branch =
                                record.git_branch.clone().filter(|branch| !branch.is_empty());
                        }
                    }
                    // Both titles are collected, and the choice between them is
                    // made once at the end: the generated one usually arrives
                    // several records *after* the person's first words, so
                    // stopping at the human title would be stopping too early.
                    if facts.generated_title.is_none() {
                        facts.generated_title = generated_title(&record);
                    }
                    if facts.human_title.is_none() {
                        facts.human_title = human_text(&record);
                    }
                    if facts.model.is_none() {
                        facts.model = record
                            .message
                            .as_ref()
                            .and_then(|message| message.model.clone())
                            .filter(|model| !model.is_empty());
                    }
                }
            }
        }
        index += 1;
        // Past the head with nothing to place the file by. Reading the rest for
        // counts nobody will see is the one way this could touch a whole file
        // for nothing, so it stops here instead.
        if index >= HEAD_LINES && facts.cwd.is_none() {
            return None;
        }
    }
    facts.cwd.is_some().then_some(facts)
}

/// What the tail window gives up: who spoke last, what they said, and the model
/// that was answering by the end of the session.
#[derive(Default)]
struct Tail {
    role: Option<String>,
    text: Option<String>,
    model: Option<String>,
}

/// The last words in the file, read backwards from a bounded window.
///
/// The window is read whole and walked in reverse rather than the file being
/// iterated: iterating would mean touching every byte a second time for the
/// sake of the last few records. A window that starts mid-line drops that first
/// fragment — half a record is not parseable and its start is somewhere behind.
fn scan_tail(path: &Path, size: u64) -> Tail {
    let mut tail = Tail::default();
    let Ok(mut file) = File::open(path) else { return tail };
    let from = size.saturating_sub(TAIL_WINDOW);
    if file.seek(SeekFrom::Start(from)).is_err() {
        return tail;
    }
    let mut window = Vec::new();
    if file.take(TAIL_WINDOW).read_to_end(&mut window).is_err() {
        return tail;
    }
    let text = String::from_utf8_lossy(&window);
    let mut lines: Vec<&str> = text.lines().collect();
    if from > 0 && !lines.is_empty() {
        lines.remove(0);
    }
    for line in lines.iter().rev() {
        if !line.contains(USER) && !line.contains(ASSISTANT) {
            continue;
        }
        let Ok(record) = serde_json::from_str::<Record>(line) else { continue };
        if tail.model.is_none() {
            tail.model = record
                .message
                .as_ref()
                .and_then(|message| message.model.clone())
                .filter(|model| !model.is_empty());
        }
        if tail.text.is_none() {
            if let Some((role, said)) = spoken_text(&record) {
                tail.role = Some(role);
                tail.text = Some(said);
            }
        }
        if tail.text.is_some() && tail.model.is_some() {
            break;
        }
    }
    tail
}

/// How many subagents this session ran.
///
/// Two layouts, because Claude Code changed where it puts them and both are on
/// disk at once. The older one writes a subagent's turns into the session's own
/// transcript, marked `isSidechain: true`, which the forward pass counts. The
/// newer one writes each subagent its own file under
/// `<session id>/subagents/agent-*.jsonl` and leaves nothing sidechained in the
/// transcript — on the machine this was written against **none** of the 783
/// transcripts holds a single `isSidechain` record, and 111 of one project's
/// 276 sessions have a folder of them.
///
/// So the folder wins when it exists. The two are not the same unit — files are
/// agents, sidechained records are turns — and the folder is the one the card
/// means, which is also what Orca shows ("3 subagents", not 3 turns). The
/// inline count stays as the answer for an older transcript, where it is the
/// only signal there is.
fn subagents(folder: &Path, id: &str, inline: u32) -> u32 {
    let dir = folder.join(id).join("subagents");
    let Ok(entries) = std::fs::read_dir(&dir) else { return inline };
    let counted = entries
        .flatten()
        .filter(|entry| {
            entry.path().extension().is_some_and(|ext| ext == "jsonl")
                && entry.file_name().to_string_lossy().starts_with("agent-")
        })
        .count() as u32;
    if counted > 0 {
        counted
    } else {
        inline
    }
}

/// The file's mtime, as the front end wants it.
///
/// The last activity of a session is deliberately the file's mtime and not the
/// timestamp on its last record: the two agree — a record is what moves the
/// mtime — and one of them costs a `stat` this code has already done while the
/// other costs finding and parsing a record at the end of the file.
fn modified_at(meta: &std::fs::Metadata) -> String {
    let Ok(time) = meta.modified() else { return String::new() };
    chrono::DateTime::<chrono::Utc>::from(time)
        .to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

/// One row, or `None` when this file is not a session of this project.
fn summarise(
    path: &Path,
    folder: &Path,
    project: &Path,
    also: Option<&Path>,
) -> Option<SessionSummary> {
    let id = path.file_stem()?.to_string_lossy().into_owned();
    let meta = std::fs::metadata(path).ok()?;
    if !meta.is_file() {
        return None;
    }
    let facts = scan_forward(File::open(path).ok()?, project, also)?;
    let tail = scan_tail(path, meta.len());
    Some(SessionSummary {
        subagents: subagents(folder, &id, facts.sidechains),
        id,
        path: path.to_string_lossy().into_owned(),
        cwd: facts.cwd.unwrap_or_default(),
        branch: facts.branch,
        /* The title rule, in one line: Claude Code's own one-liner when the
           transcript has one, and the first thing the person typed when it does
           not. `generated_title` has already refused an empty one, so an
           `ai-title` record saying nothing falls through here too. */
        title: facts.generated_title.or(facts.human_title),
        last_role: tail.role,
        last_text: tail.text,
        messages: facts.messages,
        model: tail.model.or(facts.model),
        modified_at: modified_at(&meta),
        /* The `stat` this function has already done, read a second time for
           nothing extra. See `SessionSummary::size` for who wants it. */
        size: meta.len(),
    })
}

/// Every session of this project under a given `projects` root.
///
/// `root` is a parameter rather than `~/.claude/projects` read in here, so that
/// the whole of this is testable over a temporary directory holding real
/// transcript files.
pub fn list_in(root: &Path, project: &Path) -> Vec<SessionSummary> {
    let project = project.to_path_buf();
    // Both spellings of the project's path, because a path with a symlink in it
    // — `/tmp` on macOS is `/private/tmp` — reaches this command in whichever
    // form the front end holds, while a transcript holds whichever form Claude
    // Code was started with. Comparing against both is cheaper than being wrong
    // in a way whose symptom is an empty list.
    let real = project.canonicalize().ok().filter(|real| *real != project);
    let Ok(entries) = std::fs::read_dir(root) else { return Vec::new() };

    let mut sessions = Vec::new();
    for folder in entries.flatten() {
        let name = folder.file_name().to_string_lossy().into_owned();
        let keep = folder_could_hold(&name, &project)
            || real.as_deref().is_some_and(|real| folder_could_hold(&name, real));
        if !keep || !folder.path().is_dir() {
            continue;
        }
        let Ok(files) = std::fs::read_dir(folder.path()) else { continue };
        for file in files.flatten() {
            let path = file.path();
            if path.extension().is_none_or(|ext| ext != "jsonl") {
                continue;
            }
            if let Some(session) =
                summarise(&path, &folder.path(), &project, real.as_deref())
            {
                sessions.push(session);
            }
        }
    }
    // Newest first. The timestamps are fixed-width UTC, so comparing the strings
    // is comparing the instants; the id breaks a tie inside one second, only so
    // that the order does not change between two calls that read the same disk.
    sessions.sort_by(|a, b| b.modified_at.cmp(&a.modified_at).then_with(|| a.id.cmp(&b.id)));
    sessions
}

/// Where Claude Code keeps its transcripts. `HOME` rather than a crate, the way
/// `agents::library`, `runs::browser` and `tracker::access` already read it.
pub(super) fn projects_root() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .filter(|home| !home.as_os_str().is_empty())
        .map(|home| home.join(".claude/projects"))
}

/// Every session of this project. An empty list is the answer to a machine with
/// no Claude Code on it, a folder that cannot be read, and a project nobody has
/// opened a session in — none of the three is a failure, and the command has no
/// way to report one.
pub fn list(project: &Path) -> Vec<SessionSummary> {
    match projects_root() {
        Some(root) => list_in(&root, project),
        None => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("smetana-sessions-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("a temporary directory");
        dir
    }

    /// The folder Claude Code would name for this working directory.
    fn folder_for(root: &Path, cwd: &Path) -> PathBuf {
        let name: String = cwd
            .to_string_lossy()
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
            .collect();
        let folder = root.join(name);
        std::fs::create_dir_all(&folder).expect("a project folder");
        folder
    }

    fn user_line(cwd: &Path, branch: &str, text: &str) -> String {
        format!(
            r#"{{"parentUuid":null,"isSidechain":false,"isMeta":false,"type":"user","cwd":"{}","gitBranch":"{}","message":{{"role":"user","content":{}}},"uuid":"u1"}}"#,
            cwd.display(),
            branch,
            serde_json::to_string(text).unwrap()
        )
    }

    fn assistant_line(cwd: &Path, text: &str) -> String {
        format!(
            r#"{{"parentUuid":"u1","isSidechain":false,"type":"assistant","cwd":"{}","gitBranch":"main","message":{{"role":"assistant","model":"claude-opus-5","content":[{{"type":"text","text":{}}}]}},"uuid":"a1"}}"#,
            cwd.display(),
            serde_json::to_string(text).unwrap()
        )
    }

    /// The record Claude Code writes when it has named the session itself.
    fn ai_title_line(text: &str) -> String {
        format!(
            r#"{{"type":"ai-title","aiTitle":{},"sessionId":"s1"}}"#,
            serde_json::to_string(text).unwrap()
        )
    }

    /// A transcript on disk, in the folder Claude Code would have put it in.
    fn write_session(root: &Path, cwd: &Path, id: &str, lines: &[String]) -> PathBuf {
        let folder = folder_for(root, cwd);
        let path = folder.join(format!("{id}.jsonl"));
        let mut text = lines.join("\n");
        text.push('\n');
        std::fs::write(&path, text).expect("a transcript");
        path
    }

    fn ordinary_session(root: &Path, cwd: &Path, id: &str) -> PathBuf {
        write_session(
            root,
            cwd,
            id,
            &[
                r#"{"type":"queue-operation","operation":"start","sessionId":"x"}"#.to_owned(),
                user_line(cwd, "main", "Move the card to done"),
                assistant_line(cwd, "Moved it."),
            ],
        )
    }

    #[test]
    fn a_session_of_a_different_project_is_not_listed() {
        let root = temp_dir("elsewhere-root");
        let project = temp_dir("elsewhere-project");
        let other = temp_dir("elsewhere-other");
        ordinary_session(&root, &project, "ours");
        ordinary_session(&root, &other, "theirs");

        let listed = list_in(&root, &project);
        assert_eq!(listed.len(), 1, "only the project's own session belongs");
        assert_eq!(listed[0].id, "ours");
    }

    #[test]
    fn a_session_of_a_project_whose_name_merely_starts_the_same_is_not_listed() {
        // The folder name cannot answer this and is not asked to: the encoding
        // of the sibling's path starts with the encoding of the project's.
        let root = temp_dir("sibling-root");
        let project = temp_dir("sibling-project");
        let sibling = project.with_file_name(format!(
            "{}-backend",
            project.file_name().unwrap().to_string_lossy()
        ));
        std::fs::create_dir_all(&sibling).expect("a sibling project");
        ordinary_session(&root, &sibling, "theirs");

        assert!(list_in(&root, &project).is_empty());
    }

    #[test]
    fn a_session_run_inside_a_worktree_of_the_project_is_listed() {
        let root = temp_dir("worktree-root");
        let project = temp_dir("worktree-project");
        let worktree = project.join(".worktrees/smetana-oln");
        std::fs::create_dir_all(&worktree).expect("a worktree");
        write_session(
            &root,
            &worktree,
            "inside",
            &[
                user_line(&worktree, "fix/smetana-oln", "Fix the sessions tab"),
                assistant_line(&worktree, "On it."),
            ],
        );

        let listed = list_in(&root, &project);
        assert_eq!(listed.len(), 1, "a worktree is part of the project");
        assert_eq!(listed[0].cwd, worktree.to_string_lossy());
        assert_eq!(listed[0].branch.as_deref(), Some("fix/smetana-oln"));
    }

    #[test]
    fn a_broken_line_does_not_take_the_rest_of_the_file_with_it() {
        let root = temp_dir("broken-root");
        let project = temp_dir("broken-project");
        write_session(
            &root,
            &project,
            "broken",
            &[
                "{ this is not json at all".to_owned(),
                String::new(),
                user_line(&project, "main", "Still readable"),
                "}{".to_owned(),
                assistant_line(&project, "Yes."),
            ],
        );

        let listed = list_in(&root, &project);
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].title.as_deref(), Some("Still readable"));
        assert_eq!(listed[0].last_text.as_deref(), Some("Yes."));
        assert_eq!(listed[0].messages, 2, "the two whole records still count");
    }

    #[test]
    fn a_folder_that_is_not_there_is_an_empty_list_and_not_a_failure() {
        let project = temp_dir("missing-project");
        let missing = std::env::temp_dir().join("smetana-sessions-no-such-folder");
        let _ = std::fs::remove_dir_all(&missing);
        assert!(list_in(&missing, &project).is_empty());
    }

    #[test]
    fn a_project_with_no_sessions_on_disk_is_an_empty_list() {
        let root = temp_dir("none-root");
        let project = temp_dir("none-project");
        assert!(list_in(&root, &project).is_empty());
    }

    #[test]
    fn the_title_is_the_first_thing_the_person_typed_and_not_what_was_injected() {
        let root = temp_dir("title-root");
        let project = temp_dir("title-project");
        let meta = format!(
            r#"{{"type":"user","isMeta":true,"isSidechain":false,"cwd":"{}","gitBranch":"main","message":{{"role":"user","content":[{{"type":"text","text":"Base directory for this skill: /somewhere"}}]}}}}"#,
            project.display()
        );
        let slash = format!(
            r#"{{"type":"user","isSidechain":false,"cwd":"{}","gitBranch":"main","message":{{"role":"user","content":"<command-name>/clear</command-name>"}}}}"#,
            project.display()
        );
        write_session(
            &root,
            &project,
            "titled",
            &[
                r#"{"type":"mode","mode":"normal"}"#.to_owned(),
                meta,
                slash,
                user_line(&project, "main", "Talk to me in Russian:\n  everything you say"),
                assistant_line(&project, "Understood."),
            ],
        );

        let listed = list_in(&root, &project);
        assert_eq!(
            listed[0].title.as_deref(),
            Some("Talk to me in Russian: everything you say")
        );
    }

    #[test]
    fn the_title_is_the_generated_one_when_the_transcript_carries_one() {
        // The record arrives after the person's first words, the way it does on
        // disk, so this is also the check that the pass does not stop at the
        // human title it found first.
        let root = temp_dir("generated-root");
        let project = temp_dir("generated-project");
        write_session(
            &root,
            &project,
            "generated",
            &[
                user_line(&project, "main", "Talk to me in Russian: everything you say"),
                assistant_line(&project, "Understood."),
                ai_title_line("Task menu in DONE"),
            ],
        );

        assert_eq!(list_in(&root, &project)[0].title.as_deref(), Some("Task menu in DONE"));
    }

    #[test]
    fn a_transcript_with_no_generated_title_is_still_titled_by_the_person() {
        let root = temp_dir("ungenerated-root");
        let project = temp_dir("ungenerated-project");
        ordinary_session(&root, &project, "plain");

        assert_eq!(list_in(&root, &project)[0].title.as_deref(), Some("Move the card to done"));
    }

    #[test]
    fn a_generated_title_of_nothing_falls_back_to_the_person() {
        let root = temp_dir("blank-title-root");
        let project = temp_dir("blank-title-project");
        write_session(
            &root,
            &project,
            "blank",
            &[
                ai_title_line("   "),
                user_line(&project, "main", "Move the card to done"),
                assistant_line(&project, "Moved it."),
            ],
        );

        assert_eq!(list_in(&root, &project)[0].title.as_deref(), Some("Move the card to done"));
    }

    #[test]
    fn a_generated_title_past_the_head_budget_is_not_chased_and_the_person_titles_it() {
        // Which way the budget falls, made mechanical. Nothing on disk here
        // does this — the furthest generated title measured sits at line 88 —
        // but the rule has to have an answer, and it is the answer this gave
        // for every file before that record existed.
        //
        // Past the budget in lines and, with the four enormous records under
        // it, past anything that could be held: a version of this that went
        // looking for the title by reading on would be holding megabytes. What
        // bounds the memory is MAX_LINE and TAIL_WINDOW, never the file's size.
        let root = temp_dir("late-title-root");
        let project = temp_dir("late-title-project");
        let filler = "x".repeat(4 * 1024 * 1024);
        let bulk = format!(
            r#"{{"type":"system","subtype":"note","cwd":"{}","note":"{filler}"}}"#,
            project.display()
        );
        let small = r#"{"type":"mode","mode":"normal"}"#.to_owned();
        let mut lines = vec![user_line(&project, "main", "Move the card to done")];
        lines.extend(std::iter::repeat(small).take(HEAD_LINES + 5));
        lines.extend(std::iter::repeat(bulk).take(4));
        lines.push(ai_title_line("Task menu in DONE"));
        lines.push(assistant_line(&project, "Moved it."));
        let path = write_session(&root, &project, "late", &lines);
        assert!(
            std::fs::metadata(&path).unwrap().len() > 16 * 1024 * 1024,
            "the fixture has to be bigger than anything held in memory"
        );

        let listed = list_in(&root, &project);
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].title.as_deref(), Some("Move the card to done"));
        assert_eq!(listed[0].last_text.as_deref(), Some("Moved it."));
    }

    #[test]
    fn a_session_that_holds_no_human_words_has_no_title_rather_than_a_wrong_one() {
        let root = temp_dir("untitled-root");
        let project = temp_dir("untitled-project");
        let slash = format!(
            r#"{{"type":"user","isSidechain":false,"cwd":"{}","gitBranch":"main","message":{{"role":"user","content":"<command-name>/clear</command-name>"}}}}"#,
            project.display()
        );
        write_session(&root, &project, "empty", &[slash]);

        let listed = list_in(&root, &project);
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].title, None);
    }

    #[test]
    fn the_last_line_skips_back_over_tool_traffic_to_the_last_words() {
        let root = temp_dir("last-root");
        let project = temp_dir("last-project");
        let tool_use = format!(
            r#"{{"type":"assistant","isSidechain":false,"cwd":"{}","message":{{"role":"assistant","model":"claude-opus-5","content":[{{"type":"tool_use","name":"Bash","input":{{}}}}]}}}}"#,
            project.display()
        );
        let tool_result = format!(
            r#"{{"type":"user","isSidechain":false,"cwd":"{}","message":{{"role":"user","content":[{{"type":"tool_result","content":"ok"}}]}}}}"#,
            project.display()
        );
        write_session(
            &root,
            &project,
            "tools",
            &[
                user_line(&project, "main", "Run the tests"),
                assistant_line(&project, "All green."),
                tool_use,
                tool_result,
                r#"{"type":"system","subtype":"note"}"#.to_owned(),
            ],
        );

        let listed = list_in(&root, &project);
        assert_eq!(listed[0].last_role.as_deref(), Some("assistant"));
        assert_eq!(listed[0].last_text.as_deref(), Some("All green."));
        assert_eq!(listed[0].model.as_deref(), Some("claude-opus-5"));
        assert_eq!(listed[0].messages, 4);
    }

    #[test]
    fn subagents_are_counted_from_the_folder_of_them_when_there_is_one() {
        let root = temp_dir("subagents-root");
        let project = temp_dir("subagents-project");
        let path = ordinary_session(&root, &project, "withagents");
        let folder = path.parent().unwrap().join("withagents/subagents");
        std::fs::create_dir_all(&folder).expect("a subagents folder");
        std::fs::write(folder.join("agent-one.jsonl"), "{}\n").unwrap();
        std::fs::write(folder.join("agent-one.meta.json"), "{}\n").unwrap();
        std::fs::write(folder.join("agent-two.jsonl"), "{}\n").unwrap();

        let listed = list_in(&root, &project);
        assert_eq!(listed[0].subagents, 2, "the meta file beside each is not an agent");
    }

    #[test]
    fn a_session_that_ran_none_reports_zero_subagents() {
        let root = temp_dir("nosub-root");
        let project = temp_dir("nosub-project");
        ordinary_session(&root, &project, "plain");
        assert_eq!(list_in(&root, &project)[0].subagents, 0);
    }

    #[test]
    fn an_older_transcript_with_sidechained_turns_still_reports_them() {
        let root = temp_dir("inline-root");
        let project = temp_dir("inline-project");
        let side = format!(
            r#"{{"type":"assistant","isSidechain":true,"cwd":"{}","message":{{"role":"assistant","model":"claude-opus-5","content":[{{"type":"text","text":"searching"}}]}}}}"#,
            project.display()
        );
        write_session(
            &root,
            &project,
            "older",
            &[user_line(&project, "main", "Find it"), side.clone(), side],
        );

        assert_eq!(list_in(&root, &project)[0].subagents, 2);
    }

    /// Unix only, for the same reason [`set_mtime`] under it is: the mtime is
    /// set through `utimes`, and this crate carries `libc` on unix alone.
    #[cfg(unix)]
    #[test]
    fn sessions_are_ordered_by_last_activity_with_the_newest_first() {
        let root = temp_dir("order-root");
        let project = temp_dir("order-project");
        let older = ordinary_session(&root, &project, "older");
        let newer = ordinary_session(&root, &project, "newer");
        // mtime has one-second resolution in the format this hands over, so the
        // two are set apart explicitly rather than by writing them in turn.
        let now = std::time::SystemTime::now();
        set_mtime(&older, now - std::time::Duration::from_secs(3600));
        set_mtime(&newer, now);

        let listed = list_in(&root, &project);
        assert_eq!(
            listed.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(),
            vec!["newer", "older"]
        );
    }

    /// A file's mtime, set through the platform's own call. `filetime` is not a
    /// dependency of this crate and one test does not earn it.
    #[cfg(unix)]
    fn set_mtime(path: &Path, when: std::time::SystemTime) {
        use std::os::unix::ffi::OsStrExt;
        let secs = when
            .duration_since(std::time::UNIX_EPOCH)
            .expect("a time after the epoch")
            .as_secs() as libc::time_t;
        let times = [
            libc::timeval { tv_sec: secs, tv_usec: 0 },
            libc::timeval { tv_sec: secs, tv_usec: 0 },
        ];
        let name = std::ffi::CString::new(path.as_os_str().as_bytes()).expect("a path");
        // SAFETY: both pointers are to memory owned here and outliving the call.
        let ok = unsafe { libc::utimes(name.as_ptr(), times.as_ptr()) };
        assert_eq!(ok, 0, "setting the mtime of a file this test just wrote");
    }

    #[test]
    fn a_huge_transcript_is_summarised_without_being_read_into_memory() {
        // The acceptance criterion, made mechanical. The file is built out of
        // one enormous record — larger than the head window, the tail window
        // and the line cap together — so that anything reading it whole, or
        // holding one line of it, would be holding megabytes. What is asserted
        // is the row, and the bound is the constants: the reader keeps at most
        // MAX_LINE of a line and TAIL_WINDOW of the end.
        let root = temp_dir("huge-root");
        let project = temp_dir("huge-project");
        let filler = "x".repeat(4 * 1024 * 1024);
        let bulk = format!(
            r#"{{"type":"user","isSidechain":false,"cwd":"{}","message":{{"role":"user","content":[{{"type":"tool_result","content":"{filler}"}}]}}}}"#,
            project.display()
        );
        let mut lines = vec![user_line(&project, "main", "Read the big file")];
        for _ in 0..4 {
            lines.push(bulk.clone());
        }
        lines.push(assistant_line(&project, "Done."));
        let path = write_session(&root, &project, "huge", &lines);
        assert!(
            std::fs::metadata(&path).unwrap().len() > 16 * 1024 * 1024,
            "the fixture has to be bigger than anything held in memory"
        );

        let listed = list_in(&root, &project);
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].title.as_deref(), Some("Read the big file"));
        assert_eq!(listed[0].last_text.as_deref(), Some("Done."));
        assert_eq!(listed[0].messages, 6, "an oversized line is still counted");
    }

    #[test]
    fn a_line_longer_than_the_cap_is_read_past_rather_than_held() {
        let long = format!("{}\nshort\n", "y".repeat(MAX_LINE * 3));
        let mut reader = BufReader::new(long.as_bytes());
        let mut buf = Vec::new();

        let first = next_line(&mut reader, &mut buf).unwrap().expect("the long line");
        assert!(first.truncated);
        assert_eq!(buf.len(), MAX_LINE, "the cap is the whole of the memory it costs");

        let second = next_line(&mut reader, &mut buf).unwrap().expect("the line after it");
        assert!(!second.truncated);
        assert_eq!(String::from_utf8_lossy(&buf), "short\n");
        assert!(next_line(&mut reader, &mut buf).unwrap().is_none());
    }

    #[test]
    fn a_file_that_says_nothing_about_its_working_directory_is_left_out() {
        // The head budget is a real edge and this is which way it falls: a file
        // whose first `cwd` is past HEAD_LINES cannot be placed in a project,
        // and a session listed under a project it might not belong to would be
        // worse than one missing from the list.
        let root = temp_dir("placeless-root");
        let project = temp_dir("placeless-project");
        let filler = r#"{"type":"mode","mode":"normal"}"#.to_owned();
        let mut lines = vec![filler; HEAD_LINES + 10];
        lines.push(user_line(&project, "main", "Late to say where it is"));
        write_session(&root, &project, "placeless", &lines);

        assert!(list_in(&root, &project).is_empty());
    }

    /// Not a rule but a stopwatch: where the timing in the module header comes
    /// from, kept so the next person can take the number again rather than
    /// trust this one.
    ///
    /// Ignored, because it reads a real `~/.claude/projects` that a checkout
    /// has no right to assume exists, and because the answer is a duration and
    /// not a pass or a fail. The project to list is named by the environment
    /// rather than written here — a path out of one machine's home directory
    /// is not a fact about this repository:
    ///
    /// ```text
    /// SMETANA_SESSIONS_BENCH=/path/to/project \
    ///   cargo test --release --manifest-path src-tauri/Cargo.toml \
    ///   -- --ignored --nocapture bench_listing
    /// ```
    #[test]
    #[ignore = "a measurement over the real ~/.claude/projects, not a pass or a fail"]
    fn bench_listing_the_real_projects_folder() {
        let Some(project) = std::env::var_os("SMETANA_SESSIONS_BENCH") else {
            println!("SMETANA_SESSIONS_BENCH is not set; nothing measured");
            return;
        };
        let project = PathBuf::from(project);
        let root = projects_root().expect("a HOME to look under");
        // One run to warm the page cache, then five timed, because the number
        // the header carries is what the tab costs on a machine that has just
        // been using it — a cold first read measures the disk, not this code.
        let warm = list_in(&root, &project);
        let bytes: u64 = warm.iter().map(|session| session.size).sum();
        let mut times = Vec::new();
        for _ in 0..5 {
            let started = std::time::Instant::now();
            let listed = list_in(&root, &project);
            times.push(started.elapsed());
            assert_eq!(listed.len(), warm.len(), "the same disk answers the same way twice");
        }
        times.sort();
        // What the list reads like, and not only what it cost: a title repeated
        // down the column is a column nobody can tell apart, which is the whole
        // reason the rule changed.
        let mut seen: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
        let titled = warm.iter().filter(|session| session.title.is_some()).count();
        for title in warm.iter().filter_map(|session| session.title.as_deref()) {
            *seen.entry(title).or_default() += 1;
        }
        println!(
            "{} sessions, {:.0} MB, {} titled, {} distinct, commonest repeated {} times, {:?}..{:?}",
            warm.len(),
            bytes as f64 / 1_000_000.0,
            titled,
            seen.len(),
            seen.values().max().copied().unwrap_or(0),
            times.first().unwrap(),
            times.last().unwrap()
        );
    }

    #[test]
    fn a_file_that_does_not_end_in_a_newline_still_gives_up_its_last_line() {
        let mut reader = BufReader::new("one\ntwo".as_bytes());
        let mut buf = Vec::new();
        assert!(next_line(&mut reader, &mut buf).unwrap().is_some());
        assert!(next_line(&mut reader, &mut buf).unwrap().is_some());
        assert_eq!(String::from_utf8_lossy(&buf), "two");
        assert!(next_line(&mut reader, &mut buf).unwrap().is_none());
    }
}
