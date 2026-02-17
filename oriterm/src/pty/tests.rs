//! Tests for PTY config, command building, and shell detection.
//!
//! No real PTY processes are spawned — Alacritty and WezTerm don't test
//! live PTY either. The event loop is tested with mock pipes in
//! `event_loop/tests.rs`.

use super::PtyConfig;
use super::spawn::{build_command, default_shell};

// ---------------------------------------------------------------------------
// Shell detection
// ---------------------------------------------------------------------------

#[test]
fn default_shell_is_nonempty() {
    let shell = default_shell();
    assert!(!shell.is_empty(), "default shell must not be empty");
}

#[cfg(unix)]
#[test]
fn default_shell_exists_on_disk() {
    let shell = default_shell();
    let path = std::path::Path::new(shell);
    assert!(path.exists(), "default shell `{shell}` does not exist");
}

// ---------------------------------------------------------------------------
// Command building
// ---------------------------------------------------------------------------

#[test]
fn build_command_sets_terminal_env_vars() {
    let config = PtyConfig::default();
    let cmd = build_command(&config);

    assert_eq!(
        cmd.get_env("TERM").and_then(|v| v.to_str()),
        Some("xterm-256color"),
    );
    assert_eq!(
        cmd.get_env("COLORTERM").and_then(|v| v.to_str()),
        Some("truecolor"),
    );
    assert_eq!(
        cmd.get_env("TERM_PROGRAM").and_then(|v| v.to_str()),
        Some("oriterm"),
    );
}

#[test]
fn build_command_applies_user_env_overrides() {
    let config = PtyConfig {
        env: vec![("MY_VAR".into(), "my_value".into())],
        ..Default::default()
    };
    let cmd = build_command(&config);

    assert_eq!(
        cmd.get_env("MY_VAR").and_then(|v| v.to_str()),
        Some("my_value"),
    );
}

#[test]
fn build_command_uses_custom_shell() {
    let config = PtyConfig {
        shell: Some("/bin/sh".into()),
        ..Default::default()
    };
    let cmd = build_command(&config);
    let argv = cmd.get_argv();

    assert!(!argv.is_empty());
    assert_eq!(argv[0], "/bin/sh");
}

#[test]
fn build_command_with_working_directory() {
    let config = PtyConfig {
        working_dir: Some("/tmp".into()),
        ..Default::default()
    };
    let cmd = build_command(&config);
    let argv = cmd.get_argv();

    // Command should be buildable with a working directory.
    assert!(!argv.is_empty());
}

#[test]
fn build_command_default_shell_used_when_none() {
    let config = PtyConfig::default();
    let cmd = build_command(&config);
    let argv = cmd.get_argv();

    assert!(!argv.is_empty());
    assert_eq!(argv[0], default_shell());
}
