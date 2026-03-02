#!/bin/bash
# oriterm shell integration for bash
# Emits OSC 133 prompt markers and OSC 7 CWD reporting.

# Guard: only run in interactive shells, only run once.
[[ "$-" != *i* ]] && return
[[ -n "$ORITERM_BASH_INTEGRATION_LOADED" ]] && return
ORITERM_BASH_INTEGRATION_LOADED=1

# When loaded via --rcfile (e.g. through WSL), source the user's normal
# startup files first so their PS1, aliases, etc. are available.
if [[ -n "${ORITERM_BASH_RCFILE:-}" ]]; then
    unset ORITERM_BASH_RCFILE
    if [[ -f /etc/profile ]]; then
        source /etc/profile
    fi
    if [[ -f "$HOME/.bashrc" ]]; then
        source "$HOME/.bashrc"
    fi
fi

# Source bash-preexec if not already loaded.
if [[ -z "${bash_preexec_imported:-}" ]]; then
    _oriterm_script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
    if [[ -f "${_oriterm_script_dir}/bash-preexec.sh" ]]; then
        # shellcheck source=bash-preexec.sh
        source "${_oriterm_script_dir}/bash-preexec.sh"
    fi
    unset _oriterm_script_dir
fi

# Restore the user's original HISTFILE if we overrode it during injection.
if [[ -n "${ORITERM_BASH_ORIG_HISTFILE+x}" ]]; then
    HISTFILE="$ORITERM_BASH_ORIG_HISTFILE"
    unset ORITERM_BASH_ORIG_HISTFILE
fi

# --- OSC 7: Report CWD ---
__oriterm_osc7() {
    local hostname
    hostname=$(hostname)
    printf '\e]7;file://%s%s\a' "$hostname" "$PWD"
}

# --- OSC 133: Prompt markers ---
__oriterm_precmd() {
    local exit_status="$?"
    # D — end of previous command output (with exit code)
    printf '\e]133;D;%s\a' "$exit_status"
    # Report CWD
    __oriterm_osc7
    # A — prompt start
    printf '\e]133;A\a'
}

__oriterm_preexec() {
    # C — output start (command is about to run)
    printf '\e]133;C\a'
}

# Register hooks via bash-preexec.
precmd_functions+=(__oriterm_precmd)
preexec_functions+=(__oriterm_preexec)

# Emit B marker when user presses Enter — via PROMPT_COMMAND.
# The B marker goes between the end of the prompt and the start of user input.
# We inject it via PS1 so it appears after the prompt renders.
if [[ -z "$ORITERM_NO_PS1_SUFFIX" ]]; then
    PS1="${PS1:-\\$ }"'\[\e]133;B\a\]'
fi
