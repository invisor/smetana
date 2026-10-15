//! Things that must stay equal between this crate and the rest of the repository,
//! and that nothing else would notice: each one builds green and fails only at
//! run time, through a relay, on a phone.

use std::fs;
use std::path::PathBuf;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read(path: &str) -> String {
    fs::read_to_string(root().join(path)).unwrap_or_else(|err| panic!("{path}: {err}"))
}

/// The version of `package` that a `Cargo.lock` pins.
fn locked(lock: &str, package: &str) -> String {
    let mut lines = lock.lines();
    while let Some(line) = lines.next() {
        if line == format!("name = \"{package}\"") {
            let version = lines.next().expect("a version line");
            return version.trim_start_matches("version = ").trim_matches('"').to_owned();
        }
    }
    panic!("{package} is not in the lock file");
}

#[test]
fn iroh_is_the_same_version_as_on_the_desktop_and_in_the_shared_crate() {
    let mobile = locked(&read("mobile/src-tauri/Cargo.lock"), "iroh");
    assert_eq!(mobile, locked(&read("src-tauri/Cargo.lock"), "iroh"), "desktop");
    assert_eq!(mobile, locked(&read("crates/remote-protocol/Cargo.lock"), "iroh"), "remote-protocol");
    assert!(
        read("server/Dockerfile").contains(&format!("IROH_RELAY_VERSION={mobile}")),
        "the relay image is on another iroh than {mobile}"
    );
}

#[test]
fn the_protocol_major_is_the_desktops() {
    let desktop = read("src-tauri/src/remote/mod.rs");
    let line = |name: &str| -> u16 {
        let marker = format!("pub const {name}: u16 = ");
        let rest = &desktop[desktop.find(&marker).expect("constant") + marker.len()..];
        rest[..rest.find(';').unwrap()].trim().parse().unwrap()
    };
    assert_eq!(line("PROTOCOL_MAJOR"), 1);
    assert_eq!(line("PROTOCOL_MAJOR"), smetana_companion_lib::PROTOCOL_MAJOR);
    assert!(
        smetana_companion_lib::PROTOCOL_MINOR <= line("PROTOCOL_MINOR"),
        "this build knows a protocol minor the desktop in this tree does not"
    );
}

#[test]
fn the_app_version_is_one_number_everywhere() {
    let conf: serde_json::Value = serde_json::from_str(&read("mobile/src-tauri/tauri.conf.json")).unwrap();
    let package: serde_json::Value = serde_json::from_str(&read("mobile/package.json")).unwrap();
    assert_eq!(conf["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(package["version"], env!("CARGO_PKG_VERSION"));
}

#[test]
fn the_desktops_minimum_mobile_version_admits_this_build() {
    let desktop = read("src-tauri/src/remote/mod.rs");
    let marker = "pub const MIN_MOBILE_VERSION: &str = \"";
    let rest = &desktop[desktop.find(marker).unwrap() + marker.len()..];
    let min = &rest[..rest.find('"').unwrap()];
    assert!(!smetana_companion_lib::older_than(env!("CARGO_PKG_VERSION"), min), "{min}");
}
