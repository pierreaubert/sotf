//! Layout tree for the two-panel AutoEQ form (parameters + documentation).
//!
//! Uses gpui-builder's constraint solver to produce concrete pixel sizes
//! for the form panel (left) and documentation panel (right).

use gpui_builder::{
    Axis, ContainerNode, LayoutNode, LayoutPreferences, Sizing, SlotNode, SolvedNode,
};

/// Divider thickness between form and docs panels.
const DIVIDER_SIZE: f32 = 1.0;

/// Solve the two-panel layout for the given available dimensions.
///
/// Returns a `SolvedNode` tree with two children: `"form"` and `"docs"`.
/// The docs panel collapses when there isn't enough horizontal space.
pub fn solve_autoeq_layout(width: f32, height: f32) -> SolvedNode {
    let children: [LayoutNode<'_>; 2] = [
        LayoutNode::Slot(SlotNode {
            id: "form",
            sizing: Sizing::fractional(0.55, 320.0),
            priority: 1.0,
            collapsible: false,
            display_tiers: &[],
            collapse_label: None,
        }),
        LayoutNode::Slot(SlotNode {
            id: "docs",
            sizing: Sizing::fractional(0.45, 200.0),
            priority: 0.3,
            collapsible: true,
            display_tiers: &[],
            collapse_label: Some("Help"),
        }),
    ];

    let root = LayoutNode::Container(ContainerNode {
        id: "autoeq-root",
        axis: Axis::Horizontal,
        auto_axis: None,
        sizing: Sizing::flex(0.0),
        children: &children,
        divider_size: DIVIDER_SIZE,
    });

    gpui_builder::solve(&root, width, height, &LayoutPreferences::default())
}
