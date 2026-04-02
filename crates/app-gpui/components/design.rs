//! Design system helpers for consistent spacing, corners, and typography.
//!
//! Call `Ds::from_cx(cx)` once at the top of each render function, then use
//! `d.pad_x`, `d.gap`, `d.r_md`, `d.text_sm`, etc. in method chains.

use gpui::{Pixels, px};
use gpui_design::DesignExt;

/// Pre-computed design system values as `Pixels` for direct use in GPUI method chains.
///
/// # Example
/// ```ignore
/// let d = Ds::from_cx(cx);
/// div().px(d.pad_x).py(d.pad_y).gap(d.gap).rounded(d.r_md).text_size(d.text_sm)
/// ```
#[derive(Clone, Copy)]
pub struct Ds {
    /// Grid unit (4px) — smallest spacing increment
    pub grid: Pixels,
    /// Control horizontal padding (12px)
    pub pad_x: Pixels,
    /// Control vertical padding (8px)
    pub pad_y: Pixels,
    /// Half vertical padding (4px)
    pub pad_y_half: Pixels,
    /// Control gap (8px) — space between sibling controls
    pub gap: Pixels,
    /// 1.5× control gap (12px)
    pub gap_md: Pixels,
    /// Section gap (16px) — space between sections
    pub section: Pixels,
    /// 1.5× section gap (24px)
    pub section_lg: Pixels,
    /// 2× section gap (32px)
    pub section_xl: Pixels,
    /// Card padding (16px)
    pub card: Pixels,
    /// Corner radius: small
    pub r_sm: Pixels,
    /// Corner radius: medium
    pub r_md: Pixels,
    /// Corner radius: large
    pub r_lg: Pixels,
    /// Corner radius: extra-large
    pub r_xl: Pixels,
    /// Text size: extra-small (~10px)
    pub text_xs: Pixels,
    /// Text size: small (~12px)
    pub text_sm: Pixels,
    /// Text size: base (~14px)
    pub text_base: Pixels,
    /// Text size: large (~16px)
    pub text_lg: Pixels,
}

impl Ds {
    pub fn from_cx(cx: &gpui::App) -> Self {
        let ds = cx.design();
        Self {
            grid: px(ds.spacing.grid_unit),
            pad_x: px(ds.spacing.control_padding_x),
            pad_y: px(ds.spacing.control_padding_y),
            pad_y_half: px(ds.spacing.control_padding_y * 0.5),
            gap: px(ds.spacing.control_gap),
            gap_md: px(ds.spacing.control_gap * 1.5),
            section: px(ds.spacing.section_gap),
            section_lg: px(ds.spacing.section_gap * 1.5),
            section_xl: px(ds.spacing.section_gap * 2.0),
            card: px(ds.spacing.card_padding),
            r_sm: px(ds.corners.sm),
            r_md: px(ds.corners.md),
            r_lg: px(ds.corners.lg),
            r_xl: px(ds.corners.xl),
            text_xs: px(ds.typography.small_size * 0.85),
            text_sm: px(ds.typography.small_size),
            text_base: px(ds.typography.base_size),
            text_lg: px(ds.typography.large_size),
        }
    }
}
