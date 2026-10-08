copsy() {
    local output
    output="$(command copsy "$@")"
    local exit_code=$?

    # No markers — pass raw output through (preserves --help formatting)
    if [[ "$output" != *{{MARKER_NAMESPACE}}* ]]; then
        [[ -n "$output" ]] && printf '%s\n' "$output"
        return $exit_code
    fi

    local cd_target=""
    local -a launch_cmds=()
    local -a open_cmds=()
    local -a setup_dirs=()
    local -a herdr_launches=()
    while IFS= read -r line; do
        if [[ "$line" == {{CD_MARKER}}* ]]; then
            cd_target="${line#{{CD_MARKER}}}"
        elif [[ "$line" == {{LAUNCH_MARKER}}* ]]; then
            launch_cmds+=("${line#{{LAUNCH_MARKER}}}")
        elif [[ "$line" == {{OPEN_MARKER}}* ]]; then
            open_cmds+=("${line#{{OPEN_MARKER}}}")
        elif [[ "$line" == {{HERDR_LAUNCH_MARKER}}* ]]; then
            herdr_launches+=("${line#{{HERDR_LAUNCH_MARKER}}}")
        elif [[ "$line" == {{SETUP_MARKER}}* ]]; then
            setup_dirs+=("${line#{{SETUP_MARKER}}}")
        else
            printf '%s\n' "$line"
        fi
    done <<< "$output"

    # Run setup outside command substitution so interactive commands retain the TTY.
    if (( ${#setup_dirs[@]} )); then
        for dir in "${setup_dirs[@]}"; do
            (cd "$dir" && command copsy setup --execute) || return $?
        done
    fi

    if [[ -n "$cd_target" ]]; then
        cd "$cd_target" || return 1
    fi

    # Defer Herdr launches until setup succeeds; keep the JSON out of eval.
    if (( ${#herdr_launches[@]} )); then
        for request in "${herdr_launches[@]}"; do
            command copsy herdr-launch "$request" || return $?
        done
    fi

    # LAUNCH: case-dispatched for known tools (no eval for security)
    if (( ${#launch_cmds[@]} )); then
        for entry in "${launch_cmds[@]}"; do
            local tool="${entry%%	*}"
            local dir="${entry#*	}"
            case "$tool" in
                code)   (code -- "$dir") ;;
                cursor) (cursor -- "$dir") ;;
                claude) (cd "$dir" && claude) ;;
                codex)  (cd "$dir" && codex) ;;
            esac
        done
    fi

    # OPEN: eval is intentional — only user-provided --open commands reach here
    if (( ${#open_cmds[@]} )); then
        for cmd in "${open_cmds[@]}"; do
            eval "$cmd"
        done
    fi

    return $exit_code
}
