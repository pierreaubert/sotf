//! Graphite — vintage psychoacoustic instrument.
//!
//! Deep graphite chassis with warm amber calibration accents. Inspired by
//! Bruel & Kjaer field measurement instruments. The default plugin theme.

use super::plugin_theme::PluginTheme;
use gpui::{Rgba, SharedString};

const fn rgba(hex: u32, alpha: f32) -> Rgba {
    Rgba {
        r: ((hex >> 16) & 0xff) as f32 / 255.0,
        g: ((hex >> 8) & 0xff) as f32 / 255.0,
        b: (hex & 0xff) as f32 / 255.0,
        a: alpha,
    }
}

pub fn theme() -> PluginTheme {
    PluginTheme {
        // Surfaces — deep graphite gradient
        chassis_bg_top: rgba(0x14171c, 1.0),
        chassis_bg_bottom: rgba(0x0c0e12, 1.0),
        chassis_border: rgba(0x353a44, 1.0),
        panel_bg: rgba(0x11141a, 1.0),
        panel_recess: rgba(0x07080b, 1.0),
        section_divider: rgba(0x1a1d23, 1.0),
        corner_bracket: rgba(0x353a44, 0.6),

        // Ink — warm off-white scale
        ink_hi: rgba(0xece8df, 1.0),
        ink: rgba(0xc8c3b8, 1.0),
        ink_mid: rgba(0x8a857a, 1.0),
        ink_low: rgba(0x5a564f, 1.0),
        ink_faint: rgba(0x3a3833, 1.0),

        // Accent — calibrated amber
        accent: rgba(0xe5a93d, 1.0),
        accent_bright: rgba(0xffc857, 1.0),
        accent_glow: rgba(0xe5a93d, 0.35),
        accent_arc: rgba(0xe5a93d, 0.92),
        accent_track: rgba(0xe5a93d, 0.06),

        // LED — soft green calibrated indicator
        led_active: rgba(0x62d077, 1.0),
        led_glow: rgba(0x62d077, 0.55),

        // Typography — italic serif for titles, geometric mono for data
        font_display: SharedString::from("Instrument Serif"),
        font_mono: SharedString::from("Geist Mono"),
        font_ui: SharedString::from("Hanken Grotesk"),

        // Dimensions — generous, breathing room
        knob_size_px: 78.0,
        arc_stroke_px: 2.4,
        radius_chassis: 20.0,
        radius_panel: 10.0,
        spacing_section: 48.0,
        spacing_knob_row: 28.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn graphite_has_amber_accent() {
        let t = theme();
        // Amber: r ≈ 0.9, g ≈ 0.66, b ≈ 0.24.
        assert!(
            t.accent.r > 0.8 && t.accent.g > 0.5 && t.accent.b < 0.4,
            "Graphite accent should be amber-orange, got r={} g={} b={}",
            t.accent.r,
            t.accent.g,
            t.accent.b,
        );
    }

    #[test]
    fn graphite_chassis_is_dark() {
        let t = theme();
        // All RGB channels of chassis backgrounds should be < 0.15 (dark).
        assert!(t.chassis_bg_top.r < 0.15);
        assert!(t.chassis_bg_bottom.r < 0.15);
    }
}
