use crate::git;
use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::path::{Component, Path, PathBuf};

#[derive(Deserialize, Default)]
pub struct Config {
    pub worktree: Option<WorktreeConfig>,
    pub setup: Option<SetupConfig>,
}

#[derive(Deserialize, Default)]
pub struct WorktreeConfig {
    pub base_dir: Option<String>,
    pub carry_changes: Option<bool>,
}

#[derive(Deserialize, Default)]
pub struct SetupConfig {
    #[serde(default)]
    pub auto: Vec<SetupTrigger>,
    pub command: Option<Vec<String>>,
    #[serde(default)]
    pub copy_from_main: Vec<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum SetupTrigger {
    New,
    Add,
    Pr,
}

#[derive(Deserialize, Default)]
struct RepositoryConfig {
    worktree: Option<RepositoryWorktreeConfig>,
    setup: Option<SetupConfig>,
}

#[derive(Deserialize, Default)]
struct RepositoryWorktreeConfig {
    base_dir: Option<String>,
}

impl Config {
    pub fn load() -> Result<Self> {
        let mut config = Self::load_global()?;
        // Setup is intentionally repository-local so a global command cannot
        // unexpectedly run against every repository the user opens.
        config.setup = None;

        let path = git::repository_config_path()?;
        if !path.exists() {
            return Ok(config);
        }

        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read {}", path.display()))?;
        let repository: RepositoryConfig = toml::from_str(&content)
            .with_context(|| format!("Failed to parse {}", path.display()))?;
        config.apply_repository(repository);
        config.validate_setup()?;
        Ok(config)
    }

    pub fn load_global() -> Result<Self> {
        let path = global_config_path();
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read {}", path.display()))?;
        let mut config: Config = toml::from_str(&content)
            .with_context(|| format!("Failed to parse {}", path.display()))?;
        config.setup = None;
        Ok(config)
    }

    fn apply_repository(&mut self, repository: RepositoryConfig) {
        if let Some(base_dir) = repository.worktree.and_then(|w| w.base_dir) {
            self.worktree
                .get_or_insert_with(WorktreeConfig::default)
                .base_dir = Some(base_dir);
        }
        self.setup = repository.setup;
    }

    fn validate_setup(&self) -> Result<()> {
        let Some(setup) = &self.setup else {
            return Ok(());
        };
        if setup.command.as_ref().is_some_and(Vec::is_empty) {
            bail!("setup.command must contain at least one argument");
        }
        for path in &setup.copy_from_main {
            validate_repository_relative_path(Path::new(path))
                .with_context(|| format!("Invalid setup.copy_from_main path '{path}'"))?;
        }
        Ok(())
    }

    pub fn carry_changes(&self) -> bool {
        self.worktree
            .as_ref()
            .and_then(|w| w.carry_changes)
            .unwrap_or(false)
    }

    pub fn base_dir(&self) -> Option<PathBuf> {
        self.worktree
            .as_ref()
            .and_then(|w| w.base_dir.as_ref())
            .map(|d| PathBuf::from(shellexpand_tilde(d)))
    }

    pub fn base_dir_raw(&self) -> Option<&str> {
        self.worktree
            .as_ref()
            .and_then(|worktree| worktree.base_dir.as_deref())
    }

    pub fn setup(&self) -> Option<&SetupConfig> {
        self.setup.as_ref()
    }

    pub fn auto_setup(&self, trigger: SetupTrigger) -> bool {
        self.setup
            .as_ref()
            .is_some_and(|setup| setup.auto.contains(&trigger))
    }
}

// Use XDG_CONFIG_HOME or ~/.config instead of dirs::config_dir(),
// which returns ~/Library/Application Support on macOS.
pub(crate) fn global_config_path() -> PathBuf {
    let xdg_config = std::env::var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".config")
        });
    xdg_config.join("copsy").join("config.toml")
}

pub(crate) fn validate_repository_relative_path(path: &Path) -> Result<()> {
    let mut has_component = false;
    for component in path.components() {
        match component {
            Component::Normal(value) if value != ".git" => has_component = true,
            Component::Normal(_) => bail!("paths inside .git are not allowed"),
            Component::CurDir => bail!("current-directory components are not allowed"),
            Component::ParentDir => bail!("parent-directory components are not allowed"),
            Component::RootDir | Component::Prefix(_) => bail!("absolute paths are not allowed"),
        }
    }
    if !has_component {
        bail!("path must not be empty");
    }
    Ok(())
}

pub(crate) fn shellexpand_tilde(path: &str) -> String {
    if let Some(rest) = path.strip_prefix("~/")
        && let Some(home) = dirs::home_dir()
    {
        return home.join(rest).to_string_lossy().to_string();
    }
    path.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shellexpand_tilde_expands_home() {
        let result = shellexpand_tilde("~/projects");
        assert!(!result.starts_with("~/"));
        assert!(result.ends_with("/projects"));
    }

    #[test]
    fn shellexpand_tilde_leaves_absolute_path() {
        assert_eq!(shellexpand_tilde("/usr/local"), "/usr/local");
    }

    #[test]
    fn shellexpand_tilde_leaves_relative_path() {
        assert_eq!(shellexpand_tilde("relative/path"), "relative/path");
    }

    #[test]
    fn config_deserialize_empty() {
        let config: Config = toml::from_str("").unwrap();
        assert!(config.worktree.is_none());
        assert!(config.base_dir().is_none());
    }

    #[test]
    fn config_carry_changes_default_false() {
        let config: Config = toml::from_str("").unwrap();
        assert!(!config.carry_changes());
    }

    #[test]
    fn config_carry_changes_explicit() {
        let config: Config = toml::from_str(
            r#"
            [worktree]
            carry_changes = true
            "#,
        )
        .unwrap();
        assert!(config.carry_changes());
    }

    #[test]
    fn repository_base_dir_overrides_global_without_changing_carry() {
        let mut config: Config = toml::from_str(
            r#"
            [worktree]
            base_dir = "/global"
            carry_changes = true
            "#,
        )
        .unwrap();
        let repository: RepositoryConfig = toml::from_str(
            r#"
            [worktree]
            base_dir = "/repository"
            "#,
        )
        .unwrap();

        config.apply_repository(repository);

        assert_eq!(config.base_dir(), Some(PathBuf::from("/repository")));
        assert!(config.carry_changes());
    }

    #[test]
    fn repository_without_base_dir_inherits_global() {
        let mut config: Config = toml::from_str(
            r#"
            [worktree]
            base_dir = "/global"
            "#,
        )
        .unwrap();
        let repository: RepositoryConfig = toml::from_str("[setup]").unwrap();

        config.apply_repository(repository);

        assert_eq!(config.base_dir(), Some(PathBuf::from("/global")));
    }

    #[test]
    fn setup_triggers_deserialize() {
        let repository: RepositoryConfig = toml::from_str(
            r#"
            [setup]
            auto = ["new", "pr"]
            "#,
        )
        .unwrap();
        let setup = repository.setup.unwrap();
        assert_eq!(setup.auto, vec![SetupTrigger::New, SetupTrigger::Pr]);
    }

    #[test]
    fn empty_setup_command_is_invalid() {
        let config: Config = toml::from_str(
            r#"
            [setup]
            command = []
            "#,
        )
        .unwrap();
        assert!(config.validate_setup().is_err());
    }

    #[test]
    fn validates_safe_relative_paths() {
        assert!(validate_repository_relative_path(Path::new(".env")).is_ok());
        assert!(validate_repository_relative_path(Path::new("config/local.toml")).is_ok());
    }

    #[test]
    fn rejects_unsafe_relative_paths() {
        for path in [
            "",
            ".",
            "../secret",
            "/tmp/secret",
            ".git",
            "nested/.git/config",
        ] {
            assert!(
                validate_repository_relative_path(Path::new(path)).is_err(),
                "{path}"
            );
        }
    }
}
