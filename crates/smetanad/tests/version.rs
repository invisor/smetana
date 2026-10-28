//! The server reports the app's version, not its own crate's.

#[test]
fn server_version_is_the_app_version() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../src-tauri/tauri.conf.json");
    let json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(path).expect("read tauri.conf.json"))
            .expect("parse tauri.conf.json");
    assert_eq!(smetanad::VERSION, json["version"].as_str().expect("string version"));
    assert_ne!(smetanad::VERSION, env!("CARGO_PKG_VERSION"));
}
