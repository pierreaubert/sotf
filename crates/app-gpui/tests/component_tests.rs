//! Component Tests for GPUI App
//!
//! Tests for component logic extracted from the UI layer.
//! Since the lib has `test = false` due to GPUI macro recursion issues,
//! these tests verify pure functions by mirroring the logic.
//!
//! Based on GPUI_TESTING_GUIDE.md recommendations:
//! - Unit (Logic): State transitions, data binding, business logic
//! - Component (UI): Builder verification, prop propagation
//! - Edge Cases: Empty data, boundary values, invalid inputs

mod common;

use common::*;
use std::path::{Path, PathBuf};

// ============================================================================
// ALBUM CARD TESTS
// ============================================================================

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
    let result = format_sample_info(Some(16), Some(44100));
    assert_eq!(result, Some("16/44.1k".to_string()));
}

#[test]
fn test_format_sample_info_hires_96k() {
    let result = format_sample_info(Some(24), Some(96000));
    assert_eq!(result, Some("24/96k".to_string()));
}

#[test]
fn test_format_sample_info_48k() {
    let result = format_sample_info(Some(24), Some(48000));
    assert_eq!(result, Some("24/48k".to_string()));
}

#[test]
fn test_format_sample_info_192k() {
    let result = format_sample_info(Some(32), Some(192000));
    assert_eq!(result, Some("32/192k".to_string()));
}

#[test]
fn test_format_sample_info_bit_depth_only() {
    let result = format_sample_info(Some(24), None);
    assert_eq!(result, Some("24bit".to_string()));
}

#[test]
fn test_format_sample_info_sample_rate_only() {
    let result = format_sample_info(None, Some(44100));
    assert_eq!(result, Some("44.1k".to_string()));
}

#[test]
fn test_format_sample_info_none() {
    let result = format_sample_info(None, None);
    assert_eq!(result, None);
}

#[test]
fn test_get_format_flac() {
    let path = Path::new("/music/album/track.flac");
    assert_eq!(get_format_from_path(path), Some("FLAC".to_string()));
}

#[test]
fn test_get_format_mp3() {
    let path = Path::new("/music/album/track.mp3");
    assert_eq!(get_format_from_path(path), Some("MP3".to_string()));
}

#[test]
fn test_get_format_wav() {
    let path = Path::new("/music/album/track.wav");
    assert_eq!(get_format_from_path(path), Some("WAV".to_string()));
}

#[test]
fn test_get_format_m4a() {
    let path = Path::new("/music/album/track.m4a");
    assert_eq!(get_format_from_path(path), Some("M4A".to_string()));
}

#[test]
fn test_get_format_ogg() {
    let path = Path::new("/music/album/track.ogg");
    assert_eq!(get_format_from_path(path), Some("OGG".to_string()));
}

#[test]
fn test_get_format_no_extension() {
    let path = Path::new("/music/album/track");
    assert_eq!(get_format_from_path(path), None);
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

// ============================================================================
// PLUGIN COMMON TESTS (format_shortcut_label)
// ============================================================================

#[test]
fn test_format_shortcut_label_first_letter() {
    let result = format_shortcut_label("Threshold", Some('t'));
    assert_eq!(result, "[T]hreshold");
}

#[test]
fn test_format_shortcut_label_middle_letter() {
    let result = format_shortcut_label("Gain", Some('a'));
    assert_eq!(result, "G[A]in");
}

#[test]
fn test_format_shortcut_label_not_found() {
    let result = format_shortcut_label("Gain", Some('x'));
    assert_eq!(result, "[X] Gain");
}

#[test]
fn test_format_shortcut_label_case_insensitive() {
    let result = format_shortcut_label("RATIO", Some('r'));
    assert_eq!(result, "[R]ATIO");
}

#[test]
fn test_format_shortcut_label_none() {
    let result = format_shortcut_label("Attack", None);
    assert_eq!(result, "Attack");
}

// ============================================================================
// ALBUM CARD MODE TESTS
// ============================================================================

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

// ============================================================================
// TEST HELPER TESTS
// ============================================================================

#[test]
fn test_create_test_track() {
    let track = create_test_track("/test/track.flac", Some(24), Some(96000));
    assert_eq!(track.path, PathBuf::from("/test/track.flac"));
    assert_eq!(track.bit_depth, Some(24));
    assert_eq!(track.sample_rate, Some(96000));
}

#[test]
fn test_create_test_album() {
    let tracks = vec![
        create_test_track("/test/track1.flac", Some(24), Some(96000)),
        create_test_track("/test/track2.flac", Some(24), Some(96000)),
    ];
    let album = create_test_album("Test Album", tracks, Some(14.0));
    assert_eq!(album.title, "Test Album");
    assert_eq!(album.tracks.len(), 2);
    assert_eq!(album.dynamic_range, Some(14.0));
}

// ============================================================================
// EDGE CASE TESTS
// ============================================================================

#[test]
fn test_format_sample_info_very_high_sample_rate() {
    // DSD64 equivalent
    let result = format_sample_info(Some(1), Some(2822400));
    assert_eq!(result, Some("1/2822.4k".to_string()));
}

#[test]
fn test_format_sample_info_zero_values() {
    let result = format_sample_info(Some(0), Some(0));
    assert_eq!(result, Some("0/0k".to_string()));
}

#[test]
fn test_format_dr_zero() {
    assert_eq!(format_dr(Some(0.0)), Some("0".to_string()));
}

#[test]
fn test_format_dr_negative() {
    // Shouldn't happen in practice but test boundary
    assert_eq!(format_dr(Some(-5.0)), Some("-5".to_string()));
}

#[test]
fn test_get_format_uppercase_extension() {
    let path = Path::new("/music/album/track.FLAC");
    assert_eq!(get_format_from_path(path), Some("FLAC".to_string()));
}

#[test]
fn test_get_format_mixed_case() {
    let path = Path::new("/music/album/track.FlAc");
    assert_eq!(get_format_from_path(path), Some("FLAC".to_string()));
}

#[test]
fn test_format_shortcut_label_empty_string() {
    let result = format_shortcut_label("", Some('a'));
    assert_eq!(result, "[A] ");
}

#[test]
fn test_format_shortcut_label_single_char() {
    let result = format_shortcut_label("G", Some('g'));
    assert_eq!(result, "[G]");
}

// ============================================================================
// SAMPLE RATE BOUNDARY TESTS
// ============================================================================

#[test]
fn test_format_sample_info_common_rates() {
    // Test all common sample rates
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
        let result = format_sample_info(Some(24), Some(rate));
        assert_eq!(
            result,
            Some(format!("24/{}", expected)),
            "Failed for rate {}",
            rate
        );
    }
}

// ============================================================================
// TRANSFER CURVE TESTS (Compressor/Limiter)
// ============================================================================

#[test]
fn test_limiter_clips_at_threshold() {
    // Limiter should clip anything above threshold
    let result = calculate_transfer_output(-10.0, -20.0, 10.0, 0.0, true);
    assert_eq!(result, -20.0);
}

#[test]
fn test_limiter_passes_below_threshold() {
    // Limiter should pass signals below threshold
    let result = calculate_transfer_output(-30.0, -20.0, 10.0, 0.0, true);
    assert_eq!(result, -30.0);
}

#[test]
fn test_compressor_linear_region() {
    // Below threshold - knee/2, compressor should be linear
    let result = calculate_transfer_output(-50.0, -20.0, 4.0, 6.0, false);
    assert!((result - (-50.0)).abs() < 0.001);
}

#[test]
fn test_compressor_compression_region() {
    // Well above threshold, compressor should reduce by ratio
    let result = calculate_transfer_output(-10.0, -20.0, 4.0, 0.0, false);
    // Expected: threshold + (input - threshold) / ratio = -20 + (-10 - (-20)) / 4 = -20 + 2.5 = -17.5
    assert!((result - (-17.5)).abs() < 0.001);
}

#[test]
fn test_compressor_knee_region() {
    // In the knee region, output should be between linear and compressed
    let result = calculate_transfer_output(-20.0, -20.0, 4.0, 6.0, false);
    // At threshold, with knee, output should be slightly less than input
    assert!(result <= -20.0);
    assert!(result >= -25.0);
}

// ============================================================================
// THEME TESTS
// ============================================================================

#[test]
fn test_all_themes_have_names() {
    for theme_id in ThemeId::all() {
        let name = theme_id.name();
        assert!(!name.is_empty(), "Theme {:?} has empty name", theme_id);
    }
}

#[test]
fn test_theme_count() {
    assert_eq!(ThemeId::all().len(), 10);
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

// ============================================================================
// LANGUAGE TESTS
// ============================================================================

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
    assert_eq!(Language::all().len(), 8);
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

// ============================================================================
// PARAMETER NORMALIZATION TESTS
// ============================================================================

#[test]
fn test_normalize_parameter_min() {
    let result = normalize_parameter(-60.0, -60.0, 0.0);
    assert!((result - 0.0).abs() < 0.001);
}

#[test]
fn test_normalize_parameter_max() {
    let result = normalize_parameter(0.0, -60.0, 0.0);
    assert!((result - 1.0).abs() < 0.001);
}

#[test]
fn test_normalize_parameter_mid() {
    let result = normalize_parameter(-30.0, -60.0, 0.0);
    assert!((result - 0.5).abs() < 0.001);
}

#[test]
fn test_denormalize_parameter_min() {
    let result = denormalize_parameter(0.0, -60.0, 0.0);
    assert!((result - (-60.0)).abs() < 0.001);
}

#[test]
fn test_denormalize_parameter_max() {
    let result = denormalize_parameter(1.0, -60.0, 0.0);
    assert!((result - 0.0).abs() < 0.001);
}

#[test]
fn test_denormalize_parameter_mid() {
    let result = denormalize_parameter(0.5, -60.0, 0.0);
    assert!((result - (-30.0)).abs() < 0.001);
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
    let result = clamp_parameter(-30.0, -60.0, 0.0);
    assert!((result - (-30.0)).abs() < 0.001);
}

#[test]
fn test_clamp_parameter_below_min() {
    let result = clamp_parameter(-100.0, -60.0, 0.0);
    assert!((result - (-60.0)).abs() < 0.001);
}

#[test]
fn test_clamp_parameter_above_max() {
    let result = clamp_parameter(10.0, -60.0, 0.0);
    assert!((result - 0.0).abs() < 0.001);
}

// ============================================================================
// LOGARITHMIC PARAMETER TESTS (for Hz parameters)
// ============================================================================

#[test]
fn test_normalize_log_min() {
    let result = normalize_parameter_log(20.0, 20.0, 20000.0);
    assert!((result - 0.0).abs() < 0.001);
}

#[test]
fn test_normalize_log_max() {
    let result = normalize_parameter_log(20000.0, 20.0, 20000.0);
    assert!((result - 1.0).abs() < 0.001);
}

#[test]
fn test_denormalize_log_min() {
    let result = denormalize_parameter_log(0.0, 20.0, 20000.0);
    assert!((result - 20.0).abs() < 0.1);
}

#[test]
fn test_denormalize_log_max() {
    let result = denormalize_parameter_log(1.0, 20.0, 20000.0);
    assert!((result - 20000.0).abs() < 1.0);
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

// ============================================================================
// PLUGIN PARAMETER SPEC TESTS (Boundary Validation)
// ============================================================================

/// Parameter spec constants (mirrors param_specs.rs)
mod specs {
    // Gain
    pub const GAIN_DB_MIN: f32 = -60.0;
    pub const GAIN_DB_MAX: f32 = 20.0;

    // Compressor
    pub const COMP_THRESHOLD_MIN: f32 = -60.0;
    pub const COMP_THRESHOLD_MAX: f32 = 0.0;
    pub const COMP_RATIO_MIN: f32 = 1.0;
    pub const COMP_RATIO_MAX: f32 = 20.0;
    pub const COMP_ATTACK_MIN: f32 = 0.1;
    pub const COMP_ATTACK_MAX: f32 = 100.0;

    // EQ
    pub const EQ_FREQ_MIN: f64 = 20.0;
    pub const EQ_FREQ_MAX: f64 = 20000.0;
    pub const EQ_Q_MIN: f64 = 0.1;
    pub const EQ_Q_MAX: f64 = 10.0;
    pub const EQ_GAIN_MIN: f64 = -24.0;
    pub const EQ_GAIN_MAX: f64 = 24.0;
}

#[test]
fn test_gain_range_valid() {
    assert!(specs::GAIN_DB_MIN < specs::GAIN_DB_MAX);
    assert!(specs::GAIN_DB_MIN < 0.0); // Supports attenuation
    assert!(specs::GAIN_DB_MAX > 0.0); // Supports boost
}

#[test]
fn test_compressor_threshold_range_valid() {
    assert!(specs::COMP_THRESHOLD_MIN < specs::COMP_THRESHOLD_MAX);
    assert_eq!(specs::COMP_THRESHOLD_MAX, 0.0); // Max is unity
}

#[test]
fn test_compressor_ratio_range_valid() {
    assert!(specs::COMP_RATIO_MIN >= 1.0); // Ratio must be >= 1
    assert!(specs::COMP_RATIO_MAX > specs::COMP_RATIO_MIN);
}

#[test]
fn test_compressor_attack_range_valid() {
    assert!(specs::COMP_ATTACK_MIN > 0.0); // Must be positive
    assert!(specs::COMP_ATTACK_MAX > specs::COMP_ATTACK_MIN);
}

#[test]
fn test_eq_frequency_range_covers_audible() {
    assert!(specs::EQ_FREQ_MIN <= 20.0); // At least 20 Hz
    assert!(specs::EQ_FREQ_MAX >= 20000.0); // At least 20 kHz
}

#[test]
fn test_eq_q_range_valid() {
    assert!(specs::EQ_Q_MIN > 0.0); // Q must be positive
    assert!(specs::EQ_Q_MAX > specs::EQ_Q_MIN);
}

#[test]
fn test_eq_gain_symmetric() {
    // EQ gain should be symmetric around 0
    assert!((specs::EQ_GAIN_MIN + specs::EQ_GAIN_MAX).abs() < 0.001);
}

// ============================================================================
// RGBA / COLOR TESTS
// ============================================================================

#[test]
fn test_rgba_to_u32_red() {
    let red = Rgba::rgb(1.0, 0.0, 0.0);
    assert_eq!(rgba_to_u32(red), 0xFF0000);
}

#[test]
fn test_rgba_to_u32_green() {
    let green = Rgba::rgb(0.0, 1.0, 0.0);
    assert_eq!(rgba_to_u32(green), 0x00FF00);
}

#[test]
fn test_rgba_to_u32_blue() {
    let blue = Rgba::rgb(0.0, 0.0, 1.0);
    assert_eq!(rgba_to_u32(blue), 0x0000FF);
}

#[test]
fn test_rgba_to_u32_white() {
    let white = Rgba::rgb(1.0, 1.0, 1.0);
    assert_eq!(rgba_to_u32(white), 0xFFFFFF);
}

#[test]
fn test_rgba_to_u32_black() {
    let black = Rgba::rgb(0.0, 0.0, 0.0);
    assert_eq!(rgba_to_u32(black), 0x000000);
}

#[test]
fn test_rgba_to_u32_gray() {
    let gray = Rgba::rgb(0.5, 0.5, 0.5);
    let result = rgba_to_u32(gray);
    // 0.5 * 255 = 127 (0x7F)
    assert_eq!(result, 0x7F7F7F);
}

#[test]
fn test_with_alpha_preserves_rgb() {
    let original = Rgba::new(0.5, 0.6, 0.7, 1.0);
    let modified = with_alpha(original, 0.3);
    assert!((modified.r - 0.5).abs() < 0.001);
    assert!((modified.g - 0.6).abs() < 0.001);
    assert!((modified.b - 0.7).abs() < 0.001);
    assert!((modified.a - 0.3).abs() < 0.001);
}

#[test]
fn test_rgba_new() {
    let color = Rgba::new(0.1, 0.2, 0.3, 0.4);
    assert!((color.r - 0.1).abs() < 0.001);
    assert!((color.g - 0.2).abs() < 0.001);
    assert!((color.b - 0.3).abs() < 0.001);
    assert!((color.a - 0.4).abs() < 0.001);
}

#[test]
fn test_rgba_rgb_default_alpha() {
    let color = Rgba::rgb(0.5, 0.5, 0.5);
    assert!((color.a - 1.0).abs() < 0.001);
}

// ============================================================================
// SETTINGS TAB TESTS
// ============================================================================

#[test]
fn test_settings_tab_count() {
    assert_eq!(SettingsTab::all().len(), 5);
}

#[test]
fn test_settings_tab_uniqueness() {
    let tabs = SettingsTab::all();
    for (i, tab1) in tabs.iter().enumerate() {
        for (j, tab2) in tabs.iter().enumerate() {
            if i != j {
                assert_ne!(tab1, tab2);
            }
        }
    }
}

// ============================================================================
// SCREEN TESTS
// ============================================================================

#[test]
fn test_screen_count() {
    assert_eq!(Screen::all().len(), 7);
}

#[test]
fn test_screen_uniqueness() {
    let screens = Screen::all();
    for (i, screen1) in screens.iter().enumerate() {
        for (j, screen2) in screens.iter().enumerate() {
            if i != j {
                assert_ne!(screen1, screen2);
            }
        }
    }
}

#[test]
fn test_screen_includes_library() {
    assert!(Screen::all().contains(&Screen::Library));
}

#[test]
fn test_screen_includes_settings() {
    assert!(Screen::all().contains(&Screen::Settings));
}

#[test]
fn test_screen_includes_plugins() {
    assert!(Screen::all().contains(&Screen::Plugins));
}

// ============================================================================
// INTEGRATION TESTS - Cross-Component Logic
// ============================================================================

#[test]
fn test_sample_info_and_format_together() {
    // Test that format and sample info work together for metadata display
    let path = Path::new("/music/album/track.flac");
    let format = get_format_from_path(path);
    let sample_info = format_sample_info(Some(24), Some(96000));

    assert_eq!(format, Some("FLAC".to_string()));
    assert_eq!(sample_info, Some("24/96k".to_string()));

    // Simulating how the UI would build the metadata string
    let metadata = format!(
        "{} {}",
        format.unwrap_or_default(),
        sample_info.unwrap_or_default()
    );
    assert_eq!(metadata, "FLAC 24/96k");
}

#[test]
fn test_normalization_for_slider() {
    // Test the workflow of a parameter slider
    let min = -60.0;
    let max = 0.0;
    let default_db = -20.0;

    // Calculate normalized position for slider
    let normalized = normalize_parameter(default_db, min, max);
    assert!((normalized - 0.6666).abs() < 0.01);

    // User moves slider to 75% position
    let user_position = 0.75;
    let new_value = denormalize_parameter(user_position, min, max);
    assert!((new_value - (-15.0)).abs() < 0.001);
}

#[test]
fn test_log_normalization_for_frequency() {
    // Test log normalization for frequency knobs
    let min = 20.0;
    let max = 20000.0;

    // 1 kHz should be roughly in the middle of a log scale
    let freq_1k = normalize_parameter_log(1000.0, min, max);

    // For log scale: log(1000)/log(20000/20) ≈ 0.57
    assert!(
        freq_1k > 0.4 && freq_1k < 0.7,
        "1kHz should be mid-range on log scale: {}",
        freq_1k
    );
}

#[test]
fn test_compressor_curve_full_range() {
    // Test that compressor behavior is consistent across full range
    let threshold = -20.0;
    let ratio = 4.0;
    let knee = 6.0;

    // Test multiple input levels
    let test_inputs = [-60.0, -40.0, -30.0, -20.0, -10.0, 0.0];

    for &input in &test_inputs {
        let output = calculate_transfer_output(input, threshold, ratio, knee, false);

        // Output should never exceed input (compression doesn't add gain)
        assert!(
            output <= input + 0.001,
            "Compressor output {} should not exceed input {}",
            output,
            input
        );

        // Output should always be finite
        assert!(
            output.is_finite(),
            "Output must be finite for input {}",
            input
        );
    }
}

#[test]
fn test_theme_and_language_consistency() {
    // Verify that themes and languages are properly enumerated
    assert!(
        ThemeId::all().len() >= 5,
        "Should have at least 5 theme options"
    );
    assert!(
        Language::all().len() >= 5,
        "Should have at least 5 language options"
    );

    // Verify English is always available
    assert!(Language::all().contains(&Language::English));
}

#[test]
fn test_dr_formatting_edge_cases() {
    // Dynamic range edge cases
    // Note: Rust uses banker's rounding (round half to even) for {:0}
    assert_eq!(format_dr(Some(0.0)), Some("0".to_string()));
    assert_eq!(format_dr(Some(0.4)), Some("0".to_string())); // Rounds down
    assert_eq!(format_dr(Some(0.5)), Some("0".to_string())); // Banker's rounding: 0.5 -> 0 (even)
    assert_eq!(format_dr(Some(1.5)), Some("2".to_string())); // Banker's rounding: 1.5 -> 2 (even)
    assert_eq!(format_dr(Some(20.0)), Some("20".to_string()));
    assert_eq!(format_dr(None), None);
}

// ============================================================================
// EQ BAND ADD/REMOVE TESTS
// ============================================================================

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
    let count = add_eq_band(&mut filters);
    assert_eq!(count, 1);
    assert_eq!(filters.len(), 1);
    assert_eq!(filters[0].filter_type, FilterType::Peak);
}

#[test]
fn test_add_eq_band_to_existing_list() {
    let mut filters = vec![
        TestEQFilter::new(FilterType::LowShelf, 100.0, 0.7, 3.0),
        TestEQFilter::new(FilterType::HighShelf, 10000.0, 0.7, -2.0),
    ];
    let count = add_eq_band(&mut filters);
    assert_eq!(count, 3);
    assert_eq!(filters.len(), 3);
    // New filter should be at the end
    assert_eq!(filters[2].filter_type, FilterType::Peak);
    assert!((filters[2].frequency - 1000.0).abs() < 0.001);
}

#[test]
fn test_add_multiple_eq_bands() {
    let mut filters = Vec::new();
    add_eq_band(&mut filters);
    add_eq_band(&mut filters);
    add_eq_band(&mut filters);
    assert_eq!(filters.len(), 3);
    // All should be default peak filters
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
    let result = remove_eq_band(&mut filters, 1);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 2);
    assert_eq!(filters.len(), 2);
    // First and last filters should remain
    assert_eq!(filters[0].filter_type, FilterType::LowShelf);
    assert_eq!(filters[1].filter_type, FilterType::HighShelf);
}

#[test]
fn test_remove_eq_band_first() {
    let mut filters = vec![
        TestEQFilter::new(FilterType::LowShelf, 100.0, 0.7, 3.0),
        TestEQFilter::new(FilterType::Peak, 1000.0, 1.0, 0.0),
    ];
    let result = remove_eq_band(&mut filters, 0);
    assert!(result.is_ok());
    assert_eq!(filters.len(), 1);
    assert_eq!(filters[0].filter_type, FilterType::Peak);
}

#[test]
fn test_remove_eq_band_last() {
    let mut filters = vec![
        TestEQFilter::new(FilterType::LowShelf, 100.0, 0.7, 3.0),
        TestEQFilter::new(FilterType::Peak, 1000.0, 1.0, 0.0),
    ];
    let result = remove_eq_band(&mut filters, 1);
    assert!(result.is_ok());
    assert_eq!(filters.len(), 1);
    assert_eq!(filters[0].filter_type, FilterType::LowShelf);
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
    assert_eq!(filters.len(), 2); // No change
}

#[test]
fn test_remove_last_eq_band_fails() {
    let mut filters = vec![TestEQFilter::new(FilterType::Peak, 1000.0, 1.0, 0.0)];
    let result = remove_eq_band(&mut filters, 0);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Cannot remove the last"));
    assert_eq!(filters.len(), 1); // No change
}

#[test]
fn test_validate_eq_filter_valid() {
    let filter = TestEQFilter::new(FilterType::Peak, 1000.0, 1.0, 3.0);
    assert!(validate_eq_filter(&filter).is_ok());
}

#[test]
fn test_validate_eq_filter_default_peak() {
    let filter = TestEQFilter::default_peak();
    assert!(validate_eq_filter(&filter).is_ok());
}

#[test]
fn test_validate_eq_filter_frequency_too_low() {
    let filter = TestEQFilter::new(FilterType::Peak, 10.0, 1.0, 0.0);
    let result = validate_eq_filter(&filter);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Frequency"));
}

#[test]
fn test_validate_eq_filter_frequency_too_high() {
    let filter = TestEQFilter::new(FilterType::Peak, 25000.0, 1.0, 0.0);
    let result = validate_eq_filter(&filter);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Frequency"));
}

#[test]
fn test_validate_eq_filter_frequency_at_limits() {
    // At minimum
    let filter_min = TestEQFilter::new(FilterType::Peak, 20.0, 1.0, 0.0);
    assert!(validate_eq_filter(&filter_min).is_ok());

    // At maximum
    let filter_max = TestEQFilter::new(FilterType::Peak, 20000.0, 1.0, 0.0);
    assert!(validate_eq_filter(&filter_max).is_ok());
}

#[test]
fn test_validate_eq_filter_q_too_low() {
    let filter = TestEQFilter::new(FilterType::Peak, 1000.0, 0.05, 0.0);
    let result = validate_eq_filter(&filter);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Q factor"));
}

#[test]
fn test_validate_eq_filter_q_too_high() {
    let filter = TestEQFilter::new(FilterType::Peak, 1000.0, 15.0, 0.0);
    let result = validate_eq_filter(&filter);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Q factor"));
}

#[test]
fn test_validate_eq_filter_q_negative() {
    let filter = TestEQFilter::new(FilterType::Peak, 1000.0, -1.0, 0.0);
    let result = validate_eq_filter(&filter);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Q factor"));
}

#[test]
fn test_validate_eq_filter_q_at_limits() {
    // At minimum
    let filter_min = TestEQFilter::new(FilterType::Peak, 1000.0, 0.1, 0.0);
    assert!(validate_eq_filter(&filter_min).is_ok());

    // At maximum
    let filter_max = TestEQFilter::new(FilterType::Peak, 1000.0, 10.0, 0.0);
    assert!(validate_eq_filter(&filter_max).is_ok());
}

#[test]
fn test_validate_eq_filter_gain_too_low() {
    let filter = TestEQFilter::new(FilterType::Peak, 1000.0, 1.0, -30.0);
    let result = validate_eq_filter(&filter);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Gain"));
}

#[test]
fn test_validate_eq_filter_gain_too_high() {
    let filter = TestEQFilter::new(FilterType::Peak, 1000.0, 1.0, 30.0);
    let result = validate_eq_filter(&filter);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Gain"));
}

#[test]
fn test_validate_eq_filter_gain_at_limits() {
    // At minimum
    let filter_min = TestEQFilter::new(FilterType::Peak, 1000.0, 1.0, -24.0);
    assert!(validate_eq_filter(&filter_min).is_ok());

    // At maximum
    let filter_max = TestEQFilter::new(FilterType::Peak, 1000.0, 1.0, 24.0);
    assert!(validate_eq_filter(&filter_max).is_ok());
}

#[test]
fn test_eq_filter_types() {
    // Test all filter types can be created
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
        let filter = TestEQFilter::new(filter_type, 1000.0, 1.0, 0.0);
        assert_eq!(filter.filter_type, filter_type);
        assert!(validate_eq_filter(&filter).is_ok());
    }
}

#[test]
fn test_add_and_remove_eq_band_roundtrip() {
    let mut filters = vec![TestEQFilter::new(FilterType::Peak, 500.0, 1.5, 2.0)];

    // Add a band
    add_eq_band(&mut filters);
    assert_eq!(filters.len(), 2);

    // Remove the added band (last one)
    let result = remove_eq_band(&mut filters, 1);
    assert!(result.is_ok());
    assert_eq!(filters.len(), 1);

    // Original filter should be unchanged
    assert!((filters[0].frequency - 500.0).abs() < 0.001);
    assert!((filters[0].q - 1.5).abs() < 0.001);
    assert!((filters[0].gain_db - 2.0).abs() < 0.001);
}

// ============================================================================
// EQ COORDINATE CONVERSION TESTS (mirrored from ui_eq.rs)
// ============================================================================

// Constants matching ui_eq.rs
const MIN_FREQ: f64 = 20.0;
const MAX_FREQ: f64 = 20000.0;
const MIN_GAIN_DB: f64 = -24.0;
const MAX_GAIN_DB: f64 = 24.0;
const CHART_LEFT_MARGIN: f32 = 50.0;
const CHART_TOP_MARGIN: f32 = 0.0; // No actual top offset in gpui-px layout
const CHART_BOTTOM_MARGIN: f32 = 30.0;
const CHART_HEIGHT: f32 = 300.0;
const GPUI_PX_MARGIN_TOP: f32 = 10.0; // gpui-px uses this for plot_height calculation
const Q_BAR_MIN_WIDTH: f32 = 40.0;
const Q_BAR_MAX_WIDTH: f32 = 100.0;
const Q_MIN: f64 = 0.1;
const Q_MAX: f64 = 10.0;

// Mirrored coordinate conversion functions
fn freq_to_x(freq: f64, plot_width: f32) -> f32 {
    let log_min = MIN_FREQ.ln();
    let log_max = MAX_FREQ.ln();
    let log_freq = freq.clamp(MIN_FREQ, MAX_FREQ).ln();
    let t = ((log_freq - log_min) / (log_max - log_min)) as f32;
    CHART_LEFT_MARGIN + t * plot_width
}

fn x_to_freq(x: f32, plot_width: f32) -> f64 {
    let t = ((x - CHART_LEFT_MARGIN) / plot_width).clamp(0.0, 1.0) as f64;
    let log_min = MIN_FREQ.ln();
    let log_max = MAX_FREQ.ln();
    (log_min + t * (log_max - log_min)).exp()
}

fn gain_to_y(gain: f64) -> f32 {
    // gpui-px calculates plot_height = height - margin_top(10) - margin_bottom(30)
    // but renders the plot starting at y=0 (no actual top margin offset)
    let plot_height = CHART_HEIGHT - GPUI_PX_MARGIN_TOP - CHART_BOTTOM_MARGIN;
    let t = ((MAX_GAIN_DB - gain) / (MAX_GAIN_DB - MIN_GAIN_DB)) as f32;
    CHART_TOP_MARGIN + t * plot_height
}

fn y_to_gain(y: f32) -> f64 {
    let plot_height = CHART_HEIGHT - GPUI_PX_MARGIN_TOP - CHART_BOTTOM_MARGIN;
    let t = ((y - CHART_TOP_MARGIN) / plot_height).clamp(0.0, 1.0) as f64;
    MAX_GAIN_DB - t * (MAX_GAIN_DB - MIN_GAIN_DB)
}

fn q_to_bar_width(q: f64) -> f32 {
    let t = ((q - Q_MIN) / (Q_MAX - Q_MIN)).clamp(0.0, 1.0) as f32;
    Q_BAR_MAX_WIDTH - t * (Q_BAR_MAX_WIDTH - Q_BAR_MIN_WIDTH)
}

fn drag_delta_to_q_change(delta_px: f32) -> f64 {
    let scale = (Q_MAX - Q_MIN) / 60.0;
    delta_px as f64 * scale
}

#[test]
fn test_freq_x_roundtrip() {
    let plot_width = 500.0;
    let test_freqs = [20.0, 100.0, 1000.0, 10000.0, 20000.0];

    for &freq in &test_freqs {
        let x = freq_to_x(freq, plot_width);
        let recovered_freq = x_to_freq(x, plot_width);
        let rel_error = (recovered_freq - freq).abs() / freq;
        assert!(
            rel_error < 0.001,
            "freq_to_x/x_to_freq roundtrip failed for freq={}: got {}, error={}",
            freq,
            recovered_freq,
            rel_error
        );
    }
}

#[test]
fn test_gain_y_roundtrip() {
    let test_gains = [-24.0, -12.0, 0.0, 12.0, 24.0];

    for &gain in &test_gains {
        let y = gain_to_y(gain);
        let recovered_gain = y_to_gain(y);
        let abs_error = (recovered_gain - gain).abs();
        assert!(
            abs_error < 0.01,
            "gain_to_y/y_to_gain roundtrip failed for gain={}: got {}, error={}",
            gain,
            recovered_gain,
            abs_error
        );
    }
}

#[test]
fn test_freq_to_x_boundaries() {
    let plot_width = 500.0;

    // MIN_FREQ should map to left margin
    let x_min = freq_to_x(MIN_FREQ, plot_width);
    assert!(
        (x_min - CHART_LEFT_MARGIN).abs() < 0.01,
        "MIN_FREQ should map to left margin: got {} expected {}",
        x_min,
        CHART_LEFT_MARGIN
    );

    // MAX_FREQ should map to left margin + plot_width
    let x_max = freq_to_x(MAX_FREQ, plot_width);
    let expected_max = CHART_LEFT_MARGIN + plot_width;
    assert!(
        (x_max - expected_max).abs() < 0.01,
        "MAX_FREQ should map to right edge: got {} expected {}",
        x_max,
        expected_max
    );
}

#[test]
fn test_gain_to_y_boundaries() {
    // Use GPUI_PX_MARGIN_TOP to match how gain_to_y calculates plot_height
    let plot_height = CHART_HEIGHT - GPUI_PX_MARGIN_TOP - CHART_BOTTOM_MARGIN;

    // MAX_GAIN_DB should map to top margin (which is 0 since gpui-px doesn't render top offset)
    let y_max = gain_to_y(MAX_GAIN_DB);
    assert!(
        (y_max - CHART_TOP_MARGIN).abs() < 0.01,
        "MAX_GAIN_DB should map to top margin: got {} expected {}",
        y_max,
        CHART_TOP_MARGIN
    );

    // MIN_GAIN_DB should map to top margin + plot_height
    let y_min = gain_to_y(MIN_GAIN_DB);
    let expected_min = CHART_TOP_MARGIN + plot_height;
    assert!(
        (y_min - expected_min).abs() < 0.01,
        "MIN_GAIN_DB should map to bottom edge: got {} expected {}",
        y_min,
        expected_min
    );

    // 0 dB should be at vertical center
    let y_zero = gain_to_y(0.0);
    let expected_center = CHART_TOP_MARGIN + plot_height / 2.0;
    assert!(
        (y_zero - expected_center).abs() < 0.01,
        "0 dB should map to vertical center: got {} expected {}",
        y_zero,
        expected_center
    );
}

#[test]
fn test_x_to_freq_clamping() {
    let plot_width = 500.0;

    // X before left margin should clamp to MIN_FREQ
    let freq_before = x_to_freq(0.0, plot_width);
    assert!(
        (freq_before - MIN_FREQ).abs() < 0.01,
        "x before margin should clamp to MIN_FREQ: got {}",
        freq_before
    );

    // X after right edge should clamp to MAX_FREQ
    let freq_after = x_to_freq(CHART_LEFT_MARGIN + plot_width + 100.0, plot_width);
    assert!(
        (freq_after - MAX_FREQ).abs() < 0.01,
        "x after right edge should clamp to MAX_FREQ: got {}",
        freq_after
    );
}

#[test]
fn test_y_to_gain_clamping() {
    // Y before top margin should clamp to MAX_GAIN_DB
    let gain_above = y_to_gain(0.0);
    assert!(
        (gain_above - MAX_GAIN_DB).abs() < 0.01,
        "y above margin should clamp to MAX_GAIN_DB: got {}",
        gain_above
    );

    // Y after bottom edge should clamp to MIN_GAIN_DB
    let gain_below = y_to_gain(CHART_HEIGHT + 100.0);
    assert!(
        (gain_below - MIN_GAIN_DB).abs() < 0.01,
        "y below bottom should clamp to MIN_GAIN_DB: got {}",
        gain_below
    );
}

#[test]
fn test_q_to_bar_width_conversion() {
    // Q_MIN should give maximum width
    let width_at_min_q = q_to_bar_width(Q_MIN);
    assert!(
        (width_at_min_q - Q_BAR_MAX_WIDTH).abs() < 0.01,
        "Q_MIN should give max width: got {} expected {}",
        width_at_min_q,
        Q_BAR_MAX_WIDTH
    );

    // Q_MAX should give minimum width
    let width_at_max_q = q_to_bar_width(Q_MAX);
    assert!(
        (width_at_max_q - Q_BAR_MIN_WIDTH).abs() < 0.01,
        "Q_MAX should give min width: got {} expected {}",
        width_at_max_q,
        Q_BAR_MIN_WIDTH
    );

    // Mid-Q should give mid-width
    let mid_q = (Q_MIN + Q_MAX) / 2.0;
    let mid_width = (Q_BAR_MIN_WIDTH + Q_BAR_MAX_WIDTH) / 2.0;
    let width_at_mid_q = q_to_bar_width(mid_q);
    assert!(
        (width_at_mid_q - mid_width).abs() < 1.0,
        "Mid Q should give mid width: got {} expected ~{}",
        width_at_mid_q,
        mid_width
    );
}

#[test]
fn test_drag_delta_to_q_change_conversion() {
    // Dragging 60px should change Q by the full range
    let full_range_delta = 60.0;
    let q_change = drag_delta_to_q_change(full_range_delta);
    let expected_change = Q_MAX - Q_MIN;

    assert!(
        (q_change - expected_change).abs() < 0.01,
        "60px drag should change Q by full range: got {} expected {}",
        q_change,
        expected_change
    );

    // Negative delta should decrease Q
    let negative_change = drag_delta_to_q_change(-30.0);
    assert!(
        negative_change < 0.0,
        "Negative drag should decrease Q: got {}",
        negative_change
    );
}

#[test]
fn test_freq_logarithmic_scaling() {
    let plot_width = 600.0;

    // Each octave should span equal distance on the plot
    let x_100 = freq_to_x(100.0, plot_width);
    let x_200 = freq_to_x(200.0, plot_width);
    let x_1000 = freq_to_x(1000.0, plot_width);
    let x_2000 = freq_to_x(2000.0, plot_width);

    let octave_width_low = x_200 - x_100;
    let octave_width_high = x_2000 - x_1000;

    // Octaves should be approximately equal width in log scale
    let rel_diff = (octave_width_high - octave_width_low).abs() / octave_width_low;
    assert!(
        rel_diff < 0.01,
        "Octave widths should be equal in log scale: low={} high={} diff={}",
        octave_width_low,
        octave_width_high,
        rel_diff
    );
}
