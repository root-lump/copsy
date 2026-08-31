use crate::cli::TransitionOptions;
use crate::config::{Config, SetupTrigger};
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
