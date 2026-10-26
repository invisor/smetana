//! The subcommands that are clients of a running server. Each is a function over a [`Client`] and a
//! writer, so a test drives the very code the command line runs, with a terminal that is a buffer.

use std::io::Write;
use std::time::Duration;

use remote_protocol::{BridgeError, DeviceKind};
use remote_server::gateway::{LiveWork, Mode, Outcome, Reply};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::client::Client;

/// One line a person reads about a failed call.
pub fn describe(err: &BridgeError) -> String {
    match err {
        BridgeError::Handler(Value::String(text)) => text.clone(),
        BridgeError::Handler(other) => other.to_string(),
        other => other.to_string(),
    }
}

fn decode<T: for<'de> Deserialize<'de>>(value: Value) -> Result<T, String> {
    serde_json::from_value(value).map_err(|err| format!("the server's answer is not what this build reads: {err}"))
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct DeviceRow {
    pub key: String,
    pub name: String,
    pub kind: DeviceKind,
    pub paired_at: u64,
    pub last_contact: Option<u64>,
    /// Paired with the local credential rather than by a ticket.
    #[serde(default)]
    pub local: bool,
}

pub async fn devices(client: &Client) -> Result<Vec<DeviceRow>, String> {
    decode(client.call("remote_devices", json!({})).await.map_err(|err| describe(&err))?)
}

pub fn print_devices(rows: &[DeviceRow], out: &mut impl Write) -> std::io::Result<()> {
    if rows.is_empty() {
        return writeln!(out, "No devices are paired.");
    }
    for row in rows {
        let seen = row.last_contact.map_or("never".to_owned(), |at| format!("{at} (unix seconds)"));
        writeln!(out, "{}  {:<8} {}  last seen {seen}", row.key, format!("{:?}", row.kind).to_lowercase(), row.name)?;
    }
    Ok(())
}

/// Revokes the device `id` names: a whole key, or a prefix that picks out one. A device that is
/// connected is disconnected by the server before this returns.
pub async fn revoke(client: &Client, id: &str, out: &mut impl Write) -> Result<(), String> {
    let id = id.trim().to_lowercase();
    if id.len() < 4 {
        return Err("give at least the first four characters of the device's key (see `devices`)".to_owned());
    }
    let rows = devices(client).await?;
    let matches: Vec<&DeviceRow> = rows.iter().filter(|row| row.key.starts_with(&id)).collect();
    let row = match matches.as_slice() {
        [one] => *one,
        [] => return Err(format!("no paired device has a key starting {id}")),
        many => return Err(format!("{} devices start {id}; give more of the key", many.len())),
    };
    match client.call("remote_device_revoke", json!({ "key": row.key })).await {
        Ok(_) => {}
        // Revoking the device that is making the call: the server closes the connection before it
        // can answer, which is the revocation having worked.
        Err(BridgeError::Internal(_)) if row.key == client.id() => {}
        Err(err) => return Err(describe(&err)),
    }
    writeln!(out, "Revoked {} ({}). Its connection is closed.", row.name, row.key).map_err(|err| err.to_string())?;
    if row.local {
        // Said plainly: this is a client on the server's own machine, and the file it paired through is
        // still there. The boundary is the operating system's permission on the data directory.
        writeln!(
            out,
            "Note: this device is a client on this machine. It pairs through the local credential file in the \
             data directory, so any client run by this user will pair itself again the next time it connects. \
             Revoking it ends this connection; it does not keep it out."
        )
        .map_err(|err| err.to_string())?;
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
struct Begun {
    ticket: String,
    expires_at: u64,
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or(0)
}

/// The QR code of `text` as terminal text, two modules to a character cell.
pub fn qr_text(text: &str) -> Result<String, String> {
    let code = qrcode::QrCode::new(text.as_bytes()).map_err(|err| format!("the ticket does not fit a QR code: {err}"))?;
    Ok(code
        .render::<qrcode::render::unicode::Dense1x2>()
        .dark_color(qrcode::render::unicode::Dense1x2::Dark)
        .light_color(qrcode::render::unicode::Dense1x2::Light)
        .quiet_zone(true)
        .build())
}

/// How `pair` ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Paired {
    /// A device was paired, under this name.
    Accepted(String),
    /// A request came and the person said no.
    Declined,
    /// The ticket ran out, or was withdrawn, with no request.
    Expired,
}

/// Issues a ticket of `kind`, prints it as text and as a QR code, and waits for a device to spend it.
/// The confirmation is asked here, of whoever runs this: `confirm` is handed the device's name and
/// answers yes or no. `relay` is the address the ticket sends the device through.
pub async fn pair(
    client: &mut Client,
    kind: DeviceKind,
    relay: &str,
    mut confirm: impl FnMut(&str) -> bool,
    out: &mut impl Write,
) -> Result<Paired, String> {
    let io = |err: std::io::Error| err.to_string();
    client.listen("remote:pair-request").await.map_err(|e| e.to_string())?;
    client.listen("remote:pair-ended").await.map_err(|e| e.to_string())?;
    let begun: Begun = decode(
        client
            .call("remote_pair_begin", json!({ "relayUrl": relay, "kind": kind }))
            .await
            .map_err(|err| describe(&err))?,
    )?;
    writeln!(out, "Pairing ticket for a {kind:?} device, good for one use:").map_err(io)?;
    writeln!(out, "\n{}\n", begun.ticket).map_err(io)?;
    writeln!(out, "{}", qr_text(&begun.ticket)?).map_err(io)?;
    let left = begun.expires_at.saturating_sub(now());
    writeln!(out, "Waiting for the device. The ticket expires in {left} seconds.").map_err(io)?;
    out.flush().map_err(io)?;

    let deadline = tokio::time::Instant::now() + Duration::from_secs(left + 5);
    let mut name = None;
    let mut decided = None;
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        let Some((event, payload)) = client.event_within(remaining).await else {
            let _ = client.call("remote_pair_cancel", json!({})).await;
            return Ok(Paired::Expired);
        };
        match event.as_str() {
            "remote:pair-request" => {
                let device = payload["device_name"].as_str().unwrap_or("a device").to_owned();
                let accept = confirm(&device);
                match client.call("remote_pair_decide", json!({ "accept": accept })).await {
                    Ok(_) => {
                        decided = Some(accept);
                        name = Some(device);
                    }
                    // The request ended while the question was on screen.
                    Err(err) => writeln!(out, "{}", describe(&err)).map_err(io)?,
                }
            }
            "remote:pair-ended" => {
                return Ok(match (decided, name) {
                    (Some(true), Some(device)) => Paired::Accepted(device),
                    (Some(false), _) => Paired::Declined,
                    _ => Paired::Expired,
                });
            }
            _ => {}
        }
    }
}

fn shown(reply: &Reply) -> String {
    reply
        .work
        .iter()
        .map(|work| match work.id {
            Some(id) => format!("  {:?} {} #{id} in {}", work.kind, work.label, work.project),
            None => format!("  {:?} in {}", work.kind, work.project),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

async fn ask(client: &Client, mode: Mode, confirmed: Option<&[LiveWork]>) -> Result<Reply, String> {
    decode(
        client
            .call("server_restart", json!({ "mode": mode, "confirmed": confirmed }))
            .await
            .map_err(|err| describe(&err))?,
    )
}

/// How many times a confirmation may be overtaken by new work before the command gives up.
const CONFIRM_ROUNDS: usize = 3;

/// The restart command. `now` asks the server; if work is alive the server answers with the list and
/// does nothing. The person's answer goes back **carrying that list**, and the server ends only the work
/// it names: if more has started since, it answers with the fresh list and the person is asked again.
/// `yes` is the person having said so in advance; the list is printed anyway, so what is being
/// confirmed is on the screen.
pub async fn restart(
    client: &Client,
    mode: Mode,
    yes: bool,
    mut confirm: impl FnMut(&str) -> bool,
    out: &mut impl Write,
) -> Result<Outcome, String> {
    let io = |err: std::io::Error| err.to_string();
    let mut confirmed: Option<Vec<LiveWork>> = None;
    let mut mode = mode;
    for _ in 0..CONFIRM_ROUNDS {
        let reply = ask(client, mode, confirmed.as_deref()).await?;
        if reply.outcome == Outcome::NeedsConfirmation {
            writeln!(out, "This work is alive and a restart would end it:\n{}", shown(&reply)).map_err(io)?;
            if yes {
                writeln!(out, "--yes: confirming exactly the work listed above.").map_err(io)?;
            } else if !confirm("Restart anyway and end it?") {
                writeln!(out, "Nothing was restarted. `--when-idle` waits for the work to finish instead.")
                    .map_err(io)?;
                return Ok(Outcome::NeedsConfirmation);
            }
            confirmed = Some(reply.work);
            mode = Mode::Now;
            continue;
        }
        match reply.outcome {
            Outcome::Restarting => writeln!(out, "Restarting."),
            Outcome::Scheduled => writeln!(out, "Waiting for this work to finish, then restarting:\n{}", shown(&reply)),
            Outcome::Cancelled => writeln!(out, "The scheduled restart is withdrawn."),
            Outcome::NothingScheduled => writeln!(out, "No restart was scheduled."),
            Outcome::AlreadyRestarting => writeln!(out, "The server is already restarting; it is too late to cancel."),
            Outcome::NeedsConfirmation => Ok(()),
        }
        .map_err(io)?;
        return Ok(reply.outcome);
    }
    Err("new work keeps starting; nothing was restarted. Try `--when-idle`.".to_owned())
}

/// Where a status report came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// The running server's own, taken under its environment: what its agents will actually see.
    Server,
    /// Probed here, in the shell that ran the command, because there is no server for this directory:
    /// it has never run, or it stopped cleanly and took its credential file with it.
    NoServer,
    /// Probed here, because a server may exist and did not answer: its credential file is there, but it
    /// could not be reached, was too slow, or did not know the command (an older build).
    Unanswered,
}

impl Source {
    pub fn describe(self) -> &'static str {
        match self {
            Source::Server => "reported by the running server, under its own environment",
            Source::NoServer => {
                "no server is running for this data directory; probed from this shell, whose PATH and login may \
                 differ from the service's"
            }
            Source::Unanswered => {
                "a server's credential file is here but the server did not answer (stopped uncleanly, stuck, or an \
                 older build); probed from this shell, whose PATH and login may differ from the service's"
            }
        }
    }

    /// The word the JSON output carries.
    pub fn word(self) -> &'static str {
        match self {
            Source::Server => "server",
            Source::NoServer => "none",
            Source::Unanswered => "unanswered",
        }
    }
}

/// The environment report: the running server's if one answers, else a probe in this process. `bd` is
/// only for the probe; a running server uses the one it was started with.
pub async fn status(
    data_dir: &std::path::Path,
    bd: Option<std::path::PathBuf>,
) -> (smetana_core::health::Report, Source) {
    let asked = async {
        let client = tokio::time::timeout(Duration::from_secs(5), Client::connect(data_dir)).await.ok()?.ok()?;
        let value = client.call("server_health", json!({})).await.ok()?;
        serde_json::from_value::<smetana_core::health::Report>(value).ok()
    };
    let source = if let Some(report) = asked.await {
        return (report, Source::Server);
    } else if crate::local::path(data_dir).exists() {
        Source::Unanswered
    } else {
        Source::NoServer
    };
    let mut options = crate::server::Options::new(data_dir.to_path_buf());
    options.bd = bd;
    let ctx = crate::server::context(&options, std::sync::Arc::new(crate::server::NoSink));
    (smetana_core::health::report(&ctx).await, source)
}

/// Renders a health report for a terminal.
pub fn print_status(report: &smetana_core::health::Report, out: &mut impl Write) -> std::io::Result<()> {
    let tool = |out: &mut dyn Write, tool: &smetana_core::health::Tool| -> std::io::Result<()> {
        let state = if tool.answers { "ok " } else { "!! " };
        let detail = tool.version.clone().or_else(|| tool.problem.clone()).unwrap_or_default();
        let at = tool.path.as_deref().unwrap_or("not found");
        writeln!(out, "  {state}{:<8} {detail}  ({at})", tool.name)
    };
    writeln!(out, "bd (this build expects {}):", report.bd_expected)?;
    tool(out, &report.bd)?;
    writeln!(out, "git:")?;
    tool(out, &report.git)?;
    writeln!(out, "agents:")?;
    for agent in &report.agents {
        tool(out, &agent.binary)?;
    }
    if report.ok {
        writeln!(out, "\nEverything this server needs is here.")
    } else {
        writeln!(out, "\nProblems:")?;
        for problem in &report.problems {
            writeln!(out, "  - {problem}")?;
        }
        Ok(())
    }
}
