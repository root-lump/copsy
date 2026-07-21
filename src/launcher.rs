use crate::cli::LaunchFlags;
use crate::output;

// Editors first so they're open when AI tools start interactive sessions
pub fn launch_tools(flags: &LaunchFlags, worktree_path: &std::path::Path) {
    if flags.code {
        output::request_launch("code", worktree_path);
    }
    if flags.cursor {
        output::request_launch("cursor", worktree_path);
    }
    if let Some(cmd) = &flags.open {
        output::request_open(cmd);
    }
    if flags.claude {
        output::request_launch("claude", worktree_path);
    }
    if flags.codex {
        output::request_launch("codex", worktree_path);
    }
}
