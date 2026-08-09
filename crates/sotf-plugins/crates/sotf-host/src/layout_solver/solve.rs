use super::misc::tab_name_for_role;
use super::solved_layout::SolvedLayout;
use super::types::{CollapsedTab, Direction, KnobSize, Orientation, SolvedColumn};
use crate::design_system::DesignSystem;
use crate::plugin_layout::{ColumnConstraint, ColumnRole};
use gpui_builder::{
    ColumnRole as BuilderColumnRole, GroupDirection as BuilderGroupDirection,
    KnobSize as BuilderKnobSize, PluginColumnConstraint, PluginLayoutThresholds, PluginLayoutTree,
    plugin_adaptations,
};
use gpui_builder::{LayoutPreferences, solve};

/// Solve the layout using the neutral design system (backward-compatible API).
///
/// This is now a compatibility wrapper over the generic `gpui-builder`
/// constraint solver, preserving the existing `SolvedLayout` public shape.
pub fn solve_layout(constraints: &[ColumnConstraint], available_width: f32) -> SolvedLayout {
    solve_layout_with_ds(constraints, available_width, &DesignSystem::neutral())
}

/// Solve the layout with dimensions scaled to the host's effective UI scale.
pub fn solve_layout_scaled(
    constraints: &[ColumnConstraint],
    available_width: f32,
    scale: f32,
) -> SolvedLayout {
    solve_layout_with_ds_and_scale(
        constraints,
        available_width,
        &DesignSystem::neutral(),
        scale,
    )
}

/// Solve the layout for the given constraints, available space, and design system.
pub fn solve_layout_with_ds(
    constraints: &[ColumnConstraint],
    available_width: f32,
    ds: &DesignSystem,
) -> SolvedLayout {
    solve_layout_with_ds_and_scale(constraints, available_width, ds, 1.0)
}

/// Solve the layout for a design system at the host's effective UI scale.
pub fn solve_layout_with_ds_and_scale(
    constraints: &[ColumnConstraint],
    available_width: f32,
    ds: &DesignSystem,
    scale: f32,
) -> SolvedLayout {
    let scale = if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    };
    let logical_width = available_width / scale;
    let converted: Vec<_> = constraints.iter().map(convert_constraint).collect();
    let tree = PluginLayoutTree::from_constraints(&converted);
    let source = tree.as_layout_node();
    let main_min = constraints
        .iter()
        .find(|constraint| constraint.role == ColumnRole::Main)
        .map_or(300.0, |constraint| constraint.min_width);
    let vertical = logical_width < ds.layout.vertical_threshold;
    let solve_width = if vertical {
        logical_width.min(main_min)
    } else {
        logical_width
    };
    let solved = solve(
        &source,
        solve_width.max(0.0),
        1.0,
        &LayoutPreferences::default(),
    );
    let thresholds = PluginLayoutThresholds {
        vertical_threshold: ds.layout.vertical_threshold,
        group_stack_threshold: ds.layout.group_stack_threshold,
        compact_slider_threshold: ds.layout.compact_slider_threshold,
        hide_viz_threshold: ds.layout.hide_viz_threshold,
        compact_knob_threshold: ds.layout.compact_knob_threshold,
        large_knob_threshold: ds.layout.large_knob_threshold,
        slider_height_normal: ds.layout.slider_height_normal,
        slider_height_compact: ds.layout.slider_height_compact,
    };
    let adaptations = plugin_adaptations(&solved, &thresholds);

    let order = [
        ColumnRole::Config,
        ColumnRole::Diagnostic,
        ColumnRole::Output,
    ];
    let mut collapsed_tabs = Vec::new();
    let mut visible_sidebars: Vec<_> = constraints
        .iter()
        .filter(|constraint| constraint.role != ColumnRole::Main)
        .filter(|constraint| {
            solved
                .find(role_id(constraint.role))
                .is_some_and(|node| node.visible)
        })
        .collect();
    visible_sidebars.sort_by(|a, b| {
        b.priority
            .partial_cmp(&a.priority)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut remaining = (logical_width - main_min).max(0.0);
    let mut allocated_sidebars = Vec::with_capacity(visible_sidebars.len());
    for constraint in visible_sidebars {
        if remaining >= constraint.min_width {
            let width = constraint.preferred_width.min(remaining);
            remaining -= width;
            allocated_sidebars.push((constraint.role, width));
        }
    }

    for role in order {
        let id = role_id(role);
        let Some(node) = solved.find(id) else {
            continue;
        };
        if !node.visible
            || !allocated_sidebars
                .iter()
                .any(|(allocated_role, _)| *allocated_role == role)
        {
            collapsed_tabs.push(CollapsedTab {
                role,
                name: tab_name_for_role(role),
            });
        }
    }

    let main_width = if vertical {
        logical_width.max(main_min)
    } else {
        main_min + remaining
    };
    let sidebar_width = |role| {
        allocated_sidebars
            .iter()
            .find_map(|(candidate, width)| (*candidate == role).then_some(*width))
    };
    let mut columns = Vec::with_capacity(allocated_sidebars.len() + 1);
    if let Some(width) = sidebar_width(ColumnRole::Config) {
        columns.push(SolvedColumn {
            role: ColumnRole::Config,
            width: width * scale,
        });
    }
    columns.push(SolvedColumn {
        role: ColumnRole::Main,
        width: main_width * scale,
    });
    for role in [ColumnRole::Diagnostic, ColumnRole::Output] {
        if let Some(width) = sidebar_width(role) {
            columns.push(SolvedColumn {
                role,
                width: width * scale,
            });
        }
    }

    let group_direction = if vertical {
        Direction::Column
    } else {
        match adaptations.group_direction {
            BuilderGroupDirection::Row => Direction::Row,
            BuilderGroupDirection::Column => Direction::Column,
        }
    };
    let knob_size = match adaptations.knob_size {
        BuilderKnobSize::Xs => KnobSize::Xs,
        BuilderKnobSize::Sm => KnobSize::Sm,
        BuilderKnobSize::Md => KnobSize::Md,
    };

    SolvedLayout {
        columns,
        collapsed_tabs,
        orientation: if vertical {
            Orientation::Vertical
        } else {
            Orientation::Horizontal
        },
        group_direction,
        slider_height: if main_width < ds.layout.compact_slider_threshold {
            ds.layout.slider_height_compact * scale
        } else {
            ds.layout.slider_height_normal * scale
        },
        show_visualizations: !vertical && main_width >= ds.layout.hide_viz_threshold,
        knob_size,
    }
}

fn convert_constraint(constraint: &ColumnConstraint) -> PluginColumnConstraint {
    PluginColumnConstraint {
        role: match constraint.role {
            ColumnRole::Config => BuilderColumnRole::Config,
            ColumnRole::Main => BuilderColumnRole::Main,
            ColumnRole::Output => BuilderColumnRole::Output,
            ColumnRole::Diagnostic => BuilderColumnRole::Diagnostic,
        },
        min_width: constraint.min_width,
        preferred_width: constraint.preferred_width,
        max_width: constraint.max_width,
        priority: constraint.priority,
        collapsible: constraint.collapsible,
    }
}

fn role_id(role: ColumnRole) -> &'static str {
    match role {
        ColumnRole::Config => "config",
        ColumnRole::Main => "main",
        ColumnRole::Output => "output",
        ColumnRole::Diagnostic => "diagnostic",
    }
}
