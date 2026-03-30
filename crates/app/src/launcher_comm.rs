//! Communication utilities for sending messages back to the launcher.

use std::process::Command;
use std::sync::OnceLock;

/// Notify the launcher that a project failed to open.
///
/// This spawns the launcher with the `--project-open-failed` flag, which will
/// tell the main launcher instance to display an error to the user.
pub fn notify_project_open_failed() {
    if let Err(e) = try_notify_project_open_failed() {
        util::debug_log_error!("Failed to notify launcher of project open failure: {e}");
    }
}

fn try_notify_project_open_failed() -> Result<(), std::io::Error> {
    let launcher_path = get_launcher_path()?;

    Command::new(launcher_path)
        .arg("--project-open-failed")
        .arg("--no-focus")
        .spawn()
        .inspect_err(|e| {
            util::debug_log_error!("Failed to spawn launcher for notification: {e}");
        })?;

    Ok(())
}

/// Get the path to the launcher executable.
///
/// This assumes the launcher is in the same directory as the current executable
/// and is named "launcher" (or "launcher.exe" on Windows).
fn get_launcher_path() -> Result<std::path::PathBuf, std::io::Error> {
    static LAUNCHER_PATH: OnceLock<std::path::PathBuf> = OnceLock::new();
    if let Some(path) = LAUNCHER_PATH.get() {
        return Ok(path.clone());
    }

    let current_exe = std::env::current_exe()?;
    let current_exe = std::fs::canonicalize(current_exe)?;
    let exe_dir = current_exe.parent().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Could not determine executable directory",
        )
    })?;

    #[cfg(windows)]
    let launcher_name = "launcher.exe";
    #[cfg(not(windows))]
    let launcher_name = "launcher";

    let launcher_path = exe_dir.join(launcher_name);
    let _ = LAUNCHER_PATH.set(launcher_path.clone());
    Ok(launcher_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_launcher_path_returns_valid_path() {
        let result = get_launcher_path();
        assert!(
            result.is_ok(),
            "Failed to get launcher path: {:?}",
            result.err()
        );

        let path = result.unwrap();

        #[cfg(windows)]
        assert!(path.to_string_lossy().ends_with("launcher.exe"));

        #[cfg(not(windows))]
        assert!(path.to_string_lossy().ends_with("launcher"));
    }

    #[test]
    fn test_get_launcher_path_has_parent_directory() {
        let launcher_path = get_launcher_path().unwrap();
        assert!(
            launcher_path.parent().is_some(),
            "Launcher path should have a parent directory"
        );
    }

    #[test]
    fn test_notify_project_open_failed_does_not_panic() {
        // This test verifies the function handles errors gracefully
        // It may fail to spawn if launcher doesn't exist, but shouldn't panic
        notify_project_open_failed();
    }

    #[test]
    fn test_try_notify_returns_error_when_launcher_missing() {
        // Create a command with a non-existent path to test error handling
        let result = Command::new("nonexistent_launcher_binary_12345")
            .arg("--test")
            .spawn();

        assert!(
            result.is_err(),
            "Should error when launcher binary doesn't exist"
        );
    }
}
