//! Keeping `.smetana/` out of the repository.
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

/// What gets written. The trailing slash says "a directory" — it is what git
/// itself writes for one, and it keeps the rule from matching a file that
/// happens to share the name.
const ENTRY: &str = ".smetana/";

/// The comment above it. Somebody will find this line months later in a diff
/// they did not write, and the file is the only place that can tell them who
/// put it there.
const NOTE: &str = "# Smetana";

/// Is this line already about the folder, whatever shape it was written in?
///
/// `.smetana`, `.smetana/`, `/.smetana` and `/.smetana/` all mean the same
/// thing to git. A negation — `!.smetana` — counts as covered too, and that is
/// not an oversight: it can only have been typed on purpose, and appending our
/// line below it would either contradict a deliberate choice or, worse, be
/// overridden by it and leave the file looking like it says two things.
fn mentions_the_folder(line: &str) -> bool {
    let bare = line.trim().trim_start_matches('!').trim_start_matches('/').trim_end_matches('/');
    bare == ".smetana"
}

/// The text a `.gitignore` should hold, given what it holds now — or `None`
/// when it already covers the folder and there is nothing to write. Missing
/// files arrive here as an empty string, which is the same case as an empty
/// one and needs no branch of its own.
pub fn amend(current: &str) -> Option<String> {
    if current.lines().any(mentions_the_folder) {
        return None;
    }

    let mut out = String::with_capacity(current.len() + NOTE.len() + ENTRY.len() + 3);
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
    out.push_str(ENTRY);
    out.push('\n');
    Some(out)
}

/// Put the entry in the project's `.gitignore`, creating the file if there is
/// none. Answers whether anything was written.
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

    #[test]
    fn an_empty_file_gets_the_entry_with_no_leading_blank_line() {
        assert_eq!(amend("").as_deref(), Some("# Smetana\n.smetana/\n"));
    }

    #[test]
    fn an_existing_file_keeps_everything_it_had() {
        let before = "node_modules/\ndist/\n";
        let after = amend(before).expect("the folder was not mentioned");
        assert!(after.starts_with(before), "nothing already in the file may move or vanish");
        assert!(after.ends_with("\n# Smetana\n.smetana/\n"));
    }

    #[test]
    fn a_file_not_ending_in_a_newline_does_not_have_its_last_rule_extended() {
        // "dist/# Smetana" would be one pattern, matching nothing anybody meant.
        let after = amend("node_modules/\ndist/").expect("the folder was not mentioned");
        assert_eq!(after, "node_modules/\ndist/\n\n# Smetana\n.smetana/\n");
    }

    #[test]
    fn every_shape_git_reads_as_the_folder_counts_as_already_there() {
        for line in [".smetana", ".smetana/", "/.smetana", "/.smetana/", "  .smetana/  "] {
            assert_eq!(amend(&format!("node_modules/\n{line}\n")), None, "{line} already covers it");
        }
    }

    #[test]
    fn a_deliberate_negation_is_left_standing() {
        // Only a person types this, and it means the opposite of what we would
        // add. Appending underneath would leave the file arguing with itself.
        assert_eq!(amend("!.smetana\n"), None);
    }

    #[test]
    fn a_name_that_merely_starts_the_same_is_not_it() {
        let after = amend(".smetana-old/\n").expect("that is a different folder");
        assert!(after.ends_with(".smetana/\n"));
    }

    #[test]
    fn running_it_twice_writes_once() {
        let first = amend("node_modules/\n").expect("the folder was not mentioned");
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

        assert!(ensure(&root).expect("write the entry"));
        let written = std::fs::read_to_string(root.join(".gitignore")).expect("read it back");
        assert_eq!(written, "# Smetana\n.smetana/\n");

        assert!(!ensure(&root).expect("second pass"), "already covered, so nothing to write");
        assert_eq!(std::fs::read_to_string(root.join(".gitignore")).expect("read again"), written);

        std::fs::remove_dir_all(&root).expect("remove temp root");
    }
}
