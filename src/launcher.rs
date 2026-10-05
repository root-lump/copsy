use crate::cli::LaunchOptions;
use crate::output;

// Editors first so they're open when AI tools start interactive sessions
pub fn launch_tools(
    flags: &LaunchOptions,
    worktree_path: &std::path::Path,
    change_directory: bool,
) {
    if flags.code {
        output::request_launch("code", worktree_path);
    }
    if flags.cursor {
        output::request_launch("cursor", worktree_path);
    }
    if let Some(cmd) = &flags.open {
        if change_directory {
            output::request_open(cmd);
        } else {
            // The marker remains compatible with existing shell wrappers. Quote
            // the path, and confine the user's command to a child shell so even
            // an explicit cd cannot move Herdr's calling terminal.
            let path = worktree_path.to_string_lossy().replace('\'', "'\\''");
            let command = cmd.replace('\'', "'\\''");
            output::request_open(&format!("(cd '{path}' && eval '{command}')"));
        }
    }
    if flags.claude {
        output::request_launch("claude", worktree_path);
    }
    if flags.codex {
        output::request_launch("codex", worktree_path);
    }
}
