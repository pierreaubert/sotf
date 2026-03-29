// Layout tree declaration for the 3-panel content area.
//
// Defines the constraint-based layout using gpui-builder and provides
// a solve function that returns concrete pixel sizes for Library | Queue | Rack.

use crate::app::state::ui::LayoutState;
use gpui_builder::{
    Axis, ContainerNode, DisplayTier, LayoutNode, LayoutPreferences, Sizing, SlotNode, SolvedNode,
};

/// Divider thickness in pixels — must match the PaneDivider component size.
const DIVIDER_SIZE: f32 = 6.0;

static RACK_TIERS: &[DisplayTier<'_>] = &[
    DisplayTier {
        name: "Full",
        min_size: 200.0,
    },
    DisplayTier {
        name: "Mini",
        min_size: 100.0,
    },
];

/// Solve the 3-panel content layout (Library | Queue | Rack) for the given
/// content area dimensions and user layout state.
///
/// The solver handles:
/// - Axis switching (horizontal when width > height, vertical otherwise)
/// - Fractional panel sizing with user-dragged ratio overrides
/// - Priority-based panel collapse when space is tight
/// - Rack display tiers (Full at >=200px, Mini at >=100px)
pub fn solve_app_layout(
    content_width: f32,
    content_height: f32,
    layout: &LayoutState,
) -> SolvedNode {
    let content_children: [LayoutNode<'_>; 3] = [
        LayoutNode::Slot(SlotNode {
            id: "library",
            sizing: Sizing::fractional(0.30, 100.0),
            priority: 0.5,
            collapsible: true,
            display_tiers: &[],
            collapse_label: Some("Library"),
        }),
        LayoutNode::Slot(SlotNode {
            id: "queue",
            sizing: Sizing::flex(200.0),
            priority: 1.0,
            collapsible: false,
            display_tiers: &[],
            collapse_label: None,
        }),
        LayoutNode::Slot(SlotNode {
            id: "rack",
            sizing: Sizing::fractional(0.30, 0.0),
            priority: 0.3,
            collapsible: true,
            display_tiers: RACK_TIERS,
            collapse_label: Some("Rack"),
        }),
    ];

    let root = LayoutNode::Container(ContainerNode {
        id: "content",
        axis: Axis::Horizontal,
        auto_axis: Some(1.0), // switch to vertical when height >= width
        sizing: Sizing::flex(0.0),
        children: &content_children,
        divider_size: DIVIDER_SIZE,
    });

    let ratios = [
        ("library", Axis::Horizontal, layout.library_h_ratio),
        ("library", Axis::Vertical, layout.library_v_ratio),
        ("rack", Axis::Horizontal, layout.rack_h_ratio),
        ("rack", Axis::Vertical, layout.rack_v_ratio),
    ];
    let collapsed = [
        ("library", layout.library_panel_collapsed),
        ("rack", layout.rack_panel_collapsed),
    ];
    let prefs = LayoutPreferences {
        ratios: &ratios,
        collapsed: &collapsed,
    };

    gpui_builder::solve(&root, content_width, content_height, &prefs)
}

/// Whether the solver chose horizontal axis (panels side-by-side).
pub fn solved_is_horizontal(solved: &SolvedNode) -> bool {
    solved.resolved_axis == Some(Axis::Horizontal)
}

/// Derive `RackDisplayMode` from the solver output.
pub fn solved_rack_display_mode(solved: &SolvedNode) -> crate::app::RackDisplayMode {
    match solved.find("rack") {
        Some(rack) if rack.visible => match rack.active_tier.as_deref() {
            Some("Full") => crate::app::RackDisplayMode::Full,
            Some("Mini") => crate::app::RackDisplayMode::Mini,
            _ => crate::app::RackDisplayMode::Collapsed,
        },
        _ => crate::app::RackDisplayMode::Collapsed,
    }
}

/// Whether queue meters should be hidden (rack is showing its own meters).
pub fn solved_hide_queue_meters(solved: &SolvedNode) -> bool {
    matches!(
        solved_rack_display_mode(solved),
        crate::app::RackDisplayMode::Full | crate::app::RackDisplayMode::Mini
    )
}
