//! Studio Cream — light editorial.
//!
//! Warm cream surfaces, tomato-red accent, generous serif typography. The
//! light counterpart to Graphite. Reads like a magazine spread.

use super::plugin_theme::PluginTheme;
use gpui::{Rgba, SharedString};
use gpui_audio_kit::audio_design_tokens::AudioDesignTokens;

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
        // Surfaces — warm cream paper
        chassis_bg_top: rgba(0xf5efe2, 1.0),
        chassis_bg_bottom: rgba(0xeae3d2, 1.0),
        chassis_border: rgba(0xd0c8b3, 1.0),
        panel_bg: rgba(0xfaf5e9, 1.0),
        panel_recess: rgba(0xe6dec7, 1.0),
        section_divider: rgba(0xd9d1be, 1.0),
        corner_bracket: rgba(0xb5ad94, 0.7),

        // Ink — espresso brown scale
        ink_hi: rgba(0x2a2620, 1.0),
        ink: rgba(0x4a4239, 1.0),
        ink_mid: rgba(0x7c7263, 1.0),
        ink_low: rgba(0x6b6257, 1.0),
        ink_faint: rgba(0x7c7263, 1.0),

        // Accent — terracotta tomato
        accent: rgba(0xc94a32, 1.0),
        accent_bright: rgba(0xe65b3f, 1.0),
        accent_glow: rgba(0xc94a32, 0.18),
        accent_arc: rgba(0xc94a32, 1.0),
        accent_track: rgba(0xc94a32, 0.10),

        // LED — soft viridian
        led_active: rgba(0x4a8c5e, 1.0),
        led_glow: rgba(0x4a8c5e, 0.40),

        // Typography — serif heart, traditional sans + slab mono
        font_display: SharedString::from("Fraunces"),
        font_mono: SharedString::from("JetBrains Mono"),
        font_ui: SharedString::from("Fraunces"),

        // Dimensions — slightly tighter than Graphite, more disciplined
        knob_size_px: 74.0,
        arc_stroke_px: 2.0,
        radius_chassis: 8.0,
        radius_panel: 4.0,
        spacing_section: 44.0,
        spacing_knob_row: 26.0,

        // Audio look tokens — keep the prior boxed look on knobs and meters.
        knob_label_style: AudioDesignTokens::LABEL_BOXED,
        knob_arc_glow: 0.0,
        meter_label_style: AudioDesignTokens::LABEL_BOXED,
        meter_use_gradient: false,
        meter_corner_radius: 2.0,
        meter_glow: 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn studio_cream_chassis_is_light() {
        let t = theme();
        // Light surfaces — RGB channels > 0.85.
        assert!(
            t.chassis_bg_top.r > 0.85 && t.chassis_bg_top.g > 0.85,
            "chassis_bg_top should be light cream, got r={} g={} b={}",
            t.chassis_bg_top.r,
            t.chassis_bg_top.g,
            t.chassis_bg_top.b,
        );
    }

    #[test]
    fn studio_cream_has_warm_accent() {
        let t = theme();
        // Tomato: red dominant, green moderate, blue low.
        assert!(t.accent.r > 0.6);
        assert!(t.accent.g < 0.5);
        assert!(t.accent.b < 0.3);
    }
}
