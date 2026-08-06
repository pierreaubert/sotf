use super::types::ModeSelectorInfo;
use sotf_plugins::layout_solver::solve_control_groups;
use sotf_plugins::param_specs::{ParamSpec, ParamType};
use sotf_plugins::plugin_layout::*;

pub(super) fn mode_visible_groups(
    layout: &'static PluginLayout,
    values: &[f64],
    mode: Option<&ModeSelectorInfo>,
) -> Vec<&'static ControlGroup> {
    let active_label_upper: Option<String> = mode.and_then(|info| {
        let v = values.get(info.param_idx).copied().unwrap_or(0.0) as usize;
        info.labels.get(v).map(|s| s.to_uppercase())
    });

    let mut groups = Vec::new();
    for (i, group) in layout.main.iter().enumerate() {
        if let Some(info) = mode
            && info.main_idx == i
        {
            continue;
        }

        if !group.is_visible(values) {
            continue;
        }

        // Explicit conditions supersede the legacy title/choice-label convention.
        if group.visible_when.is_none()
            && let Some(info) = mode
        {
            let title_upper = group.title.to_uppercase();
            let is_exclusive = info
                .labels
                .iter()
                .any(|l| l.eq_ignore_ascii_case(&title_upper));
            if is_exclusive && active_label_upper.as_deref() != Some(title_upper.as_str()) {
                continue;
            }
        }

        groups.push(group);
    }
    groups
}

pub(super) fn solve_main_groups(
    layout: &'static PluginLayout,
    values: &[f64],
    mode: Option<&ModeSelectorInfo>,
    main_width: f32,
) -> (Vec<&'static ControlGroup>, Vec<&'static ControlGroup>) {
    let groups = mode_visible_groups(layout, values, mode);
    let solved = solve_control_groups(&groups, main_width)
        .unwrap_or_else(|error| panic!("invalid generated plugin group layout: {error}"));
    let visible = groups
        .iter()
        .copied()
        .filter(|group| solved.find(group.id).is_some_and(|node| node.visible()))
        .collect();
    let overflow = groups
        .iter()
        .copied()
        .filter(|group| solved.find(group.id).is_some_and(|node| !node.visible()))
        .collect();

    (visible, overflow)
}

/// Detect a "mode selector" pattern in `layout.main`:
/// the first untitled `ControlGroup` containing a single choice control
/// whose bound param is a `Choice`. The labels of that Choice are
/// matched (case-insensitive) against later group titles to determine
/// which groups are mutually exclusive.
pub(super) fn detect_mode_selector(
    layout: &'static PluginLayout,
    params: &[ParamSpec],
) -> Option<ModeSelectorInfo> {
    for (i, group) in layout.main.iter().enumerate() {
        if !group.title.is_empty() || group.controls.len() != 1 {
            continue;
        }
        let spec = &group.controls[0];
        let param = params.get(spec.param_index)?;
        let labels = match (spec.control_type, param.param_type) {
            (ControlType::ButtonSet { labels }, ParamType::Choice { .. }) => labels,
            (ControlType::Selector, ParamType::Choice { labels, .. }) => labels,
            _ => continue,
        };
        // Confirm at least one later group title matches a label — otherwise
        // this isn't really a mode selector and we should leave the layout alone.
        let aliases_any_group = layout.main.iter().enumerate().any(|(j, g)| {
            j != i && !g.title.is_empty() && labels.iter().any(|l| l.eq_ignore_ascii_case(g.title))
        });
        if !aliases_any_group {
            continue;
        }
        return Some(ModeSelectorInfo {
            main_idx: i,
            param_idx: spec.param_index,
            labels,
        });
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    static LABELS: [&str; 2] = ["A", "B"];
    static MODE_CONTROLS: [ControlSpec; 1] = [ControlSpec::button_set(0, &LABELS)];
    static A_CONTROLS: [ControlSpec; 1] = [ControlSpec::knob(1)];
    static GROUPS: [ControlGroup; 2] = [
        ControlGroup::new("mode", "", &MODE_CONTROLS),
        ControlGroup::new("a", "A", &A_CONTROLS).visible_when(ParamCondition::choice(0, 1)),
    ];
    static LAYOUT: PluginLayout = PluginLayout {
        config: &[],
        main: &GROUPS,
        output: &[],
        tabs: &[],
        visualizations: &[],
        column_constraints: &[],
        dynamic_sections: &[],
    };

    #[test]
    fn explicit_group_condition_supersedes_legacy_title_matching() {
        let info = ModeSelectorInfo {
            main_idx: 0,
            param_idx: 0,
            labels: &LABELS,
        };
        let groups = mode_visible_groups(&LAYOUT, &[1.0, 0.0], Some(&info));
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].id, "a");
    }
}
