//! Built-in default keybindings.

use winit::keyboard::NamedKey;

use crate::key_encoding::Modifiers;

use super::{Action, BindingKey, KeyBinding};

/// Construct a `KeyBinding` from parts.
fn bind(key: BindingKey, mods: Modifiers, action: Action) -> KeyBinding {
    KeyBinding { key, mods, action }
}

/// All built-in default keybindings.
///
/// More-specific modifier combos come first so that e.g.
/// Ctrl+Shift+C (Copy) is checked before Ctrl+C (`SmartCopy`).
pub(crate) fn default_bindings() -> Vec<KeyBinding> {
    let ch = |s: &str| BindingKey::Character(s.to_owned());
    let named = BindingKey::Named;
    let ctrl = Modifiers::CONTROL;
    let shift = Modifiers::SHIFT;
    let alt = Modifiers::ALT;
    let cs = ctrl | shift;
    let ca = ctrl | alt;

    #[allow(
        unused_mut,
        reason = "mut needed for macOS cfg block that extends bindings"
    )]
    let mut bindings = vec![
        // Explicit copy / paste (Ctrl+Shift+C/V).
        bind(ch("c"), cs, Action::Copy),
        bind(ch("v"), cs, Action::Paste),
        // Ctrl+Insert / Shift+Insert.
        bind(named(NamedKey::Insert), ctrl, Action::Copy),
        bind(named(NamedKey::Insert), shift, Action::Paste),
        // Config / search.
        bind(ch("r"), cs, Action::ReloadConfig),
        bind(ch("f"), cs, Action::OpenSearch),
        // Zoom.
        bind(ch("="), ctrl, Action::ZoomIn),
        bind(ch("+"), ctrl, Action::ZoomIn),
        bind(ch("-"), ctrl, Action::ZoomOut),
        bind(ch("0"), ctrl, Action::ZoomReset),
        // Tabs.
        bind(ch("t"), ctrl, Action::NewTab),
        bind(ch("w"), ctrl, Action::CloseTab),
        bind(named(NamedKey::Tab), ctrl, Action::NextTab),
        bind(named(NamedKey::Tab), cs, Action::PrevTab),
        // Scrollback.
        bind(named(NamedKey::PageUp), shift, Action::ScrollPageUp),
        bind(named(NamedKey::PageDown), shift, Action::ScrollPageDown),
        bind(named(NamedKey::Home), shift, Action::ScrollToTop),
        bind(named(NamedKey::End), shift, Action::ScrollToBottom),
        // Prompt navigation.
        bind(named(NamedKey::ArrowUp), cs, Action::PreviousPrompt),
        bind(named(NamedKey::ArrowDown), cs, Action::NextPrompt),
        // Fullscreen (Alt+Enter on Windows/Linux).
        bind(named(NamedKey::Enter), alt, Action::ToggleFullscreen),
        // Mark mode (vi-style selection navigation).
        bind(ch("m"), cs, Action::EnterMarkMode),
        // Pane splitting and navigation (Ghostty-style).
        bind(ch("o"), cs, Action::SplitRight),
        bind(ch("e"), cs, Action::SplitDown),
        bind(named(NamedKey::ArrowUp), ca, Action::FocusPaneUp),
        bind(named(NamedKey::ArrowDown), ca, Action::FocusPaneDown),
        bind(named(NamedKey::ArrowLeft), ca, Action::FocusPaneLeft),
        bind(named(NamedKey::ArrowRight), ca, Action::FocusPaneRight),
        bind(ch("["), ca, Action::PrevPane),
        bind(ch("]"), ca, Action::NextPane),
        bind(ch("w"), cs, Action::ClosePane),
        // Pane resize (Ctrl+Alt+Shift+Arrow for resize, Ctrl+Shift+= for equalize).
        bind(named(NamedKey::ArrowUp), ca | shift, Action::ResizePaneUp),
        bind(
            named(NamedKey::ArrowDown),
            ca | shift,
            Action::ResizePaneDown,
        ),
        bind(
            named(NamedKey::ArrowLeft),
            ca | shift,
            Action::ResizePaneLeft,
        ),
        bind(
            named(NamedKey::ArrowRight),
            ca | shift,
            Action::ResizePaneRight,
        ),
        bind(ch("="), cs, Action::EqualizePanes),
        // Pane zoom toggle.
        bind(ch("z"), cs, Action::ToggleZoom),
        // Smart copy/paste (Ctrl+C/V without Shift) — must come AFTER
        // Ctrl+Shift variants so those match first.
        bind(ch("c"), ctrl, Action::SmartCopy),
        bind(ch("v"), ctrl, Action::SmartPaste),
    ];

    // macOS: Cmd-based bindings matching native conventions.
    #[cfg(target_os = "macos")]
    {
        let cmd = Modifiers::SUPER;
        let cmd_shift = cmd | shift;
        bindings.extend([
            bind(ch("c"), cmd, Action::Copy),
            bind(ch("v"), cmd, Action::Paste),
            bind(ch("t"), cmd, Action::NewTab),
            bind(ch("w"), cmd, Action::CloseTab),
            bind(ch("n"), cmd, Action::MoveTabToNewWindow),
            bind(ch("="), cmd, Action::ZoomIn),
            bind(ch("+"), cmd, Action::ZoomIn),
            bind(ch("-"), cmd, Action::ZoomOut),
            bind(ch("0"), cmd, Action::ZoomReset),
            bind(ch("f"), cmd, Action::OpenSearch),
            bind(named(NamedKey::ArrowUp), cmd, Action::ScrollToTop),
            bind(named(NamedKey::ArrowDown), cmd, Action::ScrollToBottom),
            // Cmd+Ctrl+F for macOS native fullscreen (green button).
            bind(ch("f"), cmd | ctrl, Action::ToggleFullscreen),
            bind(named(NamedKey::Tab), cmd, Action::NextTab),
            bind(named(NamedKey::Tab), cmd_shift, Action::PrevTab),
        ]);
    }

    bindings
}
