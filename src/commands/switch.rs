use crate::cli::TransitionOptions;
use crate::commands::worktree::{self, SetupContext};
use crate::config::Config;
use crate::git;
use crate::info;
use crate::theme;
use anyhow::{Result, bail};

pub fn run(name: Option<&str>, options: &TransitionOptions) -> Result<()> {
    let worktrees = git::list_worktrees()?;
    let non_bare: Vec<_> = worktrees.iter().filter(|w| !w.is_bare).collect();

    if non_bare.is_empty() {
        bail!("No worktrees found");
    }

    let target = match name {
        Some(name) => git::find_worktree(&non_bare, name)
            .ok_or_else(|| anyhow::anyhow!("Worktree '{name}' not found"))?,
        None => {
            let main_path = git::main_worktree_path()?;
            let items: Vec<String> = non_bare
                .iter()
                .map(|w| theme::format_worktree(&w.branch, &w.path, w.path == main_path))
                .collect();
            let Some(selection) = theme::fuzzy_select(&items, "Select worktree")? else {
                return Ok(());
            };
            non_bare[selection]
        }
    };

    let config = Config::load()?;
    info!("Switching to worktree '{}'", target.branch);
    worktree::transition(
        &target.path,
        &config,
        options,
        SetupContext::Existing,
        || Ok(()),
    )
}
