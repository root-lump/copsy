use crate::cli::LaunchFlags;
use crate::commands::add;
use crate::git;
use anyhow::Result;
use dialoguer::FuzzySelect;

pub fn run(target: Option<&str>, launch: &LaunchFlags) -> Result<()> {
    let branch = match target {
        Some(t) => git::fetch_pr(t)?,
        None => match select_pr_interactive()? {
            Some(b) => b,
            None => return Ok(()),
        },
    };

    add::run(&branch, false, None, launch)
}

fn select_pr_interactive() -> Result<Option<String>> {
    let prs = git::list_prs()?;
    if prs.is_empty() {
        anyhow::bail!("No open pull requests found");
    }

    let items: Vec<String> = prs
        .iter()
        .map(|(num, title, branch)| format!("#{num} {title} ({branch})"))
        .collect();

    let Some(selection) = FuzzySelect::new()
        .with_prompt("Select a pull request")
        .items(&items)
        .default(0)
        .interact_opt()?
    else {
        return Ok(None);
    };

    let (num, _, _) = &prs[selection];
    git::fetch_pr(num).map(Some)
}
