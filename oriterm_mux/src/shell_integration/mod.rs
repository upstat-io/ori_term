//! Shell integration: detect the user's shell, write integration scripts to
//! disk, and configure the PTY command environment so those scripts are
//! automatically sourced.
//!
//! Each shell gets its own injection mechanism (env vars / extra args) that
//! causes it to source our scripts on startup, emitting OSC 133 prompt
//! markers and OSC 7 CWD reports.

mod inject;
pub(crate) mod interceptor;
mod scripts;

use portable_pty::CommandBuilder;

pub(crate) use self::inject::setup_injection;
pub(crate) use self::scripts::ensure_scripts_on_disk;

/// Shells we know how to inject integration scripts into.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(
    clippy::enum_variant_names,
    reason = "Shell::PowerShell is clearer than Shell::Power"
)]
pub(crate) enum Shell {
    Bash,
    Zsh,
    Fish,
    PowerShell,
    /// WSL launches the user's default login shell inside a Linux VM.
    /// Simple env vars propagate via WSLENV; shell integration is user-sourced.
    Wsl,
}

/// Detect the shell from a program path or name.
///
/// Matches on the basename, ignoring `.exe` suffix on Windows.
/// Handles both `/` and `\` path separators regardless of host platform.
pub(crate) fn detect_shell(program: &str) -> Option<Shell> {
    // Handle both Unix `/` and Windows `\` separators regardless of host OS.
    let base = program.rsplit(['/', '\\']).next().unwrap_or(program);
    let name = base.strip_suffix(".exe").unwrap_or(base);

    match name {
        "bash" => Some(Shell::Bash),
        "zsh" => Some(Shell::Zsh),
        "fish" => Some(Shell::Fish),
        "pwsh" | "powershell" => Some(Shell::PowerShell),
        "wsl" => Some(Shell::Wsl),
        _ => None,
    }
}

/// Set common identification env vars on a command.
///
/// These variables are set regardless of shell type and tell the child
/// process that it's running inside oriterm.
fn set_common_env(cmd: &mut CommandBuilder) {
    cmd.env("ORITERM", "1");
    cmd.env("TERM_PROGRAM", "oriterm");
    cmd.env("TERM_PROGRAM_VERSION", env!("CARGO_PKG_VERSION"));
}

#[cfg(test)]
mod tests;
