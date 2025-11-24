# worktrunk shell integration for zsh

# Only initialize if {{ cmd_prefix }} is available (in PATH or via WORKTRUNK_BIN)
if command -v {{ cmd_prefix }} >/dev/null 2>&1 || [[ -n "${WORKTRUNK_BIN:-}" ]]; then
    # Use WORKTRUNK_BIN if set, otherwise resolve binary path
    # Must resolve BEFORE defining shell function, so lazy completion can call binary directly
    # This allows testing development builds: export WORKTRUNK_BIN=./target/debug/{{ cmd_prefix }}
    _WORKTRUNK_CMD="${WORKTRUNK_BIN:-$(command -v {{ cmd_prefix }})}"

{{ posix_shim }}

    # Override {{ cmd_prefix }} command to add --internal flag
    {{ cmd_prefix }}() {
        # Initialize _WORKTRUNK_CMD if not set (e.g., after shell snapshot restore)
        if [[ -z "$_WORKTRUNK_CMD" ]]; then
            _WORKTRUNK_CMD="${WORKTRUNK_BIN:-$(command -v {{ cmd_prefix }})}"
        fi

        local use_source=false
        local -a args
        local saved_cmd="$_WORKTRUNK_CMD"

        # Check for --source flag and strip it
        for arg in "$@"; do
            if [[ "$arg" == "--source" ]]; then
                use_source=true
            else
                args+=("$arg")
            fi
        done

        # If --source was specified, build and use local debug binary
        if [[ "$use_source" == true ]]; then
            if ! cargo build --quiet; then
                _WORKTRUNK_CMD="$saved_cmd"
                return 1
            fi
            _WORKTRUNK_CMD="./target/debug/{{ cmd_prefix }}"
        fi

        # Force colors if stderr is a TTY (directive mode outputs to stderr)
        # Respects NO_COLOR and explicit CLICOLOR_FORCE
        if [[ -z "${NO_COLOR:-}" && -z "${CLICOLOR_FORCE:-}" ]]; then
            if [[ -t 2 ]]; then export CLICOLOR_FORCE=1; fi
        fi

        # Always use --internal mode for directive support
        wt_exec --internal "${args[@]}"

        # Restore original command
        local result=$?
        _WORKTRUNK_CMD="$saved_cmd"
        return $result
    }

    # Lazy completion loader - loads real completions on first tab-press
    # This avoids ~11ms binary invocation at shell startup
    _wt_lazy_complete() {
        # Only try to install completions once
        if [[ -z "${_WT_COMPLETION_LOADED:-}" ]]; then
            typeset -g _WT_COMPLETION_LOADED=1
            local completion_script
            if completion_script=$(COMPLETE=zsh "${_WORKTRUNK_CMD:-{{ cmd_prefix }}}" 2>/dev/null); then
                eval "$completion_script"
            else
                # Failed to load - unregister to prevent future silent failures
                compdef -d {{ cmd_prefix }} 2>/dev/null
                return 1
            fi
        fi

        # Delegate to real completion function if it was installed
        if (( $+functions[_clap_dynamic_completer_{{ cmd_prefix }}] )); then
            _clap_dynamic_completer_{{ cmd_prefix }}
        fi
    }

    # Register completion - either now or deferred until compinit runs
    _wt_register_completion() {
        if (( $+functions[compdef] )); then
            compdef _wt_lazy_complete {{ cmd_prefix }}
            # Remove hook once registered
            precmd_functions=(${precmd_functions:#_wt_register_completion})
            unfunction _wt_register_completion 2>/dev/null
            return 0
        fi
        return 1
    }

    # Try immediate registration, otherwise defer via precmd hook
    if ! _wt_register_completion; then
        # Add to hook only if not already present (handles re-sourcing .zshrc)
        (( ${precmd_functions[(I)_wt_register_completion]} )) || precmd_functions+=(_wt_register_completion)
    fi
fi
