use clap::Parser;

#[derive(Parser, Debug, Clone, PartialEq, Eq, Hash)]
#[command(about = "Communication with the main application.")]
pub struct Args {
    /// The ProjectId of the project to open on startup (passed by the launcher).
    #[arg(long, allow_hyphen_values = true)]
    pub open_project: String,
}

impl Default for Args {
    fn default() -> Self {
        Self::parse()
    }
}
