// Prevents an additional console window on Windows in release; harmless elsewhere.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    smetana_companion_lib::run()
}
