use crate::config::Config;
use crate::git;
use crate::info;
use crate::output;
use crate::repository_path::RepositoryPath;
use anyhow::{Context, Result, bail};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn request_current() -> Result<()> {
    output::request_setup(&git::repo_root()?);
    Ok(())
}

pub fn execute_current() -> Result<()> {
    execute(&git::repo_root()?)
}

fn execute(worktree: &Path) -> Result<()> {
    let config = Config::load()?;
    let Some(setup) = config.setup() else {
        info!("No setup actions configured.");
        return Ok(());
    };
    if setup.copy_from_main().is_empty() && setup.command().is_none() {
        info!("No setup actions configured.");
        return Ok(());
    }

    execute_actions(
        &git::main_worktree_path()?,
        worktree,
        setup.copy_from_main(),
        setup.command(),
    )
}

fn execute_actions(
    main_worktree: &Path,
    worktree: &Path,
    copy_from_main: &[RepositoryPath],
    command: Option<&[String]>,
) -> Result<()> {
    let mut created = Vec::new();
    let result = (|| {
        for configured in copy_from_main {
            created.extend(copy_configured_path(main_worktree, worktree, configured)?);
        }
        if let Some(command) = command {
            run_command(worktree, command)?;
        }
        Ok(())
    })();
    if let Err(error) = result {
        return match remove_created_paths(&created) {
            Ok(()) => Err(error),
            Err(rollback_error) => Err(anyhow::anyhow!(
                "{error:#}\nAdditionally, setup rollback failed: {rollback_error:#}"
            )),
        };
    }
    Ok(())
}

fn copy_configured_path(
    main_worktree: &Path,
    worktree: &Path,
    relative: &RepositoryPath,
) -> Result<Vec<CreatedPath>> {
    check_existing_ancestors(main_worktree, relative)?;
    let source = main_worktree.join(relative.as_path());
    if !entry_exists(&source)? {
        info!(
            "Warning: skipping '{}': source does not exist in the main worktree.",
            relative
        );
        return Ok(Vec::new());
    }

    check_existing_ancestors(worktree, relative)?;
    let destination = worktree.join(relative.as_path());
    if entry_exists(&destination)? {
        info!("Skipping '{}': destination already exists.", relative);
        return Ok(Vec::new());
    }
    let mut created = create_safe_parent_directories(worktree, relative)?;

    info!("Copying '{}' from the main worktree...", relative);
    if let Err(error) =
        copy_entry(&source, &destination).with_context(|| format!("Failed to copy '{relative}'"))
    {
        return match remove_created_paths(&created) {
            Ok(()) => Err(error),
            Err(rollback_error) => Err(anyhow::anyhow!(
                "{error:#}\nAdditionally, copy rollback failed: {rollback_error:#}"
            )),
        };
    }
    created.push(CreatedPath::Entry(destination));
    Ok(created)
}

fn check_existing_ancestors(root: &Path, relative: &RepositoryPath) -> Result<()> {
    let mut current = root.to_path_buf();
    let Some(parent) = relative.as_path().parent() else {
        return Ok(());
    };
    for component in parent.components() {
        current.push(component.as_os_str());
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                bail!(
                    "Refusing to copy '{}': ancestor {} is a symbolic link",
                    relative,
                    current.display()
                );
            }
            Ok(metadata) if metadata.is_dir() => {}
            Ok(_) => {
                bail!(
                    "Refusing to copy '{}': ancestor {} is not a directory",
                    relative,
                    current.display()
                );
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

fn create_safe_parent_directories(
    root: &Path,
    relative: &RepositoryPath,
) -> Result<Vec<CreatedPath>> {
    let mut created = Vec::new();
    let mut current = root.to_path_buf();
    let Some(parent) = relative.as_path().parent() else {
        return Ok(created);
    };
    for component in parent.components() {
        current.push(component.as_os_str());
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                remove_created_paths(&created)?;
                bail!(
                    "Refusing to use symbolic-link ancestor {}",
                    current.display()
                );
            }
            Ok(metadata) if metadata.is_dir() => {}
            Ok(_) => {
                remove_created_paths(&created)?;
                bail!("Path ancestor {} is not a directory", current.display());
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                match fs::create_dir(&current) {
                    Ok(()) => created.push(CreatedPath::Parent(current.clone())),
                    Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                    Err(error) => {
                        remove_created_paths(&created)?;
                        return Err(error.into());
                    }
                }
                let metadata = match fs::symlink_metadata(&current) {
                    Ok(metadata) => metadata,
                    Err(error) => {
                        remove_created_paths(&created)?;
                        return Err(error.into());
                    }
                };
                if metadata.file_type().is_symlink() || !metadata.is_dir() {
                    remove_created_paths(&created)?;
                    bail!(
                        "Path ancestor {} is not a safe directory",
                        current.display()
                    );
                }
            }
            Err(error) => {
                remove_created_paths(&created)?;
                return Err(error.into());
            }
        }
    }
    Ok(created)
}

fn remove_created_paths(paths: &[CreatedPath]) -> Result<()> {
    let mut failures = Vec::new();
    for created in paths.iter().rev() {
        let path = created.path();
        let removal = match (created, fs::symlink_metadata(path)) {
            (CreatedPath::Entry(_), Ok(metadata))
                if metadata.is_dir() && !metadata.file_type().is_symlink() =>
            {
                remove_directory_tree(path)
            }
            (CreatedPath::Entry(_), Ok(_)) => remove_file_for_rollback(path),
            (CreatedPath::Parent(_), Ok(_)) => fs::remove_dir(path),
            (_, Err(error)) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            (_, Err(error)) => Err(error),
        };
        if let Err(error) = removal {
            failures.push(format!("{}: {error}", path.display()));
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        bail!("{}", failures.join("; "))
    }
}

fn remove_directory_tree(path: &Path) -> io::Result<()> {
    prepare_tree_for_removal(path)?;
    fs::remove_dir_all(path)
}

fn prepare_tree_for_removal(path: &Path) -> io::Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() {
        return Ok(());
    }
    if !metadata.is_dir() {
        return make_file_removable(path, metadata.permissions());
    }

    make_directory_removable(path, metadata.permissions())?;
    for entry in fs::read_dir(path)? {
        prepare_tree_for_removal(&entry?.path())?;
    }
    Ok(())
}

#[cfg(unix)]
fn make_directory_removable(path: &Path, mut permissions: fs::Permissions) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    permissions.set_mode(permissions.mode() | 0o700);
    fs::set_permissions(path, permissions)
}

#[cfg(not(unix))]
fn make_directory_removable(path: &Path, mut permissions: fs::Permissions) -> io::Result<()> {
    permissions.set_readonly(false);
    fs::set_permissions(path, permissions)
}

#[cfg(unix)]
fn make_file_removable(_path: &Path, _permissions: fs::Permissions) -> io::Result<()> {
    Ok(())
}

#[cfg(not(unix))]
fn make_file_removable(path: &Path, mut permissions: fs::Permissions) -> io::Result<()> {
    permissions.set_readonly(false);
    fs::set_permissions(path, permissions)
}

fn remove_file_for_rollback(path: &Path) -> io::Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    make_file_removable(path, metadata.permissions())?;
    fs::remove_file(path)
}

enum CreatedPath {
    Entry(PathBuf),
    Parent(PathBuf),
}

impl CreatedPath {
    fn path(&self) -> &Path {
        match self {
            Self::Entry(path) | Self::Parent(path) => path,
        }
    }
}

fn entry_exists(path: &Path) -> io::Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

fn copy_entry(source: &Path, destination: &Path) -> io::Result<()> {
    let metadata = fs::symlink_metadata(source)?;
    if metadata.file_type().is_symlink() {
        create_symlink(&fs::read_link(source)?, destination)
    } else if metadata.is_dir() {
        copy_directory(source, destination, &metadata)
    } else if metadata.is_file() {
        copy_file(source, destination, &metadata)
    } else {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            format!("unsupported file type at {}", source.display()),
        ))
    }
}

fn copy_directory(source: &Path, destination: &Path, metadata: &fs::Metadata) -> io::Result<()> {
    fs::create_dir(destination)?;
    let result = (|| {
        for entry in fs::read_dir(source)? {
            let entry = entry?;
            copy_entry(&entry.path(), &destination.join(entry.file_name()))?;
        }
        fs::set_permissions(destination, metadata.permissions())
    })();
    if let Err(error) = result {
        return match remove_directory_tree(destination) {
            Ok(()) => Err(error),
            Err(rollback_error) => Err(io::Error::other(format!(
                "{error}; additionally, copy rollback failed: {rollback_error}"
            ))),
        };
    }
    Ok(())
}

fn copy_file(source: &Path, destination: &Path, metadata: &fs::Metadata) -> io::Result<()> {
    let mut source = File::open(source)?;
    let mut destination_file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)?;
    let result = (|| {
        io::copy(&mut source, &mut destination_file)?;
        destination_file.flush()?;
        fs::set_permissions(destination, metadata.permissions())
    })();
    if result.is_err() {
        drop(destination_file);
        let _ = fs::remove_file(destination);
    }
    result
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

    fn repository_path(value: &str) -> RepositoryPath {
        RepositoryPath::new(value).unwrap()
    }

    #[test]
    fn copies_files_and_directories() {
        let main = tempdir().unwrap();
        let worktree = tempdir().unwrap();
        fs::write(main.path().join(".env"), "secret").unwrap();
        fs::create_dir(main.path().join("config")).unwrap();
        fs::write(main.path().join("config/local.toml"), "value = 1").unwrap();

        copy_configured_path(main.path(), worktree.path(), &repository_path(".env")).unwrap();
        copy_configured_path(main.path(), worktree.path(), &repository_path("config")).unwrap();

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

        copy_configured_path(main.path(), worktree.path(), &repository_path(".env")).unwrap();

        assert_eq!(
            fs::read_to_string(worktree.path().join(".env")).unwrap(),
            "worktree"
        );
    }

    #[test]
    fn exclusive_file_creation_prevents_check_then_write_overwrites() {
        let main = tempdir().unwrap();
        let worktree = tempdir().unwrap();
        let source = main.path().join(".env");
        let destination = worktree.path().join(".env");
        fs::write(&source, "main").unwrap();
        fs::write(&destination, "created after the initial check").unwrap();

        let error = copy_entry(&source, &destination).unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(
            fs::read_to_string(destination).unwrap(),
            "created after the initial check"
        );
    }

    #[test]
    fn missing_source_is_not_an_error() {
        let main = tempdir().unwrap();
        let worktree = tempdir().unwrap();
        copy_configured_path(main.path(), worktree.path(), &repository_path(".env")).unwrap();
        assert!(!worktree.path().join(".env").exists());
    }

    #[cfg(unix)]
    #[test]
    fn copies_leaf_symbolic_links_without_dereferencing() {
        let main = tempdir().unwrap();
        let worktree = tempdir().unwrap();
        fs::write(main.path().join("target.txt"), "target").unwrap();
        std::os::unix::fs::symlink("target.txt", main.path().join("link.txt")).unwrap();

        copy_configured_path(main.path(), worktree.path(), &repository_path("link.txt")).unwrap();

        let destination = worktree.path().join("link.txt");
        assert!(fs::symlink_metadata(&destination).unwrap().is_symlink());
        assert_eq!(fs::read_link(destination).unwrap(), Path::new("target.txt"));
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symbolic_link_ancestors_on_both_sides() {
        let main = tempdir().unwrap();
        let worktree = tempdir().unwrap();
        let outside = tempdir().unwrap();
        fs::write(outside.path().join("local.toml"), "outside").unwrap();
        std::os::unix::fs::symlink(outside.path(), main.path().join("source-link")).unwrap();
        assert!(
            copy_configured_path(
                main.path(),
                worktree.path(),
                &repository_path("source-link/local.toml")
            )
            .is_err()
        );

        fs::create_dir(main.path().join("config")).unwrap();
        fs::write(main.path().join("config/local.toml"), "source").unwrap();
        std::os::unix::fs::symlink(outside.path(), worktree.path().join("config")).unwrap();
        assert!(
            copy_configured_path(
                main.path(),
                worktree.path(),
                &repository_path("config/local.toml")
            )
            .is_err()
        );
        assert_eq!(
            fs::read_to_string(outside.path().join("local.toml")).unwrap(),
            "outside"
        );
    }

    #[cfg(unix)]
    #[test]
    fn removes_partial_directory_after_copy_failure() {
        use std::os::unix::net::UnixListener;

        let main = tempdir().unwrap();
        let worktree = tempdir().unwrap();
        fs::create_dir(main.path().join("bundle")).unwrap();
        fs::write(main.path().join("bundle/file"), "value").unwrap();
        let _socket = UnixListener::bind(main.path().join("bundle/socket")).unwrap();

        assert!(
            copy_configured_path(main.path(), worktree.path(), &repository_path("bundle")).is_err()
        );
        assert!(!worktree.path().join("bundle").exists());
    }

    #[cfg(unix)]
    #[test]
    fn runs_commands_with_literal_arguments_in_the_worktree() {
        let worktree = tempdir().unwrap();
        let command = vec![
            "sh".to_string(),
            "-c".to_string(),
            "printf '%s' \"$1\" > literal; pwd > cwd".to_string(),
            "copsy-test".to_string(),
            "$HOME".to_string(),
        ];

        run_command(worktree.path(), &command).unwrap();

        assert_eq!(
            fs::read_to_string(worktree.path().join("literal")).unwrap(),
            "$HOME"
        );
        assert_eq!(
            fs::read_to_string(worktree.path().join("cwd"))
                .unwrap()
                .trim(),
            fs::canonicalize(worktree.path()).unwrap().to_string_lossy()
        );
    }

    #[cfg(unix)]
    #[test]
    fn reports_failing_commands() {
        let worktree = tempdir().unwrap();
        let command = vec!["sh".to_string(), "-c".to_string(), "exit 7".to_string()];
        assert!(run_command(worktree.path(), &command).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn rolls_back_new_copies_when_the_command_fails() {
        let main = tempdir().unwrap();
        let worktree = tempdir().unwrap();
        fs::write(main.path().join(".env"), "main").unwrap();
        fs::create_dir(main.path().join("config")).unwrap();
        fs::write(main.path().join("config/local.toml"), "main").unwrap();
        fs::create_dir(main.path().join("bundle")).unwrap();
        fs::write(main.path().join("bundle/file"), "main").unwrap();
        fs::write(worktree.path().join("existing"), "keep").unwrap();
        let paths = [
            repository_path(".env"),
            repository_path("config/local.toml"),
            repository_path("bundle"),
        ];
        let command = ["sh".to_string(), "-c".to_string(), "exit 7".to_string()];

        assert!(execute_actions(main.path(), worktree.path(), &paths, Some(&command)).is_err());

        assert!(!worktree.path().join(".env").exists());
        assert!(!worktree.path().join("config").exists());
        assert!(!worktree.path().join("bundle").exists());
        assert_eq!(
            fs::read_to_string(worktree.path().join("existing")).unwrap(),
            "keep"
        );
    }

    #[cfg(unix)]
    #[test]
    fn rollback_preserves_destinations_that_already_existed() {
        let main = tempdir().unwrap();
        let worktree = tempdir().unwrap();
        fs::write(main.path().join(".env"), "main").unwrap();
        fs::write(worktree.path().join(".env"), "worktree").unwrap();
        let paths = [repository_path(".env")];
        let command = ["sh".to_string(), "-c".to_string(), "exit 7".to_string()];

        assert!(execute_actions(main.path(), worktree.path(), &paths, Some(&command)).is_err());

        assert_eq!(
            fs::read_to_string(worktree.path().join(".env")).unwrap(),
            "worktree"
        );
    }

    #[cfg(unix)]
    #[test]
    fn rollback_removes_copied_read_only_directories() {
        use std::os::unix::fs::PermissionsExt;

        let main = tempdir().unwrap();
        let worktree = tempdir().unwrap();
        let locked = main.path().join("bundle/locked");
        fs::create_dir_all(&locked).unwrap();
        fs::write(locked.join("file"), "main").unwrap();
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o500)).unwrap();
        let paths = [repository_path("bundle")];
        let command = ["sh".to_string(), "-c".to_string(), "exit 7".to_string()];

        assert!(execute_actions(main.path(), worktree.path(), &paths, Some(&command)).is_err());

        assert!(!worktree.path().join("bundle").exists());
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o700)).unwrap();
    }
}
