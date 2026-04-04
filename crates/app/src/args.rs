//! Contains [Args], which are parsed command-line flags.

use clap::Parser;

/// Parsed command line arguments.
#[derive(Parser, Debug, Clone, PartialEq, Eq, Hash)]
#[command(about = "Used to open and communicate with the main application UI.")]
pub struct Args {
    /// The ProjectId of the project to open on startup (passed by the launcher).
    ///
    /// Empty means no project is opened automatically.
    #[arg(long, allow_hyphen_values = true, default_value = "")]
    pub open_project: String,

    #[cfg(debug_assertions)]
    /// Disable debug logging. This option only exists if `debug_assertions` are
    /// enabled.
    #[arg(long)]
    pub no_debug_logging: bool,

    #[cfg(debug_assertions)]
    /// Enable debug error log panics. This option only exists if
    /// `debug_assertions` are enabled.
    #[arg(long, conflicts_with = "no_debug_logging")]
    pub debug_error_log_panics: bool,
}

impl Default for Args {
    fn default() -> Self {
        Self::parse()
    }
}
