//! For finding and dealing with a user's local data (e.g. OS-specific paths to
//! local app data, and handling project data). See the [project] submodule.

pub mod project;

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use crate::version;

/// The path to the root of the app's data directory, unique for each user.
///
/// This value will only be computed the first time this function is called.
/// Once computed, subsequent calls are significantly cheaper.
///
/// The directory will be created if it doesn't exist.
pub fn root_path() -> &'static Path {
    static LOCAL_DATA: LazyLock<PathBuf> = LazyLock::new(|| {
        let mut local_data =
            PathBuf::from(env::var_os(LOCAL_DATA_ROOT_ENV_VAR).unwrap_or_else(|| {
                panic!("Environment variable `{LOCAL_DATA_ROOT_ENV_VAR}` should be set.")
            }));
        for dir in LOCAL_APP_DATA_SUFFIX {
            local_data.push(dir);
        }

        ensure_dirs_exist(&local_data);
        local_data
    });

    &LOCAL_DATA
}

/// The path to the directory where the app stores project data, unique for each
/// user.
///
/// This value will only be computed the first time this function is called.
/// Once computed, subsequent calls are significantly cheaper.
///
/// The directory will be created if it doesn't exist.
pub fn projects_path() -> &'static Path {
    static LOCAL_DATA_PROJECTS: LazyLock<PathBuf> = LazyLock::new(|| {
        let mut local_data_projects = PathBuf::from(root_path());
        local_data_projects.push(PROJECTS_DIR_NAME);

        ensure_dirs_exist(&local_data_projects);
        local_data_projects
    });

    &LOCAL_DATA_PROJECTS
}

const ROOT_DIR_NAME: &str = version::APP_NAME;
const PROJECTS_DIR_NAME: &str = "Projects";

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
compile_error!("Unsupported platform.");

#[cfg(target_os = "windows")]
const LOCAL_DATA_ROOT_ENV_VAR: &str = "LOCALAPPDATA";

#[cfg(any(target_os = "macos", target_os = "linux"))]
const LOCAL_DATA_ROOT_ENV_VAR: &str = "HOME";

#[cfg(target_os = "windows")]
const LOCAL_APP_DATA_SUFFIX: &[&str] = &[ROOT_DIR_NAME];

#[cfg(target_os = "macos")]
const LOCAL_APP_DATA_SUFFIX: &[&str] = &["Library", "Application Support", ROOT_DIR_NAME];

#[cfg(target_os = "linux")]
const LOCAL_APP_DATA_SUFFIX: &[&str] = &[".local", "share", ROOT_DIR_NAME];

fn ensure_dirs_exist(path: &Path) {
    if !path.exists() {
        fs::create_dir_all(path).expect("Creating local data dirs shouldn't fail.");
    }
}
