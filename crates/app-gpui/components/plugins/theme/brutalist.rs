//! Brutalist — high-contrast monochrome.
//!
//! Pure black, pure white, hard rules, no gradients. Function over feeling.
//! Useful for accessibility, projector use, and screenshots.

use super::plugin_theme::PluginTheme;
use gpui::{Rgba, SharedString};
use gpui_ui_kit::audio_design_tokens::AudioDesignTokens;

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
        // Surfaces — flat black, hard white border
        chassis_bg_top: rgba(0x000000, 1.0),
        chassis_bg_bottom: rgba(0x000000, 1.0),
        chassis_border: rgba(0xffffff, 1.0),
        panel_bg: rgba(0x000000, 1.0),
        panel_recess: rgba(0x0a0a0a, 1.0),
        section_divider: rgba(0xffffff, 1.0),
        corner_bracket: rgba(0xffffff, 1.0),

        // Ink — no greys, just on/off
        ink_hi: rgba(0xffffff, 1.0),
        ink: rgba(0xffffff, 1.0),
        ink_mid: rgba(0xffffff, 0.7),
        ink_low: rgba(0xffffff, 0.4),
        ink_faint: rgba(0xffffff, 0.2),

        // Accent — same white. The "accent" expressiveness comes from
        // motion + stroke weight, not color.
        accent: rgba(0xffffff, 1.0),
        accent_bright: rgba(0xffffff, 1.0),
        accent_glow: rgba(0xffffff, 0.0), // no glow — brutalist
        accent_arc: rgba(0xffffff, 1.0),
        accent_track: rgba(0xffffff, 0.15),

        // LED — also white. Active state shown as solid; off as outlined.
        led_active: rgba(0xffffff, 1.0),
        led_glow: rgba(0xffffff, 0.0),

        // Typography — heavy display sans, monospace, single weight family
        font_display: SharedString::from("Archivo Black"),
        font_mono: SharedString::from("IBM Plex Mono"),
        font_ui: SharedString::from("Archivo"),

        // Dimensions — dense, hard edges
        knob_size_px: 72.0,
        arc_stroke_px: 3.0,
        radius_chassis: 0.0, // sharp corners
        radius_panel: 0.0,
        spacing_section: 32.0,
        spacing_knob_row: 24.0,

        // Brutalist look tokens — strip the chassis from both knobs and
        // sliders, light up the value arc and the meter fill with a glow,
        // and let meter fills be square-cornered with a luminance gradient.
        knob_label_style: AudioDesignTokens::LABEL_UNDERLINED,
        knob_arc_glow: 1.0,
        meter_label_style: AudioDesignTokens::LABEL_UNDERLINED,
        meter_use_gradient: true,
        meter_corner_radius: 0.0,
        meter_glow: 1.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn brutalist_chassis_is_pure_black() {
        let t = theme();
        assert_eq!(t.chassis_bg_top.r, 0.0);
        assert_eq!(t.chassis_bg_top.g, 0.0);
        assert_eq!(t.chassis_bg_top.b, 0.0);
    }

    #[test]
    fn brutalist_has_zero_corner_radius() {
        let t = theme();
        assert_eq!(t.radius_chassis, 0.0);
        assert_eq!(t.radius_panel, 0.0);
    }

    #[test]
    fn brutalist_accent_is_white() {
        let t = theme();
        assert_eq!(t.accent_arc.r, 1.0);
        assert_eq!(t.accent_arc.g, 1.0);
        assert_eq!(t.accent_arc.b, 1.0);
    }

    #[test]
    fn brutalist_audio_tokens() {
        let t = theme();
        assert_eq!(t.knob_label_style, AudioDesignTokens::LABEL_UNDERLINED);
        assert_eq!(t.knob_arc_glow, 1.0);
        assert_eq!(t.meter_label_style, AudioDesignTokens::LABEL_UNDERLINED);
        assert!(t.meter_use_gradient);
        assert_eq!(t.meter_corner_radius, 0.0);
        assert_eq!(t.meter_glow, 1.0);
    }
}
