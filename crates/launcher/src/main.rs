#![cfg_attr(
    all(windows, feature = "no-windows-console"),
    windows_subsystem = "windows"
)]

use std::process::ExitCode;

#[cfg(all(feature = "link-static", feature = "link-dylib"))]
compile_error!("Incompatible features `link-static` and `link-dylib`.");
#[cfg(feature = "link-dylib")]
use app_core as launcher_core;

fn main() -> ExitCode {
    launcher_core::launcher()
}
