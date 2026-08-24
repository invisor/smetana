//! Keeping what this app creates out of somebody else's repository.
//!
//! Two things so far: `.smetana/`, which a setup session and every run write
//! into, and `.beads.backup-*/`, the copy a tracker repair takes beside
//! `.beads` and never removes.
//!
//! Not the setting-up agent's job, and deliberately not written into its skill:
//! an instruction in prose is a thing that can be followed, argued with or
//! quietly skipped, and this one was all three — an agent reading a
//! `.gitignore` whose neighbouring lines hide the tracker and the docs will
//! reasonably conclude the folder belongs there too, or reasonably conclude the
//! opposite, and either way the answer differs from project to project. The
//! app decides once, in code, and the decision is the same everywhere.
//!
//! The pure half is `amend`, which is where the tests are; `ensure` is the disk
//! and does nothing a person did not already ask for by setting the project up.

use std::io;
use std::path::Path;

/// What gets written. The trailing slash on each says "a directory" — it is
/// what git itself writes for one, and it keeps the rule from matching a file
/// that happens to share the name.
///
/// `.beads.backup-*/` is a glob because the copies are named for the moment
/// they were taken, to the second, so every repair makes a new one rather than
/// colliding with the last. `.beads` being ignored does not cover them: git
/// reads that as one name, and `.beads.backup-20260825T123000Z` is not it.
/// Left out, an 84 MB Dolt copy becomes a permanent untracked row in the Git
/// panel — and `vcs/commands.rs` hands the untracked list to
/// `oneshot::commit_prompt`, so an agent asked to commit would be shown it with
/// nothing anywhere telling it to leave that alone. The `REPAIR` prompt says so
/// to a repair session, which is not the session that commits.
///
/// Each is looked for separately, so a project that already has one gets the
/// other appended under its own heading.
const ENTRIES: [&str; 2] = [".smetana/", ".beads.backup-*/"];

/// The comment above it. Somebody will find this line months later in a diff
/// they did not write, and the file is the only place that can tell them who
/// put it there.
const NOTE: &str = "# Smetana";

/// The pattern inside a line, with everything git treats as decoration taken
/// off: `.smetana`, `.smetana/`, `/.smetana` and `/.smetana/` all mean the same
/// thing to it.
fn bare(line: &str) -> &str {
    line.trim().trim_start_matches('!').trim_start_matches('/').trim_end_matches('/')
}

/// Is this line already about that entry, whatever shape it was written in?
///
/// A negation — `!.smetana` — counts as covered, and that is not an oversight:
/// it can only have been typed on purpose, and appending our line below it
/// would either contradict a deliberate choice or, worse, be overridden by it
/// and leave the file looking like it says two things. Per entry, so a
/// negation of one says nothing about the other.
fn mentions(line: &str, entry: &str) -> bool {
    bare(line) == bare(entry)
}

/// The text a `.gitignore` should hold, given what it holds now — or `None`
/// when it already covers everything and there is nothing to write. Missing
/// files arrive here as an empty string, which is the same case as an empty
/// one and needs no branch of its own.
pub fn amend(current: &str) -> Option<String> {
    let missing: Vec<&str> = ENTRIES
        .iter()
        .copied()
        .filter(|entry| !current.lines().any(|line| mentions(line, entry)))
        .collect();
    if missing.is_empty() {
        return None;
    }

    let mut out = String::with_capacity(current.len() + NOTE.len() + 32);
    out.push_str(current);
    // A file that does not end in a newline would otherwise take the comment
    // onto the end of its last rule, changing what that rule matches.
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    // A blank line before the comment, but not at the top of a file we are
    // creating from nothing.
    if !out.is_empty() {
        out.push('\n');
    }
    out.push_str(NOTE);
    out.push('\n');
    for entry in missing {
        out.push_str(entry);
        out.push('\n');
    }
    Some(out)
}

/// Put the entries in the project's `.gitignore`, creating the file if there is
/// none. Answers whether anything was written.
///
/// Called from two places, and both are "before the thing exists": the terminal
/// worker runs it when a session starts, and the tracker worker runs it before
/// a repair takes its copy — so neither directory is ever untracked, not even
/// for the second in between.
///
/// A root that is not a git repository is left alone, and that is the whole of
/// the multi-repository case: there the folder holding `.beads/` and
/// `.smetana/` is usually not under git at all, so there is no `.gitignore`
/// that would mean anything and nothing to keep out of anything. Each
/// repository below it tracks only itself and never sees the folder.
pub fn ensure(root: &Path) -> io::Result<bool> {
    if !root.join(".git").exists() {
        return Ok(false);
    }
    let path = root.join(".gitignore");
    let current = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(err) if err.kind() == io::ErrorKind::NotFound => String::new(),
        Err(err) => return Err(err),
    };
    match amend(&current) {
        Some(next) => std::fs::write(&path, next).map(|()| true),
        None => Ok(false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Both entries, under one heading, in the order `ENTRIES` lists them.
    const BLOCK: &str = "# Smetana\n.smetana/\n.beads.backup-*/\n";

    #[test]
    fn an_empty_file_gets_the_entries_with_no_leading_blank_line() {
        assert_eq!(amend("").as_deref(), Some(BLOCK));
    }

    #[test]
    fn an_existing_file_keeps_everything_it_had() {
        let before = "node_modules/\ndist/\n";
        let after = amend(before).expect("neither was mentioned");
        assert!(after.starts_with(before), "nothing already in the file may move or vanish");
        assert!(after.ends_with(&format!("\n{BLOCK}")));
    }

    #[test]
    fn a_file_not_ending_in_a_newline_does_not_have_its_last_rule_extended() {
        // "dist/# Smetana" would be one pattern, matching nothing anybody meant.
        let after = amend("node_modules/\ndist/").expect("neither was mentioned");
        assert_eq!(after, format!("node_modules/\ndist/\n\n{BLOCK}"));
    }

    #[test]
    fn every_shape_git_reads_as_an_entry_counts_as_already_there() {
        for (line, entry) in [
            (".smetana", ".smetana/"),
            (".smetana/", ".smetana/"),
            ("/.smetana", ".smetana/"),
            ("/.smetana/", ".smetana/"),
            ("  .smetana/  ", ".smetana/"),
            (".beads.backup-*", ".beads.backup-*/"),
            ("/.beads.backup-*/", ".beads.backup-*/"),
        ] {
            assert!(mentions(line, entry), "{line} already covers {entry}");
        }
    }

    /// The entries are looked for one at a time, so a file that has grown one
    /// of them gets the other appended and the first is not written twice.
    #[test]
    fn a_file_that_already_has_one_entry_gets_only_the_other() {
        let after = amend("node_modules/\n.smetana/\n").expect("the backups were not mentioned");
        assert_eq!(after, "node_modules/\n.smetana/\n\n# Smetana\n.beads.backup-*/\n");
        assert_eq!(after.matches(".smetana/").count(), 1, "the one it had is not repeated");
    }

    /// The backup copies are the reason this is a glob: every repair names one
    /// for the second it was taken in, so a literal would cover exactly one of
    /// them, and `.beads` covers none — git reads that as a different name.
    #[test]
    fn ignoring_the_tracker_does_not_count_as_ignoring_its_copies() {
        let after = amend(".beads\n.smetana/\n").expect("the copies are a different pattern");
        assert!(after.ends_with("# Smetana\n.beads.backup-*/\n"), "{after}");
    }

    #[test]
    fn a_deliberate_negation_is_left_standing() {
        // Only a person types this, and it means the opposite of what we would
        // add. Appending underneath would leave the file arguing with itself.
        // Per entry: the negation says nothing about the other one, which is
        // still appended.
        let after = amend("!.smetana\n").expect("the copies were not mentioned");
        assert!(!after.contains("\n.smetana/"), "the negated entry is not written after it");
        assert!(after.ends_with(".beads.backup-*/\n"));
    }

    #[test]
    fn a_name_that_merely_starts_the_same_is_not_it() {
        let after = amend(".smetana-old/\n").expect("that is a different folder");
        assert!(after.contains("\n.smetana/\n"));
    }

    #[test]
    fn running_it_twice_writes_once() {
        let first = amend("node_modules/\n").expect("neither was mentioned");
        assert_eq!(amend(&first), None, "a second pass must not stack a second copy");
    }

    #[test]
    fn a_root_outside_git_is_left_alone() {
        // The multi-repository case: nothing up here is tracked, so there is no
        // file to amend and creating one would be litter.
        let root = std::env::temp_dir().join(format!("smetana-gitignore-test-{}", std::process::id()));
        std::fs::create_dir_all(&root).expect("create temp root");
        assert!(!ensure(&root).expect("a folder outside git is not an error"));
        assert!(!root.join(".gitignore").exists(), "nothing may be created outside a repository");
        std::fs::remove_dir_all(&root).expect("remove temp root");
    }

    #[test]
    fn a_repository_gets_the_file_written_once() {
        let root = std::env::temp_dir().join(format!("smetana-gitignore-repo-{}", std::process::id()));
        std::fs::create_dir_all(root.join(".git")).expect("create fake repository");

        assert!(ensure(&root).expect("write the entries"));
        let written = std::fs::read_to_string(root.join(".gitignore")).expect("read it back");
        assert_eq!(written, BLOCK);

        assert!(!ensure(&root).expect("second pass"), "already covered, so nothing to write");
        assert_eq!(std::fs::read_to_string(root.join(".gitignore")).expect("read again"), written);

        std::fs::remove_dir_all(&root).expect("remove temp root");
    }
}
