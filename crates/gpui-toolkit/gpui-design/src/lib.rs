//! Platform-Adaptive Design System
//!
//! Defines shape, spacing, interaction, and animation rules that vary per
//! platform (Apple HIG, Material Design 3, Windows Fluent) while the Theme
//! system handles colors independently. The two layers are independently
//! combinable: any color theme works with any design system.
//!
//! This module contains only data types — no rendering code, no framework deps.
//! Platform renderers consume it alongside Theme colors.

use serde::{Serialize, Serializer, ser::SerializeStruct};

// ============================================================================
// Enums
// ============================================================================

/// Which platform design language to follow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum DesignLanguage {
    /// Apple Human Interface Guidelines (macOS, iOS).
    AppleHig,
    /// Material Design 3 (Android, ChromeOS, web).
    Material3,
    /// Windows Fluent Design (Windows 10/11).
    Fluent,
    /// Neutral default — no strong platform opinion. Matches current hardcoded values.
    Neutral,
}

/// Host platform bucket used to choose an appropriate default design language.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum DesignPlatform {
    Macos,
    Ios,
    Windows,
    Android,
    Linux,
    Other,
}

/// Corner radius rendering strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum CornerRadiusStyle {
    /// Apple continuous corners (squircle). Renderer should use smooth curves.
    Continuous,
    /// Standard circular arcs (CSS `border-radius`).
    Circular,
}

/// Toggle control visual style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum ToggleVariant {
    /// iOS-style capsule slider with thumb.
    Capsule,
    /// Thumb rides on a visible track (Material).
    ThumbOnTrack,
    /// Segmented [OFF|ON] button pair.
    Segmented,
    /// Pill-shaped toggle (Fluent).
    Pill,
}

/// Where labels appear relative to their control.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum LabelPosition {
    /// Label below the control (Apple, Material).
    Below,
    /// Label to the right of the control (Fluent, compact UIs).
    Right,
}

/// Visual style for grouping controls.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum GroupSeparatorStyle {
    /// Subtle hairline divider (Apple).
    Divider,
    /// Distinct card surface with shadow/elevation (Material).
    Card,
    /// Thin border outline (Fluent).
    Border,
    /// No visual separator — spacing only.
    None,
}

// ============================================================================
// Sub-structs
// ============================================================================

/// Corner radius values for different element sizes.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct CornerRadii {
    /// Small elements (badges, chips): px.
    pub sm: f32,
    /// Medium elements (buttons, inputs, controls): px.
    pub md: f32,
    /// Large elements (cards, panels): px.
    pub lg: f32,
    /// Extra-large / pill shape: px.
    pub xl: f32,
    /// Corner rendering style.
    pub style: CornerRadiusStyle,
}

impl CornerRadii {
    pub fn new(sm: f32, md: f32, lg: f32, xl: f32, style: CornerRadiusStyle) -> Self {
        assert!(sm >= 0.0, "sm must be >= 0");
        assert!(md >= 0.0, "md must be >= 0");
        assert!(lg >= 0.0, "lg must be >= 0");
        assert!(xl >= 0.0, "xl must be >= 0");
        Self {
            sm,
            md,
            lg,
            xl,
            style,
        }
    }
}

/// Spacing and density rules.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct SpacingRules {
    /// Base grid unit in px. All spacing should be multiples of this.
    pub grid_unit: f32,
    /// Inline (horizontal) padding for controls: px.
    pub control_padding_x: f32,
    /// Block (vertical) padding for controls: px.
    pub control_padding_y: f32,
    /// Gap between controls in a group: px.
    pub control_gap: f32,
    /// Gap between groups/sections: px.
    pub section_gap: f32,
    /// Padding inside a card/panel: px.
    pub card_padding: f32,
}

impl SpacingRules {
    pub fn new(
        grid_unit: f32,
        control_padding_x: f32,
        control_padding_y: f32,
        control_gap: f32,
        section_gap: f32,
        card_padding: f32,
    ) -> Self {
        assert!(grid_unit >= 0.0, "grid_unit must be >= 0");
        assert!(control_padding_x >= 0.0, "control_padding_x must be >= 0");
        assert!(control_padding_y >= 0.0, "control_padding_y must be >= 0");
        assert!(control_gap >= 0.0, "control_gap must be >= 0");
        assert!(section_gap >= 0.0, "section_gap must be >= 0");
        assert!(card_padding >= 0.0, "card_padding must be >= 0");
        Self {
            grid_unit,
            control_padding_x,
            control_padding_y,
            control_gap,
            section_gap,
            card_padding,
        }
    }
}

/// Touch target and interaction sizing.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct InteractionRules {
    /// Minimum touch target size in px (accessibility).
    pub min_touch_target: f32,
    /// Border width for interactive elements: px.
    pub border_width: f32,
    /// Focus ring width: px.
    pub focus_ring_width: f32,
    /// Focus ring offset from element edge: px.
    pub focus_ring_offset: f32,
}

impl InteractionRules {
    pub fn new(
        min_touch_target: f32,
        border_width: f32,
        focus_ring_width: f32,
        focus_ring_offset: f32,
    ) -> Self {
        assert!(min_touch_target >= 0.0, "min_touch_target must be >= 0");
        assert!(border_width >= 0.0, "border_width must be >= 0");
        assert!(focus_ring_width >= 0.0, "focus_ring_width must be >= 0");
        assert!(focus_ring_offset >= 0.0, "focus_ring_offset must be >= 0");
        Self {
            min_touch_target,
            border_width,
            focus_ring_width,
            focus_ring_offset,
        }
    }
}

/// Shadow/elevation model.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct ElevationRules {
    /// Level 0 (flat): shadow blur radius in px. 0 = no shadow.
    pub level_0_blur: f32,
    /// Level 1 (raised card): shadow blur radius in px.
    pub level_1_blur: f32,
    /// Level 2 (dialog/popover): shadow blur radius in px.
    pub level_2_blur: f32,
    /// Shadow opacity multiplier (0.0–1.0).
    pub shadow_opacity: f32,
    /// Shadow Y offset in px (positive = downward).
    pub shadow_y_offset: f32,
}

impl ElevationRules {
    pub fn new(
        level_0_blur: f32,
        level_1_blur: f32,
        level_2_blur: f32,
        shadow_opacity: f32,
        shadow_y_offset: f32,
    ) -> Self {
        assert!(level_0_blur >= 0.0, "level_0_blur must be >= 0");
        assert!(level_1_blur >= 0.0, "level_1_blur must be >= 0");
        assert!(level_2_blur >= 0.0, "level_2_blur must be >= 0");
        assert!(
            (0.0..=1.0).contains(&shadow_opacity),
            "shadow_opacity must be in [0, 1]"
        );
        Self {
            level_0_blur,
            level_1_blur,
            level_2_blur,
            shadow_opacity,
            shadow_y_offset,
        }
    }
}

/// Animation timing rules.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct AnimationRules {
    /// Default transition duration in milliseconds.
    pub duration_ms: u32,
    /// Fast transition (hover, press) in milliseconds.
    pub fast_ms: u32,
    /// Slow transition (page, modal) in milliseconds.
    pub slow_ms: u32,
    /// Whether to prefer spring physics over eased curves.
    pub prefer_spring: bool,
    /// Spring stiffness (used when prefer_spring is true).
    pub spring_stiffness: f32,
    /// Spring damping (used when prefer_spring is true).
    pub spring_damping: f32,
}

impl AnimationRules {
    pub fn new(
        duration_ms: u32,
        fast_ms: u32,
        slow_ms: u32,
        prefer_spring: bool,
        spring_stiffness: f32,
        spring_damping: f32,
    ) -> Self {
        assert!(duration_ms > 0, "duration_ms must be > 0");
        assert!(fast_ms > 0, "fast_ms must be > 0");
        assert!(slow_ms > 0, "slow_ms must be > 0");
        assert!(spring_stiffness > 0.0, "spring_stiffness must be > 0");
        assert!(spring_damping > 0.0, "spring_damping must be > 0");
        Self {
            duration_ms,
            fast_ms,
            slow_ms,
            prefer_spring,
            spring_stiffness,
            spring_damping,
        }
    }
}

/// Typography rules.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TypographyRules {
    /// Preferred font family name.
    pub font_family: String,
    /// Whether to use dynamic type sizes (Apple) or fixed scale.
    pub dynamic_sizing: bool,
    /// Base font size in px.
    pub base_size: f32,
    /// Small text size in px (labels, captions).
    pub small_size: f32,
    /// Large text size in px (headers, titles).
    pub large_size: f32,
}

impl TypographyRules {
    pub fn new(
        font_family: impl Into<String>,
        dynamic_sizing: bool,
        base_size: f32,
        small_size: f32,
        large_size: f32,
    ) -> Self {
        assert!(base_size > 0.0, "base_size must be > 0");
        assert!(small_size > 0.0, "small_size must be > 0");
        assert!(large_size > 0.0, "large_size must be > 0");
        Self {
            font_family: font_family.into(),
            dynamic_sizing,
            base_size,
            small_size,
            large_size,
        }
    }
}

/// Layout solver thresholds — parameterizes the constants in `layout_solver.rs`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct LayoutThresholds {
    /// Below this width, switch to vertical orientation.
    pub vertical_threshold: f32,
    /// Below this main-column width, stack control groups vertically.
    pub group_stack_threshold: f32,
    /// Below this main-column width, use compact slider height.
    pub compact_slider_threshold: f32,
    /// Below this main-column width, hide visualizations.
    pub hide_viz_threshold: f32,
    /// Below this main-column width, use extra-compact (Xs) knobs.
    pub compact_knob_threshold: f32,
    /// Above this main-column width, use medium (Md) knobs.
    pub large_knob_threshold: f32,
    /// Standard slider height in px.
    pub slider_height_normal: f32,
    /// Compact slider height in px.
    pub slider_height_compact: f32,
}

impl LayoutThresholds {
    pub fn new(
        vertical_threshold: f32,
        group_stack_threshold: f32,
        compact_slider_threshold: f32,
        hide_viz_threshold: f32,
        compact_knob_threshold: f32,
        large_knob_threshold: f32,
        slider_height_normal: f32,
        slider_height_compact: f32,
    ) -> Self {
        assert!(vertical_threshold > 0.0, "vertical_threshold must be > 0");
        assert!(
            group_stack_threshold > 0.0,
            "group_stack_threshold must be > 0"
        );
        assert!(
            compact_slider_threshold > 0.0,
            "compact_slider_threshold must be > 0"
        );
        assert!(hide_viz_threshold > 0.0, "hide_viz_threshold must be > 0");
        assert!(
            compact_knob_threshold > 0.0,
            "compact_knob_threshold must be > 0"
        );
        assert!(
            large_knob_threshold > 0.0,
            "large_knob_threshold must be > 0"
        );
        assert!(
            slider_height_normal > 0.0,
            "slider_height_normal must be > 0"
        );
        assert!(
            slider_height_compact > 0.0,
            "slider_height_compact must be > 0"
        );
        Self {
            vertical_threshold,
            group_stack_threshold,
            compact_slider_threshold,
            hide_viz_threshold,
            compact_knob_threshold,
            large_knob_threshold,
            slider_height_normal,
            slider_height_compact,
        }
    }
}

/// Audio control geometry — knob arc, slider tracks.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct AudioControlRules {
    /// Knob arc start angle in degrees from 12 o'clock, clockwise.
    pub knob_arc_start_deg: f32,
    /// Knob arc sweep in degrees (dead zone at bottom = 360 - sweep).
    pub knob_arc_sweep_deg: f32,
    /// Arc thickness in px.
    pub knob_arc_width: f32,
    /// Number of segments for arc rendering (smoothness).
    pub knob_arc_segments: u32,
    /// Knob border width in px.
    pub knob_border_width: f32,
    /// Slider track widths [Sm, Md, Lg] in px.
    pub slider_track_widths: [f32; 3],
}

impl AudioControlRules {
    pub fn new(
        knob_arc_start_deg: f32,
        knob_arc_sweep_deg: f32,
        knob_arc_width: f32,
        knob_arc_segments: u32,
        knob_border_width: f32,
        slider_track_widths: [f32; 3],
    ) -> Self {
        assert!(knob_arc_segments > 0, "knob_arc_segments must be > 0");
        assert!(knob_arc_width >= 0.0, "knob_arc_width must be >= 0");
        assert!(knob_border_width >= 0.0, "knob_border_width must be >= 0");
        for (i, &w) in slider_track_widths.iter().enumerate() {
            assert!(w >= 0.0, "slider_track_widths[{i}] must be >= 0");
        }
        Self {
            knob_arc_start_deg,
            knob_arc_sweep_deg,
            knob_arc_width,
            knob_arc_segments,
            knob_border_width,
            slider_track_widths,
        }
    }
}

// ============================================================================
// DesignSystem
// ============================================================================

/// Complete design system — all shape, spacing, and interaction rules
/// needed to render platform-appropriate UIs.
///
/// Orthogonal to the Theme (which handles colors). Any Theme works with
/// any DesignSystem.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DesignSystem {
    /// Which platform language this represents.
    pub language: DesignLanguage,
    /// Corner radius values and rendering style.
    pub corners: CornerRadii,
    /// Spacing grid and density.
    pub spacing: SpacingRules,
    /// Touch targets and interaction sizing.
    pub interaction: InteractionRules,
    /// Shadow and elevation model.
    pub elevation: ElevationRules,
    /// Animation timing.
    pub animation: AnimationRules,
    /// Typography.
    pub typography: TypographyRules,
    /// Layout solver thresholds.
    pub layout: LayoutThresholds,
    /// Audio-specific control geometry.
    pub audio_controls: AudioControlRules,
    /// Preferred toggle visual style.
    pub toggle_variant: ToggleVariant,
    /// Where labels appear relative to controls.
    pub label_position: LabelPosition,
    /// How control groups are visually separated.
    pub group_separator: GroupSeparatorStyle,
}

// ============================================================================
// Presets
// ============================================================================

impl DesignSystem {
    /// Neutral preset — matches the current hardcoded values exactly.
    /// Zero visual change for existing users.
    pub fn neutral() -> Self {
        Self {
            language: DesignLanguage::Neutral,
            corners: CornerRadii {
                sm: 4.0,
                md: 8.0,
                lg: 12.0,
                xl: 16.0,
                style: CornerRadiusStyle::Circular,
            },
            spacing: SpacingRules {
                grid_unit: 4.0,
                control_padding_x: 12.0,
                control_padding_y: 8.0,
                control_gap: 8.0,
                section_gap: 16.0,
                card_padding: 12.0,
            },
            interaction: InteractionRules {
                min_touch_target: 32.0,
                border_width: 1.0,
                focus_ring_width: 2.0,
                focus_ring_offset: 2.0,
            },
            elevation: ElevationRules {
                level_0_blur: 0.0,
                level_1_blur: 4.0,
                level_2_blur: 16.0,
                shadow_opacity: 0.15,
                shadow_y_offset: 2.0,
            },
            animation: AnimationRules {
                duration_ms: 200,
                fast_ms: 100,
                slow_ms: 400,
                prefer_spring: false,
                spring_stiffness: 170.0,
                spring_damping: 26.0,
            },
            typography: TypographyRules {
                font_family: ".SystemUIFont".to_string(),
                dynamic_sizing: false,
                base_size: 14.0,
                small_size: 11.0,
                large_size: 18.0,
            },
            layout: LayoutThresholds {
                vertical_threshold: 400.0,
                group_stack_threshold: 500.0,
                compact_slider_threshold: 700.0,
                hide_viz_threshold: 600.0,
                compact_knob_threshold: 400.0,
                large_knob_threshold: 800.0,
                slider_height_normal: 180.0,
                slider_height_compact: 120.0,
            },
            audio_controls: AudioControlRules {
                knob_arc_start_deg: 135.0,
                knob_arc_sweep_deg: 270.0,
                knob_arc_width: 2.5,
                knob_arc_segments: 48,
                knob_border_width: 2.0,
                slider_track_widths: [14.0, 18.0, 24.0],
            },
            toggle_variant: ToggleVariant::Capsule,
            label_position: LabelPosition::Below,
            group_separator: GroupSeparatorStyle::Divider,
        }
    }

    /// Apple Human Interface Guidelines preset.
    pub fn apple_hig() -> Self {
        Self {
            language: DesignLanguage::AppleHig,
            corners: CornerRadii {
                sm: 6.0,
                md: 10.0,
                lg: 14.0,
                xl: 20.0,
                style: CornerRadiusStyle::Continuous,
            },
            spacing: SpacingRules {
                grid_unit: 8.0,
                control_padding_x: 16.0,
                control_padding_y: 12.0,
                control_gap: 8.0,
                section_gap: 20.0,
                card_padding: 16.0,
            },
            interaction: InteractionRules {
                min_touch_target: 44.0,
                border_width: 0.5,
                focus_ring_width: 3.0,
                focus_ring_offset: 2.0,
            },
            elevation: ElevationRules {
                level_0_blur: 0.0,
                level_1_blur: 8.0,
                level_2_blur: 24.0,
                shadow_opacity: 0.1,
                shadow_y_offset: 4.0,
            },
            animation: AnimationRules {
                duration_ms: 350,
                fast_ms: 150,
                slow_ms: 500,
                prefer_spring: true,
                spring_stiffness: 120.0,
                spring_damping: 14.0,
            },
            typography: TypographyRules {
                font_family: ".SystemUIFont".to_string(),
                dynamic_sizing: true,
                base_size: 15.0,
                small_size: 12.0,
                large_size: 20.0,
            },
            layout: LayoutThresholds {
                vertical_threshold: 400.0,
                group_stack_threshold: 500.0,
                compact_slider_threshold: 700.0,
                hide_viz_threshold: 600.0,
                compact_knob_threshold: 400.0,
                large_knob_threshold: 800.0,
                slider_height_normal: 180.0,
                slider_height_compact: 120.0,
            },
            audio_controls: AudioControlRules {
                knob_arc_start_deg: 135.0,
                knob_arc_sweep_deg: 270.0,
                knob_arc_width: 2.5,
                knob_arc_segments: 48,
                knob_border_width: 1.5,
                slider_track_widths: [14.0, 18.0, 24.0],
            },
            toggle_variant: ToggleVariant::Capsule,
            label_position: LabelPosition::Below,
            group_separator: GroupSeparatorStyle::Divider,
        }
    }

    /// Material Design 3 preset.
    pub fn material3() -> Self {
        Self {
            language: DesignLanguage::Material3,
            corners: CornerRadii {
                sm: 8.0,
                md: 12.0,
                lg: 16.0,
                xl: 28.0,
                style: CornerRadiusStyle::Circular,
            },
            spacing: SpacingRules {
                grid_unit: 4.0,
                control_padding_x: 16.0,
                control_padding_y: 12.0,
                control_gap: 8.0,
                section_gap: 16.0,
                card_padding: 16.0,
            },
            interaction: InteractionRules {
                min_touch_target: 48.0,
                border_width: 1.0,
                focus_ring_width: 2.0,
                focus_ring_offset: 2.0,
            },
            elevation: ElevationRules {
                level_0_blur: 0.0,
                level_1_blur: 12.0,
                level_2_blur: 24.0,
                shadow_opacity: 0.2,
                shadow_y_offset: 4.0,
            },
            animation: AnimationRules {
                duration_ms: 300,
                fast_ms: 150,
                slow_ms: 500,
                prefer_spring: false,
                spring_stiffness: 170.0,
                spring_damping: 26.0,
            },
            typography: TypographyRules {
                font_family: ".SystemUIFont".to_string(),
                dynamic_sizing: false,
                base_size: 14.0,
                small_size: 12.0,
                large_size: 22.0,
            },
            layout: LayoutThresholds {
                vertical_threshold: 360.0,
                group_stack_threshold: 480.0,
                compact_slider_threshold: 700.0,
                hide_viz_threshold: 600.0,
                compact_knob_threshold: 400.0,
                large_knob_threshold: 800.0,
                slider_height_normal: 180.0,
                slider_height_compact: 120.0,
            },
            audio_controls: AudioControlRules {
                knob_arc_start_deg: 135.0,
                knob_arc_sweep_deg: 270.0,
                knob_arc_width: 3.0,
                knob_arc_segments: 48,
                knob_border_width: 2.0,
                slider_track_widths: [16.0, 20.0, 24.0],
            },
            toggle_variant: ToggleVariant::ThumbOnTrack,
            label_position: LabelPosition::Below,
            group_separator: GroupSeparatorStyle::Card,
        }
    }

    /// Windows Fluent Design preset.
    pub fn fluent() -> Self {
        Self {
            language: DesignLanguage::Fluent,
            corners: CornerRadii {
                sm: 2.0,
                md: 4.0,
                lg: 8.0,
                xl: 12.0,
                style: CornerRadiusStyle::Circular,
            },
            spacing: SpacingRules {
                grid_unit: 4.0,
                control_padding_x: 12.0,
                control_padding_y: 8.0,
                control_gap: 6.0,
                section_gap: 12.0,
                card_padding: 12.0,
            },
            interaction: InteractionRules {
                min_touch_target: 32.0,
                border_width: 1.0,
                focus_ring_width: 2.0,
                focus_ring_offset: 1.0,
            },
            elevation: ElevationRules {
                level_0_blur: 0.0,
                level_1_blur: 2.0,
                level_2_blur: 8.0,
                shadow_opacity: 0.08,
                shadow_y_offset: 1.0,
            },
            animation: AnimationRules {
                duration_ms: 200,
                fast_ms: 100,
                slow_ms: 350,
                prefer_spring: false,
                spring_stiffness: 300.0,
                spring_damping: 30.0,
            },
            typography: TypographyRules {
                font_family: ".SystemUIFont".to_string(),
                dynamic_sizing: false,
                base_size: 14.0,
                small_size: 12.0,
                large_size: 18.0,
            },
            layout: LayoutThresholds {
                vertical_threshold: 400.0,
                group_stack_threshold: 500.0,
                compact_slider_threshold: 700.0,
                hide_viz_threshold: 600.0,
                compact_knob_threshold: 400.0,
                large_knob_threshold: 800.0,
                slider_height_normal: 160.0,
                slider_height_compact: 100.0,
            },
            audio_controls: AudioControlRules {
                knob_arc_start_deg: 135.0,
                knob_arc_sweep_deg: 270.0,
                knob_arc_width: 2.0,
                knob_arc_segments: 48,
                knob_border_width: 1.0,
                slider_track_widths: [12.0, 16.0, 20.0],
            },
            toggle_variant: ToggleVariant::Pill,
            label_position: LabelPosition::Right,
            group_separator: GroupSeparatorStyle::Border,
        }
    }

    pub fn for_language(language: DesignLanguage) -> Self {
        match language {
            DesignLanguage::AppleHig => Self::apple_hig(),
            DesignLanguage::Material3 => Self::material3(),
            DesignLanguage::Fluent => Self::fluent(),
            DesignLanguage::Neutral => Self::neutral(),
        }
    }

    pub fn from_language_id(id: &str) -> Option<Self> {
        DesignLanguage::from_id(id).map(Self::for_language)
    }

    pub fn for_platform(platform: DesignPlatform) -> Self {
        match platform {
            DesignPlatform::Macos | DesignPlatform::Ios => Self::apple_hig(),
            DesignPlatform::Windows => Self::fluent(),
            DesignPlatform::Android => Self::material3(),
            DesignPlatform::Linux | DesignPlatform::Other => Self::neutral(),
        }
    }

    /// Returns the platform-appropriate default design system.
    pub fn platform_default() -> Self {
        Self::for_platform(DesignPlatform::current())
    }
}

impl DesignLanguage {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::AppleHig => "apple_hig",
            Self::Material3 => "material3",
            Self::Fluent => "fluent",
            Self::Neutral => "neutral",
        }
    }

    pub fn from_id(value: &str) -> Option<Self> {
        match value {
            "apple_hig" => Some(Self::AppleHig),
            "material3" => Some(Self::Material3),
            "fluent" => Some(Self::Fluent),
            "neutral" => Some(Self::Neutral),
            _ => None,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::AppleHig => "Apple",
            Self::Material3 => "Material",
            Self::Fluent => "Fluent",
            Self::Neutral => "Neutral",
        }
    }
}

impl DesignPlatform {
    pub fn current() -> Self {
        #[cfg(target_os = "macos")]
        {
            Self::Macos
        }
        #[cfg(target_os = "ios")]
        {
            Self::Ios
        }
        #[cfg(target_os = "windows")]
        {
            Self::Windows
        }
        #[cfg(target_os = "android")]
        {
            Self::Android
        }
        #[cfg(target_os = "linux")]
        {
            Self::Linux
        }
        #[cfg(not(any(
            target_os = "macos",
            target_os = "ios",
            target_os = "windows",
            target_os = "android",
            target_os = "linux"
        )))]
        {
            Self::Other
        }
    }
}

// ============================================================================
// Design conformance and token export
// ============================================================================

/// A style token in a shape compatible with Style Dictionary export.
#[derive(Debug, Clone, PartialEq)]
pub struct DesignToken {
    pub path: Vec<String>,
    pub value: String,
    pub token_type: &'static str,
}

impl DesignToken {
    pub fn name(&self) -> String {
        self.path.join(".")
    }
}

impl Serialize for DesignToken {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("DesignToken", 4)?;
        state.serialize_field("name", &self.name())?;
        state.serialize_field("path", &self.path)?;
        state.serialize_field("value", &self.value)?;
        state.serialize_field("token_type", &self.token_type)?;
        state.end()
    }
}

/// Accessibility and platform-conformance finding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConformanceFinding {
    pub id: &'static str,
    pub message: String,
}

/// Summary used by CI and component docs.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct DesignConformanceReport {
    pub findings: Vec<ConformanceFinding>,
}

impl DesignConformanceReport {
    pub fn passed(&self) -> bool {
        self.findings.is_empty()
    }
}

/// Conformance result for one preset and one motion policy.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DesignConformanceCase {
    pub preset_id: &'static str,
    pub reduced_motion: bool,
    pub report: DesignConformanceReport,
    pub motion: MotionSpec,
    pub token_count: usize,
}

impl DesignConformanceCase {
    pub fn passed(&self) -> bool {
        self.report.passed()
    }

    pub fn motion_label(&self) -> &'static str {
        if self.reduced_motion {
            "reduced"
        } else {
            "standard"
        }
    }
}

/// CI-facing conformance matrix across design presets and motion policies.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct DesignConformanceMatrix {
    pub cases: Vec<DesignConformanceCase>,
}

impl DesignConformanceMatrix {
    pub fn all_presets() -> Self {
        let mut cases = Vec::new();
        for (preset_id, system) in all_design_presets() {
            for reduced_motion in [false, true] {
                cases.push(DesignConformanceCase {
                    preset_id,
                    reduced_motion,
                    report: system.conformance_report(reduced_motion),
                    motion: system.motion_spec(reduced_motion),
                    token_count: system.style_dictionary_tokens().len(),
                });
            }
        }
        Self { cases }
    }

    pub fn passed(&self) -> bool {
        self.cases.iter().all(DesignConformanceCase::passed)
    }

    pub fn findings(&self) -> Vec<(&DesignConformanceCase, &ConformanceFinding)> {
        let mut findings = Vec::new();
        for case in &self.cases {
            for finding in &case.report.findings {
                findings.push((case, finding));
            }
        }
        findings
    }

    pub fn to_markdown_table(&self) -> String {
        let mut output = String::from(
            "| preset | motion | tokens | status | findings |\n\
             | --- | --- | ---: | --- | --- |\n",
        );
        for case in &self.cases {
            let findings = if case.report.findings.is_empty() {
                "none".to_string()
            } else {
                case.report
                    .findings
                    .iter()
                    .map(|finding| finding.id)
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            output.push_str(&format!(
                "| {} | {} | {} | {} | {} |\n",
                case.preset_id,
                case.motion_label(),
                case.token_count,
                if case.passed() { "pass" } else { "fail" },
                findings
            ));
        }
        output
    }
}

/// Style Dictionary token export for one preset.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DesignTokenPresetExport {
    pub preset_id: &'static str,
    pub tokens: Vec<DesignToken>,
}

/// Stable token export for all built-in presets.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct DesignTokenExport {
    pub presets: Vec<DesignTokenPresetExport>,
}

impl DesignTokenExport {
    pub fn for_all_presets() -> Self {
        Self {
            presets: all_design_presets()
                .into_iter()
                .map(|(preset_id, system)| DesignTokenPresetExport {
                    preset_id,
                    tokens: system.style_dictionary_tokens(),
                })
                .collect(),
        }
    }
}

/// Motion settings after reduced-motion policy is applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct MotionSpec {
    pub duration_ms: u32,
    pub fast_ms: u32,
    pub slow_ms: u32,
    pub prefer_spring: bool,
    pub reduced_motion: bool,
}

impl DesignSystem {
    /// Export stable platform/design tokens for Style Dictionary pipelines.
    pub fn style_dictionary_tokens(&self) -> Vec<DesignToken> {
        vec![
            token("design.language", self.language.as_str(), "string"),
            token("radius.sm", self.corners.sm, "dimension"),
            token("radius.md", self.corners.md, "dimension"),
            token("radius.lg", self.corners.lg, "dimension"),
            token("spacing.grid_unit", self.spacing.grid_unit, "dimension"),
            token(
                "spacing.control_padding_x",
                self.spacing.control_padding_x,
                "dimension",
            ),
            token(
                "spacing.control_padding_y",
                self.spacing.control_padding_y,
                "dimension",
            ),
            token("spacing.control_gap", self.spacing.control_gap, "dimension"),
            token("spacing.section_gap", self.spacing.section_gap, "dimension"),
            token(
                "spacing.card_padding",
                self.spacing.card_padding,
                "dimension",
            ),
            token(
                "interaction.min_touch_target",
                self.interaction.min_touch_target,
                "dimension",
            ),
            token(
                "interaction.border_width",
                self.interaction.border_width,
                "dimension",
            ),
            token(
                "interaction.focus_ring_width",
                self.interaction.focus_ring_width,
                "dimension",
            ),
            token(
                "interaction.focus_ring_offset",
                self.interaction.focus_ring_offset,
                "dimension",
            ),
            token(
                "elevation.level_0_blur",
                self.elevation.level_0_blur,
                "dimension",
            ),
            token(
                "elevation.level_1_blur",
                self.elevation.level_1_blur,
                "dimension",
            ),
            token(
                "elevation.level_2_blur",
                self.elevation.level_2_blur,
                "dimension",
            ),
            token(
                "elevation.shadow_opacity",
                self.elevation.shadow_opacity,
                "number",
            ),
            token(
                "typography.font_family",
                &self.typography.font_family,
                "font",
            ),
            token(
                "typography.base_size",
                self.typography.base_size,
                "dimension",
            ),
            token(
                "typography.dynamic_sizing",
                self.typography.dynamic_sizing,
                "boolean",
            ),
            token("motion.duration_ms", self.animation.duration_ms, "duration"),
            token("motion.fast_ms", self.animation.fast_ms, "duration"),
            token("motion.slow_ms", self.animation.slow_ms, "duration"),
            token(
                "motion.prefer_spring",
                self.animation.prefer_spring,
                "boolean",
            ),
            token(
                "audio.knob_arc_start_deg",
                self.audio_controls.knob_arc_start_deg,
                "number",
            ),
            token(
                "audio.knob_arc_sweep_deg",
                self.audio_controls.knob_arc_sweep_deg,
                "number",
            ),
            token(
                "audio.knob_arc_width",
                self.audio_controls.knob_arc_width,
                "dimension",
            ),
            token(
                "audio.knob_arc_segments",
                self.audio_controls.knob_arc_segments,
                "number",
            ),
            token(
                "audio.slider_track_sm",
                self.audio_controls.slider_track_widths[0],
                "dimension",
            ),
            token(
                "audio.slider_track_md",
                self.audio_controls.slider_track_widths[1],
                "dimension",
            ),
            token(
                "audio.slider_track_lg",
                self.audio_controls.slider_track_widths[2],
                "dimension",
            ),
        ]
    }

    /// Produce a lightweight conformance report for CI and docs.
    pub fn conformance_report(&self, reduced_motion_required: bool) -> DesignConformanceReport {
        let mut findings = Vec::new();

        finite_positive(
            &mut findings,
            "spacing.grid_unit",
            self.spacing.grid_unit,
            "spacing grid_unit must be finite and positive",
        );
        finite_non_negative(
            &mut findings,
            "spacing.control_padding_x",
            self.spacing.control_padding_x,
            "control horizontal padding must be finite and non-negative",
        );
        finite_non_negative(
            &mut findings,
            "spacing.control_padding_y",
            self.spacing.control_padding_y,
            "control vertical padding must be finite and non-negative",
        );
        finite_non_negative(
            &mut findings,
            "spacing.control_gap",
            self.spacing.control_gap,
            "control gap must be finite and non-negative",
        );
        finite_non_negative(
            &mut findings,
            "spacing.section_gap",
            self.spacing.section_gap,
            "section gap must be finite and non-negative",
        );

        for (id, value) in [
            ("radius.sm", self.corners.sm),
            ("radius.md", self.corners.md),
            ("radius.lg", self.corners.lg),
            ("radius.xl", self.corners.xl),
        ] {
            finite_non_negative(
                &mut findings,
                id,
                value,
                "corner radii must be finite and non-negative",
            );
        }

        finite_positive(
            &mut findings,
            "interaction.min_touch_target",
            self.interaction.min_touch_target,
            "minimum touch target must be finite and positive",
        );
        finite_non_negative(
            &mut findings,
            "interaction.focus_ring_width",
            self.interaction.focus_ring_width,
            "focus ring width must be finite and non-negative",
        );

        if self.language == DesignLanguage::AppleHig && self.interaction.min_touch_target < 44.0 {
            findings.push(ConformanceFinding {
                id: "apple.touch_target",
                message: "Apple HIG touch targets should be at least 44px".to_string(),
            });
        }
        if self.language == DesignLanguage::Material3 && self.interaction.min_touch_target < 48.0 {
            findings.push(ConformanceFinding {
                id: "material.touch_target",
                message: "Material 3 touch targets should be at least 48px".to_string(),
            });
        }

        if !(0.0..=1.0).contains(&self.elevation.shadow_opacity)
            || !self.elevation.shadow_opacity.is_finite()
        {
            findings.push(ConformanceFinding {
                id: "elevation.shadow_opacity",
                message: "shadow opacity must be finite and in [0, 1]".to_string(),
            });
        }

        if !(self.typography.small_size.is_finite()
            && self.typography.base_size.is_finite()
            && self.typography.large_size.is_finite()
            && self.typography.small_size > 0.0
            && self.typography.base_size > 0.0
            && self.typography.large_size > 0.0
            && self.typography.small_size <= self.typography.base_size
            && self.typography.base_size <= self.typography.large_size)
        {
            findings.push(ConformanceFinding {
                id: "typography.scale",
                message:
                    "typography sizes must be finite, positive, and ordered small <= base <= large"
                        .to_string(),
            });
        }
        if self.typography.dynamic_sizing && self.typography.large_size < self.typography.base_size
        {
            findings.push(ConformanceFinding {
                id: "typography.scale",
                message: "dynamic typography large_size must be >= base_size".to_string(),
            });
        }

        if !(self.animation.fast_ms <= self.animation.duration_ms
            && self.animation.duration_ms <= self.animation.slow_ms)
        {
            findings.push(ConformanceFinding {
                id: "motion.duration_order",
                message: "motion durations must be ordered fast <= default <= slow".to_string(),
            });
        }
        finite_positive(
            &mut findings,
            "motion.spring_stiffness",
            self.animation.spring_stiffness,
            "spring stiffness must be finite and positive",
        );
        finite_positive(
            &mut findings,
            "motion.spring_damping",
            self.animation.spring_damping,
            "spring damping must be finite and positive",
        );

        let motion = self.motion_spec(reduced_motion_required);
        if reduced_motion_required
            && (motion.duration_ms != 0 || motion.fast_ms != 0 || motion.slow_ms != 0)
        {
            findings.push(ConformanceFinding {
                id: "motion.reduced",
                message: "reduced-motion mode should collapse transition durations".to_string(),
            });
        }

        if self.layout.slider_height_compact > self.layout.slider_height_normal {
            findings.push(ConformanceFinding {
                id: "layout.slider_height_order",
                message: "compact slider height should not exceed normal slider height".to_string(),
            });
        }

        if !(self.audio_controls.knob_arc_sweep_deg.is_finite()
            && self.audio_controls.knob_arc_sweep_deg > 0.0
            && self.audio_controls.knob_arc_sweep_deg <= 360.0)
        {
            findings.push(ConformanceFinding {
                id: "audio.knob_arc_sweep",
                message: "knob arc sweep must be finite and in (0, 360]".to_string(),
            });
        }
        if self.audio_controls.knob_arc_segments < 12 {
            findings.push(ConformanceFinding {
                id: "audio.knob_arc_segments",
                message: "knob arc should use at least 12 segments".to_string(),
            });
        }
        for (index, width) in self.audio_controls.slider_track_widths.iter().enumerate() {
            if !width.is_finite() || *width <= 0.0 {
                findings.push(ConformanceFinding {
                    id: "audio.slider_track_width",
                    message: format!(
                        "slider track width at index {index} must be finite and positive"
                    ),
                });
            }
        }

        let token_count = self.style_dictionary_tokens().len();
        if token_count < 24 {
            findings.push(ConformanceFinding {
                id: "tokens.coverage",
                message: "style dictionary export should include core shape, spacing, interaction, typography, motion, and audio tokens".to_string(),
            });
        }

        DesignConformanceReport { findings }
    }

    pub fn motion_spec(&self, reduced_motion: bool) -> MotionSpec {
        if reduced_motion {
            MotionSpec {
                duration_ms: 0,
                fast_ms: 0,
                slow_ms: 0,
                prefer_spring: false,
                reduced_motion: true,
            }
        } else {
            MotionSpec {
                duration_ms: self.animation.duration_ms,
                fast_ms: self.animation.fast_ms,
                slow_ms: self.animation.slow_ms,
                prefer_spring: self.animation.prefer_spring,
                reduced_motion: false,
            }
        }
    }
}

fn all_design_presets() -> Vec<(&'static str, DesignSystem)> {
    vec![
        ("neutral", DesignSystem::neutral()),
        ("apple_hig", DesignSystem::apple_hig()),
        ("material3", DesignSystem::material3()),
        ("fluent", DesignSystem::fluent()),
    ]
}

fn finite_positive(
    findings: &mut Vec<ConformanceFinding>,
    id: &'static str,
    value: f32,
    message: &'static str,
) {
    if !value.is_finite() || value <= 0.0 {
        findings.push(ConformanceFinding {
            id,
            message: message.to_string(),
        });
    }
}

fn finite_non_negative(
    findings: &mut Vec<ConformanceFinding>,
    id: &'static str,
    value: f32,
    message: &'static str,
) {
    if !value.is_finite() || value < 0.0 {
        findings.push(ConformanceFinding {
            id,
            message: message.to_string(),
        });
    }
}

fn token(path: &str, value: impl ToString, token_type: &'static str) -> DesignToken {
    DesignToken {
        path: path.split('.').map(str::to_string).collect(),
        value: value.to_string(),
        token_type,
    }
}

// ============================================================================
// GPUI Global integration (behind `gpui` feature)
// ============================================================================

#[cfg(feature = "gpui")]
pub struct DesignSystemState {
    pub system: std::sync::Arc<DesignSystem>,
}

#[cfg(feature = "gpui")]
impl gpui::Global for DesignSystemState {}

#[cfg(feature = "gpui")]
impl DesignSystemState {
    pub fn new() -> Self {
        Self {
            system: std::sync::Arc::new(DesignSystem::platform_default()),
        }
    }

    pub fn with_system(system: DesignSystem) -> Self {
        Self {
            system: std::sync::Arc::new(system),
        }
    }
}

#[cfg(feature = "gpui")]
impl Default for DesignSystemState {
    fn default() -> Self {
        Self::new()
    }
}

/// Extension trait for easy design system access from GPUI `App`.
#[cfg(feature = "gpui")]
pub trait DesignExt {
    fn design(&self) -> std::sync::Arc<DesignSystem>;
}

#[cfg(feature = "gpui")]
impl DesignExt for gpui::App {
    fn design(&self) -> std::sync::Arc<DesignSystem> {
        self.try_global::<DesignSystemState>()
            .map(|s| std::sync::Arc::clone(&s.system))
            .unwrap_or_else(|| std::sync::Arc::new(DesignSystem::platform_default()))
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_neutral_matches_current_solver_constants() {
        let ds = DesignSystem::neutral();
        // These must match the constants in layout_solver.rs exactly
        assert_eq!(ds.layout.vertical_threshold, 400.0);
        assert_eq!(ds.layout.group_stack_threshold, 500.0);
        assert_eq!(ds.layout.compact_slider_threshold, 700.0);
        assert_eq!(ds.layout.hide_viz_threshold, 600.0);
        assert_eq!(ds.layout.compact_knob_threshold, 400.0);
        assert_eq!(ds.layout.large_knob_threshold, 800.0);
        assert_eq!(ds.layout.slider_height_normal, 180.0);
        assert_eq!(ds.layout.slider_height_compact, 120.0);
    }

    #[test]
    fn test_all_presets_construct() {
        let _ = DesignSystem::neutral();
        let _ = DesignSystem::apple_hig();
        let _ = DesignSystem::material3();
        let _ = DesignSystem::fluent();
    }

    #[test]
    fn test_platform_default_returns_valid() {
        let ds = DesignSystem::platform_default();
        // Should be one of the known languages
        assert!(matches!(
            ds.language,
            DesignLanguage::AppleHig
                | DesignLanguage::Material3
                | DesignLanguage::Fluent
                | DesignLanguage::Neutral
        ));
    }

    #[test]
    fn test_platform_mapping_covers_main_os_families() {
        assert_eq!(
            DesignSystem::for_platform(DesignPlatform::Macos).language,
            DesignLanguage::AppleHig
        );
        assert_eq!(
            DesignSystem::for_platform(DesignPlatform::Ios).language,
            DesignLanguage::AppleHig
        );
        assert_eq!(
            DesignSystem::for_platform(DesignPlatform::Windows).language,
            DesignLanguage::Fluent
        );
        assert_eq!(
            DesignSystem::for_platform(DesignPlatform::Android).language,
            DesignLanguage::Material3
        );
        assert_eq!(
            DesignSystem::for_platform(DesignPlatform::Linux).language,
            DesignLanguage::Neutral
        );
        assert_eq!(
            DesignSystem::for_platform(DesignPlatform::Other).language,
            DesignLanguage::Neutral
        );
    }

    #[test]
    fn test_design_language_ids_select_presets() {
        for language in [
            DesignLanguage::AppleHig,
            DesignLanguage::Material3,
            DesignLanguage::Fluent,
            DesignLanguage::Neutral,
        ] {
            assert_eq!(DesignLanguage::from_id(language.as_str()), Some(language));
            assert_eq!(
                DesignSystem::from_language_id(language.as_str())
                    .unwrap()
                    .language,
                language
            );
            assert!(!language.label().is_empty());
        }

        assert_eq!(DesignLanguage::from_id("system"), None);
        assert_eq!(DesignSystem::from_language_id("system"), None);
    }

    #[test]
    fn test_presets_use_gpui_system_font_alias() {
        for (preset_id, system) in all_design_presets() {
            assert_eq!(
                system.typography.font_family, ".SystemUIFont",
                "{preset_id} should resolve through GPUI's native system UI font alias"
            );
        }
    }

    #[test]
    fn test_presets_differ() {
        let neutral = DesignSystem::neutral();
        let apple = DesignSystem::apple_hig();
        let material = DesignSystem::material3();
        let fluent = DesignSystem::fluent();

        // Each preset should have a different language
        assert_ne!(neutral.language, apple.language);
        assert_ne!(apple.language, material.language);
        assert_ne!(material.language, fluent.language);

        // Key differentiators
        assert_ne!(neutral.toggle_variant, material.toggle_variant);
        assert_ne!(apple.corners.style, material.corners.style);
        assert_ne!(neutral.label_position, fluent.label_position);
        assert_ne!(neutral.group_separator, material.group_separator);
    }

    #[test]
    fn test_apple_uses_larger_touch_targets() {
        let apple = DesignSystem::apple_hig();
        let neutral = DesignSystem::neutral();
        assert!(apple.interaction.min_touch_target > neutral.interaction.min_touch_target);
    }

    #[test]
    fn test_material_uses_cards() {
        let material = DesignSystem::material3();
        assert_eq!(material.group_separator, GroupSeparatorStyle::Card);
    }

    #[test]
    fn test_fluent_is_compact() {
        let fluent = DesignSystem::fluent();
        let neutral = DesignSystem::neutral();
        assert!(fluent.spacing.section_gap <= neutral.spacing.section_gap);
        assert!(fluent.layout.slider_height_normal < neutral.layout.slider_height_normal);
    }

    #[test]
    fn test_serializable() {
        let ds = DesignSystem::neutral();
        let json = serde_json::to_string(&ds).unwrap();
        assert!(json.contains("\"language\":\"Neutral\""));
        assert!(json.contains("\"vertical_threshold\":400.0"));
    }

    #[test]
    fn test_design_language_as_str() {
        assert_eq!(DesignLanguage::AppleHig.as_str(), "apple_hig");
        assert_eq!(DesignLanguage::Material3.as_str(), "material3");
        assert_eq!(DesignLanguage::Fluent.as_str(), "fluent");
        assert_eq!(DesignLanguage::Neutral.as_str(), "neutral");
    }

    #[test]
    fn style_dictionary_tokens_include_platform_and_motion() {
        let ds = DesignSystem::apple_hig();
        let tokens = ds.style_dictionary_tokens();

        assert!(tokens.iter().any(|token| token.name() == "design.language"));
        assert!(
            tokens
                .iter()
                .any(|token| token.name() == "motion.duration_ms")
        );
        assert!(
            tokens
                .iter()
                .any(|token| token.name() == "interaction.min_touch_target")
        );
    }

    #[test]
    fn conformance_and_motion_reports_are_stable() {
        let ds = DesignSystem::apple_hig();

        assert!(ds.conformance_report(false).passed());
        assert_eq!(ds.motion_spec(true).duration_ms, 0);
        assert_eq!(ds.motion_spec(false).duration_ms, ds.animation.duration_ms);
    }

    #[test]
    fn conformance_matrix_covers_all_presets_and_motion_modes() {
        let matrix = DesignConformanceMatrix::all_presets();

        assert_eq!(matrix.cases.len(), 8);
        assert!(matrix.passed(), "{}", matrix.to_markdown_table());
        assert!(
            matrix
                .cases
                .iter()
                .any(|case| case.preset_id == "apple_hig" && case.reduced_motion)
        );
        assert!(
            matrix
                .to_markdown_table()
                .contains("| apple_hig | reduced |")
        );
    }

    #[test]
    fn conformance_report_catches_mutated_public_fields() {
        let mut ds = DesignSystem::neutral();
        ds.spacing.grid_unit = 0.0;
        ds.elevation.shadow_opacity = 1.5;
        ds.typography.large_size = ds.typography.base_size - 1.0;
        ds.animation.fast_ms = ds.animation.slow_ms + 1;

        let report = ds.conformance_report(false);
        let ids: Vec<_> = report.findings.iter().map(|finding| finding.id).collect();

        assert!(ids.contains(&"spacing.grid_unit"));
        assert!(ids.contains(&"elevation.shadow_opacity"));
        assert!(ids.contains(&"typography.scale"));
        assert!(ids.contains(&"motion.duration_order"));
    }

    #[test]
    fn style_dictionary_export_is_serializable_and_complete() {
        let export = DesignTokenExport::for_all_presets();
        let json = serde_json::to_string(&export).unwrap();

        assert!(json.contains("\"apple_hig\""));
        assert!(json.contains("\"interaction.min_touch_target\""));
        assert!(json.contains("\"audio.knob_arc_sweep_deg\""));
    }

    #[test]
    fn test_corner_radii_new_validates() {
        let c = CornerRadii::new(4.0, 8.0, 12.0, 16.0, CornerRadiusStyle::Circular);
        assert_eq!(c.sm, 4.0);
    }

    #[test]
    #[should_panic(expected = "sm must be >= 0")]
    fn test_corner_radii_new_rejects_negative() {
        CornerRadii::new(-1.0, 8.0, 12.0, 16.0, CornerRadiusStyle::Circular);
    }

    #[test]
    fn test_spacing_rules_new_validates() {
        let s = SpacingRules::new(4.0, 12.0, 8.0, 8.0, 16.0, 12.0);
        assert_eq!(s.grid_unit, 4.0);
    }

    #[test]
    #[should_panic(expected = "grid_unit must be >= 0")]
    fn test_spacing_rules_new_rejects_negative() {
        SpacingRules::new(-1.0, 12.0, 8.0, 8.0, 16.0, 12.0);
    }

    #[test]
    fn test_elevation_rules_new_validates_opacity() {
        let e = ElevationRules::new(0.0, 4.0, 16.0, 0.15, 2.0);
        assert_eq!(e.shadow_opacity, 0.15);
    }

    #[test]
    #[should_panic(expected = "shadow_opacity must be in [0, 1]")]
    fn test_elevation_rules_new_rejects_invalid_opacity() {
        ElevationRules::new(0.0, 4.0, 16.0, 1.5, 2.0);
    }

    #[test]
    fn test_animation_rules_new_validates() {
        let a = AnimationRules::new(200, 100, 400, false, 170.0, 26.0);
        assert_eq!(a.duration_ms, 200);
    }

    #[test]
    #[should_panic(expected = "duration_ms must be > 0")]
    fn test_animation_rules_new_rejects_zero_duration() {
        AnimationRules::new(0, 100, 400, false, 170.0, 26.0);
    }

    #[test]
    fn test_typography_rules_new_validates() {
        let t = TypographyRules::new(".SystemUIFont", false, 14.0, 11.0, 18.0);
        assert_eq!(t.base_size, 14.0);
    }

    #[test]
    #[should_panic(expected = "base_size must be > 0")]
    fn test_typography_rules_new_rejects_zero_size() {
        TypographyRules::new(".SystemUIFont", false, 0.0, 11.0, 18.0);
    }

    #[test]
    fn test_layout_thresholds_new_validates() {
        let l = LayoutThresholds::new(400.0, 500.0, 700.0, 600.0, 400.0, 800.0, 180.0, 120.0);
        assert_eq!(l.vertical_threshold, 400.0);
    }

    #[test]
    #[should_panic(expected = "vertical_threshold must be > 0")]
    fn test_layout_thresholds_new_rejects_zero() {
        LayoutThresholds::new(0.0, 500.0, 700.0, 600.0, 400.0, 800.0, 180.0, 120.0);
    }

    #[test]
    fn test_audio_control_rules_new_validates() {
        let a = AudioControlRules::new(135.0, 270.0, 2.5, 48, 2.0, [14.0, 18.0, 24.0]);
        assert_eq!(a.knob_arc_segments, 48);
    }

    #[test]
    #[should_panic(expected = "knob_arc_segments must be > 0")]
    fn test_audio_control_rules_new_rejects_zero_segments() {
        AudioControlRules::new(135.0, 270.0, 2.5, 0, 2.0, [14.0, 18.0, 24.0]);
    }
}
