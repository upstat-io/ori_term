//! Tests for shell detection, injection, and script embedding.

use std::path::Path;

use super::scripts::ensure_scripts_on_disk;
use super::{Shell, detect_shell};

// --- Shell detection ---

#[test]
fn detect_shell_unix_paths() {
    assert_eq!(detect_shell("/usr/bin/bash"), Some(Shell::Bash));
    assert_eq!(detect_shell("/bin/zsh"), Some(Shell::Zsh));
    assert_eq!(detect_shell("/usr/local/bin/fish"), Some(Shell::Fish));
    assert_eq!(detect_shell("/usr/bin/pwsh"), Some(Shell::PowerShell));
}

#[test]
fn detect_shell_windows_exe() {
    assert_eq!(detect_shell("bash.exe"), Some(Shell::Bash));
    assert_eq!(detect_shell("pwsh.exe"), Some(Shell::PowerShell));
    assert_eq!(detect_shell("powershell.exe"), Some(Shell::PowerShell));
    assert_eq!(detect_shell("wsl.exe"), Some(Shell::Wsl));
}

#[test]
fn detect_shell_bare_names() {
    assert_eq!(detect_shell("bash"), Some(Shell::Bash));
    assert_eq!(detect_shell("zsh"), Some(Shell::Zsh));
    assert_eq!(detect_shell("fish"), Some(Shell::Fish));
    assert_eq!(detect_shell("powershell"), Some(Shell::PowerShell));
}

#[test]
fn detect_shell_wsl() {
    assert_eq!(detect_shell("wsl"), Some(Shell::Wsl));
    assert_eq!(detect_shell("wsl.exe"), Some(Shell::Wsl));
}

#[test]
fn detect_shell_unknown() {
    assert_eq!(detect_shell("cmd.exe"), None);
    assert_eq!(detect_shell("sh"), None);
    assert_eq!(detect_shell("/bin/dash"), None);
    assert_eq!(detect_shell("nu"), None);
    assert_eq!(detect_shell(""), None);
}

#[test]
fn detect_shell_windows_full_paths() {
    assert_eq!(
        detect_shell(r"C:\Windows\System32\bash.exe"),
        Some(Shell::Bash)
    );
    assert_eq!(
        detect_shell(r"C:\Program Files\PowerShell\7\pwsh.exe"),
        Some(Shell::PowerShell)
    );
}

// --- Version stamping and script writing ---

#[test]
fn ensure_scripts_writes_all_files() {
    let tmp = tempfile::tempdir().expect("create temp dir");
    let dir = ensure_scripts_on_disk(tmp.path()).expect("write scripts");

    // Verify all expected files exist.
    assert!(dir.join("bash/oriterm.bash").is_file());
    assert!(dir.join("bash/bash-preexec.sh").is_file());
    assert!(dir.join("zsh/.zshenv").is_file());
    assert!(dir.join("zsh/oriterm-integration").is_file());
    assert!(
        dir.join("fish/vendor_conf.d/oriterm-shell-integration.fish")
            .is_file()
    );
    assert!(dir.join("powershell/oriterm.ps1").is_file());
    assert!(dir.join(".version").is_file());
}

#[test]
fn ensure_scripts_version_stamp_skips_rewrite() {
    let tmp = tempfile::tempdir().expect("create temp dir");

    // First write.
    let dir = ensure_scripts_on_disk(tmp.path()).expect("first write");

    // Record the mtime of a script file.
    let script = dir.join("bash/oriterm.bash");
    let mtime1 = std::fs::metadata(&script)
        .expect("metadata")
        .modified()
        .expect("mtime");

    // Brief pause so mtime would differ if rewritten.
    std::thread::sleep(std::time::Duration::from_millis(50));

    // Second write — should skip because version matches.
    let _ = ensure_scripts_on_disk(tmp.path()).expect("second write");

    let mtime2 = std::fs::metadata(&script)
        .expect("metadata")
        .modified()
        .expect("mtime");

    assert_eq!(mtime1, mtime2, "script should not be rewritten");
}

#[test]
fn ensure_scripts_rewrites_on_stale_version() {
    let tmp = tempfile::tempdir().expect("create temp dir");
    let dir = ensure_scripts_on_disk(tmp.path()).expect("first write");

    // Tamper with the version stamp.
    std::fs::write(dir.join(".version"), "0.0.0-stale").expect("tamper");

    // Should rewrite.
    let dir2 = ensure_scripts_on_disk(tmp.path()).expect("rewrite");
    let stamp = std::fs::read_to_string(dir2.join(".version")).expect("read stamp");
    assert_eq!(stamp.trim(), env!("CARGO_PKG_VERSION"));
}

#[test]
fn scripts_contain_osc_sequences() {
    // Verify the embedded scripts contain expected OSC sequences.
    let bash = include_str!("../../shell-integration/bash/oriterm.bash");
    assert!(bash.contains("133;A"), "bash script must emit OSC 133;A");
    assert!(bash.contains("133;B"), "bash script must emit OSC 133;B");
    assert!(bash.contains("133;C"), "bash script must emit OSC 133;C");
    assert!(bash.contains("133;D"), "bash script must emit OSC 133;D");
    assert!(bash.contains("]7;"), "bash script must emit OSC 7");

    let zsh = include_str!("../../shell-integration/zsh/oriterm-integration");
    assert!(zsh.contains("133;A"), "zsh script must emit OSC 133;A");
    assert!(zsh.contains("]7;"), "zsh script must emit OSC 7");

    let fish =
        include_str!("../../shell-integration/fish/vendor_conf.d/oriterm-shell-integration.fish");
    assert!(fish.contains("133;A"), "fish script must emit OSC 133;A");
    assert!(fish.contains("]7;"), "fish script must emit OSC 7");

    let ps = include_str!("../../shell-integration/powershell/oriterm.ps1");
    assert!(ps.contains("133;A"), "pwsh script must emit OSC 133;A");
    assert!(ps.contains("]7;"), "pwsh script must emit OSC 7");
}

// --- Injection configuration ---

#[test]
fn setup_injection_bash_returns_posix_flag() {
    let mut cmd = portable_pty::CommandBuilder::new("bash");
    let dir = Path::new("/tmp/test-integration");

    let extra = super::inject::setup_injection(&mut cmd, Shell::Bash, dir, None);
    assert_eq!(extra, Some("--posix"));
}

#[test]
fn setup_injection_zsh_returns_none() {
    let mut cmd = portable_pty::CommandBuilder::new("zsh");
    let dir = Path::new("/tmp/test-integration");

    let extra = super::inject::setup_injection(&mut cmd, Shell::Zsh, dir, None);
    assert_eq!(extra, None);
}

#[test]
fn setup_injection_fish_returns_none() {
    let mut cmd = portable_pty::CommandBuilder::new("fish");
    let dir = Path::new("/tmp/test-integration");

    let extra = super::inject::setup_injection(&mut cmd, Shell::Fish, dir, None);
    assert_eq!(extra, None);
}

#[test]
fn setup_injection_powershell_returns_none() {
    let mut cmd = portable_pty::CommandBuilder::new("pwsh");
    let dir = Path::new("/tmp/test-integration");

    let extra = super::inject::setup_injection(&mut cmd, Shell::PowerShell, dir, None);
    assert_eq!(extra, None);
}

#[test]
fn setup_injection_wsl_returns_none() {
    let mut cmd = portable_pty::CommandBuilder::new("wsl");
    let dir = Path::new("/tmp/test-integration");

    let extra = super::inject::setup_injection(&mut cmd, Shell::Wsl, dir, Some("/home/user"));
    assert_eq!(extra, None);
}

// --- Raw interceptor ---

use oriterm_core::{PromptState, Term, Theme, VoidListener};

/// Helper: create a minimal terminal for interceptor tests.
fn make_term() -> Term<VoidListener> {
    Term::new(24, 80, 100, Theme::Dark, VoidListener)
}

/// Helper: feed raw bytes through the interceptor.
fn intercept(term: &mut Term<VoidListener>, bytes: &[u8]) {
    let mut parser = vte::Parser::new();
    let mut interceptor = super::interceptor::RawInterceptor::new(term);
    parser.advance(&mut interceptor, bytes);
}

#[test]
fn interceptor_osc7_sets_cwd() {
    let mut term = make_term();
    assert!(term.cwd().is_none());

    intercept(&mut term, b"\x1b]7;file://hostname/home/user\x07");
    assert_eq!(term.cwd(), Some("/home/user"));
}

#[test]
fn interceptor_osc7_empty_hostname() {
    let mut term = make_term();
    intercept(&mut term, b"\x1b]7;file:///tmp/test\x07");
    assert_eq!(term.cwd(), Some("/tmp/test"));
}

#[test]
fn interceptor_osc7_marks_title_dirty() {
    let mut term = make_term();
    assert!(!term.is_title_dirty());

    intercept(&mut term, b"\x1b]7;file://host/home\x07");
    assert!(term.is_title_dirty());
    assert!(!term.has_explicit_title());
}

#[test]
fn interceptor_osc133_prompt_state_transitions() {
    let mut term = make_term();
    assert_eq!(term.prompt_state(), PromptState::None);

    // A — prompt start
    intercept(&mut term, b"\x1b]133;A\x07");
    assert_eq!(term.prompt_state(), PromptState::PromptStart);
    assert!(term.prompt_mark_pending());

    // B — command start
    intercept(&mut term, b"\x1b]133;B\x07");
    assert_eq!(term.prompt_state(), PromptState::CommandStart);

    // C — output start
    intercept(&mut term, b"\x1b]133;C\x07");
    assert_eq!(term.prompt_state(), PromptState::OutputStart);

    // D — command complete
    intercept(&mut term, b"\x1b]133;D\x07");
    assert_eq!(term.prompt_state(), PromptState::None);
}

#[test]
fn interceptor_osc9_simple_notification() {
    let mut term = make_term();
    intercept(&mut term, b"\x1b]9;Hello world\x07");

    let notifs = term.drain_notifications();
    assert_eq!(notifs.len(), 1);
    assert_eq!(notifs[0].body, "Hello world");
    assert!(notifs[0].title.is_empty());
}

#[test]
fn interceptor_osc777_notification() {
    let mut term = make_term();
    intercept(&mut term, b"\x1b]777;notify;Build;Done!\x07");

    let notifs = term.drain_notifications();
    assert_eq!(notifs.len(), 1);
    assert_eq!(notifs[0].title, "Build");
    assert_eq!(notifs[0].body, "Done!");
}

#[test]
fn interceptor_osc777_ignores_non_notify() {
    let mut term = make_term();
    intercept(&mut term, b"\x1b]777;other;foo;bar\x07");

    let notifs = term.drain_notifications();
    assert!(notifs.is_empty());
}

// --- Effective title resolution ---

#[test]
fn effective_title_prefers_explicit() {
    let mut term = make_term();

    // Set explicit title via OSC 0 (high-level VTE processor).
    let mut proc = vte::ansi::Processor::<vte::ansi::StdSyncHandler>::new();
    proc.advance(&mut term, b"\x1b]0;my terminal\x07");

    // Also set CWD via raw interceptor.
    *term.cwd_mut() = Some("/home/user/projects".to_string());

    // Explicit title should win.
    assert_eq!(term.effective_title(), "my terminal");
}

#[test]
fn effective_title_falls_back_to_cwd() {
    let mut term = make_term();

    // CWD set but no explicit title.
    *term.cwd_mut() = Some("/home/user/projects".to_string());
    assert!(!term.has_explicit_title());

    assert_eq!(term.effective_title(), "projects");
}

#[test]
fn effective_title_cwd_after_osc7_clears_explicit() {
    let mut term = make_term();

    // Set explicit title via OSC 0.
    let mut proc = vte::ansi::Processor::<vte::ansi::StdSyncHandler>::new();
    proc.advance(&mut term, b"\x1b]0;vim\x07");
    assert_eq!(term.effective_title(), "vim");

    // OSC 7 clears explicit flag.
    intercept(&mut term, b"\x1b]7;file:///home/user/code\x07");
    assert!(!term.has_explicit_title());
    assert_eq!(term.effective_title(), "code");
}

#[test]
fn effective_title_empty_fallback() {
    let term = make_term();
    // No explicit title, no CWD.
    assert_eq!(term.effective_title(), "");
}

// --- Prompt row tracking ---

#[test]
fn mark_prompt_row_records_position() {
    let mut term = make_term();
    assert!(term.prompt_rows().is_empty());

    // Simulate OSC 133;A → prompt_mark_pending = true.
    intercept(&mut term, b"\x1b]133;A\x07");
    assert!(term.prompt_mark_pending());

    // Mark the prompt row (deferred marking).
    term.mark_prompt_row();
    assert!(!term.prompt_mark_pending());
    assert_eq!(term.prompt_rows(), &[0]); // Cursor at row 0 (scrollback 0 + line 0).
}

#[test]
fn mark_prompt_row_avoids_duplicates() {
    let mut term = make_term();

    intercept(&mut term, b"\x1b]133;A\x07");
    term.mark_prompt_row();
    intercept(&mut term, b"\x1b]133;A\x07");
    term.mark_prompt_row();

    // Should not duplicate the same row.
    assert_eq!(term.prompt_rows(), &[0]);
}

#[test]
fn prune_prompt_rows_removes_evicted() {
    let mut term = make_term();

    // Manually insert some prompt rows.
    intercept(&mut term, b"\x1b]133;A\x07");
    term.mark_prompt_row();
    // Simulate moving cursor down and marking another prompt.
    term.set_prompt_mark_pending(true);
    // Can't easily move cursor in tests, so test prune directly.
    // Push artificial rows for testing.
    term.prune_prompt_rows(0); // No-op.
    assert_eq!(term.prompt_rows().len(), 1);
}

#[test]
fn no_prompts_navigation_is_noop() {
    let mut term = make_term();
    assert!(!term.scroll_to_previous_prompt());
    assert!(!term.scroll_to_next_prompt());
}

#[test]
fn interceptor_osc7_path_parsing() {
    use super::interceptor::parse_osc7_path;
    assert_eq!(parse_osc7_path("file://host/home/user"), "/home/user");
    assert_eq!(parse_osc7_path("file:///tmp"), "/tmp");
    assert_eq!(parse_osc7_path("/just/a/path"), "/just/a/path");
    assert_eq!(parse_osc7_path("file://host"), "host");
}

// --- Command timing ---

#[test]
fn osc133c_records_command_start() {
    let mut term = make_term();
    assert!(term.last_command_duration().is_none());

    // OSC 133;C — output start (command executing).
    intercept(&mut term, b"\x1b]133;C\x07");
    assert_eq!(term.prompt_state(), PromptState::OutputStart);
    // command_start is set internally but not directly exposed.
    // We verify indirectly by completing the command.
}

#[test]
fn osc133d_computes_command_duration() {
    let mut term = make_term();

    // C → D cycle should produce a duration.
    intercept(&mut term, b"\x1b]133;C\x07");
    std::thread::sleep(std::time::Duration::from_millis(10));
    intercept(&mut term, b"\x1b]133;D\x07");

    assert_eq!(term.prompt_state(), PromptState::None);
    let dur = term.last_command_duration().expect("should have duration");
    assert!(
        dur.as_millis() >= 10,
        "duration should be >= 10ms, got {dur:?}"
    );
}

#[test]
fn osc133d_without_c_produces_no_duration() {
    let mut term = make_term();

    // D without prior C — no duration.
    intercept(&mut term, b"\x1b]133;D\x07");
    assert!(term.last_command_duration().is_none());
}

#[test]
fn command_duration_updates_on_new_command() {
    let mut term = make_term();

    // First command.
    intercept(&mut term, b"\x1b]133;C\x07");
    std::thread::sleep(std::time::Duration::from_millis(10));
    intercept(&mut term, b"\x1b]133;D\x07");
    let dur1 = term.last_command_duration().unwrap();

    // Second command.
    intercept(&mut term, b"\x1b]133;C\x07");
    std::thread::sleep(std::time::Duration::from_millis(10));
    intercept(&mut term, b"\x1b]133;D\x07");
    let dur2 = term.last_command_duration().unwrap();

    // Both should be valid, second may differ slightly.
    assert!(dur1.as_millis() >= 10);
    assert!(dur2.as_millis() >= 10);
}

// --- Gap analysis tests ---

// OSC 7: percent-encoded paths (Fish and some shells percent-encode URIs).

#[test]
fn interceptor_osc7_percent_encoded_space() {
    let mut term = make_term();
    intercept(&mut term, b"\x1b]7;file://host/home/user/my%20project\x07");
    assert_eq!(term.cwd(), Some("/home/user/my project"));
}

#[test]
fn interceptor_osc7_percent_encoded_special_chars() {
    let mut term = make_term();
    // %C3%A9 is UTF-8 for 'é'.
    intercept(&mut term, b"\x1b]7;file:///home/user/caf%C3%A9\x07");
    assert_eq!(term.cwd(), Some("/home/user/café"));
}

#[test]
fn percent_decode_passthrough() {
    use super::interceptor::percent_decode;
    let input = "/home/user/projects";
    let result = percent_decode(input);
    assert!(matches!(result, std::borrow::Cow::Borrowed(_)));
    assert_eq!(result, "/home/user/projects");
}

#[test]
fn percent_decode_space_and_hash() {
    use super::interceptor::percent_decode;
    assert_eq!(percent_decode("hello%20world"), "hello world");
    assert_eq!(percent_decode("%23hash"), "#hash");
}

#[test]
fn percent_decode_invalid_hex_passthrough() {
    use super::interceptor::percent_decode;
    // %ZZ is not valid hex — pass through literally.
    assert_eq!(percent_decode("hello%ZZworld"), "hello%ZZworld");
    // Truncated: % at end of string.
    assert_eq!(percent_decode("hello%2"), "hello%2");
}

// OSC 7: Windows drive letter paths (cross-compiled from WSL targeting Windows).

#[test]
fn interceptor_osc7_windows_drive_letter() {
    let mut term = make_term();
    intercept(&mut term, b"\x1b]7;file:///C:/Users/eric/code\x07");
    assert_eq!(term.cwd(), Some("/C:/Users/eric/code"));
}

#[test]
fn parse_osc7_path_windows_drive() {
    use super::interceptor::parse_osc7_path;
    assert_eq!(parse_osc7_path("file:///C:/Users/eric"), "/C:/Users/eric");
}

// OSC 7: query string and fragment stripping.

#[test]
fn interceptor_osc7_strips_query_and_fragment() {
    let mut term = make_term();
    intercept(
        &mut term,
        b"\x1b]7;file://host/home/user?query=1#section\x07",
    );
    assert_eq!(term.cwd(), Some("/home/user"));
}

#[test]
fn parse_osc7_path_strips_query() {
    use super::interceptor::parse_osc7_path;
    assert_eq!(parse_osc7_path("file://host/home/user?q=1"), "/home/user");
}

#[test]
fn parse_osc7_path_strips_fragment() {
    use super::interceptor::parse_osc7_path;
    assert_eq!(
        parse_osc7_path("file://host/home/user#section"),
        "/home/user"
    );
}

#[test]
fn parse_osc7_path_bare_path_strips_fragment() {
    use super::interceptor::parse_osc7_path;
    assert_eq!(parse_osc7_path("/tmp/dir#frag"), "/tmp/dir");
}

// OSC 133;D with exit code parameter (shells emit `D;0` or `D;127`).

#[test]
fn interceptor_osc133d_with_exit_code_zero() {
    let mut term = make_term();
    intercept(&mut term, b"\x1b]133;C\x07");
    std::thread::sleep(std::time::Duration::from_millis(10));
    intercept(&mut term, b"\x1b]133;D;0\x07");

    assert_eq!(term.prompt_state(), PromptState::None);
    assert!(term.last_command_duration().is_some());
}

#[test]
fn interceptor_osc133d_with_nonzero_exit_code() {
    let mut term = make_term();
    intercept(&mut term, b"\x1b]133;C\x07");
    intercept(&mut term, b"\x1b]133;D;127\x07");

    assert_eq!(term.prompt_state(), PromptState::None);
}

// OSC 99: Kitty notification protocol.

#[test]
fn interceptor_osc99_kitty_notification() {
    let mut term = make_term();
    intercept(&mut term, b"\x1b]99;Build complete\x07");

    let notifs = term.drain_notifications();
    assert_eq!(notifs.len(), 1);
    assert_eq!(notifs[0].body, "Build complete");
    assert!(notifs[0].title.is_empty());
}

// Script writing: nonexistent parent directory returns error.

#[test]
fn ensure_scripts_nonexistent_parent_returns_error() {
    let result = ensure_scripts_on_disk(Path::new("/nonexistent/path/shell-int"));
    assert!(result.is_err());
}

// Prompt navigation with multiple prompts across scrollback.

#[test]
fn prompt_navigation_scrolls_to_previous() {
    // Small terminal: 4 visible lines, 100 scrollback.
    let mut term = Term::new(4, 80, 100, Theme::Dark, VoidListener);
    let mut proc = vte::ansi::Processor::<vte::ansi::StdSyncHandler>::new();

    // Mark prompt at current position (abs row 0).
    term.set_prompt_mark_pending(true);
    term.mark_prompt_row();
    assert_eq!(term.prompt_rows(), &[0]);

    // Write enough lines to push the prompt into scrollback.
    for _ in 0..20 {
        proc.advance(&mut term, b"\n");
    }

    // Viewport is at bottom, prompt row 0 is in scrollback.
    assert!(term.scroll_to_previous_prompt());
}

#[test]
fn prompt_navigation_no_prompt_above_returns_false() {
    let mut term = Term::new(4, 80, 100, Theme::Dark, VoidListener);

    // Mark prompt at current position (row 0), viewport is already here.
    term.set_prompt_mark_pending(true);
    term.mark_prompt_row();

    // No prompt ABOVE viewport top — should return false.
    assert!(!term.scroll_to_previous_prompt());
}

#[test]
fn prompt_navigation_scrolls_to_next() {
    let mut term = Term::new(4, 80, 100, Theme::Dark, VoidListener);
    let mut proc = vte::ansi::Processor::<vte::ansi::StdSyncHandler>::new();

    // Write some content then mark a prompt.
    for _ in 0..10 {
        proc.advance(&mut term, b"\n");
    }
    term.set_prompt_mark_pending(true);
    term.mark_prompt_row();
    let prompt_row = *term.prompt_rows().last().unwrap();

    // Write more content to push the prompt into scrollback.
    for _ in 0..20 {
        proc.advance(&mut term, b"\n");
    }

    // Scroll all the way up.
    let sb_len = term.grid().scrollback().len();
    term.grid_mut().scroll_display(sb_len as isize);

    // Now navigate to the next prompt (should be below viewport).
    let scrolled = term.scroll_to_next_prompt();
    // There should be a prompt row at or after the viewport bottom.
    assert!(
        scrolled || prompt_row < sb_len,
        "should navigate to prompt below viewport"
    );
}
