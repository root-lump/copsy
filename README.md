<p align="center">
  <img src="assets/logo.png" alt="copsy" width="40%">
</p>

<h1 align="center">copsy</h1>

<p align="center">
  A git worktree management CLI that makes it easy to create, switch, and manage worktrees with interactive branch selection, PR checkout, and editor/AI tool integration.
</p>

[日本語版 README](README.ja.md)

## Features

- **Interactive branch selection** with fuzzy search — local branches, remote branches, and existing worktrees
- **PR checkout** — create a worktree directly from a GitHub PR number or URL
- **Shell integration** — actually `cd` into worktrees (not just print the path)
- **Editor/AI launch** — open VS Code, Cursor, Claude Code, or Codex CLI after switching
- **Tab completion** — zsh/bash completion for subcommands, branch names, and worktree names
- **Color-coded output** — distinguish repos, worktrees, local branches, and remote branches at a glance
- **Configurable worktree directory** — choose where worktrees are created

## Requirements

- Rust (edition 2024)
- Git
- [gh](https://cli.github.com/) (GitHub CLI) — required for `copsy pr`

## Install

```sh
cargo install --path .
```

### Shell integration

Add to your `~/.zshrc`:

```sh
eval "$(copsy init zsh)"
```

Or for bash, add to your `~/.bashrc`:

```sh
eval "$(copsy init bash)"
```

This sets up a shell function that enables `cd` into worktrees and tab completion.

## Usage

### Interactive mode

```sh
copsy
```

Run without arguments to get a fuzzy-searchable list of all branches and worktrees. Select one to:
- **Existing worktree** — switch to it
- **Local/remote branch** — create a new worktree and switch to it

Press **Esc** to cancel.

### Commands

| Command | Description |
|---|---|
| `copsy new <branch>` | Create a worktree with a new branch |
| `copsy add <branch>` | Create a worktree for an existing branch |
| `copsy switch` (`sw`) | Fuzzy-select an existing worktree to switch to |
| `copsy remove` (`rm`) | Fuzzy-select a worktree to remove |
| `copsy list` (`ls`) | List all worktrees |
| `copsy status` | Show `git status --short` for every worktree |
| `copsy close` | Close the current worktree and return to the main worktree |
| `copsy pr [target]` | Checkout a PR as a worktree (interactive if target omitted) |
| `copsy init <shell>` | Print shell integration script (`zsh` or `bash`) |

### PR checkout

```sh
copsy pr 123                                       # by PR number
copsy pr https://github.com/owner/repo/pull/123    # by URL
copsy pr                                           # interactive selection
```

Fetches the PR branch and creates (or switches to) a worktree for it.

### Launch flags

Open editors or AI tools after switching. Flags can be combined:

```sh
copsy --claude              # Launch Claude Code
copsy --codex               # Launch Codex CLI
copsy --code                # Open VS Code
copsy --cursor              # Open Cursor
copsy --open "my-command"   # Run a custom command

copsy add feature --claude --code   # Create worktree + open VS Code + launch Claude
copsy pr 42 --cursor                # Checkout PR #42 + open Cursor
```

| Flag | Short | Tool |
|---|---|---|
| `--claude` | `-c` | Claude Code |
| `--codex` | `-x` | Codex CLI |
| `--code` | | VS Code |
| `--cursor` | | Cursor |
| `--open <cmd>` | | Custom command |

## Configuration

Config file: `~/.config/copsy/config.toml` (respects `$XDG_CONFIG_HOME`)

```toml
[worktree]
# Directory where worktrees are created (default: parent of repo root)
# Supports ~ expansion
base_dir = "~/worktrees"
```

Without `base_dir`, worktrees are created alongside the repository directory, named `<repo>-<branch>`.

## Worktree naming

Worktrees are named `<repo>-<branch>`, with `/` in branch names replaced by `-`.

Examples:
- Repository `myapp`, branch `feature/login` → `myapp-feature-login`
- Repository `myapp`, branch `fix-typo` → `myapp-fix-typo`

## License

MIT
