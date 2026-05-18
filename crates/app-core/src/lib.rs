//! Exports [editor] and [launcher] in the same format that the  `editor-core` &
//! `launcher-core` crates do, but the implementation comes from the dynamically
//! linked `app-core-dylib` crate.

use std::process::ExitCode;

/// Runs the editor portion of the app.
pub fn editor() -> ExitCode {
    i32_to_exit_code(unsafe { substrate_editor() })
}

/// Runs the launcher portion of the app.
pub fn launcher() -> ExitCode {
    i32_to_exit_code(unsafe { substrate_launcher() })
}

#[link(name = "app_core_dylib")]
unsafe extern "C" {
    /// Runs the editor portion of the app.
    ///
    /// A return value of `0` maps to [ExitCode::SUCCESS]. Anything else maps to
    /// [ExitCode::FAILURE].
    fn substrate_editor() -> i32;

    /// Runs the launcher portion of the app.
    ///
    /// A return value of `0` maps to [ExitCode::SUCCESS]. Anything else maps to
    /// [ExitCode::FAILURE].
    fn substrate_launcher() -> i32;
}

fn i32_to_exit_code(exit_code: i32) -> ExitCode {
    match exit_code {
        0 => ExitCode::SUCCESS,
        _ => ExitCode::FAILURE,
    }
}
