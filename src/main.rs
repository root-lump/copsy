mod cli;
mod commands;
mod config;
mod git;
mod launcher;
mod output;
mod spinner;
mod theme;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, Command};

fn main() -> Result<()> {
    // Force color output even when stdout is piped by the shell function wrapper
    colored::control::set_override(true);
    console::set_colors_enabled(true);
    console::set_colors_enabled_stderr(true);
    let cli = Cli::parse();

    match cli.command {
        None => commands::interactive::run(&cli.launch, &cli.carry)?,
        Some(Command::New {
            branch,
            from,
            launch,
            carry,
        }) => {
            commands::add::run(&branch, true, from.as_deref(), &launch, &carry)?;
        }
        Some(Command::Add {
            branch,
            launch,
            carry,
        }) => {
            commands::add::run(&branch, false, None, &launch, &carry)?;
        }
        Some(Command::Switch {
            name,
            launch,
            carry,
        }) => {
            commands::switch::run(name.as_deref(), &launch, &carry)?;
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
