# Simple shell integration for POSIX shells (bash, zsh).
#
# Captures the shell script from wt's stdout and evals it in the parent shell.
# stderr streams directly to the terminal for real-time user feedback.
#
# This pattern (stderr for logs, stdout for script) is proven by direnv.
# No FIFOs, no background processes, no job control suppression needed.
wt_exec() {
    local script exit_code=0

    # Run wt with stderr attached to terminal (2>&2)
    # Capture stdout (the shell script) into $script
    script="$(command "$WORKTRUNK_BIN" "$@" 2>&2)" || exit_code=$?

    # Eval the script (cd, exec command, etc.) even on failure
    # This ensures cd happens before returning the error code
    if [[ -n "$script" ]]; then
        eval "$script"
        # If script contains a command (--execute), use its exit code
        if [[ $exit_code -eq 0 ]]; then
            exit_code=$?
        fi
    fi

    return "$exit_code"
}
