use super::misc::tab_name_for_role;
use super::solved_layout::SolvedLayout;
use super::types::CollapsedTab;
use super::types::Direction;
use super::types::KnobSize;
use super::types::Orientation;
use super::types::SolvedColumn;
use crate::design_system::DesignSystem;
use crate::plugin_layout::{ColumnConstraint, ColumnRole};

/// Solve the layout using the neutral design system (backward-compat wrapper).
pub fn solve_layout(constraints: &[ColumnConstraint], available_width: f32) -> SolvedLayout {
    solve_layout_with_ds(constraints, available_width, &DesignSystem::neutral())
}

/// Solve the layout for the given constraints, available space, and design system.
///
/// Returns a `SolvedLayout` describing which columns are visible, which
/// became tabs, and what internal adaptations to apply.
pub fn solve_layout_with_ds(
    constraints: &[ColumnConstraint],
    available_width: f32,
    ds: &DesignSystem,
) -> SolvedLayout {
    let lt = &ds.layout;

    // 1. Vertical mode: all collapsible columns become tabs
    if available_width < lt.vertical_threshold {
        return solve_vertical(constraints, available_width, ds);
    }

    // 2. Find the Main column (never collapses)
    let main_constraint = constraints
        .iter()
        .find(|c| c.role == ColumnRole::Main)
        .copied();

    let main_min = main_constraint.map_or(300.0, |c| c.min_width);

    // 3. Collect collapsible columns, sorted by priority ascending (lowest collapses first)
    let mut collapsible: Vec<&ColumnConstraint> =
        constraints.iter().filter(|c| c.collapsible).collect();
    collapsible.sort_by(|a, b| {
        a.priority
            .partial_cmp(&b.priority)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // 4. Greedily allocate space: try to fit columns from highest priority down
    let mut remaining = available_width - main_min;
    let mut visible: Vec<SolvedColumn> = Vec::new();
    let mut collapsed: Vec<CollapsedTab> = Vec::new();

    // Process from highest priority to lowest (reverse order since sorted ascending)
    for constraint in collapsible.iter().rev() {
        if remaining >= constraint.min_width {
            let allocated = constraint.preferred_width.min(remaining);
            remaining -= allocated;
            visible.push(SolvedColumn {
                role: constraint.role,
                width: allocated,
            });
        } else {
            collapsed.push(CollapsedTab {
                role: constraint.role,
                name: tab_name_for_role(constraint.role),
            });
        }
    }

    // 5. Main gets all remaining space (flex), but at least min_width
    let main_width = (main_min + remaining).max(main_min);

    // 6. Build final column order: Config (left) → Main (center) → Diagnostic → Output (right)
    let mut columns = Vec::with_capacity(visible.len() + 1);
    if let Some(pos) = visible.iter().position(|c| c.role == ColumnRole::Config) {
        columns.push(visible[pos]);
    }
    columns.push(SolvedColumn {
        role: ColumnRole::Main,
        width: main_width,
    });
    if let Some(pos) = visible
        .iter()
        .position(|c| c.role == ColumnRole::Diagnostic)
    {
        columns.push(visible[pos]);
    }
    if let Some(pos) = visible.iter().position(|c| c.role == ColumnRole::Output) {
        columns.push(visible[pos]);
    }

    // 7. Internal adaptations — use main_width (not available_width) for decisions
    //    about main-column content, since sidebars consume part of available_width.
    let group_direction = if main_width < lt.group_stack_threshold {
        Direction::Column
    } else {
        Direction::Row
    };

    let slider_height = if main_width < lt.compact_slider_threshold {
        lt.slider_height_compact
    } else {
        lt.slider_height_normal
    };

    let show_visualizations = main_width >= lt.hide_viz_threshold;

    let knob_size = if main_width < lt.compact_knob_threshold {
        KnobSize::Xs
    } else if main_width >= lt.large_knob_threshold {
        KnobSize::Md
    } else {
        KnobSize::Sm
    };

    SolvedLayout {
        columns,
        collapsed_tabs: collapsed,
        orientation: Orientation::Horizontal,
        group_direction,
        slider_height,
        show_visualizations,
        knob_size,
    }
}

/// Vertical mode (mobile): only Main visible, everything else becomes tabs.
fn solve_vertical(
    constraints: &[ColumnConstraint],
    available_width: f32,
    ds: &DesignSystem,
) -> SolvedLayout {
    let main_min = constraints
        .iter()
        .find(|c| c.role == ColumnRole::Main)
        .map_or(300.0, |c| c.min_width);

    let collapsed: Vec<CollapsedTab> = constraints
        .iter()
        .filter(|c| c.collapsible)
        .map(|c| CollapsedTab {
            role: c.role,
            name: tab_name_for_role(c.role),
        })
        .collect();

    SolvedLayout {
        columns: vec![SolvedColumn {
            role: ColumnRole::Main,
            width: available_width.max(main_min),
        }],
        collapsed_tabs: collapsed,
        orientation: Orientation::Vertical,
        group_direction: Direction::Column,
        slider_height: ds.layout.slider_height_compact,
        show_visualizations: false,
        knob_size: KnobSize::Xs,
    }
}
