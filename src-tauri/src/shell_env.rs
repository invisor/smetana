//! The `PATH` and the locale a child process should actually be started with.
//!
//! A GUI application on macOS inherits launchd's environment, not the person's:
//! `open smetana.app` gives the process whatever `launchctl getenv PATH` says,
//! which on a stock machine is nothing at all, so the child falls back to
//! `/usr/bin:/bin:/usr/sbin:/sbin`. Everything a developer installs —
//! `~/.local/bin`, `/opt/homebrew/bin`, nvm's shims — reaches `PATH` from
//! `~/.zshrc` or `~/.zprofile`, which only a shell ever reads. The result is an
//! app that cannot find `claude` or `codex` on a machine where both are
//! installed and on `PATH` in every terminal window.
//!
//! The same environment is missing the same way in a second place, and it costs
//! more quietly: it carries no `LC_ALL`, no `LC_CTYPE` and no `LANG` either, so
//! a child started from it runs in the C locale. On macOS a process with no
//! usable locale takes its default C-string encoding from
//! `CFStringGetSystemEncoding()`, which answers MacRoman — and the system's own
//! command line tools then read UTF-8 bytes back as MacRoman. Handed the UTF-8
//! bytes of a Russian greeting that way, `pbcopy` puts `–ü—Ä–∏–≤–µ—Ç` on the
//! clipboard, losslessly, with nothing anywhere reporting a failure. That is not
//! hypothetical: it is how the text an agent had drawn correctly in its own pane
//! reached a person's clipboard as mojibake, because a coding agent copies a
//! mouse selection by running `pbcopy` itself. One login shell answers both
//! questions, so both are read out of the same call.
//!
//! It is invisible in development, which is what makes it worth a module rather
//! than a line: `npm run tauri dev` starts the binary from a terminal, so the
//! process inherits the full `PATH` and every lookup here would be redundant.
//! The bug only exists in the bundle.
//!
//! So we ask a login shell, once, and its one answer serves both halves of
//! running an agent: finding out whether one is installed (`agents::pick`) and
//! the environment it is started with (`terminal::pty::build_command`).
//! Answering only the first would trade "no agent is installed" for an agent
//! that starts and cannot find `git`, `node` or its own helpers.
//!
//! `-i` is not optional, and it is not the cautious choice either: `-l` alone
//! reads `~/.zprofile` and would have missed this machine entirely, where cargo
//! and the rest are added by `~/.zshrc`. The cost is that an interactive rc file
//! prints things — shell integration escape sequences, version notices,
//! greetings — into the same stream, which is why the value travels between
//! markers instead of being read off the first line.

use std::sync::OnceLock;

/// Fenceposts around the payload. An interactive shell writes its own noise to
/// stdout before and after the script runs, and none of it is ours to predict.
const BEGIN: &str = "__SMETANA_ENV_BEGIN__";
const END: &str = "__SMETANA_ENV_END__";

/// A ceiling on a wedged shell, in the same spirit as the two seconds
/// `terminal::service::shutdown` waits: an rc file that blocks forever — on a
/// prompt, on a network mount, on `read` — must cost this one lookup and not
/// the app. Generous rather than tight, because the alternative to waiting is
/// telling a person their installed agent does not exist.
const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

/// The three variables that decide a child's character encoding, in the order
/// POSIX resolves them: the first one set is the one that answers. The single
/// copy of that list — `terminal::pty` iterates this rather than repeating it,
/// because a closed list written out twice is how the side-tab set and the
/// agent-id list each cost this project a silent defect.
pub(crate) const LOCALE_KEYS: [&str; 3] = ["LC_ALL", "LC_CTYPE", "LANG"];

/// Both answers, resolved together because one login shell carries both.
struct Resolved {
    path: Option<String>,
    locale: Locale,
}

/// What a child should be told about its locale.
pub struct Locale {
    /// The variable to set: theirs when their value is being forwarded,
    /// `FALLBACK_KEY` when it is not. A caller has to make sure nothing
    /// `outranking` this reaches the child, or the setting being replaced would
    /// outrank its replacement and the whole thing would merely look fixed.
    pub key: &'static str,
    /// Their value, when it names the UTF-8 codeset. `None` means either that
    /// nobody said anything, or that what they said contradicts the channel —
    /// and both leave the codeset for the caller to name, since the caller is
    /// what knows the bytes its own stream carries.
    pub value: Option<String>,
}

static RESOLVED: OnceLock<Resolved> = OnceLock::new();

/// The fallbacks are the whole of the error handling. A shell that will not
/// start, answers too slowly or prints something unrecognisable leaves us
/// exactly where we were before this module existed — which is correct in
/// development and no worse than the old behaviour anywhere else.
///
/// Resolving happens once per run. Either value changing under a running app
/// means a person edited an rc file, and they can restart; re-reading it on a
/// schedule would spawn a login shell behind their back forever.
fn resolved() -> &'static Resolved {
    RESOLVED.get_or_init(|| {
        let stdout = resolve();
        let read = |key: &str| stdout.as_deref().and_then(|out| extract(out, key));
        let found = choose(read).or_else(|| choose(inherited));
        Resolved {
            path: read("PATH").or_else(|| std::env::var("PATH").ok()),
            locale: judge(found),
        }
    })
}

/// The best `PATH` we know: the login shell's when it could be read, and the
/// one this process inherited otherwise.
pub fn path() -> Option<&'static str> {
    resolved().path.as_deref()
}

/// The locale a child should be started with. Always an answer: see `Locale`
/// for what each half means and why the variable is chosen rather than fixed.
pub fn locale() -> &'static Locale {
    &resolved().locale
}

/// A resolved variable becomes an instruction. Pure, and the part of this most
/// worth a test: what it decides is invisible until somebody's tools start
/// mangling their language.
///
/// The person's own locale is forwarded, because their language, money and dates
/// are none of this app's business. Their *codeset* is, and only when it
/// contradicts the channel: a PTY here carries UTF-8 by construction, since
/// `terminal_write` sends a Rust string's own bytes and both `screen.rs` and
/// xterm.js decode UTF-8 at the other end. So a `C`, a `POSIX` or a legacy
/// codeset is dropped rather than passed on — and dropping it matters most
/// where it looks least necessary: `export LC_ALL=C` in an rc file, a common
/// habit for deterministic tool output, is an *answer*, so without this the
/// fallback would never run and the child would be handed the exact locale this
/// is about. A legacy codeset is worse than the C locale rather than equal to
/// it: `ru_RU.KOI8-R` would have the agent's tools emit KOI8-R into a pane that
/// decodes UTF-8, where `C` at least lets UTF-8 bytes through untouched.
/// A rejected value does not keep its variable, and that is the one part of this
/// with a measurement behind it rather than a principle. The fallback codeset is
/// bare `UTF-8` on macOS, and bare `UTF-8` is meaningful only in the `LC_CTYPE`
/// position: `LC_ALL=UTF-8` and `LANG=UTF-8` both resolve to US-ASCII. So
/// answering an `LC_ALL=C` in place would swap one silent failure for another.
/// Instead the answer always lands on `FALLBACK_KEY`, and `terminal::pty` drops
/// whatever outranks it — which is what removes the offending variable when the
/// offender was the one on top.
///
/// The alternative, replacing only the codeset so `ru_RU.KOI8-R` became
/// `ru_RU.UTF-8`, keeps somebody's language and is rejected all the same:
/// nothing here can tell whether the locale it just invented exists on the
/// machine, and one that does not resolves to US-ASCII with no error — the same
/// silence, arrived at by a longer road.
fn judge(found: Option<(&'static str, String)>) -> Locale {
    match found {
        Some((key, value)) if names_utf8(&value) => Locale { key, value: Some(value) },
        _ => Locale { key: FALLBACK_KEY, value: None },
    }
}

/// The first of `LOCALE_KEYS` that `lookup` answers for. Pure, and split out
/// because it is the composition the tests could not otherwise reach: with the
/// walk inlined, reordering `LOCALE_KEYS` broke nothing that runs.
fn choose(lookup: impl Fn(&str) -> Option<String>) -> Option<(&'static str, String)> {
    LOCALE_KEYS.iter().find_map(|key| lookup(key).map(|value| (*key, value)))
}

/// The variables that beat `key`, for a caller that has to stop them reaching a
/// child. `LOCALE_KEYS` is in precedence order, so they are the ones in front of
/// it; a key not in the list has nothing above it by definition.
pub(crate) fn outranking(key: &str) -> &'static [&'static str] {
    let position = LOCALE_KEYS.iter().position(|candidate| *candidate == key).unwrap_or(0);
    &LOCALE_KEYS[..position]
}

/// The variable an invented answer is written to. `LC_CTYPE` because the codeset
/// is the only part of a locale an app is entitled to decide for somebody, and
/// because it is the only one of the three a bare codeset is a legal value for
/// at all — `terminal::pty::UTF8_LOCALE` has the measurements.
const FALLBACK_KEY: &str = "LC_CTYPE";

/// Whether a locale value names a codeset the platform will actually resolve to
/// UTF-8. Split by platform because the two disagree about spelling, and being
/// generous on the wrong one is what puts a person back in US-ASCII while the
/// code claims to have covered them.
///
/// A locale is `language[_TERRITORY][.codeset][@modifier]`, and only what
/// follows the `.` is read here. A value with no `.` names no codeset at all and
/// means US-ASCII — which is why a bare `UTF-8` is never forwarded even though
/// macOS accepts it in the one position `terminal::pty` puts it in.
///
/// What this cannot check is whether the *language* exists: `xx_YY.UTF-8` is
/// well-formed and resolves to US-ASCII, and finding that out means asking the
/// system rather than reading the string. It is left alone deliberately — a
/// language nobody has is a broken rc file, the reader here is about the
/// codeset, and the pairing test in `terminal::pty` is what would catch it if it
/// ever became a real machine's answer.
#[cfg(target_os = "macos")]
pub(crate) fn names_utf8(value: &str) -> bool {
    // Measured on macOS 26.5.2, as `LC_ALL=<value> locale charmap`:
    //
    //   en_US.UTF-8 -> UTF-8      en_US.utf8         -> US-ASCII
    //   en_US.utf-8 -> utf-8      en_US.UTF8         -> US-ASCII
    //   C.UTF-8     -> UTF-8      en_US.UTF-8@euro   -> US-ASCII
    //                             "en_US.UTF-8 "     -> US-ASCII
    //
    // Only the hyphen spelling resolves, and case does not matter, but a
    // modifier or a trailing space is fatal. So the codeset has to be this and
    // nothing besides — an exact comparison rather than a prefix or a search,
    // which is what rejects the last three.
    matches!(value.split_once('.'), Some((_, codeset)) if codeset.eq_ignore_ascii_case("utf-8"))
}

/// Everywhere else, `.utf8` is the ordinary spelling — it is what `locale -a`
/// prints on Debian — so the comparison ignores punctuation and case alike, and
/// a modifier is dropped rather than treated as part of the codeset.
#[cfg(not(target_os = "macos"))]
pub(crate) fn names_utf8(value: &str) -> bool {
    let Some((_, codeset)) = value.split_once('.') else { return false };
    let codeset = codeset.split('@').next().unwrap_or(codeset);
    let letters: String =
        codeset.chars().filter(char::is_ascii_alphanumeric).map(|c| c.to_ascii_uppercase()).collect();
    letters == "UTF8"
}

/// This process's own environment, read the same way the shell's output is. In
/// development it carries the person's locale, because the binary was started
/// from their shell; in a bundle it carries nothing, which is the case this
/// module exists for.
fn inherited(key: &str) -> Option<String> {
    std::env::var(key).ok().filter(|value| !value.is_empty())
}

/// Start resolving now, on a thread of its own, so the first agent a person
/// starts does not pay for it.
///
/// This is a warm-up and not an initialisation: `path()` and `locale()` are each
/// complete on their own, and a caller that beats the thread to one simply
/// blocks inside `get_or_init` until the answer lands. Nothing has to happen in
/// a particular order.
pub fn warm() {
    std::thread::spawn(|| {
        // `resolved()` rather than one of the two readers: both answers come out
        // of the same login shell, and naming the shared work says so.
        resolved();
    });
}

/// Ask a login shell, and hand back everything it printed. `None` on any
/// failure — see `resolved()` for why that is the entire error path.
#[cfg(unix)]
fn resolve() -> Option<String> {
    use std::io::Read;
    use std::process::{Command, Stdio};

    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
    // `/usr/bin/env` rather than `printf '%s' "$PATH"` because `$PATH` is a list
    // in fish and would come back space-separated; `env` prints the colon form
    // every shell keeps for its children, which is the one we want anyway.
    let script = format!("printf '%s\\n' '{BEGIN}'; /usr/bin/env; printf '%s\\n' '{END}'");

    let mut child = Command::new(&shell)
        // Separate flags, not `-ilc`: fish takes these three and does not take
        // them bundled.
        .args(["-i", "-l", "-c", &script])
        // The same courtesy VS Code extends, and for the same reason: an rc file
        // that is expensive or chatty can look for this and skip itself.
        .env("SMETANA_RESOLVING_ENVIRONMENT", "1")
        // Closed rather than inherited: an interactive shell that finds a
        // readable stdin can block on it, and there is nobody here to type.
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        // An rc file's warnings are not ours to relay.
        .stderr(Stdio::null())
        .spawn()
        .map_err(|err| eprintln!("[env] could not start {shell}: {err}"))
        .ok()?;

    let mut stdout = child.stdout.take()?;
    // Read on another thread so the timeout below is a timeout and not a
    // deadlock: a shell that writes more than a pipe buffer and never exits
    // would block us here forever if we waited on the process first.
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = stdout.read_to_end(&mut buf);
        let _ = tx.send(buf);
    });

    match rx.recv_timeout(TIMEOUT) {
        Ok(buf) => {
            let _ = child.wait();
            // Lossy: an environment variable that is not UTF-8 is possible on
            // Unix and cannot survive the `&str` the rest of this wants —
            // a `PATH` entry naming a directory in some other encoding is the
            // realistic case. Mangling one line beats discarding the whole
            // answer, and every key read out of it is ASCII.
            Some(String::from_utf8_lossy(&buf).into_owned())
        }
        Err(_) => {
            eprintln!("[env] {shell} did not answer within {TIMEOUT:?}; using the inherited environment");
            let _ = child.kill();
            let _ = child.wait();
            None
        }
    }
}

/// Windows has no login shell to ask and no `.zshrc` problem to solve; the
/// inherited environment is already the person's own.
#[cfg(not(unix))]
fn resolve() -> Option<String> {
    None
}

/// One variable out of what the shell printed. Pure, and the part of this worth
/// a test — spawning a login shell is not testable, the same as bd's calls and
/// the PTY spawn aren't.
///
/// The first matching line between the markers wins. A multi-line value earlier
/// in the environment could in principle contain a line that looks like one,
/// and nothing in `env`'s output distinguishes the two — but the alternative
/// rules (last one, refuse when there are several) are guesses of the same kind
/// against an event nobody has seen.
fn extract(stdout: &str, key: &str) -> Option<String> {
    let body = stdout.split_once(BEGIN)?.1.split_once(END)?.0;
    let prefix = format!("{key}=");
    body.lines()
        .find_map(|line| line.strip_prefix(&prefix))
        // An empty value is not an answer: an empty `PATH` would be read as
        // "nothing is installed", an empty `LANG` says nothing about the
        // encoding, and both are indistinguishable from a shell that broke
        // halfway.
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The noise is real: it is what `/bin/zsh -i -l -c` prints on the machine
    /// this was written on, where shell integration writes OSC sequences around
    /// every command. Reading the first line of that stream would have found a
    /// terminal escape and called it a PATH.
    const NOISE: &str = "\u{1b}]133;D;0\u{1b}\\\u{1b}]7;file://host/Users/x\u{1b}\\";

    fn output(body: &str) -> String {
        format!("{NOISE}{BEGIN}\n{body}\n{END}\n{NOISE}")
    }

    /// `choose` over a shell's output, which is the composition `resolved()`
    /// performs — the real function, not a copy of its walk.
    fn locale_of(body: &str) -> Option<(&'static str, String)> {
        let out = output(body);
        choose(|key| extract(&out, key))
    }

    #[test]
    fn the_value_is_read_out_of_a_stream_an_rc_file_wrote_into() {
        let out = output("SHELL=/bin/zsh\nPATH=/Users/x/.local/bin:/usr/bin\nTERM=xterm");
        assert_eq!(extract(&out, "PATH").as_deref(), Some("/Users/x/.local/bin:/usr/bin"));
    }

    #[test]
    fn a_shell_that_printed_nothing_we_recognise_is_not_an_answer() {
        // Half an answer is the shape a killed shell leaves behind, and it must
        // not be read as one: the fallback in `resolved()` is the correct
        // outcome for all three.
        assert_eq!(extract(NOISE, "PATH"), None);
        assert_eq!(extract(&format!("{NOISE}{BEGIN}\nPATH=/usr/bin\n"), "PATH"), None);
        assert_eq!(extract(&output("SHELL=/bin/zsh\nTERM=xterm"), "PATH"), None);
    }

    #[test]
    fn an_empty_value_is_a_failure_and_not_an_empty_answer() {
        assert_eq!(extract(&output("PATH="), "PATH"), None);
        assert_eq!(extract(&output("LANG="), "LANG"), None);
    }

    #[test]
    fn a_value_of_its_own_is_left_exactly_as_the_shell_printed_it() {
        // No trimming, no splitting, no reordering: whatever the shell says is
        // what its children get, including a trailing empty entry that means
        // "the current directory" to some tools.
        let out = output("PATH=/opt/homebrew/bin::/usr/bin:");
        assert_eq!(extract(&out, "PATH").as_deref(), Some("/opt/homebrew/bin::/usr/bin:"));
    }

    #[test]
    fn a_key_is_never_answered_by_a_longer_name_it_begins() {
        // What the `=` in the prefix earns: `strip_prefix` is anchored, so a
        // name the key *ends* with was never a hazard, but one the key is the
        // start of is. `LANGUAGE` is real, common, and sorts before `LANG` in
        // the `env` output of any shell — without the `=` it would answer for
        // `LANG` with `UAGE=en_US:en`.
        let out = output("LANGUAGE=en_US:en\nLANG=ru_RU.UTF-8");
        assert_eq!(extract(&out, "LANG").as_deref(), Some("ru_RU.UTF-8"));
    }

    #[test]
    fn a_utf8_locale_is_forwarded_exactly_as_the_person_set_it() {
        // Their language, territory and modifier are none of this app's
        // business; only the codeset is, and this one agrees with the channel.
        let found = Some(("LANG", "ru_RU.UTF-8".to_string()));
        let locale = judge(found);
        assert_eq!(locale.key, "LANG");
        assert_eq!(locale.value.as_deref(), Some("ru_RU.UTF-8"));
    }

    #[test]
    fn a_codeset_that_contradicts_the_channel_takes_its_variable_down_with_it() {
        // `export LC_ALL=C` is a common habit for deterministic tool output, and
        // it is an *answer*, so nothing but this check stands between it and a
        // child handed the exact locale the bug is about. The variable goes too
        // rather than being overwritten in place: the invented value is a bare
        // codeset, which is legal only as an `LC_CTYPE`, so `LC_ALL=UTF-8` would
        // resolve to US-ASCII. `terminal::pty` removes what `outranking` names,
        // which is what clears the rejected `LC_ALL` out of the way.
        for value in ["C", "POSIX", "en_US.ISO8859-1", "ru_RU.KOI8-R"] {
            let locale = judge(Some(("LC_ALL", value.to_string())));
            assert_eq!(locale.key, FALLBACK_KEY, "{value}");
            assert_eq!(locale.value, None, "{value}");
            assert!(outranking(locale.key).contains(&"LC_ALL"), "{value} must be cleared away");
        }
    }

    #[test]
    fn the_variables_that_beat_a_key_are_the_ones_in_front_of_it() {
        assert_eq!(outranking("LC_ALL"), &[] as &[&str]);
        assert_eq!(outranking("LC_CTYPE"), &["LC_ALL"]);
        assert_eq!(outranking("LANG"), &["LC_ALL", "LC_CTYPE"]);
        // A key that is not one of these has nothing above it to clear.
        assert_eq!(outranking("TERM"), &[] as &[&str]);
    }

    #[test]
    fn nobody_naming_a_locale_leaves_both_halves_to_the_caller() {
        let locale = judge(None);
        assert_eq!(locale.key, FALLBACK_KEY);
        assert_eq!(locale.value, None);
    }

    #[test]
    fn a_value_naming_no_codeset_never_passes_on_any_platform() {
        // No `.` means no codeset, which means US-ASCII — including the bare
        // `UTF-8` that looks like a shortcut and is one only in the `LC_CTYPE`
        // position `terminal::pty` writes it to.
        for value in ["C", "POSIX", "UTF-8", "utf-8", "en_US", ""] {
            assert!(!names_utf8(value), "{value:?} names no codeset");
        }
        for value in ["ru_RU.KOI8-R", "en_US.ISO8859-1", "en_US.eucJP"] {
            assert!(!names_utf8(value), "{value} names a codeset that is not UTF-8");
        }
    }

    /// Measured on macOS 26.5.2 with `LC_ALL=<value> locale charmap`: only the
    /// hyphen spelling resolves, case is free, and a modifier or a trailing
    /// space takes the whole value down to US-ASCII. Being generous here is what
    /// would hand somebody a locale the system reads as US-ASCII while both the
    /// code and this test claimed they were covered.
    #[cfg(target_os = "macos")]
    #[test]
    fn macos_resolves_only_the_hyphen_spelling() {
        for value in ["en_US.UTF-8", "en_US.utf-8", "C.UTF-8", "en_US.Utf-8"] {
            assert!(names_utf8(value), "{value} resolves to UTF-8 on macOS");
        }
        for value in ["en_US.utf8", "en_US.UTF8", "zh_CN.UTF-8@pinyin", "en_US.UTF-8 "] {
            assert!(!names_utf8(value), "{value:?} resolves to US-ASCII on macOS");
        }
    }

    /// glibc prints `en_US.utf8` from `locale -a`, so the spelling rule that is
    /// correct on macOS would reject the ordinary form there.
    #[cfg(not(target_os = "macos"))]
    #[test]
    fn elsewhere_the_punctuation_of_a_codeset_does_not_matter() {
        for value in ["en_US.UTF-8", "en_US.utf8", "en_US.UTF8", "zh_CN.UTF-8@pinyin"] {
            assert!(names_utf8(value), "{value} names UTF-8");
        }
    }

    #[test]
    fn the_locale_is_read_from_the_same_answer_as_the_path() {
        let found = locale_of("PATH=/usr/bin\nLANG=ru_RU.UTF-8\nTERM=xterm");
        assert_eq!(found, Some(("LANG", "ru_RU.UTF-8".to_string())));
    }

    #[test]
    fn the_locale_variables_are_read_in_the_order_posix_resolves_them() {
        // A person who sets `LC_ALL` means it to override the rest, so passing
        // on their `LANG` instead would quietly hand the child the losing value.
        let body = "LANG=en_US.UTF-8\nLC_CTYPE=de_DE.UTF-8\nLC_ALL=ru_RU.UTF-8";
        assert_eq!(locale_of(body), Some(("LC_ALL", "ru_RU.UTF-8".to_string())));
        let body = "LANG=en_US.UTF-8\nLC_CTYPE=de_DE.UTF-8";
        assert_eq!(locale_of(body), Some(("LC_CTYPE", "de_DE.UTF-8".to_string())));
    }

    #[test]
    fn a_shell_with_no_locale_at_all_is_not_an_answer() {
        // The launchd environment this module exists for, and the case
        // `terminal::pty` answers for itself.
        assert_eq!(locale_of("PATH=/usr/bin\nTERM=xterm"), None);
    }

    /// Not a test of the parser but of the machine: on any developer's Unix box
    /// a login shell has a `PATH`, and if this stops being true the fallback in
    /// `path()` is what a person would be left with.
    #[cfg(unix)]
    #[test]
    fn a_login_shell_on_this_machine_answers() {
        let stdout = resolve().expect("a login shell must answer");
        let path = extract(&stdout, "PATH").expect("a login shell must report a PATH");
        assert!(path.contains('/'), "{path:?} does not look like a PATH");
    }
}
