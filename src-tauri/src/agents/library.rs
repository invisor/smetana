//! Where the bundled skills are, and whether the person already has their own
//! copy of superpowers.

use std::path::{Path, PathBuf};

use tauri::{AppHandle, Manager};

/// The skill library as this run sees it.
pub struct Skills {
    /// Our own skills. Always ours, always present.
    pub smetana: PathBuf,
    /// The vendored superpowers copy. Always a real path: `Inline` and `Auto`
    /// read from it regardless of what is installed.
    pub superpowers: PathBuf,
    /// True when the person has their own superpowers, in which case ours must
    /// not also be handed over.
    pub superpowers_installed: bool,
}

/// The name a marketplace-installed plugin gets in `installed_plugins.json`
/// is `<plugin>@<marketplace>`, so the separator is part of what we match:
/// without it `superpowers-extra` would read as the real thing.
const KEY_PREFIX: &str = "superpowers@";

/// A key in that file maps to the **list** of scoped installs behind it, and an
/// empty list is a plugin that is known about and installed nowhere. Reading
/// the key alone would call that person equipped and withhold our copy from
/// them — the exact failure the comment below says this function is shaped to
/// avoid. Anything that is not a list at all is read the same way, for the same
/// reason: of the two ways to be wrong, this is the cheap one.
fn is_installed_somewhere(scopes: &serde_json::Value) -> bool {
    scopes.as_array().is_some_and(|list| !list.is_empty())
}

/// Does the person already have their own superpowers? Reading a file costs
/// nothing and spawns no process — the same reasoning that keeps `files/` and
/// `git.rs` out of a worker.
///
/// Anything unreadable answers "no". Handing a second copy to someone who has
/// one costs a duplicate entry in a list; withholding it from someone who has
/// none removes the feature with nothing on screen to say so.
pub fn has_superpowers(installed_plugins_json: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(installed_plugins_json)
        .ok()
        .and_then(|value| value.get("plugins")?.as_object().cloned())
        .is_some_and(|plugins| {
            plugins
                .iter()
                .any(|(key, scopes)| key.starts_with(KEY_PREFIX) && is_installed_somewhere(scopes))
        })
}

/// The skill text, for the harnesses that can only take it in the prompt.
/// A missing file is an ordinary outcome — the prompt then carries the rule
/// without the full process, which is why the rule has to stand on its own.
pub fn read_skill(root: &Path, name: &str) -> Option<String> {
    std::fs::read_to_string(root.join("skills").join(name).join("SKILL.md")).ok()
}

/// Where everything is for this run.
pub fn resolve(app: &AppHandle) -> Skills {
    let resources = app.path().resource_dir().unwrap_or_else(|_| PathBuf::from("."));
    let installed = claude_plugins_file()
        .and_then(|path| std::fs::read_to_string(path).ok())
        .is_some_and(|json| has_superpowers(&json));

    Skills {
        smetana: resources.join("resources/smetana"),
        superpowers: resources.join("resources/superpowers"),
        superpowers_installed: installed,
    }
}

fn claude_plugins_file() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .map(|home| home.join(".claude/plugins/installed_plugins.json"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_plugin_map_naming_superpowers_means_the_person_has_their_own() {
        let json = r#"{"version":2,"plugins":{
            "frontend-design@claude-plugins-official":[],
            "superpowers@claude-plugins-official":[{"scope":"user"}]
        }}"#;
        assert!(has_superpowers(json));
    }

    #[test]
    fn a_plugin_map_without_it_does_not() {
        let json = r#"{"version":2,"plugins":{"frontend-design@official":[]}}"#;
        assert!(!has_superpowers(json));
    }

    #[test]
    fn a_plugin_whose_name_merely_starts_the_same_is_not_it() {
        let json = r#"{"version":2,"plugins":{"superpowers-extra@official":[]}}"#;
        assert!(!has_superpowers(json), "the marketplace separator is part of the name");
    }

    #[test]
    fn the_name_with_nothing_behind_it_is_not_an_install() {
        // The key survives an uninstall with its scope list emptied. Reading
        // that as "they have their own" would take our copy away from someone
        // who has none — the one failure this function exists to prevent.
        let json = r#"{"version":2,"plugins":{"superpowers@claude-plugins-official":[]}}"#;
        assert!(!has_superpowers(json), "an empty scope list is nothing installed");
    }

    #[test]
    fn rubbish_reads_as_not_installed() {
        // Withholding our copy from someone who has none removes the feature
        // silently; handing over a second copy to someone who has one is a
        // duplicate in a list. Of the two ways to be wrong, this is the cheap one.
        assert!(!has_superpowers("not json at all"));
        assert!(!has_superpowers("{}"));
        assert!(!has_superpowers(r#"{"plugins":[]}"#));
    }

    #[test]
    fn read_skill_joins_root_and_name_under_a_shared_skills_directory() {
        // Both roots `resolve` hands out nest each skill under `skills/<name>/SKILL.md`
        // — the vendored superpowers copy genuinely does, and `smetana` is laid out
        // to match it. This test fails if the `skills` segment is dropped from the join.
        let root = std::env::temp_dir().join(format!(
            "smetana-library-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock is after the Unix epoch")
                .as_nanos()
        ));
        let skill_dir = root.join("skills").join("filing-a-task");
        std::fs::create_dir_all(&skill_dir).expect("create fake skill directory");
        std::fs::write(skill_dir.join("SKILL.md"), "filing-a-task content")
            .expect("write fake SKILL.md");

        assert_eq!(read_skill(&root, "filing-a-task"), Some("filing-a-task content".to_string()));
        assert_eq!(read_skill(&root, "no-such-skill"), None);

        std::fs::remove_dir_all(&root).expect("remove temp dir for the fake skill root");
    }
}
