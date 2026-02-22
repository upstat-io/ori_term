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

    vec![
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
        // Smart copy/paste (Ctrl+C/V without Shift) — must come AFTER
        // Ctrl+Shift variants so those match first.
        bind(ch("c"), ctrl, Action::SmartCopy),
        bind(ch("v"), ctrl, Action::SmartPaste),
    ]
}
