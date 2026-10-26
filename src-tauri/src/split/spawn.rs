//! Finding the local server, and starting one when there is none.
//!
//! The server is `smetanad`, shipped beside `bd` (`bundle.externalBin`), run as a **separate process
//! in a group of its own**, so that closing the window, quitting the app or a Ctrl-C in the terminal
//! that ran `tauri dev` leaves it running. It has its own `--data-dir`, which the shell passes and the
//! server never guesses.
//!
//! "Is there one" is asked of the data directory's lock (`smetanad::server::is_owned`), not of the
//! network: a lock is released the instant its process is gone however it went, where an address
//! left in `local-pairing.json` by a crash answers nothing for as long as a connection attempt takes
//! to give up.

use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::mpsc::{channel, Receiver, TryRecvError};
use std::time::{Duration, Instant};

use smetanad::{local, server};

/// How long a freshly started server has to publish its credential.
const START_WAIT: Duration = Duration::from_secs(30);
/// How long a server that already owns the directory has to do the same: it is on its way up.
const OWNED_WAIT: Duration = Duration::from_secs(10);
const POLL: Duration = Duration::from_millis(100);

/// Where everything of the local placement is.
#[derive(Debug, Clone)]
pub struct Config {
    /// The server's data directory: the one `--data-dir` names, and where its credential is.
    pub server_dir: PathBuf,
    /// This client's own key and paired marker (`local-client-key`, `local-client-paired`).
    pub identity_dir: PathBuf,
    /// The `smetanad` to start. `None` when the bundle has none, which is a build mistake and is
    /// reported as the reason the connection cannot be made.
    pub exe: Option<PathBuf>,
    /// The `bd` the server is told to use.
    pub bd: Option<PathBuf>,
    /// The skill library the bundle ships.
    pub resource_dir: Option<PathBuf>,
    /// Where the server's standard output and error go.
    pub log: PathBuf,
    /// This device's own `settings.json`: the file an older single-file build left, whose server
    /// half is handed to the server once (`migrate.rs`).
    pub settings: Option<PathBuf>,
}

/// The name of a sidecar next to this executable. Tauri puts `externalBin` entries there under their
/// plain names, with `.exe` on Windows; `cargo`'s `deps` directory is stepped out of the way so a
/// test binary finds them too.
pub fn sidecar(name: &str) -> Option<PathBuf> {
    let exe = tauri::utils::platform::current_exe().ok()?;
    let dir = exe.parent()?;
    let dir = if dir.ends_with("deps") { dir.parent().unwrap_or(dir) } else { dir };
    let mut path = dir.join(name);
    if cfg!(windows) {
        path.as_mut_os_string().push(".exe");
    }
    Some(path)
}

/// The arguments the server is started with, in the order the command line reads them.
pub fn arguments(config: &Config) -> Vec<std::ffi::OsString> {
    let mut args: Vec<std::ffi::OsString> = vec!["run".into(), "--data-dir".into(), config.server_dir.clone().into()];
    if let Some(bd) = &config.bd {
        args.push("--bd".into());
        args.push(bd.clone().into());
    }
    if let Some(resources) = &config.resource_dir {
        args.push("--resource-dir".into());
        args.push(resources.clone().into());
    }
    args
}

/// Starts the server and returns what will say it has ended.
fn start(config: &Config) -> Result<Receiver<ExitStatus>, String> {
    let exe = config
        .exe
        .as_ref()
        .filter(|exe| exe.exists())
        .ok_or_else(|| "the server (smetanad) is not in this build; run `npm run build-smetanad`".to_owned())?;
    if let Some(parent) = config.log.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&config.log)
        .map_err(|err| format!("{}: {err}", config.log.display()))?;
    let log_too = log.try_clone().map_err(|err| err.to_string())?;
    let mut command = Command::new(exe);
    command.args(arguments(config)).stdin(Stdio::null()).stdout(log).stderr(log_too);
    detach(&mut command);
    let mut child = command.spawn().map_err(|err| format!("{}: {err}", exe.display()))?;
    // Waited for on a thread of its own: a child nobody waits for stays a zombie until this process
    // ends, and the answer is what tells a server that died at start from one that is just slow.
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        if let Ok(status) = child.wait() {
            let _ = tx.send(status);
        }
    });
    Ok(rx)
}

/// Puts the child outside this process's group and console, so a signal aimed at the app does not
/// reach it.
fn detach(command: &mut Command) {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW
        command.creation_flags(0x0000_0008 | 0x0000_0200 | 0x0800_0000);
    }
}

/// Waits for a credential that is not `stale` and a lock that is held: a server that is up and has
/// published *this* start's secret.
fn wait_for_credential(
    dir: &Path,
    stale: Option<local::Credential>,
    within: Duration,
    mut dead: impl FnMut() -> Option<String>,
) -> Result<(), String> {
    let until = Instant::now() + within;
    loop {
        if let Ok(found) = local::read(dir) {
            if Some(&found) != stale.as_ref() && server::is_owned(dir) {
                return Ok(());
            }
        }
        if let Some(why) = dead() {
            return Err(why);
        }
        if Instant::now() >= until {
            return Err(format!("the server did not publish its credential within {} seconds", within.as_secs()));
        }
        std::thread::sleep(POLL);
    }
}

/// Makes sure a server owns the data directory and has published its credential, starting one if
/// none does. Blocking, for up to [`START_WAIT`]; the caller runs it off the async runtime.
pub fn ensure(config: &Config) -> Result<(), String> {
    if server::is_owned(&config.server_dir) {
        // On its way up, or restarting itself: it will write the credential in a moment.
        return wait_for_credential(&config.server_dir, None, OWNED_WAIT, || None);
    }
    // Nothing owns it, so whatever credential is there is a crash's leftover and names an address
    // nobody listens on. It is not removed, which could delete the file of a server that started in
    // that instant; it is remembered, and the start is over when a different one appears.
    let stale = local::read(&config.server_dir).ok();
    let exited = start(config)?;
    let log = config.log.display().to_string();
    wait_for_credential(&config.server_dir, stale, START_WAIT, move || match exited.try_recv() {
        Ok(status) => Some(format!("the server ended at start ({status}); its log is {log}")),
        Err(TryRecvError::Empty) => None,
        Err(TryRecvError::Disconnected) => Some(format!("the server ended at start; its log is {log}")),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> Config {
        Config {
            server_dir: "/data/server".into(),
            identity_dir: "/data".into(),
            exe: Some("/bundle/smetanad".into()),
            bd: Some("/bundle/bd".into()),
            resource_dir: Some("/bundle/resources".into()),
            log: "/data/smetanad.log".into(),
            settings: None,
        }
    }

    #[test]
    fn the_server_is_started_with_its_own_data_directory_and_the_bundles_tools() {
        let args: Vec<String> = arguments(&config()).iter().map(|a| a.to_string_lossy().into_owned()).collect();
        assert_eq!(
            args,
            ["run", "--data-dir", "/data/server", "--bd", "/bundle/bd", "--resource-dir", "/bundle/resources"]
        );
    }

    #[test]
    fn what_the_bundle_does_not_ship_is_left_off() {
        let mut config = config();
        config.bd = None;
        config.resource_dir = None;
        assert_eq!(arguments(&config).len(), 3);
    }

    #[test]
    fn a_missing_server_binary_is_a_reason_and_not_a_panic() {
        let dir = tempfile::tempdir().unwrap();
        let mut config = config();
        config.server_dir = dir.path().join("server");
        config.exe = Some(dir.path().join("no-such-smetanad"));
        config.log = dir.path().join("smetanad.log");
        let err = ensure(&config).unwrap_err();
        assert!(err.contains("not in this build"), "{err}");
        config.exe = None;
        assert!(ensure(&config).unwrap_err().contains("not in this build"));
    }

    #[cfg(unix)]
    #[test]
    fn a_server_that_ends_at_start_is_reported_with_its_log() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("smetanad");
        std::fs::write(&exe, "#!/bin/sh\necho refused >&2\nexit 3\n").unwrap();
        std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755)).unwrap();
        let mut config = config();
        config.server_dir = dir.path().join("server");
        config.exe = Some(exe);
        config.log = dir.path().join("smetanad.log");
        let err = ensure(&config).unwrap_err();
        assert!(err.contains("ended at start") && err.contains("smetanad.log"), "{err}");
        assert!(std::fs::read_to_string(&config.log).unwrap().contains("refused"), "its stderr is in the log");
    }
}
