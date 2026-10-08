//! Path compatibility with Herdr v0.9.1's worktree/config implementation:
//! https://github.com/herdrdev/herdr/tree/v0.9.1/src

use crate::git;
use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

#[derive(Default, Deserialize)]
struct HerdrConfig {
    worktrees: Option<WorktreesConfig>,
}

#[derive(Default, Deserialize)]
struct WorktreesConfig {
    directory: Option<String>,
}

pub fn worktree_path(branch: &str) -> Result<PathBuf> {
    let config_path = config_path(
        std::env::var_os("HERDR_CONFIG_PATH"),
        std::env::var_os("XDG_CONFIG_HOME"),
        platform_config_dir(),
    );
    let root = root_from_config(&config_path, dirs::home_dir().as_deref())?;
    let root = if root.is_absolute() {
        root
    } else {
        std::env::current_dir()?.join(root)
    };
    let path = root
        .join(git::herdr_repository_name()?)
        .join(branch_slug(branch));
    // Git records canonical checkout paths, so a configured symlink must still
    // match an existing checkout when the same branch is selected again.
    Ok(path.canonicalize().unwrap_or(path))
}

fn platform_config_dir() -> PathBuf {
    #[cfg(windows)]
    {
        dirs::config_dir().unwrap_or_else(std::env::temp_dir)
    }
    #[cfg(not(windows))]
    {
        dirs::home_dir()
            .map(|home| home.join(".config"))
            .unwrap_or_else(std::env::temp_dir)
    }
}

fn config_path(
    override_path: Option<OsString>,
    xdg_config_home: Option<OsString>,
    platform_config_dir: PathBuf,
) -> PathBuf {
    if let Some(path) = override_path {
        return PathBuf::from(path);
    }
    xdg_config_home
        .map(PathBuf::from)
        .unwrap_or(platform_config_dir)
        .join("herdr/config.toml")
}

fn root_from_config(path: &Path, home: Option<&Path>) -> Result<PathBuf> {
    let config: HerdrConfig = match std::fs::read_to_string(path) {
        Ok(content) => toml::from_str(content.trim_start_matches('\u{feff}'))
            .with_context(|| format!("Failed to parse Herdr config {}", path.display()))?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => HerdrConfig::default(),
        Err(error) => {
            return Err(error)
                .with_context(|| format!("Failed to read Herdr config {}", path.display()));
        }
    };
    let directory = config
        .worktrees
        .and_then(|worktrees| worktrees.directory)
        .unwrap_or_else(|| "~/.herdr/worktrees".to_string());
    if directory.is_empty() {
        bail!("Herdr worktrees.directory must not be empty");
    }
    let tilde_rest = directory.strip_prefix("~/").or_else(|| {
        if cfg!(windows) {
            directory.strip_prefix("~\\")
        } else {
            None
        }
    });
    if directory == "~" || tilde_rest.is_some() {
        let home = home.context("Could not determine home directory for Herdr worktrees")?;
        return Ok(home.join(tilde_rest.unwrap_or("")));
    }
    Ok(PathBuf::from(directory))
}

fn branch_slug(branch: &str) -> String {
    let slug = branch
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|part| !part.is_empty())
        .map(str::to_ascii_lowercase)
        .collect::<Vec<_>>()
        .join("-");
    if slug.is_empty() {
        "worktree".to_string()
    } else {
        slug
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn slugs_follow_herdr_rules() {
        for (branch, expected) in [
            ("worktree/brave-river", "worktree-brave-river"),
            ("issue/137 Worktree Spaces", "issue-137-worktree-spaces"),
            ("--Feature///AUTH__v2.1--", "feature-auth-v2-1"),
            ("機能/ログイン", "worktree"),
            ("///", "worktree"),
            ("", "worktree"),
        ] {
            assert_eq!(branch_slug(branch), expected);
        }
    }

    #[test]
    fn config_path_obeys_herdr_environment_precedence() {
        let platform = PathBuf::from("/home/me/.config");
        assert_eq!(
            config_path(None, None, platform.clone()),
            platform.join("herdr/config.toml")
        );
        assert_eq!(
            config_path(None, Some("/xdg".into()), platform.clone()),
            PathBuf::from("/xdg/herdr/config.toml")
        );
        assert_eq!(
            config_path(Some("/override.toml".into()), Some("/xdg".into()), platform),
            PathBuf::from("/override.toml")
        );
    }

    #[test]
    fn missing_or_unset_settings_use_the_default_root() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("config.toml");
        let home = Path::new("/home/me");
        assert_eq!(
            root_from_config(&path, Some(home)).unwrap(),
            home.join(".herdr/worktrees")
        );
        fs::write(&path, "[ui]\nsidebar_width = 26\n[worktrees]\n").unwrap();
        assert_eq!(
            root_from_config(&path, Some(home)).unwrap(),
            home.join(".herdr/worktrees")
        );
    }

    #[test]
    fn configured_roots_expand_tilde_and_ignore_unrelated_settings() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("config.toml");
        for (directory, expected) in [
            ("~/custom", "/home/me/custom"),
            ("~", "/home/me"),
            ("/absolute", "/absolute"),
            ("relative", "relative"),
        ] {
            fs::write(
                &path,
                format!("\u{feff}[worktrees]\ndirectory = '{directory}'\n[ui]\nunknown = true\n"),
            )
            .unwrap();
            assert_eq!(
                root_from_config(&path, Some(Path::new("/home/me"))).unwrap(),
                PathBuf::from(expected)
            );
        }
    }

    #[test]
    fn invalid_or_unreadable_config_fails_instead_of_using_another_root() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("config.toml");
        for contents in [
            "invalid toml",
            "[worktrees]\ndirectory = 12",
            "[worktrees]\ndirectory = ''",
        ] {
            fs::write(&path, contents).unwrap();
            assert!(root_from_config(&path, Some(temp.path())).is_err());
        }
        assert!(root_from_config(temp.path(), Some(temp.path())).is_err());
        fs::remove_file(&path).unwrap();
        assert!(root_from_config(&path, None).is_err());
    }
}
