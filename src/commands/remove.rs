use crate::git;
use crate::info;
use crate::output;
use crate::theme;
use anyhow::{Result, bail};

pub fn run(name: Option<&str>, with_branch: bool, all: bool, force: bool) -> Result<()> {
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
        let current_wt = current_dir
            .as_ref()
            .and_then(|cd| removable.iter().find(|w| cd.starts_with(&w.path)));

        let mut failed_worktrees = Vec::new();
        let mut failed_branches = Vec::new();
        for wt in &removable {
            info!("Removing worktree '{}'...", wt.branch);
            if let Err(e) = git::remove_worktree(&main_path, &wt.path, force) {
                info!("Warning: failed to remove '{}': {e}", wt.branch);
                failed_worktrees.push(wt.path.clone());
                continue;
            }
            if with_branch && !wt.branch.is_empty() {
                info!("Deleting local branch '{}'...", wt.branch);
                if let Err(e) = git::delete_local_branch(&main_path, &wt.branch, force) {
                    info!("Warning: failed to delete branch '{}': {e}", wt.branch);
                    failed_branches.push(wt.branch.clone());
                }
            }
        }

        // Relocate the parent shell only when its worktree is really gone;
        // moving it out of a directory that survived would hide the failure.
        if let Some(wt) = current_wt
            && !failed_worktrees.contains(&wt.path)
        {
            output::request_cd(&main_path);
        }

        let removed = removable.len() - failed_worktrees.len();
        if failed_worktrees.is_empty() && failed_branches.is_empty() {
            info!("Done. Removed {removed} worktree(s).");
            return Ok(());
        }

        if !force {
            info!("Retry with --force to discard uncommitted changes and unmerged branches.");
        }
        bail!(
            "Removed {removed} of {} worktree(s); {} removal(s) and {} branch deletion(s) failed.",
            removable.len(),
            failed_worktrees.len(),
            failed_branches.len()
        );
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
        if !force {
            let status = git::get_status(&target.path)?;
            if !status.is_empty() {
                info!("Warning: worktree has uncommitted changes:");
                for line in status.lines() {
                    info!("  {line}");
                }
                bail!("Commit or stash changes before removing, or pass --force.");
            }
        }
        output::request_cd(&main_path);
    }

    let branch = target.branch.clone();
    info!("Removing worktree '{branch}'...");
    git::remove_worktree(&main_path, &target.path, force)?;
    if with_branch && !branch.is_empty() {
        info!("Deleting local branch '{branch}'...");
        git::delete_local_branch(&main_path, &branch, force)?;
    }
    info!("Done.");

    Ok(())
}
