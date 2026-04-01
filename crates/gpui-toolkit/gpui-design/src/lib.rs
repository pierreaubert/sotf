//! Platform-Adaptive Design System
//!
//! Defines shape, spacing, interaction, and animation rules that vary per
//! platform (Apple HIG, Material Design 3, Windows Fluent) while the Theme
//! system handles colors independently. The two layers are independently
//! combinable: any color theme works with any design system.
//!
//! This module contains only data types — no rendering code, no framework deps.
//! Platform renderers consume it alongside Theme colors.

use serde::Serialize;

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
                font_family: ".SystemUI".to_string(),
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
                font_family: "SF Pro".to_string(),
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
                font_family: "Roboto".to_string(),
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
                font_family: "Segoe UI Variable".to_string(),
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

    /// Returns the platform-appropriate default design system.
    pub fn platform_default() -> Self {
        #[cfg(any(target_os = "macos", target_os = "ios"))]
        {
            Self::apple_hig()
        }
        #[cfg(target_os = "windows")]
        {
            Self::fluent()
        }
        #[cfg(target_os = "android")]
        {
            Self::material3()
        }
        #[cfg(not(any(
            target_os = "macos",
            target_os = "ios",
            target_os = "windows",
            target_os = "android"
        )))]
        {
            Self::neutral()
        }
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
}

// ============================================================================
// GPUI Global integration (behind `gpui` feature)
// ============================================================================

#[cfg(feature = "gpui")]
pub struct DesignSystemState {
    pub system: DesignSystem,
}

#[cfg(feature = "gpui")]
impl gpui::Global for DesignSystemState {}

#[cfg(feature = "gpui")]
impl DesignSystemState {
    pub fn new() -> Self {
        Self {
            system: DesignSystem::platform_default(),
        }
    }

    pub fn with_system(system: DesignSystem) -> Self {
        Self { system }
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
    fn design(&self) -> DesignSystem;
}

#[cfg(feature = "gpui")]
impl DesignExt for gpui::App {
    fn design(&self) -> DesignSystem {
        self.try_global::<DesignSystemState>()
            .map(|s| s.system.clone())
            .unwrap_or_else(DesignSystem::platform_default)
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
}
