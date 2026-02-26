use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "dot", about = "minimal ai agent")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    #[arg(
        short = 's',
        long = "session",
        help = "resume a previous session by id"
    )]
    pub session: Option<String>,
}

#[derive(Subcommand)]
pub enum Commands {
    Login,
    Config,
    /// List configured MCP servers and their tools
    Mcp,
    /// List installed extensions
    Extensions,
    /// Install an extension from a git URL
    Install {
        /// Git URL or local path to the extension
        source: String,
    },
    /// Uninstall an extension by name
    Uninstall {
        /// Name of the extension to remove
        name: String,
    },
}
