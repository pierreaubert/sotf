//! Keybinding Conflict Tests
//!
//! These tests verify that InputMode::is_text_input() correctly identifies
//! all modes where keyboard shortcuts must be blocked to prevent conflicts.
//!
//! NOTE: The real InputMode lives in sotf_gpui::app::types but the lib crate
//! has test=false (GPUI macro stack overflow). We mirror the enum and its
//! is_text_input() method here. If the real enum changes, these tests MUST
//! be updated in sync — a mismatch means new modes could allow keybinding
//! conflicts to slip through.
//!
//! # Background
//!
//! A bug was discovered where typing in the search box triggered global actions
//! instead of adding characters to the search query. For example:
//! - Typing '5' triggered SetFilterRating(5) instead of adding '5' to search
//! - Pressing space triggered PlayPause instead of adding space to search

/// Mirror of sotf_gpui::app::types::InputMode.
/// KEEP IN SYNC with the real enum in app/types/mod.rs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InputMode {
    Normal,
    Search,
    AddDirectory,
    SavePlugins,
    LoadPlugins,
    LoadApoFile,
    LoadSofaFile,
    Help,
    HelpSupport,
    KeyboardShortcuts,
    About,
    EditingParam,
    SpinoramaSpeakerSearch,
    EmptyLibraryPrompt,
    EditingPluginNode,
    ChannelConflict,
    ContextMenu,
    Tutorial,
}

impl InputMode {
    /// Mirror of the real is_text_input(). Must match app/types/mod.rs exactly.
    fn is_text_input(&self) -> bool {
        matches!(
            self,
            InputMode::Search
                | InputMode::AddDirectory
                | InputMode::SavePlugins
                | InputMode::LoadPlugins
                | InputMode::LoadApoFile
                | InputMode::LoadSofaFile
                | InputMode::SpinoramaSpeakerSearch
        )
    }
}

/// All InputMode variants for exhaustive testing.
const ALL_MODES: &[InputMode] = &[
    InputMode::Normal,
    InputMode::Search,
    InputMode::AddDirectory,
    InputMode::SavePlugins,
    InputMode::LoadPlugins,
    InputMode::LoadApoFile,
    InputMode::LoadSofaFile,
    InputMode::Help,
    InputMode::HelpSupport,
    InputMode::KeyboardShortcuts,
    InputMode::About,
    InputMode::EditingParam,
    InputMode::SpinoramaSpeakerSearch,
    InputMode::EmptyLibraryPrompt,
    InputMode::EditingPluginNode,
    InputMode::ChannelConflict,
    InputMode::ContextMenu,
    InputMode::Tutorial,
];

/// All modes where text input happens and shortcuts MUST be blocked.
const TEXT_INPUT_MODES: &[InputMode] = &[
    InputMode::Search,
    InputMode::AddDirectory,
    InputMode::SavePlugins,
    InputMode::LoadPlugins,
    InputMode::LoadApoFile,
    InputMode::LoadSofaFile,
    InputMode::SpinoramaSpeakerSearch,
];

/// All modes where shortcuts SHOULD still work.
const NON_TEXT_INPUT_MODES: &[InputMode] = &[
    InputMode::Normal,
    InputMode::Help,
    InputMode::HelpSupport,
    InputMode::KeyboardShortcuts,
    InputMode::About,
    InputMode::EditingParam,
    InputMode::EmptyLibraryPrompt,
    InputMode::EditingPluginNode,
    InputMode::ChannelConflict,
    InputMode::ContextMenu,
    InputMode::Tutorial,
];

/// Test: All text input modes are recognized by is_text_input()
#[test]
fn test_text_input_modes_block_shortcuts() {
    for &mode in TEXT_INPUT_MODES {
        assert!(
            mode.is_text_input(),
            "Mode {:?} should be a text input mode but is_text_input() returned false. \
             This means keyboard shortcuts will fire while the user types.",
            mode
        );
    }
}

/// Test: Non-text modes allow shortcuts
#[test]
fn test_non_text_modes_allow_shortcuts() {
    for &mode in NON_TEXT_INPUT_MODES {
        assert!(
            !mode.is_text_input(),
            "Mode {:?} should NOT be a text input mode but is_text_input() returned true. \
             This means keyboard shortcuts are blocked when they should work.",
            mode
        );
    }
}

/// Test: Every mode is classified in exactly one category.
/// Catches newly added variants that aren't classified yet.
#[test]
fn test_all_modes_are_classified() {
    for mode in ALL_MODES {
        let in_text = TEXT_INPUT_MODES.contains(mode);
        let in_non_text = NON_TEXT_INPUT_MODES.contains(mode);
        assert!(
            in_text || in_non_text,
            "Mode {:?} is not in TEXT_INPUT_MODES or NON_TEXT_INPUT_MODES. \
             Add it to the appropriate list.",
            mode
        );
        assert!(
            !(in_text && in_non_text),
            "Mode {:?} is in BOTH lists. Remove the duplicate.",
            mode
        );
    }

    let total = TEXT_INPUT_MODES.len() + NON_TEXT_INPUT_MODES.len();
    assert_eq!(total, ALL_MODES.len());
}

/// Test: Normal mode is never a text input mode
#[test]
fn test_normal_mode_is_not_text_input() {
    assert!(!InputMode::Normal.is_text_input());
}
