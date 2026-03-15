//! Circle Packing — <https://observablehq.com/@d3/pack/2>
//!
//! Demonstrates: Circle packing layout for hierarchical data.
//! Uses a simplified packing algorithm (greedy front-chain).

use crate::hierarchy::HierarchyNode;
use crate::shape::path::{Path, PathBuilder};
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Debug, Clone)]
pub struct PackCircle {
    pub name: String,
    pub x: f64,
    pub y: f64,
    pub r: f64,
    pub depth: usize,
    pub is_leaf: bool,
    pub value: f64,
}

#[derive(Debug)]
pub struct PackResult {
    pub width: f64,
    pub height: f64,
    pub circles: Vec<PackCircle>,
    pub circle_paths: Vec<Path>,
}

#[derive(Clone, Debug)]
pub struct PackNode {
    pub name: String,
    pub value: f64,
}

/// Build a sample hierarchy for circle packing.
pub fn default_hierarchy() -> Rc<RefCell<HierarchyNode<PackNode>>> {
    let root = HierarchyNode::new(PackNode {
        name: "root".to_string(),
        value: 0.0,
    });

    let categories = [
        ("analytics", vec![("cluster", 3938.0), ("graph", 5714.0), ("opt", 2105.0)]),
        ("animate", vec![("easing", 1700.0), ("tween", 6006.0)]),
        ("data", vec![("field", 1082.0), ("schema", 1616.0), ("source", 1255.0)]),
        ("display", vec![("sprite", 3322.0), ("text", 2220.0)]),
        ("flex", vec![("vis", 4116.0)]),
        ("physics", vec![("drag", 1200.0), ("spring", 2314.0), ("nbody", 3416.0)]),
        ("scale", vec![("linear", 1316.0), ("log", 3151.0), ("ordinal", 1420.0)]),
        ("util", vec![("arrays", 8258.0), ("dates", 1727.0), ("maths", 3085.0)]),
    ];

    let mut children = Vec::new();
    for (cat_name, items) in &categories {
        let cat = HierarchyNode::new(PackNode {
            name: cat_name.to_string(),
            value: 0.0,
        });
        let mut cat_children = Vec::new();
        for (item_name, val) in items {
            cat_children.push(HierarchyNode::new(PackNode {
                name: item_name.to_string(),
                value: *val,
            }));
        }
        cat.borrow_mut().set_children(&cat, cat_children);
        children.push(cat);
    }
    root.borrow_mut().set_children(&root, children);
    fix_depths(root.clone(), 0);
    HierarchyNode::sum(root.clone(), |d| if d.value > 0.0 { d.value } else { 0.0 });

    root
}

fn fix_depths<T>(node: Rc<RefCell<HierarchyNode<T>>>, depth: usize) {
    node.borrow_mut().depth = depth;
    let children = node.borrow().children.clone();
    if let Some(children) = children {
        for child in children {
            fix_depths(child, depth + 1);
        }
    }
}

/// Simple circle packing layout.
///
/// This is a simplified version that:
/// 1. Assigns radius to leaves proportional to sqrt(value)
/// 2. Packs sibling circles using a greedy placement
/// 3. Computes enclosing circle for each internal node
pub fn compute() -> PackResult {
    let width: f64 = 928.0;
    let height: f64 = 928.0;
    let padding = 3.0;

    let root = default_hierarchy();

    // Collect all circles via recursive packing
    let mut circles = Vec::new();
    pack_node(&root, width / 2.0, height / 2.0, width / 2.0 - 10.0, padding, &mut circles);

    // Generate circle paths (16-gon approximation)
    let n_sides = 32;
    let circle_paths: Vec<Path> = circles
        .iter()
        .map(|c| {
            let mut builder = PathBuilder::new();
            for v in 0..n_sides {
                let angle = std::f64::consts::TAU * v as f64 / n_sides as f64;
                let x = c.x + c.r * angle.cos();
                let y = c.y + c.r * angle.sin();
                if v == 0 {
                    builder = builder.move_to(x, y);
                } else {
                    builder = builder.line_to(x, y);
                }
            }
            builder = builder.close_path();
            builder.build()
        })
        .collect();

    PackResult {
        width,
        height,
        circles,
        circle_paths,
    }
}

/// Recursively pack a hierarchy node's children into a circle without overlap.
fn pack_node(
    node: &Rc<RefCell<HierarchyNode<PackNode>>>,
    cx: f64,
    cy: f64,
    radius: f64,
    padding: f64,
    result: &mut Vec<PackCircle>,
) {
    // Extract data from borrow before recursion
    let (is_leaf, name, depth, value, child_data) = {
        let n = node.borrow();
        let is_leaf = n.children.is_none() || n.children.as_ref().unwrap().is_empty();
        let child_data: Vec<(Rc<RefCell<HierarchyNode<PackNode>>>, f64)> = n
            .children
            .as_ref()
            .map(|cs| {
                cs.iter()
                    .map(|c| (c.clone(), c.borrow().value.unwrap_or(0.0)))
                    .collect()
            })
            .unwrap_or_default();
        (is_leaf, n.data.name.clone(), n.depth, n.value.unwrap_or(0.0), child_data)
    };

    result.push(PackCircle {
        name: name.clone(),
        x: cx,
        y: cy,
        r: radius,
        depth,
        is_leaf,
        value,
    });

    if is_leaf || child_data.is_empty() {
        return;
    }

    // Sort descending (largest first for better packing)
    let mut child_data = child_data;
    child_data.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    let total_value: f64 = child_data.iter().map(|(_, v)| *v).sum();
    if total_value <= 0.0 {
        return;
    }

    // Calculate child radii: proportional to sqrt(value), scaled to fit inside parent
    let available = radius - padding;
    let child_radii: Vec<f64> = child_data
        .iter()
        .map(|(_, v)| (v / total_value).sqrt() * available * 0.65)
        .collect();

    // Place children without overlap using greedy placement
    let mut placed: Vec<(f64, f64, f64)> = Vec::new(); // (x, y, r)
    let _n_children = child_data.len();

    for (i, ((child, _), &child_r)) in child_data.iter().zip(child_radii.iter()).enumerate() {
        let (child_cx, child_cy) = if i == 0 {
            // First child at center
            (cx, cy)
        } else if i == 1 {
            // Second child adjacent to first
            (cx + placed[0].2 + child_r + padding, cy)
        } else {
            // Place each subsequent child tangent to two already-placed circles
            // Try placing around the existing circles, pick the position closest to center
            let mut best = (cx, cy + available);
            let mut best_dist = f64::MAX;
            for j in 0..placed.len() {
                for k in (j + 1)..placed.len() {
                    // Try both intersection points of circles expanded by child_r
                    let (x1, y1, r1) = placed[j];
                    let (x2, y2, r2) = placed[k];
                    let d1 = r1 + child_r + padding;
                    let d2 = r2 + child_r + padding;

                    let dx = x2 - x1;
                    let dy = y2 - y1;
                    let d = (dx * dx + dy * dy).sqrt();
                    if d > d1 + d2 || d < (d1 - d2).abs() || d < 1e-10 {
                        continue;
                    }

                    let a = (d1 * d1 - d2 * d2 + d * d) / (2.0 * d);
                    let h_sq = d1 * d1 - a * a;
                    if h_sq < 0.0 { continue; }
                    let h = h_sq.sqrt();

                    let mx = x1 + a * dx / d;
                    let my = y1 + a * dy / d;

                    for sign in [-1.0, 1.0] {
                        let px = mx + sign * h * (-dy) / d;
                        let py = my + sign * h * dx / d;

                        // Check no overlap with any placed circle
                        let overlaps = placed.iter().any(|&(ox, oy, or)| {
                            let dist = ((px - ox).powi(2) + (py - oy).powi(2)).sqrt();
                            dist < or + child_r + padding * 0.5
                        });
                        if overlaps { continue; }

                        // Check within parent
                        let dist_center = ((px - cx).powi(2) + (py - cy).powi(2)).sqrt();
                        if dist_center + child_r > available + padding { continue; }

                        if dist_center < best_dist {
                            best_dist = dist_center;
                            best = (px, py);
                        }
                    }
                }
            }
            best
        };

        placed.push((child_cx, child_cy, child_r));
        pack_node(child, child_cx, child_cy, child_r.max(3.0), padding * 0.5, result);
    }
}
