//! Design system helpers for consistent spacing, corners, and typography.
//!
//! Call `Ds::from_cx(cx)` once at the top of each render function, then use
//! `d.pad_x`, `d.gap`, `d.r_md`, `d.text_sm`, etc. in method chains.
//!
//! # Rem-based vs pixel-based fields
//!
//! Fields that should scale with the user's font-zoom setting are expressed in
//! `Rems` (they multiply against `window.rem_size`, which is updated by the
//! `IncreaseFontSize` / `DecreaseFontSize` / `ResetFontSize` actions in
//! `ui/render.rs`). Corner radii are kept in absolute `Pixels` — scaling radii
//! with font zoom produces visually "bubbly" zoomed UI, so the convention is
//! that radii track the design language (Apple HIG / Material 3 / Fluent)
//! rather than the text scale.
//!
//! # Typography unification (Typography Phase 1)
//!
//! The text-sized fields (`text_sm`, `text_base`, `text_lg`, `text_xl`,
//! `text_xxl`) derive from the active [`gpui_design::TypographyRules`] so
//! platform presets affect more than font family. The neutral preset preserves
//! the previous scale: caption 10px, small 14px, base 16px, large 18px, xl
//! 20px, and xxl 24px at the 16px rem baseline.
//!
//! The exception is `text_xs`: it stays a specialized "caption / axis tick"
//! size. Use `text_xs` for chart axis labels, badges, and other micro-type;
//! use `d.text_sm` for body-small labels.

use gpui::{Pixels, Rems, px, rems};
use gpui_design::{DesignExt, TypographyRules};

/// GPUI's default root rem size, matching the baseline used when
/// `window.set_rem_size` has not been adjusted. Design-token pixel values are
/// divided by this constant to produce rem-relative sizes.
const BASE_REM_PX: f32 = 16.0;

const LEGACY_BASE_SIZE_PX: f32 = 14.0;
const LEGACY_LARGE_SIZE_PX: f32 = 18.0;
const LEGACY_CAPTION_OFFSET_PX: f32 = 1.0;

/// Typography rem values derived from platform typography tokens.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TypographyRems {
    pub text_xs: Rems,
    pub text_sm: Rems,
    pub text_base: Rems,
    pub text_lg: Rems,
    pub text_xl: Rems,
    pub text_xxl: Rems,
}

/// Convert platform typography tokens into the app's historical size slots.
///
/// The neutral preset stays byte-for-byte equivalent to the old scale while
/// Apple / Material / Fluent can tune base and heading size through their
/// design-system presets.
pub fn typography_rems_from_rules(typography: &TypographyRules) -> TypographyRems {
    let to_rems = |px_value: f32| rems(px_value / BASE_REM_PX);
    let caption_px = (typography.small_size - LEGACY_CAPTION_OFFSET_PX).max(8.0);

    TypographyRems {
        text_xs: to_rems(caption_px),
        text_sm: to_rems(typography.base_size),
        text_base: to_rems(typography.base_size * (16.0 / LEGACY_BASE_SIZE_PX)),
        text_lg: to_rems(typography.large_size),
        text_xl: to_rems(typography.large_size * (20.0 / LEGACY_LARGE_SIZE_PX)),
        text_xxl: to_rems(typography.large_size * (24.0 / LEGACY_LARGE_SIZE_PX)),
    }
}

/// Pre-computed design system values for direct use in GPUI method chains.
///
/// Scalable fields (spacing, text) are `Rems` and respond to font zoom.
/// Corner radii are `Pixels` and remain constant across zoom levels.
///
/// # Example
/// ```ignore
/// let d = Ds::from_cx(cx);
/// div().px(d.pad_x).py(d.pad_y).gap(d.gap).rounded(d.r_md).text_size(d.text_sm)
/// ```
#[derive(Clone, Copy)]
pub struct Ds {
    /// Half-grid unit (~2px) — hairline spacing that still follows zoom.
    pub half_grid: Rems,
    /// Grid unit (~4px) — smallest spacing increment. Scales with font zoom.
    pub grid: Rems,
    /// Control horizontal padding (~12px). Scales with font zoom.
    pub pad_x: Rems,
    /// Control vertical padding (~8px). Scales with font zoom.
    pub pad_y: Rems,
    /// Half vertical padding (~4px). Scales with font zoom.
    pub pad_y_half: Rems,
    /// Control gap (~8px) — space between sibling controls. Scales with font zoom.
    pub gap: Rems,
    /// 1.5× control gap (~12px). Scales with font zoom.
    pub gap_md: Rems,
    /// Section gap (~16px). Scales with font zoom.
    pub section: Rems,
    /// 1.5× section gap (~24px). Scales with font zoom.
    pub section_lg: Rems,
    /// 2× section gap (~32px). Scales with font zoom.
    pub section_xl: Rems,
    /// Card padding (~16px). Scales with font zoom.
    pub card: Rems,
    /// Corner radius: small. Fixed in pixels — does not scale.
    pub r_sm: Pixels,
    /// Corner radius: medium. Fixed in pixels — does not scale.
    pub r_md: Pixels,
    /// Corner radius: large. Fixed in pixels — does not scale.
    pub r_lg: Pixels,
    /// Corner radius: extra-large. Fixed in pixels — does not scale.
    pub r_xl: Pixels,
    /// Text size: caption (~10 px at baseline). Intentionally smaller than
    /// [`TextSize::Xs`] (12 px) for chart axis ticks, badges, and micro-type.
    pub text_xs: Rems,
    /// Text size: small. Matches [`TextSize::Sm`] — 0.875 rem (~14 px).
    pub text_sm: Rems,
    /// Text size: base. Matches [`TextSize::Md`] — 1.0 rem (~16 px).
    pub text_base: Rems,
    /// Text size: large. Matches [`TextSize::Lg`] — 1.125 rem (~18 px).
    pub text_lg: Rems,
    /// Text size: extra-large. Matches [`TextSize::Xl`] — 1.25 rem (~20 px).
    pub text_xl: Rems,
    /// Text size: 2× large. Matches [`TextSize::Xxl`] — 1.5 rem (~24 px).
    pub text_xxl: Rems,
}

impl Ds {
    pub fn from_cx(cx: &gpui::App) -> Self {
        let ds = cx.design();
        let to_rems = |px_value: f32| rems(px_value / BASE_REM_PX);
        Self {
            half_grid: to_rems(ds.spacing.grid_unit * 0.5),
            grid: to_rems(ds.spacing.grid_unit),
            pad_x: to_rems(ds.spacing.control_padding_x),
            pad_y: to_rems(ds.spacing.control_padding_y),
            pad_y_half: to_rems(ds.spacing.control_padding_y * 0.5),
            gap: to_rems(ds.spacing.control_gap),
            gap_md: to_rems(ds.spacing.control_gap * 1.5),
            section: to_rems(ds.spacing.section_gap),
            section_lg: to_rems(ds.spacing.section_gap * 1.5),
            section_xl: to_rems(ds.spacing.section_gap * 2.0),
            card: to_rems(ds.spacing.card_padding),
            r_sm: px(ds.corners.sm),
            r_md: px(ds.corners.md),
            r_lg: px(ds.corners.lg),
            r_xl: px(ds.corners.xl),
            ..Self::from_typography_rems(typography_rems_from_rules(&ds.typography))
        }
    }

    fn from_typography_rems(typography: TypographyRems) -> Self {
        Self {
            half_grid: rems(0.0),
            grid: rems(0.0),
            pad_x: rems(0.0),
            pad_y: rems(0.0),
            pad_y_half: rems(0.0),
            gap: rems(0.0),
            gap_md: rems(0.0),
            section: rems(0.0),
            section_lg: rems(0.0),
            section_xl: rems(0.0),
            card: rems(0.0),
            r_sm: px(0.0),
            r_md: px(0.0),
            r_lg: px(0.0),
            r_xl: px(0.0),
            text_xs: typography.text_xs,
            text_sm: typography.text_sm,
            text_base: typography.text_base,
            text_lg: typography.text_lg,
            text_xl: typography.text_xl,
            text_xxl: typography.text_xxl,
        }
    }
}
