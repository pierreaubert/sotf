//! Layout Constraint Solver
//!
//! Lightweight greedy solver that decides which columns are visible vs. collapsed
//! to tabs based on available screen space. Platform-agnostic — same solver runs
//! on GPUI, SwiftUI, Android.
//!
//! Algorithm:
//! 1. If width < VERTICAL_THRESHOLD → vertical mode, all collapsible columns become tabs
//! 2. Reserve min_width for Main (never collapses)
//! 3. Greedily fit remaining columns by priority (highest first)
//! 4. Columns that don't fit become tabs
//! 5. Distribute leftover space to Main (flex)
//! 6. Determine internal adaptations (slider height, group direction, viz visibility)

use crate::plugin_layout::{ColumnConstraint, ColumnRole};

// ============================================================================
// Thresholds
// ============================================================================

/// Below this width, switch to vertical orientation (mobile).
pub const VERTICAL_THRESHOLD: f32 = 400.0;

/// Below this width, stack control groups vertically instead of side-by-side.
pub const GROUP_STACK_THRESHOLD: f32 = 500.0;

/// Below this width, use compact slider height (120px instead of 180px).
pub const COMPACT_SLIDER_THRESHOLD: f32 = 700.0;

/// Below this width, hide visualizations.
pub const HIDE_VIZ_THRESHOLD: f32 = 600.0;

/// Below this main-column width, use compact (Xs) knobs instead of Sm.
pub const COMPACT_KNOB_THRESHOLD: f32 = 400.0;

/// Above this main-column width, use medium (Md) knobs instead of Sm.
pub const LARGE_KNOB_THRESHOLD: f32 = 800.0;

/// Standard slider height in pixels.
pub const SLIDER_HEIGHT_NORMAL: f32 = 180.0;

/// Compact slider height in pixels.
pub const SLIDER_HEIGHT_COMPACT: f32 = 120.0;

// ============================================================================
// Solver Output
// ============================================================================

/// Orientation of the overall layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Orientation {
    /// Normal desktop: columns side-by-side.
    Horizontal,
    /// Mobile/very narrow: main on top, everything else below.
    Vertical,
}

/// Direction for arranging control groups within the main area.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// Groups arranged side-by-side (e.g., DYNAMICS | TIMING).
    Row,
    /// Groups stacked vertically.
    Column,
}

/// Knob size tier chosen by the solver based on available width.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KnobSize {
    /// Extra-compact for very narrow layouts.
    Xs,
    /// Default compact size.
    Sm,
    /// Medium size for wide layouts.
    Md,
}

/// A column that the solver decided should be visible.
#[derive(Debug, Clone, Copy)]
pub struct SolvedColumn {
    pub role: ColumnRole,
    /// Allocated width in pixels.
    pub width: f32,
}

/// A column that was collapsed into a tab.
#[derive(Debug, Clone, Copy)]
pub struct CollapsedTab {
    pub role: ColumnRole,
    /// Tab label derived from column role.
    pub name: &'static str,
}

/// Complete solver output describing the resolved layout.
#[derive(Debug, Clone)]
pub struct SolvedLayout {
    /// Columns to render as visible (ordered left → right).
    pub columns: Vec<SolvedColumn>,
    /// Columns that were collapsed into tabs.
    pub collapsed_tabs: Vec<CollapsedTab>,
    /// Overall layout orientation.
    pub orientation: Orientation,
    /// Direction for arranging main area control groups.
    pub group_direction: Direction,
    /// Slider height to use (180px normal, 120px compact).
    pub slider_height: f32,
    /// Whether to show visualizations (transfer curves, graphs).
    pub show_visualizations: bool,
    /// Knob size tier for standard controls (Xs/Sm/Md based on main width).
    pub knob_size: KnobSize,
}

impl SolvedLayout {
    /// Returns the allocated width for a given column role, or None if collapsed.
    pub fn column_width(&self, role: ColumnRole) -> Option<f32> {
        self.columns
            .iter()
            .find(|c| c.role == role)
            .map(|c| c.width)
    }

    /// Returns true if a column with the given role is visible (not collapsed).
    pub fn is_visible(&self, role: ColumnRole) -> bool {
        self.columns.iter().any(|c| c.role == role)
    }

    /// Returns true if a column with the given role was collapsed into a tab.
    pub fn is_collapsed(&self, role: ColumnRole) -> bool {
        self.collapsed_tabs.iter().any(|t| t.role == role)
    }
}

// ============================================================================
// Solver
// ============================================================================

fn tab_name_for_role(role: ColumnRole) -> &'static str {
    match role {
        ColumnRole::Config => "Config",
        ColumnRole::Main => "Main",
        ColumnRole::Output => "Output",
        ColumnRole::Diagnostic => "Diagnostic",
    }
}

/// Solve the layout for the given constraints and available space.
///
/// Returns a `SolvedLayout` describing which columns are visible, which
/// became tabs, and what internal adaptations to apply.
pub fn solve_layout(constraints: &[ColumnConstraint], available_width: f32) -> SolvedLayout {
    // 1. Vertical mode: all collapsible columns become tabs
    if available_width < VERTICAL_THRESHOLD {
        return solve_vertical(constraints, available_width);
    }

    // 2. Find the Main column (never collapses)
    let main_constraint = constraints
        .iter()
        .find(|c| c.role == ColumnRole::Main)
        .copied();

    let main_min = main_constraint.map_or(300.0, |c| c.min_width);

    // 3. Collect collapsible columns, sorted by priority ascending (lowest collapses first)
    let mut collapsible: Vec<&ColumnConstraint> = constraints
        .iter()
        .filter(|c| c.collapsible)
        .collect();
    collapsible.sort_by(|a, b| a.priority.partial_cmp(&b.priority).unwrap_or(std::cmp::Ordering::Equal));

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
    // Config goes first (left)
    if let Some(pos) = visible.iter().position(|c| c.role == ColumnRole::Config) {
        columns.push(visible[pos]);
    }
    // Main always present
    columns.push(SolvedColumn {
        role: ColumnRole::Main,
        width: main_width,
    });
    // Diagnostic (if visible) between main and output
    if let Some(pos) = visible.iter().position(|c| c.role == ColumnRole::Diagnostic) {
        columns.push(visible[pos]);
    }
    // Output goes last (right)
    if let Some(pos) = visible.iter().position(|c| c.role == ColumnRole::Output) {
        columns.push(visible[pos]);
    }

    // 7. Internal adaptations — use main_width (not available_width) for decisions
    //    about main-column content, since sidebars consume part of available_width.
    let group_direction = if main_width < GROUP_STACK_THRESHOLD {
        Direction::Column
    } else {
        Direction::Row
    };

    let slider_height = if main_width < COMPACT_SLIDER_THRESHOLD {
        SLIDER_HEIGHT_COMPACT
    } else {
        SLIDER_HEIGHT_NORMAL
    };

    let show_visualizations = main_width >= HIDE_VIZ_THRESHOLD;

    let knob_size = if main_width < COMPACT_KNOB_THRESHOLD {
        KnobSize::Xs
    } else if main_width >= LARGE_KNOB_THRESHOLD {
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
fn solve_vertical(constraints: &[ColumnConstraint], available_width: f32) -> SolvedLayout {
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
        slider_height: SLIDER_HEIGHT_COMPACT,
        show_visualizations: false,
        knob_size: KnobSize::Xs,
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin_layout::ColumnConstraint;

    /// Standard 3-column constraints (Config | Main | Output) like Compressor.
    fn compressor_constraints() -> Vec<ColumnConstraint> {
        vec![
            ColumnConstraint::config(100.0, 0.5),
            ColumnConstraint::main(300.0),
            ColumnConstraint::output(120.0, 0.6),
        ]
    }

    /// 4-column constraints (Config | Main | Diagnostic | Output).
    fn four_column_constraints() -> Vec<ColumnConstraint> {
        vec![
            ColumnConstraint::config(100.0, 0.5),
            ColumnConstraint::main(300.0),
            ColumnConstraint::diagnostic(150.0, 0.3),
            ColumnConstraint::output(120.0, 0.6),
        ]
    }

    #[test]
    fn test_wide_all_columns_visible() {
        let constraints = compressor_constraints();
        let solved = solve_layout(&constraints, 1200.0);

        assert_eq!(solved.orientation, Orientation::Horizontal);
        assert!(solved.is_visible(ColumnRole::Config));
        assert!(solved.is_visible(ColumnRole::Main));
        assert!(solved.is_visible(ColumnRole::Output));
        assert!(solved.collapsed_tabs.is_empty());
        assert_eq!(solved.slider_height, SLIDER_HEIGHT_NORMAL);
        assert!(solved.show_visualizations);
    }

    #[test]
    fn test_medium_output_stays_config_stays() {
        let constraints = compressor_constraints();
        // 600px: enough for Config(100) + Main(300) + Output(120) = 520, fits
        let solved = solve_layout(&constraints, 600.0);

        assert!(solved.is_visible(ColumnRole::Config));
        assert!(solved.is_visible(ColumnRole::Main));
        assert!(solved.is_visible(ColumnRole::Output));
    }

    #[test]
    fn test_narrow_output_collapses_first_then_config() {
        let constraints = compressor_constraints();
        // 450px: Main(300) + Output(120) = 420, so Output fits but not with Config
        // Output has higher priority (0.6) than Config (0.5), so Config collapses first
        let solved = solve_layout(&constraints, 450.0);

        assert!(solved.is_visible(ColumnRole::Main));
        assert!(solved.is_visible(ColumnRole::Output));
        assert!(solved.is_collapsed(ColumnRole::Config));
        assert_eq!(solved.collapsed_tabs.len(), 1);
    }

    #[test]
    fn test_four_columns_diagnostic_collapses_first() {
        let constraints = four_column_constraints();
        // Diagnostic has lowest priority (0.3), should collapse first
        // Total min: Config(100) + Main(300) + Diag(150) + Output(120) = 670
        let solved = solve_layout(&constraints, 600.0);

        assert!(solved.is_visible(ColumnRole::Main));
        assert!(solved.is_visible(ColumnRole::Output));
        assert!(solved.is_collapsed(ColumnRole::Diagnostic));
        // Config might or might not fit depending on remaining space
    }

    #[test]
    fn test_very_narrow_vertical_mode() {
        let constraints = compressor_constraints();
        let solved = solve_layout(&constraints, 350.0);

        assert_eq!(solved.orientation, Orientation::Vertical);
        assert_eq!(solved.columns.len(), 1);
        assert_eq!(solved.columns[0].role, ColumnRole::Main);
        assert_eq!(solved.group_direction, Direction::Column);
        assert_eq!(solved.slider_height, SLIDER_HEIGHT_COMPACT);
        assert!(!solved.show_visualizations);
        // Both Config and Output should be collapsed
        assert!(solved.is_collapsed(ColumnRole::Config));
        assert!(solved.is_collapsed(ColumnRole::Output));
    }

    #[test]
    fn test_column_order_is_correct() {
        let constraints = four_column_constraints();
        let solved = solve_layout(&constraints, 1200.0);

        // Order should be: Config, Main, Diagnostic, Output
        let roles: Vec<ColumnRole> = solved.columns.iter().map(|c| c.role).collect();
        let config_pos = roles.iter().position(|r| *r == ColumnRole::Config);
        let main_pos = roles.iter().position(|r| *r == ColumnRole::Main);
        let diag_pos = roles.iter().position(|r| *r == ColumnRole::Diagnostic);
        let output_pos = roles.iter().position(|r| *r == ColumnRole::Output);

        assert!(config_pos < main_pos);
        assert!(main_pos < diag_pos);
        assert!(diag_pos < output_pos);
    }

    #[test]
    fn test_main_never_collapses() {
        let constraints = compressor_constraints();
        // Even at minimum width, Main should be visible
        let solved = solve_layout(&constraints, 100.0);

        assert!(solved.is_visible(ColumnRole::Main));
    }

    #[test]
    fn test_main_gets_remaining_space() {
        let constraints = compressor_constraints();
        // 1000px: Config(100) + Output(120) = 220 fixed, Main gets 780
        let solved = solve_layout(&constraints, 1000.0);

        let main_width = solved.column_width(ColumnRole::Main).unwrap();
        assert!(main_width > 300.0, "Main should get remaining space, got {main_width}");
    }

    #[test]
    fn test_compact_slider_height() {
        let constraints = compressor_constraints();
        // main_width = 650 - 120 - 100 = 430 < 700 → compact
        let solved = solve_layout(&constraints, 650.0);
        assert_eq!(solved.slider_height, SLIDER_HEIGHT_COMPACT);

        // main_width = 920 - 120 - 100 = 700 → normal
        let solved_wide = solve_layout(&constraints, 920.0);
        assert_eq!(solved_wide.slider_height, SLIDER_HEIGHT_NORMAL);
    }

    #[test]
    fn test_group_direction_stacks_when_narrow() {
        let constraints = compressor_constraints();
        // main_width = 480 - 120 (Config collapses) = 360 < 500 → Column
        let solved = solve_layout(&constraints, 480.0);
        assert_eq!(solved.group_direction, Direction::Column);

        // main_width = 720 - 120 - 100 = 500 → Row
        let solved_wide = solve_layout(&constraints, 720.0);
        assert_eq!(solved_wide.group_direction, Direction::Row);
    }

    #[test]
    fn test_no_constraints_only_main() {
        // Empty constraints (no columns defined) — solver should still work
        let solved = solve_layout(&[], 800.0);

        // Should have at least Main
        assert_eq!(solved.columns.len(), 1);
        assert_eq!(solved.columns[0].role, ColumnRole::Main);
    }

    #[test]
    fn test_viz_hidden_when_narrow() {
        let constraints = compressor_constraints();
        // main_width = 550 - 120 - 100 = 330 < 600 → hidden
        let solved = solve_layout(&constraints, 550.0);
        assert!(!solved.show_visualizations);

        // main_width = 820 - 120 - 100 = 600 → visible
        let solved_wide = solve_layout(&constraints, 820.0);
        assert!(solved_wide.show_visualizations);
    }

    #[test]
    fn test_group_direction_based_on_main_width_not_available() {
        // available_width=600 (>500 GROUP_STACK_THRESHOLD), but sidebars eat 220px:
        // Config(100) + Output(120) = 220, so main_width = 600 - 220 = 380.
        // Groups should NOT be Row since main column only has 380px.
        let constraints = compressor_constraints();
        let solved = solve_layout(&constraints, 600.0);
        let main_width = solved.column_width(ColumnRole::Main).unwrap();
        assert!(
            main_width < GROUP_STACK_THRESHOLD,
            "main_width={main_width} should be < {GROUP_STACK_THRESHOLD}"
        );
        assert_eq!(
            solved.group_direction,
            Direction::Column,
            "Groups should stack vertically when main area is only {main_width}px"
        );
    }

    #[test]
    fn test_group_direction_row_when_main_has_enough_space() {
        // When main column has >= GROUP_STACK_THRESHOLD, groups should be side-by-side.
        // No sidebars → all space goes to main.
        let constraints = vec![ColumnConstraint::main(300.0)];
        let solved = solve_layout(&constraints, 800.0);
        assert_eq!(solved.group_direction, Direction::Row);

        // With sidebars but plenty of total space → main still gets enough
        let constraints = compressor_constraints();
        let solved = solve_layout(&constraints, 1200.0);
        let main_width = solved.column_width(ColumnRole::Main).unwrap();
        assert!(main_width >= GROUP_STACK_THRESHOLD);
        assert_eq!(solved.group_direction, Direction::Row);
    }

    #[test]
    fn test_total_width_does_not_exceed_available() {
        let constraints = compressor_constraints();
        // 450px: tight enough that preferred_width could exceed remaining
        // Config(min=100, pref=100) + Main(min=300) + Output(min=120, pref=120)
        // remaining after Main = 150. Output(pref=120) fits, remaining=30.
        // Config(pref=100) > remaining(30), so Config collapses.
        for width in [450.0, 500.0, 600.0, 800.0, 1200.0] {
            let solved = solve_layout(&constraints, width);
            let total: f32 = solved.columns.iter().map(|c| c.width).sum();
            assert!(
                total <= width,
                "Total column width {total} exceeds available {width}"
            );
        }
    }

    #[test]
    fn test_column_width_clamped_to_remaining() {
        // Create a scenario where preferred_width > remaining after Main deduction
        // but min_width fits. The column should get clamped width, not preferred.
        let constraints = vec![
            ColumnConstraint {
                role: ColumnRole::Config,
                min_width: 50.0,
                preferred_width: 200.0,
                max_width: 200.0,
                priority: 0.5,
                collapsible: true,
            },
            ColumnConstraint::main(300.0),
            ColumnConstraint {
                role: ColumnRole::Output,
                min_width: 50.0,
                preferred_width: 200.0,
                max_width: 200.0,
                priority: 0.6,
                collapsible: true,
            },
        ];
        // 450px: remaining = 150 after Main(300).
        // Output (higher priority) allocated first: pref=200, but clamped to 150.
        // Config: remaining=0, min=50 > 0 → collapses.
        let solved = solve_layout(&constraints, 450.0);
        let total: f32 = solved.columns.iter().map(|c| c.width).sum();
        assert!(total <= 450.0, "Total {total} exceeds 450.0");

        let output_width = solved.column_width(ColumnRole::Output).unwrap();
        assert_eq!(output_width, 150.0, "Output should be clamped to remaining space");
        assert!(solved.is_collapsed(ColumnRole::Config));
    }
}
