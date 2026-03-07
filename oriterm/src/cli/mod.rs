//! CLI subcommands for headless diagnostics.
//!
//! Provides `ls-fonts`, `show-keys`, `list-themes`, `validate-config`,
//! `show-config`, and `completions` subcommands that run without opening a
//! window. Standard in modern terminals (`WezTerm` `ls-fonts`, Ghostty
//! `+list-fonts`).

use std::fmt::Write;
use std::process;

use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::Shell;

use crate::config::{self, Config};
use crate::font::discovery;
use crate::keybindings::{self, Action, BindingKey, KeyBinding};

/// GPU-accelerated terminal emulator.
#[derive(Parser)]
#[command(
    name = "oriterm",
    version = env!("ORITERM_VERSION"),
    long_version = env!("ORITERM_VERSION"),
    about
)]
pub(crate) struct Cli {
    /// Subcommand to run (omit to launch the terminal).
    #[command(subcommand)]
    pub command: Option<SubCommand>,

    /// Connect to a running mux daemon at this socket path.
    ///
    /// Instead of running an embedded mux, the terminal connects to
    /// an existing `oriterm-mux` daemon for multiplexer state. Used
    /// together with `--window` for cross-process tab migration.
    #[arg(long)]
    pub connect: Option<std::path::PathBuf>,

    /// Claim an existing mux window ID (used with `--connect`).
    ///
    /// When connecting to a daemon that already has a window allocated,
    /// pass its numeric ID here. The terminal will render that window
    /// instead of creating a new one.
    #[arg(long, requires = "connect")]
    pub window: Option<u64>,

    /// Open a new window (default when daemon is running).
    #[arg(long)]
    pub new_window: bool,

    /// Force embedded (single-process) mode, ignoring config.
    ///
    /// Bypasses daemon auto-start entirely. Useful for debugging,
    /// CI testing, or environments where daemon spawning isn't possible.
    #[arg(long)]
    pub embedded: bool,
}

/// Diagnostic subcommands that run headlessly.
#[derive(Subcommand)]
pub(crate) enum SubCommand {
    /// List discovered fonts and fallback chain.
    LsFonts(LsFontsArgs),
    /// Dump current keybindings.
    ShowKeys(ShowKeysArgs),
    /// List available color themes.
    ListThemes(ListThemesArgs),
    /// Validate the config file without launching.
    ValidateConfig,
    /// Dump the resolved config (defaults + user overrides) as TOML.
    ShowConfig,
    /// Generate shell completion scripts.
    Completions(CompletionsArgs),
}

/// Arguments for the `ls-fonts` subcommand.
#[derive(Parser)]
pub(crate) struct LsFontsArgs {
    /// Show which font resolves a specific character.
    #[arg(long)]
    codepoint: Option<char>,
}

/// Arguments for the `show-keys` subcommand.
#[derive(Parser)]
pub(crate) struct ShowKeysArgs {
    /// Show only built-in default bindings (ignore user config).
    #[arg(long)]
    default: bool,
}

/// Arguments for the `list-themes` subcommand.
#[derive(Parser)]
pub(crate) struct ListThemesArgs {
    /// Print a 16-color sample for each theme.
    #[arg(long)]
    preview: bool,
}

/// Arguments for the `completions` subcommand.
#[derive(Parser)]
pub(crate) struct CompletionsArgs {
    /// Shell to generate completions for.
    #[arg(value_enum)]
    pub shell: Shell,
}

/// Attach to the parent console on Windows so CLI output is visible.
///
/// The `#![windows_subsystem = "windows"]` attribute suppresses the console.
/// CLI subcommands need to write to the parent terminal.
pub(crate) fn attach_console() {
    #[cfg(windows)]
    {
        // SAFETY: `AttachConsole` is a standard Win32 API. Passing
        // `ATTACH_PARENT_PROCESS` attaches to the console of the parent
        // process (e.g. cmd.exe / PowerShell). Failure is harmless —
        // output just won't be visible.
        #[allow(unsafe_code)]
        unsafe {
            windows_sys::Win32::System::Console::AttachConsole(
                windows_sys::Win32::System::Console::ATTACH_PARENT_PROCESS,
            );
        }
    }
}

/// Dispatch a CLI subcommand. Prints to stdout and exits.
pub(crate) fn dispatch(cmd: SubCommand) -> ! {
    match cmd {
        SubCommand::LsFonts(args) => run_ls_fonts(&args),
        SubCommand::ShowKeys(args) => run_show_keys(&args),
        SubCommand::ListThemes(args) => run_list_themes(&args),
        SubCommand::ValidateConfig => run_validate_config(),
        SubCommand::ShowConfig => run_show_config(),
        SubCommand::Completions(args) => run_completions(&args),
    }
}

/// `ls-fonts` — list discovered fonts with fallback chain.
fn run_ls_fonts(args: &LsFontsArgs) -> ! {
    let config = Config::load();
    let weight = config.font.effective_weight();
    let result = discovery::discover_fonts(config.font.family.as_deref(), weight);

    let mut out = String::new();
    let _ = writeln!(out, "Primary font family: {}", result.primary.family_name);
    let _ = writeln!(out, "  Origin: {:?}", result.primary.origin);

    let labels = ["Regular", "Bold", "Italic", "Bold Italic"];
    for (i, label) in labels.iter().enumerate() {
        if result.primary.has_variant[i] {
            let path_str = result.primary.paths[i]
                .as_ref()
                .map_or_else(|| "(embedded)".to_owned(), |p| p.display().to_string());
            let _ = writeln!(out, "  {label}: {path_str}");
        } else {
            let _ = writeln!(out, "  {label}: (synthesized)");
        }
    }

    if !result.fallbacks.is_empty() {
        let _ = writeln!(out, "\nFallback chain:");
        for (i, fb) in result.fallbacks.iter().enumerate() {
            let _ = writeln!(
                out,
                "  {}. {} (index {})",
                i + 1,
                fb.path.display(),
                fb.face_index
            );
        }
    }

    if let Some(ch) = args.codepoint {
        let _ = writeln!(out, "\nCodepoint U+{:04X} ({ch}):", ch as u32);
        let _ = writeln!(
            out,
            "  (font resolution requires loading — run the terminal to test)"
        );
    }

    print!("{out}");
    process::exit(0)
}

/// `show-keys` — dump keybindings in human-readable format.
fn run_show_keys(args: &ShowKeysArgs) -> ! {
    let bindings = if args.default {
        keybindings::default_bindings()
    } else {
        let config = Config::load();
        keybindings::merge_bindings(&config.keybind)
    };

    let mut out = String::new();
    let source = if args.default { "Default" } else { "Active" };
    let _ = writeln!(out, "{source} keybindings:\n");

    for b in &bindings {
        let _ = writeln!(out, "  {}", format_binding(b));
    }

    print!("{out}");
    process::exit(0)
}

/// `list-themes` — list available color schemes.
fn run_list_themes(args: &ListThemesArgs) -> ! {
    let mut out = String::new();
    let _ = writeln!(out, "Available themes:\n");
    let _ = writeln!(out, "  * Catppuccin Mocha (default)");

    if args.preview {
        let _ = writeln!(out);
        let _ = writeln!(out, "  16-color palette:");
        let _ = write!(out, "  ");
        // Standard ANSI colors 0-7.
        for i in 0..8u8 {
            let _ = write!(out, "\x1b[48;5;{i}m  ");
        }
        let _ = writeln!(out, "\x1b[0m");
        let _ = write!(out, "  ");
        // Bright colors 8-15.
        for i in 8..16u8 {
            let _ = write!(out, "\x1b[48;5;{i}m  ");
        }
        let _ = writeln!(out, "\x1b[0m");
    }

    print!("{out}");
    process::exit(0)
}

/// `validate-config` — parse and validate the config file, exit 0 or 1.
fn run_validate_config() -> ! {
    let exit_code = match validate_config_inner() {
        Ok(()) => {
            println!("config: valid");
            0
        }
        Err(errors) => {
            for e in &errors {
                eprintln!("error: {e}");
            }
            1
        }
    };
    process::exit(exit_code)
}

/// Core validation logic, separated for testability.
///
/// Returns `Ok(())` when the config is valid, or a list of error messages.
fn validate_config_inner() -> Result<(), Vec<String>> {
    let config = match Config::try_load() {
        Ok(c) => c,
        Err(e) => return Err(vec![e]),
    };

    let mut errors = Vec::new();
    validate_colors(&config, &mut errors);
    validate_keybindings(&config, &mut errors);

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Validate hex color strings in the config.
fn validate_colors(config: &Config, errors: &mut Vec<String>) {
    let fields: &[(&str, &Option<String>)] = &[
        ("colors.foreground", &config.colors.foreground),
        ("colors.background", &config.colors.background),
        ("colors.cursor", &config.colors.cursor),
        (
            "colors.selection_foreground",
            &config.colors.selection_foreground,
        ),
        (
            "colors.selection_background",
            &config.colors.selection_background,
        ),
    ];

    for (name, value) in fields {
        if let Some(hex) = value {
            if config::parse_hex_color(hex).is_none() {
                errors.push(format!("{name}: invalid hex color {hex:?}"));
            }
        }
    }

    for (key, hex) in &config.colors.ansi {
        if config::parse_hex_color(hex).is_none() {
            errors.push(format!("colors.ansi.{key}: invalid hex color {hex:?}"));
        }
    }
    for (key, hex) in &config.colors.bright {
        if config::parse_hex_color(hex).is_none() {
            errors.push(format!("colors.bright.{key}: invalid hex color {hex:?}"));
        }
    }

    if let Some(hex) = &config.bell.color {
        if config::parse_hex_color(hex).is_none() {
            errors.push(format!("bell.color: invalid hex color {hex:?}"));
        }
    }
}

/// Validate keybinding entries.
fn validate_keybindings(config: &Config, errors: &mut Vec<String>) {
    for (i, kb) in config.keybind.iter().enumerate() {
        if keybindings::parse_key(&kb.key).is_none() {
            errors.push(format!("keybind[{i}]: unknown key {:?}", kb.key));
        }
        if keybindings::parse_action(&kb.action).is_none() {
            errors.push(format!("keybind[{i}]: unknown action {:?}", kb.action));
        }
    }
}

/// `show-config` — dump the resolved config as TOML.
fn run_show_config() -> ! {
    let config = Config::load();
    match toml::to_string_pretty(&config) {
        Ok(toml) => {
            print!("{toml}");
            process::exit(0);
        }
        Err(e) => {
            eprintln!("error: failed to serialize config: {e}");
            process::exit(1);
        }
    }
}

/// `completions` — generate shell completion script for the given shell.
fn run_completions(args: &CompletionsArgs) -> ! {
    use std::io::IsTerminal;

    let mut cmd = Cli::command();
    clap_complete::generate(args.shell, &mut cmd, "oriterm", &mut std::io::stdout());

    // When output goes to a terminal (not redirected to a file), print
    // install instructions on stderr so the user knows what to do.
    if std::io::stdout().is_terminal() {
        eprintln!();
        match args.shell {
            Shell::Bash => {
                eprintln!("# To install, run:");
                eprintln!(
                    "#   oriterm completions bash > ~/.local/share/bash-completion/completions/oriterm"
                );
            }
            Shell::Zsh => {
                eprintln!("# To install, add to your fpath and run:");
                eprintln!("#   oriterm completions zsh > ~/.zfunc/_oriterm");
                eprintln!("#   echo 'fpath=(~/.zfunc $fpath)' >> ~/.zshrc");
            }
            Shell::Fish => {
                eprintln!("# To install, run:");
                eprintln!("#   oriterm completions fish > ~/.config/fish/completions/oriterm.fish");
            }
            Shell::PowerShell => {
                eprintln!("# To install, add to your PowerShell profile:");
                eprintln!("#   oriterm completions powershell >> $PROFILE");
            }
            _ => {}
        }
    }

    process::exit(0)
}

/// Generate completion script into a byte buffer (for testing).
#[cfg(test)]
fn generate_completions(shell: Shell) -> Vec<u8> {
    let mut cmd = Cli::command();
    let mut buf = Vec::new();
    clap_complete::generate(shell, &mut cmd, "oriterm", &mut buf);
    buf
}

/// Format a single keybinding as `Mods+Key -> Action`.
fn format_binding(b: &KeyBinding) -> String {
    let mut parts = Vec::new();

    if b.mods.contains(crate::key_encoding::Modifiers::CONTROL) {
        parts.push("Ctrl");
    }
    if b.mods.contains(crate::key_encoding::Modifiers::SHIFT) {
        parts.push("Shift");
    }
    if b.mods.contains(crate::key_encoding::Modifiers::ALT) {
        parts.push("Alt");
    }
    if b.mods.contains(crate::key_encoding::Modifiers::SUPER) {
        parts.push("Super");
    }

    let key_name = format_binding_key(&b.key);
    parts.push(&key_name);
    let combo = parts.join("+");

    let action = format_action(&b.action);
    format!("{combo} -> {action}")
}

/// Format a `BindingKey` as a human-readable string.
fn format_binding_key(key: &BindingKey) -> String {
    match key {
        BindingKey::Named(n) => format!("{n:?}"),
        BindingKey::Character(s) => s.to_uppercase(),
    }
}

/// Format an `Action` as a human-readable string.
fn format_action(action: &Action) -> String {
    match action {
        Action::SendText(t) => format!("SendText:{t:?}"),
        other => other.as_str().to_owned(),
    }
}

#[cfg(test)]
mod tests;
