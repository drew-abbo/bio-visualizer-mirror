#![cfg_attr(
    all(windows, feature = "no-windows-console"),
    windows_subsystem = "windows"
)]
#[cfg(all(feature = "link-static", feature = "link-dylib"))]
compile_error!("Incompatible features `link-static` and `link-dylib`.");

use std::process::ExitCode;

#[cfg(feature = "link-static")]
use launcher_core::launcher;
#[cfg(feature = "link-dylib")]
use app_core::launcher;

fn main() -> ExitCode {
    launcher()
}
