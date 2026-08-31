use crate::cli::TransitionOptions;
use crate::commands::add;
use crate::commands::worktree::{self, CreationKind, SetupContext};
use crate::config::Config;
use crate::git;
use crate::info;
use crate::theme;
use anyhow::Result;
use colored::Colorize;

pub fn run(options: &TransitionOptions) -> Result<()> {
    let (worktrees, local_branches, remote_branches) =
        crate::spinner::with_spinner("Loading branches...", || {
            let wt = git::list_worktrees();
            let lb = git::list_branches();
            let rb = git::list_remote_branches().unwrap_or_default();
            wt.and_then(|w| lb.map(|l| (w, l, rb)))
        })?;

    let main_path = git::main_worktree_path()?;
    let worktree_branches: Vec<&str> = worktrees.iter().map(|w| w.branch.as_str()).collect();

    let mut items: Vec<(String, ItemKind)> = Vec::new();

    for wt in &worktrees {
        if wt.is_bare {
            continue;
        }
        items.push((
            theme::format_worktree(&wt.branch, &wt.path, wt.path == main_path),
            ItemKind::ExistingWorktree(wt.path.clone()),
        ));
    }

    for branch in &local_branches {
        if !worktree_branches.contains(&branch.as_str()) {
            items.push((
                format!("{} {}", console::style("[local]").white(), branch),
                ItemKind::NewWorktree(branch.clone()),
            ));
        }
    }

    for branch in &remote_branches {
        if !worktree_branches.contains(&branch.as_str()) && !local_branches.contains(branch) {
            items.push((
                format!(
                    "{} {}",
                    console::style("[remote]").yellow(),
                    console::style(branch).dim()
                ),
                ItemKind::NewWorktree(branch.clone()),
            ));
        }
    }

    if items.is_empty() {
        anyhow::bail!("No branches found");
    }

    let display: Vec<String> = items.iter().map(|(s, _)| s.clone()).collect();
    let Some(selection) = theme::fuzzy_select(&display, "Select a branch")? else {
        return Ok(());
    };

    match &items[selection].1 {
        ItemKind::ExistingWorktree(path) => {
            let config = Config::load()?;
            info!("{}", "Switching to worktree".green());
            worktree::transition(path, &config, options, SetupContext::Existing, || Ok(()))?;
        }
        ItemKind::NewWorktree(branch) => {
            add::run(branch, CreationKind::Add, None, options)?;
        }
    }

    Ok(())
}

enum ItemKind {
    ExistingWorktree(std::path::PathBuf),
    NewWorktree(String),
}
