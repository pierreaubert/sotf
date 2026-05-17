use d3rs::text::{GlyphTextConfig, measure_glyph_text};

#[test]
fn text_metrics_are_nonzero() {
    let metrics = measure_glyph_text("1000 Hz", 12.0);
    assert!(metrics.width > 0.0);
    assert!(metrics.height > 0.0);
}

#[test]
fn rotated_text_config_keeps_font_size_independent() {
    let config = GlyphTextConfig::rotated(14.0, gpui::rgb(0xffffff), std::f32::consts::FRAC_PI_2);
    assert_eq!(config.font_size, 14.0);
    assert_eq!(config.rotation, std::f32::consts::FRAC_PI_2);
}
