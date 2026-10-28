// The server's version is the app's, and the app's lives in `src-tauri/tauri.conf.json` — the one file a
// release bumps. The crate's own `CARGO_PKG_VERSION` stays 0.1.0 forever, which made the version lock-step
// compare two equal strings. Failing the build is deliberate: a server that does not know its version is
// exactly that bug.
fn main() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../src-tauri/tauri.conf.json");
    println!("cargo:rerun-if-changed=../../src-tauri/tauri.conf.json");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    let json: serde_json::Value = serde_json::from_str(&text)
        .unwrap_or_else(|e| panic!("cannot parse {}: {e}", path.display()));
    let version = json
        .get("version")
        .and_then(|v| v.as_str())
        .unwrap_or_else(|| panic!("{} has no string `version`", path.display()));
    println!("cargo:rustc-env=SMETANA_APP_VERSION={version}");
}
