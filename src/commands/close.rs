use crate::git;
use crate::info;
use crate::output;
use anyhow::{Result, bail};

pub fn run() -> Result<()> {
    let current_dir = std::env::current_dir()?;
    let main_path = git::main_worktree_path()?;

    if current_dir.starts_with(&main_path) {
        bail!("Already in the main worktree. Nothing to close.");
    }

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
    // cd to main before removal — the shell function processes markers in order
    output::request_cd(&main_path);
    git::remove_worktree(&wt_path)?;
    info!("Done.");

    Ok(())
}
