//! Domain decomposition for Schwarz-PML solver
//!
//! Implements 1D strip decomposition along the x-axis with overlap regions
//! and PML extensions at artificial boundaries.

use crate::boundary::{PmlProfile, PmlRegion, optimal_sigma_max};
use crate::mesh::{Mesh, Point};
use crate::schwarz_pml::config::SchwarzPmlConfig;
use std::collections::{HashMap, HashSet};
use std::f64::consts::PI;

/// Information about a single subdomain in the decomposition
#[derive(Debug, Clone)]
pub struct SubdomainInfo {
    /// Subdomain index
    pub index: usize,
    /// Core subdomain x-extent (without overlap)
    pub core_x_min: f64,
    pub core_x_max: f64,
    /// Extended subdomain x-extent (including overlap)
    pub overlap_x_min: f64,
    pub overlap_x_max: f64,
    /// Full subdomain x-extent (including PML extensions)
    pub full_x_min: f64,
    pub full_x_max: f64,
    /// Whether the left boundary is artificial (interior, needs PML)
    pub is_left_artificial: bool,
    /// Whether the right boundary is artificial (interior, needs PML)
    pub is_right_artificial: bool,
    /// Mapping from global node indices to local node indices
    pub global_to_local: HashMap<usize, usize>,
    /// Mapping from local node indices to global node indices
    pub local_to_global: Vec<usize>,
    /// Global element indices belonging to this subdomain
    pub element_indices: Vec<usize>,
    /// PML regions for this subdomain
    pub pml_regions: Vec<PmlRegion>,
    /// Local node indices where homogeneous Dirichlet BCs are applied (PML truncation boundary)
    pub dirichlet_local_nodes: HashSet<usize>,
    /// Local node indices on the overlap boundary (overlap/PML interface)
    /// These receive Dirichlet values from the current iterate during Schwarz iteration
    pub overlap_boundary_nodes: HashSet<usize>,
    /// Y-extent of the domain (for reference)
    pub y_min: f64,
    pub y_max: f64,
}

/// Decompose a mesh into overlapping subdomains with PML extensions
///
/// Uses 1D strip decomposition along the x-axis. Each strip extends the full
/// y-range of the domain.
///
/// # Arguments
/// * `mesh` - The global mesh
/// * `config` - Schwarz-PML configuration
/// * `k` - Wavenumber (real part, for PML thickness calculation)
pub fn decompose_domain(
    mesh: &Mesh,
    config: &SchwarzPmlConfig,
    k: f64,
) -> Vec<SubdomainInfo> {
    let n = config.num_subdomains;
    assert!(n >= 2, "Need at least 2 subdomains");

    // Compute bounding box
    let (x_min, x_max, y_min, y_max) = bounding_box(mesh);
    let domain_width = x_max - x_min;
    let strip_width = domain_width / n as f64;

    // PML thickness = pml_wavelengths * lambda, where lambda = 2*pi/k
    let lambda = 2.0 * PI / k.max(1e-10);
    let pml_thickness = config.pml_wavelengths * lambda;

    // Overlap delta
    let delta = config.overlap_fraction * strip_width;

    let mut subdomains = Vec::with_capacity(n);

    for j in 0..n {
        let core_x_min = x_min + j as f64 * strip_width;
        let core_x_max = x_min + (j + 1) as f64 * strip_width;

        let is_left_artificial = j > 0;
        let is_right_artificial = j < n - 1;

        // Extend by overlap on artificial sides
        let overlap_x_min = if is_left_artificial {
            core_x_min - delta
        } else {
            core_x_min
        };
        let overlap_x_max = if is_right_artificial {
            core_x_max + delta
        } else {
            core_x_max
        };

        // Extend by PML on artificial sides (clamp to domain)
        let full_x_min = if is_left_artificial {
            (overlap_x_min - pml_thickness).max(x_min)
        } else {
            overlap_x_min
        };
        let full_x_max = if is_right_artificial {
            (overlap_x_max + pml_thickness).min(x_max)
        } else {
            overlap_x_max
        };

        // Classify elements: include if centroid falls within [full_x_min, full_x_max]
        let element_indices: Vec<usize> = (0..mesh.num_elements())
            .filter(|&ei| {
                let centroid = mesh.element_centroid(ei);
                centroid.x >= full_x_min && centroid.x <= full_x_max
            })
            .collect();

        // Build node mappings from selected elements
        let mut node_set: HashSet<usize> = HashSet::new();
        for &ei in &element_indices {
            for &node in mesh.elements[ei].vertices() {
                node_set.insert(node);
            }
        }

        let mut local_to_global: Vec<usize> = node_set.into_iter().collect();
        local_to_global.sort_unstable();

        let global_to_local: HashMap<usize, usize> = local_to_global
            .iter()
            .enumerate()
            .map(|(local, &global)| (global, local))
            .collect();

        // Create PML regions
        let mut pml_regions = Vec::new();
        if is_left_artificial {
            let actual_pml_thickness = overlap_x_min - full_x_min;
            if actual_pml_thickness > 1e-14 {
                let sigma_max = optimal_sigma_max(
                    config.pml_power,
                    actual_pml_thickness,
                    k,
                    config.pml_target_reflection,
                );
                let mut pml = PmlRegion::x_negative(overlap_x_min, actual_pml_thickness, sigma_max, k);
                pml.profile = PmlProfile::Polynomial { power: config.pml_power };
                pml_regions.push(pml);
            }
        }
        if is_right_artificial {
            let actual_pml_thickness = full_x_max - overlap_x_max;
            if actual_pml_thickness > 1e-14 {
                let sigma_max = optimal_sigma_max(
                    config.pml_power,
                    actual_pml_thickness,
                    k,
                    config.pml_target_reflection,
                );
                let mut pml = PmlRegion::x_positive(overlap_x_max, actual_pml_thickness, sigma_max, k);
                pml.profile = PmlProfile::Polynomial { power: config.pml_power };
                pml_regions.push(pml);
            }
        }

        // Mark PML truncation boundary nodes as Dirichlet
        // These are nodes at the outermost PML edges
        let pml_boundary_tol = 1e-10;
        let mut dirichlet_local_nodes = HashSet::new();
        // Mark overlap boundary nodes (overlap/PML interface)
        // These receive Dirichlet values from the current Schwarz iterate
        let mut overlap_boundary_nodes = HashSet::new();
        for (local_idx, &global_idx) in local_to_global.iter().enumerate() {
            let p = &mesh.nodes[global_idx];
            if is_left_artificial && (p.x - full_x_min).abs() < pml_boundary_tol {
                dirichlet_local_nodes.insert(local_idx);
            }
            if is_right_artificial && (p.x - full_x_max).abs() < pml_boundary_tol {
                dirichlet_local_nodes.insert(local_idx);
            }
            if is_left_artificial && (p.x - overlap_x_min).abs() < pml_boundary_tol {
                overlap_boundary_nodes.insert(local_idx);
            }
            if is_right_artificial && (p.x - overlap_x_max).abs() < pml_boundary_tol {
                overlap_boundary_nodes.insert(local_idx);
            }
        }

        subdomains.push(SubdomainInfo {
            index: j,
            core_x_min,
            core_x_max,
            overlap_x_min,
            overlap_x_max,
            full_x_min,
            full_x_max,
            is_left_artificial,
            is_right_artificial,
            global_to_local,
            local_to_global,
            element_indices,
            pml_regions,
            dirichlet_local_nodes,
            overlap_boundary_nodes,
            y_min,
            y_max,
        });
    }

    subdomains
}

/// Compute partition-of-unity weights for each global node
///
/// Returns a vector of (subdomain_index, weight) for each global node.
/// In the overlap region between subdomains j and j+1, weights blend linearly.
/// Nodes in PML regions (outside overlap) get weight 0.
pub fn compute_partition_of_unity(
    mesh: &Mesh,
    subdomains: &[SubdomainInfo],
) -> Vec<Vec<(usize, f64)>> {
    let n_nodes = mesh.num_nodes();
    let mut weights: Vec<Vec<(usize, f64)>> = vec![Vec::new(); n_nodes];

    for sub in subdomains {
        for (&global, &_local) in &sub.global_to_local {
            let x = mesh.nodes[global].x;

            // Weight is 1 in the core region, blends linearly in overlap, 0 in PML-only
            let w = partition_weight(x, sub);
            if w > 0.0 {
                weights[global].push((sub.index, w));
            }
        }
    }

    // Normalize weights so they sum to 1 at each node
    for node_weights in &mut weights {
        let total: f64 = node_weights.iter().map(|(_, w)| *w).sum();
        if total > 1e-15 {
            for (_, w) in node_weights.iter_mut() {
                *w /= total;
            }
        }
    }

    weights
}

/// Compute the un-normalized partition-of-unity weight for a point in a subdomain
fn partition_weight(x: f64, sub: &SubdomainInfo) -> f64 {
    // Inside core region: weight = 1
    if x >= sub.core_x_min && x <= sub.core_x_max {
        return 1.0;
    }

    // In left overlap (between overlap_x_min and core_x_min)
    if sub.is_left_artificial && x >= sub.overlap_x_min && x < sub.core_x_min {
        let overlap_width = sub.core_x_min - sub.overlap_x_min;
        if overlap_width > 1e-15 {
            return (x - sub.overlap_x_min) / overlap_width;
        }
        return 0.0;
    }

    // In right overlap (between core_x_max and overlap_x_max)
    if sub.is_right_artificial && x > sub.core_x_max && x <= sub.overlap_x_max {
        let overlap_width = sub.overlap_x_max - sub.core_x_max;
        if overlap_width > 1e-15 {
            return (sub.overlap_x_max - x) / overlap_width;
        }
        return 0.0;
    }

    // In PML region (beyond overlap): weight = 0
    0.0
}

/// Compute the bounding box of a mesh
fn bounding_box(mesh: &Mesh) -> (f64, f64, f64, f64) {
    let mut x_min = f64::INFINITY;
    let mut x_max = f64::NEG_INFINITY;
    let mut y_min = f64::INFINITY;
    let mut y_max = f64::NEG_INFINITY;

    for node in &mesh.nodes {
        x_min = x_min.min(node.x);
        x_max = x_max.max(node.x);
        y_min = y_min.min(node.y);
        y_max = y_max.max(node.y);
    }

    (x_min, x_max, y_min, y_max)
}

/// Extract a local mesh for a subdomain (for constructing local RHS)
///
/// Returns (local_mesh, global_to_local_map)
/// The local mesh has renumbered nodes and elements using only the subdomain's nodes.
pub fn extract_local_mesh(mesh: &Mesh, subdomain: &SubdomainInfo) -> Mesh {
    let mut local_mesh = Mesh::new(mesh.dimension);

    // Add nodes in local order
    for &global_idx in &subdomain.local_to_global {
        let p = &mesh.nodes[global_idx];
        local_mesh.add_node(Point::new_3d(p.x, p.y, p.z));
    }

    // Add elements with remapped node indices
    for &elem_idx in &subdomain.element_indices {
        let elem = &mesh.elements[elem_idx];
        let local_nodes: Vec<usize> = elem
            .vertices()
            .iter()
            .map(|&g| subdomain.global_to_local[&g])
            .collect();
        local_mesh.add_element(elem.element_type, local_nodes);
    }

    local_mesh
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::unit_square_triangles;

    #[test]
    fn test_decompose_domain_basic() {
        let mesh = unit_square_triangles(8);
        let config = SchwarzPmlConfig {
            num_subdomains: 2,
            ..Default::default()
        };

        let subs = decompose_domain(&mesh, &config, 2.0);
        assert_eq!(subs.len(), 2);

        // First subdomain should not have left artificial boundary
        assert!(!subs[0].is_left_artificial);
        assert!(subs[0].is_right_artificial);

        // Last subdomain should not have right artificial boundary
        assert!(subs[1].is_left_artificial);
        assert!(!subs[1].is_right_artificial);

        // All elements should be covered
        let mut all_elements: HashSet<usize> = HashSet::new();
        for sub in &subs {
            for &ei in &sub.element_indices {
                all_elements.insert(ei);
            }
        }
        assert_eq!(all_elements.len(), mesh.num_elements());
    }

    #[test]
    fn test_partition_of_unity_sums_to_one() {
        let mesh = unit_square_triangles(8);
        let config = SchwarzPmlConfig {
            num_subdomains: 3,
            ..Default::default()
        };

        let subs = decompose_domain(&mesh, &config, 2.0);
        let weights = compute_partition_of_unity(&mesh, &subs);

        // For interior nodes that are covered by at least one subdomain,
        // weights should sum to approximately 1
        for (i, node_weights) in weights.iter().enumerate() {
            if !node_weights.is_empty() {
                let total: f64 = node_weights.iter().map(|(_, w)| *w).sum();
                assert!(
                    (total - 1.0).abs() < 1e-10,
                    "Node {} weights sum to {} instead of 1.0",
                    i,
                    total
                );
            }
        }
    }

    #[test]
    fn test_pml_regions_created() {
        let mesh = unit_square_triangles(8);
        let config = SchwarzPmlConfig {
            num_subdomains: 3,
            ..Default::default()
        };

        let subs = decompose_domain(&mesh, &config, 5.0);

        // Middle subdomain should have 2 PML regions (left and right)
        assert_eq!(subs[1].pml_regions.len(), 2);

        // First subdomain should have 1 PML region (right only)
        assert_eq!(subs[0].pml_regions.len(), 1);

        // Last subdomain should have 1 PML region (left only)
        assert_eq!(subs[2].pml_regions.len(), 1);
    }

    #[test]
    fn test_extract_local_mesh() {
        let mesh = unit_square_triangles(8);
        let config = SchwarzPmlConfig {
            num_subdomains: 2,
            ..Default::default()
        };

        let subs = decompose_domain(&mesh, &config, 2.0);
        let local_mesh = extract_local_mesh(&mesh, &subs[0]);

        assert_eq!(local_mesh.num_nodes(), subs[0].local_to_global.len());
        assert_eq!(local_mesh.num_elements(), subs[0].element_indices.len());
    }
}
