//! A logger that writes to standard error, because a service manager collects that and a server with
//! no terminal has nowhere else a line could go. Level from `SMETANAD_LOG` (`error`, `warn`, `info`,
//! `debug`, `trace`), `info` by default. The iroh stack is capped at `warn` whatever that says short of
//! `debug`: at `info` it writes a line per packet, which buried every line this server wrote itself.

use log::{LevelFilter, Log, Metadata, Record};

struct Stderr;

impl Log for Stderr {
    fn enabled(&self, metadata: &Metadata) -> bool {
        passes(metadata.target(), None, metadata.level(), log::max_level())
    }

    fn log(&self, record: &Record) {
        if !passes(record.target(), record.module_path(), record.level(), log::max_level()) {
            return;
        }
        eprintln!("{} {:<5} {}", timestamp(), record.level(), record.args());
    }

    fn flush(&self) {}
}

/// Crates whose `info` is per-packet chatter. Names as they appear in a `log` target, so with
/// underscores; each is matched as a whole path component, which is why `iroh` does not cover
/// `iroh_relay` and both are listed. The shell's own logger in `src-tauri/src/lib.rs` keeps the same list.
const NOISY: &[&str] = &[
    "iroh",
    "iroh_base",
    "iroh_dns",
    "iroh_metrics",
    "iroh_relay",
    "n0_dns_resolver",
    "n0_future",
    "n0_watcher",
    "netwatch",
    "noq",
    "noq_proto",
    "noq_udp",
    "portmapper",
    "quinn",
    "quinn_proto",
    "quinn_udp",
];

/// `tracing` is built with its `log` feature, which turns every span into a record whose target is
/// `tracing::span` or `tracing::span::active` and whose *module path* names the crate that opened
/// it. So `poll_send` and `handle_message` arrive with a target that says nothing about iroh.
const SPAN_TARGETS: &[&str] = &["tracing::span", "tracing::span::active"];

fn within(path: &str, root: &str) -> bool {
    path.strip_prefix(root).is_some_and(|rest| rest.is_empty() || rest.starts_with("::"))
}

fn is_noisy(target: &str, module_path: Option<&str>) -> bool {
    let named = |path: &str| NOISY.iter().any(|root| within(path, root));
    if SPAN_TARGETS.contains(&target) {
        // A span with no module path cannot be attributed; it is capped too, since nothing of this
        // server's own opens one.
        return module_path.map_or(true, named);
    }
    named(target)
}

/// Whether a record is written, given the level the person configured: the configured level for
/// everything, except that the iroh stack never goes above `warn` unless the person asked for
/// `debug` or `trace`, when it is meant to show.
fn passes(target: &str, module_path: Option<&str>, level: log::Level, configured: LevelFilter) -> bool {
    if level > configured {
        return false;
    }
    configured >= LevelFilter::Debug || level <= log::Level::Warn || !is_noisy(target, module_path)
}

/// Seconds since the Unix epoch: no calendar to get wrong, and `date -d @N` reads it.
fn timestamp() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or(0);
    format!("{secs}")
}

pub fn init() {
    let level = match std::env::var("SMETANAD_LOG").ok().as_deref() {
        Some("error") => LevelFilter::Error,
        Some("warn") => LevelFilter::Warn,
        Some("debug") => LevelFilter::Debug,
        Some("trace") => LevelFilter::Trace,
        _ => LevelFilter::Info,
    };
    if log::set_logger(&Stderr).is_ok() {
        log::set_max_level(level);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use log::Level::{Debug, Error, Info, Trace, Warn};

    #[test]
    fn iroh_info_is_dropped_at_the_default_level() {
        assert!(!passes("iroh::socket", None, Info, LevelFilter::Info));
        assert!(!passes("iroh_relay::client", None, Info, LevelFilter::Info));
        assert!(!passes("noq_proto::connection", None, Info, LevelFilter::Info));
        assert!(!passes("netwatch", None, Info, LevelFilter::Info));
    }

    #[test]
    fn iroh_warnings_and_errors_still_pass() {
        assert!(passes("iroh::socket", None, Warn, LevelFilter::Info));
        assert!(passes("portmapper::mapping", None, Error, LevelFilter::Info));
    }

    #[test]
    fn our_own_info_passes() {
        assert!(passes("smetanad::server", None, Info, LevelFilter::Info));
        assert!(passes("smetana_remote_server", None, Info, LevelFilter::Info));
        // A name that merely starts with a capped one is not that crate.
        assert!(passes("irohish::thing", None, Info, LevelFilter::Info));
    }

    #[test]
    fn a_bridged_span_is_judged_by_the_crate_that_opened_it() {
        assert!(!passes("tracing::span", Some("iroh::socket::remote_map"), Info, LevelFilter::Info));
        assert!(!passes("tracing::span::active", Some("noq::endpoint"), Info, LevelFilter::Info));
        assert!(!passes("tracing::span", None, Info, LevelFilter::Info));
        assert!(passes("tracing::span", Some("smetanad::server"), Info, LevelFilter::Info));
    }

    #[test]
    fn debug_and_trace_show_the_iroh_stack() {
        assert!(passes("iroh::socket", None, Debug, LevelFilter::Debug));
        assert!(passes("iroh::socket", None, Info, LevelFilter::Debug));
        assert!(passes("tracing::span", Some("iroh::socket"), Trace, LevelFilter::Trace));
    }

    #[test]
    fn the_configured_level_still_applies_to_everything() {
        assert!(!passes("smetanad::server", None, Info, LevelFilter::Warn));
        assert!(!passes("iroh::socket", None, Warn, LevelFilter::Error));
        assert!(!passes("smetanad::server", None, Debug, LevelFilter::Info));
    }
}
