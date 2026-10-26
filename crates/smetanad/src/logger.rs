//! A logger that writes to standard error, because a service manager collects that and a server with
//! no terminal has nowhere else a line could go. Level from `SMETANAD_LOG` (`error`, `warn`, `info`,
//! `debug`, `trace`), `info` by default.

use log::{LevelFilter, Log, Metadata, Record};

struct Stderr;

impl Log for Stderr {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= log::max_level()
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        eprintln!("{} {:<5} {}", timestamp(), record.level(), record.args());
    }

    fn flush(&self) {}
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
