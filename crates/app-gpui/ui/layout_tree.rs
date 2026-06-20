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

/// Owned result of solving the 3-panel content layout.
///
/// `gpui_builder::SolvedNode` borrows strings from the source tree, but the
/// source tree is built from local values inside `solve_app_layout`. This
/// wrapper copies the small amount of data the UI actually needs so it can be
/// returned to callers without lifetime issues.
#[derive(Debug, Clone)]
pub struct AppSolvedLayout {
    pub is_horizontal: bool,
    pub library: SolvedSlot,
    pub queue: SolvedSlot,
    pub rack: SolvedSlot,
}

/// Resolved state for a single layout slot.
#[derive(Debug, Clone, Default)]
pub struct SolvedSlot {
    pub visible: bool,
    pub width: f32,
    pub height: f32,
    pub active_tier: Option<String>,
}

impl AppSolvedLayout {
    /// Look up a slot by its id.
    pub fn find(&self, id: &str) -> Option<&SolvedSlot> {
        match id {
            "library" => Some(&self.library),
            "queue" => Some(&self.queue),
            "rack" => Some(&self.rack),
            _ => None,
        }
    }
}

impl From<SolvedNode<'_>> for AppSolvedLayout {
    fn from(solved: SolvedNode<'_>) -> Self {
        Self {
            is_horizontal: solved.resolved_axis == Some(Axis::Horizontal),
            library: solved.find("library").map(SolvedSlot::from).unwrap_or_default(),
            queue: solved.find("queue").map(SolvedSlot::from).unwrap_or_default(),
            rack: solved.find("rack").map(SolvedSlot::from).unwrap_or_default(),
        }
    }
}

impl From<&SolvedNode<'_>> for SolvedSlot {
    fn from(node: &SolvedNode<'_>) -> Self {
        Self {
            visible: node.visible,
            width: node.width,
            height: node.height,
            active_tier: node.active_tier.map(String::from),
        }
    }
}

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
) -> AppSolvedLayout {
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
    let prefs = LayoutPreferences::new(&ratios, &collapsed);

    AppSolvedLayout::from(gpui_builder::solve(
        &root,
        content_width,
        content_height,
        &prefs,
    ))
}

/// Whether the solver chose horizontal axis (panels side-by-side).
pub fn solved_is_horizontal(solved: &AppSolvedLayout) -> bool {
    solved.is_horizontal
}

/// Derive `RackDisplayMode` from the solver output.
pub fn solved_rack_display_mode(solved: &AppSolvedLayout) -> crate::app::RackDisplayMode {
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
pub fn solved_hide_queue_meters(solved: &AppSolvedLayout) -> bool {
    matches!(
        solved_rack_display_mode(solved),
        crate::app::RackDisplayMode::Full | crate::app::RackDisplayMode::Mini
    )
}
