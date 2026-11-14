pub(super) const ROOM_EQ_PYTHON_DEFAULT_SMOOTHING_OCTAVES: f64 = 1.0 / 6.0;

/// Window-width threshold above which the per-channel review charts
/// switch from stacked to a 2-column grid.
pub(super) const ROOM_EQ_REVIEW_WIDE_BREAKPOINT_PX: f32 = 1600.0;

pub(super) const ROOM_EQ_CHANNEL_COLORS: [u32; 10] = [
    0x001f_77b4, // blue
    0x00ff_7f0e, // orange
    0x002c_a02c, // green
    0x00d6_2728, // red
    0x0094_67bd, // purple
    0x008c_564b, // brown
    0x00e3_77c2, // pink
    0x007f_7f7f, // gray
    0x00bc_bd22, // olive
    0x0017_becf, // cyan
];

pub(super) const ROOM_EQ_DRIVER_OPACITIES: [f32; 4] = [1.0, 0.78, 0.58, 0.42];

/// Fixed-canvas chart width for a computed available width.
///
/// ui.md Phase 4 rollout: legibility floors must never exceed the space the
/// chart actually has. At normal widths this is identical to
/// `available.clamp(floor, ceiling)`; on narrow windows the floor yields to
/// the available width instead of forcing horizontal overflow.
#[must_use]
pub fn room_eq_chart_width(available_px: f32, floor_px: f32, ceiling_px: f32) -> f32 {
    let available = available_px.max(1.0);
    available.clamp(floor_px.min(available), ceiling_px)
}
