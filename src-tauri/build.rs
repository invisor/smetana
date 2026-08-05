fn main() {
  // tauri_build copies bundle.resources into target/<profile>/resources, which
  // is what makes resource_dir() answer under `tauri dev`. Cargo reruns a build
  // script only when something the script itself declared has changed, and the
  // resource files are not on tauri_build's list — so adding a skill left the
  // dev build handing agents yesterday's copy of the library. The session then
  // starts, is told to use a skill by name, and answers "Unknown skill": a
  // failure with nothing on either side to say the file simply never arrived.
  //
  // Cargo walks a directory named here, so one line covers both plugins. This
  // does not switch off the default watch-the-whole-package behaviour either —
  // tauri_build already emits directives of its own, so that was off already.
  println!("cargo:rerun-if-changed=resources");
  tauri_build::build()
}
