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
use cli::{Cli, Command, ConfigCommand};

fn main() -> Result<()> {
    // The shell function wraps `command copsy` and captures stdout, which makes
    // both `colored` and `console` think they're not on a terminal. Override
    // before clap parsing so --help and all output stay colored.
    colored::control::set_override(true);
    console::set_colors_enabled(true);
    console::set_colors_enabled_stderr(true);
    let cli = Cli::parse();

    match cli.command {
        None => commands::interactive::run(&cli.launch, &cli.carry, &cli.setup)?,
        Some(Command::New {
            branch,
            from,
            launch,
            carry,
            setup,
        }) => {
            commands::add::run(
                &branch,
                commands::add::CreationKind::New,
                from.as_deref(),
                &launch,
                &carry,
                &setup,
            )?;
        }
        Some(Command::Add {
            branch,
            launch,
            carry,
            setup,
        }) => {
            commands::add::run(
                &branch,
                commands::add::CreationKind::Add,
                None,
                &launch,
                &carry,
                &setup,
            )?;
        }
        Some(Command::Switch {
            name,
            launch,
            carry,
            setup,
        }) => {
            commands::switch::run(name.as_deref(), &launch, &carry, &setup)?;
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
        Some(Command::Pr {
            target,
            launch,
            setup,
        }) => {
            commands::pr::run(target.as_deref(), &launch, &setup)?;
        }
        Some(Command::Config {
            command: ConfigCommand::Init,
        }) => {
            commands::config::run_init()?;
        }
        Some(Command::Setup { execute }) => {
            commands::setup::run(execute)?;
        }
    }

    Ok(())
}
