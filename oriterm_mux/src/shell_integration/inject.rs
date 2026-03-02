//! Per-shell injection mechanisms.
//!
//! Each shell requires a different strategy to source our integration scripts
//! on startup. This module configures the `CommandBuilder` environment so the
//! target shell finds and loads the scripts automatically.

use std::path::Path;

use portable_pty::CommandBuilder;

use super::{Shell, set_common_env};

/// Configure the command environment for shell integration injection.
///
/// Sets env vars on `cmd` so the target shell will source our integration
/// scripts from `integration_dir`. The `cwd` parameter provides the inherited
/// working directory for WSL's `--cd` argument.
///
/// Returns an optional extra argument to append to the command (e.g.
/// `--posix` for bash).
pub(crate) fn setup_injection(
    cmd: &mut CommandBuilder,
    shell: Shell,
    integration_dir: &Path,
    cwd: Option<&str>,
) -> Option<&'static str> {
    set_common_env(cmd);

    match shell {
        Shell::Bash => {
            inject_bash(cmd, integration_dir);
            Some("--posix")
        }
        Shell::Zsh => {
            inject_zsh(cmd, integration_dir);
            None
        }
        Shell::Fish => {
            inject_fish(cmd, integration_dir);
            None
        }
        Shell::PowerShell => {
            inject_powershell(cmd, integration_dir);
            None
        }
        Shell::Wsl => {
            inject_wsl(cmd, cwd);
            None
        }
    }
}

/// Bash: `--posix` mode sources `$ENV` on startup.
fn inject_bash(cmd: &mut CommandBuilder, dir: &Path) {
    let script = dir.join("bash").join("oriterm.bash");
    cmd.env("ENV", script.to_string_lossy().as_ref());
    cmd.env("ORITERM_BASH_INJECT", "1");

    // Preserve the user's HISTFILE since --posix mode may change it.
    if let Ok(histfile) = std::env::var("HISTFILE") {
        cmd.env("ORITERM_BASH_ORIG_HISTFILE", histfile);
    }

    log::info!("shell_integration: bash injection configured");
}

/// Zsh: redirect `ZDOTDIR` so zsh sources our `.zshenv` first.
fn inject_zsh(cmd: &mut CommandBuilder, dir: &Path) {
    let zsh_dir = dir.join("zsh");

    // Save the original ZDOTDIR so our .zshenv can restore it.
    // Falls back to HOME if ZDOTDIR is unset (zsh default behavior).
    let orig_zdotdir = std::env::var("ZDOTDIR")
        .or_else(|_| std::env::var("HOME"))
        .ok();
    if let Some(dir) = orig_zdotdir {
        cmd.env("ORITERM_ZSH_ZDOTDIR", dir);
    }

    cmd.env("ZDOTDIR", zsh_dir.to_string_lossy().as_ref());

    log::info!("shell_integration: zsh injection configured");
}

/// Fish: prepend our dir to `XDG_DATA_DIRS` so Fish finds `vendor_conf.d/`.
fn inject_fish(cmd: &mut CommandBuilder, dir: &Path) {
    let fish_dir = dir.join("fish");
    let existing = std::env::var("XDG_DATA_DIRS").unwrap_or_default();
    let new_val = if existing.is_empty() {
        fish_dir.to_string_lossy().into_owned()
    } else {
        format!("{}:{existing}", fish_dir.to_string_lossy())
    };
    cmd.env("XDG_DATA_DIRS", &new_val);

    log::info!("shell_integration: fish injection configured");
}

/// `PowerShell`: set env var that the user's `$PROFILE` can check.
fn inject_powershell(cmd: &mut CommandBuilder, dir: &Path) {
    let script = dir.join("powershell").join("oriterm.ps1");
    cmd.env("ORITERM_PS_PROFILE", script.to_string_lossy().as_ref());

    log::info!("shell_integration: powershell injection configured");
}

/// WSL: propagate env vars via `WSLENV`; users manually source scripts.
fn inject_wsl(cmd: &mut CommandBuilder, cwd: Option<&str>) {
    cmd.arg("--cd");
    cmd.arg(cwd.unwrap_or("~"));

    let mut wslenv = std::env::var("WSLENV").unwrap_or_default();
    if !wslenv.is_empty() {
        wslenv.push(':');
    }
    wslenv.push_str("ORITERM:TERM_PROGRAM:TERM_PROGRAM_VERSION");
    cmd.env("WSLENV", &wslenv);

    log::info!("shell_integration: wsl configured (env vars via WSLENV, no auto-injection)");
}
