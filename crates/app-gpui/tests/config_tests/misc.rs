use sotf_audio_player_gpui::{ImageAccessTracker, compute_responsive_scale};

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
