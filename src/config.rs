use crate::git;
use crate::repository_path::RepositoryPath;
use anyhow::{Context, Result, bail};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use std::path::{Path, PathBuf};

#[derive(Default)]
pub struct Config {
    base_dir: Option<String>,
    carry_changes: bool,
    setup: Option<SetupConfig>,
}

pub struct SetupConfig {
    auto: Vec<SetupTrigger>,
    command: Option<Vec<String>>,
    copy_from_main: Vec<RepositoryPath>,
}

impl SetupConfig {
    pub fn command(&self) -> Option<&[String]> {
        self.command.as_deref()
    }

    pub fn copy_from_main(&self) -> &[RepositoryPath] {
        &self.copy_from_main
    }

    fn runs_automatically_for(&self, trigger: SetupTrigger) -> bool {
        self.auto.contains(&trigger)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum SetupTrigger {
    New,
    Add,
    Pr,
}

impl SetupTrigger {
    pub const ALL: [Self; 3] = [Self::New, Self::Add, Self::Pr];

    pub fn config_value(self) -> &'static str {
        match self {
            Self::New => "new",
            Self::Add => "add",
            Self::Pr => "pr",
        }
    }

    pub fn prompt_label(self) -> &'static str {
        match self {
            Self::New => "copsy new",
            Self::Add => "copsy add",
            Self::Pr => "copsy pr (may execute untrusted code)",
        }
    }
}

#[derive(Default, Deserialize)]
struct GlobalConfig {
    worktree: Option<GlobalWorktreeConfig>,
}

#[derive(Default, Deserialize)]
struct GlobalWorktreeConfig {
    base_dir: Option<String>,
    carry_changes: Option<bool>,
}

#[derive(Default, Deserialize)]
struct RepositoryConfig {
    worktree: Option<RepositoryWorktreeConfig>,
    setup: Option<RawSetupConfig>,
}

#[derive(Default, Deserialize)]
struct RepositoryWorktreeConfig {
    base_dir: Option<String>,
}

#[derive(Default, Deserialize)]
struct RawSetupConfig {
    #[serde(default)]
    auto: Vec<SetupTrigger>,
    command: Option<Vec<String>>,
    #[serde(default)]
    copy_from_main: Vec<String>,
}

impl Config {
    pub fn load() -> Result<Self> {
        let repository_path = git::repository_config_path()?;
        Self::load_from_paths(&global_config_path(), &repository_path)
    }

    pub fn load_global() -> Result<Self> {
        let global = read_optional::<GlobalConfig>(&global_config_path())?;
        Ok(Self::from_global(global))
    }

    fn load_from_paths(global_path: &Path, repository_path: &Path) -> Result<Self> {
        let global = read_optional::<GlobalConfig>(global_path)?;
        let repository = read_optional::<RepositoryConfig>(repository_path)?;
        Self::from_raw(global, repository)
    }

    fn from_global(global: GlobalConfig) -> Self {
        let worktree = global.worktree.unwrap_or_default();
        Self {
            base_dir: worktree.base_dir,
            carry_changes: worktree.carry_changes.unwrap_or(false),
            setup: None,
        }
    }

    fn from_raw(global: GlobalConfig, repository: RepositoryConfig) -> Result<Self> {
        let mut config = Self::from_global(global);
        if let Some(base_dir) = repository.worktree.and_then(|worktree| worktree.base_dir) {
            config.base_dir = Some(base_dir);
        }
        config.setup = repository.setup.map(resolve_setup).transpose()?;
        Ok(config)
    }

    pub fn carry_changes(&self) -> bool {
        self.carry_changes
    }

    pub fn base_dir(&self) -> Option<PathBuf> {
        self.base_dir
            .as_ref()
            .map(|directory| PathBuf::from(expand_tilde(directory)))
    }

    pub fn base_dir_raw(&self) -> Option<&str> {
        self.base_dir.as_deref()
    }

    pub fn setup(&self) -> Option<&SetupConfig> {
        self.setup.as_ref()
    }

    pub fn auto_setup(&self, trigger: SetupTrigger) -> bool {
        self.setup
            .as_ref()
            .is_some_and(|setup| setup.runs_automatically_for(trigger))
    }

    #[cfg(test)]
    pub(crate) fn with_auto_setup(auto: Vec<SetupTrigger>) -> Self {
        Self {
            setup: Some(SetupConfig {
                auto,
                command: None,
                copy_from_main: Vec::new(),
            }),
            ..Self::default()
        }
    }
}

fn resolve_setup(raw: RawSetupConfig) -> Result<SetupConfig> {
    if raw.command.as_ref().is_some_and(Vec::is_empty) {
        bail!("setup.command must contain at least one argument");
    }
    let copy_from_main = raw
        .copy_from_main
        .into_iter()
        .map(|path| {
            RepositoryPath::new(path.clone())
                .with_context(|| format!("Invalid setup.copy_from_main path '{path}'"))
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(SetupConfig {
        auto: raw.auto,
        command: raw.command,
        copy_from_main,
    })
}

fn read_optional<T>(path: &Path) -> Result<T>
where
    T: Default + DeserializeOwned,
{
    if !path.exists() {
        return Ok(T::default());
    }
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read {}", path.display()))?;
    toml::from_str(&content).with_context(|| format!("Failed to parse {}", path.display()))
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

pub(crate) fn expand_tilde(path: &str) -> String {
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
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn expand_tilde_expands_home() {
        let result = expand_tilde("~/projects");
        assert!(!result.starts_with("~/"));
        assert!(result.ends_with("/projects"));
    }

    #[test]
    fn expand_tilde_leaves_other_paths_unchanged() {
        assert_eq!(expand_tilde("/usr/local"), "/usr/local");
        assert_eq!(expand_tilde("relative/path"), "relative/path");
    }

    #[test]
    fn empty_configuration_uses_defaults() {
        let config = Config::default();
        assert!(config.base_dir().is_none());
        assert!(!config.carry_changes());
        assert!(config.setup().is_none());
    }

    #[test]
    fn loads_and_merges_global_and_repository_files() {
        let directory = tempdir().unwrap();
        let global = directory.path().join("global.toml");
        let repository = directory.path().join("repository.toml");
        fs::write(
            &global,
            r#"
            [worktree]
            base_dir = "/global"
            carry_changes = true
            "#,
        )
        .unwrap();
        fs::write(
            &repository,
            r#"
            [worktree]
            base_dir = "/repository"
            [setup]
            auto = ["new", "pr"]
            command = ["npm", "install"]
            copy_from_main = [".env"]
            "#,
        )
        .unwrap();

        let config = Config::load_from_paths(&global, &repository).unwrap();

        assert_eq!(config.base_dir(), Some(PathBuf::from("/repository")));
        assert!(config.carry_changes());
        assert!(config.auto_setup(SetupTrigger::New));
        assert!(config.auto_setup(SetupTrigger::Pr));
        assert_eq!(
            config.setup().unwrap().command().unwrap(),
            ["npm", "install"]
        );
        assert_eq!(
            config.setup().unwrap().copy_from_main(),
            [RepositoryPath::new(".env").unwrap()]
        );
    }

    #[test]
    fn repository_without_base_dir_inherits_global() {
        let global: GlobalConfig = toml::from_str(
            r#"
            [worktree]
            base_dir = "/global"
            "#,
        )
        .unwrap();
        let repository: RepositoryConfig = toml::from_str("[setup]").unwrap();

        let config = Config::from_raw(global, repository).unwrap();

        assert_eq!(config.base_dir(), Some(PathBuf::from("/global")));
    }

    #[test]
    fn global_setup_table_is_ignored_without_parsing_its_values() {
        let global: GlobalConfig = toml::from_str(
            r#"
            [worktree]
            carry_changes = true
            [setup]
            auto = ["not-a-trigger"]
            command = []
            "#,
        )
        .unwrap();

        let config = Config::from_global(global);

        assert!(config.carry_changes());
        assert!(config.setup().is_none());
    }

    #[test]
    fn rejects_empty_setup_command() {
        let repository: RepositoryConfig = toml::from_str(
            r#"
            [setup]
            command = []
            "#,
        )
        .unwrap();
        assert!(Config::from_raw(GlobalConfig::default(), repository).is_err());
    }

    #[test]
    fn rejects_invalid_copy_paths_during_resolution() {
        let repository: RepositoryConfig = toml::from_str(
            r#"
            [setup]
            copy_from_main = ["nested/.git/config"]
            "#,
        )
        .unwrap();
        assert!(Config::from_raw(GlobalConfig::default(), repository).is_err());
    }
}
