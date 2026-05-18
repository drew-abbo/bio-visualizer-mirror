#![cfg_attr(all(windows, feature = "build-package"), windows_subsystem = "windows")]

use std::process::ExitCode;

use launcher_core::launcher;

fn main() -> ExitCode {
    launcher()
}
