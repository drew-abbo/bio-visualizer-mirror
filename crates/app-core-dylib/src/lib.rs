//! Re-exports `editor` and `launcher` from the `editor-core` & `launcher-core`
//! crates as C ABI symbols [substrate_editor] and [substrate_launcher].

use std::process::ExitCode;

/// Runs the editor portion of the app.
///
/// A return value of `0` maps to [ExitCode::SUCCESS]. Anything else maps to
/// [ExitCode::FAILURE].
#[unsafe(no_mangle)]
pub extern "C" fn substrate_editor() -> i32 {
    exit_code_to_i32(editor_core::editor())
}

/// Runs the launcher portion of the app.
///
/// A return value of `0` maps to [ExitCode::SUCCESS]. Anything else maps to
/// [ExitCode::FAILURE].
#[unsafe(no_mangle)]
pub extern "C" fn substrate_launcher() -> i32 {
    exit_code_to_i32(launcher_core::launcher())
}

fn exit_code_to_i32(exit_code: ExitCode) -> i32 {
    match exit_code {
        ExitCode::SUCCESS => 0,
        _ => 1,
    }
}
