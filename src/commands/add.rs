use crate::cli::TransitionOptions;
use crate::commands::worktree::{self, CreationKind, SetupContext};
use crate::config::Config;
use crate::git;
use crate::herdr;
use crate::info;
use anyhow::{Result, bail};
use colored::Colorize;

pub fn run(
    branch: &str,
    kind: CreationKind,
    from: Option<&str>,
    options: &TransitionOptions,
    herdr: bool,
) -> Result<()> {
    // Anchor the ordinary layout to the main worktree: using the current
    // directory would nest newly created worktrees under their siblings.
    let main_worktree = git::main_worktree_path()?;
    let config = Config::load()?;
    let worktree_path = if herdr {
        herdr::worktree_path(branch)?
    } else {
        git::worktree_dir_name(
            &git::repository_name(&main_worktree),
            &main_worktree,
            branch,
            config.base_dir().as_deref(),
            config.layout(),
        )
    };

    if worktree_path.exists() {
        let worktrees = git::list_worktrees()?;
        if let Some(existing) = worktrees
            .iter()
            .find(|worktree| worktree.path == worktree_path)
        {
            // Herdr's lowercase slugs can map distinct Git branches to one path.
            if herdr && existing.branch != branch {
                bail!(
                    "Herdr worktree path {} is already used by branch '{}'",
                    worktree_path.display(),
                    existing.branch
                );
            }
            info!("Worktree already exists at {}", worktree_path.display());
            worktree::transition(
                &worktree_path,
                &config,
                options,
                SetupContext::Existing,
                || Ok(()),
            )?;
            if herdr {
                herdr::open_workspace(&main_worktree, &worktree_path);
            }
            return Ok(());
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
    )?;
    if herdr {
        herdr::open_workspace(&main_worktree, &worktree_path);
    }
    Ok(())
}
