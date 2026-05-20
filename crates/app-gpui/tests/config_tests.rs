//! Config and component tests for GPUI App.
//!
//! These tests verify config serialization, playback state defaults,
//! image cache behavior, tick scale calculations, and icon properties.
//! Extracted from inline tests to work around GPUI macro recursion issues.

use sotf_audio_player_gpui::{
    Config, IconName, IconSize, ImageAccessTracker, PanelLayout, PlaybackDeviceConfig,
    PlaybackState, RecordingConfigState, RecordingDeviceConfig, RecordingSignalType, ScaleType,
    WindowGeometry, compute_responsive_scale, default_volume, estimate_grid_dimensions,
};

use std::path::PathBuf;

// ============================================================================
// Config Tests (from app/config.rs)
// ============================================================================

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
}

#[test]
fn test_panel_layout_serialization() {
    let layout = PanelLayout {
        queue_ratio: 0.5,
        meters_ratio: 0.3,
        queue_list_ratio: 0.4,
        lufs_ratio: 0.2,
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
    use sotf_audio_player::ReleaseChannel;
    use sotf_audio_player_gpui::i18n::Language;
    use sotf_audio_player_gpui::keybindings::KeymapPreset;
    use sotf_audio_player_gpui::theme::ThemeId;

    let config = Config {
        directories: Vec::new(),
        last_loaded_plugin_preset: Some("test_preset".to_string()),
        theme: ThemeId::default(),
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
}

#[test]
fn test_config_default_volume() {
    assert!((default_volume() - 0.1).abs() < 0.001);
}

// ============================================================================
// PlaybackState Tests (from app/state/playback.rs)
// ============================================================================

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

// ============================================================================
// ImageAccessTracker Tests (from components/home/image_cache.rs)
// ============================================================================

const MAX_CACHE_SIZE: usize = 200;

#[test]
fn test_tracker_creation() {
    let tracker = ImageAccessTracker::new();
    assert_eq!(tracker.stats().tracked, 0);
    assert_eq!(tracker.stats().capacity, MAX_CACHE_SIZE);
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

// ============================================================================
// ScaleType Tests (from components/plugins/ticks.rs)
// ============================================================================

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

// ============================================================================
// Icon Tests (from components/icons/mod.rs)
// ============================================================================

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

// ============================================================================
// UI Responsive Scale Tests (from ui/mod.rs)
// ============================================================================

fn assert_f32_eq(actual: f32, expected: f32) {
    assert!(
        (actual - expected).abs() < f32::EPSILON,
        "expected {expected}, got {actual}"
    );
}

#[test]
fn test_compute_responsive_scale_reference_size() {
    assert_f32_eq(compute_responsive_scale(1200.0, 800.0), 1.0);
}

#[test]
fn test_compute_responsive_scale_min_clamp() {
    assert_f32_eq(compute_responsive_scale(100.0, 100.0), 0.55);
}

#[test]
fn test_compute_responsive_scale_max_clamp() {
    assert_f32_eq(compute_responsive_scale(3840.0, 2160.0), 2.5);
}

#[test]
fn test_compute_responsive_scale_uses_constraining_axis() {
    assert_f32_eq(compute_responsive_scale(2400.0, 400.0), 0.55);
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
