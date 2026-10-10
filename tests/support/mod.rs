use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use tempfile::{TempDir, tempdir};

pub struct Repository {
    _temp: TempDir,
    pub root: PathBuf,
    pub main: PathBuf,
    pub home: PathBuf,
    pub config: PathBuf,
}

impl Repository {
    pub fn new() -> Self {
        let temp = tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let main = root.join("local-clone");
        let home = root.join("home");
        let config = root.join("config");
        for path in [&main, &home, &config] {
            fs::create_dir(path).unwrap();
        }
        git(&main, &["init", "-q", "-b", "main"]);
        git(&main, &["commit", "-q", "--allow-empty", "-m", "initial"]);
        git(
            &main,
            &[
                "remote",
                "add",
                "origin",
                "https://example.invalid/remote-name.git",
            ],
        );
        Self {
            _temp: temp,
            root,
            main,
            home,
            config,
        }
    }

    pub fn command(&self, cwd: &Path) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_copsy"));
        command
            .current_dir(cwd)
            .env("HOME", &self.home)
            .env("USERPROFILE", &self.home)
            .env("XDG_CONFIG_HOME", &self.config)
            .env_remove("HERDR_CONFIG_PATH")
            .env_remove("HERDR_ENV")
            .env_remove("HERDR_WORKSPACE_ID")
            .env_remove("HERDR_PANE_ID")
            .env_remove("HERDR_API_SOCKET")
            .env("GIT_CONFIG_GLOBAL", self.root.join("no-global-config"))
            .env("GIT_CONFIG_NOSYSTEM", "1");
        command
    }

    pub fn run(&self, cwd: &Path, arguments: &[&str]) -> Output {
        self.command(cwd).args(arguments).output().unwrap()
    }
}

pub fn git(cwd: &Path, arguments: &[&str]) -> Output {
    let output = Command::new("git")
        .current_dir(cwd)
        .args([
            "-c",
            "user.name=Copsy Test",
            "-c",
            "user.email=copsy@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "-c",
            "core.hooksPath=/dev/null",
        ])
        .args(arguments)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {arguments:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    output
}
