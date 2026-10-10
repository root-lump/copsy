//! Remote selection and branch discovery share the same precedence rules.

use super::{git_output, git_output_in};
use anyhow::Result;
use std::path::Path;

/// Remote that names the repository and owns the branches worth offering.
///
/// `origin` comes first so that repositories which already have one keep the
/// behaviour they had before remotes were resolved at all. The `gh-resolved`
/// remote is the last resort: under a fork workflow it points at the upstream
/// repository, whose branches are not the ones a contributor works on.
fn resolve_primary_remote(remotes: &[String], gh_default: Option<&str>) -> Option<String> {
    if remotes.iter().any(|r| r == "origin") {
        return Some("origin".to_string());
    }
    if let [only] = remotes {
        return Some(only.clone());
    }
    let gh_default = gh_default?;
    remotes
        .iter()
        .any(|r| r == gh_default)
        .then(|| gh_default.to_string())
}

/// Remote that holds the pull requests `gh` reports on.
///
/// `gh repo set-default` decides which repository `gh pr view` answers for, so
/// its remote has to win here: under a fork workflow the pull request lives in
/// the upstream repository, not in the `origin` fork.
fn resolve_gh_remote(remotes: &[String], gh_default: Option<&str>) -> Option<String> {
    if let Some(gh_default) = gh_default
        && remotes.iter().any(|r| r == gh_default)
    {
        return Some(gh_default.to_string());
    }
    if remotes.iter().any(|r| r == "origin") {
        return Some("origin".to_string());
    }
    if let [only] = remotes {
        return Some(only.clone());
    }
    None
}

// Reads the key rather than splitting on every dot, because a remote name may
// itself contain dots: "remote.my.fork.gh-resolved base" names "my.fork".
fn parse_gh_resolved_remote(config_output: &str) -> Option<String> {
    let key = config_output.lines().next()?.split_whitespace().next()?;
    let name = key.strip_prefix("remote.")?.strip_suffix(".gh-resolved")?;
    (!name.is_empty()).then(|| name.to_string())
}

fn list_remotes_in(dir: &Path) -> Vec<String> {
    git_output_in(dir, &["remote"])
        .map(|output| output.lines().map(|line| line.to_string()).collect())
        .unwrap_or_default()
}

fn gh_default_remote_in(dir: &Path) -> Option<String> {
    let output = git_output_in(
        dir,
        &["config", "--get-regexp", r"^remote\..*\.gh-resolved$"],
    )
    .ok()?;
    parse_gh_resolved_remote(&output)
}

pub(super) fn primary_remote_in(dir: &Path) -> Option<String> {
    let remotes = list_remotes_in(dir);
    resolve_primary_remote(&remotes, gh_default_remote_in(dir).as_deref())
}

fn gh_remote_in(dir: &Path) -> Option<String> {
    let remotes = list_remotes_in(dir);
    resolve_gh_remote(&remotes, gh_default_remote_in(dir).as_deref())
}

fn primary_remote() -> Option<String> {
    let cwd = std::env::current_dir().unwrap_or_default();
    primary_remote_in(&cwd)
}

pub(super) fn gh_remote() -> Option<String> {
    let cwd = std::env::current_dir().unwrap_or_default();
    gh_remote_in(&cwd)
}

pub(super) fn gh_default_remote() -> Option<String> {
    let cwd = std::env::current_dir().unwrap_or_default();
    gh_default_remote_in(&cwd)
}

// Splits on both separators because git accepts two URL shapes for the same
// remote: "https://host/org/repo.git" and the scp-like "git@host:org/repo.git".
pub(super) fn repository_name_from_url(url: &str) -> Option<String> {
    let trimmed = url.trim().trim_end_matches('/');
    let name = trimmed.rsplit(['/', ':']).next()?;
    let name = name.strip_suffix(".git").unwrap_or(name);
    (!name.is_empty()).then(|| name.to_string())
}

pub fn list_branches() -> Result<Vec<String>> {
    let stdout = git_output(&["branch", "--format=%(refname:short)"])?;
    Ok(stdout.lines().map(|s| s.to_string()).collect())
}

pub fn list_remote_branches() -> Result<Vec<String>> {
    let stdout = git_output(&["branch", "-r", "--format=%(refname:short)"])?;
    Ok(strip_remote_prefix(&stdout, primary_remote().as_deref()))
}

// Only the primary remote's prefix is stripped: a bare branch name is what
// `git worktree add` needs to create a tracking branch, and stripping every
// remote's prefix would make branches of different remotes collide.
fn strip_remote_prefix(branch_list: &str, primary: Option<&str>) -> Vec<String> {
    let prefix = primary.map(|name| format!("{name}/"));
    branch_list
        .lines()
        .filter(|line| !line.contains("HEAD"))
        .map(|line| match &prefix {
            Some(prefix) => line
                .strip_prefix(prefix.as_str())
                .unwrap_or(line)
                .to_string(),
            None => line.to_string(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repository_name_from_url_accepts_every_url_shape() {
        assert_eq!(
            repository_name_from_url("https://github.com/root-lump/copsy.git").unwrap(),
            "copsy"
        );
        assert_eq!(
            repository_name_from_url("git@github.com:root-lump/copsy.git").unwrap(),
            "copsy"
        );
        assert_eq!(
            repository_name_from_url("ssh://git@github.com/root-lump/copsy").unwrap(),
            "copsy"
        );
        assert_eq!(
            repository_name_from_url("/srv/git/copsy.git/\n").unwrap(),
            "copsy"
        );
    }

    #[test]
    fn repository_name_from_url_rejects_urls_without_a_name() {
        assert!(repository_name_from_url("").is_none());
        assert!(repository_name_from_url("/").is_none());
    }

    fn remotes(names: &[&str]) -> Vec<String> {
        names.iter().map(|n| n.to_string()).collect()
    }

    #[test]
    fn primary_remote_prefers_origin_then_the_only_remote_then_gh_default() {
        assert_eq!(
            resolve_primary_remote(&remotes(&["origin", "upstream"]), None).as_deref(),
            Some("origin")
        );
        assert_eq!(
            resolve_primary_remote(&remotes(&["upstream"]), None).as_deref(),
            Some("upstream")
        );
        assert_eq!(
            resolve_primary_remote(&remotes(&["upstream", "fork"]), Some("fork")).as_deref(),
            Some("fork")
        );
        assert_eq!(
            resolve_primary_remote(&remotes(&["upstream", "fork"]), None),
            None
        );
        assert_eq!(
            resolve_primary_remote(&remotes(&["upstream", "fork"]), Some("gone")),
            None
        );
        assert_eq!(resolve_primary_remote(&[], None), None);
    }

    #[test]
    fn gh_remote_prefers_the_gh_default_then_origin_then_the_only_remote() {
        assert_eq!(
            resolve_gh_remote(&remotes(&["origin", "upstream"]), Some("upstream")).as_deref(),
            Some("upstream")
        );
        assert_eq!(
            resolve_gh_remote(&remotes(&["origin", "upstream"]), None).as_deref(),
            Some("origin")
        );
        assert_eq!(
            resolve_gh_remote(&remotes(&["upstream"]), None).as_deref(),
            Some("upstream")
        );
        assert_eq!(resolve_gh_remote(&[], None), None);
    }

    #[test]
    fn parse_gh_resolved_remote_reads_the_remote_name() {
        assert_eq!(
            parse_gh_resolved_remote("remote.upstream.gh-resolved base").as_deref(),
            Some("upstream")
        );
        assert_eq!(
            parse_gh_resolved_remote("remote.my.fork.gh-resolved base").as_deref(),
            Some("my.fork")
        );
        assert_eq!(parse_gh_resolved_remote(""), None);
    }

    #[test]
    fn strip_remote_prefix_only_strips_the_primary_remote() {
        let branch_list = "upstream/feature/login\norigin/topic\norigin/HEAD -> origin/main";
        assert_eq!(
            strip_remote_prefix(branch_list, Some("upstream")),
            vec!["feature/login".to_string(), "origin/topic".to_string()]
        );
        assert_eq!(
            strip_remote_prefix(branch_list, None),
            vec![
                "upstream/feature/login".to_string(),
                "origin/topic".to_string()
            ]
        );
    }
}
