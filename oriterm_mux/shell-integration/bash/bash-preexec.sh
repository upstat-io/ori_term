#!/bin/bash
# Minimal bash-preexec implementation for oriterm shell integration.
#
# Based on the bash-preexec project (https://github.com/rcaloras/bash-preexec)
# Licensed under the MIT License.
#
# Provides `precmd_functions` and `preexec_functions` arrays:
#   - Functions in `precmd_functions` run before each prompt.
#   - Functions in `preexec_functions` run before each command executes.

[[ -n "${bash_preexec_imported:-}" ]] && return
bash_preexec_imported="defined"

declare -a precmd_functions
declare -a preexec_functions

__bp_last_ret_value="$?"
__bp_preexec_ran=0

__bp_precmd() {
    __bp_last_ret_value="$?"

    # Reset preexec flag so next command triggers preexec again.
    __bp_preexec_ran=0

    local fn
    for fn in "${precmd_functions[@]}"; do
        "$fn"
    done

    return $__bp_last_ret_value
}

# DEBUG trap handler: runs before each command.
__bp_preexec_invoke() {
    # Don't run for PROMPT_COMMAND itself or if already ran for this command.
    [[ "$__bp_preexec_ran" == 1 ]] && return
    # Ignore if COMP_LINE is set (tab completion).
    [[ -n "${COMP_LINE:-}" ]] && return
    # Avoid running during PROMPT_COMMAND evaluation.
    local this_command
    this_command="$(HISTTIMEFORMAT= builtin history 1)"
    # Strip leading history number.
    this_command="${this_command##*([[:space:]])+([[:digit:]])+([[:space:]])}"
    [[ -z "$this_command" ]] && return

    __bp_preexec_ran=1

    local fn
    for fn in "${preexec_functions[@]}"; do
        "$fn" "$this_command"
    done
}

# Install hooks.
if [[ -z "${PROMPT_COMMAND:-}" ]]; then
    PROMPT_COMMAND="__bp_precmd"
elif [[ "$PROMPT_COMMAND" != *"__bp_precmd"* ]]; then
    PROMPT_COMMAND="__bp_precmd;${PROMPT_COMMAND}"
fi

trap '__bp_preexec_invoke' DEBUG
