use crate::git;
use crate::info;
use anyhow::Result;
use colored::Colorize;

pub fn run() -> Result<()> {
    let worktrees = git::list_worktrees()?;

    if worktrees.is_empty() {
        info!("No worktrees found.");
        return Ok(());
    }

    for wt in &worktrees {
        if wt.is_bare {
            continue;
        }
        let status = git::get_status(&wt.path).unwrap_or_else(|_| "error".to_string());
        let file_count = if status.is_empty() {
            0
        } else {
            status.lines().count()
        };

        let branch_display = wt.branch.white().bold();
        let status_display = if file_count == 0 {
            "clean".green().bold().to_string()
        } else {
            format!("{file_count} changed").yellow().bold().to_string()
        };

        info!("  {branch_display}  [{status_display}]");
        info!("    {}", wt.path.display().to_string().dimmed());

        if file_count > 0 {
            for line in status.lines().take(5) {
                let colored_line = colorize_status_line(line);
                info!("      {colored_line}");
            }
            if file_count > 5 {
                info!(
                    "      {}",
                    format!("... and {} more", file_count - 5).dimmed()
                );
            }
        }
    }

    Ok(())
}

// `git status --short` uses a fixed 2-char prefix (XY) for status indicators.
fn colorize_status_line(line: &str) -> String {
    if line.len() < 2 {
        return line.to_string();
    }
    let indicator = &line[..2];
    let rest = &line[2..];
    match indicator.trim() {
        "M" => format!("{} {}", "M".yellow(), rest),
        "A" => format!("{} {}", "A".green(), rest),
        "D" => format!("{} {}", "D".red(), rest),
        "R" => format!("{} {}", "R".cyan(), rest),
        "??" => format!("{}", line.dimmed()),
        _ => line.to_string(),
    }
}
