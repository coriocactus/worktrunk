# worktrunk shell integration for zsh
#
# Completions require zsh's completion system (compinit). If completions don't work:
#   autoload -Uz compinit && compinit  # add before this line in your .zshrc

# Only initialize if {{ cmd_prefix }} is available (in PATH or via WORKTRUNK_BIN)
if command -v {{ cmd_prefix }} >/dev/null 2>&1 || [[ -n "${WORKTRUNK_BIN:-}" ]]; then
    # Resolve binary path once at init. WORKTRUNK_BIN can override (for testing dev builds).
    export WORKTRUNK_BIN="${WORKTRUNK_BIN:-$(command -v {{ cmd_prefix }})}"

{{ posix_shim }}

    # Override {{ cmd_prefix }} command to add --internal flag
    {{ cmd_prefix }}() {
        # Re-resolve if unset (e.g., after shell snapshot restore)
        if [[ -z "$WORKTRUNK_BIN" ]]; then
            export WORKTRUNK_BIN="$(command -v {{ cmd_prefix }})"
        fi

        local use_source=false
        local -a args
        local saved_bin="$WORKTRUNK_BIN"

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
                return 1
            fi
            export WORKTRUNK_BIN="./target/debug/{{ cmd_prefix }}"
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
        export WORKTRUNK_BIN="$saved_bin"
        return $result
    }

    # Lazy completions - generate on first TAB, then delegate to clap's completer
    _{{ cmd_prefix }}_lazy_complete() {
        # Generate completions function once (check if clap's function exists)
        if ! (( $+functions[_clap_dynamic_completer_{{ cmd_prefix }}] )); then
            eval "$(COMPLETE=zsh "$WORKTRUNK_BIN" 2>/dev/null)" || return
        fi
        _clap_dynamic_completer_{{ cmd_prefix }} "$@"
    }

    # Register completion (silently skip if compinit hasn't run yet).
    # We don't warn here because this script runs on every shell startup - users
    # shouldn't see warnings every time they open a terminal. Instead, `wt config
    # shell install` detects missing compinit and shows a one-time advisory.
    if (( $+functions[compdef] )); then
        compdef _{{ cmd_prefix }}_lazy_complete {{ cmd_prefix }}
    fi
fi
