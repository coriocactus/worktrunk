#!/usr/bin/env bash
# Check that commands using --internal mode don't bypass the output system
#
# Commands that support --internal (switch, remove, merge) must use the output
# system (crate::output::*) rather than direct println!/eprintln! calls.
# This prevents directive leaks in shell wrapper integration.

set -euo pipefail

# Files that must use output system (support --internal mode)
RESTRICTED_FILES=(
    "src/commands/worktree.rs"
    "src/commands/merge.rs"
)

# Allowed exceptions (test code, etc.)
ALLOWED_PATTERNS=(
    "spacing_test\.rs"
    "command_approval\.rs"
)

exit_code=0

for file in "${RESTRICTED_FILES[@]}"; do
    if [[ ! -f "$file" ]]; then
        continue
    fi

    # Check for direct print!/println!/eprint!/eprintln! usage
    while IFS= read -r line; do
        # Skip if it matches any allowed pattern
        skip=false
        for pattern in "${ALLOWED_PATTERNS[@]}"; do
            if echo "$line" | grep -q "$pattern"; then
                skip=true
                break
            fi
        done

        if [[ "$skip" == "true" ]]; then
            continue
        fi

        echo "❌ Direct output in $line"
        echo "   Commands using --internal must use crate::output::* functions"
        echo "   Replace print!/println! with output::progress() or output::success()"
        exit_code=1
    done < <(grep -n "print!\|println!\|eprint!\|eprintln!" "$file" | grep -v "//.*print" || true)
done

if [[ $exit_code -eq 0 ]]; then
    echo "✅ Output system check passed"
fi

exit $exit_code
