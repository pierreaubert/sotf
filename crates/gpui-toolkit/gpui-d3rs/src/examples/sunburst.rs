//! Sunburst — <https://observablehq.com/@d3/sunburst/2>
//!
//! Demonstrates: Partition layout with Arc rendering for hierarchical data.

use crate::hierarchy::HierarchyNode;
use crate::shape::arc::{Arc, ArcDatum};
use crate::shape::path::Path;
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Debug, Clone)]
pub struct SunburstSlice {
    pub name: String,
    pub depth: usize,
    pub x0: f64, // start angle
    pub x1: f64, // end angle
    pub y0: f64, // inner radius
    pub y1: f64, // outer radius
    pub value: f64,
    pub arc_path: Path,
}

#[derive(Debug)]
pub struct SunburstResult {
    pub width: f64,
    pub height: f64,
    pub slices: Vec<SunburstSlice>,
}

#[derive(Clone, Debug)]
pub struct SunNode {
    pub name: String,
    pub value: f64,
}

/// Build a sample hierarchy for sunburst.
pub fn default_hierarchy() -> Rc<RefCell<HierarchyNode<SunNode>>> {
    let root = HierarchyNode::new(SunNode {
        name: "root".to_string(),
        value: 0.0,
    });

    let categories = [
        ("analytics", vec![("cluster", 3938.0), ("graph", 5714.0), ("optimization", 2105.0)]),
        ("animate", vec![("easing", 1700.0), ("tween", 6006.0), ("interpolate", 2801.0)]),
        ("data", vec![("converters", 1082.0), ("field", 1616.0), ("schema", 1255.0)]),
        ("display", vec![("sprite", 3322.0), ("text", 2220.0), ("render", 4230.0)]),
        ("physics", vec![("drag", 1200.0), ("spring", 2314.0), ("nbody", 3416.0)]),
        ("scale", vec![("linear", 1316.0), ("log", 3151.0), ("time", 5290.0)]),
        ("util", vec![("arrays", 8258.0), ("dates", 1727.0), ("maths", 3085.0)]),
    ];

    let mut children = Vec::new();
    for (cat_name, items) in &categories {
        let cat = HierarchyNode::new(SunNode {
            name: cat_name.to_string(),
            value: 0.0,
        });
        let mut cat_children = Vec::new();
        for (item_name, val) in items {
            cat_children.push(HierarchyNode::new(SunNode {
                name: item_name.to_string(),
                value: *val,
            }));
        }
        cat.borrow_mut().set_children(&cat, cat_children);
        children.push(cat);
    }
    root.borrow_mut().set_children(&root, children);
    // Fix depths: set_children only propagates one level, so re-compute all depths
    fix_depths(root.clone(), 0);
    HierarchyNode::sum(root.clone(), |d| if d.value > 0.0 { d.value } else { 0.0 });

    root
}

/// Recursively fix depth values after tree construction.
fn fix_depths<T>(node: Rc<RefCell<HierarchyNode<T>>>, depth: usize) {
    node.borrow_mut().depth = depth;
    let children = node.borrow().children.clone();
    if let Some(children) = children {
        for child in children {
            fix_depths(child, depth + 1);
        }
    }
}

/// Compute sunburst layout using partition (angular x, radial y).
pub fn compute() -> SunburstResult {
    let width: f64 = 928.0;
    let height: f64 = 928.0;
    let radius = width.min(height) / 2.0;

    let root = default_hierarchy();
    let root_value = root.borrow().value.unwrap_or(1.0);

    // Compute max depth by traversal (HierarchyNode.height isn't auto-computed)
    let mut max_depth = 0usize;
    HierarchyNode::each(root.clone(), |n| {
        let d = n.borrow().depth;
        if d > max_depth { max_depth = d; }
    });
    let ring_width = radius / (max_depth as f64 + 1.0);

    // Partition layout: assign angular extent based on value proportion
    let mut slices = Vec::new();
    let arc_gen = Arc::default();
    partition_node(
        &root,
        0.0,
        std::f64::consts::TAU,
        root_value,
        ring_width,
        &arc_gen,
        &mut slices,
    );

    SunburstResult {
        width,
        height,
        slices,
    }
}

fn partition_node(
    node: &Rc<RefCell<HierarchyNode<SunNode>>>,
    start_angle: f64,
    end_angle: f64,
    parent_value: f64,
    ring_width: f64,
    arc_gen: &Arc,
    result: &mut Vec<SunburstSlice>,
) {
    // Extract all needed data from borrow, then drop it before recursion
    let (depth, value, name, children_data) = {
        let n = node.borrow();
        let children_data: Vec<(Rc<RefCell<HierarchyNode<SunNode>>>, f64)> = n
            .children
            .as_ref()
            .map(|cs| {
                cs.iter()
                    .map(|c| (c.clone(), c.borrow().value.unwrap_or(0.0)))
                    .collect()
            })
            .unwrap_or_default();
        (n.depth, n.value.unwrap_or(0.0), n.data.name.clone(), children_data)
    };

    let y0 = depth as f64 * ring_width;
    let y1 = y0 + ring_width;

    // Skip root (depth 0) from rendering but still recurse
    if depth > 0 && end_angle - start_angle > 0.001 {
        let datum = ArcDatum {
            inner_radius: y0,
            outer_radius: y1,
            start_angle,
            end_angle,
            corner_radius: 0.0,
            pad_angle: 0.0,
        };
        let arc_path = arc_gen.generate(&datum);

        result.push(SunburstSlice {
            name,
            depth,
            x0: start_angle,
            x1: end_angle,
            y0,
            y1,
            value,
            arc_path,
        });
    }

    // Recurse into children
    let mut angle = start_angle;
    for (child, child_val) in &children_data {
        let child_extent = if parent_value > 0.0 {
            (end_angle - start_angle) * (child_val / parent_value)
        } else {
            0.0
        };
        partition_node(
            child,
            angle,
            angle + child_extent,
            *child_val,
            ring_width,
            arc_gen,
            result,
        );
        angle += child_extent;
    }
}
