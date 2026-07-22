mod cli;
mod commands;
mod config;
mod git;
mod launcher;
mod output;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, Command};

fn main() -> Result<()> {
    // Force color output even when stdout is piped by the shell function wrapper
    colored::control::set_override(true);
    let cli = Cli::parse();

    match cli.command {
        None => commands::interactive::run(&cli.launch)?,
        Some(Command::New {
            branch,
            from,
            launch,
        }) => {
            commands::add::run(&branch, true, from.as_deref(), &launch)?;
        }
        Some(Command::Add { branch, launch }) => {
            commands::add::run(&branch, false, None, &launch)?;
        }
        Some(Command::Switch { name, launch }) => {
            commands::switch::run(name.as_deref(), &launch)?;
        }
        Some(Command::Remove {
            name,
            with_branch,
            all,
        }) => {
            commands::remove::run(name.as_deref(), with_branch, all)?;
        }
        Some(Command::List) => {
            commands::list::run()?;
        }
        Some(Command::Status) => {
            commands::status::run()?;
        }
        Some(Command::Close { with_branch }) => {
            commands::close::run(with_branch)?;
        }
        Some(Command::Init { shell }) => {
            commands::init::run(&shell)?;
        }
        Some(Command::Pr { target, launch }) => {
            commands::pr::run(target.as_deref(), &launch)?;
        }
    }

    Ok(())
}
