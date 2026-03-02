# oriterm shell integration for fish.
# Emits OSC 133 prompt markers and OSC 7 CWD reporting.

# Guard: only run in interactive shells, only run once.
status is-interactive; or exit
set -q ORITERM_FISH_INTEGRATION_LOADED; and exit
set -g ORITERM_FISH_INTEGRATION_LOADED 1

# --- OSC 7: Report CWD ---
function __oriterm_osc7
    printf '\e]7;file://%s%s\a' (hostname) "$PWD"
end

# --- OSC 133: Prompt markers ---
# fish_prompt event fires before the prompt is displayed.
function __oriterm_prompt --on-event fish_prompt
    # D marker for previous command (fish provides $status automatically)
    printf '\e]133;D;%s\a' "$__oriterm_last_status"
    # Report CWD
    __oriterm_osc7
    # A — prompt start
    printf '\e]133;A\a'
end

# fish_preexec event fires before each command executes.
function __oriterm_preexec --on-event fish_preexec
    # B — end of prompt / start of input (emitted just before command)
    printf '\e]133;B\a'
    # C — output start
    printf '\e]133;C\a'
end

# Capture exit status before prompt renders.
function __oriterm_postexec --on-event fish_postexec
    set -g __oriterm_last_status $status
end

# Initialize last status.
set -g __oriterm_last_status 0
