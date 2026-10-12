//! Workspace registration and deferred launches through the Herdr session API.

use crate::cli::LaunchOptions;
use crate::info;
use crate::output;
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

// Shell startup took about 1-1.5 seconds in observed Herdr sessions. Polling
// more frequently keeps the common case responsive, while ten seconds leaves
// room for slower startup files without waiting indefinitely.
const INITIAL_PANE_POLL_INTERVAL: Duration = Duration::from_millis(100);
const INITIAL_PANE_WAIT_TIMEOUT: Duration = Duration::from_secs(10);
// Startup commands can leave the shell briefly alone between processes, so a
// single ready observation is not enough to prove that startup has settled.
const INITIAL_PANE_READY_OBSERVATIONS: u8 = 3;

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
        // Only a workspace created by this request has an unused initial pane.
        // Older responses without an explicit freshness signal use new tabs.
        let initial_pane = if response.result.already_open == Some(false) && source != target {
            response.result.root_pane
        } else {
            None
        };
        if let Some(pane) = &initial_pane {
            validate_pane(pane, &workspace_id)?;
        }
        // Setup executes in the shell wrapper after this process exits. Deferring
        // launches ensures no agent starts before setup has completed successfully.
        let request = LaunchRequest {
            workspace_id,
            path: target.to_path_buf(),
            launch: launch.clone(),
            initial_pane,
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
    already_open: Option<bool>,
    root_pane: Option<PaneIdentity>,
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

#[derive(Deserialize, Serialize)]
struct PaneIdentity {
    workspace_id: String,
    pane_id: String,
}

#[derive(Deserialize)]
struct PaneResponse {
    result: PaneResult,
}

#[derive(Deserialize)]
struct PaneResult {
    pane: PaneState,
}

#[derive(Deserialize)]
struct PaneState {
    #[serde(flatten)]
    identity: PaneIdentity,
    agent: Option<String>,
    agent_session: Option<serde_json::Value>,
}

#[derive(Deserialize)]
struct ProcessResponse {
    result: ProcessResult,
}

#[derive(Deserialize)]
struct ProcessResult {
    process_info: PaneProcesses,
}

#[derive(Deserialize)]
struct PaneProcesses {
    pane_id: String,
    shell_pid: Option<u32>,
    foreground_process_group_id: Option<u32>,
    #[serde(default)]
    foreground_processes: Vec<ForegroundProcess>,
}

#[derive(Deserialize)]
struct ForegroundProcess {
    pid: u32,
    name: String,
    cwd: Option<PathBuf>,
}

enum InitialPaneState {
    Ready,
    Starting,
    Unavailable,
}

#[derive(Deserialize, Serialize)]
struct LaunchRequest {
    workspace_id: String,
    path: PathBuf,
    launch: LaunchOptions,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    initial_pane: Option<PaneIdentity>,
}

pub fn launch_tools(request: &str) -> Result<()> {
    if !inside_herdr() {
        bail!("Herdr tool launches require running copsy inside Herdr");
    }
    let mut request: LaunchRequest =
        serde_json::from_str(request).context("Invalid deferred Herdr launch request")?;
    validate_id(&request.workspace_id)?;
    let mut initial_pane = request.initial_pane.take();
    if let Some(pane) = &initial_pane {
        validate_pane(pane, &request.workspace_id)?;
    }
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
        // Consume the initial pane even on failure: never submit another tool
        // into a pane whose previous command may already have been delivered.
        if let Err(error) = launch_in_tab(&request, label, command, initial_pane.take()) {
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

fn launch_in_tab(
    request: &LaunchRequest,
    label: &str,
    command: &str,
    initial_pane: Option<PaneIdentity>,
) -> Result<()> {
    // Setup may have taken long enough for someone to start using the initial
    // pane. Registration-time ownership alone is no longer sufficient here.
    let initial_pane = initial_pane.filter(|pane| match wait_for_initial_pane(request, pane) {
        Ok(true) => true,
        Ok(false) => false,
        Err(error) => {
            info!("Herdr initial pane could not be verified; using a new tab: {error:#}");
            false
        }
    });
    let pane = if let Some(pane) = initial_pane {
        pane
    } else {
        create_tab(request, label)?
    };
    // Both the initial pane and additional tabs start in the target checkout.
    // Known tools use fixed commands; only --open contributes shell source.
    run_herdr(
        Command::new("herdr").args(["pane", "run", &pane.pane_id, command]),
        "pane run",
    )
    .with_context(|| format!("Command submission to pane {} failed", pane.pane_id))?;
    Ok(())
}

fn wait_for_initial_pane(request: &LaunchRequest, pane: &PaneIdentity) -> Result<bool> {
    let started_at = Instant::now();
    let mut ready_observations = 0;
    loop {
        match initial_pane_state(request, pane)? {
            InitialPaneState::Ready => {
                ready_observations += 1;
                if ready_observations == INITIAL_PANE_READY_OBSERVATIONS {
                    return Ok(true);
                }
            }
            InitialPaneState::Starting => ready_observations = 0,
            InitialPaneState::Unavailable => {
                info!("Herdr initial pane is in use or could not be verified; using a new tab");
                return Ok(false);
            }
        }
        if started_at.elapsed() >= INITIAL_PANE_WAIT_TIMEOUT {
            info!("Herdr initial pane is still starting; using a new tab");
            return Ok(false);
        }
        std::thread::sleep(INITIAL_PANE_POLL_INTERVAL);
    }
}

fn initial_pane_state(request: &LaunchRequest, pane: &PaneIdentity) -> Result<InitialPaneState> {
    let output = run_herdr(
        Command::new("herdr").args(["pane", "get", &pane.pane_id]),
        "pane get",
    )?;
    let state: PaneResponse = serde_json::from_slice(&output.stdout)?;
    let state = state.result.pane;
    if state.identity.pane_id != pane.pane_id
        || state.identity.workspace_id != request.workspace_id
        || state.agent.is_some()
        || state.agent_session.is_some()
    {
        return Ok(InitialPaneState::Unavailable);
    }
    let output = run_herdr(
        Command::new("herdr").args(["pane", "process-info", "--pane", &pane.pane_id]),
        "pane process-info",
    )?;
    let processes: ProcessResponse = serde_json::from_slice(&output.stdout)?;
    let processes = processes.result.process_info;
    if processes.pane_id != pane.pane_id {
        return Ok(InitialPaneState::Unavailable);
    }
    let Some(shell_pid) = processes.shell_pid.filter(|pid| *pid != 0) else {
        return Ok(InitialPaneState::Unavailable);
    };
    // Startup helpers remain in the shell's foreground process group, while
    // job-controlled commands use a different group and mean the pane is busy.
    if processes.foreground_process_group_id != Some(shell_pid) {
        return Ok(InitialPaneState::Unavailable);
    }
    let Some(process) = processes
        .foreground_processes
        .iter()
        .find(|process| process.pid == shell_pid)
    else {
        return Ok(InitialPaneState::Unavailable);
    };
    let name = process
        .name
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or("")
        .trim_start_matches('-')
        .to_ascii_lowercase();
    let shell_name = name.strip_suffix(".exe").unwrap_or(&name);
    // An agent started with exec can retain the shell PID, so check the live
    // executable too. Unknown processes or unavailable cwd data are not safe.
    let known_shell = matches!(
        shell_name,
        "sh" | "bash"
            | "dash"
            | "zsh"
            | "fish"
            | "ksh"
            | "mksh"
            | "csh"
            | "tcsh"
            | "elvish"
            | "xonsh"
            | "nu"
            | "pwsh"
            | "powershell"
            | "cmd"
    );
    let same_directory = process
        .cwd
        .as_ref()
        .and_then(|path| path.canonicalize().ok())
        .zip(request.path.canonicalize().ok())
        .is_some_and(|(actual, target)| actual == target);
    if !known_shell || !same_directory {
        return Ok(InitialPaneState::Unavailable);
    }
    if processes.foreground_processes.len() == 1 {
        Ok(InitialPaneState::Ready)
    } else {
        Ok(InitialPaneState::Starting)
    }
}

fn create_tab(request: &LaunchRequest, label: &str) -> Result<PaneIdentity> {
    // Existing workspaces and additional tools need fresh terminals; their
    // other panes may contain an editor, agent, or unfinished command.
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
    validate_pane(&pane, &request.workspace_id)?;
    Ok(pane)
}

fn validate_pane(pane: &PaneIdentity, workspace_id: &str) -> Result<()> {
    validate_id(&pane.pane_id)?;
    if pane.workspace_id != workspace_id {
        bail!("Herdr returned a pane outside the requested workspace; no command was submitted");
    }
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
