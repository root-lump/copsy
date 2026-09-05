mod cli;
mod commands;
mod config;
mod git;
mod launcher;
mod output;
mod repository_path;
mod spinner;
mod theme;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, Command, ConfigCommand};
use commands::worktree::CreationKind;

fn main() -> Result<()> {
    // The shell function wraps `command copsy` and captures stdout, which makes
    // both `colored` and `console` think they're not on a terminal. Override
    // before clap parsing so --help and all output stay colored.
    colored::control::set_override(true);
    console::set_colors_enabled(true);
    console::set_colors_enabled_stderr(true);
    let Cli {
        command,
        transition: root_transition,
    } = Cli::parse();

    match command {
        None => commands::interactive::run(&root_transition.resolve()?)?,
        Some(Command::New {
            branch,
            from,
            transition,
        }) => commands::add::run(
            &branch,
            CreationKind::New,
            from.as_deref(),
            &root_transition.resolve_with(&transition)?,
        )?,
        Some(Command::Add { branch, transition }) => commands::add::run(
            &branch,
            CreationKind::Add,
            None,
            &root_transition.resolve_with(&transition)?,
        )?,
        Some(Command::Switch { name, transition }) => {
            commands::switch::run(name.as_deref(), &root_transition.resolve_with(&transition)?)?
        }
        Some(Command::Remove {
            name,
            with_branch,
            all,
            force,
        }) => {
            root_transition.ensure_unused("remove")?;
            commands::remove::run(name.as_deref(), with_branch, all, force)?;
        }
        Some(Command::List) => {
            root_transition.ensure_unused("list")?;
            commands::list::run()?;
        }
        Some(Command::Status) => {
            root_transition.ensure_unused("status")?;
            commands::status::run()?;
        }
        Some(Command::Close { with_branch }) => {
            root_transition.ensure_unused("close")?;
            commands::close::run(with_branch)?;
        }
        Some(Command::Init { shell }) => {
            root_transition.ensure_unused("init")?;
            commands::init::run(&shell)?;
        }
        Some(Command::Pr { target, transition }) => {
            commands::pr::run(target.as_deref(), &root_transition.resolve_pr(&transition)?)?
        }
        Some(Command::Config {
            command: ConfigCommand::Repo,
        }) => {
            root_transition.ensure_unused("config")?;
            commands::config::run_repo()?;
        }
        Some(Command::Config {
            command: ConfigCommand::Global,
        }) => {
            root_transition.ensure_unused("config")?;
            commands::config::run_global()?;
        }
        Some(Command::Setup { execute }) => {
            root_transition.ensure_unused("setup")?;
            if execute {
                commands::setup::execute_current()?;
            } else {
                commands::setup::request_current()?;
            }
        }
    }

    Ok(())
}
