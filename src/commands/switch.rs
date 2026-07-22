use crate::cli::{CarryFlags, LaunchFlags};
use crate::config::Config;
use crate::git;
use crate::info;
use crate::launcher;
use crate::output;
use anyhow::{Result, bail};
use console::style;
use dialoguer::FuzzySelect;

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
                .map(|w| {
                    let label = if w.path == main_path {
                        style("[repo]").blue().bold()
                    } else {
                        style("[worktree]").magenta().bold()
                    };
                    format!(
                        "{} {} {}",
                        label,
                        style(&w.branch).bold(),
                        style(w.path.display()).dim()
                    )
                })
                .collect();
            let Some(selection) = FuzzySelect::with_theme(&crate::theme::CopsyTheme::new())
                .with_prompt("Select worktree")
                .items(&items)
                .default(0)
                .interact_opt()?
            else {
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
