use crate::cli::{CarryFlags, LaunchFlags};
use crate::commands::add;
use crate::git;
use crate::theme;
use anyhow::Result;

pub fn run(target: Option<&str>, launch: &LaunchFlags) -> Result<()> {
    let branch = match target {
        Some(t) => git::fetch_pr(t)?,
        None => match select_pr_interactive()? {
            Some(b) => b,
            None => return Ok(()),
        },
    };

    let no_carry = CarryFlags {
        carry: false,
        no_carry: true,
    };
    add::run(&branch, false, None, launch, &no_carry)
}

fn select_pr_interactive() -> Result<Option<String>> {
    let prs = crate::spinner::with_spinner("Fetching pull requests...", git::list_prs)?;
    if prs.is_empty() {
        anyhow::bail!("No open pull requests found");
    }

    let items: Vec<String> = prs
        .iter()
        .map(|(num, title, branch)| format!("#{num} {title} ({branch})"))
        .collect();

    let Some(selection) = theme::fuzzy_select(&items, "Select a pull request")? else {
        return Ok(None);
    };

    let (num, _, _) = &prs[selection];
    git::fetch_pr(num).map(Some)
}
