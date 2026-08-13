use sotf_audio_player_gpui::app::state::ui::{
    LUFS_PANEL_MAX_RATIO, LUFS_PANEL_MIN_RATIO, LayoutState, lufs_panel_ratio_from_drag,
};
use sotf_audio_player_gpui::app::state::{
    RACK_STRIP_MAX_HEIGHT, RACK_STRIP_MIN_HEIGHT, rack_strip_height_from_drag,
};
use sotf_audio_player_gpui::queue_render::queue_meters_panel_width;
use sotf_audio_player_gpui::{
    Config, IconName, IconSize, ImageAccessTracker, PanelLayout, PlaybackDeviceConfig,
    PlaybackState, RecordingConfigState, RecordingDeviceConfig, RecordingSignalType, ScaleType,
    WindowGeometry, default_volume, estimate_grid_dimensions,
};
use std::path::PathBuf;

#[test]
fn test_recording_config_state_default() {
    let config = RecordingConfigState::default();
    assert_eq!(config.signal_duration_secs, 5.0);
    assert!((config.signal_level_db - -6.0206).abs() < 0.0001);
    assert!(config.mic_calibration_path.is_none());
    match &config.recording_base_directory {
        Some(base_directory) => {
            assert!(config.recording_directory.is_some());
            assert!(PathBuf::from(base_directory).ends_with("Recordings"));
        }
        None => assert!(config.recording_directory.is_none()),
    }
}

#[test]
fn test_window_geometry_default() {
    let geometry = WindowGeometry::default();
    assert_eq!(geometry.x, 100.0);
    assert_eq!(geometry.y, 100.0);
    assert_eq!(geometry.width, 1200.0);
    assert_eq!(geometry.height, 800.0);
}

#[test]
fn test_panel_layout_default() {
    let layout = PanelLayout::default();
    assert!((layout.queue_ratio - 0.35).abs() < 0.001);
    assert!((layout.meters_ratio - 0.25).abs() < 0.001);
    assert!((layout.queue_list_ratio - 0.30).abs() < 0.001);
    assert!((layout.lufs_ratio - 0.25).abs() < 0.001);
    assert!((layout.rack_detail_ratio - 0.22).abs() < 0.001);
}

#[test]
fn test_panel_layout_serialization() {
    let layout = PanelLayout {
        queue_ratio: 0.5,
        meters_ratio: 0.3,
        queue_list_ratio: 0.4,
        lufs_ratio: 0.2,
        rack_detail_ratio: 0.24,
        library_h_ratio: 0.3,
        queue_h_ratio: 0.4,
        rack_h_ratio: 0.3,
        library_v_ratio: 0.4,
        queue_v_ratio: 0.35,
        rack_v_ratio: 0.25,
    };
    let json = serde_json::to_string(&layout).unwrap();
    let deserialized: PanelLayout = serde_json::from_str(&json).unwrap();
    assert!((deserialized.queue_ratio - 0.5).abs() < 0.001);
    assert!((deserialized.meters_ratio - 0.3).abs() < 0.001);
    assert!((deserialized.rack_detail_ratio - 0.24).abs() < 0.001);
}

#[test]
fn test_lufs_level_meter_divider_drag_updates_lufs_ratio() {
    let mut layout = LayoutState {
        lufs_panel_ratio: 0.25,
        ..Default::default()
    };

    layout.begin_lufs_panel_drag(200.0);
    layout.update_lufs_panel_drag(320.0, 600.0);

    assert!((layout.lufs_panel_ratio - 0.45).abs() < 0.001);
    assert!(layout.end_lufs_panel_drag());
    assert!(!layout.is_dragging_lufs_divider);
}

#[test]
fn test_lufs_level_meter_divider_ratio_clamps_to_meter_bounds() {
    assert_eq!(
        lufs_panel_ratio_from_drag(0.25, -1000.0, 600.0),
        LUFS_PANEL_MIN_RATIO
    );
    assert_eq!(
        lufs_panel_ratio_from_drag(0.25, 1000.0, 600.0),
        LUFS_PANEL_MAX_RATIO
    );
}

#[test]
fn test_queue_meters_panel_width_shrinks_below_nominal_min_when_narrow() {
    let width = queue_meters_panel_width(0.25, 141.7583);

    assert!((width - 85.054985).abs() < 0.0001);
}

#[test]
fn test_queue_meters_panel_width_keeps_bounds_ordered() {
    assert_eq!(queue_meters_panel_width(0.10, 800.0), 120.0);
    assert!((queue_meters_panel_width(0.80, 800.0) - 480.0).abs() < 0.0001);
    assert_eq!(queue_meters_panel_width(f32::NAN, 800.0), 120.0);
    assert_eq!(queue_meters_panel_width(0.25, f32::NAN), 0.0);
}

#[test]
fn test_rack_detail_divider_drag_updates_strip_height() {
    assert_eq!(rack_strip_height_from_drag(180.0, 40.0), 220.0);
    assert_eq!(rack_strip_height_from_drag(180.0, -30.0), 150.0);
}

#[test]
fn test_rack_detail_divider_height_clamps_to_rack_bounds() {
    assert_eq!(
        rack_strip_height_from_drag(180.0, -1000.0),
        RACK_STRIP_MIN_HEIGHT
    );
    assert_eq!(
        rack_strip_height_from_drag(180.0, 1000.0),
        RACK_STRIP_MAX_HEIGHT
    );
}

#[test]
fn test_window_geometry_serialization() {
    let geometry = WindowGeometry {
        x: 200.0,
        y: 150.0,
        width: 1600.0,
        height: 900.0,
    };
    let json = serde_json::to_string(&geometry).unwrap();
    let deserialized: WindowGeometry = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.x, 200.0);
    assert_eq!(deserialized.y, 150.0);
    assert_eq!(deserialized.width, 1600.0);
    assert_eq!(deserialized.height, 900.0);
}

#[test]
fn test_recording_config_state_serialization() {
    let config = RecordingConfigState {
        playback: PlaybackDeviceConfig::default(),
        recording: RecordingDeviceConfig::default(),
        signal_type: RecordingSignalType::PinkNoise,
        signal_duration_secs: 10.0,
        signal_level_db: -12.0,
        mic_calibration_path: Some("/path/to/cal.txt".to_string()),
        mic_calibration_paths: vec![Some("/path/to/cal.txt".to_string())],
        recording_directory: Some("/recordings".to_string()),
        recording_base_directory: None,
    };
    let json = serde_json::to_string(&config).unwrap();
    let deserialized: RecordingConfigState = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.signal_type, RecordingSignalType::PinkNoise);
    assert_eq!(deserialized.signal_duration_secs, 10.0);
    assert_eq!(
        deserialized.mic_calibration_path,
        Some("/path/to/cal.txt".to_string())
    );
}

#[test]
fn test_config_serialization() {
    use gpui_themes::{AccessibilityPalette, ThemeModePreference, ThemeSchedule, TimeOfDay};
    use sotf_audio_player::ReleaseChannel;
    use sotf_audio_player_gpui::app::types::DensityMode;
    use sotf_audio_player_gpui::i18n::Language;
    use sotf_audio_player_gpui::keybindings::KeymapPreset;
    use sotf_audio_player_gpui::theme::{CommunityThemeId, ThemeAccentPreference, ThemeId};

    let schedule = ThemeSchedule::new(TimeOfDay::new(6, 30), TimeOfDay::new(21, 15));
    let config = Config {
        directories: Vec::new(),
        last_loaded_plugin_preset: Some("test_preset".to_string()),
        theme: ThemeId::default(),
        theme_mode_preference: ThemeModePreference::Scheduled { schedule },
        accessibility_palette: AccessibilityPalette::default(),
        theme_accent_preference: ThemeAccentPreference::System,
        community_theme_id: Some(CommunityThemeId::Nord),
        reduce_motion: false,
        density_mode: DensityMode::Standard,
        language: Language::default(),
        keymap_preset: KeymapPreset::default(),
        panel_layout: PanelLayout::default(),
        window_geometry: WindowGeometry::default(),
        volume: 0.75,
        muted: true,
        recording_config: RecordingConfigState::default(),
        font_scale: 1.0,
        release_channel: ReleaseChannel::default(),
        scanner_threads: Some(2),
        tutorial_completed: false,
        seen_hints: Vec::new(),
        max_cpu_cores: None,
        min_font_size_px: None,
        max_font_size_px: None,
        design_language: None,
        rack_theme_state: Default::default(),
        remote_library_identity: None,
    };
    let json = serde_json::to_string(&config).unwrap();
    let deserialized: Config = serde_json::from_str(&json).unwrap();
    assert_eq!(
        deserialized.last_loaded_plugin_preset,
        Some("test_preset".to_string())
    );
    assert!((deserialized.volume - 0.75).abs() < 0.001);
    assert!(deserialized.muted);
    assert_eq!(deserialized.scanner_threads, Some(2));
    assert_eq!(
        deserialized.theme_mode_preference,
        ThemeModePreference::Scheduled { schedule }
    );
    assert_eq!(
        deserialized.accessibility_palette,
        AccessibilityPalette::Standard
    );
    assert_eq!(
        deserialized.theme_accent_preference,
        ThemeAccentPreference::System
    );
    assert_eq!(
        deserialized.community_theme_id,
        Some(CommunityThemeId::Nord)
    );
    assert!(!deserialized.reduce_motion);
}

#[test]
fn test_config_default_volume() {
    assert!((default_volume() - 0.1).abs() < 0.001);
}

#[test]
fn test_theme_accessibility_palette_mapping() {
    use gpui_themes::{AccessibilityPalette, ThemeAppearance};
    use sotf_audio_player_gpui::theme::ThemeId;

    assert_eq!(
        ThemeId::for_accessibility_palette(AccessibilityPalette::Standard, ThemeAppearance::Light),
        ThemeId::Light
    );
    assert_eq!(
        ThemeId::for_accessibility_palette(
            AccessibilityPalette::HighContrast,
            ThemeAppearance::Dark
        ),
        ThemeId::BlackAndWhite
    );
    assert_eq!(
        ThemeId::for_accessibility_palette(AccessibilityPalette::Protanopia, ThemeAppearance::Dark),
        ThemeId::Protanopia
    );
    assert_eq!(
        ThemeId::Deuteranopia.accessibility_palette(),
        AccessibilityPalette::Deuteranopia
    );
}

#[test]
fn test_theme_mode_and_motion_state_defaults() {
    use gpui_themes::{AccessibilityPalette, ThemeModePreference};
    use sotf_audio_player_gpui::app::state::UIState;
    use sotf_audio_player_gpui::{app::types::DensityMode, theme::ThemeAccentPreference};

    let state = UIState::default();
    assert_eq!(
        state.theme_mode_preference,
        ThemeModePreference::FollowSystem
    );
    assert_eq!(state.accessibility_palette, AccessibilityPalette::Standard);
    assert_eq!(state.theme_accent_preference, ThemeAccentPreference::Theme);
    assert_eq!(state.density_mode, DensityMode::Standard);
    assert_eq!(state.community_theme_id, None);
    assert!(state.community_theme_json_draft.is_empty());
    assert!(!state.reduce_motion);
}

#[test]
fn test_community_theme_preset_exports_valid_bundle() {
    use gpui_themes::CommunityThemeBundle;
    use sotf_audio_player_gpui::theme::{CommunityThemeId, Theme};

    for id in CommunityThemeId::all() {
        let json = id.to_community_json().unwrap();
        let bundle = CommunityThemeBundle::from_json(&json).unwrap();
        assert_eq!(bundle.manifest.id, id.value());
        assert_eq!(bundle.manifest.display_name, id.name());
        assert!(bundle.validate().is_ok());

        let app_theme = Theme::from_community_bundle(&bundle)
            .unwrap_or_else(|err| panic!("{} failed app contrast validation: {err}", id.name()));
        assert_eq!(app_theme.accent, bundle.theme.accent.to_rgba());
        assert!(!app_theme.plugin_palette.band_colors.is_empty());
        assert_eq!(
            app_theme.plugin_palette.channel_colors,
            app_theme.plugin_palette.band_colors
        );
    }
}

#[test]
fn test_community_theme_import_rejects_low_contrast_text() {
    use gpui_themes::{CommunityThemeBundle, EditorTheme};
    use sotf_audio_player_gpui::theme::Theme;

    let mut editor_theme = EditorTheme::dark();
    editor_theme.text_primary = editor_theme.background;
    let bundle = CommunityThemeBundle::from_theme(editor_theme);

    assert!(bundle.validate().is_ok());
    let error = Theme::from_community_bundle(&bundle).unwrap_err();
    assert!(error.contains("text_primary/background"));
}

#[test]
fn test_app_community_theme_selection_updates_theme_state() {
    use sotf_audio_player_gpui::{
        App,
        theme::{CommunityThemeId, ThemeAccentPreference, ThemeId},
    };

    let mut app = App::new();
    app.set_community_theme(CommunityThemeId::Dracula);
    assert_eq!(
        app.ui_state.community_theme_id,
        Some(CommunityThemeId::Dracula)
    );
    assert_eq!(app.ui_state.theme_id, ThemeId::Dark);
    assert_eq!(
        app.ui_state.theme.accent,
        CommunityThemeId::Dracula.theme().accent
    );

    app.set_theme_accent_preference(ThemeAccentPreference::Mint);
    let accent = ThemeAccentPreference::Mint
        .seed_and_source()
        .unwrap()
        .0
        .to_rgba();
    assert_eq!(
        app.ui_state.community_theme_id,
        Some(CommunityThemeId::Dracula)
    );
    assert_eq!(app.ui_state.theme.accent, accent);

    app.set_theme_accent_preference(ThemeAccentPreference::Theme);
    let json = CommunityThemeId::Nord.to_community_json().unwrap();
    app.set_community_theme_from_json(&json).unwrap();
    assert_eq!(
        app.ui_state.community_theme_id,
        Some(CommunityThemeId::Nord)
    );
    assert_eq!(
        app.ui_state.theme.accent,
        CommunityThemeId::Nord.theme().accent
    );

    app.set_theme(ThemeId::Light);
    assert_eq!(app.ui_state.community_theme_id, None);
    assert_eq!(app.ui_state.theme_id, ThemeId::Light);
}

#[test]
fn test_app_community_theme_json_draft_import_flow() {
    use sotf_audio_player_gpui::{App, theme::CommunityThemeId};

    let mut app = App::new();
    let json = CommunityThemeId::Nord.to_community_json().unwrap();
    app.set_community_theme_json_draft(json.clone());
    assert_eq!(app.ui_state.community_theme_json_draft, json);
    app.apply_community_theme_json_draft().unwrap();
    assert_eq!(
        app.ui_state.community_theme_id,
        Some(CommunityThemeId::Nord)
    );

    app.set_community_theme_json_draft("");
    assert!(app.apply_community_theme_json_draft().is_err());

    app.set_community_theme_json_draft("{\"manifest\":{},\"theme\":{}}");
    assert!(app.apply_community_theme_json_draft().is_err());
}

#[test]
fn test_app_theme_policy_updates_theme_state() {
    use gpui_themes::{AccessibilityPalette, ThemeAppearance, ThemeModePreference};
    use sotf_audio_player_gpui::{
        App,
        theme::{ThemeAccentPreference, ThemeId},
    };

    let mut app = App::new();
    app.set_theme_accent_preference(ThemeAccentPreference::Rose);
    let accent = ThemeAccentPreference::Rose
        .seed_and_source()
        .unwrap()
        .0
        .to_rgba();
    assert_eq!(app.ui_state.theme.accent, accent);

    app.set_theme_mode_preference_with_system(
        ThemeModePreference::FollowSystem,
        ThemeAppearance::Light,
    );
    assert_eq!(app.ui_state.theme_id, ThemeId::Light);
    assert_eq!(app.ui_state.theme.accent, accent);
    assert_eq!(
        app.ui_state.accessibility_palette,
        AccessibilityPalette::Standard
    );

    app.set_accessibility_palette_with_system(
        AccessibilityPalette::Protanopia,
        ThemeAppearance::Light,
    );
    assert_eq!(app.ui_state.theme_id, ThemeId::Protanopia);
    assert_eq!(app.ui_state.theme.accent, accent);
    assert_eq!(
        app.ui_state.accessibility_palette,
        AccessibilityPalette::Protanopia
    );

    app.set_theme_mode_preference_with_system(ThemeModePreference::Dark, ThemeAppearance::Dark);
    assert_eq!(app.ui_state.theme_id, ThemeId::Protanopia);
    assert_eq!(
        app.ui_state.accessibility_palette,
        AccessibilityPalette::Protanopia
    );

    app.set_reduce_motion(true);
    assert_eq!(app.theme_transition_duration_ms(), 0);
}

#[test]
fn test_app_scheduled_theme_switching_updates_at_boundaries() {
    use gpui_themes::{ThemeAppearance, ThemeSchedule, TimeOfDay};
    use sotf_audio_player_gpui::{App, theme::ThemeId};

    let mut app = App::new();
    let schedule = ThemeSchedule::new(TimeOfDay::new(6, 30), TimeOfDay::new(21, 15));

    app.set_theme_schedule_at_minutes(schedule, ThemeAppearance::Dark, 7 * 60);
    assert_eq!(app.theme_schedule(), schedule);
    assert_eq!(app.ui_state.theme_id, ThemeId::Light);

    assert!(!app.refresh_scheduled_theme_at_minutes(8 * 60));
    assert_eq!(app.ui_state.theme_id, ThemeId::Light);

    assert!(app.refresh_scheduled_theme_at_minutes(22 * 60));
    assert_eq!(app.ui_state.theme_id, ThemeId::Dark);
}

#[test]
fn test_playback_state_defaults() {
    let state = PlaybackState::default();

    assert!(!state.is_playing);
    assert_eq!(state.current_queue_index, None);
    // Default volume is 0.1 (DEFAULT_STARTUP_VOLUME from constants)
    assert!((state.volume - 0.1).abs() < 0.01);
    assert!(!state.muted);
    assert_eq!(state.position_secs, 0.0);
    assert_eq!(state.duration_secs, 0.0);
    assert!(state.input_loudness_info.is_none());
    assert!(state.loudness_info.is_none());
    assert!(state.spectrum_info.is_none());
    assert!(state.compressor_info.is_none());
}

#[test]
fn test_tracker_with_custom_capacity() {
    let tracker = ImageAccessTracker::with_capacity(50);
    assert_eq!(tracker.stats().capacity, 50);
}

#[test]
fn test_tracker_record_and_check() {
    let mut tracker = ImageAccessTracker::new();
    let path = PathBuf::from("/test/image.jpg");

    tracker.record_access(&path);

    assert!(tracker.was_accessed(&path));
    assert_eq!(tracker.access_count(&path), 1);
}

#[test]
fn test_tracker_eviction() {
    let mut tracker = ImageAccessTracker::with_capacity(3);

    for i in 0..5 {
        let path = PathBuf::from(format!("/test/image{}.jpg", i));
        tracker.record_access(&path);
    }

    // Only the last 3 should be tracked
    assert_eq!(tracker.stats().tracked, 3);

    // First two should be evicted
    assert!(!tracker.was_accessed(&PathBuf::from("/test/image0.jpg")));
    assert!(!tracker.was_accessed(&PathBuf::from("/test/image1.jpg")));

    // Last three should be present
    assert!(tracker.was_accessed(&PathBuf::from("/test/image2.jpg")));
    assert!(tracker.was_accessed(&PathBuf::from("/test/image3.jpg")));
    assert!(tracker.was_accessed(&PathBuf::from("/test/image4.jpg")));
}

#[test]
fn test_tracker_lru_order() {
    let mut tracker = ImageAccessTracker::with_capacity(3);

    let path1 = PathBuf::from("/test/image1.jpg");
    let path2 = PathBuf::from("/test/image2.jpg");
    let path3 = PathBuf::from("/test/image3.jpg");
    let path4 = PathBuf::from("/test/image4.jpg");

    tracker.record_access(&path1);
    tracker.record_access(&path2);
    tracker.record_access(&path3);

    // Access path1 to make it recently used
    tracker.record_access(&path1);

    // Record path4, should evict path2 (least recently used)
    tracker.record_access(&path4);

    assert!(tracker.was_accessed(&path1)); // Recently accessed
    assert!(!tracker.was_accessed(&path2)); // Evicted
    assert!(tracker.was_accessed(&path3)); // Still present
    assert!(tracker.was_accessed(&path4)); // Newly added
}

#[test]
fn test_tracker_clear() {
    let mut tracker = ImageAccessTracker::new();

    tracker.record_access(&PathBuf::from("/test/image1.jpg"));
    tracker.record_access(&PathBuf::from("/test/image2.jpg"));

    assert_eq!(tracker.stats().tracked, 2);

    tracker.clear();

    assert_eq!(tracker.stats().tracked, 0);
}

#[test]
fn test_tracker_preload() {
    let mut tracker = ImageAccessTracker::new();
    let path = PathBuf::from("/test/image.jpg");

    assert!(!tracker.is_preloaded(&path));

    tracker.mark_preloaded(&path);

    assert!(tracker.is_preloaded(&path));
}

#[test]
fn test_tracker_stats() {
    let tracker = ImageAccessTracker::with_capacity(100);
    let stats = tracker.stats();

    assert_eq!(stats.tracked, 0);
    assert_eq!(stats.capacity, 100);
    assert_eq!(stats.utilization(), 0.0);
}

#[test]
fn test_tracker_recent_paths() {
    let mut tracker = ImageAccessTracker::new();

    tracker.record_access(&PathBuf::from("/test/image1.jpg"));
    tracker.record_access(&PathBuf::from("/test/image2.jpg"));
    tracker.record_access(&PathBuf::from("/test/image3.jpg"));

    let recent = tracker.recent_paths(2);
    assert_eq!(recent.len(), 2);
    // Most recent should be first (reversed order)
    assert_eq!(recent[0], &PathBuf::from("/test/image3.jpg"));
    assert_eq!(recent[1], &PathBuf::from("/test/image2.jpg"));
}

#[test]
fn test_preload_candidates() {
    let mut tracker = ImageAccessTracker::new();

    let paths: Vec<PathBuf> = (0..5)
        .map(|i| PathBuf::from(format!("/test/image{}.jpg", i)))
        .collect();

    // Mark first two as preloaded
    tracker.mark_preloaded(&paths[0]);
    tracker.mark_preloaded(&paths[1]);

    // Should get next 3 as candidates (skipping preloaded ones)
    let candidates = tracker.get_preload_candidates(&paths, 3);
    assert_eq!(candidates.len(), 3);
    assert_eq!(candidates[0], paths[2]);
    assert_eq!(candidates[1], paths[3]);
    assert_eq!(candidates[2], paths[4]);
}

#[test]
fn test_linear_scale() {
    let scale = ScaleType::Linear;
    assert!((scale.value_to_position(0.0, 0.0, 100.0) - 0.0).abs() < 0.001);
    assert!((scale.value_to_position(50.0, 0.0, 100.0) - 0.5).abs() < 0.001);
    assert!((scale.value_to_position(100.0, 0.0, 100.0) - 1.0).abs() < 0.001);
}

#[test]
fn test_quadratic_scale() {
    let scale = ScaleType::Quadratic;
    // At 50% of the value range, position should be 25% (0.5^2 = 0.25)
    assert!((scale.value_to_position(50.0, 0.0, 100.0) - 0.25).abs() < 0.001);
    // At 0 and max, should be same as linear
    assert!((scale.value_to_position(0.0, 0.0, 100.0) - 0.0).abs() < 0.001);
    assert!((scale.value_to_position(100.0, 0.0, 100.0) - 1.0).abs() < 0.001);
}

#[test]
fn test_db_scale_emphasis() {
    let scale = ScaleType::Quadratic;
    // With quadratic, -10 dB (which is 50/60 = 83% of range) maps to 0.69
    let pos_10 = scale.value_to_position(-10.0, -60.0, 0.0);
    let pos_30 = scale.value_to_position(-30.0, -60.0, 0.0);
    // Values near 0 should be more spread out
    assert!(pos_10 > pos_30);
    // The top portion should take more visual space
    assert!(1.0 - pos_10 < pos_10 - pos_30);
}

#[test]
fn test_icon_paths() {
    assert_eq!(IconName::Play.path(), "icons/play.svg");
    assert_eq!(IconName::Pause.path(), "icons/pause.svg");
    assert_eq!(IconName::Settings.path(), "icons/settings.svg");
}

#[test]
#[allow(deprecated)] // `IconSize::px()` is retained for APIs that require Pixels; this pins its legacy contract.
fn test_icon_sizes() {
    use gpui::px;
    assert_eq!(IconSize::Xs.px(), px(12.0));
    assert_eq!(IconSize::Sm.px(), px(16.0));
    assert_eq!(IconSize::Md.px(), px(20.0));
    assert_eq!(IconSize::Lg.px(), px(24.0));
    assert_eq!(IconSize::Xl.px(), px(32.0));
}

#[test]
fn test_icon_sizes_to_rems() {
    use gpui::rems;
    // Rem-based sizing is the primary API — it scales with `window.rem_size`
    // driven by IncreaseFontSize / DecreaseFontSize. Values are chosen so that
    // at the default 16px rem baseline they match the absolute-pixel contract
    // above (0.75 rem * 16 = 12px, etc.).
    assert_eq!(IconSize::Xs.to_rems(), rems(0.75));
    assert_eq!(IconSize::Sm.to_rems(), rems(1.0));
    assert_eq!(IconSize::Md.to_rems(), rems(1.25));
    assert_eq!(IconSize::Lg.to_rems(), rems(1.5));
    assert_eq!(IconSize::Xl.to_rems(), rems(2.0));
    assert_eq!(IconSize::Xxl.to_rems(), rems(2.25));
}

#[test]
fn test_pagination_reference_window() {
    let (cols, rows) = estimate_grid_dimensions(1200.0, 800.0, 1.0, None, None);
    assert!((5..=8).contains(&cols), "expected 5-8 columns, got {cols}");
    assert!((1..=4).contains(&rows), "expected 1-4 rows, got {rows}");
}

#[test]
fn test_pagination_phone_window() {
    let (cols, rows) = estimate_grid_dimensions(375.0, 667.0, 1.0, None, None);
    assert!((1..=5).contains(&cols), "expected 1-5 columns, got {cols}");
    assert!(rows >= 1, "expected at least 1 row, got {rows}");
}

#[test]
fn test_pagination_4k_window() {
    let (cols, rows) = estimate_grid_dimensions(3840.0, 2160.0, 1.0, None, None);
    assert!(cols >= 8, "expected >=8 columns on 4K, got {cols}");
    assert!(rows >= 2, "expected >=2 rows on 4K, got {rows}");
}

#[test]
#[allow(deprecated)] // Cross-checks the legacy `px()` contract against `to_rems()` at the base rem size.
fn test_icon_size_to_rems_matches_px_at_base_rem() {
    let base_rem = 16.0_f32;
    let variants = [
        IconSize::Xs,
        IconSize::Sm,
        IconSize::Md,
        IconSize::Lg,
        IconSize::Xl,
        IconSize::Xxl,
    ];
    for size in variants {
        let from_rems = size.to_rems().0 * base_rem;
        let from_px: f32 = size.px().into();
        assert!(
            (from_rems - from_px).abs() < 0.01,
            "{size:?}: to_rems gives {from_rems}px but px() gives {from_px}px"
        );
    }
}

/// Regression test: preferences changed through the settings UI (language) and
/// the tutorial's "don't show again" checkbox must survive a save/load cycle.
/// Both handlers persist via `App::save_config` immediately when they fire;
/// this pins the state -> Config -> disk -> Config roundtrip they rely on.
#[test]
fn test_save_and_load_config_persists_language_and_tutorial_flag() {
    use sotf_audio_player::config::set_config_dir_override;
    use sotf_audio_player_gpui::app::state::ui::LayoutState;
    use sotf_audio_player_gpui::i18n::Language;
    use sotf_audio_player_gpui::{App, Config};

    let dir = std::env::temp_dir().join(format!("sotf-config-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    set_config_dir_override(dir.clone());

    let mut app = App::new();
    // Use a non-default language so the assertion proves a real write/read
    // roundtrip (Config::load falls back to Language::default() == English
    // when the state file was never written).
    app.set_language(Language::French);
    app.tutorial.completed = true;
    app.save_config(&LayoutState::default()).unwrap();

    let loaded = Config::load().unwrap();
    assert_eq!(loaded.language, Language::French);
    assert!(loaded.tutorial_completed);

    let _ = std::fs::remove_dir_all(&dir);
}
