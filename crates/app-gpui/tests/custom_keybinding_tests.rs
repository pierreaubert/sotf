use sotf_audio_player_gpui::keybindings::{
    CustomKeybinding, KeymapPreset, get_documented_keybindings, get_keybindings_with_overrides,
    keybinding_conflict, set_custom_keybinding,
};

fn action_name(preset: KeymapPreset, description: &str) -> &'static str {
    get_documented_keybindings(preset)
        .into_iter()
        .find(|binding| binding.description == description)
        .and_then(|binding| binding.action_name)
        .unwrap_or_else(|| panic!("missing executable action for {description}"))
}

fn key_spec(binding: &gpui::KeyBinding) -> String {
    binding
        .keystrokes()
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(" ")
}

#[test]
fn conflict_is_named_and_overwrite_changes_the_executable_keymap() {
    let preset = KeymapPreset::Default;
    let play_pause = action_name(preset, "Play/Pause");
    let next_track = action_name(preset, "Next track");

    let conflict = keybinding_conflict(preset, &[], next_track, "f8")
        .expect("F8 is a valid shortcut")
        .expect("F8 should already execute Play/Pause");
    assert_eq!(conflict.existing_action_name, play_pause);
    assert_eq!(conflict.key_spec, "f8");

    let mut overrides = Vec::new();
    set_custom_keybinding(&mut overrides, next_track, "f8");
    let runtime = get_keybindings_with_overrides(preset, &overrides);

    assert!(
        runtime
            .iter()
            .any(|binding| { binding.action().name() == next_track && key_spec(binding) == "f8" })
    );
    assert!(
        !runtime
            .iter()
            .any(|binding| { binding.action().name() == play_pause && key_spec(binding) == "f8" })
    );
}

#[test]
fn resetting_one_or_all_shortcuts_restores_the_selected_preset() {
    let preset = KeymapPreset::Default;
    let next_track = action_name(preset, "Next track");
    let previous_track = action_name(preset, "Previous track");
    let preset_runtime = get_keybindings_with_overrides(preset, &[]);

    let mut overrides = vec![
        CustomKeybinding::new(next_track, "f6"),
        CustomKeybinding::new(previous_track, "f5"),
    ];
    overrides.retain(|custom| custom.action_name != next_track);
    let single_reset_runtime = get_keybindings_with_overrides(preset, &overrides);

    let preset_next_keys = preset_runtime
        .iter()
        .filter(|binding| binding.action().name() == next_track)
        .map(key_spec)
        .collect::<Vec<_>>();
    let single_reset_next_keys = single_reset_runtime
        .iter()
        .filter(|binding| binding.action().name() == next_track)
        .map(key_spec)
        .collect::<Vec<_>>();
    assert_eq!(single_reset_next_keys, preset_next_keys);
    assert!(
        single_reset_runtime.iter().any(|binding| {
            binding.action().name() == previous_track && key_spec(binding) == "f5"
        })
    );

    overrides.clear();
    let all_reset_runtime = get_keybindings_with_overrides(preset, &overrides);
    let runtime_identity = |bindings: &[gpui::KeyBinding]| {
        bindings
            .iter()
            .map(|binding| (binding.action().name(), key_spec(binding)))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        runtime_identity(&all_reset_runtime),
        runtime_identity(&preset_runtime)
    );
}

#[test]
fn editor_contract_exposes_capture_conflict_overwrite_and_scoped_reset_controls() {
    let source = include_str!("../components/settings/keybindings.rs");
    let player_view = include_str!("../ui/player_view.rs");
    for contract in [
        "settings.keybinding.capture",
        "settings.keybinding.conflict",
        "settings.keybinding.overwrite",
        "settings.keybinding.reset.",
        "settings.keybinding.reset-all",
        "settings.keybinding.reset-all-confirm",
        "apply_keymap_and_persist",
    ] {
        assert!(
            source.contains(contract),
            "missing production contract: {contract}"
        );
    }
    assert!(
        player_view.contains("intercept_keystrokes"),
        "capture must run before GPUI resolves an existing shortcut action"
    );
    assert!(
        player_view.contains("if start_runtime"),
        "hidden screenshot views must not install competing interceptors"
    );
}
