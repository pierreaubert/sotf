use super::consts::GROUP_STACK_THRESHOLD;
use super::consts::SLIDER_HEIGHT_COMPACT;
use super::consts::SLIDER_HEIGHT_NORMAL;
use super::solve::{solve_layout, solve_layout_scaled};
use super::types::Direction;
use super::types::Orientation;
use crate::plugin_layout::{ColumnConstraint, ColumnRole};

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
    assert!(
        main_width > 300.0,
        "Main should get remaining space, got {main_width}"
    );
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
fn scaled_layout_preserves_logical_breakpoints_and_scales_geometry() {
    let constraints = compressor_constraints();
    let baseline = solve_layout_scaled(&constraints, 650.0, 1.0);
    let zoomed = solve_layout_scaled(&constraints, 975.0, 1.5);

    assert_eq!(zoomed.orientation, baseline.orientation);
    assert_eq!(zoomed.group_direction, baseline.group_direction);
    assert_eq!(zoomed.knob_size, baseline.knob_size);
    assert_eq!(zoomed.show_visualizations, baseline.show_visualizations);
    assert_eq!(zoomed.slider_height, baseline.slider_height * 1.5);
}

#[test]
fn invalid_scale_falls_back_to_baseline_geometry() {
    let constraints = compressor_constraints();
    let baseline = solve_layout(&constraints, 650.0);

    for scale in [0.0, -1.0, f32::NAN, f32::INFINITY] {
        assert_eq!(solve_layout_scaled(&constraints, 650.0, scale), baseline);
    }
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
    assert_eq!(
        output_width, 150.0,
        "Output should be clamped to remaining space"
    );
    assert!(solved.is_collapsed(ColumnRole::Config));
}
