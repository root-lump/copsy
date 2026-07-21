use clap::{Args, Parser, Subcommand};

#[derive(Parser)]
#[command(name = "copsy", about = "Git worktree management CLI")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,

    #[command(flatten)]
    pub launch: LaunchFlags,
}

#[derive(Subcommand)]
pub enum Command {
    /// Create a worktree with a new branch
    New {
        branch: String,
        /// Base branch to create from (default: current HEAD)
        #[arg(long)]
        from: Option<String>,
        #[command(flatten)]
        launch: LaunchFlags,
    },
    /// Create a worktree for an existing branch
    Add {
        branch: String,
        #[command(flatten)]
        launch: LaunchFlags,
    },
    /// Switch to a worktree
    #[command(visible_alias = "sw")]
    Switch {
        name: Option<String>,
        #[command(flatten)]
        launch: LaunchFlags,
    },
    /// Remove a worktree
    #[command(visible_alias = "rm")]
    Remove { name: Option<String> },
    /// List all worktrees
    #[command(visible_alias = "ls")]
    List,
    /// Show git status for all worktrees
    Status,
    /// Close current worktree and return to main
    Close,
    /// Output shell integration function
    Init {
        /// Shell type (zsh or bash)
        shell: String,
    },
    /// Checkout a pull request as a worktree
    Pr {
        /// PR number or URL (interactive if omitted)
        target: Option<String>,
        #[command(flatten)]
        launch: LaunchFlags,
    },
}

#[derive(Args, Clone)]
pub struct LaunchFlags {
    /// Launch claude after switching
    #[arg(long, short = 'c')]
    pub claude: bool,

    /// Launch codex after switching
    #[arg(long, short = 'x')]
    pub codex: bool,

    /// Open in VS Code
    #[arg(long)]
    pub code: bool,

    /// Open in Cursor
    #[arg(long)]
    pub cursor: bool,

    /// Run a custom command after switching
    #[arg(long, value_name = "CMD")]
    pub open: Option<String>,
}
