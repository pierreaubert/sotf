//! Audio Design Tokens
//!
//! Lightweight design token struct consumed by audio UI components
//! (Potentiometer, VerticalSlider, Toggle). This replaces hardcoded
//! geometry/timing values with configurable parameters driven by the
//! platform design system (Apple HIG, Material 3, Fluent, Neutral).
//!
//! Components use `Default::default()` when no tokens are provided,
//! which reproduces the current hardcoded behavior (backward compatible).
//!
//! The host application converts its `DesignSystem` into `AudioDesignTokens`
//! and passes them to components via `.design_tokens(tokens)` builder methods.

/// Audio component design tokens — configurable geometry and timing.
#[derive(Debug, Clone, PartialEq)]
pub struct AudioDesignTokens {
    // -- Potentiometer (knob) geometry --
    /// Knob arc start angle in degrees from 12 o'clock, clockwise.
    /// Default: 135.0 (7:30 position).
    pub knob_arc_start_deg: f32,
    /// Knob arc sweep in degrees. Dead zone = 360 - sweep.
    /// Default: 270.0 (90° dead zone at bottom).
    pub knob_arc_sweep_deg: f32,
    /// Arc thickness in px (per size variant: [Xs, Sm, Md, Lg]).
    /// Default: [2.5, 3.0, 3.5, 4.0].
    pub knob_arc_widths: [f32; 4],
    /// Number of segments for arc rendering (smoothness).
    /// Default: 48.
    pub knob_arc_segments: u32,
    /// Knob border width in px.
    /// Default: 2.0.
    pub knob_border_width: f32,

    // -- VerticalSlider geometry --
    /// Slider track widths [Sm, Md, Lg] in px.
    /// Default: [14.0, 18.0, 24.0].
    pub slider_track_widths: [f32; 3],

    // -- Toggle variant --
    /// Toggle visual style.
    /// 0=Sliding (iOS capsule), 1=Segmented ([OFF|ON]), 2=ThumbOnTrack (Material), 3=Pill (Fluent).
    /// Default: 0 (Sliding).
    pub toggle_variant: u8,

    // -- Spacing --
    /// Control corner radius in px.
    /// Default: 8.0.
    pub corner_radius: f32,
    /// Minimum touch target size in px.
    /// Default: 32.0.
    pub min_touch_target: f32,
    /// Horizontal control padding in px.
    /// Default: 12.0.
    pub control_padding_x: f32,
    /// Vertical control padding in px.
    /// Default: 8.0.
    pub control_padding_y: f32,

    // -- Animation --
    /// Default animation duration in milliseconds.
    /// Default: 200.
    pub animation_duration_ms: u32,
    /// Whether to prefer spring physics over eased curves.
    /// Default: false.
    pub prefer_spring: bool,
    /// Spring stiffness (used when prefer_spring is true).
    /// Default: 170.0.
    pub spring_stiffness: f32,
    /// Spring damping (used when prefer_spring is true).
    /// Default: 26.0.
    pub spring_damping: f32,
}

impl Default for AudioDesignTokens {
    /// Returns tokens matching the current hardcoded values (neutral preset).
    fn default() -> Self {
        Self {
            knob_arc_start_deg: 135.0,
            knob_arc_sweep_deg: 270.0,
            knob_arc_widths: [2.5, 3.0, 3.5, 4.0],
            knob_arc_segments: 48,
            knob_border_width: 2.0,
            slider_track_widths: [14.0, 18.0, 24.0],
            toggle_variant: 0,
            corner_radius: 8.0,
            min_touch_target: 32.0,
            control_padding_x: 12.0,
            control_padding_y: 8.0,
            animation_duration_ms: 200,
            prefer_spring: false,
            spring_stiffness: 170.0,
            spring_damping: 26.0,
        }
    }
}

/// Toggle variant constants (matches design_system::ToggleVariant ordering).
impl AudioDesignTokens {
    pub const TOGGLE_SLIDING: u8 = 0;
    pub const TOGGLE_SEGMENTED: u8 = 1;
    pub const TOGGLE_THUMB_ON_TRACK: u8 = 2;
    pub const TOGGLE_PILL: u8 = 3;
}
