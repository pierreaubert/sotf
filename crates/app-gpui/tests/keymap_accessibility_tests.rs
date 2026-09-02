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
    assert!(appearance.contains("focus_language_relative"));
    assert!(appearance.contains("\"enter\" | \"space\""));
    assert!(appearance.contains("dev_track_with_state"));
    assert!(appearance.contains("settings.language.{}"));
}

#[test]
fn settings_tabs_publish_compact_keyboard_accessibility_contracts() {
    let settings_shell = include_str!("../components/mod.rs");

    assert!(settings_shell.contains("compact_tab_header"));
    assert!(settings_shell.contains("cx.register_accessible(AccessibilityNode"));
    assert!(settings_shell.contains("AriaState::Pressed(true)"));
    assert!(settings_shell.contains(".track_focus_element(&focus_handle)"));
    assert!(settings_shell.contains("focus_settings_tab_relative"));
    assert!(settings_shell.contains("\"enter\" | \"space\""));
    assert!(settings_shell.contains("themed_tooltip(label, &tooltip_theme, cx)"));
}
