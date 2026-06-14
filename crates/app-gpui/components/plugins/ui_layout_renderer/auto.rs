use super::misc::AUTO_COLUMN_MIN_MAIN_WIDTH;
use super::misc::AUTO_COLUMN_MIN_SIDE_WIDTH;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{CollapseDirection, PaneDivider, PaneDividerTheme};

pub(super) fn auto_side_max_width(
    available_width: f32,
    other_side_width: f32,
    divider_total: f32,
) -> f32 {
    (available_width - other_side_width - divider_total - AUTO_COLUMN_MIN_MAIN_WIDTH)
        .max(AUTO_COLUMN_MIN_SIDE_WIDTH)
}

pub(super) fn auto_tab_divider(plugin_idx: usize, theme: PaneDividerTheme) -> impl IntoElement {
    PaneDivider::horizontal(
        SharedString::from(format!("plugin-auto-{plugin_idx}-main-tabs")),
        CollapseDirection::Down,
    )
    .theme(theme)
    .thickness(px(4.0))
}
