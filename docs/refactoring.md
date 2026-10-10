# Integration boundaries

Git operations, Herdr integration, and generated shell code have different
failure modes. Keeping their implementations together made routine changes
require reading unrelated path, protocol, and process handling. This refactor
separates those responsibilities while retaining the command-facing interfaces.

## Ownership

- `src/git/mod.rs` owns subprocess execution and re-exports the operations used
  by command handlers. `repository.rs` resolves repository identity, shared
  configuration paths, and ignored setup paths; `remote.rs` owns remote
  precedence and branch discovery; `worktree.rs` owns discovery, layout, and
  removal; `carry.rs` owns stash transport; `pr.rs` owns GitHub CLI discovery
  and fetching. Unit tests live beside the policy they exercise.
- Main-worktree lookup and worktree listing share one porcelain parser, so
  detached and bare entries follow the same parsing rules. Git's first entry
  remains the main repository. PR discovery returns named fields instead of
  positional tuples without changing the `gh` query or incomplete-row handling.
- `src/herdr/paths.rs` owns Herdr-compatible configuration and path resolution.
  `session.rs` owns workspace registration, deferred launch requests, pane
  ownership/readiness checks, and command submission. These remain separate
  from ordinary local launch markers.
- `src/shell/` contains the wrapper template and both completion scripts.
  `commands/init.rs` embeds them at compile time and substitutes the marker
  constants. The executable still ships without runtime script dependencies.
- `tests/support/` provides isolated repositories for both ordinary CLI and
  Herdr integration tests. Worktree removal from the caller's current directory
  is tested in a subprocess instead of changing the test runner's process-wide
  current directory.

## Behavior to preserve

CLI flags, aliases, help text, configuration formats, layout naming, remote
precedence, and Git/gh arguments are unchanged. No dependencies are changed.
Normal commands keep user messages on stderr and stdout available for markers;
`init` still prints the generated shell code and help retains its existing
output behavior.

Carry runs before target preparation, and a failed preparation leaves its
uniquely tagged stash available for recovery. Setup markers precede directory
and launch markers. The shell completes setup before starting tools. Herdr
requests remain JSON passed to the hidden dispatcher without `eval`; only the
user's `--open` command is treated as shell source. Herdr registration failures
preserve the checkout, and pane readiness, freshness, and ownership checks
remain in place.

Worktree deletion still runs Git from the main checkout. Dirty-worktree guards,
branch deletion policy, partial `remove --all` reporting and relocation, and
pruning only the empty nested repository directory retain their existing order.

## Verification

The pre-refactor baseline at `31b2baa1cf7246ad0b8f8db5c30026771674ac83`
passed formatting, Clippy, and 91 unit plus 35 integration tests on Linux with
Rust 1.97.1, Bash, and Zsh.

Run the repository's checks after changes:

```sh
cargo fmt --check
cargo clippy -- -D warnings
cargo test
```

The regression suite adds porcelain and PR parsing cases, exact marker ordering
for creation/reuse/switch, carry success and recovery, deletion from the caller's
checkout, dirty-worktree refusal, partial bulk removal, nested cleanup, and
ordinary Bash/Zsh setup and launches with literal paths. Existing Herdr tests
exercise registration failure, slug collisions, shell transitions, launch
isolation, initial-pane reuse, readiness delays, and fallback to fresh tabs.

For extraction-only changes, also compare `init bash`, `init zsh`, version,
help, and invalid-argument output with the baseline executable. This catches
byte-level shell or CLI drift that semantic assertions may miss. Herdr tests
use a mock session API; they do not replace a live Herdr/macOS smoke test.
