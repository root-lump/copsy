# copsy

Git worktree management CLI written in Rust.

## Language policy

All output to this repository (code, comments, commit messages, PR descriptions, CLAUDE.md) must be in English regardless of the user's language settings. README files are excluded as they have language-specific versions. Responses to the user should follow the user's configured language.

## Build & Run

```sh
cargo build
cargo run -- <args>
```

## Lint & Test

```sh
cargo fmt --check
cargo clippy -- -D warnings
cargo test
```

## Architecture

- `src/main.rs` — Entry point, dispatches to command handlers
- `src/cli.rs` — clap derive definitions
- `src/git.rs` — Git/gh command wrappers
- `src/config.rs` — Config file loading (`~/.config/copsy/config.toml`)
- `src/output.rs` — Marker protocol (`__COPSY_CD__`, `__COPSY_LAUNCH__`, `__COPSY_OPEN__`) for shell function communication
- `src/launcher.rs` — Emits launch markers for editors/AI tools
- `src/commands/` — One file per subcommand

## Key design decisions

- All user-facing messages go to stderr via `info!` macro; stdout is reserved for markers consumed by the shell function
- `colored::control::set_override(true)` forces color output even when stdout is piped by the shell function
- Shell integration uses `copsy init zsh` to output a shell function + zsh completion; the function captures stdout and dispatches markers
- `__COPSY_LAUNCH__` uses case-based dispatch (no eval) for security; `__COPSY_OPEN__` uses eval for user-provided commands only

## Conventions

- Code comments explain **why**, not what. Add a comment only when the reason would surprise a reader: a hidden constraint, a workaround for a specific tool/library behavior, or an invariant the code alone doesn't convey. Do not reference tasks, PRs, or callers — those belong in commit messages
- When adding or changing CLI options, always update the `--help` description text in the clap `#[arg]` or `#[command]` attributes
- When adding or changing CLI options, always update the zsh/bash completion definitions in `src/commands/init.rs` — including top-level flags and per-subcommand flags as appropriate
- Only markers (`__COPSY_*`) may be written to stdout. All user-facing messages must use the `info!` macro (stderr)
- Use `use` declarations for external module references instead of inline `crate::foo`
- Interactive dialogs (`FuzzySelect`, etc.) must use `interact_opt()` so Esc cancels the dialog
- When adding or changing CLI options, always update the zsh/bash completion definitions in `src/commands/init.rs`
- `add::run` is called from multiple places (`main.rs`, `interactive.rs`, `pr.rs`). Update all call sites when changing its signature
- `colored::control::set_override(true)` must be called before clap parsing to force colors through the shell function's pipe
