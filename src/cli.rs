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

    #[command(flatten)]
    pub setup: SetupFlags,
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
        #[command(flatten)]
        setup: SetupFlags,
    },
    /// Create a worktree for an existing branch
    Add {
        branch: String,
        #[command(flatten)]
        launch: LaunchFlags,
        #[command(flatten)]
        carry: CarryFlags,
        #[command(flatten)]
        setup: SetupFlags,
    },
    /// Switch to a worktree
    #[command(visible_alias = "sw")]
    Switch {
        name: Option<String>,
        #[command(flatten)]
        launch: LaunchFlags,
        #[command(flatten)]
        carry: CarryFlags,
        #[command(flatten)]
        setup: SetupFlags,
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
        #[command(flatten)]
        setup: SetupFlags,
    },
    /// Manage repository configuration
    Config {
        #[command(subcommand)]
        command: ConfigCommand,
    },
    /// Run repository setup for the current worktree
    Setup {
        /// Execute setup immediately instead of using shell integration
        #[arg(long, hide = true)]
        execute: bool,
    },
}

#[derive(Subcommand)]
pub enum ConfigCommand {
    /// Create repository configuration interactively
    Init,
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

#[derive(Args, Clone, Default)]
pub struct SetupFlags {
    /// Run repository setup for the target worktree
    #[arg(long)]
    pub setup: bool,

    /// Do not run repository setup (overrides config)
    #[arg(long, conflicts_with = "setup")]
    pub no_setup: bool,
}

impl SetupFlags {
    pub fn should_setup(&self, auto_enabled: bool, newly_created: bool) -> bool {
        if self.setup {
            true
        } else if self.no_setup {
            false
        } else {
            newly_created && auto_enabled
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_config_init() {
        let cli = Cli::try_parse_from(["copsy", "config", "init"]).unwrap();
        assert!(matches!(
            cli.command,
            Some(Command::Config {
                command: ConfigCommand::Init
            })
        ));
    }

    #[test]
    fn parses_setup_command() {
        let cli = Cli::try_parse_from(["copsy", "setup"]).unwrap();
        assert!(matches!(
            cli.command,
            Some(Command::Setup { execute: false })
        ));
    }

    #[test]
    fn setup_flags_conflict() {
        let result = Cli::try_parse_from(["copsy", "new", "feature", "--setup", "--no-setup"]);
        assert!(result.is_err());
    }

    #[test]
    fn setup_flag_precedence_is_explicit_then_auto() {
        let automatic = SetupFlags::default();
        assert!(automatic.should_setup(true, true));
        assert!(!automatic.should_setup(true, false));

        let forced = SetupFlags {
            setup: true,
            no_setup: false,
        };
        assert!(forced.should_setup(false, false));

        let suppressed = SetupFlags {
            setup: false,
            no_setup: true,
        };
        assert!(!suppressed.should_setup(true, true));
    }
}
