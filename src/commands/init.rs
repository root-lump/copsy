use anyhow::{Result, bail};

pub fn run(shell: &str) -> Result<()> {
    match shell {
        "zsh" => print!("{}{}", shell_function(), zsh_completion()),
        "bash" => print!("{}{}", shell_function(), bash_completion()),
        _ => bail!("Unsupported shell: {shell}. Supported: zsh, bash"),
    }
    Ok(())
}

// Shell function wrapper that captures stdout markers and dispatches them.
// Without markers (e.g. --help), output is passed through unchanged to avoid garbling.
fn shell_function() -> &'static str {
    r#"copsy() {
    local output
    output="$(command copsy "$@")"
    local exit_code=$?

    # No markers — pass raw output through (preserves --help formatting)
    if [[ "$output" != *__COPSY_* ]]; then
        [[ -n "$output" ]] && printf '%s\n' "$output"
        return $exit_code
    fi

    local cd_target=""
    local -a launch_cmds
    local -a open_cmds
    local -a setup_dirs
    while IFS= read -r line; do
        if [[ "$line" == __COPSY_CD__* ]]; then
            cd_target="${line#__COPSY_CD__}"
        elif [[ "$line" == __COPSY_LAUNCH__* ]]; then
            launch_cmds+=("${line#__COPSY_LAUNCH__}")
        elif [[ "$line" == __COPSY_OPEN__* ]]; then
            open_cmds+=("${line#__COPSY_OPEN__}")
        elif [[ "$line" == __COPSY_SETUP__* ]]; then
            setup_dirs+=("${line#__COPSY_SETUP__}")
        else
            printf '%s\n' "$line"
        fi
    done <<< "$output"

    # Run setup outside command substitution so interactive commands retain the TTY.
    for dir in "${setup_dirs[@]}"; do
        (cd "$dir" && command copsy setup --execute) || return $?
    done

    if [[ -n "$cd_target" ]]; then
        cd "$cd_target" || return 1
    fi

    # LAUNCH: case-dispatched for known tools (no eval for security)
    for entry in "${launch_cmds[@]}"; do
        local tool="${entry%%	*}"
        local dir="${entry#*	}"
        case "$tool" in
            code)   code -- "$dir" ;;
            cursor) cursor -- "$dir" ;;
            claude) claude ;;
            codex)  codex ;;
        esac
    done

    # OPEN: eval is intentional — only user-provided --open commands reach here
    for cmd in "${open_cmds[@]}"; do
        eval "$cmd"
    done

    return $exit_code
}
"#
}

fn zsh_completion() -> &'static str {
    r#"
_copsy_branches() {
    local -a branches
    branches=(${(f)"$(git branch --format='%(refname:short)' 2>/dev/null)"})
    local -a remote_branches
    remote_branches=(${(f)"$(git branch -r --format='%(refname:short)' 2>/dev/null | sed 's|^origin/||' | grep -v HEAD)"})
    _describe 'branch' branches
    _describe 'remote branch' remote_branches
}

_copsy_worktrees() {
    local -a worktrees
    worktrees=(${(f)"$(git worktree list --porcelain 2>/dev/null | grep '^branch ' | sed 's|^branch refs/heads/||')"})
    _describe 'worktree' worktrees
}

_copsy() {
    local ret=1
    local -a args

    args=(
        '(-c --claude)'{-c,--claude}'[Launch claude]'
        '(-x --codex)'{-x,--codex}'[Launch codex]'
        '--code[Open in VS Code]'
        '--cursor[Open in Cursor]'
        '--open=[Run custom command]:command:'
        '(--no-carry)--carry[Carry uncommitted changes]'
        '(--carry)--no-carry[Do not carry uncommitted changes]'
        '(--no-setup)--setup[Run repository setup]'
        '(--setup)--no-setup[Do not run repository setup]'
        '(-h --help)'{-h,--help}'[Print help]'
        '1:subcommand:->subcmd'
        '*::arg:->args'
    )

    _arguments -s -S $args && ret=0

    case "$state" in
        subcmd)
            local -a subcmds
            subcmds=(
                'new:Create a worktree with a new branch'
                'add:Create a worktree for an existing branch'
                'switch:Switch to a worktree'
                'sw:Switch to a worktree'
                'remove:Remove a worktree'
                'rm:Remove a worktree'
                'list:List all worktrees'
                'ls:List all worktrees'
                'status:Show git status for all worktrees'
                'close:Close current worktree and return to main'
                'init:Output shell integration function'
                'pr:Checkout a pull request as a worktree'
                'config:Manage repository configuration'
                'setup:Run repository setup for the current worktree'
            )
            _describe 'subcommand' subcmds && ret=0
            ;;
        args)
            local -a launch_flags
            launch_flags=(
                '(-c --claude)'{-c,--claude}'[Launch claude]'
                '(-x --codex)'{-x,--codex}'[Launch codex]'
                '--code[Open in VS Code]'
                '--cursor[Open in Cursor]'
                '--open=[Run custom command]:command:'
            )
            local -a carry_flags
            carry_flags=(
                '(--no-carry)--carry[Carry uncommitted changes]'
                '(--carry)--no-carry[Do not carry uncommitted changes]'
            )
            local -a setup_flags
            setup_flags=(
                '(--no-setup)--setup[Run repository setup]'
                '(--setup)--no-setup[Do not run repository setup]'
            )
            case "${words[1]}" in
                new)
                    _arguments -s -S $launch_flags $carry_flags $setup_flags '--from=[Base branch]:branch:_copsy_branches' '1:branch:_copsy_branches' && ret=0
                    ;;
                add)
                    _arguments -s -S $launch_flags $carry_flags $setup_flags '1:branch:_copsy_branches' && ret=0
                    ;;
                switch|sw)
                    _arguments -s -S $launch_flags $carry_flags $setup_flags '1:worktree:_copsy_worktrees' && ret=0
                    ;;
                close)
                    _arguments -s -S '--with-branch[Also delete the local branch]' && ret=0
                    ;;
                remove|rm)
                    _arguments -s -S '--with-branch[Also delete the local branch]' '--all[Remove all worktrees]' '1:worktree:_copsy_worktrees' && ret=0
                    ;;
                pr)
                    _arguments -s -S $launch_flags $setup_flags '1:PR number or URL:' && ret=0
                    ;;
                init)
                    _arguments '1:shell:(zsh bash)' && ret=0
                    ;;
                config)
                    _arguments '1:config command:(init)' && ret=0
                    ;;
                setup)
                    _arguments && ret=0
                    ;;
            esac
            ;;
    esac

    return ret
}

compdef _copsy copsy
"#
}

fn bash_completion() -> &'static str {
    r#"
_copsy_bash() {
    local cur prev subcmds
    cur="${COMP_WORDS[COMP_CWORD]}"
    prev="${COMP_WORDS[COMP_CWORD-1]}"
    subcmds="new add switch sw remove rm list ls status close init pr config setup"

    if [[ ${COMP_CWORD} -eq 1 ]]; then
        if [[ "${cur}" == -* ]]; then
            COMPREPLY=($(compgen -W "--carry --no-carry --setup --no-setup -c --claude -x --codex --code --cursor --open" -- "${cur}"))
        else
            COMPREPLY=($(compgen -W "${subcmds}" -- "${cur}"))
        fi
        return
    fi

    case "${COMP_WORDS[1]}" in
        new|add)
            if [[ "${cur}" == -* ]]; then
                COMPREPLY=($(compgen -W "--carry --no-carry --setup --no-setup --from -c --claude -x --codex --code --cursor --open" -- "${cur}"))
            elif [[ ${COMP_CWORD} -eq 2 ]]; then
                local branches
                branches="$(git branch --format='%(refname:short)' 2>/dev/null)"
                COMPREPLY=($(compgen -W "${branches}" -- "${cur}"))
            fi
            ;;
        switch|sw)
            if [[ "${cur}" == -* ]]; then
                COMPREPLY=($(compgen -W "--carry --no-carry --setup --no-setup -c --claude -x --codex --code --cursor --open" -- "${cur}"))
            elif [[ ${COMP_CWORD} -eq 2 ]]; then
                local worktrees
                worktrees="$(git worktree list --porcelain 2>/dev/null | grep '^branch ' | sed 's|^branch refs/heads/||')"
                COMPREPLY=($(compgen -W "${worktrees}" -- "${cur}"))
            fi
            ;;
        close)
            COMPREPLY=($(compgen -W "--with-branch" -- "${cur}"))
            ;;
        remove|rm)
            if [[ "${cur}" == -* ]]; then
                COMPREPLY=($(compgen -W "--with-branch --all" -- "${cur}"))
            else
                local worktrees
                worktrees="$(git worktree list --porcelain 2>/dev/null | grep '^branch ' | sed 's|^branch refs/heads/||')"
                COMPREPLY=($(compgen -W "${worktrees}" -- "${cur}"))
            fi
            ;;
        init)
            if [[ ${COMP_CWORD} -eq 2 ]]; then
                COMPREPLY=($(compgen -W "zsh bash" -- "${cur}"))
            fi
            ;;
        pr)
            if [[ "${cur}" == -* ]]; then
                COMPREPLY=($(compgen -W "--setup --no-setup -c --claude -x --codex --code --cursor --open" -- "${cur}"))
            fi
            ;;
        config)
            if [[ ${COMP_CWORD} -eq 2 ]]; then
                COMPREPLY=($(compgen -W "init" -- "${cur}"))
            fi
            ;;
    esac
}

complete -F _copsy_bash copsy
"#
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_runs_setup_before_cd_and_launch() {
        let shell = shell_function();
        let setup = shell.find("for dir in \"${setup_dirs[@]}\"").unwrap();
        let cd = shell.find("if [[ -n \"$cd_target\" ]]").unwrap();
        let launch = shell.find("for entry in \"${launch_cmds[@]}\"").unwrap();
        assert!(setup < cd);
        assert!(cd < launch);
    }

    #[test]
    fn completions_include_setup_commands_and_flags() {
        for completion in [zsh_completion(), bash_completion()] {
            assert!(completion.contains("config"));
            assert!(completion.contains("setup"));
            assert!(completion.contains("--no-setup"));
        }
    }
}
