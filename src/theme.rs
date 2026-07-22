use anyhow::{Result, bail};
use console::style;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

/// Run fzf with the given items and prompt. Returns the selected index,
/// or None if the user cancelled (Esc / Ctrl-C).
pub fn fuzzy_select(items: &[String], prompt: &str) -> Result<Option<usize>> {
    let input = items.join("\n");

    let mut child = Command::new("fzf")
        .args([
            "--ansi",
            "--reverse",
            "--height=~50%",
            "--prompt",
            &format!("{prompt} > "),
            "--pointer=❯",
            "--color=pointer:green,prompt:bold,fg+:green:bold,bg+:236,hl:green,hl+:green:bold",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()?;

    child.stdin.take().unwrap().write_all(input.as_bytes())?;

    let output = child.wait_with_output()?;

    match output.status.code() {
        Some(0) => {}
        Some(1) => return Ok(None),   // no match
        Some(130) => return Ok(None), // cancelled
        _ => bail!("fzf exited with status {}", output.status),
    }

    let selected = String::from_utf8(output.stdout)?.trim().to_string();
    let idx = items.iter().position(|item| *item == selected);
    Ok(idx)
}

/// Format a worktree entry for display in fuzzy select lists.
pub fn format_worktree(branch: &str, path: &Path, is_main: bool) -> String {
    let label = if is_main {
        style("[repo]").blue().bold()
    } else {
        style("[worktree]").magenta().bold()
    };
    format!(
        "{} {} {}",
        label,
        style(branch).bold(),
        style(path.display()).dim()
    )
}
