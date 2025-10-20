# worktrunk shell integration for {{ shell_name }}

# Helper function to parse wt output and handle directives
_wt_exec() {
    local output line exit_code
    output="$(command wt "$@" 2>&1)"
    exit_code=$?

    # Parse output line by line
    while IFS= read -r line; do
        if [[ "$line" == __WORKTRUNK_CD__* ]]; then
            # Extract path and change directory
            \cd "${line#__WORKTRUNK_CD__}"
        else
            # Regular output - print it
            echo "$line"
        fi
    done <<< "$output"

    return $exit_code
}

# Override {{ cmd_prefix }} command to add --internal flag for switch and finish
{{ cmd_prefix }}() {
    local subcommand="$1"

    case "$subcommand" in
        switch|finish)
            # Commands that need --internal for directory change support
            shift
            _wt_exec "$subcommand" --internal "$@"
            ;;
        *)
            # All other commands pass through directly
            command wt "$@"
            ;;
    esac
}

{% if hook.to_string() == "prompt" %}
# Prompt hook for tracking current worktree
_wt_prompt_hook() {
    # Call wt to update tracking
    command wt hook prompt 2>/dev/null || true
}

# Add to PROMPT_COMMAND
if [[ -z "${PROMPT_COMMAND}" ]]; then
    PROMPT_COMMAND="_wt_prompt_hook"
else
    PROMPT_COMMAND="${PROMPT_COMMAND}; _wt_prompt_hook"
fi
{% endif %}

# Dynamic completion function
_{{ cmd_prefix }}_complete() {
    local cur="${COMP_WORDS[COMP_CWORD]}"

    # Call wt complete with current command line
    local completions=$(command wt complete "${COMP_WORDS[@]}" 2>/dev/null)
    COMPREPLY=($(compgen -W "$completions" -- "$cur"))
}

# Register dynamic completion
complete -F _{{ cmd_prefix }}_complete {{ cmd_prefix }}
