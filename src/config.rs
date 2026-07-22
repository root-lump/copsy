use anyhow::Result;
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Deserialize, Default)]
pub struct Config {
    pub worktree: Option<WorktreeConfig>,
}

#[derive(Deserialize, Default)]
pub struct WorktreeConfig {
    pub base_dir: Option<String>,
    pub carry_changes: Option<bool>,
}

impl Config {
    pub fn load() -> Result<Self> {
        let path = config_path();
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(&path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
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
            .map(|d| {
                let expanded = shellexpand_tilde(d);
                PathBuf::from(expanded)
            })
    }
}

// Use XDG_CONFIG_HOME or ~/.config instead of dirs::config_dir(),
// which returns ~/Library/Application Support on macOS
fn config_path() -> PathBuf {
    let xdg_config = std::env::var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".config")
        });
    xdg_config.join("copsy").join("config.toml")
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
    fn config_deserialize_with_base_dir() {
        let config: Config = toml::from_str(
            r#"
            [worktree]
            base_dir = "/tmp/worktrees"
            "#,
        )
        .unwrap();
        assert_eq!(config.base_dir(), Some(PathBuf::from("/tmp/worktrees")));
    }
}
