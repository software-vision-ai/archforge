//! Archforge: the desktop application's binary, a thin start over its library (`archforge`).
#![cfg_attr(windows, windows_subsystem = "windows")]

/// Starts the application and ends the process with its exit code.
fn main() {
    std::process::exit(archforge::start());
}
