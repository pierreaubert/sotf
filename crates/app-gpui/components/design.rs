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
//! # Typography unification (Typography Phase 2)
//!
//! The text-sized fields (`text_sm`, `text_base`, `text_lg`, `text_xl`,
//! `text_xxl`) delegate to [`gpui_ui_kit::TextSize::to_rems`] so that both
//! styling APIs resolve to identical rem values. A call to `Text::new(...)
//! .size(TextSize::Sm)` and a call to `div().text_size(d.text_sm)` now render
//! at the same size, whereas previously they could drift (e.g. 14 px vs 11 px
//! at 1× zoom with the neutral platform rules).
//!
//! The exception is `text_xs`: it stays a specialized "caption / axis tick"
//! size, smaller than [`TextSize::Xs`] by design. Use `text_xs` for chart axis
//! labels, badges, and other micro-type; use `TextSize::Xs` / `d.text_sm` for
//! body-small labels.

use gpui::{Pixels, Rems, px, rems};
use gpui_design::DesignExt;
use gpui_ui_kit::TextSize;

/// GPUI's default root rem size, matching the baseline used when
/// `window.set_rem_size` has not been adjusted. Design-token pixel values are
/// divided by this constant to produce rem-relative sizes.
const BASE_REM_PX: f32 = 16.0;

/// Rem value for the caption-sized `text_xs` field. Intentionally smaller than
/// [`TextSize::Xs`] (0.75 rem) so chart axis ticks, badges, and other
/// micro-type can shrink below body-small without callers reaching for raw
/// `rems()` values. Equivalent to roughly 10 px at the 16 px rem baseline.
const TEXT_XS_CAPTION_REMS: f32 = 0.625;

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
            // Typography fields delegate to `TextSize::to_rems()` so both APIs
            // (`Text::new(...).size(TextSize::Sm)` and `.text_size(d.text_sm)`)
            // resolve to identical values — see module docs. `text_xs` stays
            // smaller than `TextSize::Xs` by design (chart tick / caption use).
            text_xs: rems(TEXT_XS_CAPTION_REMS),
            text_sm: TextSize::Sm.to_rems(),
            text_base: TextSize::Md.to_rems(),
            text_lg: TextSize::Lg.to_rems(),
            text_xl: TextSize::Xl.to_rems(),
            text_xxl: TextSize::Xxl.to_rems(),
        }
    }
}
