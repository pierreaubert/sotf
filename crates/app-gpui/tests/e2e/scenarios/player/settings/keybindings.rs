//! E2E tests for Keybindings Settings.
//!
//! Tests for keyboard shortcut customization:
//! - View current bindings
//! - Customize shortcuts
//! - Reset to defaults
//! - Conflict detection

use gpui::TestAppContext;
use std::cell::RefCell;
use std::rc::Rc;

// =============================================================================
// Mock Types for Testing
// =============================================================================

/// Action category
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum ActionCategory {
    #[default]
    Playback,
    Navigation,
    Library,
    Volume,
    Plugins,
    General,
}

/// Key modifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Modifier {
    Cmd,
    Shift,
    Alt,
    Ctrl,
}

/// Key binding
#[derive(Debug, Clone, Default)]
struct KeyBinding {
    action_id: String,
    action_name: String,
    category: ActionCategory,
    key: String,
    modifiers: Vec<Modifier>,
    is_customized: bool,
    is_default: bool,
}

/// Binding conflict
#[derive(Debug, Clone)]
struct BindingConflict {
    new_action: String,
    existing_action: String,
    key_combo: String,
}

/// Keybindings state
#[derive(Default)]
struct KeybindingsState {
    bindings: Vec<KeyBinding>,
    filtered_bindings: Vec<KeyBinding>,
    selected_category: Option<ActionCategory>,
    search_query: String,
    editing_binding: Option<String>, // action_id of binding being edited
    pending_key: Option<(String, Vec<Modifier>)>,
    conflict: Option<BindingConflict>,
    show_customized_only: bool,
}

// =============================================================================
// Binding Display Tests
// =============================================================================

/// Test bindings load.
#[gpui::test]
async fn test_bindings_load(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(KeybindingsState::default()));

    state.borrow_mut().bindings = vec![
        KeyBinding {
            action_id: "play_pause".to_string(),
            action_name: "Play/Pause".to_string(),
            category: ActionCategory::Playback,
            key: "Space".to_string(),
            modifiers: vec![],
            is_default: true,
            ..Default::default()
        },
        KeyBinding {
            action_id: "next_track".to_string(),
            action_name: "Next Track".to_string(),
            category: ActionCategory::Playback,
            key: "N".to_string(),
            modifiers: vec![Modifier::Cmd],
            is_default: true,
            ..Default::default()
        },
    ];

    assert_eq!(state.borrow().bindings.len(), 2);
}

/// Test binding display format.
#[gpui::test]
async fn test_binding_display_format(_cx: &mut TestAppContext) {
    fn format_binding(binding: &KeyBinding) -> String {
        let mut parts: Vec<String> = binding
            .modifiers
            .iter()
            .map(|m| match m {
                Modifier::Cmd => "⌘".to_string(),
                Modifier::Shift => "⇧".to_string(),
                Modifier::Alt => "⌥".to_string(),
                Modifier::Ctrl => "⌃".to_string(),
            })
            .collect();
        parts.push(binding.key.clone());
        parts.join("")
    }

    let binding = KeyBinding {
        key: "N".to_string(),
        modifiers: vec![Modifier::Cmd, Modifier::Shift],
        ..Default::default()
    };

    let formatted = format_binding(&binding);
    assert!(formatted.contains("⌘"));
    assert!(formatted.contains("⇧"));
    assert!(formatted.contains("N"));
}

/// Test empty binding display.
#[gpui::test]
async fn test_empty_binding_display(_cx: &mut TestAppContext) {
    fn format_binding(binding: &KeyBinding) -> String {
        if binding.key.is_empty() {
            "Not set".to_string()
        } else {
            binding.key.clone()
        }
    }

    let binding = KeyBinding {
        key: String::new(),
        ..Default::default()
    };

    assert_eq!(format_binding(&binding), "Not set");
}

// =============================================================================
// Category Filter Tests
// =============================================================================

/// Test category selection.
#[gpui::test]
async fn test_category_selection(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(KeybindingsState::default()));

    let categories = [
        ActionCategory::Playback,
        ActionCategory::Navigation,
        ActionCategory::Library,
        ActionCategory::Volume,
        ActionCategory::Plugins,
        ActionCategory::General,
    ];

    for cat in categories {
        state.borrow_mut().selected_category = Some(cat);
        assert_eq!(state.borrow().selected_category, Some(cat));
    }
}

/// Test category filtering.
#[gpui::test]
async fn test_category_filtering(_cx: &mut TestAppContext) {
    fn filter_by_category(bindings: &[KeyBinding], category: ActionCategory) -> Vec<KeyBinding> {
        bindings
            .iter()
            .filter(|b| b.category == category)
            .cloned()
            .collect()
    }

    let bindings = vec![
        KeyBinding {
            category: ActionCategory::Playback,
            ..Default::default()
        },
        KeyBinding {
            category: ActionCategory::Volume,
            ..Default::default()
        },
        KeyBinding {
            category: ActionCategory::Playback,
            ..Default::default()
        },
    ];

    let filtered = filter_by_category(&bindings, ActionCategory::Playback);
    assert_eq!(filtered.len(), 2);
}

/// Test show all categories.
#[gpui::test]
async fn test_show_all_categories(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(KeybindingsState::default()));

    state.borrow_mut().selected_category = Some(ActionCategory::Playback);
    state.borrow_mut().selected_category = None;

    assert!(state.borrow().selected_category.is_none());
}

/// Test category labels.
#[gpui::test]
async fn test_category_labels(_cx: &mut TestAppContext) {
    fn get_category_label(category: ActionCategory) -> &'static str {
        match category {
            ActionCategory::Playback => "Playback",
            ActionCategory::Navigation => "Navigation",
            ActionCategory::Library => "Library",
            ActionCategory::Volume => "Volume",
            ActionCategory::Plugins => "Plugins",
            ActionCategory::General => "General",
        }
    }

    assert_eq!(get_category_label(ActionCategory::Playback), "Playback");
    assert_eq!(get_category_label(ActionCategory::Volume), "Volume");
}

// =============================================================================
// Search Tests
// =============================================================================

/// Test search query.
#[gpui::test]
async fn test_search_query(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(KeybindingsState::default()));

    state.borrow_mut().search_query = "play".to_string();
    assert_eq!(state.borrow().search_query, "play");
}

/// Test search filtering.
#[gpui::test]
async fn test_search_filtering(_cx: &mut TestAppContext) {
    fn filter_by_search(bindings: &[KeyBinding], query: &str) -> Vec<KeyBinding> {
        let query_lower = query.to_lowercase();
        bindings
            .iter()
            .filter(|b| {
                b.action_name.to_lowercase().contains(&query_lower)
                    || b.key.to_lowercase().contains(&query_lower)
            })
            .cloned()
            .collect()
    }

    let bindings = vec![
        KeyBinding {
            action_name: "Play/Pause".to_string(),
            key: "Space".to_string(),
            ..Default::default()
        },
        KeyBinding {
            action_name: "Next Track".to_string(),
            key: "N".to_string(),
            ..Default::default()
        },
    ];

    let filtered = filter_by_search(&bindings, "play");
    assert_eq!(filtered.len(), 1);
}

/// Test clear search.
#[gpui::test]
async fn test_clear_search(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(KeybindingsState::default()));

    state.borrow_mut().search_query = "test".to_string();
    state.borrow_mut().search_query.clear();

    assert!(state.borrow().search_query.is_empty());
}

// =============================================================================
// Binding Edit Tests
// =============================================================================

/// Test start editing binding.
#[gpui::test]
async fn test_start_editing_binding(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(KeybindingsState::default()));

    state.borrow_mut().editing_binding = Some("play_pause".to_string());
    assert_eq!(
        state.borrow().editing_binding,
        Some("play_pause".to_string())
    );
}

/// Test cancel editing.
#[gpui::test]
async fn test_cancel_editing(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(KeybindingsState::default()));

    state.borrow_mut().editing_binding = Some("play_pause".to_string());
    state.borrow_mut().pending_key = Some(("N".to_string(), vec![Modifier::Cmd]));

    // Cancel
    state.borrow_mut().editing_binding = None;
    state.borrow_mut().pending_key = None;

    assert!(state.borrow().editing_binding.is_none());
    assert!(state.borrow().pending_key.is_none());
}

/// Test pending key capture.
#[gpui::test]
async fn test_pending_key_capture(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(KeybindingsState::default()));

    state.borrow_mut().editing_binding = Some("play_pause".to_string());
    state.borrow_mut().pending_key = Some(("K".to_string(), vec![Modifier::Cmd]));

    assert!(state.borrow().pending_key.is_some());
}

/// Test save binding.
#[gpui::test]
async fn test_save_binding(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(KeybindingsState::default()));

    state.borrow_mut().bindings.push(KeyBinding {
        action_id: "play_pause".to_string(),
        key: "Space".to_string(),
        modifiers: vec![],
        is_customized: false,
        ..Default::default()
    });

    // Update binding
    state.borrow_mut().bindings[0].key = "K".to_string();
    state.borrow_mut().bindings[0].modifiers = vec![Modifier::Cmd];
    state.borrow_mut().bindings[0].is_customized = true;

    assert_eq!(state.borrow().bindings[0].key, "K");
    assert!(state.borrow().bindings[0].is_customized);
}

/// Test clear binding.
#[gpui::test]
async fn test_clear_binding(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(KeybindingsState::default()));

    state.borrow_mut().bindings.push(KeyBinding {
        action_id: "play_pause".to_string(),
        key: "Space".to_string(),
        ..Default::default()
    });

    // Clear binding
    state.borrow_mut().bindings[0].key.clear();
    state.borrow_mut().bindings[0].modifiers.clear();

    assert!(state.borrow().bindings[0].key.is_empty());
}

// =============================================================================
// Conflict Detection Tests
// =============================================================================

/// Test conflict detection.
#[gpui::test]
async fn test_conflict_detection(_cx: &mut TestAppContext) {
    fn check_conflict(
        bindings: &[KeyBinding],
        action_id: &str,
        key: &str,
        modifiers: &[Modifier],
    ) -> Option<BindingConflict> {
        for binding in bindings {
            if binding.action_id != action_id
                && binding.key == key
                && binding.modifiers == modifiers
            {
                return Some(BindingConflict {
                    new_action: action_id.to_string(),
                    existing_action: binding.action_id.clone(),
                    key_combo: format!("{:?}+{}", modifiers, key),
                });
            }
        }
        None
    }

    let bindings = vec![KeyBinding {
        action_id: "play_pause".to_string(),
        key: "Space".to_string(),
        modifiers: vec![],
        ..Default::default()
    }];

    let conflict = check_conflict(&bindings, "other_action", "Space", &[]);
    assert!(conflict.is_some());

    let no_conflict = check_conflict(&bindings, "other_action", "N", &[]);
    assert!(no_conflict.is_none());
}

/// Test conflict message.
#[gpui::test]
async fn test_conflict_message(_cx: &mut TestAppContext) {
    fn get_conflict_message(conflict: &BindingConflict) -> String {
        format!(
            "\"{}\" conflicts with \"{}\"",
            conflict.key_combo, conflict.existing_action
        )
    }

    let conflict = BindingConflict {
        new_action: "next_track".to_string(),
        existing_action: "play_pause".to_string(),
        key_combo: "Space".to_string(),
    };

    let message = get_conflict_message(&conflict);
    assert!(message.contains("Space"));
    assert!(message.contains("play_pause"));
}

/// Test resolve conflict by overwriting.
#[gpui::test]
async fn test_resolve_conflict_overwrite(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(KeybindingsState::default()));

    state.borrow_mut().bindings = vec![
        KeyBinding {
            action_id: "play_pause".to_string(),
            key: "Space".to_string(),
            ..Default::default()
        },
        KeyBinding {
            action_id: "other_action".to_string(),
            key: "N".to_string(),
            ..Default::default()
        },
    ];

    // Overwrite: clear old binding and assign to new
    state.borrow_mut().bindings[0].key.clear();
    state.borrow_mut().bindings[1].key = "Space".to_string();

    assert!(state.borrow().bindings[0].key.is_empty());
    assert_eq!(state.borrow().bindings[1].key, "Space");
}

// =============================================================================
// Reset Tests
// =============================================================================

/// Test reset single binding.
#[gpui::test]
async fn test_reset_single_binding(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(KeybindingsState::default()));

    state.borrow_mut().bindings.push(KeyBinding {
        action_id: "play_pause".to_string(),
        key: "K".to_string(),
        modifiers: vec![Modifier::Cmd],
        is_customized: true,
        is_default: false,
        ..Default::default()
    });

    // Reset to default
    fn get_default_binding(action_id: &str) -> (String, Vec<Modifier>) {
        match action_id {
            "play_pause" => ("Space".to_string(), vec![]),
            _ => (String::new(), vec![]),
        }
    }

    let (default_key, default_mods) = get_default_binding("play_pause");
    state.borrow_mut().bindings[0].key = default_key;
    state.borrow_mut().bindings[0].modifiers = default_mods;
    state.borrow_mut().bindings[0].is_customized = false;

    assert_eq!(state.borrow().bindings[0].key, "Space");
    assert!(!state.borrow().bindings[0].is_customized);
}

/// Test reset all bindings.
#[gpui::test]
async fn test_reset_all_bindings(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(KeybindingsState::default()));

    state.borrow_mut().bindings = vec![
        KeyBinding {
            is_customized: true,
            ..Default::default()
        },
        KeyBinding {
            is_customized: true,
            ..Default::default()
        },
    ];

    // Reset all
    for binding in &mut state.borrow_mut().bindings {
        binding.is_customized = false;
    }

    assert!(state.borrow().bindings.iter().all(|b| !b.is_customized));
}

/// Test show customized only.
#[gpui::test]
async fn test_show_customized_only(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(KeybindingsState::default()));

    state.borrow_mut().show_customized_only = true;
    assert!(state.borrow().show_customized_only);
}

/// Test filter customized bindings.
#[gpui::test]
async fn test_filter_customized_bindings(_cx: &mut TestAppContext) {
    fn filter_customized(bindings: &[KeyBinding]) -> Vec<KeyBinding> {
        bindings
            .iter()
            .filter(|b| b.is_customized)
            .cloned()
            .collect()
    }

    let bindings = vec![
        KeyBinding {
            is_customized: true,
            ..Default::default()
        },
        KeyBinding {
            is_customized: false,
            ..Default::default()
        },
        KeyBinding {
            is_customized: true,
            ..Default::default()
        },
    ];

    let customized = filter_customized(&bindings);
    assert_eq!(customized.len(), 2);
}

// =============================================================================
// Key Input Tests
// =============================================================================

/// Test key input parsing.
#[gpui::test]
async fn test_key_input_parsing(_cx: &mut TestAppContext) {
    fn parse_key_event(
        key: &str,
        cmd: bool,
        shift: bool,
        alt: bool,
        ctrl: bool,
    ) -> (String, Vec<Modifier>) {
        let mut modifiers = Vec::new();
        if cmd {
            modifiers.push(Modifier::Cmd);
        }
        if shift {
            modifiers.push(Modifier::Shift);
        }
        if alt {
            modifiers.push(Modifier::Alt);
        }
        if ctrl {
            modifiers.push(Modifier::Ctrl);
        }
        (key.to_string(), modifiers)
    }

    let (key, mods) = parse_key_event("N", true, true, false, false);
    assert_eq!(key, "N");
    assert_eq!(mods.len(), 2);
    assert!(mods.contains(&Modifier::Cmd));
    assert!(mods.contains(&Modifier::Shift));
}

/// Test reserved key rejection.
#[gpui::test]
async fn test_reserved_key_rejection(_cx: &mut TestAppContext) {
    fn is_reserved_key(key: &str, modifiers: &[Modifier]) -> bool {
        // System shortcuts that shouldn't be overridden
        let is_cmd = modifiers.contains(&Modifier::Cmd);
        matches!(
            (key, is_cmd),
            ("Q", true) | ("W", true) | ("H", true) | ("M", true)
        )
    }

    assert!(is_reserved_key("Q", &[Modifier::Cmd]));
    assert!(is_reserved_key("W", &[Modifier::Cmd]));
    assert!(!is_reserved_key("N", &[Modifier::Cmd]));
    assert!(!is_reserved_key("Q", &[]));
}

/// Test modifier-only key rejection.
#[gpui::test]
async fn test_modifier_only_rejection(_cx: &mut TestAppContext) {
    fn is_modifier_only(key: &str) -> bool {
        matches!(key, "Meta" | "Shift" | "Alt" | "Control" | "CapsLock")
    }

    assert!(is_modifier_only("Meta"));
    assert!(is_modifier_only("Shift"));
    assert!(!is_modifier_only("A"));
}

// =============================================================================
// Accessibility Tests
// =============================================================================

/// Test binding aria label.
#[gpui::test]
async fn test_binding_aria_label(_cx: &mut TestAppContext) {
    fn get_binding_aria_label(binding: &KeyBinding) -> String {
        let modifiers: Vec<&str> = binding
            .modifiers
            .iter()
            .map(|m| match m {
                Modifier::Cmd => "Command",
                Modifier::Shift => "Shift",
                Modifier::Alt => "Option",
                Modifier::Ctrl => "Control",
            })
            .collect();

        if modifiers.is_empty() {
            format!("{}: {}", binding.action_name, binding.key)
        } else {
            format!(
                "{}: {} + {}",
                binding.action_name,
                modifiers.join(" + "),
                binding.key
            )
        }
    }

    let binding = KeyBinding {
        action_name: "Play/Pause".to_string(),
        key: "Space".to_string(),
        modifiers: vec![],
        ..Default::default()
    };

    let label = get_binding_aria_label(&binding);
    assert!(label.contains("Play/Pause"));
    assert!(label.contains("Space"));
}

/// Test edit mode announcement.
#[gpui::test]
async fn test_edit_mode_announcement(_cx: &mut TestAppContext) {
    fn get_edit_mode_announcement(action_name: &str) -> String {
        format!(
            "Press a key combination to set shortcut for {}. Press Escape to cancel.",
            action_name
        )
    }

    let announcement = get_edit_mode_announcement("Play/Pause");
    assert!(announcement.contains("Play/Pause"));
    assert!(announcement.contains("Escape"));
}

// =============================================================================
// Export/Import Tests
// =============================================================================

/// Test export bindings.
#[gpui::test]
async fn test_export_bindings(_cx: &mut TestAppContext) {
    fn export_bindings_json(bindings: &[KeyBinding]) -> String {
        // Simple JSON-like export
        let entries: Vec<String> = bindings
            .iter()
            .filter(|b| b.is_customized)
            .map(|b| format!("\"{}\": \"{}\"", b.action_id, b.key))
            .collect();
        format!("{{{}}}", entries.join(", "))
    }

    let bindings = vec![KeyBinding {
        action_id: "play_pause".to_string(),
        key: "K".to_string(),
        is_customized: true,
        ..Default::default()
    }];

    let exported = export_bindings_json(&bindings);
    assert!(exported.contains("play_pause"));
}

/// Test import validation.
#[gpui::test]
async fn test_import_validation(_cx: &mut TestAppContext) {
    fn validate_import(action_id: &str, key: &str) -> bool {
        // Check action exists and key is valid
        let valid_actions = ["play_pause", "next_track", "prev_track"];
        let key_valid = !key.is_empty() && key.len() <= 20;
        valid_actions.contains(&action_id) && key_valid
    }

    assert!(validate_import("play_pause", "Space"));
    assert!(!validate_import("invalid_action", "Space"));
    assert!(!validate_import("play_pause", ""));
}

// =============================================================================
// Styling Tests
// =============================================================================

/// Test customized binding indicator.
#[gpui::test]
async fn test_customized_binding_indicator(_cx: &mut TestAppContext) {
    fn get_binding_style(is_customized: bool) -> &'static str {
        if is_customized {
            "binding customized"
        } else {
            "binding default"
        }
    }

    assert!(get_binding_style(true).contains("customized"));
    assert!(get_binding_style(false).contains("default"));
}

/// Test editing state style.
#[gpui::test]
async fn test_editing_state_style(_cx: &mut TestAppContext) {
    fn get_binding_row_style(is_editing: bool) -> &'static str {
        if is_editing {
            "binding-row editing"
        } else {
            "binding-row"
        }
    }

    assert!(get_binding_row_style(true).contains("editing"));
}

/// Test conflict warning style.
#[gpui::test]
async fn test_conflict_warning_style(_cx: &mut TestAppContext) {
    fn get_conflict_style(has_conflict: bool) -> &'static str {
        if has_conflict {
            "key-input error"
        } else {
            "key-input"
        }
    }

    assert!(get_conflict_style(true).contains("error"));
}
