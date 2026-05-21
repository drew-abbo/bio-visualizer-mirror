//! Defines constants related to the app's version.

use std::fs;
use std::io::{self, Write};
use std::path::Path;

/// The name of the app.
pub const APP_NAME: &str = "Substrate";

/// The version of the app.
pub const APP_VERSION: &str = env!("APP_VERSION");

/// Prints version info to a new file at `file_path` (truncated if it doesn't
/// exist). [stdout](io::stdout) is used if `file` isn't provided.
///
/// The [APP_NAME] and [APP_VERSION] (space-separated) is printed with a
/// trailing newline (e.g. `Substrate 1.2.3`).
pub fn print(file_path: Option<impl AsRef<Path>>) -> Result<(), io::Error> {
    let version_str = format!("{APP_NAME} {APP_VERSION}\n");
    match file_path {
        Some(file_path) => fs::write(file_path, version_str),
        None => io::stdout().write_all(version_str.as_ref()),
    }
}
