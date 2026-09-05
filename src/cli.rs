use anyhow::{Result, bail};
use clap::{Args, Parser, Subcommand};

#[derive(Parser)]
#[command(name = "copsy", about = "Git worktree management CLI")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,

    #[command(flatten)]
    pub transition: TransitionFlags,
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
        transition: TransitionFlags,
    },
    /// Create a worktree for an existing branch
    Add {
        branch: String,
        #[command(flatten)]
        transition: TransitionFlags,
    },
    /// Switch to a worktree
    #[command(visible_alias = "sw")]
    Switch {
        name: Option<String>,
        #[command(flatten)]
        transition: TransitionFlags,
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
        /// Discard uncommitted changes and delete unmerged branches
        #[arg(long)]
        force: bool,
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
    /// Output shell integration and completion definitions
    Init {
        /// Shell type (zsh or bash)
        shell: String,
    },
    /// Checkout a pull request as a worktree
    Pr {
        /// PR number or URL (interactive if omitted)
        target: Option<String>,
        #[command(flatten)]
        transition: PrTransitionFlags,
    },
    /// Manage copsy configuration
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
    Repo,
    /// Create global configuration interactively
    Global,
}

#[derive(Args, Clone, Default)]
pub struct TransitionFlags {
    #[command(flatten)]
    launch: LaunchFlags,
    #[command(flatten)]
    carry: CarryFlags,
    #[command(flatten)]
    setup: SetupFlags,
}

impl TransitionFlags {
    pub fn resolve(&self) -> Result<TransitionOptions> {
        self.resolve_with(&Self::default())
    }

    pub fn resolve_with(&self, local: &Self) -> Result<TransitionOptions> {
        Ok(TransitionOptions {
            launch: self.launch.merge(&local.launch)?,
            carry: self
                .carry
                .selection()
                .merge(local.carry.selection(), "carry")?,
            setup: self
                .setup
                .selection()
                .merge(local.setup.selection(), "setup")?,
        })
    }

    pub fn resolve_pr(&self, local: &PrTransitionFlags) -> Result<TransitionOptions> {
        let local = Self {
            launch: local.launch.clone(),
            carry: CarryFlags::default(),
            setup: local.setup.clone(),
        };
        self.resolve_with(&local)?.for_pr()
    }

    pub fn ensure_unused(&self, command: &str) -> Result<()> {
        if self.launch.has_values()
            || self.carry.selection() != Selection::Default
            || self.setup.selection() != Selection::Default
        {
            bail!("transition options are not supported with '{command}'");
        }
        Ok(())
    }
}

#[derive(Args, Clone, Default)]
pub struct PrTransitionFlags {
    #[command(flatten)]
    launch: LaunchFlags,
    #[command(flatten)]
    setup: SetupFlags,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TransitionOptions {
    launch: LaunchOptions,
    carry: Selection,
    setup: Selection,
}

impl TransitionOptions {
    pub fn launch(&self) -> &LaunchOptions {
        &self.launch
    }

    pub fn should_carry(&self, config_default: bool) -> bool {
        self.carry.resolve(config_default)
    }

    pub fn should_setup(&self, automatic: bool) -> bool {
        self.setup.resolve(automatic)
    }

    fn for_pr(mut self) -> Result<Self> {
        if self.carry == Selection::Enabled {
            bail!("--carry is not supported with 'pr'");
        }
        self.carry = Selection::Disabled;
        Ok(self)
    }
}

#[derive(Args, Clone, Default)]
struct CarryFlags {
    /// Carry uncommitted changes to the target worktree
    #[arg(long)]
    carry: bool,

    /// Do not carry uncommitted changes (overrides config)
    #[arg(long, conflicts_with = "carry")]
    no_carry: bool,
}

impl CarryFlags {
    fn selection(&self) -> Selection {
        Selection::from_pair(self.carry, self.no_carry)
    }
}

#[derive(Args, Clone, Default)]
struct SetupFlags {
    /// Run repository setup for the target worktree
    #[arg(long)]
    setup: bool,

    /// Do not run repository setup (overrides config)
    #[arg(long, conflicts_with = "setup")]
    no_setup: bool,
}

impl SetupFlags {
    fn selection(&self) -> Selection {
        Selection::from_pair(self.setup, self.no_setup)
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum Selection {
    #[default]
    Default,
    Enabled,
    Disabled,
}

impl Selection {
    fn from_pair(enabled: bool, disabled: bool) -> Self {
        if enabled {
            Self::Enabled
        } else if disabled {
            Self::Disabled
        } else {
            Self::Default
        }
    }

    fn merge(self, other: Self, name: &str) -> Result<Self> {
        match (self, other) {
            (Self::Default, value) | (value, Self::Default) => Ok(value),
            (left, right) if left == right => Ok(left),
            _ => bail!("conflicting --{name} and --no-{name} options"),
        }
    }

    fn resolve(self, default: bool) -> bool {
        match self {
            Self::Default => default,
            Self::Enabled => true,
            Self::Disabled => false,
        }
    }
}

#[derive(Args, Clone, Default)]
struct LaunchFlags {
    /// Launch claude after switching
    #[arg(long, short = 'c')]
    claude: bool,

    /// Launch codex after switching
    #[arg(long, short = 'x')]
    codex: bool,

    /// Open in VS Code
    #[arg(long)]
    code: bool,

    /// Open in Cursor
    #[arg(long)]
    cursor: bool,

    /// Run a custom command after switching
    #[arg(long, value_name = "CMD")]
    open: Option<String>,
}

impl LaunchFlags {
    fn merge(&self, other: &Self) -> Result<LaunchOptions> {
        let open = match (&self.open, &other.open) {
            (Some(left), Some(right)) if left != right => {
                bail!("conflicting --open options '{left}' and '{right}'");
            }
            (Some(value), _) | (_, Some(value)) => Some(value.clone()),
            (None, None) => None,
        };
        Ok(LaunchOptions {
            claude: self.claude || other.claude,
            codex: self.codex || other.codex,
            code: self.code || other.code,
            cursor: self.cursor || other.cursor,
            open,
        })
    }

    fn has_values(&self) -> bool {
        self.claude || self.codex || self.code || self.cursor || self.open.is_some()
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LaunchOptions {
    pub claude: bool,
    pub codex: bool,
    pub code: bool,
    pub cursor: bool,
    pub open: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resolve_new(arguments: &[&str]) -> Result<TransitionOptions> {
        let cli = Cli::try_parse_from(arguments)?;
        let Command::New { transition, .. } = cli.command.unwrap() else {
            panic!("expected new command");
        };
        cli.transition.resolve_with(&transition)
    }

    #[test]
    fn parses_config_scopes_and_rejects_init() {
        let repo = Cli::try_parse_from(["copsy", "config", "repo"]).unwrap();
        assert!(matches!(
            repo.command,
            Some(Command::Config {
                command: ConfigCommand::Repo
            })
        ));
        let global = Cli::try_parse_from(["copsy", "config", "global"]).unwrap();
        assert!(matches!(
            global.command,
            Some(Command::Config {
                command: ConfigCommand::Global
            })
        ));
        assert!(Cli::try_parse_from(["copsy", "config", "init"]).is_err());
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
    fn resolves_transition_flags_before_or_after_subcommand() {
        let before = resolve_new(&["copsy", "--setup", "--code", "new", "feature"]).unwrap();
        let after = resolve_new(&["copsy", "new", "feature", "--setup", "--code"]).unwrap();

        assert_eq!(before, after);
        assert!(before.should_setup(false));
        assert!(before.launch().code);
    }

    #[test]
    fn rejects_conflicts_across_flag_positions() {
        assert!(resolve_new(&["copsy", "--setup", "new", "feature", "--no-setup"]).is_err());
        assert!(
            resolve_new(&[
                "copsy", "--open", "first", "new", "feature", "--open", "second"
            ])
            .is_err()
        );
    }

    #[test]
    fn setup_defaults_to_automatic_policy() {
        let options = TransitionOptions::default();
        assert!(options.should_setup(true));
        assert!(!options.should_setup(false));
    }

    #[test]
    fn pr_rejects_explicit_carry_and_disables_config_default() {
        let cli = Cli::try_parse_from(["copsy", "--carry", "pr", "12"]).unwrap();
        let Command::Pr { transition, .. } = cli.command.unwrap() else {
            panic!("expected pr command");
        };
        assert!(cli.transition.resolve_pr(&transition).is_err());

        let cli = Cli::try_parse_from(["copsy", "pr", "12"]).unwrap();
        let Command::Pr { transition, .. } = cli.command.unwrap() else {
            panic!("expected pr command");
        };
        let options = cli.transition.resolve_pr(&transition).unwrap();
        assert!(!options.should_carry(true));
    }

    #[test]
    fn clap_rejects_conflicts_in_one_position() {
        assert!(Cli::try_parse_from(["copsy", "new", "feature", "--setup", "--no-setup"]).is_err());
    }
}
