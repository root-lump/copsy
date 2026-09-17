use crate::cli::TransitionOptions;
use crate::commands::worktree::{self, CreationKind, SetupContext};
use crate::config::Config;
use crate::git;
use crate::info;
use anyhow::{Result, bail};
use colored::Colorize;

pub fn run(
    branch: &str,
    kind: CreationKind,
    from: Option<&str>,
    options: &TransitionOptions,
) -> Result<()> {
    // Anchored to the main worktree, not the inherited current directory: running
    // this from inside a worktree would otherwise nest the new one under its sibling.
    let main_worktree = git::main_worktree_path()?;
    let repo_name = git::repository_name(&main_worktree);
    let config = Config::load()?;
    let base_dir = config.base_dir();
    let worktree_path = git::worktree_dir_name(
        &repo_name,
        &main_worktree,
        branch,
        base_dir.as_deref(),
        config.layout(),
    );

    if worktree_path.exists() {
        let worktrees = git::list_worktrees()?;
        if worktrees
            .iter()
            .any(|worktree| worktree.path == worktree_path)
        {
            info!("Worktree already exists at {}", worktree_path.display());
            return worktree::transition(
                &worktree_path,
                &config,
                options,
                SetupContext::Existing,
                || Ok(()),
            );
        }
        bail!(
            "Directory {} already exists but is not a worktree",
            worktree_path.display()
        );
    }

    // The worktree's own parent is created rather than base_dir, because the
    // nested layout inserts a <base_dir>/<repo> level that may not exist yet.
    if let Some(parent) = worktree_path.parent()
        && !parent.exists()
    {
        std::fs::create_dir_all(parent)?;
    }

    let verb = if kind == CreationKind::New {
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

    worktree::transition(
        &worktree_path,
        &config,
        options,
        SetupContext::Created(kind),
        || git::add_worktree(&worktree_path, branch, kind.creates_branch(), from),
    )
}
