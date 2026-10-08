
_copsy_bash() {
    local cur prev subcmds subcommand word
    local subcommand_index=0
    local index
    local skip_next=0
    cur="${COMP_WORDS[COMP_CWORD]}"
    prev="${COMP_WORDS[COMP_CWORD-1]}"
    subcmds="new add switch sw remove rm list ls status close init pr config setup"

    for ((index=1; index<COMP_CWORD; index++)); do
        word="${COMP_WORDS[index]}"
        if [[ $skip_next -eq 1 ]]; then
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
                subcommand_index=$index
                break
                ;;
        esac
    done

    if [[ -z "$subcommand" ]]; then
        if [[ "${cur}" == -* ]]; then
            COMPREPLY=($(compgen -W "--herdr --carry --no-carry --setup --no-setup -c --claude -x --codex --code --cursor --open -V --version" -- "${cur}"))
        else
            COMPREPLY=($(compgen -W "${subcmds}" -- "${cur}"))
        fi
        return
    fi

    # Global flags remain available even for commands without local flags.
    if [[ "${cur}" == -* ]]; then
        COMPREPLY=($(compgen -W "--herdr" -- "${cur}"))
    fi

    case "$subcommand" in
        new)
            if [[ "${cur}" == -* ]]; then
                COMPREPLY=($(compgen -W "--herdr --carry --no-carry --setup --no-setup --from -c --claude -x --codex --code --cursor --open" -- "${cur}"))
            elif [[ "$prev" != "--open" ]]; then
                local branches
                branches="$(git branch --format='%(refname:short)' 2>/dev/null)"
                COMPREPLY=($(compgen -W "${branches}" -- "${cur}"))
            fi
            ;;
        add)
            if [[ "${cur}" == -* ]]; then
                COMPREPLY=($(compgen -W "--herdr --carry --no-carry --setup --no-setup -c --claude -x --codex --code --cursor --open" -- "${cur}"))
            elif [[ "$prev" != "--open" ]]; then
                local branches
                branches="$(git branch --format='%(refname:short)' 2>/dev/null)"
                COMPREPLY=($(compgen -W "${branches}" -- "${cur}"))
            fi
            ;;
        switch|sw)
            if [[ "${cur}" == -* ]]; then
                COMPREPLY=($(compgen -W "--herdr --carry --no-carry --setup --no-setup -c --claude -x --codex --code --cursor --open" -- "${cur}"))
            elif [[ "$prev" != "--open" ]]; then
                local worktrees
                worktrees="$(git worktree list --porcelain 2>/dev/null | grep '^branch ' | sed 's|^branch refs/heads/||')"
                COMPREPLY=($(compgen -W "${worktrees}" -- "${cur}"))
            fi
            ;;
        close)
            COMPREPLY=($(compgen -W "--herdr --with-branch" -- "${cur}"))
            ;;
        remove|rm)
            if [[ "${cur}" == -* ]]; then
                COMPREPLY=($(compgen -W "--herdr --with-branch --all --force" -- "${cur}"))
            else
                local worktrees
                worktrees="$(git worktree list --porcelain 2>/dev/null | grep '^branch ' | sed 's|^branch refs/heads/||')"
                COMPREPLY=($(compgen -W "${worktrees}" -- "${cur}"))
            fi
            ;;
        init)
            if [[ "${cur}" != -* && ${COMP_CWORD} -eq $((subcommand_index + 1)) ]]; then
                COMPREPLY=($(compgen -W "zsh bash" -- "${cur}"))
            fi
            ;;
        pr)
            if [[ "${cur}" == -* ]]; then
                COMPREPLY=($(compgen -W "--herdr --setup --no-setup -c --claude -x --codex --code --cursor --open" -- "${cur}"))
            fi
            ;;
        config)
            if [[ "${cur}" != -* && ${COMP_CWORD} -eq $((subcommand_index + 1)) ]]; then
                COMPREPLY=($(compgen -W "repo global" -- "${cur}"))
            fi
            ;;
    esac
}

complete -F _copsy_bash copsy
