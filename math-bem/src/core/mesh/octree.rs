//! Octree construction for multi-level FMM
//!
//! This module provides hierarchical spatial partitioning for MLFMM.
//! The octree recursively subdivides space into 8 children per node,
//! enabling O(N log N) complexity for far-field interactions.

use ndarray::Array1;

/// Axis-aligned bounding box
#[derive(Debug, Clone)]
pub struct AABB {
    /// Minimum corner coordinates
    pub min: Array1<f64>,
    /// Maximum corner coordinates
    pub max: Array1<f64>,
}

impl AABB {
    /// Create a new bounding box
    pub fn new(min: Array1<f64>, max: Array1<f64>) -> Self {
        Self { min, max }
    }

    /// Create an empty (invalid) bounding box
    pub fn empty() -> Self {
        Self {
            min: Array1::from_vec(vec![f64::INFINITY, f64::INFINITY, f64::INFINITY]),
            max: Array1::from_vec(vec![
                f64::NEG_INFINITY,
                f64::NEG_INFINITY,
                f64::NEG_INFINITY,
            ]),
        }
    }

    /// Expand the bounding box to include a point
    pub fn expand(&mut self, point: &Array1<f64>) {
        for i in 0..3 {
            if point[i] < self.min[i] {
                self.min[i] = point[i];
            }
            if point[i] > self.max[i] {
                self.max[i] = point[i];
            }
        }
    }

    /// Get the center of the bounding box
    pub fn center(&self) -> Array1<f64> {
        (&self.min + &self.max) / 2.0
    }

    /// Get the half-size (extent) of the bounding box
    pub fn half_size(&self) -> Array1<f64> {
        (&self.max - &self.min) / 2.0
    }

    /// Get the maximum dimension
    pub fn max_dimension(&self) -> f64 {
        let size = &self.max - &self.min;
        size[0].max(size[1]).max(size[2])
    }

    /// Check if a point is inside the bounding box
    pub fn contains(&self, point: &Array1<f64>) -> bool {
        for i in 0..3 {
            if point[i] < self.min[i] || point[i] > self.max[i] {
                return false;
            }
        }
        true
    }

    /// Get the child octant index (0-7) for a point
    pub fn child_index(&self, point: &Array1<f64>) -> usize {
        let center = self.center();
        let mut index = 0;
        if point[0] >= center[0] {
            index |= 1;
        }
        if point[1] >= center[1] {
            index |= 2;
        }
        if point[2] >= center[2] {
            index |= 4;
        }
        index
    }

    /// Get the bounding box of a child octant
    pub fn child_bounds(&self, index: usize) -> AABB {
        let center = self.center();
        let mut child_min = self.min.clone();
        let mut child_max = self.max.clone();

        if index & 1 != 0 {
            child_min[0] = center[0];
        } else {
            child_max[0] = center[0];
        }

        if index & 2 != 0 {
            child_min[1] = center[1];
        } else {
            child_max[1] = center[1];
        }

        if index & 4 != 0 {
            child_min[2] = center[2];
        } else {
            child_max[2] = center[2];
        }

        AABB::new(child_min, child_max)
    }
}

/// Octree node
#[derive(Debug, Clone)]
pub struct OctreeNode {
    /// Bounding box of this node
    pub bounds: AABB,
    /// Center of this node
    pub center: Array1<f64>,
    /// Level in the tree (0 = root)
    pub level: usize,
    /// Parent node index (None for root)
    pub parent: Option<usize>,
    /// Children node indices (None if leaf)
    pub children: Option<[usize; 8]>,
    /// Element indices contained in this node (only for leaves)
    pub element_indices: Vec<usize>,
    /// Near-field cluster indices (for FMM)
    pub near_clusters: Vec<usize>,
    /// Far-field cluster indices (for FMM)
    pub far_clusters: Vec<usize>,
}

impl OctreeNode {
    /// Create a new octree node
    pub fn new(bounds: AABB, level: usize, parent: Option<usize>) -> Self {
        let center = bounds.center();
        Self {
            bounds,
            center,
            level,
            parent,
            children: None,
            element_indices: Vec::new(),
            near_clusters: Vec::new(),
            far_clusters: Vec::new(),
        }
    }

    /// Check if this node is a leaf
    pub fn is_leaf(&self) -> bool {
        self.children.is_none()
    }

    /// Get the radius (half diagonal) of this node
    pub fn radius(&self) -> f64 {
        let half = self.bounds.half_size();
        (half[0] * half[0] + half[1] * half[1] + half[2] * half[2]).sqrt()
    }
}

/// Octree for spatial partitioning
#[derive(Debug, Clone)]
pub struct Octree {
    /// All nodes in the tree
    pub nodes: Vec<OctreeNode>,
    /// Maximum elements per leaf before subdivision
    pub max_elements_per_leaf: usize,
    /// Maximum tree depth
    pub max_depth: usize,
    /// Number of leaves
    pub num_leaves: usize,
    /// Number of levels
    pub num_levels: usize,
}

impl Octree {
    /// Build an octree from element centers
    ///
    /// # Arguments
    /// * `centers` - Element center coordinates
    /// * `max_per_leaf` - Maximum elements per leaf node
    /// * `max_depth` - Maximum tree depth
    pub fn build(centers: &[Array1<f64>], max_per_leaf: usize, max_depth: usize) -> Self {
        if centers.is_empty() {
            return Self {
                nodes: Vec::new(),
                max_elements_per_leaf: max_per_leaf,
                max_depth,
                num_leaves: 0,
                num_levels: 0,
            };
        }

        // Compute bounding box of all centers
        let mut bounds = AABB::empty();
        for center in centers {
            bounds.expand(center);
        }

        // Add small padding to avoid boundary issues
        let padding = bounds.max_dimension() * 0.01;
        for i in 0..3 {
            bounds.min[i] -= padding;
            bounds.max[i] += padding;
        }

        // Make the bounding box cubic (same size in all dimensions)
        let max_dim = bounds.max_dimension();
        let center = bounds.center();
        let half = max_dim / 2.0;
        bounds.min = Array1::from_vec(vec![center[0] - half, center[1] - half, center[2] - half]);
        bounds.max = Array1::from_vec(vec![center[0] + half, center[1] + half, center[2] + half]);

        let mut octree = Self {
            nodes: Vec::new(),
            max_elements_per_leaf: max_per_leaf,
            max_depth,
            num_leaves: 0,
            num_levels: 0,
        };

        // Create root node
        let root = OctreeNode::new(bounds, 0, None);
        octree.nodes.push(root);

        // Insert all elements
        let all_indices: Vec<usize> = (0..centers.len()).collect();
        octree.nodes[0].element_indices = all_indices;

        // Recursively subdivide
        octree.subdivide(0, centers);

        // Count leaves and levels
        octree.num_leaves = octree.nodes.iter().filter(|n| n.is_leaf()).count();
        octree.num_levels = octree.nodes.iter().map(|n| n.level).max().unwrap_or(0) + 1;

        octree
    }

    /// Recursively subdivide a node
    fn subdivide(&mut self, node_idx: usize, centers: &[Array1<f64>]) {
        let node = &self.nodes[node_idx];

        // Check termination conditions
        if node.level >= self.max_depth {
            return;
        }
        if node.element_indices.len() <= self.max_elements_per_leaf {
            return;
        }

        let bounds = node.bounds.clone();
        let level = node.level;
        let element_indices = node.element_indices.clone();

        // Create 8 children
        let mut child_indices = [0usize; 8];
        let first_child = self.nodes.len();

        for (i, child_idx) in child_indices.iter_mut().enumerate() {
            let child_bounds = bounds.child_bounds(i);
            let child = OctreeNode::new(child_bounds, level + 1, Some(node_idx));
            self.nodes.push(child);
            *child_idx = first_child + i;
        }

        // Distribute elements to children
        for &elem_idx in &element_indices {
            let octant = bounds.child_index(&centers[elem_idx]);
            self.nodes[child_indices[octant]]
                .element_indices
                .push(elem_idx);
        }

        // Clear parent's elements
        self.nodes[node_idx].element_indices.clear();
        self.nodes[node_idx].children = Some(child_indices);

        // Recursively subdivide non-empty children
        for &child_idx in &child_indices {
            if !self.nodes[child_idx].element_indices.is_empty() {
                self.subdivide(child_idx, centers);
            }
        }
    }

    /// Get indices of all leaf nodes
    pub fn leaves(&self) -> Vec<usize> {
        self.nodes
            .iter()
            .enumerate()
            .filter(|(_, n)| n.is_leaf() && !n.element_indices.is_empty())
            .map(|(i, _)| i)
            .collect()
    }

    /// Get indices of nodes at a given level
    pub fn level_nodes(&self, level: usize) -> Vec<usize> {
        self.nodes
            .iter()
            .enumerate()
            .filter(|(_, n)| n.level == level)
            .map(|(i, _)| i)
            .collect()
    }

    /// Get all non-empty nodes
    pub fn non_empty_nodes(&self) -> Vec<usize> {
        self.nodes
            .iter()
            .enumerate()
            .filter(|(_, n)| !n.element_indices.is_empty() || n.children.is_some())
            .map(|(i, _)| i)
            .collect()
    }

    /// Compute near/far cluster lists for FMM
    ///
    /// Two nodes are in the far-field if their distance is greater than
    /// `separation_ratio * (radius_i + radius_j)`.
    pub fn compute_interaction_lists(&mut self, separation_ratio: f64) {
        // Get all leaf indices
        let leaves = self.leaves();

        // First, collect all the data we need (immutable borrow)
        let node_data: Vec<(Array1<f64>, f64)> = leaves
            .iter()
            .map(|&i| (self.nodes[i].center.clone(), self.nodes[i].radius()))
            .collect();

        // Now compute interaction lists
        for (idx_i, &i) in leaves.iter().enumerate() {
            let (ref center_i, radius_i) = node_data[idx_i];

            let mut near = Vec::new();
            let mut far = Vec::new();

            for (idx_j, &j) in leaves.iter().enumerate() {
                if i == j {
                    // Self-interaction is always near-field
                    near.push(j);
                    continue;
                }

                let (ref center_j, radius_j) = node_data[idx_j];

                // Distance between centers
                let dist = distance(center_i, center_j);

                // Separation criterion
                let separation = separation_ratio * (radius_i + radius_j);

                if dist > separation {
                    far.push(j);
                } else {
                    near.push(j);
                }
            }

            // Now apply the results (mutable borrow)
            self.nodes[i].near_clusters = near;
            self.nodes[i].far_clusters = far;
        }
    }

    /// Get statistics about the octree
    pub fn stats(&self) -> OctreeStats {
        let leaves = self.leaves();
        let num_leaves = leaves.len();

        let elements_per_leaf: Vec<usize> = leaves
            .iter()
            .map(|&i| self.nodes[i].element_indices.len())
            .collect();

        let total_elements: usize = elements_per_leaf.iter().sum();
        let avg_elements = if num_leaves > 0 {
            total_elements as f64 / num_leaves as f64
        } else {
            0.0
        };
        let max_elements = elements_per_leaf.iter().copied().max().unwrap_or(0);
        let min_elements = elements_per_leaf.iter().copied().min().unwrap_or(0);

        OctreeStats {
            num_nodes: self.nodes.len(),
            num_leaves,
            num_levels: self.num_levels,
            avg_elements_per_leaf: avg_elements,
            max_elements_per_leaf: max_elements,
            min_elements_per_leaf: min_elements,
        }
    }
}

/// Statistics about an octree
#[derive(Debug, Clone)]
pub struct OctreeStats {
    /// Total number of nodes
    pub num_nodes: usize,
    /// Number of leaf nodes
    pub num_leaves: usize,
    /// Number of levels
    pub num_levels: usize,
    /// Average elements per leaf
    pub avg_elements_per_leaf: f64,
    /// Maximum elements in any leaf
    pub max_elements_per_leaf: usize,
    /// Minimum elements in any non-empty leaf
    pub min_elements_per_leaf: usize,
}

/// Euclidean distance between two points
fn distance(a: &Array1<f64>, b: &Array1<f64>) -> f64 {
    let diff = a - b;
    diff.dot(&diff).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_points() -> Vec<Array1<f64>> {
        vec![
            Array1::from_vec(vec![0.0, 0.0, 0.0]),
            Array1::from_vec(vec![1.0, 0.0, 0.0]),
            Array1::from_vec(vec![0.0, 1.0, 0.0]),
            Array1::from_vec(vec![1.0, 1.0, 0.0]),
            Array1::from_vec(vec![0.0, 0.0, 1.0]),
            Array1::from_vec(vec![1.0, 0.0, 1.0]),
            Array1::from_vec(vec![0.0, 1.0, 1.0]),
            Array1::from_vec(vec![1.0, 1.0, 1.0]),
        ]
    }

    #[test]
    fn test_aabb_basic() {
        let min = Array1::from_vec(vec![0.0, 0.0, 0.0]);
        let max = Array1::from_vec(vec![1.0, 1.0, 1.0]);
        let aabb = AABB::new(min, max);

        let center = aabb.center();
        assert!((center[0] - 0.5).abs() < 1e-10);
        assert!((center[1] - 0.5).abs() < 1e-10);
        assert!((center[2] - 0.5).abs() < 1e-10);
    }

    #[test]
    fn test_aabb_child_index() {
        let min = Array1::from_vec(vec![0.0, 0.0, 0.0]);
        let max = Array1::from_vec(vec![2.0, 2.0, 2.0]);
        let aabb = AABB::new(min, max);

        // Point in octant 0 (x < cx, y < cy, z < cz)
        let p0 = Array1::from_vec(vec![0.5, 0.5, 0.5]);
        assert_eq!(aabb.child_index(&p0), 0);

        // Point in octant 7 (x >= cx, y >= cy, z >= cz)
        let p7 = Array1::from_vec(vec![1.5, 1.5, 1.5]);
        assert_eq!(aabb.child_index(&p7), 7);
    }

    #[test]
    fn test_octree_build() {
        let points = make_test_points();
        let octree = Octree::build(&points, 2, 4);

        // Should have created some nodes
        assert!(!octree.nodes.is_empty());

        // All points should be in leaves
        let leaves = octree.leaves();
        let mut total_elements = 0;
        for &leaf_idx in &leaves {
            total_elements += octree.nodes[leaf_idx].element_indices.len();
        }
        assert_eq!(total_elements, points.len());
    }

    #[test]
    fn test_octree_empty() {
        let points: Vec<Array1<f64>> = vec![];
        let octree = Octree::build(&points, 2, 4);
        assert!(octree.nodes.is_empty());
    }

    #[test]
    fn test_octree_single_point() {
        let points = vec![Array1::from_vec(vec![0.5, 0.5, 0.5])];
        let octree = Octree::build(&points, 2, 4);

        assert_eq!(octree.nodes.len(), 1);
        assert!(octree.nodes[0].is_leaf());
        assert_eq!(octree.nodes[0].element_indices.len(), 1);
    }

    #[test]
    fn test_octree_levels() {
        let points = make_test_points();
        let octree = Octree::build(&points, 1, 4);

        // Check that level 0 has one node (root)
        let level0 = octree.level_nodes(0);
        assert_eq!(level0.len(), 1);

        // Root should have children
        assert!(octree.nodes[0].children.is_some());
    }

    #[test]
    fn test_interaction_lists() {
        let points = make_test_points();
        let mut octree = Octree::build(&points, 2, 4);

        // Compute interaction lists with typical FMM separation
        octree.compute_interaction_lists(1.5);

        // Each leaf should have itself in near-field
        for leaf_idx in octree.leaves() {
            let node = &octree.nodes[leaf_idx];
            assert!(node.near_clusters.contains(&leaf_idx));
        }
    }

    #[test]
    fn test_octree_stats() {
        let points = make_test_points();
        let octree = Octree::build(&points, 2, 4);

        let stats = octree.stats();
        assert!(stats.num_leaves > 0);
        assert!(stats.num_levels >= 1);
    }
}
