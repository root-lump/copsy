use crate::config::Config;
use crate::git;
use crate::info;
use anyhow::{Context, Result, bail};
use console::{Key, Term};
use dialoguer::{MultiSelect, Select, theme::ColorfulTheme};
use std::fs::OpenOptions;
use std::io::Write;

const NO_COPY_LABEL: &str = "Do not copy anything";
const NO_AUTO_LABEL: &str = "Do not run setup automatically";

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

    let copy_from_main = if ignored.is_empty() {
        Vec::new()
    } else {
        let mut options = vec![NO_COPY_LABEL.to_string()];
        options.extend(ignored.iter().cloned());
        let mut defaults = vec![false; options.len()];
        defaults[0] = true;
        let Some(selected) = MultiSelect::with_theme(&theme)
            .with_prompt("Copy files from the main worktree")
            .items(&options)
            .defaults(&defaults)
            .interact_opt()?
        else {
            return Ok(());
        };
        selected
            .into_iter()
            .filter(|index| *index > 0)
            .map(|index| options[index].clone())
            .collect()
    };

    let auto_options = [
        NO_AUTO_LABEL,
        "copsy new",
        "copsy add",
        "copsy pr (may execute untrusted code)",
    ];
    let auto_defaults = [true, false, false, false];
    let Some(selected_auto) = MultiSelect::with_theme(&theme)
        .with_prompt("Automatically run setup for")
        .items(auto_options)
        .defaults(&auto_defaults)
        .interact_opt()?
    else {
        return Ok(());
    };
    let auto = selected_auto
        .into_iter()
        .filter_map(|index| match index {
            1 => Some("new"),
            2 => Some("add"),
            3 => Some("pr"),
            _ => None,
        })
        .collect::<Vec<_>>();

    let copy_examples = if ignored.is_empty() {
        vec![".env".to_string(), ".env.local".to_string()]
    } else {
        ignored
    };
    let content = render_config(
        base_dir.as_deref(),
        global.base_dir_raw(),
        &auto,
        &copy_from_main,
        &copy_examples,
    );

    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&destination)
        .with_context(|| format!("Failed to create {}", destination.display()))?;
    file.write_all(content.as_bytes())
        .with_context(|| format!("Failed to write {}", destination.display()))?;

    info!("Created {}", destination.display());
    Ok(())
}

fn render_config(
    base_dir: Option<&str>,
    global_base_dir: Option<&str>,
    auto: &[&str],
    copy_from_main: &[String],
    copy_examples: &[String],
) -> String {
    let mut lines = Vec::new();
    if let Some(base_dir) = base_dir {
        lines.push("[worktree]".to_string());
        lines.push(format!("base_dir = {}", quote_toml(base_dir)));
    } else {
        lines.push("# [worktree]".to_string());
        lines.push(format!(
            "# base_dir = {}",
            quote_toml(global_base_dir.unwrap_or("~/worktrees"))
        ));
    }

    lines.push("[setup]".to_string());
    if auto.is_empty() {
        lines.push("# auto = [\"new\"]".to_string());
    } else {
        let values = auto
            .iter()
            .map(|value| quote_toml(value))
            .collect::<Vec<_>>()
            .join(", ");
        lines.push(format!("auto = [{values}]"));
    }
    lines.push("# command = [\"npm\", \"install\"]".to_string());

    if copy_from_main.is_empty() {
        render_copy_array(&mut lines, copy_examples, true);
    } else {
        render_copy_array(&mut lines, copy_from_main, false);
    }
    format!("{}\n", lines.join("\n"))
}

fn render_copy_array(lines: &mut Vec<String>, paths: &[String], commented: bool) {
    let prefix = if commented { "# " } else { "" };
    lines.push(format!("{prefix}copy_from_main = ["));
    for path in paths {
        lines.push(format!("{prefix}  {},", quote_toml(path)));
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

    #[test]
    fn renders_commented_defaults_without_blank_lines() {
        let examples = vec![".env".to_string(), "target".to_string()];
        let result = render_config(None, Some("~/worktrees"), &[], &[], &examples);

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
    fn renders_selected_copy_as_multiline_array() {
        let selected = vec![".env".to_string()];
        let result = render_config(
            Some("~/repo-worktrees"),
            None,
            &["new", "add"],
            &selected,
            &selected,
        );

        assert_eq!(
            result,
            concat!(
                "[worktree]\n",
                "base_dir = \"~/repo-worktrees\"\n",
                "[setup]\n",
                "auto = [\"new\", \"add\"]\n",
                "# command = [\"npm\", \"install\"]\n",
                "copy_from_main = [\n",
                "  \".env\",\n",
                "]\n"
            )
        );
    }

    #[test]
    fn escapes_toml_strings() {
        assert_eq!(quote_toml("a\"b\\c"), "\"a\\\"b\\\\c\"");
    }
}
