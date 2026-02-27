//! Keybinding system — map key+modifiers to actions.

mod defaults;
mod parse;

use winit::keyboard::{Key, NamedKey};

use crate::key_encoding::Modifiers;

pub(crate) use defaults::default_bindings;
pub(crate) use parse::merge_bindings;
#[allow(unused_imports, reason = "re-exported for future CLI use")]
pub(crate) use parse::parse_mods;
pub(crate) use parse::{parse_action, parse_key};

/// Identifies a key independent of modifiers.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum BindingKey {
    Named(NamedKey),
    /// Always stored lowercase.
    Character(String),
}

/// Action to execute when a keybinding matches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Action {
    Copy,
    Paste,
    /// Copy if selection exists, else fall through to PTY.
    SmartCopy,
    /// Paste from clipboard (Ctrl+V without Shift).
    SmartPaste,
    NewTab,
    CloseTab,
    NextTab,
    PrevTab,
    ZoomIn,
    ZoomOut,
    ZoomReset,
    ScrollPageUp,
    ScrollPageDown,
    ScrollToTop,
    ScrollToBottom,
    OpenSearch,
    ReloadConfig,
    /// Navigate to previous prompt mark (OSC 133;A).
    PreviousPrompt,
    /// Navigate to next prompt mark (OSC 133;A).
    NextPrompt,
    /// Duplicate the current tab (spawn new tab with same CWD).
    DuplicateTab,
    /// Move the current tab into a new window.
    MoveTabToNewWindow,
    /// Toggle fullscreen mode.
    ToggleFullscreen,
    /// Enter vi-style mark/selection mode.
    EnterMarkMode,
    /// Split the focused pane, placing the new pane to the right.
    SplitRight,
    /// Split the focused pane, placing the new pane below.
    SplitDown,
    /// Move focus to the pane above.
    FocusPaneUp,
    /// Move focus to the pane below.
    FocusPaneDown,
    /// Move focus to the pane to the left.
    FocusPaneLeft,
    /// Move focus to the pane to the right.
    FocusPaneRight,
    /// Cycle to the next pane (sequential order).
    NextPane,
    /// Cycle to the previous pane (sequential order).
    PrevPane,
    /// Close the focused pane (not the entire tab).
    ClosePane,
    /// Resize the focused pane upward (push nearest horizontal border up).
    ResizePaneUp,
    /// Resize the focused pane downward (push nearest horizontal border down).
    ResizePaneDown,
    /// Resize the focused pane leftward (push nearest vertical border left).
    ResizePaneLeft,
    /// Resize the focused pane rightward (push nearest vertical border right).
    ResizePaneRight,
    /// Reset all split ratios to equal (0.5).
    EqualizePanes,
    /// Toggle zoom on the focused pane.
    ToggleZoom,
    /// Toggle a floating pane: spawn one if none exist, or focus the topmost.
    ToggleFloatingPane,
    /// Move the focused pane between floating and tiled.
    ToggleFloatTile,
    /// Undo the last split tree mutation.
    UndoSplit,
    /// Redo the last undone split tree mutation.
    RedoSplit,
    /// Send literal bytes to the PTY.
    SendText(String),
    /// Explicitly unbinds a default binding.
    None,
}

impl Action {
    /// Canonical string for this action variant.
    ///
    /// Returns the same strings that [`parse_action`] accepts, ensuring
    /// round-trip consistency between `show-keys` output and TOML config.
    /// `SendText` is not covered — it carries dynamic payload.
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::Copy => "Copy",
            Self::Paste => "Paste",
            Self::SmartCopy => "SmartCopy",
            Self::SmartPaste => "SmartPaste",
            Self::NewTab => "NewTab",
            Self::CloseTab => "CloseTab",
            Self::NextTab => "NextTab",
            Self::PrevTab => "PrevTab",
            Self::ZoomIn => "ZoomIn",
            Self::ZoomOut => "ZoomOut",
            Self::ZoomReset => "ZoomReset",
            Self::ScrollPageUp => "ScrollPageUp",
            Self::ScrollPageDown => "ScrollPageDown",
            Self::ScrollToTop => "ScrollToTop",
            Self::ScrollToBottom => "ScrollToBottom",
            Self::OpenSearch => "OpenSearch",
            Self::ReloadConfig => "ReloadConfig",
            Self::PreviousPrompt => "PreviousPrompt",
            Self::NextPrompt => "NextPrompt",
            Self::DuplicateTab => "DuplicateTab",
            Self::MoveTabToNewWindow => "MoveTabToNewWindow",
            Self::ToggleFullscreen => "ToggleFullscreen",
            Self::EnterMarkMode => "EnterMarkMode",
            Self::SplitRight => "SplitRight",
            Self::SplitDown => "SplitDown",
            Self::FocusPaneUp => "FocusPaneUp",
            Self::FocusPaneDown => "FocusPaneDown",
            Self::FocusPaneLeft => "FocusPaneLeft",
            Self::FocusPaneRight => "FocusPaneRight",
            Self::NextPane => "NextPane",
            Self::PrevPane => "PrevPane",
            Self::ClosePane => "ClosePane",
            Self::ResizePaneUp => "ResizePaneUp",
            Self::ResizePaneDown => "ResizePaneDown",
            Self::ResizePaneLeft => "ResizePaneLeft",
            Self::ResizePaneRight => "ResizePaneRight",
            Self::EqualizePanes => "EqualizePanes",
            Self::ToggleZoom => "ToggleZoom",
            Self::ToggleFloatingPane => "ToggleFloatingPane",
            Self::ToggleFloatTile => "ToggleFloatTile",
            Self::UndoSplit => "UndoSplit",
            Self::RedoSplit => "RedoSplit",
            Self::SendText(_) => "SendText",
            Self::None => "None",
        }
    }
}

/// A resolved keybinding: key + modifiers -> action.
#[derive(Debug, Clone)]
pub(crate) struct KeyBinding {
    pub key: BindingKey,
    pub mods: Modifiers,
    pub action: Action,
}

/// Convert a winit `Key` to a `BindingKey`, normalizing characters to lowercase.
pub(crate) fn key_to_binding_key(key: &Key) -> Option<BindingKey> {
    match key {
        Key::Named(n) => Some(BindingKey::Named(*n)),
        Key::Character(s) => {
            let b = s.as_bytes();
            // Fast path: single ASCII byte avoids `to_lowercase` iterator + heap churn.
            let lower = if b.len() == 1 && b[0].is_ascii() {
                String::from(b[0].to_ascii_lowercase() as char)
            } else {
                let l = s.as_str().to_lowercase();
                if l.is_empty() {
                    return None;
                }
                l
            };
            Some(BindingKey::Character(lower))
        }
        _ => None,
    }
}

/// Find the first binding matching the given key and modifiers.
pub(crate) fn find_binding<'a>(
    bindings: &'a [KeyBinding],
    key: &BindingKey,
    mods: Modifiers,
) -> Option<&'a Action> {
    bindings.iter().find_map(|b| {
        if b.key == *key && b.mods == mods {
            Some(&b.action)
        } else {
            None
        }
    })
}

#[cfg(test)]
mod tests;
