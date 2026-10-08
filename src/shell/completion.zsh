
_copsy_branches() {
    local -a branches
    branches=(${(f)"$(git branch --format='%(refname:short)' 2>/dev/null)"})
    # Resolve the primary remote the same way copsy does, so the candidates
    # match the branch names copsy accepts.
    local -a remotes
    remotes=(${(f)"$(git remote 2>/dev/null)"})
    local primary=""
    if (( ${remotes[(I)origin]} )); then
        primary=origin
    elif (( ${#remotes} == 1 )); then
        primary=${remotes[1]}
    else
        primary="$(git config --get-regexp '^remote\..*\.gh-resolved$' 2>/dev/null | head -1)"
        primary=${primary%% *}
        primary=${primary#remote.}
        primary=${primary%.gh-resolved}
    fi
    local -a remote_branches
    remote_branches=(${(f)"$(git branch -r --format='%(refname:short)' 2>/dev/null | grep -v HEAD)"})
    if [[ -n $primary ]]; then
        remote_branches=(${remote_branches#${primary}/})
    fi
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
    local subcommand=""
    local word
    local -a args

    args=(
        '--herdr[Keep the caller in place and launch tools in Herdr child tabs]'
        '(-c --claude)'{-c,--claude}'[Launch Claude Code (in a child workspace tab with --herdr)]'
        '(-x --codex)'{-x,--codex}'[Launch Codex (in a child workspace tab with --herdr)]'
        '--code[Open VS Code (from a child workspace tab with --herdr)]'
        '--cursor[Open Cursor (from a child workspace tab with --herdr)]'
        '--open=[Run a custom command (in a child workspace tab with --herdr)]:command:'
        '(--no-carry)--carry[Carry uncommitted changes to the target worktree]'
        '(--carry)--no-carry[Do not carry uncommitted changes (overrides config)]'
        '(--no-setup)--setup[Run repository setup for the target worktree]'
        '(--setup)--no-setup[Do not run repository setup (overrides config)]'
        '(-h --help)'{-h,--help}'[Print help]'
        '(-V --version)'{-V,--version}'[Print version]'
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
                'init:Output shell integration and completion definitions'
                'pr:Checkout a pull request as a worktree'
                'config:Manage copsy configuration'
                'setup:Run repository setup for the current worktree'
            )
            _describe 'subcommand' subcmds && ret=0
            ;;
        args)
            local -a herdr_flags
            herdr_flags=('--herdr[Keep the caller in place and launch tools in Herdr child tabs]')
            local -a launch_flags
            launch_flags=(
                '(-c --claude)'{-c,--claude}'[Launch Claude Code (in a child workspace tab with --herdr)]'
                '(-x --codex)'{-x,--codex}'[Launch Codex (in a child workspace tab with --herdr)]'
                '--code[Open VS Code (from a child workspace tab with --herdr)]'
                '--cursor[Open Cursor (from a child workspace tab with --herdr)]'
                '--open=[Run a custom command (in a child workspace tab with --herdr)]:command:'
            )
            local -a carry_flags
            carry_flags=(
                '(--no-carry)--carry[Carry uncommitted changes to the target worktree]'
                '(--carry)--no-carry[Do not carry uncommitted changes (overrides config)]'
            )
            local -a setup_flags
            setup_flags=(
                '(--no-setup)--setup[Run repository setup for the target worktree]'
                '(--setup)--no-setup[Do not run repository setup (overrides config)]'
            )
            local skip_next=0
            for word in "${words[@]}"; do
                if (( skip_next )); then
                    skip_next=0
                    continue
                fi
                case "$word" in
                    --open)
                        skip_next=1
                        ;;
                    --open=*)
                        ;;
                    new|add|switch|sw|remove|rm|list|ls|status|close|init|pr|config|setup)
                        subcommand="$word"
                        break
                        ;;
                esac
            done
            case "$subcommand" in
                new)
                    _arguments -s -S $herdr_flags $launch_flags $carry_flags $setup_flags '--from=[Base branch to create from (default\: current HEAD)]:branch:_copsy_branches' '1:branch:_copsy_branches' && ret=0
                    ;;
                add)
                    _arguments -s -S $herdr_flags $launch_flags $carry_flags $setup_flags '1:branch:_copsy_branches' && ret=0
                    ;;
                switch|sw)
                    _arguments -s -S $herdr_flags $launch_flags $carry_flags $setup_flags '1:worktree:_copsy_worktrees' && ret=0
                    ;;
                list|ls|status)
                    _arguments -s -S $herdr_flags && ret=0
                    ;;
                close)
                    _arguments -s -S $herdr_flags '--with-branch[Also delete the local branch]' && ret=0
                    ;;
                remove|rm)
                    _arguments -s -S $herdr_flags '--with-branch[Also delete the local branch]' '--all[Remove all worktrees]' '--force[Discard uncommitted changes and delete unmerged branches]' '1:worktree:_copsy_worktrees' && ret=0
                    ;;
                pr)
                    _arguments -s -S $herdr_flags $launch_flags $setup_flags '1:PR number or URL:' && ret=0
                    ;;
                init)
                    _arguments $herdr_flags '1:shell:(zsh bash)' && ret=0
                    ;;
                config)
                    _arguments $herdr_flags '1:config command:(repo global)' && ret=0
                    ;;
                setup)
                    _arguments $herdr_flags && ret=0
                    ;;
            esac
            ;;
    esac

    return ret
}

compdef _copsy copsy
