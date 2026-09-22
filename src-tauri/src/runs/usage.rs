//! What the subscription has left, and what a run does about it.
//!
//! The port of `holiday-curb`'s usage gating — the one piece of that script
//! `docs/superpowers/specs/2026-08-05-runs-design.md` deliberately left behind,
//! on the grounds that reading `claude -p "/usage"` is a parse of somebody
//! else's prose that breaks silently. That reasoning stands; what did not is
//! the trade it was made for. A run that exhausts its allowance overnight
//! spends five sessions and a minute of backoff discovering it, then stops with
//! `Crashed` — which says the harness kept failing, when nothing failed at all
//! and the work was never stuck. The two need opposite responses from a person,
//! and the run was giving them the wrong one.
//!
//! So the parse comes back, with its failure mode named rather than assumed:
//! **an unreadable answer never blocks a run**. It reads as `Normal` and the
//! batch goes at full size, which is exactly where things were before this
//! module existed. That is the same shape layer B keeps in `agents/claude.rs`
//! — no match leaves the previous behaviour in place instead of inventing one.
//!
//! The gate runs **before** each batch rather than after a failure, and that is
//! the whole of why it is worth having: an allowance is checked before it is
//! spent, so the exhausted case costs no session at all. `service.rs` asks the
//! same question a second time after a session exits non-zero, for the case the
//! allowance ran out mid-batch — there it is not a gate but a classification,
//! telling a spent limit apart from a harness that fell over.
//!
//! **The probe's working directory is an empty folder of the app's own, never
//! the process's inherited one.** A bundled app started from Finder or
//! launchd is sitting in `/`, and Claude Code indexes every file under its cwd
//! at start-up — asked from `/` it walked the whole disk in the first three
//! seconds, and macOS put up a permission prompt for every protected folder it
//! crossed on the way, for data this app never touches (smetana-48iy). The
//! project root was refused too: the footer's probe has no project, a one-shot
//! question gets its whole context in the prompt already, and either way
//! Claude Code would pick up that project's own `CLAUDE.md`, hooks and index —
//! paying both the cost and part of the risk this fix removes.
//! `std::env::temp_dir()` was refused as well: it is a folder shared with
//! whatever else on the machine writes into it, and nothing in it is worth
//! indexing either — an app-owned folder costs nothing more and shares with
//! nobody. `read` is
//! handed the path rather than computing it, so this module never learns Tauri
//! exists; `agents::probe_dir` is where the caller gets it, off
//! `app.path().app_data_dir()` and created if it is not there yet.
//!
//! Pure apart from `read`, which is the one function here that spawns anything.

use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use crate::agents::{Profile, UsageSource};

/// At or above this, take no work at all and wait for the reset. The source's
/// number, and it is not 100 for a reason: the reading is approximate — it
/// counts local sessions on this machine and not other devices — so a batch
/// started at 95% is one that runs into the wall halfway through, which costs
/// a killed session and the recovery phase that follows it.
pub const PAUSE_THRESHOLD: u8 = 90;
/// At or above this, take fewer tasks per batch.
pub const REDUCED_THRESHOLD: u8 = 75;
/// How many tasks a batch may take while reduced. Tasks, not agents: a lead
/// spawns whatever teammates a task needs, and this is the count of tasks it
/// may have in flight at once — `[defaults].max_parallel_tasks`, whose own
/// default is 3.
pub const REDUCED_MAX_TASKS: u8 = 2;
/// How often to ask again while paused. A session limit resets in hours and a
/// weekly one in days, so asking oftener than this only spends the machine.
pub const POLL: Duration = Duration::from_secs(10 * 60);
/// A probe that hangs is worse than one that fails: the run would sit between
/// batches with nothing on screen to say why.
const PROBE_TIMEOUT: Duration = Duration::from_secs(60);

/// A threshold that is off. Not a percentage anybody could mean: "pause when 0%
/// of the allowance is used" is "never run at all", so the value is free to
/// carry the other meaning — and it carries it on the wire and in
/// `settings.json` too, because `adopt()` in `src/views/SettingsWindow.vue`
/// skips a field whose value is `null` and an `Option` would therefore never
/// reach the settings window when somebody turned a threshold off.
pub const OFF: u8 = 0;

/// At or above this the allowance is out, whatever the person has set their own
/// gate to. Deliberately a second constant rather than a reuse of
/// `PAUSE_THRESHOLD`, though it ships with the same number: that one is a
/// default somebody may move, and this one is the app's own reading of "the
/// harness will refuse the next session", which is not theirs to move. It is
/// only ever asked *after* a session has already exited non-zero, so it costs
/// nothing when things are going well.
pub const SPENT: u8 = 90;

/// The thresholds a person has chosen, read off `settings.json`.
///
/// Read at every gate check rather than once when the run started, which is the
/// opposite of what `drive` does with `agent` and `remove_worktrees` and for a
/// reason that does not apply to them: changing those mid-run would make a run
/// ask about one subscription and spend another, while changing these only
/// moves when it waits. Somebody watching a paused run and lowering the gate
/// wants that run to go on, not to be stopped and started again.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    pub pause_at: u8,
    pub reduced_at: u8,
}

impl Default for Limits {
    fn default() -> Self {
        Self { pause_at: PAUSE_THRESHOLD, reduced_at: REDUCED_THRESHOLD }
    }
}

/// What the harness said is left. Percentages used, not remaining.
///
/// **A percentage is optional, and that is the whole point of the type.** The
/// harness prints two limit lines and either of them can go missing — a
/// reworded line, a build that prints one of them and not the other — and the
/// half that was not read has no number at all. A zero standing in for it is a
/// claim about an allowance nobody measured, which for a run is merely the
/// benign direction to be wrong in and on the settings window is a sentence
/// the app has no grounds for (smetana-7rp). So the absent half is `None`
/// here and `null` on the wire, and a real `0%` — which the harness does
/// print, on a fresh week — stays `Some(0)` and is drawn.
///
/// `Serialize` because this rides out to the settings window as well as into
/// the run gate, and `camelCase` because that is what every other type crossing
/// that boundary uses — `settings/model.rs` and `git.rs` among them.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Usage {
    pub session_pct: Option<u8>,
    /// When it resets, in the harness's own words. Deliberately a string and
    /// never a moment in time: "Aug 11 at 5:59pm (Europe/Moscow)" is written
    /// for a person to read, and turning it into an instant would add a second
    /// parse of the same prose — one whose failure would be a run that woke at
    /// the wrong hour rather than one that showed a line it could not use.
    pub session_reset: Option<String>,
    /// The same reset as `session_reset`, normalized for failover scheduling.
    /// A failed parse leaves the visible text and percentage intact.
    #[serde(skip_serializing)]
    pub session_reset_at: Option<DateTime<Utc>>,
    /// The source's name for the first window. Claude Code's prose parser
    /// leaves this absent, preserving its established "Session" wording;
    /// Codex derives it from `windowDurationMins` (for example, "5 hours").
    pub session_label: Option<String>,
    pub week_pct: Option<u8>,
    pub week_reset: Option<String>,
    /// The same reset as `week_reset`, normalized for failover scheduling.
    #[serde(skip_serializing)]
    pub week_reset_at: Option<DateTime<Utc>>,
    /// The source's name for the second window; see `session_label`.
    pub week_label: Option<String>,
}

/// The one allowance window currently blocking work. Keeping these facts in a
/// single value prevents a percentage from one window being paired with a reset
/// from another while deciding whether a failover wait is short enough.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlockingAllowance<'a> {
    pub pct: u8,
    pub resets: Option<&'a str>,
    pub reset_at: Option<DateTime<Utc>>,
}

impl Usage {
    /// The limit that is actually in the way, out of the halves that were read.
    /// Both are reported when both are there, and the run stops for whichever
    /// is nearer its ceiling; one half alone is the answer on its own, and
    /// neither is no answer at all.
    ///
    /// `Ord` on `Option` puts `None` below every `Some`, so the missing half
    /// can never win the comparison and can never be read as a zero either.
    pub fn pct(&self) -> Option<u8> {
        self.blocking().map(|allowance| allowance.pct)
    }

    /// When *that* one resets. A tie goes to the session, which is the sooner
    /// of the two and therefore the more useful thing to put on screen — and
    /// by the same ordering, a session that was not read loses to a week that
    /// was.
    fn reset(&self) -> Option<&str> {
        self.blocking().and_then(|allowance| allowance.resets)
    }

    /// The largest measured window, with the historical session-first tie
    /// rule. The text and machine reset moment travel from that same window.
    pub fn blocking(&self) -> Option<BlockingAllowance<'_>> {
        if self.session_pct >= self.week_pct {
            self.session_pct.map(|pct| BlockingAllowance {
                pct,
                resets: self.session_reset.as_deref(),
                reset_at: self.session_reset_at,
            })
        } else {
            self.week_pct.map(|pct| BlockingAllowance {
                pct,
                resets: self.week_reset.as_deref(),
                reset_at: self.week_reset_at,
            })
        }
    }
}

/// What to do about it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    /// Nothing. Also the answer when the allowance could not be read at all.
    Normal,
    /// Run, but take fewer tasks. Carries the reading for the same reason
    /// `Pause` does: a batch quietly running at half the size somebody chose is
    /// a behaviour with nothing on screen to explain it.
    Reduced { pct: u8 },
    /// Take nothing and wait.
    Pause { pct: u8, resets: Option<String> },
}

/// Three bands, and `None` — an unreadable answer, or one with neither half of
/// it read — is deliberately the most permissive of the four. Refusing to work
/// because a probe failed would turn every hiccup in somebody else's CLI into a
/// stopped run, and the failure this module exists to prevent is not that one.
///
/// The bands are the person's own, out of `settings.json`, and `Limits::default`
/// is what this module used to hold as constants. A threshold set to `OFF` is
/// never entered at all, however high the reading — which is a decision about
/// pre-empting and not about noticing: `spent` below is the rule that keeps on
/// noticing, and `gate` is what puts the two together.
pub fn decide(usage: Option<&Usage>, limits: Limits) -> Decision {
    let Some(usage) = usage else { return Decision::Normal };
    // A reading with neither half in it says nothing about the allowance, so it
    // takes the same answer as no reading at all — the rule this module is
    // built on is that nothing which failed to be read may hold a run up.
    let Some(pct) = usage.pct() else { return Decision::Normal };
    if limits.pause_at != OFF && pct >= limits.pause_at {
        return Decision::Pause { pct, resets: usage.reset().map(str::to_owned) };
    }
    if limits.reduced_at != OFF && pct >= limits.reduced_at {
        return Decision::Reduced { pct };
    }
    Decision::Normal
}

/// Whether the allowance is out, by a rule no setting reaches.
///
/// The classification after a session exits non-zero asks this and nothing
/// else: a limit that ran out mid-batch and a harness that fell over are the
/// same absence to anyone reading an exit code, and they need opposite
/// responses. Were this to follow the person's own pause threshold, turning
/// that threshold off would make every spent allowance read as a crash, and the
/// run would stop with `Crashed` after `MAX_CRASHES` — which is the failure this
/// whole module exists to prevent, arriving through the settings window.
///
/// `None` is not spent. An unreadable probe never holds a run up.
pub fn spent(usage: Option<&Usage>) -> bool {
    usage.and_then(Usage::pct).is_some_and(|pct| pct >= SPENT)
}

/// Whether a pause is the hold above rather than one of the person's own
/// thresholds — the one distinction the run bar needs, because "Run anyway" is
/// worth offering for a threshold and worth refusing for a spent allowance,
/// where the next session would die the moment it started.
///
/// Deliberately not read off the `Decision`: both arrive as `Pause` and the
/// difference is in what produced them. Asked beside `gate` with the same two
/// arguments, so the two answers cannot come from different readings.
///
/// True whenever the hold applies, even where a threshold would have paused the
/// run anyway. Pressing the button in that case would release the threshold and
/// leave the allowance exactly as spent, which is the churn the gate exists to
/// prevent.
pub fn held(usage: Option<&Usage>, after_limited: bool) -> bool {
    after_limited && spent(usage)
}

/// What the run loop's gate does with a reading: the person's own bands, unless
/// the batch before this one died on a spent allowance and the allowance is
/// still spent.
///
/// The second half is what keeps "off" meaning *do not pre-empt* rather than
/// *do not notice*. Without it a run with the gate off would spend a session
/// discovering the wall, be told `LastBatch::Limited`, come straight back here,
/// be told to go, and do it again for as long as the queue lasts.
pub fn gate(usage: Option<&Usage>, limits: Limits, after_limited: bool) -> Decision {
    let decision = decide(usage, limits);
    if !after_limited || matches!(decision, Decision::Pause { .. }) {
        return decision;
    }
    match usage.filter(|reading| spent(Some(reading))) {
        Some(reading) => Decision::Pause {
            // `spent` is only true for a reading with a percentage in it, so the
            // fall-back is unreachable; it is there so this cannot panic.
            pct: reading.pct().unwrap_or(SPENT),
            resets: reading.reset().map(str::to_owned),
        },
        None => decision,
    }
}

/// Which of `decide`'s three bands a reading falls in, and nothing else from it.
///
/// The band travels to the front end while the comparison stays here. Handing
/// the percentages over and comparing them in JS was the alternative, and it
/// was refused for the reason two copies of a threshold are always refused: the
/// second copy drifts from the first with nothing on screen to say it has.
///
/// Which numbers produced the band is now the person's, out of `settings.json`
/// — `PAUSE_THRESHOLD` and `REDUCED_THRESHOLD` are what ships, not what applies.
/// That is what makes the sentence under the percentages in the settings window
/// and what a run actually does one fact rather than two: both come through
/// `decide` with the same `Limits`.
///
/// The reading itself is not repeated inside it the way `Decision` repeats it,
/// since the whole `Usage` is already beside it in the answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Band {
    Normal,
    Reduced,
    Pause,
}

impl Band {
    fn of(decision: &Decision) -> Self {
        match decision {
            Decision::Normal => Band::Normal,
            Decision::Reduced { .. } => Band::Reduced,
            Decision::Pause { .. } => Band::Pause,
        }
    }
}

/// What the settings window is told when it asks what is left of the
/// subscription.
///
/// **Three distinguishable states rather than an `Option<Usage>`**, and the
/// third is the whole reason for the type. Through an `Option` the front end
/// could not tell "this agent does not answer that question at all" — Codex has
/// no `usage_command` — from "this agent was asked and could not answer", and
/// those are different sentences for a person and different things for them to
/// do about it. The run gate needs no such distinction, since both of them are
/// `Decision::Normal` there, which is why `decide` keeps taking an `Option` and
/// this is a second reading of the same fact rather than a change to that one.
///
/// **The agent rides in the answer.** `agents::pick` substitutes the first
/// installed profile for a configured one that is not on `PATH`, so the block
/// headed "Claude Code subscription" can be about Codex; the heading has to
/// name whoever actually answered rather than whoever is showing in the
/// dropdown.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum AgentUsage {
    /// There is nothing to ask. Either the profile has no `usage_command` of
    /// its own, or no agent is installed at all — and that second case is the
    /// one with no agent to name, which is what the `Option` is for.
    Unsupported { agent: Option<String> },
    /// The probe was made but did not produce a trustworthy allowance. The
    /// reason is deliberately a closed, safe vocabulary rather than an app
    /// server error string, which could contain account or network detail.
    Unreadable { agent: String, reason: Unavailable },
    /// A reading, with the band it falls in.
    ///
    /// **`enumerates_windows` is what the front end branches on instead of an
    /// agent id.** `usageFooter.js`'s two-slot strip used to ask whether the
    /// agent was named `"codex"` to decide whether a window with no number in
    /// it means "not read yet" or "does not exist" — exactly the class of
    /// hardcode `agents::catalogue` exists to remove. The true fact is not
    /// which harness answered, it is *how* the source reports: `AppServer`
    /// names the windows it has outright, so a missing half really is a
    /// missing window, while `Command` parses somebody else's prose, where a
    /// missing half is a line that was not read. So this field is derived
    /// from `UsageSource` alone, here, once, where `agent` already is.
    ///
    /// An answer from a build before this field existed carries neither key
    /// at all, and the front end's truthiness check on a missing property
    /// reads that as `false` — which is `UsageSource::Command`'s own
    /// behaviour, Claude Code's two fixed slots with dashes, so an old-shape
    /// answer changes nothing.
    ///
    /// The container's own `rename_all` reaches variant names and not their
    /// fields, so this variant carries its own — without it `enumerates_windows`
    /// would ride as-is and the front end's `answer.enumeratesWindows` would
    /// read `undefined` from a build that sends it.
    #[serde(rename_all = "camelCase")]
    Read { agent: String, usage: Usage, band: Band, enumerates_windows: bool },
}

/// Why a probe could not yield a normalized reading. These values deliberately
/// name a next step without exposing app-server output, authentication data,
/// or an implementation-specific protocol error to the interface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Unavailable {
    NotSignedIn,
    UnsupportedAccount,
    TimedOut,
    InvalidResponse,
}

/// The one mapping from "who would answer, and what did they say" to what the
/// settings window draws. Pure, and the whole of the command behind it: the
/// command's own body is the two blocking calls that produce these arguments.
pub fn report(
    profile: Option<&'static dyn Profile>,
    reading: Result<Usage, Unavailable>,
    limits: Limits,
) -> AgentUsage {
    let Some(profile) = profile else { return AgentUsage::Unsupported { agent: None } };
    let agent = profile.id().to_owned();
    // Asked before the reading is looked at, because a profile that cannot be
    // asked and one that was asked and said nothing both arrive here as `None`
    // — `read` answers that for every way of failing, this one included.
    let Some(source) = profile.usage_source() else {
        return AgentUsage::Unsupported { agent: Some(agent) };
    };
    let usage = match reading {
        Ok(usage) => usage,
        Err(reason) => return AgentUsage::Unreadable { agent, reason },
    };
    let band = Band::of(&decide(Some(&usage), limits));
    // See `AgentUsage::Read`'s own doc: derived from the source's shape and
    // nowhere else, never from the agent's id.
    let enumerates_windows = matches!(source, UsageSource::AppServer);
    AgentUsage::Read { agent, usage, band, enumerates_windows }
}

/// How many tasks the next batch may take.
///
/// The number a person chose is never rewritten — `RunSettings` keeps it and
/// the report names it — and this is what one batch is run with instead. The
/// same split `views/panelWidths.js` makes between the width that is stored and
/// the width that is drawn, for the same reason: a condition of the moment must
/// not silently become a preference.
///
/// `None` stays `None`: that is `Solo`, where the lead does the work itself and
/// a number of tasks would be a second instruction contradicting the first.
pub fn cap(chosen: Option<u8>, decision: &Decision) -> Option<u8> {
    match decision {
        Decision::Reduced { .. } => chosen.map(|n| n.min(REDUCED_MAX_TASKS)),
        Decision::Normal | Decision::Pause { .. } => chosen,
    }
}

/// The probe's command line, built rather than run — the shape
/// `runs/preflight.rs::curl` uses ("built rather than run, so a test can read
/// it"), so a test reads `Command::get_current_dir()` back off it without
/// spawning anything.
fn command(profile: &'static dyn Profile, args: &'static [&'static str], cwd: &Path) -> Command {
    let mut command = Command::new(profile.binary());
    command
        .args(args)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    // The login shell's PATH, for the reason `terminal/service.rs` records at
    // its own `agents::pick`: a bundled app started from Finder inherits
    // launchd's, where nothing a person installed is reachable.
    if let Some(path) = crate::shell_env::path() {
        command.env("PATH", path);
    }
    command
}

/// Ask the harness. `None` means the question could not be asked or the answer
/// could not be read, which `decide` treats as no reason to hold anything up.
///
/// Blocking, and called from `spawn_blocking`. The output is small — a couple
/// of kilobytes — so reading it after the wait cannot deadlock on a full pipe
/// the way a large one would.
///
/// `cwd` is the caller's to resolve — see this module's header — and it rides
/// in beside `profile` rather than being read off the disk in here, which is
/// what keeps this file free of Tauri.
pub fn read(profile: &'static dyn Profile, cwd: &Path) -> Option<Usage> {
    read_detail(profile, cwd).ok()
}

/// Read one current subscription snapshot with a named failure for the two UI
/// surfaces. `read` above deliberately discards that name for the run gate:
/// unknown allowance must remain permissive, while a person deserves to know
/// whether to sign in, wait, or update Codex.
pub fn read_detail(profile: &'static dyn Profile, cwd: &Path) -> Result<Usage, Unavailable> {
    match profile.usage_source().ok_or(Unavailable::InvalidResponse)? {
        UsageSource::Command => read_command(profile, cwd),
        UsageSource::AppServer => read_app_server(profile, cwd),
    }
}

fn read_command(profile: &'static dyn Profile, cwd: &Path) -> Result<Usage, Unavailable> {
    let args = profile.usage_command().ok_or(Unavailable::InvalidResponse)?;
    let mut child = command(profile, args, cwd).spawn().map_err(|_| Unavailable::InvalidResponse)?;
    let deadline = Instant::now() + PROBE_TIMEOUT;
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => break,
            // A non-zero probe says nothing about the allowance — it says the
            // probe failed — so it is the same answer as no probe at all.
            Ok(Some(_)) => return Err(Unavailable::InvalidResponse),
            Ok(None) if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(Unavailable::TimedOut);
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(200)),
            Err(_) => return Err(Unavailable::InvalidResponse),
        }
    }
    let output = child.wait_with_output().map_err(|_| Unavailable::InvalidResponse)?;
    profile.parse_usage(&String::from_utf8_lossy(&output.stdout)).ok_or(Unavailable::InvalidResponse)
}

/// The Codex app-server is a line-delimited JSON-RPC process which continues
/// serving after the response we need. Read its one response on a helper
/// thread, bound the wait here, then always kill and reap the child. Keeping
/// the child alive would leave an app-server behind every ten-minute poll;
/// closing stdin instead is not enough because it makes a server free to exit
/// before it sends the pending result.
fn read_app_server(profile: &'static dyn Profile, cwd: &Path) -> Result<Usage, Unavailable> {
    let mut child = app_server_command(profile, cwd)
        .spawn()
        .map_err(|_| Unavailable::InvalidResponse)?;
    let result = (|| {
        let mut stdin = child.stdin.take().ok_or(Unavailable::InvalidResponse)?;
        let stdout = child.stdout.take().ok_or(Unavailable::InvalidResponse)?;
        let (sent, received) = mpsc::channel();
        // Deliberately detached. A process Codex started can retain stdout after
        // its direct parent is killed; joining this reader on a timeout would
        // turn the probe's ceiling into an unbounded wait. The direct app-server
        // is still killed and reaped below on every path.
        std::thread::spawn(move || read_app_server_responses(stdout, sent));
        app_server_session(&mut stdin, &received, Instant::now() + PROBE_TIMEOUT)
            .and_then(|response| profile.parse_usage_response(&response).ok_or(Unavailable::InvalidResponse))
    })();
    let _ = child.kill();
    let _ = child.wait();
    result
}

fn app_server_command(profile: &'static dyn Profile, cwd: &Path) -> Command {
    let mut command = Command::new(profile.binary());
    command
        .args(["app-server", "--stdio"])
        .current_dir(cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    if let Some(path) = crate::shell_env::path() {
        command.env("PATH", path);
    }
    command
}

/// Complete the read-only app-server handshake in protocol order. The one
/// deadline belongs to the whole exchange: initialization, account identity,
/// and rate limits collectively have sixty seconds, not sixty each.
fn app_server_session(
    stdin: &mut impl Write,
    received: &mpsc::Receiver<(i64, Result<serde_json::Value, Unavailable>)>,
    deadline: Instant,
) -> Result<serde_json::Value, Unavailable> {
    write_rpc(stdin, 1, "initialize", serde_json::json!({
        "clientInfo": { "name": "smetana", "title": "Smetana", "version": env!("CARGO_PKG_VERSION") },
        "capabilities": {}
    }))?;
    wait_for_rpc(received, deadline, 1)?;

    write_notification(stdin, "initialized", serde_json::json!({}))?;
    write_rpc(stdin, 2, "account/read", serde_json::json!({}))?;
    let account = wait_for_rpc(received, deadline, 2)?;
    subscription_account(&account)?;

    write_rpc(stdin, 3, "account/rateLimits/read", serde_json::Value::Null)?;
    wait_for_rpc(received, deadline, 3)
}

fn write_rpc(
    stdin: &mut impl Write,
    id: i64,
    method: &str,
    params: serde_json::Value,
) -> Result<(), Unavailable> {
    serde_json::to_writer(&mut *stdin, &serde_json::json!({ "id": id, "method": method, "params": params }))
        .map_err(|_| Unavailable::InvalidResponse)?;
    stdin.write_all(b"\n").and_then(|_| stdin.flush()).map_err(|_| Unavailable::InvalidResponse)
}

fn write_notification(
    stdin: &mut impl Write,
    method: &str,
    params: serde_json::Value,
) -> Result<(), Unavailable> {
    serde_json::to_writer(&mut *stdin, &serde_json::json!({ "method": method, "params": params }))
        .map_err(|_| Unavailable::InvalidResponse)?;
    stdin.write_all(b"\n").and_then(|_| stdin.flush()).map_err(|_| Unavailable::InvalidResponse)
}

fn wait_for_rpc(
    received: &mpsc::Receiver<(i64, Result<serde_json::Value, Unavailable>)>,
    deadline: Instant,
    wanted: i64,
) -> Result<serde_json::Value, Unavailable> {
    loop {
        let remaining = deadline.checked_duration_since(Instant::now()).ok_or(Unavailable::TimedOut)?;
        match received.recv_timeout(remaining) {
            Ok((id, answer)) if id == wanted => return answer,
            Ok(_) => continue,
            Err(mpsc::RecvTimeoutError::Timeout) => return Err(Unavailable::TimedOut),
            Err(mpsc::RecvTimeoutError::Disconnected) => return Err(Unavailable::InvalidResponse),
        }
    }
}

/// `account/read` is the stable, structured answer for what kind of login the
/// app-server is using. It is intentionally checked before rate limits: text in
/// an RPC error is not an account contract and must not become a UI diagnosis.
fn subscription_account(account: &serde_json::Value) -> Result<(), Unavailable> {
    match account.get("account") {
        Some(serde_json::Value::Null) => Err(Unavailable::NotSignedIn),
        Some(account) if account.get("type").and_then(serde_json::Value::as_str) == Some("chatgpt") => Ok(()),
        Some(account) if account.get("type").and_then(serde_json::Value::as_str).is_some() => {
            Err(Unavailable::UnsupportedAccount)
        }
        _ => Err(Unavailable::InvalidResponse),
    }
}

fn read_app_server_responses(
    stdout: std::process::ChildStdout,
    sent: mpsc::Sender<(i64, Result<serde_json::Value, Unavailable>)>,
) {
    for line in BufReader::new(stdout).lines() {
        let Ok(line) = line else { break };
        let Ok(message) = serde_json::from_str::<serde_json::Value>(&line) else { continue };
        let Some(id) = message.get("id").and_then(serde_json::Value::as_i64) else { continue };
        let answer = if let Some(error) = message.get("error") {
            let _ = error;
            Err(Unavailable::InvalidResponse)
        } else {
            message.get("result").cloned().ok_or(Unavailable::InvalidResponse)
        };
        if sent.send((id, answer)).is_err() {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[derive(Clone)]
    struct SharedWire(Arc<Mutex<Vec<u8>>>);

    impl SharedWire {
        fn methods(&self) -> Vec<String> {
            // `to_writer` may call `write` more than once. The other thread
            // can therefore only observe frames it knows are complete, not a
            // valid prefix which happens to end in the middle of a string.
            self.0.lock().unwrap().split_inclusive(|byte| *byte == b'\n')
                .filter(|frame| frame.last() == Some(&b'\n'))
                .map(|frame| serde_json::from_slice::<serde_json::Value>(frame).unwrap()["method"].as_str().unwrap().to_owned())
                .collect()
        }

        fn wait_for_methods(&self, expected: &[&str]) {
            let deadline = Instant::now() + Duration::from_secs(1);
            while Instant::now() < deadline {
                if self.methods().iter().map(String::as_str).eq(expected.iter().copied()) {
                    return;
                }
                std::thread::sleep(Duration::from_millis(1));
            }
            assert_eq!(self.methods(), expected, "app-server request order");
        }
    }

    impl Write for SharedWire {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(bytes);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    /// Built rather than run: the probe's cwd is never the process's
    /// inherited one, and this is checked without spawning a harness at all.
    #[test]
    fn the_probe_runs_in_the_cwd_it_is_given() {
        let dir = Path::new("/tmp/smetana-usage-probe-test");
        let cmd = command(&crate::agents::claude::Claude, &["-p", "/usage"], dir);
        assert_eq!(cmd.get_current_dir(), Some(dir));
    }

    #[test]
    fn the_codex_probe_is_the_app_server_in_the_cwd_it_is_given() {
        let dir = Path::new("/tmp/smetana-usage-probe-test");
        let cmd = app_server_command(&crate::agents::codex::Codex, dir);
        assert_eq!(cmd.get_current_dir(), Some(dir));
        assert_eq!(
            cmd.get_args().map(|arg| arg.to_string_lossy()).collect::<Vec<_>>(),
            ["app-server", "--stdio"]
        );
    }

    #[test]
    fn app_server_waits_for_initialize_before_sending_the_follow_up_requests() {
        // The writer is intentionally observed while the session thread is
        // running; repeating it catches framing races that a one-off run can
        // easily miss.
        for _ in 0..200 {
            let (sent, received) = mpsc::channel();
            let wire = SharedWire(Arc::new(Mutex::new(Vec::new())));
            let worker_wire = wire.clone();
            let session = std::thread::spawn(move || {
                let mut writer = worker_wire;
                app_server_session(&mut writer, &received, Instant::now() + Duration::from_secs(1))
            });

            wire.wait_for_methods(&["initialize"]);
            sent.send((1, Ok(serde_json::json!({})))).unwrap();
            wire.wait_for_methods(&["initialize", "initialized", "account/read"]);
            sent.send((2, Ok(serde_json::json!({ "account": { "type": "chatgpt" }, "requiresOpenaiAuth": true })))).unwrap();
            wire.wait_for_methods(&["initialize", "initialized", "account/read", "account/rateLimits/read"]);
            sent.send((3, Ok(serde_json::json!({ "rateLimits": {} })))).unwrap();
            assert_eq!(
                session.join().unwrap(),
                Ok(serde_json::json!({ "rateLimits": {} }))
            );
        }
    }

    #[test]
    fn an_initialize_error_stops_the_protocol_before_any_follow_up_request() {
        let (sent, received) = mpsc::channel();
        sent.send((1, Err(Unavailable::InvalidResponse))).unwrap();
        let mut wire = Vec::new();

        assert_eq!(
            app_server_session(&mut wire, &received, Instant::now() + Duration::from_secs(1)),
            Err(Unavailable::InvalidResponse)
        );
        let messages = String::from_utf8(wire).unwrap();
        assert_eq!(messages.lines().count(), 1);
        assert_eq!(serde_json::from_str::<serde_json::Value>(messages.trim()).unwrap()["method"], "initialize");
    }

    #[test]
    fn account_read_is_the_structured_source_for_safe_login_and_account_reasons() {
        assert_eq!(subscription_account(&serde_json::json!({ "account": null })), Err(Unavailable::NotSignedIn));
        assert_eq!(
            subscription_account(&serde_json::json!({ "account": { "type": "apiKey" } })),
            Err(Unavailable::UnsupportedAccount)
        );
        assert_eq!(
            subscription_account(&serde_json::json!({ "account": { "type": "amazonBedrock" } })),
            Err(Unavailable::UnsupportedAccount)
        );
        assert_eq!(subscription_account(&serde_json::json!({ "account": { "type": "chatgpt" } })), Ok(()));
        assert_eq!(subscription_account(&serde_json::json!({ "unexpected": true })), Err(Unavailable::InvalidResponse));
    }

    #[test]
    fn an_unrelated_response_id_is_not_accepted_as_the_requested_answer() {
        let (sent, received) = mpsc::channel();
        sent.send((99, Ok(serde_json::json!({ "wrong": true })))).unwrap();
        sent.send((1, Ok(serde_json::json!({ "right": true })))).unwrap();
        assert_eq!(
            wait_for_rpc(&received, Instant::now() + Duration::from_secs(1), 1),
            Ok(serde_json::json!({ "right": true }))
        );
    }

    #[test]
    fn an_expired_shared_deadline_does_not_start_a_new_timeout_for_the_handshake() {
        let (_sent, received) = mpsc::channel();
        let mut wire = Vec::new();
        assert_eq!(
            app_server_session(&mut wire, &received, Instant::now()),
            Err(Unavailable::TimedOut)
        );
        assert_eq!(std::str::from_utf8(&wire).unwrap().lines().count(), 1);
    }

    fn usage(session: u8, week: u8) -> Usage {
        Usage {
            session_pct: Some(session),
            session_reset: Some("Aug 7 at 8pm".into()),
            session_reset_at: None,
            session_label: None,
            week_pct: Some(week),
            week_reset: Some("Aug 11 at 5:59pm".into()),
            week_reset_at: None,
            week_label: None,
        }
    }

    #[test]
    fn blocking_keeps_the_percentage_text_and_reset_instant_from_one_window() {
        let session_at = chrono::DateTime::parse_from_rfc3339("2026-09-22T10:00:00Z").unwrap().with_timezone(&Utc);
        let week_at = chrono::DateTime::parse_from_rfc3339("2026-09-23T10:00:00Z").unwrap().with_timezone(&Utc);
        let usage = Usage {
            session_pct: Some(90),
            session_reset: Some("soon".into()),
            session_reset_at: Some(session_at),
            week_pct: Some(80),
            week_reset: Some("later".into()),
            week_reset_at: Some(week_at),
            ..Usage::default()
        };
        let blocking = usage.blocking().expect("a window was measured");
        assert_eq!(blocking.pct, 90);
        assert_eq!(blocking.resets, Some("soon"));
        assert_eq!(blocking.reset_at, Some(session_at));
    }

    /// A reading with only the session in it: the shape `agents/claude.rs`
    /// hands over when one of the two lines it looks for has been reworded.
    fn session_only(session: u8) -> Usage {
        Usage { week_pct: None, week_reset: None, ..usage(session, 0) }
    }

    #[test]
    fn an_allowance_that_could_not_be_read_never_holds_a_run_up() {
        // The whole reason this module is allowed to parse somebody else's
        // prose: when the parse fails, things are exactly where they were
        // before it existed.
        assert_eq!(decide(None, Limits::default()), Decision::Normal);
    }

    #[test]
    fn a_reading_with_neither_half_in_it_is_no_reading_either() {
        // `claude.rs` does not produce this one — it answers `None` rather than
        // an empty reading — but `decide` is the place the rule is written, and
        // a second caller must not be able to stop a run by handing over an
        // allowance nobody read.
        assert_eq!(decide(Some(&Usage::default()), Limits::default()), Decision::Normal);
        assert_eq!(Usage::default().pct(), None);
    }

    #[test]
    fn one_half_of_a_reading_is_the_whole_of_the_decision() {
        // The bug this shape exists for: with the week unread, a `0` standing
        // in for it used to be compared against the session and lose, which is
        // harmless here and a lie on the settings window. The half that
        // arrived is the number, not its maximum with an invented zero.
        assert_eq!(session_only(80).pct(), Some(80));
        assert_eq!(decide(Some(&session_only(80)), Limits::default()), Decision::Reduced { pct: 80 });
        assert_eq!(session_only(0).pct(), Some(0), "a real zero is a reading");

        let week_only = Usage { session_pct: None, session_reset: None, ..usage(0, 95) };
        assert_eq!(week_only.pct(), Some(95));
        assert_eq!(
            decide(Some(&week_only), Limits::default()),
            Decision::Pause { pct: 95, resets: Some("Aug 11 at 5:59pm".into()) },
            "the reset named is the one of the half that is in the way"
        );
    }

    #[test]
    fn the_three_bands_are_read_off_whichever_limit_is_nearer_its_ceiling() {
        assert_eq!(decide(Some(&usage(0, 0)), Limits::default()), Decision::Normal);
        assert_eq!(decide(Some(&usage(74, 74)), Limits::default()), Decision::Normal);
        assert_eq!(decide(Some(&usage(REDUCED_THRESHOLD, 0)), Limits::default()), Decision::Reduced { pct: REDUCED_THRESHOLD });
        assert_eq!(decide(Some(&usage(0, REDUCED_THRESHOLD)), Limits::default()), Decision::Reduced { pct: REDUCED_THRESHOLD });
        assert_eq!(decide(Some(&usage(89, 89)), Limits::default()), Decision::Reduced { pct: 89 });
        assert!(matches!(decide(Some(&usage(PAUSE_THRESHOLD, 0)), Limits::default()), Decision::Pause { .. }));
        assert!(matches!(decide(Some(&usage(0, PAUSE_THRESHOLD)), Limits::default()), Decision::Pause { .. }));
    }

    #[test]
    fn a_pause_names_the_reset_of_the_limit_that_is_in_the_way() {
        // Showing the session's reset while it is the week that is exhausted
        // would send somebody back in an hour to find the run still paused.
        let Decision::Pause { pct, resets } = decide(Some(&usage(10, 95)), Limits::default()) else {
            panic!("95% of the week is a pause");
        };
        assert_eq!(pct, 95);
        assert_eq!(resets.as_deref(), Some("Aug 11 at 5:59pm"));

        let Decision::Pause { pct, resets } = decide(Some(&usage(95, 10)), Limits::default()) else {
            panic!("95% of the session is a pause");
        };
        assert_eq!(pct, 95);
        assert_eq!(resets.as_deref(), Some("Aug 7 at 8pm"));
    }

    #[test]
    fn a_reduced_batch_takes_fewer_tasks_and_never_more_than_was_asked_for() {
        // Reduced is a ceiling, not a number: somebody who chose one task at a
        // time must not find two running because the allowance ran low.
        assert_eq!(cap(Some(8), &Decision::Reduced { pct: 80 }), Some(REDUCED_MAX_TASKS));
        assert_eq!(cap(Some(3), &Decision::Reduced { pct: 80 }), Some(REDUCED_MAX_TASKS));
        assert_eq!(cap(Some(1), &Decision::Reduced { pct: 80 }), Some(1));
    }

    #[test]
    fn nothing_but_reduced_touches_the_number_of_tasks() {
        for decision in [Decision::Normal, Decision::Pause { pct: 99, resets: None }] {
            assert_eq!(cap(Some(4), &decision), Some(4));
        }
    }

    #[test]
    fn an_app_server_probe_that_fails_is_unreadable_rather_than_unsupported() {
        // Codex has a source, so a failed app-server read must prompt a safe
        // recovery action rather than claiming the subscription is unsupported.
        assert_eq!(
            report(
                Some(&crate::agents::codex::Codex),
                Err(Unavailable::InvalidResponse),
                Limits::default()
            ),
            AgentUsage::Unreadable { agent: "codex".into(), reason: Unavailable::InvalidResponse }
        );
    }

    #[test]
    fn a_machine_with_no_agent_at_all_has_nobody_to_name() {
        assert_eq!(
            report(None, Err(Unavailable::InvalidResponse), Limits::default()),
            AgentUsage::Unsupported { agent: None }
        );
    }

    #[test]
    fn a_probe_that_gave_nothing_back_is_unreadable_and_never_a_reading_of_zero() {
        // A `Usage::default` here would put "0% used" on the screen of somebody
        // who is simply not signed in.
        assert_eq!(
            report(
                Some(&crate::agents::claude::Claude),
                Err(Unavailable::InvalidResponse),
                Limits::default()
            ),
            AgentUsage::Unreadable { agent: "claude".into(), reason: Unavailable::InvalidResponse }
        );
    }

    #[test]
    fn a_reading_carries_the_agent_that_answered_and_the_band_it_falls_in() {
        let AgentUsage::Read { agent, usage: read, band, enumerates_windows } =
            report(Some(&crate::agents::claude::Claude), Ok(usage(10, 80)), Limits::default())
        else {
            panic!("a reading from a profile that can be asked");
        };
        assert_eq!(agent, "claude");
        assert_eq!(read, usage(10, 80));
        assert_eq!(band, Band::Reduced);
        // Claude Code's source is `UsageSource::Command`, a prose parser — a
        // missing half of its reading is a line that was not read, not a
        // window that does not exist.
        assert!(!enumerates_windows);
    }

    #[test]
    fn a_source_that_lists_its_own_windows_says_so_in_the_reading() {
        let AgentUsage::Read { enumerates_windows, .. } =
            report(Some(&crate::agents::codex::Codex), Ok(usage(10, 80)), Limits::default())
        else {
            panic!("a reading from a profile that can be asked");
        };
        // Codex's app-server names the windows it has outright, which is the
        // fact `usageFooter.js`'s two-slot strip needs — never the agent id.
        assert!(enumerates_windows);
    }

    #[test]
    fn the_wire_shape_is_the_one_the_settings_window_reads() {
        // The names are load-bearing and nothing else pins them: the front end
        // reads `state`, `agent`, `band`, `enumeratesWindows` and the four
        // camelCase fields of the reading, and a rename here would empty the
        // block with every gate still green.
        let json = serde_json::to_value(report(
            Some(&crate::agents::claude::Claude),
            Ok(usage(10, 20)),
            Limits::default(),
        ))
        .expect("the answer serializes");
        assert_eq!(json["state"], "read");
        assert_eq!(json["agent"], "claude");
        assert_eq!(json["band"], "normal");
        assert_eq!(json["enumeratesWindows"], false);
        assert_eq!(json["usage"]["sessionPct"], 10);
        assert_eq!(json["usage"]["sessionReset"], "Aug 7 at 8pm");
        assert!(json["usage"]["sessionLabel"].is_null());
        assert_eq!(json["usage"]["weekPct"], 20);
        assert_eq!(json["usage"]["weekReset"], "Aug 11 at 5:59pm");
        assert!(json["usage"]["weekLabel"].is_null());

        let json = serde_json::to_value(report(
            Some(&crate::agents::codex::Codex),
            Ok(usage(10, 20)),
            Limits::default(),
        ))
        .expect("the answer serializes");
        assert_eq!(json["enumeratesWindows"], true, "the app-server source lists its own windows");

        // A half that was not read travels as an explicit `null` under the key
        // it would have had, rather than by the key going missing: the front
        // end reads it with `Number.isFinite`, which refuses both, but the two
        // are not the same promise and only one of them is testable from here.
        let json = serde_json::to_value(report(
            Some(&crate::agents::claude::Claude),
            Ok(session_only(10)),
            Limits::default(),
        ))
        .expect("the answer serializes");
        assert_eq!(json["usage"]["sessionPct"], 10);
        assert!(json["usage"]["weekPct"].is_null(), "an unread half is null and never a zero");
        assert!(json["usage"].as_object().expect("a reading is an object").contains_key("weekPct"));

        let json = serde_json::to_value(report(
            Some(&crate::agents::codex::Codex),
            Err(Unavailable::InvalidResponse),
            Limits::default()
        ))
            .expect("the answer serializes");
        assert_eq!(json["state"], "unreadable");
        assert_eq!(json["agent"], "codex");
    }

    #[test]
    fn solo_has_no_number_of_tasks_to_reduce() {
        // `RunSettings::validate` is what makes it `None` there, and a batch
        // that suddenly grew one would be told to delegate work it was
        // started to do itself.
        for decision in
            [Decision::Normal, Decision::Reduced { pct: 80 }, Decision::Pause { pct: 99, resets: None }]
        {
            assert_eq!(cap(None, &decision), None);
        }
    }

    #[test]
    fn a_threshold_that_is_off_is_never_entered() {
        let limits = Limits { pause_at: OFF, reduced_at: REDUCED_THRESHOLD };
        // 99% used, and the person has said not to pause on it.
        assert_eq!(decide(Some(&usage(99, 0)), limits), Decision::Reduced { pct: 99 });
        let limits = Limits { pause_at: PAUSE_THRESHOLD, reduced_at: OFF };
        assert_eq!(decide(Some(&usage(80, 0)), limits), Decision::Normal);
    }

    #[test]
    fn both_thresholds_off_is_always_normal() {
        let limits = Limits { pause_at: OFF, reduced_at: OFF };
        assert_eq!(decide(Some(&usage(100, 100)), limits), Decision::Normal);
    }

    #[test]
    fn the_shipped_limits_are_the_bands_this_module_had() {
        let limits = Limits::default();
        assert_eq!(decide(Some(&usage(74, 0)), limits), Decision::Normal);
        assert_eq!(decide(Some(&usage(75, 0)), limits), Decision::Reduced { pct: 75 });
        assert!(matches!(decide(Some(&usage(90, 0)), limits), Decision::Pause { .. }));
    }

    #[test]
    fn a_spent_allowance_is_read_at_ninety_and_above() {
        assert!(spent(Some(&usage(SPENT, 0))));
        assert!(spent(Some(&usage(0, 99))));
        assert!(!spent(Some(&usage(89, 89))));
    }

    #[test]
    fn nothing_that_could_not_be_read_is_ever_spent() {
        assert!(!spent(None));
        assert!(!spent(Some(&Usage::default())));
    }

    #[test]
    fn the_gate_holds_after_a_limited_batch_with_every_threshold_off() {
        let limits = Limits { pause_at: OFF, reduced_at: OFF };
        let reading = usage(95, 0);
        // Nothing was limited yet: the person's own thresholds are the whole
        // answer, and they say go.
        assert_eq!(gate(Some(&reading), limits, false), Decision::Normal);
        // A batch has just died on a spent allowance, so the run waits it out
        // rather than spending another session finding out again.
        assert!(matches!(gate(Some(&reading), limits, true), Decision::Pause { pct: 95, .. }));
    }

    #[test]
    fn the_gate_lets_a_run_through_once_the_reading_has_dropped() {
        let limits = Limits { pause_at: OFF, reduced_at: OFF };
        assert_eq!(gate(Some(&usage(3, 40)), limits, true), Decision::Normal);
    }

    #[test]
    fn the_gate_carries_the_reset_of_the_limit_that_is_in_the_way() {
        let limits = Limits { pause_at: OFF, reduced_at: OFF };
        let reading = Usage {
            session_pct: Some(96),
            session_reset: Some("Sep 1 at 6pm (Europe/Moscow)".into()),
            session_reset_at: None,
            session_label: None,
            week_pct: Some(20),
            week_reset: Some("Sep 4 at 9am (Europe/Moscow)".into()),
            week_reset_at: None,
            week_label: None,
        };
        assert_eq!(
            gate(Some(&reading), limits, true),
            Decision::Pause { pct: 96, resets: Some("Sep 1 at 6pm (Europe/Moscow)".into()) }
        );
    }

    #[test]
    fn a_reduced_band_the_person_chose_still_stands_after_a_limited_batch() {
        // The hold is only ever a `Pause`, so a reading inside somebody's own
        // reduced band comes back reduced rather than being promoted.
        let limits = Limits { pause_at: OFF, reduced_at: 50 };
        assert_eq!(gate(Some(&usage(60, 0)), limits, true), Decision::Reduced { pct: 60 });
    }

    #[test]
    fn the_band_the_settings_window_draws_is_the_persons_own() {
        // 80% with the pause threshold moved down to 80: the window must say a
        // run would stop here, not that it would merely take fewer tasks.
        let limits = Limits { pause_at: 80, reduced_at: 50 };
        let answer = report(Some(&crate::agents::claude::Claude), Ok(usage(80, 0)), limits);
        assert!(matches!(answer, AgentUsage::Read { band: Band::Pause, .. }));
    }

    #[test]
    fn a_hold_is_told_from_a_threshold_by_what_produced_it() {
        let reading = usage(95, 0);
        // Nobody's batch has died yet: whatever the bands say, this is the
        // person's own gate and the button is worth offering.
        assert!(!held(Some(&reading), false));
        // The batch before this one died on it, and it is still spent.
        assert!(held(Some(&reading), true));
        // True even where a threshold would have paused the run anyway:
        // releasing the threshold would leave the allowance just as spent.
        assert!(held(Some(&reading), true));
        // Dropped back under the line: the hold is over, and a pause here can
        // only be somebody's own threshold again.
        assert!(!held(Some(&usage(60, 0)), true));
        // Nothing that could not be read ever holds a run.
        assert!(!held(None, true));
    }
}
