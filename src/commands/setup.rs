use crate::config::{Config, validate_repository_relative_path};
use crate::git;
use crate::info;
use crate::output;
use anyhow::{Context, Result, bail};
use std::fs;
use std::io;
use std::path::Path;
use std::process::Command;

pub fn run(execute: bool) -> Result<()> {
    let worktree = git::repo_root()?;
    if execute {
        execute_now(&worktree)
    } else {
        output::request_setup(&worktree);
        Ok(())
    }
}

fn execute_now(worktree: &Path) -> Result<()> {
    let config = Config::load()?;
    let Some(setup) = config.setup() else {
        info!("No setup actions configured.");
        return Ok(());
    };
    if setup.copy_from_main.is_empty() && setup.command.is_none() {
        info!("No setup actions configured.");
        return Ok(());
    }

    let main_worktree = git::main_worktree_path()?;
    for configured in &setup.copy_from_main {
        copy_configured_path(&main_worktree, worktree, configured)?;
    }

    if let Some(command) = &setup.command {
        run_command(worktree, command)?;
    }
    Ok(())
}

fn copy_configured_path(main_worktree: &Path, worktree: &Path, configured: &str) -> Result<()> {
    let relative = Path::new(configured);
    validate_repository_relative_path(relative)
        .with_context(|| format!("Invalid setup.copy_from_main path '{configured}'"))?;

    let source = main_worktree.join(relative);
    let destination = worktree.join(relative);
    if entry_exists(&destination)? {
        info!(
            "Skipping '{}': destination already exists.",
            relative.display()
        );
        return Ok(());
    }
    if !entry_exists(&source)? {
        info!(
            "Warning: skipping '{}': source does not exist in the main worktree.",
            relative.display()
        );
        return Ok(());
    }

    info!("Copying '{}' from the main worktree...", relative.display());
    copy_entry(&source, &destination)
        .with_context(|| format!("Failed to copy '{}'", relative.display()))
}

fn entry_exists(path: &Path) -> io::Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

fn copy_entry(source: &Path, destination: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(source)?;
    if metadata.file_type().is_symlink() {
        let target = fs::read_link(source)?;
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)?;
        }
        create_symlink(&target, destination)?;
    } else if metadata.is_dir() {
        fs::create_dir(destination)?;
        for entry in fs::read_dir(source)? {
            let entry = entry?;
            copy_entry(&entry.path(), &destination.join(entry.file_name()))?;
        }
        fs::set_permissions(destination, metadata.permissions())?;
    } else if metadata.is_file() {
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(source, destination)?;
    } else {
        bail!("Unsupported file type at {}", source.display());
    }
    Ok(())
}

#[cfg(unix)]
fn create_symlink(target: &Path, destination: &Path) -> io::Result<()> {
    std::os::unix::fs::symlink(target, destination)
}

#[cfg(not(unix))]
fn create_symlink(_target: &Path, _destination: &Path) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "copying symbolic links is unsupported on this platform",
    ))
}

fn run_command(worktree: &Path, command: &[String]) -> Result<()> {
    let Some((program, args)) = command.split_first() else {
        bail!("setup.command must contain at least one argument");
    };
    info!("Running setup command: {}", command.join(" "));
    let status = Command::new(program)
        .args(args)
        .current_dir(worktree)
        .status()
        .with_context(|| format!("Failed to run setup command '{program}'"))?;
    if !status.success() {
        bail!("Setup command failed with status {status}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn copies_files_and_directories() {
        let main = tempdir().unwrap();
        let worktree = tempdir().unwrap();
        fs::write(main.path().join(".env"), "secret").unwrap();
        fs::create_dir(main.path().join("config")).unwrap();
        fs::write(main.path().join("config/local.toml"), "value = 1").unwrap();

        copy_configured_path(main.path(), worktree.path(), ".env").unwrap();
        copy_configured_path(main.path(), worktree.path(), "config").unwrap();

        assert_eq!(
            fs::read_to_string(worktree.path().join(".env")).unwrap(),
            "secret"
        );
        assert_eq!(
            fs::read_to_string(worktree.path().join("config/local.toml")).unwrap(),
            "value = 1"
        );
    }

    #[test]
    fn does_not_overwrite_existing_destination() {
        let main = tempdir().unwrap();
        let worktree = tempdir().unwrap();
        fs::write(main.path().join(".env"), "main").unwrap();
        fs::write(worktree.path().join(".env"), "worktree").unwrap();

        copy_configured_path(main.path(), worktree.path(), ".env").unwrap();

        assert_eq!(
            fs::read_to_string(worktree.path().join(".env")).unwrap(),
            "worktree"
        );
    }

    #[test]
    fn missing_source_is_not_an_error() {
        let main = tempdir().unwrap();
        let worktree = tempdir().unwrap();
        copy_configured_path(main.path(), worktree.path(), ".env").unwrap();
        assert!(!worktree.path().join(".env").exists());
    }

    #[cfg(unix)]
    #[test]
    fn copies_symbolic_links_without_dereferencing() {
        let main = tempdir().unwrap();
        let worktree = tempdir().unwrap();
        fs::write(main.path().join("target.txt"), "target").unwrap();
        std::os::unix::fs::symlink("target.txt", main.path().join("link.txt")).unwrap();

        copy_configured_path(main.path(), worktree.path(), "link.txt").unwrap();

        let destination = worktree.path().join("link.txt");
        assert!(fs::symlink_metadata(&destination).unwrap().is_symlink());
        assert_eq!(fs::read_link(destination).unwrap(), Path::new("target.txt"));
    }

    #[test]
    fn rejects_unsafe_copy_paths() {
        let main = tempdir().unwrap();
        let worktree = tempdir().unwrap();
        assert!(copy_configured_path(main.path(), worktree.path(), "../outside").is_err());
        assert!(copy_configured_path(main.path(), worktree.path(), ".git").is_err());
    }

    #[cfg(unix)]
    #[test]
    fn reports_failing_commands() {
        let worktree = tempdir().unwrap();
        let command = vec!["sh".to_string(), "-c".to_string(), "exit 7".to_string()];
        assert!(run_command(worktree.path(), &command).is_err());
    }
}
