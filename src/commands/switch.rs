use crate::cli::LaunchFlags;
use crate::git;
use crate::info;
use crate::launcher;
use crate::output;
use anyhow::{Result, bail};
use colored::Colorize;
use dialoguer::FuzzySelect;

pub fn run(name: Option<&str>, launch: &LaunchFlags) -> Result<()> {
    let worktrees = git::list_worktrees()?;
    let non_bare: Vec<_> = worktrees.iter().filter(|w| !w.is_bare).collect();

    if non_bare.is_empty() {
        bail!("No worktrees found");
    }

    let target = match name {
        Some(name) => non_bare
            .iter()
            .find(|w| {
                w.branch == name
                    || w.path
                        .file_name()
                        .is_some_and(|n| n.to_string_lossy() == name)
            })
            .ok_or_else(|| anyhow::anyhow!("Worktree '{name}' not found"))?,
        None => {
            let main_path = git::main_worktree_path()?;
            let items: Vec<String> = non_bare
                .iter()
                .map(|w| {
                    let label = if w.path == main_path {
                        "[repo]".blue().bold()
                    } else {
                        "[worktree]".green().bold()
                    };
                    format!(
                        "{} {} {}",
                        label,
                        w.branch.white().bold(),
                        w.path.display().to_string().dimmed()
                    )
                })
                .collect();
            let Some(selection) = FuzzySelect::new()
                .with_prompt("Select worktree")
                .items(&items)
                .default(0)
                .interact_opt()?
            else {
                return Ok(());
            };
            non_bare[selection]
        }
    };

    info!("Switching to worktree '{}'", target.branch);
    output::request_cd(&target.path);
    launcher::launch_tools(launch, &target.path);

    Ok(())
}
