//! The `PATH` a child process should actually be started with.
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
//! It is invisible in development, which is what makes it worth a module rather
//! than a line: `npm run tauri dev` starts the binary from a terminal, so the
//! process inherits the full `PATH` and every lookup here would be redundant.
//! The bug only exists in the bundle.
//!
//! So we ask a login shell what `PATH` is, once, and use its answer for both
//! halves of running an agent: finding out whether one is installed
//! (`agents::pick`) and the environment it is started with
//! (`terminal::pty::build_command`). Answering only the first would trade "no
//! agent is installed" for an agent that starts and cannot find `git`, `node`
//! or its own helpers.
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

static PATH: OnceLock<Option<String>> = OnceLock::new();

/// The best `PATH` we know: the login shell's when it could be read, and the
/// one this process inherited otherwise.
///
/// The fallback is the whole of the error handling. A shell that will not start,
/// answers too slowly or prints something unrecognisable leaves us exactly where
/// we were before this module existed — which is correct in development and no
/// worse than the old behaviour anywhere else.
///
/// Resolving happens once per run. `PATH` changing under a running app means a
/// person edited an rc file, and they can restart; re-reading it on a schedule
/// would spawn a login shell behind their back forever.
pub fn path() -> Option<&'static str> {
    PATH.get_or_init(|| resolve().or_else(|| std::env::var("PATH").ok())).as_deref()
}

/// Start resolving now, on a thread of its own, so the first agent a person
/// starts does not pay for it.
///
/// This is a warm-up and not an initialisation: `path()` is complete on its own,
/// and a caller that beats the thread to it simply blocks inside `get_or_init`
/// until the answer lands. Nothing has to happen in a particular order.
pub fn warm() {
    std::thread::spawn(|| {
        path();
    });
}

/// Ask a login shell. `None` on any failure — see `path()` for why that is the
/// entire error path.
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
            // Lossy: a path that is not UTF-8 is possible on Unix and cannot
            // survive the `&str` the rest of this wants. Mangling one entry
            // beats discarding the whole answer over it.
            extract(&String::from_utf8_lossy(&buf))
        }
        Err(_) => {
            eprintln!("[env] {shell} did not answer within {TIMEOUT:?}; using the inherited PATH");
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

/// `PATH` out of what the shell printed. Pure, and the part of this worth a
/// test — spawning a login shell is not testable, the same as bd's calls and
/// the PTY spawn aren't.
///
/// The first `PATH=` line between the markers wins. A multi-line value earlier
/// in the environment could in principle contain a line that looks like one,
/// and nothing in `env`'s output distinguishes the two — but the alternative
/// rules (last one, refuse when there are several) are guesses of the same kind
/// against an event nobody has seen.
fn extract(stdout: &str) -> Option<String> {
    let body = stdout.split_once(BEGIN)?.1.split_once(END)?.0;
    body.lines()
        .find_map(|line| line.strip_prefix("PATH="))
        // An empty PATH is not an answer: it would be read as "nothing is
        // installed" and is indistinguishable from a shell that broke halfway.
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

    #[test]
    fn the_value_is_read_out_of_a_stream_an_rc_file_wrote_into() {
        let out = output("SHELL=/bin/zsh\nPATH=/Users/x/.local/bin:/usr/bin\nTERM=xterm");
        assert_eq!(extract(&out).as_deref(), Some("/Users/x/.local/bin:/usr/bin"));
    }

    #[test]
    fn a_shell_that_printed_nothing_we_recognise_is_not_an_answer() {
        // Half an answer is the shape a killed shell leaves behind, and it must
        // not be read as one: the fallback in `path()` is the correct outcome
        // for all three.
        assert_eq!(extract(NOISE), None);
        assert_eq!(extract(&format!("{NOISE}{BEGIN}\nPATH=/usr/bin\n")), None);
        assert_eq!(extract(&output("SHELL=/bin/zsh\nTERM=xterm")), None);
    }

    #[test]
    fn an_empty_path_is_a_failure_and_not_an_empty_answer() {
        assert_eq!(extract(&output("PATH=")), None);
    }

    #[test]
    fn a_value_of_its_own_is_left_exactly_as_the_shell_printed_it() {
        // No trimming, no splitting, no reordering: whatever the shell says is
        // what its children get, including a trailing empty entry that means
        // "the current directory" to some tools.
        let out = output("PATH=/opt/homebrew/bin::/usr/bin:");
        assert_eq!(extract(&out).as_deref(), Some("/opt/homebrew/bin::/usr/bin:"));
    }

    /// Not a test of the parser but of the machine: on any developer's Unix box
    /// a login shell has a `PATH`, and if this stops being true the fallback in
    /// `path()` is what a person would be left with.
    #[cfg(unix)]
    #[test]
    fn a_login_shell_on_this_machine_answers() {
        let resolved = resolve().expect("a login shell must report a PATH");
        assert!(resolved.contains('/'), "{resolved:?} does not look like a PATH");
    }
}
