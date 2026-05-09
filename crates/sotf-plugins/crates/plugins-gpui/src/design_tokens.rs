//! Conversion from `DesignSystem` to `AudioDesignTokens`.
//!
//! This module bridges `gpui_design::DesignSystem` (the full platform design
//! system) and `gpui_ui_kit::audio_design_tokens::AudioDesignTokens` (the
//! lightweight tokens consumed by audio UI components).

use gpui_design::{DesignSystem, ToggleVariant};
use gpui_ui_kit::audio_design_tokens::AudioDesignTokens;

/// Convert a `DesignSystem` into `AudioDesignTokens` for UI components.
pub fn audio_tokens_from_ds(ds: &DesignSystem) -> AudioDesignTokens {
    let arc_w = ds.audio_controls.knob_arc_width;
    let arc_widths = [arc_w, arc_w + 0.5, arc_w + 1.0, arc_w + 1.5];
    AudioDesignTokens {
        knob_arc_start_deg: ds.audio_controls.knob_arc_start_deg,
        knob_arc_sweep_deg: ds.audio_controls.knob_arc_sweep_deg,
        knob_arc_widths: arc_widths,
        knob_arc_track_widths: arc_widths,
        knob_arc_glow: 0.0,
        knob_arc_segments: ds.audio_controls.knob_arc_segments,
        knob_border_width: ds.audio_controls.knob_border_width,
        knob_label_style: AudioDesignTokens::LABEL_BOXED,
        knob_indicator_style: AudioDesignTokens::INDICATOR_DOT,
        slider_track_widths: ds.audio_controls.slider_track_widths,
        meter_label_style: AudioDesignTokens::LABEL_BOXED,
        meter_use_gradient: false,
        meter_corner_radius: 2.0,
        meter_glow: 0.0,
        toggle_variant: match ds.toggle_variant {
            ToggleVariant::Capsule => AudioDesignTokens::TOGGLE_SLIDING,
            ToggleVariant::ThumbOnTrack => AudioDesignTokens::TOGGLE_THUMB_ON_TRACK,
            ToggleVariant::Segmented => AudioDesignTokens::TOGGLE_SEGMENTED,
            ToggleVariant::Pill => AudioDesignTokens::TOGGLE_PILL,
        },
        corner_radius: ds.corners.md,
        min_touch_target: ds.interaction.min_touch_target,
        control_padding_x: ds.spacing.control_padding_x,
        control_padding_y: ds.spacing.control_padding_y,
        animation_duration_ms: ds.animation.duration_ms,
        prefer_spring: ds.animation.prefer_spring,
        spring_stiffness: ds.animation.spring_stiffness,
        spring_damping: ds.animation.spring_damping,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_neutral_matches_default_tokens() {
        let ds = DesignSystem::neutral();
        let tokens = audio_tokens_from_ds(&ds);
        let default = AudioDesignTokens::default();

        assert_eq!(tokens.knob_arc_start_deg, default.knob_arc_start_deg);
        assert_eq!(tokens.knob_arc_sweep_deg, default.knob_arc_sweep_deg);
        assert_eq!(tokens.knob_arc_widths, default.knob_arc_widths);
        assert_eq!(tokens.knob_arc_track_widths, default.knob_arc_track_widths);
        assert_eq!(tokens.knob_arc_glow, default.knob_arc_glow);
        assert_eq!(tokens.knob_arc_segments, default.knob_arc_segments);
        assert_eq!(tokens.knob_border_width, default.knob_border_width);
        assert_eq!(tokens.knob_label_style, default.knob_label_style);
        assert_eq!(tokens.knob_indicator_style, default.knob_indicator_style);
        assert_eq!(tokens.slider_track_widths, default.slider_track_widths);
        assert_eq!(tokens.meter_label_style, default.meter_label_style);
        assert_eq!(tokens.meter_use_gradient, default.meter_use_gradient);
        assert_eq!(tokens.meter_corner_radius, default.meter_corner_radius);
        assert_eq!(tokens.meter_glow, default.meter_glow);
        assert_eq!(tokens.toggle_variant, default.toggle_variant);
        assert_eq!(tokens.corner_radius, default.corner_radius);
        assert_eq!(tokens.min_touch_target, default.min_touch_target);
        assert_eq!(tokens.control_padding_x, default.control_padding_x);
        assert_eq!(tokens.control_padding_y, default.control_padding_y);
        assert_eq!(tokens.animation_duration_ms, default.animation_duration_ms);
        assert_eq!(tokens.prefer_spring, default.prefer_spring);
        assert_eq!(tokens.spring_stiffness, default.spring_stiffness);
        assert_eq!(tokens.spring_damping, default.spring_damping);
    }

    #[test]
    fn test_material_toggle_is_thumb_on_track() {
        let ds = DesignSystem::material3();
        let tokens = audio_tokens_from_ds(&ds);
        assert_eq!(
            tokens.toggle_variant,
            AudioDesignTokens::TOGGLE_THUMB_ON_TRACK
        );
    }

    #[test]
    fn test_fluent_toggle_is_pill() {
        let ds = DesignSystem::fluent();
        let tokens = audio_tokens_from_ds(&ds);
        assert_eq!(tokens.toggle_variant, AudioDesignTokens::TOGGLE_PILL);
    }

    #[test]
    fn test_apple_has_larger_touch_target() {
        let ds = DesignSystem::apple_hig();
        let tokens = audio_tokens_from_ds(&ds);
        assert_eq!(tokens.min_touch_target, 44.0);
    }
}
