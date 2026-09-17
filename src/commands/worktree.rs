use crate::cli::TransitionOptions;
use crate::config::{Config, SetupTrigger, WorktreeLayout};
use crate::git;
use crate::launcher;
use crate::output;
use anyhow::Result;
use std::path::Path;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CreationKind {
    New,
    Add,
    Pr,
}

impl CreationKind {
    pub fn creates_branch(self) -> bool {
        self == Self::New
    }

    fn setup_trigger(self) -> SetupTrigger {
        match self {
            Self::New => SetupTrigger::New,
            Self::Add => SetupTrigger::Add,
            Self::Pr => SetupTrigger::Pr,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SetupContext {
    Existing,
    Created(CreationKind),
}

pub fn transition<F>(
    target: &Path,
    config: &Config,
    options: &TransitionOptions,
    setup_context: SetupContext,
    prepare_target: F,
) -> Result<()>
where
    F: FnOnce() -> Result<()>,
{
    let current_dir = std::env::current_dir()?;
    let stash_tag = git::carry_stash(&current_dir, options.should_carry(config.carry_changes()))?;

    prepare_target()?;
    git::carry_unstash(target, &stash_tag);

    if should_request_setup(config, options, setup_context) {
        output::request_setup(target);
    }
    output::request_cd(target);
    launcher::launch_tools(options.launch(), target);
    Ok(())
}

/// Remove the repository directory the nested layout leaves behind.
///
/// `git worktree remove` deletes only the worktree itself, so the enclosing
/// directory would linger once its last worktree is gone. The parent is matched
/// against `nested_repository_dir` by path rather than by name so that changing
/// the layout setting or `base_dir` later cannot delete a shared `base_dir`.
pub fn prune_empty_repository_dir(
    worktree_path: &Path,
    repository_dir: &Path,
    layout: WorktreeLayout,
) {
    if layout != WorktreeLayout::Nested {
        return;
    }
    if worktree_path.parent() != Some(repository_dir) {
        return;
    }
    let Ok(mut entries) = std::fs::read_dir(repository_dir) else {
        return;
    };
    if entries.next().is_some() {
        return;
    }
    // Cleanup is only housekeeping: the worktree is already gone, so a failure
    // here must not turn a successful removal into an error.
    let _ = std::fs::remove_dir(repository_dir);
}

fn should_request_setup(
    config: &Config,
    options: &TransitionOptions,
    context: SetupContext,
) -> bool {
    let automatic = match context {
        SetupContext::Existing => false,
        SetupContext::Created(kind) => config.auto_setup(kind.setup_trigger()),
    };
    options.should_setup(automatic)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::tempdir;

    // Builds <root>/<parent_name>/<branch> and returns the worktree path,
    // standing in for a nested-layout worktree that git has just removed.
    fn removed_worktree(root: &Path, parent_name: &str) -> PathBuf {
        let parent = root.join(parent_name);
        fs::create_dir_all(&parent).unwrap();
        parent.join("feat-sample")
    }

    #[test]
    fn prunes_the_repository_directory_once_it_is_empty() {
        let root = tempdir().unwrap();
        let repository_dir = root.path().join("myapp");
        let worktree = removed_worktree(root.path(), "myapp");

        prune_empty_repository_dir(&worktree, &repository_dir, WorktreeLayout::Nested);

        assert!(!repository_dir.exists());
    }

    #[test]
    fn keeps_the_repository_directory_with_remaining_worktrees() {
        let root = tempdir().unwrap();
        let repository_dir = root.path().join("myapp");
        let worktree = removed_worktree(root.path(), "myapp");
        fs::create_dir(repository_dir.join("fix-typo")).unwrap();

        prune_empty_repository_dir(&worktree, &repository_dir, WorktreeLayout::Nested);

        assert!(repository_dir.exists());
    }

    #[test]
    fn keeps_directories_that_are_not_the_repository_directory() {
        let root = tempdir().unwrap();
        let worktree = removed_worktree(root.path(), "worktrees");

        prune_empty_repository_dir(
            &worktree,
            &root.path().join("myapp"),
            WorktreeLayout::Nested,
        );

        assert!(root.path().join("worktrees").exists());
    }

    #[test]
    fn the_flat_layout_prunes_nothing() {
        let root = tempdir().unwrap();
        let repository_dir = root.path().join("myapp");
        let worktree = removed_worktree(root.path(), "myapp");

        prune_empty_repository_dir(&worktree, &repository_dir, WorktreeLayout::Flat);

        assert!(repository_dir.exists());
    }

    #[test]
    fn creation_kinds_map_to_distinct_setup_triggers() {
        assert_eq!(CreationKind::New.setup_trigger(), SetupTrigger::New);
        assert_eq!(CreationKind::Add.setup_trigger(), SetupTrigger::Add);
        assert_eq!(CreationKind::Pr.setup_trigger(), SetupTrigger::Pr);
    }

    #[test]
    fn existing_worktrees_do_not_use_automatic_setup() {
        assert!(!should_request_setup(
            &Config::default(),
            &TransitionOptions::default(),
            SetupContext::Existing
        ));
    }

    #[test]
    fn created_worktrees_use_only_the_matching_automatic_trigger() {
        let config = Config::with_auto_setup(vec![SetupTrigger::New]);
        let options = TransitionOptions::default();

        assert!(should_request_setup(
            &config,
            &options,
            SetupContext::Created(CreationKind::New)
        ));
        assert!(!should_request_setup(
            &config,
            &options,
            SetupContext::Created(CreationKind::Add)
        ));
    }
}
