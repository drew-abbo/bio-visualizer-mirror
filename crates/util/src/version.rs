//! Defines constants related to the app's version.

mod prelude;

pub use prelude::*;

/// The name of the app.
pub const APP_NAME: &str = "Bio Visualizer";

/// The version of the app.
pub const APP_VERSION: Version = crate::version_const!("0.1.0");
