//! Contains [Args], which are parsed command-line flags.

use clap::{Parser, ValueEnum};

/// Parsed command line arguments.
#[derive(Parser, Debug, Clone, PartialEq, Eq, Hash)]
#[command(about = "Used to open and communicate with the launcher UI.")]
pub struct Args {
    /// Provide a custom command to launch the editor (the rest of the arguments
    /// are consumed). An additional project ID argument is appended when this
    /// is invoked.
    #[arg(long, num_args(1..), trailing_var_arg = true, allow_hyphen_values = true)]
    pub editor_cmd: Vec<String>,

    /// Require this instance to be a sender.
    #[arg(long, num_args(0..=1), default_missing_value = "true")]
    pub send_only: Option<ForcibleFlag>,

    /// Require this instance to be a receiver.
    #[arg(long, num_args(0..=1), default_missing_value = "true")]
    pub receive_only: Option<ForcibleFlag>,

    /// If this instance is a sender, don't tell the main instance to focus the
    /// UI.
    #[arg(long)]
    pub no_focus: bool,

    /// If this instance is a sender, tell the main instance that a project's
    /// information has changed (requires refresh).
    #[arg(long)]
    pub rescan_projects: bool,

    /// If this instance is a sender, tell the main instance that a project
    /// failed to be opened for editing.
    #[arg(long)]
    pub project_open_failed: bool,

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

/// Indicates a whether a flag was provided normally or with force.
#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ForcibleFlag {
    /// Exit successfully if this instance can't fulfill this role.
    True,
    /// Exit with an error if this instance can't fulfill this role.
    Force,
}
