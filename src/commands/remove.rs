use crate::git;
use crate::info;
use crate::output;
use anyhow::{Result, bail};
use colored::Colorize;
use dialoguer::FuzzySelect;

pub fn run(name: Option<&str>) -> Result<()> {
    let worktrees = git::list_worktrees()?;
    let main_path = git::main_worktree_path()?;
    let removable: Vec<_> = worktrees
        .iter()
        .filter(|w| !w.is_bare && w.path != main_path)
        .collect();

    if removable.is_empty() {
        bail!("No removable worktrees found");
    }

    let target = match name {
        Some(name) => removable
            .iter()
            .find(|w| {
                w.branch == name
                    || w.path
                        .file_name()
                        .is_some_and(|n| n.to_string_lossy() == name)
            })
            .ok_or_else(|| anyhow::anyhow!("Worktree '{name}' not found"))?,
        None => {
            let items: Vec<String> = removable
                .iter()
                .map(|w| {
                    format!(
                        "{} {}",
                        w.branch.white().bold(),
                        w.path.display().to_string().dimmed()
                    )
                })
                .collect();
            let Some(selection) = FuzzySelect::new()
                .with_prompt("Select worktree to remove")
                .items(&items)
                .default(0)
                .interact_opt()?
            else {
                return Ok(());
            };
            removable[selection]
        }
    };

    let current_dir = std::env::current_dir().ok();
    let is_current = current_dir
        .as_ref()
        .is_some_and(|cd| cd.starts_with(&target.path));

    if is_current {
        let status = git::get_status(&target.path)?;
        if !status.is_empty() {
            info!("Warning: worktree has uncommitted changes:");
            for line in status.lines() {
                info!("  {line}");
            }
            bail!("Commit or stash changes before removing.");
        }
        output::request_cd(&main_path);
    }

    info!("Removing worktree '{}'...", target.branch);
    git::remove_worktree(&target.path)?;
    info!("Done.");

    Ok(())
}
