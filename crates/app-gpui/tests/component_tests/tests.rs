use super::common::{
    FilterType, TestEQFilter, add_eq_band, clamp_parameter, denormalize_parameter,
    denormalize_parameter_log, normalize_parameter, normalize_parameter_log, remove_eq_band,
    validate_eq_filter,
};
use gpui_design::DesignSystem;
use sotf_audio_player_gpui::IconName;
use sotf_audio_player_gpui::components::design::typography_rems_from_rules;
use sotf_audio_player_gpui::components::home::album_card::{
    AlbumCardMode, album_card_height, format_channel_info, format_dr, format_sample_info,
    get_format_from_path,
};
use sotf_audio_player_gpui::components::plugins::common::{
    compute_transfer, format_shortcut_label,
};
use sotf_audio_player_gpui::components::{settings_tab_icon_name, settings_tab_label};
use sotf_audio_player_gpui::i18n::{Language, Translations};
use sotf_audio_player_gpui::plugin_file_picker::{FilePickerOpenTarget, file_picker_open_target};
use sotf_audio_player_gpui::theme::{Theme, ThemeId};
use sotf_audio_player_gpui::{InputMode, Screen, SettingsTab};
use sotf_plugins::param_specs::{self, ParamType};
use sotf_plugins::plugin_layout::ControlType;
use std::collections::BTreeMap;
use std::path::Path;

fn app_source(relative: &str) -> String {
    std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(relative))
        .unwrap_or_else(|err| panic!("failed to read {relative}: {err}"))
}

fn repo_source(relative: &str) -> String {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let repo_root = manifest_dir
        .parent()
        .and_then(Path::parent)
        .expect("app-gpui should live under crates/");
    std::fs::read_to_string(repo_root.join(relative))
        .unwrap_or_else(|err| panic!("failed to read {relative}: {err}"))
}

#[test]
fn plugin_rendering_uses_cached_registry_and_safe_layout_slots() {
    let plugins = app_source("components/plugins/mod.rs");
    assert!(
        plugins.contains("static GPUI_VIEW_REGISTRY")
            || plugins.contains("OnceLock<GpuiViewRegistry>"),
        "plugin renderer should cache the custom view registry"
    );
    assert!(
        !plugins.contains("let registry = GpuiViewRegistry::new();"),
        "plugin renderer must not construct GpuiViewRegistry per render"
    );

    let layout = app_source("ui/three_panel_layout.rs");
    assert!(
        !layout.contains(".unwrap()"),
        "three-panel rendering must not panic when a solved layout slot is missing"
    );
}

#[test]
fn dev_api_compile_guard_is_active_for_release_builds() {
    let lib = app_source("lib.rs");
    assert!(
        lib.contains("#[cfg(all(feature = \"dev-api\", not(debug_assertions)))]")
            && lib.contains("compile_error!("),
        "release builds must not compile the dev API"
    );
    assert!(
        !lib.contains("// #[cfg(all(feature = \"dev-api\", not(debug_assertions)))]"),
        "dev-api release compile guard must not be commented out"
    );
}

#[test]
fn release_gpui_recipe_excludes_dev_api_feature() {
    let justfile = repo_source("justfile");
    let gpui_recipe = justfile
        .split("\n[group('build')]\ngpui:\n")
        .nth(1)
        .and_then(|rest| rest.split("\n\n").next())
        .expect("missing build gpui recipe");

    assert!(
        gpui_recipe.contains("{{release_test_features}}"),
        "release gpui recipe must use the feature set that excludes dev-api"
    );
    assert!(
        !gpui_recipe.contains("{{test_features_macos}}") && !gpui_recipe.contains("dev-api"),
        "release gpui recipe must not compile the QA-only dev-api feature"
    );
}

#[test]
fn eq_renderer_caches_static_frequency_grid() {
    let eq_render = app_source("components/plugins/ui_eq/render.rs");
    assert!(
        eq_render.contains("static EQ_FREQUENCY_POINTS")
            && eq_render.contains("eq_frequency_points()"),
        "EQ renderer should reuse the log-spaced frequency grid"
    );
    assert!(
        !eq_render.contains("let freq_points: Vec<f64> = (0..num_points)"),
        "EQ renderer must not rebuild the static frequency grid every render"
    );
}

#[test]
fn eq_renderer_uses_curve_render_cache_helper() {
    let eq_render = app_source("components/plugins/ui_eq/render.rs");
    assert!(
        eq_render.contains("struct EqCurveRenderCache")
            && eq_render.contains("fn get_or_build(")
            && eq_render.contains("eq_curve_cache()")
            && eq_render.contains(".lock()")
            && eq_render.contains("cache.get_or_build(filters, freq_points)")
            && eq_render.contains("filter.topology")
            && eq_render.contains(".lambda")
            && eq_render.contains("filter.kautz_sections"),
        "EQ curve and band-response data should be built through a cache helper"
    );
    assert!(
        !eq_render.contains("let combined_response: Vec<f64> = freq_points"),
        "EQ renderer must not allocate combined response data inline every render"
    );
}

#[test]
fn app_gpui_unwrap_expect_production_audit_is_current() {
    fn visit_rs_files(path: &Path, files: &mut Vec<std::path::PathBuf>) {
        for entry in std::fs::read_dir(path).unwrap_or_else(|err| {
            panic!("failed to read {}: {err}", path.display());
        }) {
            let entry = entry.unwrap_or_else(|err| panic!("failed to read dir entry: {err}"));
            let path = entry.path();
            if path.is_dir() {
                visit_rs_files(&path, files);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                files.push(path);
            }
        }
    }

    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    for dir in ["app", "components", "ui"] {
        visit_rs_files(&root.join(dir), &mut files);
    }

    let mut actual = BTreeMap::new();
    for file in files {
        let relative = file
            .strip_prefix(root)
            .unwrap()
            .to_string_lossy()
            .to_string();
        let source = std::fs::read_to_string(&file)
            .unwrap_or_else(|err| panic!("failed to read {relative}: {err}"));
        let count = source.matches(".unwrap()").count() + source.matches(".expect(").count();
        if count > 0 {
            actual.insert(relative, count);
        }
    }

    let expected = BTreeMap::from([
        ("app/cast.rs".to_string(), 2),
        ("app/dev_api/server/parse.rs".to_string(), 1),
        ("app/federation/local.rs".to_string(), 2),
        ("app/midi_input.rs".to_string(), 3),
        ("app/remote/consts.rs".to_string(), 5),
        ("components/headphone_eq/actions/misc.rs".to_string(), 2),
        ("components/plugins/spatial_spider/data.rs".to_string(), 13),
        (
            "components/plugins/ui_layout_renderer/render.rs".to_string(),
            1,
        ),
        ("components/spinorama_eq/misc.rs".to_string(), 1),
        ("components/spinorama_eq/types.rs".to_string(), 1),
    ]);

    assert_eq!(
        actual, expected,
        "app-gpui unwrap/expect audit changed; replace user-triggerable sites \
         with graceful handling or update this baseline with a reason"
    );
}

#[test]
fn upmixer_renderer_uses_static_config_metadata() {
    let upmixer = app_source("components/plugins/ui_upmixer/render.rs");
    assert!(
        upmixer.contains("static UPMIXER_CONFIG_SPECS")
            && upmixer.contains("upmixer_config_specs()"),
        "upmixer renderer should reuse static config tab metadata"
    );
    assert!(
        !upmixer.contains("CONFIG_ITEMS.iter().enumerate().map(|(i, label)|"),
        "upmixer renderer must not rebuild tab ids and labels ad hoc every render"
    );
}

#[test]
fn plugin_ui_p1_hardcoded_pixels_are_documented_or_tokenized() {
    for relative in [
        "components/plugins/ui_eq/render.rs",
        "components/plugins/ui_upmixer/render.rs",
    ] {
        let source = app_source(relative);
        assert!(
            source.contains("Ds") && source.contains("d."),
            "{relative} should use the app design-token shim for spacing and sizing"
        );
        assert!(
            source.contains("intentional:") || source.contains("CHART_"),
            "{relative} should document domain-specific fixed geometry that cannot be a design token"
        );
    }
}

#[test]
fn channel_mute_solo_buttons_have_click_handlers() {
    let mute_solo = app_source("components/plugins/ui_mute_solo.rs");
    assert!(
        mute_solo
            .matches(".on_mouse_down(MouseButton::Left")
            .count()
            >= 3,
        "channel mute/solo/dim controls must be clickable"
    );
    assert!(mute_solo.contains("MsdAction::Mute"));
    assert!(mute_solo.contains("MsdAction::Solo"));
    assert!(mute_solo.contains("MsdAction::Dim"));
    assert!(
        mute_solo.contains("pending_plugin_update"),
        "channel mute/solo/dim clicks must schedule plugin graph reconfiguration"
    );
}

#[test]
fn test_typography_rems_platform_presets_affect_type_scale() {
    let neutral = typography_rems_from_rules(&DesignSystem::neutral().typography);
    let apple = typography_rems_from_rules(&DesignSystem::apple_hig().typography);
    let material = typography_rems_from_rules(&DesignSystem::material3().typography);

    assert!(apple.text_sm.0 > neutral.text_sm.0);
    assert!(apple.text_lg.0 > neutral.text_lg.0);
    assert!(material.text_lg.0 > neutral.text_lg.0);
}

#[test]
fn test_plugin_file_picker_keys_have_open_targets() {
    assert_eq!(
        file_picker_open_target("sofa_file"),
        Some(FilePickerOpenTarget::Sofa)
    );
    assert_eq!(
        file_picker_open_target("ir_file"),
        Some(FilePickerOpenTarget::Ir)
    );
    assert_eq!(
        file_picker_open_target("room_ir_file"),
        Some(FilePickerOpenTarget::Ir)
    );
    assert_eq!(
        file_picker_open_target("path_a_config"),
        Some(FilePickerOpenTarget::AbConfig("a"))
    );
    assert_eq!(
        file_picker_open_target("path_b_config"),
        Some(FilePickerOpenTarget::AbConfig("b"))
    );
}

#[test]
fn test_ab_compare_paths_tab_declares_actionable_file_pickers() {
    let params = param_specs::ab_compare::PARAMS;
    let paths_tab = param_specs::ab_compare::LAYOUT
        .tabs
        .iter()
        .find(|tab| tab.name == "Paths")
        .expect("A/B Compare must expose its config loaders in a Paths tab");

    let file_picker_keys: Vec<&'static str> = paths_tab
        .controls
        .iter()
        .filter(|control| matches!(control.control_type, ControlType::FilePicker))
        .map(|control| params[control.param_index].engine_key)
        .collect();

    assert_eq!(file_picker_keys, vec!["path_a_config", "path_b_config"]);
    for key in file_picker_keys {
        assert!(
            matches!(params.iter().find(|param| param.engine_key == key), Some(param) if matches!(param.param_type, ParamType::FilePath)),
            "{key} must remain a FilePath parameter"
        );
        assert!(
            file_picker_open_target(key).is_some(),
            "{key} must have a GPUI open action"
        );
    }
}

#[test]
fn test_settings_tabs_have_distinct_scan_icons() {
    assert_eq!(
        settings_tab_icon_name(SettingsTab::Library),
        IconName::Library
    );
    assert_eq!(
        settings_tab_icon_name(SettingsTab::Theme),
        IconName::PenTool
    );
    assert_eq!(
        settings_tab_icon_name(SettingsTab::AudioDevice),
        IconName::Speaker
    );
    assert_eq!(
        settings_tab_icon_name(SettingsTab::ReleaseChannel),
        IconName::AudioWaveform
    );
}

#[test]
fn test_settings_tabs_use_product_labels() {
    let translations = Translations::for_language(Language::English);

    assert_eq!(
        settings_tab_label(SettingsTab::Library, &translations),
        "Local Library"
    );
    assert_eq!(
        settings_tab_label(SettingsTab::Theme, &translations),
        "Appearance"
    );
    assert_eq!(
        settings_tab_label(SettingsTab::Misc, &translations),
        "Resources"
    );
    assert_eq!(
        settings_tab_label(SettingsTab::ReleaseChannel, &translations),
        "Features"
    );
}

#[test]
fn test_ios_settings_tabs_hide_local_library_and_keybindings() {
    let ios_tabs = SettingsTab::visible_tabs_for_ios(true);
    assert!(!ios_tabs.contains(&SettingsTab::Library));
    assert!(!ios_tabs.contains(&SettingsTab::Keybindings));
    assert!(ios_tabs.contains(&SettingsTab::Servers));
    assert!(ios_tabs.contains(&SettingsTab::AudioDevice));

    let desktop_tabs = SettingsTab::visible_tabs_for_ios(false);
    assert!(desktop_tabs.contains(&SettingsTab::Library));
    assert!(desktop_tabs.contains(&SettingsTab::Keybindings));
}

#[test]
fn test_album_card_height_grid() {
    assert_eq!(album_card_height(AlbumCardMode::Grid), 180.0);
}

#[test]
fn test_album_card_height_list() {
    assert_eq!(album_card_height(AlbumCardMode::List), 80.0);
}

#[test]
fn test_album_card_height_compact() {
    assert_eq!(album_card_height(AlbumCardMode::Compact), 56.0);
}

#[test]
fn test_format_sample_info_standard_44k() {
    assert_eq!(
        format_sample_info(Some(16), Some(44100)),
        Some("16/44.1k".to_string())
    );
}

#[test]
fn test_format_sample_info_hires_96k() {
    assert_eq!(
        format_sample_info(Some(24), Some(96000)),
        Some("24/96k".to_string())
    );
}

#[test]
fn test_format_sample_info_48k() {
    assert_eq!(
        format_sample_info(Some(24), Some(48000)),
        Some("24/48k".to_string())
    );
}

#[test]
fn test_format_sample_info_192k() {
    assert_eq!(
        format_sample_info(Some(32), Some(192000)),
        Some("32/192k".to_string())
    );
}

#[test]
fn test_format_sample_info_bit_depth_only() {
    assert_eq!(
        format_sample_info(Some(24), None),
        Some("24bit".to_string())
    );
}

#[test]
fn test_format_sample_info_sample_rate_only() {
    assert_eq!(
        format_sample_info(None, Some(44100)),
        Some("44.1k".to_string())
    );
}

#[test]
fn test_format_sample_info_none() {
    assert_eq!(format_sample_info(None, None), None);
}

#[test]
fn test_format_channel_info_surround_51() {
    assert_eq!(format_channel_info(Some(6)), Some("5.1".to_string()));
}

#[test]
fn test_format_channel_info_stereo_hidden() {
    assert_eq!(format_channel_info(Some(2)), None);
}

#[test]
fn test_format_channel_info_high_count() {
    assert_eq!(
        format_channel_info(Some(10)),
        Some("10ch (5.1.4/7.1.2)".to_string())
    );
}

#[test]
fn test_get_format_flac() {
    assert_eq!(
        get_format_from_path(Path::new("/music/album/track.flac")),
        Some("FLAC".to_string())
    );
}

#[test]
fn test_get_format_mp3() {
    assert_eq!(
        get_format_from_path(Path::new("/music/album/track.mp3")),
        Some("MP3".to_string())
    );
}

#[test]
fn test_get_format_wav() {
    assert_eq!(
        get_format_from_path(Path::new("/music/album/track.wav")),
        Some("WAV".to_string())
    );
}

#[test]
fn test_get_format_m4a() {
    assert_eq!(
        get_format_from_path(Path::new("/music/album/track.m4a")),
        Some("M4A".to_string())
    );
}

#[test]
fn test_get_format_ogg() {
    assert_eq!(
        get_format_from_path(Path::new("/music/album/track.ogg")),
        Some("OGG".to_string())
    );
}

#[test]
fn test_get_format_no_extension() {
    assert_eq!(get_format_from_path(Path::new("/music/album/track")), None);
}

#[test]
fn test_format_dr_some() {
    assert_eq!(format_dr(Some(14.3)), Some("14".to_string()));
}

#[test]
fn test_format_dr_rounds() {
    assert_eq!(format_dr(Some(14.7)), Some("15".to_string()));
}

#[test]
fn test_format_dr_none() {
    assert_eq!(format_dr(None), None);
}

#[test]
fn test_format_shortcut_label_first_letter() {
    assert_eq!(format_shortcut_label("Threshold", Some('t')), "[T]hreshold");
}

#[test]
fn test_format_shortcut_label_middle_letter() {
    assert_eq!(format_shortcut_label("Gain", Some('a')), "G[A]in");
}

#[test]
fn test_format_shortcut_label_not_found() {
    assert_eq!(format_shortcut_label("Gain", Some('x')), "[X] Gain");
}

#[test]
fn test_format_shortcut_label_case_insensitive() {
    assert_eq!(format_shortcut_label("RATIO", Some('r')), "[R]ATIO");
}

#[test]
fn test_format_shortcut_label_none() {
    assert_eq!(format_shortcut_label("Attack", None), "Attack");
}

#[test]
fn test_album_card_mode_eq() {
    assert_eq!(AlbumCardMode::Grid, AlbumCardMode::Grid);
    assert_ne!(AlbumCardMode::Grid, AlbumCardMode::List);
    assert_ne!(AlbumCardMode::List, AlbumCardMode::Compact);
}

#[test]
fn test_album_card_mode_copy() {
    let mode = AlbumCardMode::Grid;
    let copy = mode;
    assert_eq!(mode, copy);
}

#[test]
fn test_format_sample_info_very_high_sample_rate() {
    // DSD64 equivalent
    assert_eq!(
        format_sample_info(Some(1), Some(2822400)),
        Some("1/2822.4k".to_string())
    );
}

#[test]
fn test_format_sample_info_zero_values() {
    assert_eq!(
        format_sample_info(Some(0), Some(0)),
        Some("0/0k".to_string())
    );
}

#[test]
fn test_format_dr_zero() {
    assert_eq!(format_dr(Some(0.0)), Some("0".to_string()));
}

#[test]
fn test_format_dr_negative() {
    assert_eq!(format_dr(Some(-5.0)), Some("-5".to_string()));
}

#[test]
fn test_get_format_uppercase_extension() {
    assert_eq!(
        get_format_from_path(Path::new("/music/album/track.FLAC")),
        Some("FLAC".to_string())
    );
}

#[test]
fn test_get_format_mixed_case() {
    assert_eq!(
        get_format_from_path(Path::new("/music/album/track.FlAc")),
        Some("FLAC".to_string())
    );
}

#[test]
fn test_format_shortcut_label_empty_string() {
    assert_eq!(format_shortcut_label("", Some('a')), "[A] ");
}

#[test]
fn test_format_shortcut_label_single_char() {
    assert_eq!(format_shortcut_label("G", Some('g')), "[G]");
}

#[test]
fn test_format_sample_info_common_rates() {
    let test_cases = vec![
        (44100, "44.1k"),
        (48000, "48k"),
        (88200, "88.2k"),
        (96000, "96k"),
        (176400, "176.4k"),
        (192000, "192k"),
        (352800, "352.8k"),
        (384000, "384k"),
    ];

    for (rate, expected) in test_cases {
        assert_eq!(
            format_sample_info(Some(24), Some(rate)),
            Some(format!("24/{}", expected)),
            "Failed for rate {}",
            rate
        );
    }
}

#[test]
fn test_limiter_clips_at_threshold() {
    assert_eq!(compute_transfer(-10.0, -20.0, 10.0, 0.0, true), -20.0);
}

#[test]
fn test_limiter_passes_below_threshold() {
    assert_eq!(compute_transfer(-30.0, -20.0, 10.0, 0.0, true), -30.0);
}

#[test]
fn test_compressor_linear_region() {
    let result = compute_transfer(-50.0, -20.0, 4.0, 6.0, false);
    assert!((result - (-50.0)).abs() < 0.001);
}

#[test]
fn test_compressor_compression_region() {
    let result = compute_transfer(-10.0, -20.0, 4.0, 0.0, false);
    // threshold + (input - threshold) / ratio = -20 + 10/4 = -17.5
    assert!((result - (-17.5)).abs() < 0.001);
}

#[test]
fn test_compressor_knee_region() {
    let result = compute_transfer(-20.0, -20.0, 4.0, 6.0, false);
    assert!(result <= -20.0);
    assert!(result >= -25.0);
}

#[test]
fn test_all_themes_have_names() {
    for theme_id in ThemeId::all() {
        let name = theme_id.name();
        assert!(!name.is_empty(), "Theme {:?} has empty name", theme_id);
    }
}

#[test]
fn test_theme_count() {
    assert_eq!(ThemeId::all().len(), 9);
}

#[test]
fn test_theme_names_unique() {
    let names: Vec<_> = ThemeId::all().iter().map(|t| t.name()).collect();
    let mut unique_names = names.clone();
    unique_names.sort();
    unique_names.dedup();
    assert_eq!(
        names.len(),
        unique_names.len(),
        "Theme names must be unique"
    );
}

#[test]
fn test_builtin_themes_pass_core_contrast_validation() {
    for theme_id in ThemeId::all() {
        let theme = Theme::from_id(*theme_id);
        let validation = theme.validate_accessibility();
        assert!(
            validation.is_ok(),
            "{} should pass core contrast validation: {:?}",
            theme_id.name(),
            validation.err()
        );
    }
}

#[test]
fn test_theme_contrast_validation_rejects_unreadable_text() {
    let mut theme = Theme::dark();
    theme.text_primary = theme.background;

    let error = theme.validate_accessibility().unwrap_err();
    assert!(error.contains("text_primary/background"));
}

#[test]
fn test_all_languages_have_names() {
    for lang in Language::all() {
        let name = lang.name();
        assert!(!name.is_empty(), "Language {:?} has empty name", lang);
    }
}

#[test]
fn test_all_languages_have_codes() {
    for lang in Language::all() {
        let code = lang.code();
        assert_eq!(code.len(), 2, "Language {:?} should have 2-char code", lang);
    }
}

#[test]
fn test_language_count() {
    assert_eq!(Language::all().len(), 4);
}

#[test]
fn test_language_codes_unique() {
    let codes: Vec<_> = Language::all().iter().map(|l| l.code()).collect();
    let mut unique_codes = codes.clone();
    unique_codes.sort();
    unique_codes.dedup();
    assert_eq!(
        codes.len(),
        unique_codes.len(),
        "Language codes must be unique"
    );
}

#[test]
fn test_normalize_parameter_min() {
    assert!((normalize_parameter(-60.0, -60.0, 0.0) - 0.0).abs() < 0.001);
}

#[test]
fn test_normalize_parameter_max() {
    assert!((normalize_parameter(0.0, -60.0, 0.0) - 1.0).abs() < 0.001);
}

#[test]
fn test_normalize_parameter_mid() {
    assert!((normalize_parameter(-30.0, -60.0, 0.0) - 0.5).abs() < 0.001);
}

#[test]
fn test_denormalize_parameter_min() {
    assert!((denormalize_parameter(0.0, -60.0, 0.0) - (-60.0)).abs() < 0.001);
}

#[test]
fn test_denormalize_parameter_max() {
    assert!((denormalize_parameter(1.0, -60.0, 0.0) - 0.0).abs() < 0.001);
}

#[test]
fn test_denormalize_parameter_mid() {
    assert!((denormalize_parameter(0.5, -60.0, 0.0) - (-30.0)).abs() < 0.001);
}

#[test]
fn test_normalize_denormalize_roundtrip() {
    let original = -25.0;
    let normalized = normalize_parameter(original, -60.0, 0.0);
    let denormalized = denormalize_parameter(normalized, -60.0, 0.0);
    assert!((denormalized - original).abs() < 0.001);
}

#[test]
fn test_clamp_parameter_within_range() {
    assert!((clamp_parameter(-30.0, -60.0, 0.0) - (-30.0)).abs() < 0.001);
}

#[test]
fn test_clamp_parameter_below_min() {
    assert!((clamp_parameter(-100.0, -60.0, 0.0) - (-60.0)).abs() < 0.001);
}

#[test]
fn test_clamp_parameter_above_max() {
    assert!((clamp_parameter(10.0, -60.0, 0.0) - 0.0).abs() < 0.001);
}

#[test]
fn test_normalize_log_min() {
    assert!((normalize_parameter_log(20.0, 20.0, 20000.0) - 0.0).abs() < 0.001);
}

#[test]
fn test_normalize_log_max() {
    assert!((normalize_parameter_log(20000.0, 20.0, 20000.0) - 1.0).abs() < 0.001);
}

#[test]
fn test_denormalize_log_min() {
    assert!((denormalize_parameter_log(0.0, 20.0, 20000.0) - 20.0).abs() < 0.1);
}

#[test]
fn test_denormalize_log_max() {
    assert!((denormalize_parameter_log(1.0, 20.0, 20000.0) - 20000.0).abs() < 1.0);
}

#[test]
fn test_log_normalize_denormalize_roundtrip() {
    let original = 1000.0;
    let normalized = normalize_parameter_log(original, 20.0, 20000.0);
    let denormalized = denormalize_parameter_log(normalized, 20.0, 20000.0);
    assert!(
        (denormalized - original).abs() < 1.0,
        "Expected ~{}, got {}",
        original,
        denormalized
    );
}

#[test]
fn test_screen_includes_library() {
    // Verify key screen variants exist and are usable
    let _now_playing = Screen::NowPlaying;
    let _library = Screen::Library;
    let _settings = Screen::Settings;
    let _plugins = Screen::PluginGraph;
    let _recording = Screen::Recording;
    let _headphone = Screen::HeadphoneEq;
    let _spinorama = Screen::Spinorama;
    let _room_eq = Screen::RoomEq;
    let _queue = Screen::Queue;
    let _spectrum = Screen::Spectrum;
    let _studio = Screen::Studio;
    let _playlists = Screen::Playlists;
}

#[test]
fn test_screen_equality() {
    assert_eq!(Screen::Library, Screen::Library);
    assert_ne!(Screen::Library, Screen::Settings);
}

#[test]
fn test_settings_tab_variants_exist() {
    let _library = SettingsTab::Library;
    let _theme = SettingsTab::Theme;
    let _language = SettingsTab::Language;
    let _keybindings = SettingsTab::Keybindings;
    let _audio = SettingsTab::AudioDevice;
    let _misc = SettingsTab::Misc;
    let _federation = SettingsTab::Federation;
    let _servers = SettingsTab::Servers;
    let _release = SettingsTab::ReleaseChannel;
}

#[test]
fn test_settings_tab_equality() {
    assert_eq!(SettingsTab::Library, SettingsTab::Library);
    assert_ne!(SettingsTab::Library, SettingsTab::Theme);
}

#[test]
fn test_sample_info_and_format_together() {
    let format = get_format_from_path(Path::new("/music/album/track.flac"));
    let sample_info = format_sample_info(Some(24), Some(96000));

    assert_eq!(format, Some("FLAC".to_string()));
    assert_eq!(sample_info, Some("24/96k".to_string()));

    let metadata = format!(
        "{} {}",
        format.unwrap_or_default(),
        sample_info.unwrap_or_default()
    );
    assert_eq!(metadata, "FLAC 24/96k");
}

#[test]
fn test_normalization_for_slider() {
    let min = -60.0;
    let max = 0.0;
    let default_db = -20.0;

    let normalized = normalize_parameter(default_db, min, max);
    assert!((normalized - 0.6666).abs() < 0.01);

    let user_position = 0.75;
    let new_value = denormalize_parameter(user_position, min, max);
    assert!((new_value - (-15.0)).abs() < 0.001);
}

#[test]
fn test_log_normalization_for_frequency() {
    let freq_1k = normalize_parameter_log(1000.0, 20.0, 20000.0);
    assert!(
        freq_1k > 0.4 && freq_1k < 0.7,
        "1kHz should be mid-range on log scale: {}",
        freq_1k
    );
}

#[test]
fn test_compressor_curve_full_range() {
    let threshold = -20.0;
    let ratio = 4.0;
    let knee = 6.0;

    let test_inputs = [-60.0, -40.0, -30.0, -20.0, -10.0, 0.0];

    for &input in &test_inputs {
        let output = compute_transfer(input, threshold, ratio, knee, false);
        assert!(
            output <= input + 0.001,
            "Compressor output {} should not exceed input {}",
            output,
            input
        );
        assert!(
            output.is_finite(),
            "Output must be finite for input {}",
            input
        );
    }
}

#[test]
fn test_theme_and_language_consistency() {
    assert!(
        ThemeId::all().len() >= 5,
        "Should have at least 5 theme options"
    );
    assert!(
        Language::all().len() >= 3,
        "Should have at least 3 language options"
    );
    assert!(Language::all().contains(&Language::English));
}

#[test]
fn test_dr_formatting_edge_cases() {
    // Rust uses banker's rounding (round half to even) for {:.0}
    assert_eq!(format_dr(Some(0.0)), Some("0".to_string()));
    assert_eq!(format_dr(Some(0.4)), Some("0".to_string()));
    assert_eq!(format_dr(Some(0.5)), Some("0".to_string())); // Banker's: 0.5 -> 0
    assert_eq!(format_dr(Some(1.5)), Some("2".to_string())); // Banker's: 1.5 -> 2
    assert_eq!(format_dr(Some(20.0)), Some("20".to_string()));
    assert_eq!(format_dr(None), None);
}

#[test]
fn test_default_eq_filter_values() {
    let filter = TestEQFilter::default_peak();
    assert_eq!(filter.filter_type, FilterType::Peak);
    assert!((filter.frequency - 1000.0).abs() < 0.001);
    assert!((filter.q - 1.0).abs() < 0.001);
    assert!((filter.gain_db - 0.0).abs() < 0.001);
}

#[test]
fn test_add_eq_band_to_empty_list() {
    let mut filters = Vec::new();
    assert_eq!(add_eq_band(&mut filters), 1);
    assert_eq!(filters[0].filter_type, FilterType::Peak);
}

#[test]
fn test_add_eq_band_to_existing_list() {
    let mut filters = vec![
        TestEQFilter::new(FilterType::LowShelf, 100.0, 0.7, 3.0),
        TestEQFilter::new(FilterType::HighShelf, 10000.0, 0.7, -2.0),
    ];
    assert_eq!(add_eq_band(&mut filters), 3);
    assert_eq!(filters[2].filter_type, FilterType::Peak);
}

#[test]
fn test_add_multiple_eq_bands() {
    let mut filters = Vec::new();
    add_eq_band(&mut filters);
    add_eq_band(&mut filters);
    add_eq_band(&mut filters);
    assert_eq!(filters.len(), 3);
    for filter in &filters {
        assert_eq!(filter.filter_type, FilterType::Peak);
    }
}

#[test]
fn test_remove_eq_band_valid_index() {
    let mut filters = vec![
        TestEQFilter::new(FilterType::LowShelf, 100.0, 0.7, 3.0),
        TestEQFilter::new(FilterType::Peak, 1000.0, 1.0, 0.0),
        TestEQFilter::new(FilterType::HighShelf, 10000.0, 0.7, -2.0),
    ];
    assert_eq!(remove_eq_band(&mut filters, 1).unwrap(), 2);
    assert_eq!(filters[0].filter_type, FilterType::LowShelf);
    assert_eq!(filters[1].filter_type, FilterType::HighShelf);
}

#[test]
fn test_remove_eq_band_out_of_bounds() {
    let mut filters = vec![
        TestEQFilter::new(FilterType::Peak, 1000.0, 1.0, 0.0),
        TestEQFilter::new(FilterType::Peak, 2000.0, 1.0, 0.0),
    ];
    let result = remove_eq_band(&mut filters, 5);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Invalid band index"));
}

#[test]
fn test_remove_last_eq_band_fails() {
    let mut filters = vec![TestEQFilter::new(FilterType::Peak, 1000.0, 1.0, 0.0)];
    let result = remove_eq_band(&mut filters, 0);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Cannot remove the last"));
}

#[test]
fn test_validate_eq_filter_valid() {
    assert!(validate_eq_filter(&TestEQFilter::new(FilterType::Peak, 1000.0, 1.0, 3.0)).is_ok());
}

#[test]
fn test_validate_eq_filter_frequency_too_low() {
    assert!(validate_eq_filter(&TestEQFilter::new(FilterType::Peak, 10.0, 1.0, 0.0)).is_err());
}

#[test]
fn test_validate_eq_filter_frequency_too_high() {
    assert!(validate_eq_filter(&TestEQFilter::new(FilterType::Peak, 25000.0, 1.0, 0.0)).is_err());
}

#[test]
fn test_validate_eq_filter_frequency_at_limits() {
    assert!(validate_eq_filter(&TestEQFilter::new(FilterType::Peak, 20.0, 1.0, 0.0)).is_ok());
    assert!(validate_eq_filter(&TestEQFilter::new(FilterType::Peak, 20000.0, 1.0, 0.0)).is_ok());
}

#[test]
fn test_validate_eq_filter_q_too_low() {
    assert!(validate_eq_filter(&TestEQFilter::new(FilterType::Peak, 1000.0, 0.05, 0.0)).is_err());
}

#[test]
fn test_validate_eq_filter_q_too_high() {
    assert!(validate_eq_filter(&TestEQFilter::new(FilterType::Peak, 1000.0, 15.0, 0.0)).is_err());
}

#[test]
fn test_validate_eq_filter_q_at_limits() {
    assert!(validate_eq_filter(&TestEQFilter::new(FilterType::Peak, 1000.0, 0.1, 0.0)).is_ok());
    assert!(validate_eq_filter(&TestEQFilter::new(FilterType::Peak, 1000.0, 10.0, 0.0)).is_ok());
}

#[test]
fn test_validate_eq_filter_gain_too_low() {
    assert!(validate_eq_filter(&TestEQFilter::new(FilterType::Peak, 1000.0, 1.0, -30.0)).is_err());
}

#[test]
fn test_validate_eq_filter_gain_too_high() {
    assert!(validate_eq_filter(&TestEQFilter::new(FilterType::Peak, 1000.0, 1.0, 30.0)).is_err());
}

#[test]
fn test_validate_eq_filter_gain_at_limits() {
    assert!(validate_eq_filter(&TestEQFilter::new(FilterType::Peak, 1000.0, 1.0, -24.0)).is_ok());
    assert!(validate_eq_filter(&TestEQFilter::new(FilterType::Peak, 1000.0, 1.0, 24.0)).is_ok());
}

#[test]
fn test_eq_filter_types() {
    let types = [
        FilterType::Peak,
        FilterType::LowShelf,
        FilterType::HighShelf,
        FilterType::LowPass,
        FilterType::HighPass,
        FilterType::BandPass,
        FilterType::Notch,
    ];
    for filter_type in types {
        assert!(validate_eq_filter(&TestEQFilter::new(filter_type, 1000.0, 1.0, 0.0)).is_ok());
    }
}

#[test]
fn test_add_and_remove_eq_band_roundtrip() {
    let mut filters = vec![TestEQFilter::new(FilterType::Peak, 500.0, 1.5, 2.0)];
    add_eq_band(&mut filters);
    assert_eq!(filters.len(), 2);
    assert!(remove_eq_band(&mut filters, 1).is_ok());
    assert_eq!(filters.len(), 1);
    assert!((filters[0].frequency - 500.0).abs() < 0.001);
}

#[test]
fn test_library_state_set_search_query_clears_previous() {
    use sotf_audio_player_gpui::app::state::library::LibraryState;
    let mut state = LibraryState::new_for_test();

    state.set_search_query("first query".to_string());
    assert_eq!(state.search_query, "first query");

    state.set_search_query("second query".to_string());
    assert_eq!(state.search_query, "second query");
}

#[test]
fn test_library_state_set_search_query_empty() {
    use sotf_audio_player_gpui::app::state::library::LibraryState;
    let mut state = LibraryState::new_for_test();

    state.set_search_query("test".to_string());
    state.set_search_query("".to_string());
    assert_eq!(state.search_query, "");
}

#[test]
fn test_library_state_search_query_default_empty() {
    use sotf_audio_player_gpui::app::state::library::LibraryState;
    let state = LibraryState::new_for_test();
    assert_eq!(state.search_query, "");
}

#[test]
fn test_library_state_content_generation_tracks_content_invalidations() {
    use sotf_audio_player_gpui::app::state::library::LibraryState;
    let mut state = LibraryState::new_for_test();

    let initial = state.content_generation();
    state.set_search_query("view-only filter".to_string());
    assert_eq!(state.content_generation(), initial);

    state.invalidate_cache();
    assert_ne!(state.content_generation(), initial);
}

#[test]
fn test_input_mode_is_text_input_search() {
    assert!(InputMode::Search.is_text_input());
}

#[test]
fn test_input_mode_is_text_input_normal() {
    assert!(!InputMode::Normal.is_text_input());
}

#[test]
fn test_input_mode_is_text_input_add_directory() {
    assert!(InputMode::AddDirectory.is_text_input());
}

#[test]
fn test_input_mode_is_text_input_editing_param() {
    // EditingParam uses stepper/knob interaction, not text input
    assert!(!InputMode::EditingParam.is_text_input());
}
