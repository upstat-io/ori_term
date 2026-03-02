# oriterm zsh integration bootstrap.
# This file is used as ZDOTDIR/.zshenv to inject oriterm hooks.

# Restore original ZDOTDIR so user config files load normally.
if [[ -n "${ORITERM_ZSH_ZDOTDIR+x}" ]]; then
    ZDOTDIR="$ORITERM_ZSH_ZDOTDIR"
    unset ORITERM_ZSH_ZDOTDIR
else
    unset ZDOTDIR
fi

# Source the real user .zshenv if it exists.
if [[ -n "$ZDOTDIR" && -f "$ZDOTDIR/.zshenv" ]]; then
    source "$ZDOTDIR/.zshenv"
elif [[ -f "$HOME/.zshenv" ]]; then
    source "$HOME/.zshenv"
fi

# Source oriterm integration (precmd/preexec hooks).
_oriterm_integration="${0:A:h}/oriterm-integration"
if [[ -f "$_oriterm_integration" ]]; then
    source "$_oriterm_integration"
fi
unset _oriterm_integration
