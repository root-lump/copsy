use crate::cli::{CarryFlags, LaunchFlags};
use crate::config::Config;
use crate::git;
use crate::info;
use crate::launcher;
use crate::output;
use crate::theme;
use anyhow::{Result, bail};

pub fn run(name: Option<&str>, launch: &LaunchFlags, carry: &CarryFlags) -> Result<()> {
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
    let should_carry = carry.should_carry(config.carry_changes());
    let current_dir = std::env::current_dir()?;
    let stash_tag = git::carry_stash(&current_dir, should_carry)?;

    info!("Switching to worktree '{}'", target.branch);
    output::request_cd(&target.path);

    git::carry_unstash(&target.path, &stash_tag);

    launcher::launch_tools(launch, &target.path);

    Ok(())
}
