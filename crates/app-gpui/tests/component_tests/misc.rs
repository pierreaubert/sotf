use gpui_design::DesignSystem;
use sotf_audio_player_gpui::components::design::typography_rems_from_rules;
use sotf_audio_player_gpui::components::graphs::common::rgba_to_u32;

fn assert_rem_close(actual: f32, expected: f32) {
    assert!(
        (actual - expected).abs() < 0.0001,
        "expected {expected}, got {actual}"
    );
}

#[test]
fn test_typography_rems_neutral_preserves_existing_scale() {
    let typography = typography_rems_from_rules(&DesignSystem::neutral().typography);

    assert_rem_close(typography.text_xs.0, 0.625);
    assert_rem_close(typography.text_sm.0, 0.875);
    assert_rem_close(typography.text_base.0, 1.0);
    assert_rem_close(typography.text_lg.0, 1.125);
    assert_rem_close(typography.text_xl.0, 1.25);
    assert_rem_close(typography.text_xxl.0, 1.5);
}

fn rgba(r: f32, g: f32, b: f32) -> gpui::Rgba {
    gpui::Rgba { r, g, b, a: 1.0 }
}

#[test]
fn test_rgba_to_u32_red() {
    assert_eq!(rgba_to_u32(rgba(1.0, 0.0, 0.0)), 0xFF0000);
}

#[test]
fn test_rgba_to_u32_green() {
    assert_eq!(rgba_to_u32(rgba(0.0, 1.0, 0.0)), 0x00FF00);
}

#[test]
fn test_rgba_to_u32_blue() {
    assert_eq!(rgba_to_u32(rgba(0.0, 0.0, 1.0)), 0x0000FF);
}

#[test]
fn test_rgba_to_u32_white() {
    assert_eq!(rgba_to_u32(rgba(1.0, 1.0, 1.0)), 0xFFFFFF);
}

#[test]
fn test_rgba_to_u32_black() {
    assert_eq!(rgba_to_u32(rgba(0.0, 0.0, 0.0)), 0x000000);
}

#[test]
fn test_rgba_to_u32_gray() {
    // 0.5 * 255 = 127 (0x7F)
    assert_eq!(rgba_to_u32(rgba(0.5, 0.5, 0.5)), 0x7F7F7F);
}
