use crate::cli::TransitionOptions;
use crate::commands::add;
use crate::commands::worktree::CreationKind;
use crate::git;
use crate::spinner;
use crate::theme;
use anyhow::Result;

pub fn run(target: Option<&str>, options: &TransitionOptions, herdr: bool) -> Result<()> {
    let branch = match target {
        Some(t) => git::fetch_pr(t)?,
        None => match select_pr_interactive()? {
            Some(b) => b,
            None => return Ok(()),
        },
    };

    add::run(&branch, CreationKind::Pr, None, options, herdr)
}

fn select_pr_interactive() -> Result<Option<String>> {
    let prs = spinner::with_spinner("Fetching pull requests...", git::list_prs)?;
    if prs.is_empty() {
        anyhow::bail!("No open pull requests found");
    }

    let items: Vec<String> = prs
        .iter()
        .map(|pr| format!("#{} {} ({})", pr.number, pr.title, pr.branch))
        .collect();

    let Some(selection) = theme::fuzzy_select(&items, "Select a pull request")? else {
        return Ok(None);
    };

    let pr = &prs[selection];
    git::fetch_pr_branch(&pr.number, &pr.branch)?;
    Ok(Some(pr.branch.clone()))
}
