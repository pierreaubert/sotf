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
