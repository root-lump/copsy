use crate::config::{Config, SetupTrigger};
use crate::git;
use crate::info;
use crate::repository_path::RepositoryPath;
use anyhow::{Context, Result, bail};
use console::{Key, Term};
use dialoguer::{MultiSelect, Select, theme::ColorfulTheme};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

const NO_COPY_LABEL: &str = "Do not copy anything";
const NO_AUTO_LABEL: &str = "Do not run setup automatically";

struct ConfigDraft {
    base_dir: Option<String>,
    inherited_base_dir: Option<String>,
    auto: Vec<SetupTrigger>,
    copy_from_main: Vec<RepositoryPath>,
    copy_examples: Vec<RepositoryPath>,
}

pub fn run_init() -> Result<()> {
    let destination = git::repository_config_path()?;
    if destination.exists() {
        bail!(
            "Repository config already exists at {}",
            destination.display()
        );
    }

    let global = Config::load_global()?;
    let main_worktree = git::main_worktree_path()?;
    let ignored = git::list_ignored_paths(&main_worktree)?;
    let theme = ColorfulTheme::default();

    let global_label = match global.base_dir_raw() {
        Some(path) => format!("Inherit global setting: {path}"),
        None => "Inherit global setting: not set (use copsy default)".to_string(),
    };
    let path_options = [global_label, "Override for this repository".to_string()];
    let Some(path_selection) = Select::with_theme(&theme)
        .with_prompt("Worktree base directory")
        .items(&path_options)
        .default(0)
        .interact_opt()?
    else {
        return Ok(());
    };

    let base_dir = if path_selection == 1 {
        let Some(path) = OptionalInput::new("Base directory for this repository").interact_opt()?
        else {
            return Ok(());
        };
        Some(path)
    } else {
        None
    };

    let copy_from_main = select_copy_paths(&theme, &ignored)?;
    let Some(copy_from_main) = copy_from_main else {
        return Ok(());
    };
    let auto = select_auto_triggers(&theme)?;
    let Some(auto) = auto else {
        return Ok(());
    };

    let copy_examples = if ignored.is_empty() {
        vec![
            RepositoryPath::new(".env")?,
            RepositoryPath::new(".env.local")?,
        ]
    } else {
        ignored
    };
    let draft = ConfigDraft {
        base_dir,
        inherited_base_dir: global.base_dir_raw().map(str::to_owned),
        auto,
        copy_from_main,
        copy_examples,
    };

    persist_config(&destination, &render_config(&draft))?;
    info!("Created {}", destination.display());
    Ok(())
}

fn select_copy_paths(
    theme: &ColorfulTheme,
    ignored: &[RepositoryPath],
) -> Result<Option<Vec<RepositoryPath>>> {
    if ignored.is_empty() {
        return Ok(Some(Vec::new()));
    }

    let mut options = vec![NO_COPY_LABEL.to_string()];
    options.extend(ignored.iter().map(|path| path.as_str().to_string()));
    let mut defaults = vec![false; options.len()];
    defaults[0] = true;
    let Some(selected) = MultiSelect::with_theme(theme)
        .with_prompt("Copy files from the main worktree")
        .items(&options)
        .defaults(&defaults)
        .interact_opt()?
    else {
        return Ok(None);
    };

    Ok(Some(
        selected
            .into_iter()
            .filter(|index| *index > 0)
            .map(|index| ignored[index - 1].clone())
            .collect(),
    ))
}

fn select_auto_triggers(theme: &ColorfulTheme) -> Result<Option<Vec<SetupTrigger>>> {
    let mut options = vec![NO_AUTO_LABEL];
    options.extend(SetupTrigger::ALL.map(SetupTrigger::prompt_label));
    let mut defaults = vec![false; options.len()];
    defaults[0] = true;
    let Some(selected) = MultiSelect::with_theme(theme)
        .with_prompt("Automatically run setup for")
        .items(&options)
        .defaults(&defaults)
        .interact_opt()?
    else {
        return Ok(None);
    };

    Ok(Some(
        selected
            .into_iter()
            .filter(|index| *index > 0)
            .map(|index| SetupTrigger::ALL[index - 1])
            .collect(),
    ))
}

fn persist_config(destination: &Path, content: &str) -> Result<()> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)
        .with_context(|| format!("Failed to create {}", destination.display()))?;
    file.write_all(content.as_bytes())
        .with_context(|| format!("Failed to write {}", destination.display()))
}

fn render_config(draft: &ConfigDraft) -> String {
    let mut lines = Vec::new();
    if let Some(base_dir) = &draft.base_dir {
        lines.push("[worktree]".to_string());
        lines.push(format!("base_dir = {}", quote_toml(base_dir)));
    } else {
        lines.push("# [worktree]".to_string());
        lines.push(format!(
            "# base_dir = {}",
            quote_toml(draft.inherited_base_dir.as_deref().unwrap_or("~/worktrees"))
        ));
    }

    lines.push("[setup]".to_string());
    if draft.auto.is_empty() {
        lines.push("# auto = [\"new\"]".to_string());
    } else {
        let values = draft
            .auto
            .iter()
            .map(|trigger| quote_toml(trigger.config_value()))
            .collect::<Vec<_>>()
            .join(", ");
        lines.push(format!("auto = [{values}]"));
    }
    lines.push("# command = [\"npm\", \"install\"]".to_string());

    if draft.copy_from_main.is_empty() {
        render_copy_array(&mut lines, &draft.copy_examples, true);
    } else {
        render_copy_array(&mut lines, &draft.copy_from_main, false);
    }
    format!("{}\n", lines.join("\n"))
}

fn render_copy_array(lines: &mut Vec<String>, paths: &[RepositoryPath], commented: bool) {
    let prefix = if commented { "# " } else { "" };
    lines.push(format!("{prefix}copy_from_main = ["));
    for path in paths {
        lines.push(format!("{prefix}  {},", quote_toml(path.as_str())));
    }
    lines.push(format!("{prefix}]"));
}

fn quote_toml(value: &str) -> String {
    let mut quoted = String::from("\"");
    for value in value.chars() {
        match value {
            '\u{0008}' => quoted.push_str("\\b"),
            '\t' => quoted.push_str("\\t"),
            '\n' => quoted.push_str("\\n"),
            '\u{000C}' => quoted.push_str("\\f"),
            '\r' => quoted.push_str("\\r"),
            '"' => quoted.push_str("\\\""),
            '\\' => quoted.push_str("\\\\"),
            value if value.is_control() => {
                quoted.push_str(&format!("\\u{:04X}", value as u32));
            }
            value => quoted.push(value),
        }
    }
    quoted.push('"');
    quoted
}

struct OptionalInput<'a> {
    prompt: &'a str,
}

impl<'a> OptionalInput<'a> {
    fn new(prompt: &'a str) -> Self {
        Self { prompt }
    }

    // dialoguer's Input has no optional interaction API, so this small input
    // keeps path entry cancellable with Esc like the surrounding dialogs.
    fn interact_opt(&self) -> Result<Option<String>> {
        let term = Term::stderr();
        let mut input = String::new();
        self.redraw(&term, &input)?;

        loop {
            match term.read_key()? {
                Key::Escape => {
                    term.clear_line()?;
                    return Ok(None);
                }
                Key::Enter if !input.is_empty() => {
                    term.write_line("")?;
                    return Ok(Some(input));
                }
                Key::Backspace if !input.is_empty() => {
                    input.pop();
                    self.redraw(&term, &input)?;
                }
                Key::Char(value) if !value.is_control() => {
                    input.push(value);
                    self.redraw(&term, &input)?;
                }
                _ => {}
            }
        }
    }

    fn redraw(&self, term: &Term, input: &str) -> Result<()> {
        term.clear_line()?;
        term.write_str(&format!("? {} › {}", self.prompt, input))?;
        term.flush()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn path(value: &str) -> RepositoryPath {
        RepositoryPath::new(value).unwrap()
    }

    fn draft() -> ConfigDraft {
        ConfigDraft {
            base_dir: None,
            inherited_base_dir: Some("~/worktrees".to_string()),
            auto: Vec::new(),
            copy_from_main: Vec::new(),
            copy_examples: vec![path(".env"), path("target")],
        }
    }

    #[test]
    fn renders_commented_defaults_without_blank_lines() {
        let result = render_config(&draft());

        assert_eq!(
            result,
            concat!(
                "# [worktree]\n",
                "# base_dir = \"~/worktrees\"\n",
                "[setup]\n",
                "# auto = [\"new\"]\n",
                "# command = [\"npm\", \"install\"]\n",
                "# copy_from_main = [\n",
                "#   \".env\",\n",
                "#   \"target\",\n",
                "# ]\n"
            )
        );
        assert!(!result.contains("\n\n"));
    }

    #[test]
    fn renders_typed_selections_and_multiline_copy() {
        let draft = ConfigDraft {
            base_dir: Some("~/repo-worktrees".to_string()),
            inherited_base_dir: None,
            auto: vec![SetupTrigger::New, SetupTrigger::Add],
            copy_from_main: vec![path(".env"), path("config/local.toml")],
            copy_examples: Vec::new(),
        };

        assert_eq!(
            render_config(&draft),
            concat!(
                "[worktree]\n",
                "base_dir = \"~/repo-worktrees\"\n",
                "[setup]\n",
                "auto = [\"new\", \"add\"]\n",
                "# command = [\"npm\", \"install\"]\n",
                "copy_from_main = [\n",
                "  \".env\",\n",
                "  \"config/local.toml\",\n",
                "]\n"
            )
        );
    }

    #[test]
    fn escapes_toml_strings() {
        assert_eq!(quote_toml("a\"b\\c"), "\"a\\\"b\\\\c\"");
    }

    #[test]
    fn persistence_does_not_overwrite_existing_config() {
        let directory = tempdir().unwrap();
        let destination = directory.path().join("copsy.toml");
        fs::write(&destination, "existing").unwrap();

        assert!(persist_config(&destination, "replacement").is_err());
        assert_eq!(fs::read_to_string(destination).unwrap(), "existing");
    }
}
