use crate::git;
use crate::info;
use crate::output;
use crate::theme;
use anyhow::{Result, bail};

pub fn run(name: Option<&str>, with_branch: bool, all: bool) -> Result<()> {
    let worktrees = git::list_worktrees()?;
    let main_path = git::main_worktree_path()?;
    let removable: Vec<_> = worktrees
        .iter()
        .filter(|w| !w.is_bare && w.path != main_path)
        .collect();

    if removable.is_empty() {
        bail!("No removable worktrees found");
    }

    if all {
        let current_dir = std::env::current_dir().ok();
        let in_removable = current_dir
            .as_ref()
            .is_some_and(|cd| removable.iter().any(|w| cd.starts_with(&w.path)));

        if in_removable {
            output::request_cd(&main_path);
        }

        let mut errors = Vec::new();
        for wt in &removable {
            info!("Removing worktree '{}'...", wt.branch);
            if let Err(e) = git::remove_worktree(&wt.path) {
                info!("Warning: failed to remove '{}': {e}", wt.branch);
                errors.push(wt.branch.clone());
                continue;
            }
            if with_branch && !wt.branch.is_empty() {
                info!("Deleting local branch '{}'...", wt.branch);
                if let Err(e) = git::delete_local_branch(&wt.branch) {
                    info!("Warning: failed to delete branch '{}': {e}", wt.branch);
                }
            }
        }

        if errors.is_empty() {
            info!("Done. Removed {} worktree(s).", removable.len());
        } else {
            info!(
                "Done with errors. {} removed, {} failed.",
                removable.len() - errors.len(),
                errors.len()
            );
        }

        return Ok(());
    }

    let target = match name {
        Some(name) => git::find_worktree(&removable, name)
            .ok_or_else(|| anyhow::anyhow!("Worktree '{name}' not found"))?,
        None => {
            let items: Vec<String> = removable
                .iter()
                .map(|w| {
                    format!(
                        "{} {}",
                        console::style(&w.branch).bold(),
                        console::style(w.path.display()).dim()
                    )
                })
                .collect();
            let Some(selection) = theme::fuzzy_select(&items, "Select worktree to remove")? else {
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

    let branch = target.branch.clone();
    info!("Removing worktree '{branch}'...");
    git::remove_worktree(&target.path)?;
    if with_branch && !branch.is_empty() {
        info!("Deleting local branch '{branch}'...");
        git::delete_local_branch(&branch)?;
    }
    info!("Done.");

    Ok(())
}
