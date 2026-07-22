use crate::cli::{CarryFlags, LaunchFlags};
use crate::commands::add;
use crate::config::Config;
use crate::git;
use crate::info;
use crate::launcher;
use crate::output;
use anyhow::Result;
use colored::Colorize;
use dialoguer::FuzzySelect;

pub fn run(launch: &LaunchFlags, carry: &CarryFlags) -> Result<()> {
    let worktrees = git::list_worktrees()?;
    let local_branches = git::list_branches()?;
    let remote_branches = git::list_remote_branches().unwrap_or_default();

    let main_path = git::main_worktree_path()?;
    let worktree_branches: Vec<&str> = worktrees.iter().map(|w| w.branch.as_str()).collect();

    let mut items: Vec<(String, ItemKind)> = Vec::new();

    for wt in &worktrees {
        if wt.is_bare {
            continue;
        }
        let label = if wt.path == main_path {
            "[repo]".blue().bold()
        } else {
            "[worktree]".green().bold()
        };
        items.push((
            format!(
                "{} {} {}",
                label,
                wt.branch.white().bold(),
                wt.path.display().to_string().dimmed()
            ),
            ItemKind::ExistingWorktree(wt.path.clone()),
        ));
    }

    for branch in &local_branches {
        if !worktree_branches.contains(&branch.as_str()) {
            items.push((
                format!("{} {}", "[local]".cyan(), branch),
                ItemKind::NewWorktree(branch.clone()),
            ));
        }
    }

    for branch in &remote_branches {
        if !worktree_branches.contains(&branch.as_str()) && !local_branches.contains(branch) {
            items.push((
                format!("{} {}", "[remote]".yellow(), branch.dimmed()),
                ItemKind::NewWorktree(branch.clone()),
            ));
        }
    }

    if items.is_empty() {
        anyhow::bail!("No branches found");
    }

    let display: Vec<&str> = items.iter().map(|(s, _)| s.as_str()).collect();
    let Some(selection) = FuzzySelect::new()
        .with_prompt("Select a branch")
        .items(&display)
        .default(0)
        .interact_opt()?
    else {
        return Ok(());
    };

    match &items[selection].1 {
        ItemKind::ExistingWorktree(path) => {
            let config = Config::load()?;
            let should_carry = carry.should_carry(config.carry_changes());
            let current_dir = std::env::current_dir()?;

            let mut stash_tag = None;
            if should_carry && git::has_changes(&current_dir)? {
                info!("Stashing uncommitted changes...");
                stash_tag = git::stash_changes(&current_dir)?;
            }

            info!("{}", "Switching to worktree".green());
            output::request_cd(path);

            if let Some(tag) = &stash_tag {
                info!("Applying stashed changes...");
                if let Err(e) = git::unstash_changes(path, tag) {
                    info!(
                        "Warning: failed to apply changes: {e}\n  Run 'git stash pop' manually to recover."
                    );
                }
            }

            launcher::launch_tools(launch, path);
        }
        ItemKind::NewWorktree(branch) => {
            add::run(branch, false, None, launch, carry)?;
        }
    }

    Ok(())
}

enum ItemKind {
    ExistingWorktree(std::path::PathBuf),
    NewWorktree(String),
}
