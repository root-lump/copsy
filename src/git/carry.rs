//! Carry changes using a uniquely tagged stash; a failed pop stays recoverable.

use super::git_output_in;
use crate::info;
use anyhow::{Context, Result, bail};
use std::path::Path;
use std::process::Command;

pub fn has_changes(path: &Path) -> Result<bool> {
    let status = get_status(path)?;
    Ok(!status.is_empty())
}

pub fn stash_changes(path: &Path) -> Result<Option<String>> {
    // Unique tag lets us find the exact stash entry later, even if the user
    // creates other stashes between stash and pop.
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let tag = format!("copsy:carry:{nanos}");
    let before = git_output_in(path, &["stash", "list"])?;
    git_output_in(path, &["stash", "push", "--include-untracked", "-m", &tag])?;
    let after = git_output_in(path, &["stash", "list"])?;
    // `git stash push` is a no-op when the working tree is clean; detect that
    // by comparing the stash list before and after.
    if before == after {
        Ok(None)
    } else {
        Ok(Some(tag))
    }
}

pub fn unstash_changes(path: &Path, tag: &str) -> Result<()> {
    let list = git_output_in(path, &["stash", "list"])?;
    for line in list.lines() {
        if line.contains(tag) {
            let ref_name = line
                .split(':')
                .next()
                .context("Unexpected stash list format")?
                .trim();
            git_output_in(path, &["stash", "pop", ref_name])?;
            return Ok(());
        }
    }
    bail!("Stash entry '{tag}' not found")
}

/// Stash uncommitted changes in `source` if carry is enabled and changes exist.
/// Returns the stash tag if changes were stashed.
pub fn carry_stash(source: &Path, should_carry: bool) -> Result<Option<String>> {
    if should_carry && has_changes(source)? {
        info!("Stashing uncommitted changes...");
        stash_changes(source)
    } else {
        Ok(None)
    }
}

/// Pop a previously stashed carry entry into `target`.
/// Prints a warning instead of failing if the unstash fails.
pub fn carry_unstash(target: &Path, stash_tag: &Option<String>) {
    if let Some(tag) = stash_tag {
        info!("Applying stashed changes...");
        if let Err(e) = unstash_changes(target, tag) {
            info!(
                "Warning: failed to apply changes: {e}\n  Run 'git stash pop' manually to recover."
            );
        }
    }
}

pub fn get_status(path: &Path) -> Result<String> {
    let output = Command::new("git")
        .args(["status", "--short"])
        .current_dir(path)
        .output()
        .context("Failed to run git status")?;
    Ok(String::from_utf8(output.stdout)?.trim().to_string())
}
