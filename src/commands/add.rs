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
    let root = git::repo_root()?;
    let config = Config::load()?;
    let base_dir = config.base_dir();
    let worktree_path = git::worktree_dir_name(&root, branch, base_dir.as_deref());

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

    if let Some(ref directory) = base_dir
        && !directory.exists()
    {
        std::fs::create_dir_all(directory)?;
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
