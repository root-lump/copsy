use anyhow::{Result, bail};
use std::fmt;
use std::path::{Component, Path};

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct RepositoryPath(String);

impl RepositoryPath {
    pub fn new(value: impl Into<String>) -> Result<Self> {
        let value = value.into();
        if has_explicit_dot_component(&value) {
            bail!("current-directory components are not allowed");
        }
        validate(Path::new(&value))?;
        Ok(Self(value))
    }

    pub fn as_path(&self) -> &Path {
        Path::new(&self.0)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

fn has_explicit_dot_component(value: &str) -> bool {
    value.split('/').any(|component| component == ".")
        || (cfg!(windows) && value.split('\\').any(|component| component == "."))
}

impl fmt::Display for RepositoryPath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

fn validate(path: &Path) -> Result<()> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_repository_relative_paths() {
        for path in [".env", "config/local.toml", "target"] {
            assert!(RepositoryPath::new(path).is_ok(), "{path}");
        }
    }

    #[test]
    fn rejects_paths_outside_the_repository_namespace() {
        for path in [
            "",
            ".",
            "./config",
            "config/./local.toml",
            "config/.",
            "../secret",
            "/tmp/secret",
            ".git",
            "nested/.git/config",
        ] {
            assert!(RepositoryPath::new(path).is_err(), "{path}");
        }
    }
}
