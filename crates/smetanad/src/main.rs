use std::io::{BufRead, Write};
use std::process::ExitCode;

use clap::Parser;
use remote_protocol::DeviceKind;
use remote_server::gateway::Mode;
use smetanad::cli::{Cli, Command, Kind};
use smetanad::client::Client;
use smetanad::server::{self, Exit, Options, Server};
use smetanad::{logger, ops};

/// A yes/no question on the terminal. Anything but `y` or `yes` is no, and so is a closed input: a
/// server must never be told yes by silence.
fn ask(question: &str) -> bool {
    let mut stdout = std::io::stdout();
    let _ = write!(stdout, "{question} [y/N] ");
    let _ = stdout.flush();
    let mut line = String::new();
    if std::io::stdin().lock().read_line(&mut line).is_err() {
        return false;
    }
    matches!(line.trim().to_lowercase().as_str(), "y" | "yes")
}

fn fail(message: impl std::fmt::Display) -> ExitCode {
    eprintln!("smetanad: {message}");
    ExitCode::from(1)
}

#[tokio::main]
async fn main() -> ExitCode {
    logger::init();
    let cli = Cli::parse();
    match cli.command {
        Command::Run { data_dir, bd, resource_dir, inherit_from } => run(data_dir, bd, resource_dir, inherit_from).await,
        Command::Pair { data_dir, kind, relay } => {
            let mut client = match Client::connect(&data_dir).await {
                Ok(client) => client,
                Err(err) => return fail(err),
            };
            let relay = relay.unwrap_or_else(|| {
                smetana_core::settings::file::load(&data_dir.join("settings.json")).0.remote.relay_url
            });
            let kind = match kind {
                Kind::Desktop => DeviceKind::Desktop,
                Kind::Phone => DeviceKind::Phone,
            };
            let confirm = |name: &str| ask(&format!("\n{name} wants to pair. Allow it?"));
            match ops::pair(&mut client, kind, &relay, confirm, &mut std::io::stdout()).await {
                Ok(ops::Paired::Accepted(name)) => {
                    println!("Paired {name}.");
                    ExitCode::SUCCESS
                }
                Ok(ops::Paired::Declined) => fail("the pairing was declined"),
                Ok(ops::Paired::Expired) => fail("the ticket expired before a device used it"),
                Err(err) => fail(err),
            }
        }
        Command::Devices { data_dir } => with_client(&data_dir, |client| async move {
            let rows = ops::devices(&client).await?;
            ops::print_devices(&rows, &mut std::io::stdout()).map_err(|err| err.to_string())
        })
        .await,
        Command::Revoke { data_dir, id } => {
            with_client(&data_dir, |client| async move { ops::revoke(&client, &id, &mut std::io::stdout()).await }).await
        }
        Command::Status { data_dir, bd, json } => {
            let (report, source) = ops::status(&data_dir, bd).await;
            if json {
                let mut value = serde_json::to_value(&report).unwrap_or_default();
                value["source"] = serde_json::json!(source.word());
                println!("{}", serde_json::to_string_pretty(&value).unwrap_or_default());
            } else {
                println!("({})\n", source.describe());
                let _ = ops::print_status(&report, &mut std::io::stdout());
            }
            if report.ok { ExitCode::SUCCESS } else { ExitCode::from(1) }
        }
        Command::Restart { data_dir, when_idle, cancel, yes } => {
            let mode = if when_idle {
                Mode::WhenIdle
            } else if cancel {
                Mode::Cancel
            } else {
                Mode::Now
            };
            with_client(&data_dir, |client| async move {
                ops::restart(&client, mode, yes, ask, &mut std::io::stdout()).await.map(|_| ())
            })
            .await
        }
    }
}

async fn with_client<F, Fut>(data_dir: &std::path::Path, body: F) -> ExitCode
where
    F: FnOnce(Client) -> Fut,
    Fut: std::future::Future<Output = Result<(), String>>,
{
    match Client::connect(data_dir).await {
        Ok(client) => match body(client).await {
            Ok(()) => ExitCode::SUCCESS,
            Err(err) => fail(err),
        },
        Err(err) => fail(err),
    }
}

async fn run(
    data_dir: std::path::PathBuf,
    bd: Option<std::path::PathBuf>,
    resource_dir: Option<std::path::PathBuf>,
    inherit_from: Option<std::path::PathBuf>,
) -> ExitCode {
    if let Some(euid) = server::effective_uid() {
        if let Err(message) = server::refuse_root(euid) {
            return fail(message);
        }
    }
    let mut options = Options::new(data_dir);
    options.bd = bd;
    options.resource_dir = resource_dir;
    options.inherit_from = inherit_from;
    let server = match Server::start(options).await {
        Ok(server) => server,
        Err(err) => return fail(err),
    };
    let exit = server.wait().await;
    log::info!("stopping ({exit:?})");
    server.shutdown().await;
    if exit == Exit::Restart {
        return restart_self();
    }
    ExitCode::SUCCESS
}

/// Replaces this process with a fresh one running the same command. A restart that ended every child
/// and then did not come back would leave a server down because somebody asked for a restart, so where
/// the platform cannot do this the exit code (75, "try again") is the signal for a service manager to.
fn restart_self() -> ExitCode {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        match std::env::current_exe() {
            Ok(exe) => {
                let err = std::process::Command::new(exe).args(std::env::args_os().skip(1)).exec();
                fail(format!("could not start again: {err}"))
            }
            Err(err) => fail(format!("could not find this executable to start again: {err}")),
        }
    }
    #[cfg(not(unix))]
    {
        ExitCode::from(75)
    }
}
