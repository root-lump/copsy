use crate::git;
use anyhow::Result;
use colored::Colorize;

pub fn run() -> Result<()> {
    let worktrees = git::list_worktrees()?;
    let current_dir = std::env::current_dir().ok();
    let main_path = git::main_worktree_path()?;

    if worktrees.is_empty() {
        println!("No worktrees found.");
        return Ok(());
    }

    for wt in &worktrees {
        if wt.is_bare {
            continue;
        }
        let is_current = current_dir
            .as_ref()
            .is_some_and(|cd| cd.starts_with(&wt.path));
        let is_main = wt.path == main_path;

        let marker = if is_current {
            "*".green().bold().to_string()
        } else {
            " ".to_string()
        };
        let branch = if is_current {
            wt.branch.green().bold().to_string()
        } else {
            wt.branch.white().to_string()
        };
        let label = if is_main {
            format!(" {}", "(repo)".blue())
        } else {
            String::new()
        };
        println!("{marker} {branch}{label}");
        println!("    {}", wt.path.display().to_string().dimmed());
    }

    Ok(())
}
