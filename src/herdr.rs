//! Path compatibility with Herdr v0.9.1's worktree/config implementation:
//! https://github.com/herdrdev/herdr/tree/v0.9.1/src

use crate::cli::LaunchOptions;
use crate::git;
use crate::info;
use crate::output;
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

pub fn open_workspace(source: &Path, target: &Path, launch: &LaunchOptions) {
    if !inside_herdr() {
        info!(
            "Herdr path created/reused; workspace registration requires running copsy inside Herdr"
        );
        if !launch.is_empty() {
            info!(
                "Launch options were skipped; run copsy inside Herdr to launch in the child workspace"
            );
        }
        return;
    }
    if let Err(error) = register_workspace(source, target, launch) {
        // Git creation and carry have already succeeded. Preserve that checkout
        // even when the session API is unavailable; add can retry registration.
        info!("Warning: worktree is ready, but Herdr workspace registration failed: {error:#}");
        info!("Retry inside Herdr with: copsy add <branch> --herdr (include any launch options)");
    }
}

fn inside_herdr() -> bool {
    std::env::var_os("HERDR_ENV").as_deref() == Some(std::ffi::OsStr::new("1"))
}

fn register_workspace(source: &Path, target: &Path, launch: &LaunchOptions) -> Result<()> {
    // Matching the directory layout does not create sidebar membership. Herdr's
    // worktree.open API records the parent and child relationships. An explicit
    // repo parent avoids another client's focused workspace and linked sources.
    let output = run_herdr(
        Command::new("herdr")
            .args(["worktree", "open", "--cwd"])
            .arg(source)
            .arg("--path")
            .arg(target)
            .arg("--no-focus"),
        "worktree open",
    )?;
    if !launch.is_empty() {
        let response: WorkspaceResponse = serde_json::from_slice(&output.stdout)
            .context("Invalid Herdr worktree open response; launch options were skipped")?;
        let workspace_id = response.result.workspace.workspace_id;
        validate_id(&workspace_id)?;
        // Setup executes in the shell wrapper after this process exits. Deferring
        // launches ensures no agent starts before setup has completed successfully.
        let request = LaunchRequest {
            workspace_id,
            path: target.to_path_buf(),
            launch: launch.clone(),
        };
        output::request_herdr_launch(&serde_json::to_string(&request)?);
    }
    Ok(())
}

#[derive(Deserialize)]
struct WorkspaceResponse {
    result: WorkspaceResult,
}

#[derive(Deserialize)]
struct WorkspaceResult {
    workspace: WorkspaceIdentity,
}

#[derive(Deserialize)]
struct WorkspaceIdentity {
    workspace_id: String,
}

#[derive(Deserialize)]
struct TabResponse {
    result: TabResult,
}

#[derive(Deserialize)]
struct TabResult {
    root_pane: PaneIdentity,
}

#[derive(Deserialize)]
struct PaneIdentity {
    workspace_id: String,
    pane_id: String,
}

#[derive(Deserialize, Serialize)]
struct LaunchRequest {
    workspace_id: String,
    path: PathBuf,
    launch: LaunchOptions,
}

pub fn launch_tools(request: &str) -> Result<()> {
    if !inside_herdr() {
        bail!("Herdr tool launches require running copsy inside Herdr");
    }
    let request: LaunchRequest =
        serde_json::from_str(request).context("Invalid deferred Herdr launch request")?;
    validate_id(&request.workspace_id)?;
    let flags = &request.launch;
    let mut commands = Vec::new();
    if flags.code {
        commands.push(("VS Code", "code -- ."));
    }
    if flags.cursor {
        commands.push(("Cursor", "cursor -- ."));
    }
    if flags.claude {
        commands.push(("Claude Code", "claude"));
    }
    if flags.codex {
        commands.push(("Codex", "codex"));
    }
    if let Some(command) = flags.open.as_deref().filter(|command| !command.is_empty()) {
        commands.push(("Custom command", command));
    }
    let mut failed = false;
    for (label, command) in commands {
        if let Err(error) = launch_in_tab(&request, label, command) {
            // Other requested tools are independent; do not rerun successful
            // launches or fall back to the invoking terminal after a partial failure.
            info!("Warning: failed to launch {label} in Herdr: {error:#}");
            failed = true;
        }
    }
    if failed {
        bail!(
            "Some Herdr launches failed; the worktree is ready. Inspect its tabs before retrying only the failed launch options"
        );
    }
    Ok(())
}

fn launch_in_tab(request: &LaunchRequest, label: &str, command: &str) -> Result<()> {
    // Always allocate a fresh terminal, even for a reused workspace: its root
    // pane may already contain an editor, agent, or unfinished command.
    let output = run_herdr(
        Command::new("herdr")
            .args([
                "tab",
                "create",
                "--workspace",
                &request.workspace_id,
                "--cwd",
            ])
            .arg(&request.path)
            .args(["--label", label, "--no-focus"]),
        "tab create",
    )?;
    let response: TabResponse = serde_json::from_slice(&output.stdout)
        .context("Invalid Herdr tab create response; no command was submitted")?;
    let pane = response.result.root_pane;
    validate_id(&pane.pane_id)?;
    if pane.workspace_id != request.workspace_id {
        bail!("Herdr returned a pane outside the requested workspace; no command was submitted");
    }
    // The tab's explicit cwd avoids shell-specific path quoting. Known tools use
    // fixed commands; only --open contributes user-provided shell source.
    run_herdr(
        Command::new("herdr").args(["pane", "run", &pane.pane_id, command]),
        "pane run",
    )
    .with_context(|| format!("Command submission to pane {} failed", pane.pane_id))?;
    Ok(())
}

fn validate_id(id: &str) -> Result<()> {
    if id.is_empty() || id.starts_with('-') || id.chars().any(char::is_control) {
        bail!("Invalid Herdr identifier; launch options were skipped");
    }
    Ok(())
}

fn run_herdr(command: &mut Command, operation: &str) -> Result<Output> {
    let output = command
        .stdin(Stdio::null())
        .output()
        .with_context(|| format!("Failed to run herdr {operation}"))?;
    if !output.status.success() {
        bail!(
            "herdr {operation} failed ({}): {}{}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim(),
            String::from_utf8_lossy(&output.stdout).trim(),
        );
    }
    Ok(output)
}

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
