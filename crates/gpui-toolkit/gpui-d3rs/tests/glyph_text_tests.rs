use d3rs::text::{
    measure_glyph_text, measure_glyph_text_width, render_glyph_text, render_glyph_text_anchored,
    GlyphTextConfig, HorizontalTextAnchor, VerticalTextAnchor,
};

#[test]
fn text_metrics_are_nonzero() {
    let metrics = measure_glyph_text("1000 Hz", 12.0);
    assert!(metrics.width > 0.0);
    assert!(metrics.height > 0.0);
}

#[test]
fn horizontal_width_measurement_is_stable() {
    let direct_width = measure_glyph_text_width("500", 12.0);
    let metrics_width = measure_glyph_text("500", 12.0).width;

    assert_eq!(direct_width, metrics_width);
}

#[test]
fn rotated_text_config_keeps_font_size_independent() {
    let config = GlyphTextConfig::rotated(14.0, gpui::rgb(0xffffff), std::f32::consts::FRAC_PI_2);
    assert_eq!(config.font_size, 14.0);
    assert_eq!(config.rotation, std::f32::consts::FRAC_PI_2);
}

#[test]
fn anchored_text_element_builds() {
    let config = GlyphTextConfig::horizontal(12.0, gpui::rgb(0xffffff));
    let _element = render_glyph_text_anchored(
        "1000 Hz",
        &config,
        HorizontalTextAnchor::Middle,
        VerticalTextAnchor::Alphabetic,
    );
}

#[test]
fn plain_horizontal_text_element_builds() {
    let config = GlyphTextConfig::horizontal(12.0, gpui::rgb(0xffffff));
    let _element = render_glyph_text("20k", &config);
}
