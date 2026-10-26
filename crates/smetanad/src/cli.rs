//! The command line. Every subcommand takes `--data-dir`: the server never guesses where it lives, and
//! the subcommands that talk to a running server find it by the credential in that same directory.

use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

#[derive(Debug, Parser)]
#[command(name = "smetanad", version, about = "The headless Smetana server")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Kind {
    Desktop,
    Phone,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Run the server in the foreground (a service manager keeps it alive).
    Run {
        /// Where the server keeps everything. Never guessed.
        #[arg(long)]
        data_dir: PathBuf,
        /// The bd binary. Default: the one beside this executable, then the one on PATH.
        #[arg(long)]
        bd: Option<PathBuf>,
        /// The directory the skill library ships in.
        #[arg(long)]
        resource_dir: Option<PathBuf>,
    },
    /// Print a pairing ticket as text and as a QR code, and confirm the pairing here.
    Pair {
        #[arg(long)]
        data_dir: PathBuf,
        /// What the device that spends the ticket may do.
        #[arg(long, value_enum)]
        kind: Kind,
        /// The relay the ticket sends the device through. Default: the server's setting.
        #[arg(long)]
        relay: Option<String>,
    },
    /// List the paired devices.
    Devices {
        #[arg(long)]
        data_dir: PathBuf,
    },
    /// Revoke a device; its connection closes at once. The id is the key shown by `devices`, or a
    /// prefix of it that names one device.
    Revoke {
        #[arg(long)]
        data_dir: PathBuf,
        id: String,
    },
    /// Report on the environment: bd, git, and each agent's binary and whether it answers.
    Status {
        #[arg(long)]
        data_dir: PathBuf,
        #[arg(long)]
        bd: Option<PathBuf>,
        /// Print the report as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Restart the server, now or when its work has finished. Nothing is ended without a yes.
    Restart {
        #[arg(long)]
        data_dir: PathBuf,
        /// Wait until no run, session or shell is alive, then restart. Ends nothing.
        #[arg(long, conflicts_with_all = ["cancel", "yes"])]
        when_idle: bool,
        /// Withdraw a restart that was waiting for the work to finish.
        #[arg(long, conflicts_with = "yes")]
        cancel: bool,
        /// Restart now even if work is alive, without asking.
        #[arg(long)]
        yes: bool,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn the_command_line_is_well_formed() {
        Cli::command().debug_assert();
    }

    #[test]
    fn run_cannot_be_started_without_a_data_directory() {
        assert!(Cli::try_parse_from(["smetanad", "run"]).is_err());
        let parsed = Cli::try_parse_from(["smetanad", "run", "--data-dir", "/srv/smetana"]).unwrap();
        assert!(matches!(parsed.command, Command::Run { .. }));
    }

    #[test]
    fn pair_names_the_kind_it_admits() {
        assert!(Cli::try_parse_from(["smetanad", "pair", "--data-dir", "/d"]).is_err());
        assert!(Cli::try_parse_from(["smetanad", "pair", "--data-dir", "/d", "--kind", "tablet"]).is_err());
        assert!(Cli::try_parse_from(["smetanad", "pair", "--data-dir", "/d", "--kind", "phone"]).is_ok());
    }
}
