use crate::app::actions;
use gpui::KeyBinding;

/// Bindings for the plugin control context
pub(super) fn plugin_control_bindings() -> Vec<KeyBinding> {
    vec![
        KeyBinding::new("up", actions::IncrementPluginParam, Some("plugin-control")),
        KeyBinding::new(
            "right",
            actions::IncrementPluginParam,
            Some("plugin-control"),
        ),
        KeyBinding::new(
            "down",
            actions::DecrementPluginParam,
            Some("plugin-control"),
        ),
        KeyBinding::new(
            "left",
            actions::DecrementPluginParam,
            Some("plugin-control"),
        ),
        KeyBinding::new(
            "shift-up",
            actions::IncrementPluginParamSmall,
            Some("plugin-control"),
        ),
        KeyBinding::new(
            "shift-right",
            actions::IncrementPluginParamSmall,
            Some("plugin-control"),
        ),
        KeyBinding::new(
            "shift-down",
            actions::DecrementPluginParamSmall,
            Some("plugin-control"),
        ),
        KeyBinding::new(
            "shift-left",
            actions::DecrementPluginParamSmall,
            Some("plugin-control"),
        ),
        KeyBinding::new(
            "pageup",
            actions::IncrementPluginParamLarge,
            Some("plugin-control"),
        ),
        KeyBinding::new(
            "pagedown",
            actions::DecrementPluginParamLarge,
            Some("plugin-control"),
        ),
        KeyBinding::new("+", actions::IncrementPluginParam, Some("plugin-control")),
        KeyBinding::new("=", actions::IncrementPluginParam, Some("plugin-control")),
        KeyBinding::new("-", actions::DecrementPluginParam, Some("plugin-control")),
        KeyBinding::new("_", actions::DecrementPluginParam, Some("plugin-control")),
        KeyBinding::new("a", actions::ToggleABPath, Some("ABCompare")),
        // EQ band navigation (next/prev)
        KeyBinding::new("tab", actions::SelectNextEqBand, Some("plugin-control")),
        KeyBinding::new(
            "shift-tab",
            actions::SelectPrevEqBand,
            Some("plugin-control"),
        ),
        KeyBinding::new("]", actions::SelectNextEqBand, Some("plugin-control")),
        KeyBinding::new("[", actions::SelectPrevEqBand, Some("plugin-control")),
        // Band selection for multiband plugins
        KeyBinding::new("0", actions::SelectBandGlobal, Some("plugin-control")),
        KeyBinding::new("1", actions::SelectBand1, Some("plugin-control")),
        KeyBinding::new("2", actions::SelectBand2, Some("plugin-control")),
        KeyBinding::new("3", actions::SelectBand3, Some("plugin-control")),
        KeyBinding::new("4", actions::SelectBand4, Some("plugin-control")),
        KeyBinding::new("5", actions::SelectBand5, Some("plugin-control")),
    ]
}

/// Arrow-key editing for the focused EQ chart. This context is registered
/// after the generic plugin-control bindings so chart focus owns the arrows.
pub(super) fn eq_chart_bindings() -> Vec<KeyBinding> {
    vec![
        KeyBinding::new("left", actions::EqChartNudgeLeft, Some("EqChart")),
        KeyBinding::new("right", actions::EqChartNudgeRight, Some("EqChart")),
        KeyBinding::new("up", actions::EqChartNudgeUp, Some("EqChart")),
        KeyBinding::new("down", actions::EqChartNudgeDown, Some("EqChart")),
        KeyBinding::new("shift-left", actions::EqChartNudgeLeftFine, Some("EqChart")),
        KeyBinding::new(
            "shift-right",
            actions::EqChartNudgeRightFine,
            Some("EqChart"),
        ),
        KeyBinding::new("shift-up", actions::EqChartNudgeUpFine, Some("EqChart")),
        KeyBinding::new("shift-down", actions::EqChartNudgeDownFine, Some("EqChart")),
    ]
}
