use sotf_audio_player_gpui::{
    ImageAccessTracker, compute_combined_scale, compute_responsive_scale, resolve_sizing_context,
    responsive_scale_reference_size,
};

const MAX_CACHE_SIZE: usize = 200;

#[test]
fn test_tracker_creation() {
    let tracker = ImageAccessTracker::new();
    assert_eq!(tracker.stats().tracked, 0);
    assert_eq!(tracker.stats().capacity, MAX_CACHE_SIZE);
}

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
fn test_compute_responsive_scale_does_not_shrink_small_windows() {
    assert_f32_eq(compute_responsive_scale(100.0, 100.0), 1.0);
}

#[test]
fn test_compute_responsive_scale_does_not_enlarge_large_windows() {
    assert_f32_eq(compute_responsive_scale(3840.0, 2160.0), 1.0);
}

#[test]
fn test_compute_responsive_scale_preserves_reading_size() {
    // Width alone drives the scale: a wide but short window keeps full text
    // size and reflows vertically instead of shrinking the UI (ui.md Phase 1).
    assert_f32_eq(compute_responsive_scale(2400.0, 400.0), 1.0);
    assert_f32_eq(compute_responsive_scale(1200.0, 600.0), 1.0);
    assert_f32_eq(compute_responsive_scale(600.0, 1200.0), 1.0);
}

#[test]
fn typography_is_stable_across_resize_at_every_supported_zoom() {
    for zoom in [1.0, 1.25, 1.5, 2.0] {
        for (width, height) in [
            (320.0, 900.0),
            (700.0, 900.0),
            (1280.0, 600.0),
            (2560.0, 1440.0),
        ] {
            let sizing = resolve_sizing_context(width, height, zoom, None, None);
            assert_f32_eq(sizing.effective_rem, 16.0 * zoom);
            assert_f32_eq(sizing.window_width_rems, width / (16.0 * zoom));
        }
    }
}

#[test]
fn invalid_persisted_sizing_values_have_finite_fallbacks() {
    for invalid in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        let sizing = resolve_sizing_context(invalid, 800.0, invalid, Some(invalid), Some(invalid));
        assert_f32_eq(sizing.effective_rem, 16.0);
        assert_f32_eq(sizing.window_width_rems, 0.0);
    }
}

#[test]
fn transport_breakpoints_use_the_space_after_navigation() {
    assert_f32_eq(
        resolve_sizing_context(700.0, 900.0, 1.0, None, None).desktop_content_width_rems(false),
        43.75,
    );
    assert_f32_eq(
        resolve_sizing_context(1050.0, 900.0, 1.0, None, None).desktop_content_width_rems(false),
        53.625,
    );
    assert_f32_eq(
        resolve_sizing_context(1050.0, 900.0, 2.0, None, None).desktop_content_width_rems(false),
        32.8125,
    );
}

#[test]
fn test_compute_responsive_scale_uses_phone_reference_size() {
    assert_eq!(
        responsive_scale_reference_size(390.0, 844.0),
        (390.0, 844.0)
    );
    assert_eq!(
        responsive_scale_reference_size(844.0, 390.0),
        (844.0, 390.0)
    );
    assert_f32_eq(compute_responsive_scale(390.0, 844.0), 1.0);
    assert_f32_eq(compute_responsive_scale(844.0, 390.0), 1.0);
}

#[test]
fn test_resolve_sizing_context_uses_configured_font_bounds() {
    // The shared sizing context must honor configured font bounds (not the
    // defaults): a tight max bound shrinks the rem denominator and therefore
    // grows the rem-measured window width.
    let tight = resolve_sizing_context(1200.0, 800.0, 2.0, Some(8.0), Some(16.0));
    let loose = resolve_sizing_context(1200.0, 800.0, 2.0, Some(8.0), Some(32.0));
    assert!(tight.window_width_rems > loose.window_width_rems);
    assert_f32_eq(tight.combined_scale, tight.effective_rem / 16.0);
    assert_f32_eq(loose.window_width_rems, 1200.0 / loose.effective_rem);
}

#[test]
fn test_compute_combined_scale_applies_zoom_and_font_bounds() {
    assert_f32_eq(
        compute_combined_scale(1200.0, 800.0, 1.5, Some(8.0), Some(32.0)),
        1.5,
    );
    assert_f32_eq(
        compute_combined_scale(1200.0, 800.0, 4.0, Some(8.0), Some(24.0)),
        1.5,
    );
    assert_f32_eq(
        compute_combined_scale(1200.0, 800.0, 0.1, Some(12.0), Some(32.0)),
        0.75,
    );
}
