//! Example: Music player app layout (mirrors SotF app-gpui).
//!
//! Demonstrates:
//! - Hard constraints (header/footer)
//! - Nested container with auto-axis switching
//! - Collapsible panels with display tiers
//! - User preferences (dragged ratios, collapsed state)
//!
//! Run: cargo run -p gpui-builder --example app_layout

use gpui_builder::{
    Axis, ContainerNode, DisplayTier, LayoutNode, LayoutPreferences, Sizing, SlotNode, solve,
};

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

fn app_tree() -> (
    [LayoutNode<'static>; 3], // content children
    [LayoutNode<'static>; 3], // root children need content ref
) {
    let content_children = [
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
    // We can't return a reference to a local, so we return the parts
    // and assemble outside.
    let root_children = [
        LayoutNode::Slot(SlotNode {
            id: "header",
            sizing: Sizing::Fixed(40.0),
            priority: 1.0,
            collapsible: false,
            display_tiers: &[],
            collapse_label: None,
        }),
        // placeholder — will be replaced
        LayoutNode::Slot(SlotNode {
            id: "_placeholder",
            sizing: Sizing::flex(0.0),
            priority: 1.0,
            collapsible: false,
            display_tiers: &[],
            collapse_label: None,
        }),
        LayoutNode::Slot(SlotNode {
            id: "footer",
            sizing: Sizing::Fixed(100.0),
            priority: 1.0,
            collapsible: false,
            display_tiers: &[],
            collapse_label: None,
        }),
    ];
    (content_children, root_children)
}

fn print_solved(solved: &gpui_builder::SolvedNode, indent: usize) {
    let pad = " ".repeat(indent);
    let status = if solved.visible {
        "visible"
    } else {
        "collapsed"
    };
    let tier = solved.active_tier.as_deref().unwrap_or("-");
    let axis = match solved.resolved_axis {
        Some(Axis::Horizontal) => " [H]",
        Some(Axis::Vertical) => " [V]",
        None => "",
    };
    println!(
        "{pad}{id}{axis}  {w:.0}x{h:.0}  {status}  tier={tier}",
        id = solved.id,
        w = solved.width,
        h = solved.height,
    );
    for child in &solved.children {
        print_solved(child, indent + 2);
    }
}

fn main() {
    let (content_children, mut root_children) = app_tree();

    // Build the content container referencing content_children
    let content = LayoutNode::Container(ContainerNode {
        id: "content",
        axis: Axis::Horizontal,
        auto_axis: Some(1.0), // switch to vertical when height > width
        sizing: Sizing::flex(0.0),
        children: &content_children,
        divider_size: 6.0,
    });
    root_children[1] = content;

    let root = LayoutNode::Container(ContainerNode {
        id: "root",
        axis: Axis::Vertical,
        auto_axis: None,
        sizing: Sizing::flex(0.0),
        children: &root_children,
        divider_size: 0.0,
    });

    // --- Scenario 1: Wide desktop window ---
    println!("=== Wide desktop (1200x800) ===");
    let solved = solve(&root, 1200.0, 800.0, &LayoutPreferences::default());
    print_solved(&solved, 0);

    let tabs = solved.collapsed_tabs();
    if !tabs.is_empty() {
        println!("  Collapsed tabs: {:?}", tabs);
    }
    println!();

    // --- Scenario 2: Narrow window (library collapses) ---
    println!("=== Narrow window (500x800) ===");
    let solved = solve(&root, 500.0, 800.0, &LayoutPreferences::default());
    print_solved(&solved, 0);

    let tabs = solved.collapsed_tabs();
    if !tabs.is_empty() {
        println!("  Collapsed tabs: {:?}", tabs);
    }
    println!();

    // --- Scenario 3: Tall window (auto-axis switches to vertical) ---
    println!("=== Tall window (600x1000) ===");
    let solved = solve(&root, 600.0, 1000.0, &LayoutPreferences::default());
    print_solved(&solved, 0);
    println!();

    // --- Scenario 4: User dragged library wider, collapsed rack ---
    println!("=== User prefs: library=45%, rack collapsed (1200x800) ===");
    let prefs = LayoutPreferences {
        ratios: &[("library", Axis::Horizontal, 0.45)],
        collapsed: &[("rack", true)],
    };
    let solved = solve(&root, 1200.0, 800.0, &prefs);
    print_solved(&solved, 0);

    let tabs = solved.collapsed_tabs();
    if !tabs.is_empty() {
        println!("  Collapsed tabs: {:?}", tabs);
    }
}
