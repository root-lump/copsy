use clap::{Args, Parser, Subcommand};

#[derive(Parser)]
#[command(name = "copsy", about = "Git worktree management CLI")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,

    #[command(flatten)]
    pub launch: LaunchFlags,

    #[command(flatten)]
    pub carry: CarryFlags,
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
        #[command(flatten)]
        carry: CarryFlags,
    },
    /// Create a worktree for an existing branch
    Add {
        branch: String,
        #[command(flatten)]
        launch: LaunchFlags,
        #[command(flatten)]
        carry: CarryFlags,
    },
    /// Switch to a worktree
    #[command(visible_alias = "sw")]
    Switch {
        name: Option<String>,
        #[command(flatten)]
        launch: LaunchFlags,
        #[command(flatten)]
        carry: CarryFlags,
    },
    /// Remove a worktree
    #[command(visible_alias = "rm")]
    Remove {
        name: Option<String>,
        /// Also delete the local branch
        #[arg(long)]
        with_branch: bool,
        /// Remove all worktrees
        #[arg(long)]
        all: bool,
    },
    /// List all worktrees
    #[command(visible_alias = "ls")]
    List,
    /// Show git status for all worktrees
    Status,
    /// Close current worktree and return to main
    Close {
        /// Also delete the local branch
        #[arg(long)]
        with_branch: bool,
    },
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
pub struct CarryFlags {
    /// Carry uncommitted changes to the target worktree
    #[arg(long)]
    pub carry: bool,

    /// Do not carry uncommitted changes (overrides config)
    #[arg(long, conflicts_with = "carry")]
    pub no_carry: bool,
}

impl CarryFlags {
    pub fn should_carry(&self, config_default: bool) -> bool {
        if self.carry {
            true
        } else if self.no_carry {
            false
        } else {
            config_default
        }
    }
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
