---
name: branch-naming
description: Invoke BEFORE creating any new git branch in this repository (git checkout -b / git switch -c, including branches created by other skills or subagents). Defines the copsy branch naming convention.
allowed-tools:
  - Bash(git log *)
  - Bash(git branch *)
---

# Branch naming (copsy)

Apply this before the branch is created, whether the creation happens directly
or through another skill or subagent.

## Branch name

| Format | Examples |
|--------|----------|
| `<type>/<short-description>` | `feat/add-stash-command`, `fix/shell-function-color-output`, `docs/update-readme` |

- `<type>` matches the commit-prefix vocabulary used in this repository:
  `feat` / `fix` / `refactor` / `chore` / `docs` / `test` / `perf` / `ci` /
  `style` / `build`
- `<short-description>` is concise English kebab-case
- Derive the description from the user's request or the upcoming change; if it
  is ambiguous, confirm it together with any other pending question in a
  single `AskUserQuestion` call

## Base branch

- All branches are based on `main`

## Language policy

Branch names are English. Questions and status reports to the user in the
shell follow the user's language.
