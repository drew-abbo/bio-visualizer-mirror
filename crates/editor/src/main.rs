#![cfg_attr(all(windows, feature = "build-package"), windows_subsystem = "windows")]

use std::process::ExitCode;

use editor_core::editor;

fn main() -> ExitCode {
    editor()
}
