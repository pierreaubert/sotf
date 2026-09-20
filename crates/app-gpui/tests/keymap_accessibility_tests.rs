#[test]
fn keymap_preset_controls_publish_individual_keyboard_accessibility_contracts() {
    let keybindings = include_str!("../components/settings/keybindings.rs");

    assert!(!keybindings.contains("ButtonSet::new(\"keymap-preset\")"));
    assert!(keybindings.contains("cx.register_accessible(AccessibilityNode"));
    assert!(keybindings.contains("AriaState::Pressed(true)"));
    assert!(keybindings.contains(".selected(selected)"));
    assert!(keybindings.contains(".track_focus_element(&focus_handle)"));
    assert!(keybindings.contains("focus_keymap_preset_relative"));
    assert!(keybindings.contains("\"enter\" | \"space\""));
    assert!(keybindings.contains("dev_track_with_state"));
    assert!(keybindings.contains("settings.keymap-preset.{}"));
}

#[test]
fn language_controls_publish_individual_keyboard_accessibility_contracts() {
    let appearance = include_str!("../components/settings/appearance/theme.rs");

    assert!(!appearance.contains("ButtonSet::new(\"language-select\")"));
    assert!(appearance.contains("cx.register_accessible(AccessibilityNode"));
    assert!(appearance.contains("AriaState::Pressed(true)"));
    assert!(appearance.contains(".selected(selected)"));
    assert!(appearance.contains(".track_focus_element(&focus_handle)"));
    assert!(appearance.contains("focus_settings_choice_relative"));
    assert!(appearance.contains("\"enter\" | \"space\""));
    assert!(appearance.contains("dev_track_with_state"));
    assert!(appearance.contains("settings.language.{}"));
}

#[test]
fn preferences_publish_labeled_compact_navigation_contracts() {
    let settings_shell = include_str!("../components/settings/navigation.rs");

    // The shared Select owns keyboard/focus handling; the app must supply a
    // visible label, accessible name, translated options and selected state.
    assert!(settings_shell.contains("Select::new(\"preferences-category\")"));
    assert!(settings_shell.contains(".label(text.category)"));
    assert!(settings_shell.contains(".aria_label(text.category)"));
    assert!(settings_shell.contains("text.categories[*group as usize]"));
    assert!(settings_shell.contains(".selected((category as usize).to_string())"));
    assert!(settings_shell.contains("state.app.ui_state.active_settings_tab = *tab"));
    assert!(settings_shell.contains("state.app.settings.navigation.category_open = false"));
    assert!(settings_shell.contains(".aria_label(text.search)"));
}
