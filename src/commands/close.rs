use crate::commands::worktree;
use crate::config::Config;
use crate::git;
use crate::info;
use crate::output;
use anyhow::{Result, bail};

pub fn run(with_branch: bool) -> Result<()> {
    let current_dir = std::env::current_dir()?;
    let main_path = git::main_worktree_path()?;
    if current_dir.starts_with(&main_path) {
        bail!("Already in the main worktree. Nothing to close.");
    }

    // Read before anything is removed so a malformed config cannot fail the
    // command once the worktree is already gone.
    let config = Config::load()?;
    let layout = config.layout();
    let repository_dir = git::nested_repository_dir(
        &git::repository_name(&main_path),
        &main_path,
        config.base_dir().as_deref(),
    );

    let worktrees = git::list_worktrees()?;
    let current_wt = worktrees
        .iter()
        .find(|w| current_dir.starts_with(&w.path))
        .ok_or_else(|| anyhow::anyhow!("Current directory is not inside a worktree"))?;

    let status = git::get_status(&current_wt.path)?;
    if !status.is_empty() {
        info!("Warning: worktree has uncommitted changes:");
        for line in status.lines() {
            info!("  {line}");
        }
        bail!("Commit or stash changes before closing.");
    }

    let wt_path = current_wt.path.clone();
    let branch = current_wt.branch.clone();

    info!("Closing worktree '{branch}'...");
    // Relocate the parent shell as soon as this captured command completes,
    // because its current directory is removed below.
    output::request_cd(&main_path);
    git::remove_worktree(&main_path, &wt_path, false)?;
    worktree::prune_empty_repository_dir(&wt_path, &repository_dir, layout);
    if with_branch {
        info!("Deleting local branch '{branch}'...");
        git::delete_local_branch(&main_path, &branch, false)?;
    }
    info!("Done.");

    Ok(())
}
