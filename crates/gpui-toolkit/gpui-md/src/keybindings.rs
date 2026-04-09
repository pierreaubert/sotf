use gpui::KeyBinding;
use gpui_keybinding::{
    DocumentedKeybinding, KeybindingCategory, KeybindingProvider, KeymapPreset,
};

use crate::actions;

/// Keybinding provider for the markdown editor.
pub struct MdKeybindingProvider;

impl KeybindingProvider for MdKeybindingProvider {
    fn bindings(&self, preset: KeymapPreset) -> Vec<KeyBinding> {
        let mut bindings = common_bindings();
        bindings.extend(preset_bindings(preset));
        bindings
    }

    fn documented_bindings(&self, preset: KeymapPreset) -> Vec<DocumentedKeybinding> {
        let mut bindings = vec![
            DocumentedKeybinding::new("Ctrl+N", "New file", KeybindingCategory::FileOps),
            DocumentedKeybinding::new("Ctrl+O", "Open file", KeybindingCategory::FileOps),
            DocumentedKeybinding::new("Ctrl+S", "Save file", KeybindingCategory::FileOps),
            DocumentedKeybinding::new("Ctrl+Shift+S", "Save as", KeybindingCategory::FileOps),
            DocumentedKeybinding::new("Ctrl+Z", "Undo", KeybindingCategory::Editing),
            DocumentedKeybinding::new("Ctrl+Shift+Z", "Redo", KeybindingCategory::Editing),
            DocumentedKeybinding::new("Ctrl+B", "Toggle bold", KeybindingCategory::Formatting),
            DocumentedKeybinding::new("Ctrl+I", "Toggle italic", KeybindingCategory::Formatting),
            DocumentedKeybinding::new("Ctrl+E", "Toggle code", KeybindingCategory::Formatting),
            DocumentedKeybinding::new(
                "Ctrl+Shift+X",
                "Toggle strikethrough",
                KeybindingCategory::Formatting,
            ),
            DocumentedKeybinding::new("Ctrl+K", "Insert link", KeybindingCategory::Formatting),
            DocumentedKeybinding::new("Ctrl+F", "Find", KeybindingCategory::Search),
            DocumentedKeybinding::new(
                "Ctrl+Shift+V",
                "Toggle preview",
                KeybindingCategory::View,
            ),
        ];

        if matches!(preset, KeymapPreset::Emacs) {
            bindings.extend([
                DocumentedKeybinding::new("C-g", "Abort", KeybindingCategory::Editing),
                DocumentedKeybinding::new("C-l", "Recenter", KeybindingCategory::View),
                DocumentedKeybinding::new("C-u", "Universal argument", KeybindingCategory::Editing),
                DocumentedKeybinding::new("C-s", "Incremental search forward", KeybindingCategory::Search),
                DocumentedKeybinding::new("C-r", "Incremental search backward", KeybindingCategory::Search),
                DocumentedKeybinding::new("M-x", "Command palette", KeybindingCategory::Editing),
                DocumentedKeybinding::new("M-u", "Upcase word", KeybindingCategory::Editing),
                DocumentedKeybinding::new("M-l", "Downcase word", KeybindingCategory::Editing),
                DocumentedKeybinding::new("M-b", "Word left", KeybindingCategory::Navigation),
                DocumentedKeybinding::new("M-f", "Word right", KeybindingCategory::Navigation),
                DocumentedKeybinding::new("M-v", "Page up", KeybindingCategory::Navigation),
                DocumentedKeybinding::new("C-v", "Page down", KeybindingCategory::Navigation),
                DocumentedKeybinding::new("C-x C-c", "Quit", KeybindingCategory::FileOps),
                DocumentedKeybinding::new("C-x C-x", "Exchange point and mark", KeybindingCategory::Editing),
            ]);
        }

        bindings
    }
}

/// Bindings shared across all presets.
fn common_bindings() -> Vec<KeyBinding> {
    vec![
        // File operations
        KeyBinding::new("secondary-n", actions::NewFile, None),
        KeyBinding::new("secondary-o", actions::OpenFile, None),
        KeyBinding::new("secondary-s", actions::SaveFile, None),
        KeyBinding::new("secondary-shift-s", actions::SaveFileAs, None),
        // Edit operations
        KeyBinding::new("secondary-z", actions::Undo, None),
        KeyBinding::new("secondary-shift-z", actions::Redo, None),
        KeyBinding::new("secondary-x", actions::Cut, None),
        KeyBinding::new("secondary-c", actions::Copy, None),
        KeyBinding::new("secondary-v", actions::Paste, None),
        KeyBinding::new("secondary-a", actions::SelectAll, None),
        // Formatting
        KeyBinding::new("secondary-b", actions::ToggleBold, None),
        KeyBinding::new("secondary-i", actions::ToggleItalic, None),
        KeyBinding::new("secondary-e", actions::ToggleCode, None),
        KeyBinding::new("secondary-shift-x", actions::ToggleStrikethrough, None),
        KeyBinding::new("secondary-k", actions::InsertLink, None),
        // View
        KeyBinding::new("secondary-shift-v", actions::TogglePreview, None),
        // Search
        KeyBinding::new("secondary-f", actions::Find, None),
        KeyBinding::new("secondary-h", actions::FindReplace, None),
    ]
}

/// Preset-specific bindings (navigation overrides).
fn preset_bindings(preset: KeymapPreset) -> Vec<KeyBinding> {
    match preset {
        KeymapPreset::Vim => vec![
            // Vim-specific: heading insertion via leader key
            KeyBinding::new("g 1", actions::InsertHeading1, Some("EditorPane")),
            KeyBinding::new("g 2", actions::InsertHeading2, Some("EditorPane")),
            KeyBinding::new("g 3", actions::InsertHeading3, Some("EditorPane")),
        ],
        KeymapPreset::Emacs => vec![
            // Emacs chord sequences (C-x prefix)
            KeyBinding::new("ctrl-x ctrl-s", actions::SaveFile, None),
            KeyBinding::new("ctrl-x ctrl-f", actions::OpenFile, None),
            KeyBinding::new("ctrl-x ctrl-x", actions::ExchangePointAndMark, None),
            KeyBinding::new("ctrl-x ctrl-c", gpui_ui_kit::app::miniapp::Quit, None),
            // Emacs single-key commands
            KeyBinding::new("ctrl-g", actions::Abort, None),
            KeyBinding::new("ctrl-l", actions::Recenter, None),
            KeyBinding::new("ctrl-u", actions::UniversalArgument, None),
            KeyBinding::new("ctrl-s", actions::IsearchForward, None),
            KeyBinding::new("ctrl-r", actions::IsearchBackward, None),
            KeyBinding::new("ctrl-v", actions::PageDown, None),
            // Meta (alt) bindings
            KeyBinding::new("alt-x", actions::CommandPalette, None),
            KeyBinding::new("alt-u", actions::UpcaseWord, None),
            KeyBinding::new("alt-l", actions::DowncaseWord, None),
            KeyBinding::new("alt-b", actions::WordLeft, None),
            KeyBinding::new("alt-f", actions::WordRight, None),
            KeyBinding::new("alt-v", actions::PageUp, None),
        ],
        _ => vec![],
    }
}
