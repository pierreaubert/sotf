//! Layout Constraint Solver
//!
//! Pure function that resolves a `LayoutNode` tree into a `SolvedNode` tree
//! with concrete pixel sizes. No framework dependencies, no side effects.
//!
//! Algorithm (per container, recursive):
//! 1. Resolve axis (check `auto_axis` against width/height ratio)
//! 2. Apply user collapse preferences
//! 3. Allocate main-axis space:
//!    a. Sum Fixed children + divider space
//!    b. Reserve minimums for Fractional/Flex children
//!    c. If minimums exceed remaining → priority-based collapse (lowest first)
//!    d. Distribute remaining space
//! 4. Determine display tiers for each slot
//! 5. Recurse into container children

use gpui_pretext::{
    EngineProfile, PrepareOptions, layout, layout_with_lines, prepare, prepare_with_segments,
};

use crate::solved::SolvedNode;
use crate::types::{Axis, ContainerNode, LayoutNode, LayoutPreferences, Sizing, SlotNode};

/// Solve the layout tree into concrete pixel sizes.
///
/// `root` is the declaration tree. `width` and `height` are the available
/// space (typically the window size). `prefs` provides user overrides for
/// ratios and collapsed states.
pub fn solve(
    root: &LayoutNode<'_>,
    width: f32,
    height: f32,
    prefs: &LayoutPreferences<'_>,
) -> SolvedNode {
    solve_node(root, width, height, prefs)
}

fn solve_node(
    node: &LayoutNode<'_>,
    width: f32,
    height: f32,
    prefs: &LayoutPreferences<'_>,
) -> SolvedNode {
    match node {
        LayoutNode::Slot(slot) => solve_slot(slot, width, height, prefs),
        LayoutNode::Container(container) => solve_container(container, width, height, prefs),
    }
}

fn solve_slot(
    slot: &SlotNode<'_>,
    width: f32,
    height: f32,
    prefs: &LayoutPreferences<'_>,
) -> SolvedNode {
    let user_collapsed = slot.collapsible && prefs.is_collapsed(slot.id);

    if user_collapsed {
        return SolvedNode {
            id: slot.id.to_string(),
            width: 0.0,
            height: 0.0,
            visible: false,
            active_tier: None,
            collapse_label: slot.collapse_label.map(String::from),
            resolved_axis: None,
            children: Vec::new(),
        };
    }

    // Determine active display tier based on the smaller of width/height
    // (we use the size that will be constrained by the parent's main axis,
    // but at this point we don't know the parent axis, so we use the
    // minimum dimension as a conservative estimate — the solver will
    // re-assign the tier after allocation).
    let active_tier = resolve_display_tier(slot, width.min(height));

    SolvedNode {
        id: slot.id.to_string(),
        width,
        height,
        visible: true,
        active_tier,
        collapse_label: slot.collapse_label.map(String::from),
        resolved_axis: None,
        children: Vec::new(),
    }
}

fn solve_container(
    container: &ContainerNode<'_>,
    width: f32,
    height: f32,
    prefs: &LayoutPreferences<'_>,
) -> SolvedNode {
    // Step 1: Resolve axis
    let axis = resolve_axis(container, width, height);

    let main_size = match axis {
        Axis::Horizontal => width,
        Axis::Vertical => height,
    };
    let cross_size = match axis {
        Axis::Horizontal => height,
        Axis::Vertical => width,
    };

    // Step 2: Classify children, apply user collapse, pre-compute Text sizes
    let profile = EngineProfile::default();
    let options = PrepareOptions::default();
    let mut child_infos: Vec<ChildInfo<'_>> = container
        .children
        .iter()
        .map(|child| {
            let user_collapsed = child.collapsible() && prefs.is_collapsed(child.id());
            let computed_text_size = if !user_collapsed {
                if let Sizing::Text {
                    text,
                    measure,
                    line_height,
                    min,
                } = child.sizing()
                {
                    Some(compute_text_size(
                        text,
                        measure,
                        line_height,
                        min,
                        axis,
                        cross_size,
                        &profile,
                        &options,
                    ))
                } else {
                    None
                }
            } else {
                None
            };
            ChildInfo {
                node: child,
                user_collapsed,
                solver_collapsed: false,
                allocated_size: 0.0,
                computed_text_size,
            }
        })
        .collect();

    // Step 3: Allocate main-axis space
    allocate_main_axis(
        &mut child_infos,
        main_size,
        container.divider_size,
        axis,
        prefs,
    );

    // Step 4+5: Build solved children (determine tiers, recurse into containers)
    let children: Vec<SolvedNode> = child_infos
        .iter()
        .map(|info| {
            let visible = !info.user_collapsed && !info.solver_collapsed;
            if !visible {
                // Collapsed child
                let collapse_label = match info.node {
                    LayoutNode::Slot(s) => s.collapse_label.map(String::from),
                    LayoutNode::Container(_) => None,
                };
                return SolvedNode {
                    id: info.node.id().to_string(),
                    width: 0.0,
                    height: 0.0,
                    visible: false,
                    active_tier: None,
                    collapse_label,
                    resolved_axis: None,
                    children: Vec::new(),
                };
            }

            let (child_w, child_h) = match axis {
                Axis::Horizontal => (info.allocated_size, cross_size),
                Axis::Vertical => (cross_size, info.allocated_size),
            };

            match info.node {
                LayoutNode::Slot(slot) => {
                    let active_tier = resolve_display_tier(slot, info.allocated_size);
                    SolvedNode {
                        id: slot.id.to_string(),
                        width: child_w,
                        height: child_h,
                        visible: true,
                        active_tier,
                        collapse_label: slot.collapse_label.map(String::from),
                        resolved_axis: None,
                        children: Vec::new(),
                    }
                }
                LayoutNode::Container(_) => solve_node(info.node, child_w, child_h, prefs),
            }
        })
        .collect();

    SolvedNode {
        id: container.id.to_string(),
        width,
        height,
        visible: true,
        active_tier: None,
        collapse_label: None,
        resolved_axis: Some(axis),
        children,
    }
}

// ============================================================================
// Axis Resolution
// ============================================================================

fn resolve_axis(container: &ContainerNode<'_>, width: f32, height: f32) -> Axis {
    match container.auto_axis {
        Some(threshold) => {
            if height > 0.0 && (width / height) > threshold {
                Axis::Horizontal
            } else {
                Axis::Vertical
            }
        }
        None => container.axis,
    }
}

// ============================================================================
// Main-Axis Space Allocation
// ============================================================================

struct ChildInfo<'a> {
    node: &'a LayoutNode<'a>,
    user_collapsed: bool,
    solver_collapsed: bool,
    allocated_size: f32,
    /// Pre-computed size for `Sizing::Text` nodes (None for other sizing types).
    computed_text_size: Option<f32>,
}

/// Compute the size for a `Sizing::Text` node using gpui-pretext.
///
/// - In a **vertical** container (main axis = height): returns text height
///   with `cross_size` (the container's width) as `max_width`.
/// - In a **horizontal** container (main axis = width): returns the maximum
///   line width with no wrapping constraint.
fn compute_text_size(
    text: &str,
    measure: &dyn gpui_pretext::TextMeasure,
    line_height: f32,
    min: f32,
    axis: Axis,
    cross_size: f32,
    profile: &EngineProfile,
    options: &PrepareOptions,
) -> f32 {
    let size = match axis {
        Axis::Vertical => {
            let prepared = prepare(text, measure, profile, options);
            layout(&prepared, cross_size as f64, line_height as f64, profile).height as f32
        }
        Axis::Horizontal => {
            let prepared = prepare_with_segments(text, measure, profile, options);
            let result = layout_with_lines(&prepared, f64::MAX, line_height as f64, profile);
            result.lines.iter().map(|l| l.width).fold(0.0_f64, f64::max) as f32
        }
    };
    size.max(min)
}

fn allocate_main_axis(
    children: &mut [ChildInfo<'_>],
    available: f32,
    divider_size: f32,
    axis: Axis,
    prefs: &LayoutPreferences<'_>,
) {
    // Pass A: Allocate non-collapsible Fixed and Text children unconditionally.
    // Collapsible Fixed/Text children participate in collapse logic below.
    let mut unconditional_fixed = 0.0_f32;
    for child in children.iter_mut() {
        if child.user_collapsed {
            continue;
        }
        if child.node.collapsible() {
            continue;
        }
        match child.node.sizing() {
            Sizing::Fixed(size) => {
                child.allocated_size = size;
                unconditional_fixed += size;
            }
            Sizing::Text { min, .. } => {
                let size = child.computed_text_size.unwrap_or(min);
                child.allocated_size = size;
                unconditional_fixed += size;
            }
            _ => {}
        }
    }

    // Count initially visible children for divider space
    let initial_visible = children.iter().filter(|c| !c.user_collapsed).count();
    let initial_divider_space = if initial_visible > 1 {
        divider_size * (initial_visible - 1) as f32
    } else {
        0.0
    };

    let space_after_fixed = (available - unconditional_fixed - initial_divider_space).max(0.0);

    // Pass B: Sum minimums of all non-unconditional-fixed visible children
    // (collapsible Fixed/Text + Fractional + Flex)
    let total_minimums: f32 = children
        .iter()
        .filter(|c| {
            !c.user_collapsed
                && (c.node.collapsible()
                    || !matches!(c.node.sizing(), Sizing::Fixed(_) | Sizing::Text { .. }))
        })
        .map(|c| c.node.sizing().min_size())
        .sum();

    // Pass C: Priority-based collapse if minimums exceed remaining
    if total_minimums > space_after_fixed {
        let mut collapsible_indices: Vec<usize> = children
            .iter()
            .enumerate()
            .filter(|(_, c)| !c.user_collapsed && c.node.collapsible())
            .map(|(i, _)| i)
            .collect();

        // Sort by priority ascending (lowest priority collapses first)
        collapsible_indices.sort_by(|&a, &b| {
            children[a]
                .node
                .priority()
                .partial_cmp(&children[b].node.priority())
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let mut current_minimums = total_minimums;
        for idx in collapsible_indices {
            if current_minimums <= space_after_fixed {
                break;
            }
            children[idx].solver_collapsed = true;
            current_minimums -= children[idx].node.sizing().min_size();
        }
    }

    // Recompute available after collapse (divider count may have changed)
    let visible_after = children
        .iter()
        .filter(|c| !c.user_collapsed && !c.solver_collapsed)
        .count();
    let divider_space_after = if visible_after > 1 {
        divider_size * (visible_after - 1) as f32
    } else {
        0.0
    };
    let remaining = (available - unconditional_fixed - divider_space_after).max(0.0);

    // Pass D: Distribute remaining among visible collapsible-Fixed + Fractional + Flex
    distribute_remaining(children, remaining, axis, prefs);
}

fn distribute_remaining(
    children: &mut [ChildInfo<'_>],
    remaining: f32,
    axis: Axis,
    prefs: &LayoutPreferences<'_>,
) {
    // Collapsible Fixed/Text nodes that survived collapse get their fixed/measured size
    let mut used_by_fixed = 0.0_f32;
    for child in children.iter_mut() {
        if child.user_collapsed || child.solver_collapsed {
            continue;
        }
        if !child.node.collapsible() {
            continue;
        }
        match child.node.sizing() {
            Sizing::Fixed(size) => {
                child.allocated_size = size;
                used_by_fixed += size;
            }
            Sizing::Text { min, .. } => {
                let size = child.computed_text_size.unwrap_or(min);
                child.allocated_size = size;
                used_by_fixed += size;
            }
            _ => {}
        }
    }

    let distributable = (remaining - used_by_fixed).max(0.0);

    // Collect fractional and flex demands
    let mut fractional_demand = 0.0_f32;
    let mut flex_total_weight = 0.0_f32;

    for child in children.iter() {
        if child.user_collapsed || child.solver_collapsed {
            continue;
        }
        match child.node.sizing() {
            Sizing::Fractional { initial, .. } => {
                let ratio = prefs.ratio_for(child.node.id(), axis).unwrap_or(initial);
                fractional_demand += ratio;
            }
            Sizing::Flex { weight, .. } => {
                flex_total_weight += weight;
            }
            Sizing::Fixed(_) | Sizing::Text { .. } => {}
        }
    }

    // If total fractional ratios > 1.0, scale them down proportionally
    let ratio_scale = if fractional_demand > 1.0 {
        1.0 / fractional_demand
    } else {
        1.0
    };

    // Allocate fractional children their share
    let mut used_by_fractional = 0.0_f32;
    for child in children.iter_mut() {
        if child.user_collapsed || child.solver_collapsed {
            continue;
        }
        if let Sizing::Fractional { initial, min, max } = child.node.sizing() {
            let ratio = prefs.ratio_for(child.node.id(), axis).unwrap_or(initial);
            let target = (ratio * ratio_scale * distributable).clamp(min, max);
            child.allocated_size = target;
            used_by_fractional += target;
        }
    }

    // Flex children split leftover (clamped to available, not unbounded)
    let flex_remaining = (distributable - used_by_fractional).max(0.0);
    if flex_total_weight > 0.0 {
        // First pass: compute proportional shares with min floor.
        let mut flex_shares: Vec<(usize, f32)> = Vec::new();
        let mut total_flex = 0.0_f32;
        for (i, child) in children.iter().enumerate() {
            if child.user_collapsed || child.solver_collapsed {
                continue;
            }
            if let Sizing::Flex { min, weight } = child.node.sizing() {
                let proportional = flex_remaining * (weight / flex_total_weight);
                let share = proportional.max(min).min(flex_remaining);
                flex_shares.push((i, share));
                total_flex += share;
            }
        }

        // Second pass: if total exceeds available space, scale proportionally.
        let scale = if total_flex > flex_remaining && total_flex > 0.0 {
            flex_remaining / total_flex
        } else {
            1.0
        };

        for (i, share) in flex_shares {
            children[i].allocated_size = share * scale;
        }
    }
}

// ============================================================================
// Display Tier Resolution
// ============================================================================

fn resolve_display_tier(slot: &SlotNode<'_>, main_size: f32) -> Option<String> {
    if slot.display_tiers.is_empty() {
        return None;
    }

    // Find the tier with the largest min_size that still fits
    let mut best: Option<&str> = None;
    let mut best_threshold = f32::NEG_INFINITY;

    for tier in slot.display_tiers {
        if main_size >= tier.min_size && tier.min_size > best_threshold {
            best = Some(tier.name);
            best_threshold = tier.min_size;
        }
    }

    best.map(String::from)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{ContainerNode, DisplayTier, SlotNode};

    fn simple_slot<'a>(id: &'a str, sizing: Sizing<'a>) -> LayoutNode<'a> {
        LayoutNode::Slot(SlotNode {
            id,
            sizing,
            priority: 1.0,
            collapsible: false,
            display_tiers: &[],
            collapse_label: None,
        })
    }

    fn collapsible_slot<'a>(
        id: &'a str,
        sizing: Sizing<'a>,
        priority: f32,
        label: &'a str,
    ) -> LayoutNode<'a> {
        LayoutNode::Slot(SlotNode {
            id,
            sizing,
            priority,
            collapsible: true,
            display_tiers: &[],
            collapse_label: Some(label),
        })
    }

    // ===== Basic layout tests =====

    #[test]
    fn single_flex_child_gets_all_space() {
        let children = [simple_slot("main", Sizing::flex(100.0))];
        let root = LayoutNode::Container(ContainerNode {
            id: "root",
            axis: Axis::Horizontal,
            auto_axis: None,
            sizing: Sizing::flex(0.0),
            children: &children,
            divider_size: 0.0,
        });

        let solved = solve(&root, 800.0, 600.0, &LayoutPreferences::default());
        let main = solved.find("main").unwrap();
        assert_eq!(main.width, 800.0);
        assert_eq!(main.height, 600.0);
        assert!(main.visible);
    }

    #[test]
    fn fixed_plus_flex() {
        let children = [
            simple_slot("header", Sizing::Fixed(50.0)),
            simple_slot("content", Sizing::flex(100.0)),
            simple_slot("footer", Sizing::Fixed(80.0)),
        ];
        let root = LayoutNode::Container(ContainerNode {
            id: "root",
            axis: Axis::Vertical,
            auto_axis: None,
            sizing: Sizing::flex(0.0),
            children: &children,
            divider_size: 0.0,
        });

        let solved = solve(&root, 1200.0, 800.0, &LayoutPreferences::default());

        let header = solved.find("header").unwrap();
        assert_eq!(header.height, 50.0);
        assert_eq!(header.width, 1200.0); // cross-axis = full

        let content = solved.find("content").unwrap();
        assert_eq!(content.height, 670.0); // 800 - 50 - 80
        assert_eq!(content.width, 1200.0);

        let footer = solved.find("footer").unwrap();
        assert_eq!(footer.height, 80.0);
    }

    #[test]
    fn fractional_children_with_flex_center() {
        let children = [
            simple_slot("left", Sizing::fractional(0.3, 100.0)),
            simple_slot("center", Sizing::flex(200.0)),
            simple_slot("right", Sizing::fractional(0.2, 80.0)),
        ];
        let root = LayoutNode::Container(ContainerNode {
            id: "root",
            axis: Axis::Horizontal,
            auto_axis: None,
            sizing: Sizing::flex(0.0),
            children: &children,
            divider_size: 0.0,
        });

        let solved = solve(&root, 1000.0, 600.0, &LayoutPreferences::default());

        let left = solved.find("left").unwrap();
        assert_eq!(left.width, 300.0); // 0.3 * 1000

        let right = solved.find("right").unwrap();
        assert_eq!(right.width, 200.0); // 0.2 * 1000

        let center = solved.find("center").unwrap();
        assert_eq!(center.width, 500.0); // 1000 - 300 - 200
    }

    #[test]
    fn divider_space_reserved() {
        let children = [
            simple_slot("a", Sizing::flex(100.0)),
            simple_slot("b", Sizing::flex(100.0)),
        ];
        let root = LayoutNode::Container(ContainerNode {
            id: "root",
            axis: Axis::Horizontal,
            auto_axis: None,
            sizing: Sizing::flex(0.0),
            children: &children,
            divider_size: 6.0,
        });

        let solved = solve(&root, 1000.0, 600.0, &LayoutPreferences::default());
        let a = solved.find("a").unwrap();
        let b = solved.find("b").unwrap();

        // Total = a + b + divider = 1000
        let total = a.width + b.width + 6.0;
        assert!(
            (total - 1000.0).abs() < 0.01,
            "total={total}, expected 1000.0"
        );
    }

    // ===== Collapse tests =====

    #[test]
    fn user_collapsed_slot_gets_zero_size() {
        let children = [
            collapsible_slot("sidebar", Sizing::fractional(0.3, 100.0), 0.5, "Sidebar"),
            simple_slot("main", Sizing::flex(200.0)),
        ];
        let root = LayoutNode::Container(ContainerNode {
            id: "root",
            axis: Axis::Horizontal,
            auto_axis: None,
            sizing: Sizing::flex(0.0),
            children: &children,
            divider_size: 0.0,
        });

        let prefs = LayoutPreferences {
            ratios: &[],
            collapsed: &[("sidebar", true)],
        };

        let solved = solve(&root, 1000.0, 600.0, &prefs);
        let sidebar = solved.find("sidebar").unwrap();
        assert!(!sidebar.visible);
        assert_eq!(sidebar.width, 0.0);

        let main = solved.find("main").unwrap();
        assert_eq!(main.width, 1000.0);
    }

    #[test]
    fn priority_collapse_when_space_tight() {
        let children = [
            collapsible_slot("config", Sizing::fractional(0.2, 100.0), 0.5, "Config"),
            simple_slot("main", Sizing::flex(300.0)),
            collapsible_slot("output", Sizing::fractional(0.2, 120.0), 0.6, "Output"),
        ];
        let root = LayoutNode::Container(ContainerNode {
            id: "root",
            axis: Axis::Horizontal,
            auto_axis: None,
            sizing: Sizing::flex(0.0),
            children: &children,
            divider_size: 0.0,
        });

        // 450px: Main needs 300 min, Output needs 120 min = 420, Config needs 100 min = 520
        // Config (priority 0.5) collapses first since 300 + 120 + 100 > 450
        let solved = solve(&root, 450.0, 600.0, &LayoutPreferences::default());

        let config = solved.find("config").unwrap();
        assert!(!config.visible, "Config should collapse (lowest priority)");
        assert_eq!(config.collapse_label.as_deref(), Some("Config"));

        let output = solved.find("output").unwrap();
        assert!(output.visible);

        let main = solved.find("main").unwrap();
        assert!(main.visible);
        assert!(main.width >= 300.0);
    }

    #[test]
    fn all_collapsible_collapse_when_very_tight() {
        let children = [
            collapsible_slot("config", Sizing::fractional(0.2, 100.0), 0.5, "Config"),
            simple_slot("main", Sizing::flex(300.0)),
            collapsible_slot("output", Sizing::fractional(0.2, 120.0), 0.6, "Output"),
        ];
        let root = LayoutNode::Container(ContainerNode {
            id: "root",
            axis: Axis::Horizontal,
            auto_axis: None,
            sizing: Sizing::flex(0.0),
            children: &children,
            divider_size: 0.0,
        });

        // 250px: only Main(300 min) can fit — everything else collapses
        let solved = solve(&root, 250.0, 600.0, &LayoutPreferences::default());
        assert!(!solved.find("config").unwrap().visible);
        assert!(!solved.find("output").unwrap().visible);
        assert!(solved.find("main").unwrap().visible);
    }

    #[test]
    fn collapsed_tabs_collected() {
        let children = [
            collapsible_slot("config", Sizing::fractional(0.2, 100.0), 0.5, "Config"),
            simple_slot("main", Sizing::flex(300.0)),
            collapsible_slot("output", Sizing::fractional(0.2, 120.0), 0.6, "Output"),
        ];
        let root = LayoutNode::Container(ContainerNode {
            id: "root",
            axis: Axis::Horizontal,
            auto_axis: None,
            sizing: Sizing::flex(0.0),
            children: &children,
            divider_size: 0.0,
        });

        let solved = solve(&root, 250.0, 600.0, &LayoutPreferences::default());
        let tabs = solved.collapsed_tabs();
        assert_eq!(tabs.len(), 2);
        assert!(
            tabs.iter()
                .any(|(id, label)| *id == "config" && *label == "Config")
        );
        assert!(
            tabs.iter()
                .any(|(id, label)| *id == "output" && *label == "Output")
        );
    }

    // ===== Auto-axis tests =====

    #[test]
    fn auto_axis_switches_based_on_aspect_ratio() {
        let children = [
            simple_slot("a", Sizing::flex(0.0)),
            simple_slot("b", Sizing::flex(0.0)),
        ];
        let root = LayoutNode::Container(ContainerNode {
            id: "root",
            axis: Axis::Horizontal, // default
            auto_axis: Some(1.0),   // switch at w/h ratio 1.0
            sizing: Sizing::flex(0.0),
            children: &children,
            divider_size: 0.0,
        });

        // Wide window → Horizontal
        let solved = solve(&root, 1200.0, 800.0, &LayoutPreferences::default());
        assert_eq!(solved.resolved_axis, Some(Axis::Horizontal));

        // Tall window → Vertical
        let solved = solve(&root, 600.0, 800.0, &LayoutPreferences::default());
        assert_eq!(solved.resolved_axis, Some(Axis::Vertical));

        // Square → Vertical (ratio = 1.0, not > threshold)
        let solved = solve(&root, 800.0, 800.0, &LayoutPreferences::default());
        assert_eq!(solved.resolved_axis, Some(Axis::Vertical));
    }

    // ===== Display tier tests =====

    #[test]
    fn display_tiers_resolve_correctly() {
        static TIERS: &[DisplayTier<'_>] = &[
            DisplayTier {
                name: "Full",
                min_size: 200.0,
            },
            DisplayTier {
                name: "Mini",
                min_size: 100.0,
            },
        ];

        let children = [LayoutNode::Slot(SlotNode {
            id: "rack",
            sizing: Sizing::flex(0.0),
            priority: 1.0,
            collapsible: false,
            display_tiers: TIERS,
            collapse_label: None,
        })];
        let root = LayoutNode::Container(ContainerNode {
            id: "root",
            axis: Axis::Horizontal,
            auto_axis: None,
            sizing: Sizing::flex(0.0),
            children: &children,
            divider_size: 0.0,
        });

        // Wide → Full tier
        let solved = solve(&root, 300.0, 600.0, &LayoutPreferences::default());
        assert_eq!(
            solved.find("rack").unwrap().active_tier.as_deref(),
            Some("Full")
        );

        // Medium → Mini tier
        let solved = solve(&root, 150.0, 600.0, &LayoutPreferences::default());
        assert_eq!(
            solved.find("rack").unwrap().active_tier.as_deref(),
            Some("Mini")
        );

        // Tiny → no tier
        let solved = solve(&root, 50.0, 600.0, &LayoutPreferences::default());
        assert_eq!(solved.find("rack").unwrap().active_tier, None);
    }

    // ===== Preference override tests =====

    #[test]
    fn ratio_preference_overrides_initial() {
        let children = [
            simple_slot("left", Sizing::fractional(0.3, 50.0)),
            simple_slot("right", Sizing::flex(100.0)),
        ];
        let root = LayoutNode::Container(ContainerNode {
            id: "root",
            axis: Axis::Horizontal,
            auto_axis: None,
            sizing: Sizing::flex(0.0),
            children: &children,
            divider_size: 0.0,
        });

        let prefs = LayoutPreferences {
            ratios: &[("left", Axis::Horizontal, 0.5)],
            collapsed: &[],
        };

        let solved = solve(&root, 1000.0, 600.0, &prefs);
        let left = solved.find("left").unwrap();
        assert_eq!(left.width, 500.0); // 0.5 * 1000
    }

    #[test]
    fn per_axis_ratio_preferences() {
        let children = [
            simple_slot("panel", Sizing::fractional(0.3, 50.0)),
            simple_slot("main", Sizing::flex(100.0)),
        ];
        let root = LayoutNode::Container(ContainerNode {
            id: "root",
            axis: Axis::Horizontal,
            auto_axis: Some(1.0),
            sizing: Sizing::flex(0.0),
            children: &children,
            divider_size: 0.0,
        });

        let prefs = LayoutPreferences {
            ratios: &[
                ("panel", Axis::Horizontal, 0.4),
                ("panel", Axis::Vertical, 0.25),
            ],
            collapsed: &[],
        };

        // Wide → Horizontal → uses 0.4
        let solved = solve(&root, 1000.0, 600.0, &prefs);
        let panel = solved.find("panel").unwrap();
        assert_eq!(panel.width, 400.0);

        // Tall → Vertical → uses 0.25
        let solved = solve(&root, 600.0, 1000.0, &prefs);
        let panel = solved.find("panel").unwrap();
        assert_eq!(panel.height, 250.0);
    }

    // ===== Nested container tests =====

    #[test]
    fn nested_containers() {
        let inner_children = [
            simple_slot("a", Sizing::flex(0.0)),
            simple_slot("b", Sizing::flex(0.0)),
        ];
        let children = [
            simple_slot("header", Sizing::Fixed(50.0)),
            LayoutNode::Container(ContainerNode {
                id: "content",
                axis: Axis::Horizontal,
                auto_axis: None,
                sizing: Sizing::flex(0.0),
                children: &inner_children,
                divider_size: 0.0,
            }),
            simple_slot("footer", Sizing::Fixed(80.0)),
        ];
        let root = LayoutNode::Container(ContainerNode {
            id: "root",
            axis: Axis::Vertical,
            auto_axis: None,
            sizing: Sizing::flex(0.0),
            children: &children,
            divider_size: 0.0,
        });

        let solved = solve(&root, 1000.0, 800.0, &LayoutPreferences::default());

        let content = solved.find("content").unwrap();
        assert_eq!(content.height, 670.0); // 800 - 50 - 80
        assert_eq!(content.width, 1000.0);

        let a = solved.find("a").unwrap();
        assert_eq!(a.width, 500.0); // 1000/2
        assert_eq!(a.height, 670.0);

        let b = solved.find("b").unwrap();
        assert_eq!(b.width, 500.0);
    }

    // ===== Total width/height invariant =====

    #[test]
    fn total_allocation_never_exceeds_available() {
        let children = [
            collapsible_slot("config", Sizing::fractional(0.2, 100.0), 0.5, "Config"),
            simple_slot("main", Sizing::flex(300.0)),
            collapsible_slot("diag", Sizing::fractional(0.15, 150.0), 0.3, "Diag"),
            collapsible_slot("output", Sizing::fractional(0.15, 120.0), 0.6, "Output"),
        ];
        let root = LayoutNode::Container(ContainerNode {
            id: "root",
            axis: Axis::Horizontal,
            auto_axis: None,
            sizing: Sizing::flex(0.0),
            children: &children,
            divider_size: 6.0,
        });

        for width in [200.0, 450.0, 600.0, 800.0, 1200.0] {
            let solved = solve(&root, width, 600.0, &LayoutPreferences::default());
            let visible: Vec<&SolvedNode> = solved.children.iter().filter(|c| c.visible).collect();
            let total_children: f32 = visible.iter().map(|c| c.width).sum();
            let dividers = if visible.len() > 1 {
                6.0 * (visible.len() - 1) as f32
            } else {
                0.0
            };
            let total = total_children + dividers;
            assert!(
                total <= width + 0.01,
                "width={width}: total={total} (children={total_children} + dividers={dividers})"
            );
        }
    }

    #[test]
    fn flex_min_sum_exceeds_remaining_is_scaled_down() {
        // Two flex children each with min=50 and weight=1 in a container
        // with only 80 pixels of remaining space. Without scaling, each
        // would get 50 (total 100 > 80). The fix scales them down so the
        // total never exceeds the available space.
        let children = [
            simple_slot(
                "a",
                Sizing::Flex {
                    min: 50.0,
                    weight: 1.0,
                },
            ),
            simple_slot(
                "b",
                Sizing::Flex {
                    min: 50.0,
                    weight: 1.0,
                },
            ),
        ];
        let root = LayoutNode::Container(ContainerNode {
            id: "root",
            axis: Axis::Horizontal,
            auto_axis: None,
            sizing: Sizing::flex(0.0),
            children: &children,
            divider_size: 0.0,
        });

        let solved = solve(&root, 80.0, 600.0, &LayoutPreferences::default());
        let a = solved.find("a").unwrap();
        let b = solved.find("b").unwrap();
        assert_eq!(
            a.width + b.width,
            80.0,
            "total flex allocation should not exceed available space"
        );
        assert!(a.width >= 0.0 && b.width >= 0.0);
    }

    // ===== App-like layout test =====

    #[test]
    fn app_layout_scenario() {
        // Models the SotF app: header | (library | queue | rack) | footer
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

        let content_children = [
            LayoutNode::Slot(SlotNode {
                id: "library",
                sizing: Sizing::fractional(0.3, 100.0),
                priority: 0.5,
                collapsible: true,
                display_tiers: &[],
                collapse_label: Some("Library"),
            }),
            simple_slot("queue", Sizing::flex(200.0)),
            LayoutNode::Slot(SlotNode {
                id: "rack",
                sizing: Sizing::fractional(0.3, 0.0),
                priority: 0.3,
                collapsible: true,
                display_tiers: RACK_TIERS,
                collapse_label: Some("Rack"),
            }),
        ];

        let root_children = [
            simple_slot("header", Sizing::Fixed(40.0)),
            LayoutNode::Container(ContainerNode {
                id: "content",
                axis: Axis::Horizontal,
                auto_axis: Some(1.0),
                sizing: Sizing::flex(0.0),
                children: &content_children,
                divider_size: 6.0,
            }),
            simple_slot("footer", Sizing::Fixed(100.0)),
        ];

        let root = LayoutNode::Container(ContainerNode {
            id: "root",
            axis: Axis::Vertical,
            auto_axis: None,
            sizing: Sizing::flex(0.0),
            children: &root_children,
            divider_size: 0.0,
        });

        // Wide window
        let solved = solve(&root, 1200.0, 800.0, &LayoutPreferences::default());

        assert_eq!(solved.find("header").unwrap().height, 40.0);
        assert_eq!(solved.find("footer").unwrap().height, 100.0);

        let content = solved.find("content").unwrap();
        assert_eq!(content.resolved_axis, Some(Axis::Horizontal));
        assert_eq!(content.height, 660.0); // 800 - 40 - 100

        let library = solved.find("library").unwrap();
        assert!(library.visible);

        let rack = solved.find("rack").unwrap();
        assert!(rack.visible);
        assert_eq!(rack.active_tier.as_deref(), Some("Full"));

        // Narrow tall window → vertical
        let solved = solve(&root, 500.0, 900.0, &LayoutPreferences::default());
        let content = solved.find("content").unwrap();
        assert_eq!(content.resolved_axis, Some(Axis::Vertical));
    }

    // ===== Sizing::Text tests =====

    struct FixedWidthMeasure {
        char_width: f64,
    }

    impl gpui_pretext::TextMeasure for FixedWidthMeasure {
        fn measure_width(&self, text: &str) -> f64 {
            text.chars().count() as f64 * self.char_width
        }
    }

    #[test]
    fn text_sizing_vertical_container() {
        // Each char is 10px wide. "hello world" = 110px wide, wraps at 80px.
        // At 80px max_width: "hello " on line 1, "world" on line 2 → height = 2 * 20 = 40.
        let measure = FixedWidthMeasure { char_width: 10.0 };
        let line_height = 20.0_f32;

        let children = [
            simple_slot("header", Sizing::Fixed(30.0)),
            LayoutNode::Slot(SlotNode {
                id: "label",
                sizing: Sizing::Text {
                    text: "hello world",
                    measure: &measure,
                    line_height,
                    min: 0.0,
                },
                priority: 1.0,
                collapsible: false,
                display_tiers: &[],
                collapse_label: None,
            }),
            simple_slot("footer", Sizing::Fixed(10.0)),
        ];
        let root = LayoutNode::Container(ContainerNode {
            id: "root",
            axis: Axis::Vertical,
            auto_axis: None,
            sizing: Sizing::flex(0.0),
            children: &children,
            divider_size: 0.0,
        });

        // Container is 80px wide → text wraps to 2 lines → label height = 40
        let solved = solve(&root, 80.0, 500.0, &LayoutPreferences::default());

        assert_eq!(solved.find("header").unwrap().height, 30.0);
        let label = solved.find("label").unwrap();
        assert!(label.visible);
        assert_eq!(label.height, 40.0);
        assert_eq!(solved.find("footer").unwrap().height, 10.0);
    }

    #[test]
    fn text_sizing_horizontal_container() {
        // Each char is 10px wide. "hi" = 20px wide → single line, width = 20.
        let measure = FixedWidthMeasure { char_width: 10.0 };

        let children = [
            LayoutNode::Slot(SlotNode {
                id: "tag",
                sizing: Sizing::Text {
                    text: "hi",
                    measure: &measure,
                    line_height: 20.0,
                    min: 0.0,
                },
                priority: 1.0,
                collapsible: false,
                display_tiers: &[],
                collapse_label: None,
            }),
            simple_slot("rest", Sizing::flex(0.0)),
        ];
        let root = LayoutNode::Container(ContainerNode {
            id: "root",
            axis: Axis::Horizontal,
            auto_axis: None,
            sizing: Sizing::flex(0.0),
            children: &children,
            divider_size: 0.0,
        });

        let solved = solve(&root, 500.0, 100.0, &LayoutPreferences::default());
        let tag = solved.find("tag").unwrap();
        assert!(tag.visible);
        assert_eq!(tag.width, 20.0); // "hi" = 2 chars * 10px
    }

    #[test]
    fn text_sizing_respects_min_floor() {
        // Empty text → height = 0, but min = 50 → height = 50.
        let measure = FixedWidthMeasure { char_width: 10.0 };

        let children = [LayoutNode::Slot(SlotNode {
            id: "label",
            sizing: Sizing::Text {
                text: "",
                measure: &measure,
                line_height: 20.0,
                min: 50.0,
            },
            priority: 1.0,
            collapsible: false,
            display_tiers: &[],
            collapse_label: None,
        })];
        let root = LayoutNode::Container(ContainerNode {
            id: "root",
            axis: Axis::Vertical,
            auto_axis: None,
            sizing: Sizing::flex(0.0),
            children: &children,
            divider_size: 0.0,
        });

        let solved = solve(&root, 200.0, 500.0, &LayoutPreferences::default());
        assert_eq!(solved.find("label").unwrap().height, 50.0);
    }
}
