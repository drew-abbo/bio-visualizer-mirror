//! Exports [editor] and [launcher] in the same format that the  `editor-core` &
//! `launcher-core` crates do, but the implementation comes from the dynamically
//! linked `app-core-dylib` crate.

use std::process::ExitCode;

/// Runs the editor portion of the app.
pub fn editor() -> ExitCode {
    i32_to_exit_code(unsafe { app_core__editor() })
}

/// Runs the launcher portion of the app.
pub fn launcher() -> ExitCode {
    i32_to_exit_code(unsafe { app_core__launcher() })
}

// Defined in `app-core-dylib`.
#[link(name = "app_core_dylib")]
unsafe extern "C" {
    fn app_core__editor() -> i32;

    fn app_core__launcher() -> i32;
}

fn i32_to_exit_code(exit_code: i32) -> ExitCode {
    match exit_code {
        0 => ExitCode::SUCCESS,
        _ => ExitCode::FAILURE,
    }
}
