//! Communication utilities for sending messages back to the launcher.

use std::process::Command;

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
    let current_exe = std::env::current_exe()?;
    let exe_dir = current_exe
        .parent()
        .ok_or_else(|| std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Could not determine executable directory",
        ))?;
    
    #[cfg(windows)]
    let launcher_name = "launcher.exe";
    #[cfg(not(windows))]
    let launcher_name = "launcher";
    
    Ok(exe_dir.join(launcher_name))
}

/// Notify the launcher that a project was updated and needs rescanning.
/// 
/// This is useful after saving project data.
pub fn notify_project_updated() {
    if let Err(e) = try_notify_project_updated() {
        util::debug_log_error!("Failed to notify launcher of project update: {e}");
    }
}

fn try_notify_project_updated() -> Result<(), std::io::Error> {
    let launcher_path = get_launcher_path()?;
    
    Command::new(launcher_path)
        .arg("--rescan-projects")
        .arg("--no-focus")
        .spawn()
        .inspect_err(|e| {
            util::debug_log_error!("Failed to spawn launcher for notification: {e}");
        })?;
    
    Ok(())
}
