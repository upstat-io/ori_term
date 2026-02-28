//! Tests for PTY config, command building, and shell detection.
//!
//! No real PTY processes are spawned — Alacritty and WezTerm don't test
//! live PTY either. The event loop is tested with mock pipes in
//! `event_loop/tests.rs`.

use super::PtyConfig;
use super::spawn::{build_command, compute_wslenv, default_shell};

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

#[cfg(windows)]
#[test]
fn build_command_sets_wslenv_for_cross_boundary_propagation() {
    let config = PtyConfig::default();
    let cmd = build_command(&config);

    let wslenv = cmd
        .get_env("WSLENV")
        .and_then(|v| v.to_str())
        .expect("WSLENV must be set on Windows");
    assert!(
        wslenv.contains("TERM"),
        "WSLENV must include TERM: {wslenv}",
    );
    assert!(
        wslenv.contains("COLORTERM"),
        "WSLENV must include COLORTERM: {wslenv}",
    );
    assert!(
        wslenv.contains("TERM_PROGRAM"),
        "WSLENV must include TERM_PROGRAM: {wslenv}",
    );
}

// ---------------------------------------------------------------------------
// WSLENV computation (cross-platform — tests the pure string logic)
// ---------------------------------------------------------------------------

#[test]
fn wslenv_empty_existing_adds_builtins() {
    let result = compute_wslenv("", &[]).unwrap();
    assert!(result.contains("TERM"));
    assert!(result.contains("COLORTERM"));
    assert!(result.contains("TERM_PROGRAM"));
    // Must not start with ':'.
    assert!(!result.starts_with(':'));
}

#[test]
fn wslenv_appends_to_existing() {
    let result = compute_wslenv("FOO:BAR", &[]).unwrap();
    assert!(
        result.starts_with("FOO:BAR:"),
        "must preserve existing entries: {result}",
    );
    assert!(result.contains("TERM"));
    assert!(result.contains("COLORTERM"));
    assert!(result.contains("TERM_PROGRAM"));
}

#[test]
fn wslenv_dedup_existing_entries() {
    // TERM already in WSLENV — should not appear twice in output.
    let result = compute_wslenv("TERM", &[]).unwrap();
    let count = result.split(':').filter(|s| *s == "TERM").count();
    assert_eq!(count, 1, "TERM must appear exactly once: {result}");
}

#[test]
fn wslenv_case_insensitive_dedup() {
    // Mixed-case "Term" should match our "TERM" and prevent duplicate.
    let result = compute_wslenv("Term:colorterm", &[]).unwrap();
    let keys: Vec<&str> = result.split(':').collect();

    // Only TERM_PROGRAM should be added (Term and colorterm already cover TERM and COLORTERM).
    assert_eq!(
        keys.iter()
            .filter(|k| k.eq_ignore_ascii_case("TERM"))
            .count(),
        1,
        "case-insensitive dedup for TERM: {result}",
    );
    assert_eq!(
        keys.iter()
            .filter(|k| k.eq_ignore_ascii_case("COLORTERM"))
            .count(),
        1,
        "case-insensitive dedup for COLORTERM: {result}",
    );
}

#[test]
fn wslenv_preserves_existing_flags() {
    // Entries with flags like `FOO/u` must survive in the output.
    let result = compute_wslenv("FOO/u:BAR/l", &[]).unwrap();
    assert!(
        result.contains("FOO/u"),
        "must preserve flags on existing entries: {result}",
    );
    assert!(
        result.contains("BAR/l"),
        "must preserve flags on existing entries: {result}",
    );
}

#[test]
fn wslenv_path_never_added() {
    // Even if user explicitly passes PATH, it must be excluded from WSLENV.
    let result = compute_wslenv("", &["PATH", "MY_VAR"]).unwrap();
    let keys: Vec<&str> = result.split(':').collect();
    assert!(
        !keys.iter().any(|k| k.eq_ignore_ascii_case("PATH")),
        "PATH must never appear in WSLENV: {result}",
    );
    assert!(
        keys.iter().any(|k| *k == "MY_VAR"),
        "user keys (non-PATH) must appear: {result}",
    );
}

#[test]
fn wslenv_user_env_overlapping_builtin() {
    // User provides TERM — should not appear twice.
    let result = compute_wslenv("", &["TERM", "MY_VAR"]).unwrap();
    let count = result.split(':').filter(|s| *s == "TERM").count();
    assert_eq!(
        count, 1,
        "overlapping user key must not duplicate: {result}"
    );
    assert!(result.contains("MY_VAR"), "user key must appear: {result}");
}

#[test]
fn wslenv_all_already_present_returns_none() {
    // Every builtin already in WSLENV, no user keys — nothing to add.
    let result = compute_wslenv("TERM:COLORTERM:TERM_PROGRAM", &[]);
    assert!(
        result.is_none(),
        "should return None when nothing to add: {result:?}",
    );
}

#[test]
fn wslenv_multiple_user_keys() {
    let result = compute_wslenv("", &["A", "B", "C"]).unwrap();
    let keys: Vec<&str> = result.split(':').collect();
    assert!(keys.contains(&"A"), "user key A missing: {result}");
    assert!(keys.contains(&"B"), "user key B missing: {result}");
    assert!(keys.contains(&"C"), "user key C missing: {result}");
    // Builtins also present.
    assert!(keys.contains(&"TERM"), "builtin TERM missing: {result}");
}

#[test]
fn wslenv_builtin_keys_before_user_keys() {
    let result = compute_wslenv("", &["ZZZ"]).unwrap();
    let keys: Vec<&str> = result.split(':').collect();
    let term_pos = keys.iter().position(|k| *k == "TERM").unwrap();
    let zzz_pos = keys.iter().position(|k| *k == "ZZZ").unwrap();
    assert!(
        term_pos < zzz_pos,
        "builtins should precede user keys: {result}",
    );
}

#[test]
fn wslenv_user_env_special_values_are_keys_not_values() {
    // Keys with unusual characters (underscores, digits) must pass through.
    let result = compute_wslenv("", &["MY_VAR_2", "X11_DISPLAY"]).unwrap();
    assert!(
        result.contains("MY_VAR_2"),
        "underscore+digit key: {result}"
    );
    assert!(result.contains("X11_DISPLAY"), "mixed key: {result}");
}
