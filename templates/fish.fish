# worktrunk shell integration for fish

# Only initialize if {{ cmd_prefix }} is available (in PATH or via WORKTRUNK_BIN)
if type -q {{ cmd_prefix }}; or set -q WORKTRUNK_BIN
    # Resolve binary path once at init. WORKTRUNK_BIN can override (for testing dev builds).
    if not set -q WORKTRUNK_BIN
        set -gx WORKTRUNK_BIN (type -p {{ cmd_prefix }})
    end

    # Helper function to run wt and eval the output script
    # stderr streams to terminal for real-time feedback, stdout is captured as shell script
    function wt_exec
        # Capture stdout (shell script), let stderr flow to terminal
        # Use string collect to join lines into a single string (fish splits on newlines by default)
        set -l script (command $WORKTRUNK_BIN $argv 2>&2 | string collect)
        set -l exit_code $pipestatus[1]

        # Eval the script (cd, exec command, etc.) even on failure
        # This ensures cd happens before returning the error code
        if test -n "$script"
            eval $script
            # If script contains a command (--execute), use its exit code
            if test $exit_code -eq 0
                set exit_code $status
            end
        end

        return $exit_code
    end

    # Override {{ cmd_prefix }} command to add --internal flag
    function {{ cmd_prefix }}
        set -l use_source false
        set -l args
        set -l saved_bin $WORKTRUNK_BIN

        # Check for --source flag and strip it
        for arg in $argv
            if test "$arg" = "--source"
                set use_source true
            else
                set -a args $arg
            end
        end

        # If --source was specified, build and use local debug binary
        if test $use_source = true
            if not cargo build --quiet
                return 1
            end
            set -gx WORKTRUNK_BIN ./target/debug/{{ cmd_prefix }}
        end

        # Force colors if stderr is a TTY (directive mode outputs to stderr)
        # Respects NO_COLOR and explicit CLICOLOR_FORCE
        if not set -q NO_COLOR; and not set -q CLICOLOR_FORCE
            if isatty stderr
                set -x CLICOLOR_FORCE 1
            end
        end

        # Always use --internal mode for directive support
        wt_exec --internal $args

        # Restore original command
        set -l result $status
        set -gx WORKTRUNK_BIN $saved_bin
        return $result
    end

    # Completions are in ~/.config/fish/completions/wt.fish (installed by `wt config shell install`)
end
