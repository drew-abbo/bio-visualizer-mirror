//! Defines constants related to the app's version.

/// The name of the app.
pub const APP_NAME: &str = "Substrate";

/// The version of the app.
pub const APP_VERSION: &str = include_str!("../../../version").trim_ascii();
