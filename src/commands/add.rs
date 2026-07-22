use crate::cli::{CarryFlags, LaunchFlags};
use crate::config::Config;
use crate::git;
use crate::info;
use crate::launcher;
use crate::output;
use anyhow::{Result, bail};
use colored::Colorize;

pub fn run(
    branch: &str,
    create_branch: bool,
    from: Option<&str>,
    launch: &LaunchFlags,
    carry: &CarryFlags,
) -> Result<()> {
    let root = git::repo_root()?;
    let config = Config::load()?;
    let should_carry = carry.should_carry(config.carry_changes());
    let base_dir = config.base_dir();
    let worktree_path = git::worktree_dir_name(&root, branch, base_dir.as_deref());

    if worktree_path.exists() {
        let worktrees = git::list_worktrees()?;
        if worktrees.iter().any(|w| w.path == worktree_path) {
            info!("Worktree already exists at {}", worktree_path.display());
            output::request_cd(&worktree_path);
            launcher::launch_tools(launch, &worktree_path);
            return Ok(());
        }
        bail!(
            "Directory {} already exists but is not a worktree",
            worktree_path.display()
        );
    }

    if let Some(ref dir) = base_dir
        && !dir.exists()
    {
        std::fs::create_dir_all(dir)?;
    }

    let verb = if create_branch {
        "Creating new branch"
    } else {
        "Adding"
    };
    info!(
        "{verb} worktree for branch '{}' at {}",
        branch.green().bold(),
        worktree_path.display().to_string().dimmed()
    );
    if let Some(base) = from {
        info!("  based on '{}'", base.cyan());
    }

    let current_dir = std::env::current_dir()?;
    let stash_tag = git::carry_stash(&current_dir, should_carry)?;

    git::add_worktree(&worktree_path, branch, create_branch, from)?;
    output::request_cd(&worktree_path);

    git::carry_unstash(&worktree_path, &stash_tag);

    launcher::launch_tools(launch, &worktree_path);

    Ok(())
}
