//! Where this build keeps what it keeps: the data directory and the config directory.
//!
//! A release keeps them where Tauri says, under the bundle identifier. A **development build does
//! not**: `npm run tauri dev` runs under the same identifier as the installed release
//! (`tauri.conf.json`), so Tauri's answer is the release's own directory, and the first start of a
//! development build would spawn a local server that inherits the release's device key and paired
//! phones (`--inherit-from`) and would relieve the release's `settings.json` of its server half
//! (`client/migrate.rs`). The design says that migration is tested on a copy and never on the live
//! directory; until the switch-over the separate "Smetana Next" identifier kept the two apart, and
//! this is what keeps them apart now. A development build lives in a sibling directory named after
//! the identifier with `.dev` on the end, starts empty, inherits nothing and migrates nothing
//! (`client::config`).
//!
//! Every reader of the data or config directory in this shell goes through here. The two plugins
//! that keep files of their own under the identifier (the window-state and log plugins) do not, and
//! need not: neither holds anything the server or the migration reads.

use std::path::{Path, PathBuf};

use tauri::{AppHandle, Manager};

/// Whether this is a development build, which keeps its data apart from the release's.
pub const DEVELOPMENT: bool = cfg!(debug_assertions);

/// The directory a development build uses in place of the release's `dir`: a sibling of it, named
/// after it with `.dev` added. Never inside it, so nothing that walks the release's directory (the
/// inheritance above all) can find a development build's files there.
pub fn development_dir(dir: &Path) -> PathBuf {
    let mut name = dir.file_name().map(|name| name.to_os_string()).unwrap_or_else(|| "smetana".into());
    name.push(".dev");
    match dir.parent() {
        Some(parent) => parent.join(name),
        None => PathBuf::from(name),
    }
}

fn for_this_build(dir: PathBuf) -> PathBuf {
    if DEVELOPMENT {
        development_dir(&dir)
    } else {
        dir
    }
}

/// The app's data directory: the server's directory, this client's identity and the server choice.
pub fn data_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path().app_data_dir().map(for_this_build).map_err(|err| err.to_string())
}

/// The app's config directory: where the device's `settings.json` is.
pub fn config_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path().app_config_dir().map(for_this_build).map_err(|err| err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_development_build_keeps_a_sibling_of_the_release_directory_and_never_the_directory_itself() {
        let release = Path::new("/Users/me/Library/Application Support/com.invisor.smetana");
        let dev = development_dir(release);
        assert_eq!(dev, Path::new("/Users/me/Library/Application Support/com.invisor.smetana.dev"));
        assert!(!dev.starts_with(release), "nothing of the development build is inside the release's directory");
        assert!(!release.starts_with(&dev));
    }

    #[test]
    fn the_test_build_is_a_development_build() {
        // `cargo test` builds with debug assertions, as `npm run tauri dev` does: the switch that keeps
        // a development build away from the release's data is on wherever a test can see it.
        assert!(DEVELOPMENT);
    }
}
