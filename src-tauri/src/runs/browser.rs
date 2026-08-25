//! Whether there is anything on this machine to drive a browser with.
//!
//! A run whose `[live_check].mode` is `browser` opens a browser and works it.
//! Nothing else in the app knew whether that was possible: the config says what
//! the *project* wants and says nothing about the machine the run rides on, so a
//! run with the live check switched on started happily on a laptop with no
//! Playwright and no extension, and found out inside the check, as INFRA.
//!
//! Two tools can drive one, and either is enough:
//!
//! - **Playwright**, which is two facts and not one — an MCP server entry in the
//!   agent's configuration *and* the browsers themselves downloaded. A machine
//!   with the entry and no browsers cannot drive anything, and the two are
//!   reported apart so the tooltip can name which half is missing.
//! - **Claude in Chrome**, an extension in a Chrome profile, found by its id.
//!
//! Everything here is fragile by nature and that is accepted rather than hidden.
//! An extension does not write itself into any agent's configuration, so from
//! outside there is no other way to see it than the directory it unpacks into;
//! the id and the profile layout are Google's and Anthropic's to change. So the
//! ids and path fragments below are named constants with the date and the
//! version they were established at, and **anything unobservable reads as "no",
//! loudly** — the toggle goes off and the tooltip says what was not found,
//! rather than the toggle staying live on a guess.
//!
//! Shaped after `agents/library.rs`: pure functions over file *contents* and
//! directory *listings* carry the tests, and one `detect` does the disk reads
//! and calls them. No worker, for the reason `files/` and `git.rs` have none —
//! four file reads and two directory listings guard no state.
//!
//! **One honest gap, and it is the expensive direction of error.**
//! `PLAYWRIGHT_BROWSERS_PATH` is read from *this process's* environment, and a
//! bundled app on macOS is handed launchd's, never what a person set in
//! `~/.zshrc` — the whole reason `src/shell_env.rs` exists. So somebody who
//! keeps their browsers somewhere else reads here as having none, and the
//! toggle is blocked with a tooltip naming a tool they do have. By this
//! module's own cost model that is the wrong way round: it removes a working
//! feature rather than leaving things where they were. Asking the login shell
//! is what would close it, and that belongs with `shell_env` rather than here.
//! The bug is invisible under `npm run tauri dev` for the same reason it is
//! there — a binary started from a terminal already has the full environment.
//!
//! There is a second reader of these facts, and it is where that gap gets
//! closed rather than only paid for. `render` puts them into the briefing a
//! project-setup session starts with, beside `survey`'s scan of the folder, so
//! an agent writing `[live_check].mode = "browser"` knows whether this machine
//! can honour it — the alternative being a project that stands for months on a
//! mode nothing here can carry out, discovered inside the check, at night. The
//! wrong answer costs less there (an offer to install something already
//! installed, rather than a feature removed), and the skill spends nothing on
//! it: the session runs under the person's own login shell, so it is told to
//! look for itself before it offers, and it sees what this process cannot.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use serde::Serialize;

/// The Claude in Chrome extension's id. Established on 2026-08-09 by reading
/// the manifest under this id in the `Default` profile of a machine that has it
/// installed: `name` is "Claude", version 1.0.84. Known-fragile — an extension
/// republished under a new id reads as absent here, which is the loud "no" this
/// module promises rather than a silent wrong answer.
const EXTENSION_ID: &str = "fcoeoabgfenejglbffodgkkbkcdhcgfn";

/// A Chrome user-data directory holds far more than profiles — `Crashpad`,
/// `GrShaderCache`, `FileTypePolicies` and a dozen more sit beside them — so the
/// profile directories are named rather than walked for. Chrome's own names are
/// `Default` for the first and `Profile 2`, `Profile 3`, … for the rest.
/// Verified against a machine carrying all three on 2026-08-09.
const DEFAULT_PROFILE: &str = "Default";
const PROFILE_PREFIX: &str = "Profile ";

/// What a downloaded browser's directory is called under the Playwright cache:
/// `chromium-1169`, `chromium_headless_shell-1169`, `firefox-1482`,
/// `webkit-2158`. The prefix is the engine and the number is Playwright's own
/// build. Matching by prefix is what keeps `ffmpeg-1011` and the `.links`
/// bookkeeping out of the answer — neither of those drives anything.
const BROWSER_PREFIXES: [&str; 3] = ["chromium", "firefox", "webkit"];

/// The word every Playwright MCP entry carries somewhere. See
/// `entry_is_playwright` for why one word is the whole test.
const PLAYWRIGHT: &str = "playwright";

/// What the machine can drive a browser with, as facts rather than as a verdict.
///
/// A verdict would have to carry the sentence explaining it, and that sentence
/// is UI copy — it belongs on the front end, next to `branchChoice.js`, where a
/// test can reach it. So this is four facts and the front end composes them.
///
/// snake_case on the wire, like `runs::model`: the run payloads already cross
/// this way and one shape in two spellings costs more than one unusual payload.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct BrowserTools {
    /// A Playwright MCP server is in an agent's configuration.
    pub playwright_mcp: bool,
    /// Playwright's browsers are actually on disk. The entry above without this
    /// is a configuration pointing at nothing.
    pub playwright_browsers: bool,
    /// The Claude in Chrome extension is unpacked in some Chrome profile.
    pub extension: bool,
    /// Another project's run is driving a browser right now, named by its path.
    /// `None` is the ordinary case.
    ///
    /// This is the app's own runs and nothing else, which is the whole of what
    /// it can honestly say. **A person's own browser session, or an agent
    /// somebody started by hand, is invisible from here** — and so is the
    /// extension's busy-ness in every form, since a Chrome window holding it is
    /// not a thing this process can see. That gap is recorded rather than
    /// papered over with a check that would be a guess.
    pub busy_project: Option<String>,
}

/// The same four facts as prose, for the briefing a setup session starts with.
///
/// Shaped after `survey::render`, and the two sit side by side in one prompt
/// for the reason they are two functions: that one is prose about the folder,
/// this is prose about the machine the folder will be worked on. It decides
/// nothing — three fields, said one at a time — because a verdict carries the
/// sentence explaining it, and that sentence is UI copy living on the front end
/// where a test can reach it, exactly as `BrowserTools` says above.
///
/// **It keeps no list of its own.** What counts as a way of driving a browser
/// is written once, in `BrowserTools`, and a second copy here is precisely the
/// pair of files with nothing mechanical joining them that this project's own
/// `[merge].hazards` exists to warn about. The struct pattern below is that
/// joint: it names every field and has no `..`, so a fifth fact added to
/// `BrowserTools` stops this file compiling rather than quietly going unsaid in
/// every setup session from that day on.
///
/// `busy_project` is read and deliberately dropped. Busy-ness is one run
/// holding Playwright's single persistent profile at this moment; it says
/// nothing about whether the tool is installed, which is the only question a
/// setup session is asking.
pub fn render(tools: &BrowserTools) -> String {
    let BrowserTools { playwright_mcp, playwright_browsers, extension, busy_project: _ } = tools;

    let mut out = String::from("What this machine can drive a browser with:\n\n");
    for (label, found, present, absent) in [
        (
            "Playwright MCP server",
            *playwright_mcp,
            "found in an agent's configuration",
            "not found in any agent configuration",
        ),
        ("Playwright browsers", *playwright_browsers, "downloaded", "not downloaded"),
        (
            "Claude in Chrome extension",
            *extension,
            "found in a Chrome profile",
            "not found in a Chrome profile",
        ),
    ] {
        out.push_str("- ");
        out.push_str(label);
        out.push_str(": ");
        out.push_str(if found { present } else { absent });
        out.push('\n');
    }
    out.push_str(CAVEAT);
    out
}

/// The last line of the block, and it is load-bearing rather than a hedge.
///
/// The gap at the top of this file arrives in a setup session as a "not found"
/// for a tool that is sitting on the disk. Here it costs a question the person
/// did not need rather than a feature taken away, and the `project-setup` skill
/// closes it from the other side: the session runs under the person's real
/// login shell, so it can see what this process could not, and it is told to
/// look before it offers to install anything.
const CAVEAT: &str = "\nThese were read from the app's own environment and may be incomplete: a bundled\n\
                      app on macOS is handed launchd's environment rather than the login shell's, so\n\
                      browsers kept under a PLAYWRIGHT_BROWSERS_PATH set in a shell profile read here\n\
                      as absent. Check in this session before acting on a \"not found\".\n";

/// Is this MCP server entry Playwright?
///
/// Either the name or what it runs is enough, and that asymmetry is deliberate.
/// The name is conventional — `"playwright"` is what the official instructions
/// tell people to type, and it is what this machine's `~/.claude.json` carries —
/// but it is a person's free choice and somebody's is called `browser`. What it
/// runs is the load-bearing fact (`npx -y @playwright/mcp@latest`, or an `url`
/// for the hosted form), but it can equally be a wrapper script whose path says
/// nothing. Requiring both would call a real install absent; accepting either
/// only mistakes something else for Playwright, and the two costs are not the
/// same size. A false "present" leaves the toggle live and the run finds out
/// inside the check, which is exactly where things were before this module
/// existed. A false "absent" takes a working feature away and puts a tooltip on
/// screen claiming a tool is missing that is sitting right there.
fn entry_is_playwright(name: &str, words: &[String]) -> bool {
    name.to_ascii_lowercase().contains(PLAYWRIGHT)
        || words.iter().any(|word| word.to_ascii_lowercase().contains(PLAYWRIGHT))
}

/// The strings in one JSON entry worth reading: what it runs, and — for the
/// `http`/`sse` forms, which have no command at all — where it points.
fn json_words(entry: &serde_json::Value) -> Vec<String> {
    let mut words = Vec::new();
    for key in ["command", "url"] {
        if let Some(text) = entry.get(key).and_then(|value| value.as_str()) {
            words.push(text.to_string());
        }
    }
    if let Some(args) = entry.get("args").and_then(|value| value.as_array()) {
        words.extend(args.iter().filter_map(|value| value.as_str()).map(str::to_string));
    }
    words
}

/// Any Playwright entry in a `{ "<name>": { … } }` server map.
fn json_map_has_playwright(map: Option<&serde_json::Value>) -> bool {
    map.and_then(|value| value.as_object())
        .is_some_and(|servers| {
            servers.iter().any(|(name, entry)| entry_is_playwright(name, &json_words(entry)))
        })
}

/// `~/.claude.json`, which keeps servers in two places: the root map, which
/// every project sees, and a per-project override under
/// `projects["<absolute path>"].mcpServers`. Both count — a project that
/// configured Playwright for itself alone can drive a browser just as well as
/// one relying on the root map, and reading only the root would call it absent.
///
/// Anything unparseable answers "no", the way `library.rs` does and for the same
/// reason spelled out at the top of this file.
pub fn claude_json_has_playwright(text: &str, project: &str) -> bool {
    let Ok(root) = serde_json::from_str::<serde_json::Value>(text) else {
        return false;
    };
    let per_project = root.get("projects").and_then(|projects| projects.get(project));
    json_map_has_playwright(root.get("mcpServers"))
        || json_map_has_playwright(per_project.and_then(|entry| entry.get("mcpServers")))
}

/// A project's own `.mcp.json`, the standard `{ "mcpServers": { … } }` shape.
/// Most projects have none, and that is an ordinary outcome rather than a
/// failure — the answer is simply "no" and the other two sources still speak.
pub fn mcp_json_has_playwright(text: &str) -> bool {
    let Ok(root) = serde_json::from_str::<serde_json::Value>(text) else {
        return false;
    };
    json_map_has_playwright(root.get("mcpServers"))
}

/// `~/.codex/config.toml`, where the same servers live under
/// `[mcp_servers.<name>]` — Codex's spelling of the same idea.
pub fn codex_config_has_playwright(text: &str) -> bool {
    let Ok(root) = toml::from_str::<toml::Value>(text) else {
        return false;
    };
    let Some(servers) = root.get("mcp_servers").and_then(|value| value.as_table()) else {
        return false;
    };
    servers.iter().any(|(name, entry)| {
        let mut words = Vec::new();
        for key in ["command", "url"] {
            if let Some(text) = entry.get(key).and_then(|value| value.as_str()) {
                words.push(text.to_string());
            }
        }
        if let Some(args) = entry.get("args").and_then(|value| value.as_array()) {
            words.extend(args.iter().filter_map(|value| value.as_str()).map(str::to_string));
        }
        entry_is_playwright(name, &words)
    })
}

/// Has anything drivable actually been downloaded into the Playwright cache?
/// The listing is the whole input, so the test needs no directory.
pub fn browsers_downloaded(entries: &[String]) -> bool {
    entries
        .iter()
        .any(|name| BROWSER_PREFIXES.iter().any(|prefix| name.starts_with(prefix)))
}

/// Is this entry in Chrome's user-data directory one of its profiles? See
/// `DEFAULT_PROFILE` for why the set is named rather than walked.
pub fn is_chrome_profile(name: &str) -> bool {
    name == DEFAULT_PROFILE
        || name
            .strip_prefix(PROFILE_PREFIX)
            .is_some_and(|rest| !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit()))
}

/// Is the extension unpacked in any of these profiles?
///
/// The listing goes in and the one disk question comes in as `unpacked`, which
/// is the shape `browsers_downloaded` already has and for the same reason: the
/// fragile part of this answer is not `is_dir` but the two things around it —
/// which entries count as profiles, and the fact that only a *profile* is asked
/// at all. A Chrome user-data directory is mostly not profiles, so walking it
/// whole would look for the extension under `Crashpad`, and asking every entry
/// would have quietly worked on this machine while going wrong on a fresh one.
/// Injecting the predicate is what lets a test pin both without a Chrome
/// installation to point at.
pub fn extension_in_profiles(profiles: &[String], mut unpacked: impl FnMut(&str) -> bool) -> bool {
    // `FnMut`, the way `Iterator::any` takes its own predicate: it costs the
    // caller nothing and it is what lets the test record which names were asked,
    // which is half of what there is to pin here.
    profiles.iter().filter(|name| is_chrome_profile(name)).any(|name| unpacked(name))
}

/// Where Playwright keeps its browsers.
///
/// `configured` is `PLAYWRIGHT_BROWSERS_PATH`, taken as an argument rather than
/// read here so the branch below is reachable from a test without mutating the
/// process environment — which `cargo test`'s threads share.
///
/// It wins when it is set, and its one non-path value is handled by name: `0`
/// means the browsers live inside the npm package rather than in a shared cache,
/// which is a directory nothing here can name without knowing which
/// `node_modules` the agent will use. Unobservable, so "no".
///
/// The three platform defaults are Playwright's own. macOS is the one this was
/// established against; the other two are written from Playwright's documented
/// layout and have not been run.
fn playwright_cache_dir(home: &Path, configured: Option<&OsStr>) -> Option<PathBuf> {
    if let Some(explicit) = configured {
        if explicit == "0" {
            return None;
        }
        return Some(PathBuf::from(explicit));
    }
    Some(if cfg!(target_os = "macos") {
        home.join("Library/Caches/ms-playwright")
    } else if cfg!(target_os = "windows") {
        std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join("AppData/Local"))
            .join("ms-playwright")
    } else {
        home.join(".cache/ms-playwright")
    })
}

/// Where Chrome keeps its profiles. Same caveat as above: macOS is measured, the
/// other two are Chrome's documented layout.
fn chrome_user_data_dir(home: &Path) -> PathBuf {
    if cfg!(target_os = "macos") {
        home.join("Library/Application Support/Google/Chrome")
    } else if cfg!(target_os = "windows") {
        std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join("AppData/Local"))
            .join("Google/Chrome/User Data")
    } else {
        home.join(".config/google-chrome")
    }
}

/// The names in a directory, or an empty list. A directory that is not there and
/// one that cannot be read are the same answer here, which is the "no" rule
/// again: neither says a browser is available.
fn entries_of(dir: &Path) -> Vec<String> {
    let Ok(listing) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    listing
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect()
}

fn read(path: PathBuf) -> String {
    std::fs::read_to_string(path).unwrap_or_default()
}

/// The disk reads, and the only impure thing here.
///
/// All three MCP sources are consulted whatever agent is configured, and that is
/// a simplification worth knowing about: a machine with Playwright set up for
/// Claude Code and a run that will go out under Codex reads as available. It
/// errs toward the live toggle, which is the direction `entry_is_playwright`
/// already chose and for the same reason — the cost is a check that fails the
/// way it did before this module, against a feature removed with a tooltip that
/// is wrong.
pub fn detect(project: &Path, busy_project: Option<String>) -> BrowserTools {
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from);

    let Some(home) = home else {
        // No home directory means none of the four paths below can even be
        // named. Loudly "no", by the rule at the top of this file.
        return BrowserTools { busy_project, ..BrowserTools::default() };
    };

    let project_key = project.to_string_lossy();
    let playwright_mcp = claude_json_has_playwright(&read(home.join(".claude.json")), &project_key)
        || mcp_json_has_playwright(&read(project.join(".mcp.json")))
        || codex_config_has_playwright(&read(home.join(".codex/config.toml")));

    let configured = std::env::var_os("PLAYWRIGHT_BROWSERS_PATH");
    let playwright_browsers = playwright_cache_dir(&home, configured.as_deref())
        .is_some_and(|cache| browsers_downloaded(&entries_of(&cache)));

    let chrome = chrome_user_data_dir(&home);
    let extension = extension_in_profiles(&entries_of(&chrome), |profile| {
        chrome.join(profile).join("Extensions").join(EXTENSION_ID).is_dir()
    });

    BrowserTools { playwright_mcp, playwright_browsers, extension, busy_project }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The real entry off this machine's `~/.claude.json`, copied verbatim
    /// rather than minimised: what has to be pinned is the shape a person's file
    /// actually has, and a hand-tidied one would only agree with itself.
    const CLAUDE_JSON: &str = r#"{
        "mcpServers": {
            "context7": { "type": "http", "url": "https://mcp.context7.com/mcp" },
            "playwright": {
                "type": "stdio",
                "command": "npx",
                "args": ["-y", "@playwright/mcp@latest"],
                "env": {}
            }
        },
        "projects": {
            "/Users/someone/other": { "mcpServers": { "puppeteer": { "command": "npx" } } }
        }
    }"#;

    #[test]
    fn the_root_server_map_is_read() {
        assert!(claude_json_has_playwright(CLAUDE_JSON, "/Users/someone/project"));
    }

    #[test]
    fn a_configuration_without_it_is_not_a_playwright_machine() {
        let json = r#"{"mcpServers":{"puppeteer":{"command":"npx","args":["-y","puppeteer-mcp"]}}}"#;
        assert!(!claude_json_has_playwright(json, "/p"));
    }

    #[test]
    fn a_per_project_override_counts_as_much_as_the_root_map() {
        // Configured for one project alone, which drives a browser exactly as
        // well as the root map does. Reading only the root would call it absent.
        let json = r#"{
            "mcpServers": {},
            "projects": {
                "/Users/someone/project": {
                    "mcpServers": { "playwright": { "command": "npx" } }
                }
            }
        }"#;
        assert!(claude_json_has_playwright(json, "/Users/someone/project"));
        assert!(!claude_json_has_playwright(json, "/Users/someone/elsewhere"));
    }

    #[test]
    fn an_entry_named_anything_is_read_by_what_it_runs() {
        // The name is a person's free choice; the command is what actually runs.
        let json = r#"{"mcpServers":{"browser":{"command":"npx","args":["-y","@playwright/mcp@latest"]}}}"#;
        assert!(mcp_json_has_playwright(json));
    }

    #[test]
    fn the_hosted_form_is_read_by_its_url() {
        // No command at all — the http and sse forms carry a url instead, and
        // dropping them would call a working remote server absent.
        let json = r#"{"mcpServers":{"browser":{"type":"http","url":"https://example.test/playwright/mcp"}}}"#;
        assert!(mcp_json_has_playwright(json));
    }

    #[test]
    fn rubbish_and_absence_both_read_as_no() {
        // `read` hands an unreadable file over as an empty string, so this is
        // also the answer for a project with no .mcp.json — the ordinary case.
        assert!(!mcp_json_has_playwright(""));
        assert!(!mcp_json_has_playwright("not json at all"));
        assert!(!mcp_json_has_playwright("{}"));
        assert!(!mcp_json_has_playwright(r#"{"mcpServers":[]}"#));
        assert!(!claude_json_has_playwright("", "/p"));
        assert!(!codex_config_has_playwright("not toml ["));
    }

    #[test]
    fn codex_keeps_the_same_servers_under_its_own_spelling() {
        let config = r#"
model = "gpt-5"

[mcp_servers.playwright]
command = "npx"
args = ["-y", "@playwright/mcp@latest"]

[mcp_servers.node_repl]
command = "node"
"#;
        assert!(codex_config_has_playwright(config));
    }

    #[test]
    fn a_codex_config_with_other_servers_only_is_not_a_playwright_machine() {
        let config = "[mcp_servers.node_repl]\ncommand = \"node\"\n";
        assert!(!codex_config_has_playwright(config));
        assert!(!codex_config_has_playwright("model = \"gpt-5\"\n"));
    }

    #[test]
    fn a_configured_server_with_nothing_downloaded_drives_nothing() {
        // The two halves of "Playwright is available" are separate facts on
        // purpose: this listing is a cache that has been created and emptied,
        // and the entry in the configuration above points at nothing.
        assert!(!browsers_downloaded(&[]));
        assert!(!browsers_downloaded(&[".links".into(), "ffmpeg-1011".into()]));
    }

    #[test]
    fn a_downloaded_engine_is_a_browser_whichever_one_it_is() {
        // Verbatim off this machine's cache on 2026-08-09.
        let listing: Vec<String> = ["chromium-1169", "chromium_headless_shell-1169", "ffmpeg-1011", "firefox-1482", "webkit-2158"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert!(browsers_downloaded(&listing));
        assert!(browsers_downloaded(&["firefox-1482".to_string()]));
        assert!(browsers_downloaded(&["webkit-2158".to_string()]));
        // The headless shell alone still drives a browser.
        assert!(browsers_downloaded(&["chromium_headless_shell-1169".to_string()]));
    }

    #[test]
    fn chromes_profiles_are_told_apart_from_everything_else_beside_them() {
        assert!(is_chrome_profile("Default"));
        assert!(is_chrome_profile("Profile 2"));
        assert!(is_chrome_profile("Profile 13"));
        // All real neighbours of those three in a Chrome user-data directory.
        assert!(!is_chrome_profile("Crashpad"));
        assert!(!is_chrome_profile("Guest Profile"));
        assert!(!is_chrome_profile("GrShaderCache"));
        assert!(!is_chrome_profile("Profile"));
        assert!(!is_chrome_profile("Profile "));
        assert!(!is_chrome_profile("Profile X"));
    }

    #[test]
    fn the_extension_is_looked_for_in_profiles_and_nowhere_else() {
        // Verbatim neighbours from a real Chrome user-data directory. If the
        // walk ever stops filtering, `Crashpad` gets asked and this fails.
        let listing: Vec<String> =
            ["Crashpad", "Default", "GrShaderCache", "Profile 2", "Guest Profile"]
                .iter()
                .map(|s| s.to_string())
                .collect();

        let mut asked: Vec<String> = Vec::new();
        let found = extension_in_profiles(&listing, |profile| {
            asked.push(profile.to_string());
            profile == "Profile 2"
        });
        assert!(found, "the extension in any one profile is enough");
        assert_eq!(asked, ["Default", "Profile 2"], "only profiles are ever asked");
    }

    #[test]
    fn no_profile_carrying_it_is_a_no_rather_than_a_maybe() {
        let listing = vec!["Default".to_string(), "Profile 2".to_string()];
        assert!(!extension_in_profiles(&listing, |_| false));
        // A user-data directory with no profiles at all — or none this reader
        // recognises — is the same "no", loudly, by the rule at the top.
        assert!(!extension_in_profiles(&["Crashpad".to_string()], |_| true));
        assert!(!extension_in_profiles(&[], |_| true));
    }

    #[test]
    fn a_configured_browsers_path_wins_over_the_platform_default() {
        let home = PathBuf::from("/home/someone");
        let configured = std::ffi::OsString::from("/opt/pw-browsers");
        assert_eq!(
            playwright_cache_dir(&home, Some(configured.as_os_str())),
            Some(PathBuf::from("/opt/pw-browsers"))
        );
        // Unset falls to the platform default, which is under the home given.
        let fallback = playwright_cache_dir(&home, None).expect("a default exists on every platform");
        assert!(fallback.starts_with(&home) || cfg!(target_os = "windows"), "{fallback:?}");
        assert!(fallback.ends_with("ms-playwright"), "{fallback:?}");
    }

    #[test]
    fn browsers_kept_inside_the_package_are_unobservable_and_so_read_as_none() {
        // `PLAYWRIGHT_BROWSERS_PATH=0` puts them in whichever node_modules the
        // agent happens to use, which is not a directory this can name. The
        // rule at the top of the file says what that means: "no", loudly.
        let home = PathBuf::from("/home/someone");
        assert_eq!(playwright_cache_dir(&home, Some(std::ffi::OsStr::new("0"))), None);
    }

    #[test]
    fn a_machine_with_nothing_on_it_names_all_three_tools_as_missing() {
        // What a setup session on a fresh laptop is told. Each tool is named
        // apart from the others because the work of fixing them differs: an
        // MCP entry is written into a configuration, browsers are downloaded,
        // and the extension is installed in Chrome.
        let text = render(&BrowserTools::default());
        assert!(text.contains("Playwright MCP server: not found in any agent configuration"), "{text}");
        assert!(text.contains("Playwright browsers: not downloaded"), "{text}");
        assert!(
            text.contains("Claude in Chrome extension: not found in a Chrome profile"),
            "{text}"
        );
        // The caveat is the reason the agent is told to look for itself before
        // offering to install anything, so it is here in every case.
        assert!(text.contains("may be incomplete"), "{text}");
        assert!(text.contains("PLAYWRIGHT_BROWSERS_PATH"), "{text}");
    }

    #[test]
    fn a_machine_with_everything_on_it_reads_as_prose_rather_than_an_empty_list() {
        // The failure this pins is a block that only ever says what is missing:
        // on a machine that has everything it would come out as a heading with
        // nothing under it, which reads as a prompt that was cut short.
        let text = render(&BrowserTools {
            playwright_mcp: true,
            playwright_browsers: true,
            extension: true,
            busy_project: Some("/Users/someone/other".into()),
        });
        assert!(text.contains("Playwright MCP server: found in an agent's configuration"), "{text}");
        assert!(text.contains("Playwright browsers: downloaded"), "{text}");
        assert!(text.contains("Claude in Chrome extension: found in a Chrome profile"), "{text}");
        // Not a bare "not found": the caveat's own last sentence carries that
        // phrase in quotes, and the three lines are what this is about.
        assert!(!text.contains(": not "), "nothing is missing on this machine: {text}");
        // Busy-ness is another run holding the profile right now and has
        // nothing to do with what is installed, so it stays out of the block —
        // and out of the prompt of a session that will outlive that run.
        assert!(!text.contains("/Users/someone/other"), "{text}");
    }

    #[test]
    fn detect_on_a_folder_with_nothing_in_it_carries_the_busy_answer_through() {
        // Whatever the machine turns out to have, busy-ness comes from the run
        // worker and passes through untouched — the one fact here that is not
        // read off a disk.
        let root = std::env::temp_dir().join(format!(
            "smetana-browser-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock is after the Unix epoch")
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).expect("create the fixture root");
        let tools = detect(&root, Some("/Users/someone/other".into()));
        assert_eq!(tools.busy_project.as_deref(), Some("/Users/someone/other"));
        std::fs::remove_dir_all(&root).expect("clean up");
    }
}
