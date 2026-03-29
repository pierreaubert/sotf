//! Solver output types.
//!
//! The solver produces a `SolvedNode` tree that mirrors the input `LayoutNode`
//! tree, with concrete pixel sizes and visibility states resolved.

use crate::types::Axis;

/// A resolved node in the layout tree.
#[derive(Debug, Clone)]
pub struct SolvedNode {
    /// Matches the `id` from the source `LayoutNode`.
    pub id: String,
    /// Resolved width in pixels.
    pub width: f32,
    /// Resolved height in pixels.
    pub height: f32,
    /// Whether this node is visible (false = collapsed or hidden).
    pub visible: bool,
    /// Which display tier is active (for slots with `display_tiers`).
    /// `None` if no tier matches or node has no tiers.
    pub active_tier: Option<String>,
    /// Tab label if this slot was collapsed.
    pub collapse_label: Option<String>,
    /// The resolved axis for this container (`None` for slots).
    pub resolved_axis: Option<Axis>,
    /// Resolved children (empty for slots, populated for containers).
    pub children: Vec<SolvedNode>,
}

impl SolvedNode {
    /// Find a solved node by id (depth-first search).
    pub fn find(&self, id: &str) -> Option<&SolvedNode> {
        if self.id == id {
            return Some(self);
        }
        for child in &self.children {
            if let Some(found) = child.find(id) {
                return Some(found);
            }
        }
        None
    }

    /// Returns the size along the given axis.
    pub fn size_along(&self, axis: Axis) -> f32 {
        match axis {
            Axis::Horizontal => self.width,
            Axis::Vertical => self.height,
        }
    }

    /// Collect all collapsed nodes with their labels.
    pub fn collapsed_tabs(&self) -> Vec<(&str, &str)> {
        let mut tabs = Vec::new();
        self.collect_collapsed(&mut tabs);
        tabs
    }

    fn collect_collapsed<'a>(&'a self, tabs: &mut Vec<(&'a str, &'a str)>) {
        if !self.visible
            && let Some(ref label) = self.collapse_label
        {
            tabs.push((&self.id, label));
        }
        for child in &self.children {
            child.collect_collapsed(tabs);
        }
    }
}
