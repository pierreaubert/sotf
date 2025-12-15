//! Traditional BEM (O(N²) dense matrix) assembly
//!
//! Direct port of NC_BuildSystemTBEM from NC_EquationSystem.cpp.
//! Uses the Burton-Miller formulation to avoid spurious resonances.

use ndarray::{Array1, Array2};
use num_complex::Complex64;

use crate::core::integration::{regular_integration, singular_integration};
use crate::core::types::{BoundaryCondition, Element, IntegrationResult, PhysicsParams};

/// Result of TBEM assembly
pub struct TbemSystem {
    /// Coefficient matrix (dense, row-major)
    pub matrix: Array2<Complex64>,
    /// Right-hand side vector
    pub rhs: Array1<Complex64>,
    /// Number of DOFs
    pub num_dofs: usize,
}

impl TbemSystem {
    /// Create a new system with given number of DOFs
    pub fn new(num_dofs: usize) -> Self {
        Self {
            matrix: Array2::zeros((num_dofs, num_dofs)),
            rhs: Array1::zeros(num_dofs),
            num_dofs,
        }
    }
}

/// Build the TBEM system matrix and RHS vector
///
/// This implements NC_BuildSystemTBEM from the C++ code.
/// Uses Burton-Miller formulation: (γτG + βH^T) for the system matrix.
///
/// # Arguments
/// * `elements` - Vector of mesh elements
/// * `nodes` - Node coordinates (num_nodes × 3)
/// * `physics` - Physics parameters (wave number, etc.)
///
/// # Returns
/// TbemSystem with assembled matrix and RHS
pub fn build_tbem_system(
    elements: &[Element],
    nodes: &Array2<f64>,
    physics: &PhysicsParams,
) -> TbemSystem {
    build_tbem_system_with_beta(elements, nodes, physics, physics.burton_miller_beta())
}

/// Build TBEM system with mesh-adaptive Burton-Miller coupling
///
/// Uses β = i/(k + k_ref) where k_ref = 1/element_size for better conditioning
/// at low-to-mid frequencies (ka < 2). This avoids the 1/k divergence of the
/// traditional β = i/k formulation.
///
/// # Arguments
/// * `elements` - Vector of mesh elements
/// * `nodes` - Node coordinates (num_nodes × 3)
/// * `physics` - Physics parameters (wave number, etc.)
/// * `avg_element_size` - Average element edge length or sqrt(area)
pub fn build_tbem_system_bounded(
    elements: &[Element],
    nodes: &Array2<f64>,
    physics: &PhysicsParams,
    avg_element_size: f64,
) -> TbemSystem {
    let beta = physics.burton_miller_beta_optimal(avg_element_size);
    build_tbem_system_with_beta(elements, nodes, physics, beta)
}

/// Build TBEM system with scaled Burton-Miller coupling
///
/// Uses β = scale × i/k for improved numerical conditioning.
/// Testing shows scale = 2.0 gives significantly better accuracy than the
/// standard β = i/k, especially at low to medium frequencies (ka < 2).
///
/// # Arguments
/// * `elements` - Vector of mesh elements
/// * `nodes` - Node coordinates (num_nodes × 3)
/// * `physics` - Physics parameters (wave number, etc.)
/// * `scale` - Beta scaling factor (recommended: 2.0)
pub fn build_tbem_system_scaled(
    elements: &[Element],
    nodes: &Array2<f64>,
    physics: &PhysicsParams,
    scale: f64,
) -> TbemSystem {
    let beta = physics.burton_miller_beta_scaled(scale);
    build_tbem_system_with_beta(elements, nodes, physics, beta)
}

/// Build TBEM system with custom Burton-Miller coupling parameter
pub fn build_tbem_system_with_beta(
    elements: &[Element],
    nodes: &Array2<f64>,
    physics: &PhysicsParams,
    beta: Complex64,
) -> TbemSystem {
    let num_dofs = count_dofs(elements);
    let mut system = TbemSystem::new(num_dofs);

    let gamma = Complex64::new(physics.gamma(), 0.0);
    let tau = Complex64::new(physics.tau, 0.0);

    // Calculate approximate ka based on mesh bounding box
    let mut avg_radius = 0.0;
    let n_calc = elements.len().min(100);
    for element in elements.iter().take(n_calc) {
        let r = element.center.dot(&element.center).sqrt();
        avg_radius += r;
    }
    if n_calc > 0 {
        avg_radius /= n_calc as f64;
    }
    let ka = physics.wave_number * avg_radius;

    // Switch between formulations based on frequency
    // Low freq (ka < 0.5): Use Modified formulation (+K) for stability
    // High freq (ka >= 0.5): Use Standard formulation (-K) for accuracy
    let dg_dn_sign = if ka < 0.5 { 1.0 } else { -1.0 };

    // Loop over source elements
    for (iel, source_elem) in elements.iter().enumerate() {
        // Skip evaluation elements (property == 2)
        if source_elem.property.is_evaluation() {
            continue;
        }

        let source_point = &source_elem.center;
        let source_normal = &source_elem.normal;
        let source_dof = source_elem.dof_addresses[0];

        // Get boundary condition type and value
        let (bc_type, bc_value) = get_bc_type_and_value(&source_elem.boundary_condition);

        // Add diagonal free terms (jump terms from BIE)
        add_free_terms(
            &mut system,
            source_dof,
            bc_type,
            &bc_value,
            gamma,
            tau,
            beta,
        );

        // Loop over field elements
        for (jel, field_elem) in elements.iter().enumerate() {
            // Skip evaluation elements
            if field_elem.property.is_evaluation() {
                continue;
            }

            // Get field element coordinates
            let element_coords = get_element_coords(field_elem, nodes);
            let field_dof = field_elem.dof_addresses[0];

            // Get field BC for RHS contribution
            let (field_bc_type, field_bc_values) =
                get_bc_type_and_value(&field_elem.boundary_condition);
            let compute_rhs = has_nonzero_bc(&field_bc_values);

            // Compute integrals
            let mut result = if jel == iel {
                // Singular integration (self-element)
                singular_integration(
                    source_point,
                    source_normal,
                    &element_coords,
                    field_elem.element_type,
                    physics,
                    if compute_rhs {
                        Some(&field_bc_values)
                    } else {
                        None
                    },
                    field_bc_type,
                    compute_rhs,
                )
            } else {
                // Regular integration
                regular_integration(
                    source_point,
                    source_normal,
                    &element_coords,
                    field_elem.element_type,
                    field_elem.area,
                    physics,
                    if compute_rhs {
                        Some(&field_bc_values)
                    } else {
                        None
                    },
                    field_bc_type,
                    compute_rhs,
                )
            };

            // Apply sign switch for stability/accuracy trade-off
            result.dg_dn_integral *= dg_dn_sign;

            // Assemble contributions to matrix and RHS
            assemble_tbem(
                &mut system,
                source_dof,
                field_dof,
                bc_type,
                field_bc_type,
                &result,
                gamma,
                tau,
                beta,
                compute_rhs,
            );
        }
    }

    system
}

/// Count total number of DOFs
fn count_dofs(elements: &[Element]) -> usize {
    elements
        .iter()
        .filter(|e| !e.property.is_evaluation())
        .map(|e| e.dof_addresses.len())
        .sum()
}

/// Get boundary condition type (0=velocity, 1=pressure) and values
fn get_bc_type_and_value(bc: &BoundaryCondition) -> (i32, Vec<Complex64>) {
    match bc {
        BoundaryCondition::Velocity(v) => (0, v.clone()),
        BoundaryCondition::Pressure(p) => (1, p.clone()),
        BoundaryCondition::VelocityWithAdmittance { velocity, .. } => (0, velocity.clone()),
        BoundaryCondition::TransferAdmittance { .. } => (2, vec![Complex64::new(0.0, 0.0)]),
        BoundaryCondition::TransferWithSurfaceAdmittance { .. } => {
            (2, vec![Complex64::new(0.0, 0.0)])
        }
    }
}

/// Check if BC values are non-zero
fn has_nonzero_bc(values: &[Complex64]) -> bool {
    values.iter().any(|v| v.norm() > 1e-15)
}

/// Get element node coordinates as Array2
fn get_element_coords(element: &Element, nodes: &Array2<f64>) -> Array2<f64> {
    let num_nodes = element.connectivity.len();
    let mut coords = Array2::zeros((num_nodes, 3));

    for (i, &node_idx) in element.connectivity.iter().enumerate() {
        for j in 0..3 {
            coords[[i, j]] = nodes[[node_idx, j]];
        }
    }

    coords
}

/// Add diagonal free terms from jump conditions
///
/// From NC_ComputeEntriesForTBEM in C++:
/// - For velocity BC: diagonal += (Admia3*zBta3 - Gama3)*0.5
///   Without admittance: diagonal += -γ/2
/// - For pressure BC: diagonal -= β*τ/2
///
/// This matches the classic Burton-Miller formulation for exterior problems.
fn add_free_terms(
    system: &mut TbemSystem,
    source_dof: usize,
    bc_type: i32,
    bc_value: &[Complex64],
    gamma: Complex64,
    tau: Complex64,
    beta: Complex64,
) {
    let avg_bc = bc_value.iter().sum::<Complex64>() / bc_value.len() as f64;

    match bc_type {
        0 => {
            // Velocity BC: diagonal term from CBIE jump
            // Using c = -0.5 (matching C++ NumCalc) with negated K' gives best results
            system.matrix[[source_dof, source_dof]] -= gamma * 0.5;
            // RHS contribution from velocity BC (from NC_ComputeEntriesForTBEM line 239)
            // z0 += Zbvi03*zBta3*Tao_*0.5
            system.rhs[source_dof] += avg_bc * beta * tau * 0.5;
        }
        1 => {
            // Pressure BC: diagonal term from HBIE jump
            // C++: zcoefl[diagonal] -= zBta3*(0.5*Tao_)
            system.matrix[[source_dof, source_dof]] -= beta * tau * 0.5;
            // RHS contribution from pressure BC
            system.rhs[source_dof] += avg_bc * tau * 0.5;
        }
        _ => {
            // Transfer admittance - more complex handling
        }
    }
}

/// Assemble element contributions to matrix and RHS
///
/// The Burton-Miller formulation gives:
/// - For velocity BC on field element: coeff += (γτH + βE)
/// - For pressure BC on field element: coeff += (-γτG - βH^T)
fn assemble_tbem(
    system: &mut TbemSystem,
    source_dof: usize,
    field_dof: usize,
    _source_bc_type: i32,
    field_bc_type: i32,
    result: &IntegrationResult,
    gamma: Complex64,
    tau: Complex64,
    beta: Complex64,
    compute_rhs: bool,
) {
    let coeff = match field_bc_type {
        0 => {
            // Velocity BC (Neumann): unknown is total surface pressure p
            // DIRECT formulation using Kirchhoff-Helmholtz representation
            // Burton-Miller: (c + K' + βH)[p] = p_inc + β*∂p_inc/∂n
            // K' = ∫ ∂G/∂n_y dS = dg_dn_integral (double-layer transpose)
            // H = ∫ ∂²G/(∂n_x∂n_y) dS = d2g_dnxdny_integral (hypersingular)
            result.dg_dn_integral * gamma * tau + result.d2g_dnxdny_integral * beta
        }
        1 => {
            // Pressure BC (Dirichlet): unknown is surface velocity v
            // Uses G and H^T kernels
            -(result.g_integral * gamma * tau + result.dg_dnx_integral * beta)
        }
        _ => Complex64::new(0.0, 0.0),
    };

    system.matrix[[source_dof, field_dof]] += coeff;

    if compute_rhs {
        system.rhs[source_dof] += result.rhs_contribution;
    }
}

/// Build TBEM system in parallel using rayon
#[cfg(feature = "parallel")]
pub fn build_tbem_system_parallel(
    elements: &[Element],
    nodes: &Array2<f64>,
    physics: &PhysicsParams,
) -> TbemSystem {
    use rayon::prelude::*;
    use std::sync::Mutex;

    let num_dofs = count_dofs(elements);
    let system = Mutex::new(TbemSystem::new(num_dofs));

    let gamma = Complex64::new(physics.gamma(), 0.0);
    let tau = Complex64::new(physics.tau, 0.0);
    let beta = physics.burton_miller_beta();

    // Heuristic for formulation switch
    // Calculate approximate ka based on mesh bounding box
    let mut avg_radius = 0.0;
    // We can't access elements[i] easily before loop? Yes we can.
    let n_calc = elements.len().min(100);
    for i in 0..n_calc {
        let r = elements[i].center.dot(&elements[i].center).sqrt();
        avg_radius += r;
    }
    if n_calc > 0 {
        avg_radius /= n_calc as f64;
    }
    let ka = physics.wave_number * avg_radius;

    // Switch between formulations based on frequency
    let dg_dn_sign = if ka < 0.5 { 1.0 } else { -1.0 };

    // Process source elements in parallel
    elements
        .par_iter()
        .enumerate()
        .for_each(|(iel, source_elem)| {
            if source_elem.property.is_evaluation() {
                return;
            }

            let source_point = &source_elem.center;
            let source_normal = &source_elem.normal;
            let source_dof = source_elem.dof_addresses[0];
            let (bc_type, bc_value) = get_bc_type_and_value(&source_elem.boundary_condition);

            // Compute row locally
            let mut local_row = Array1::<Complex64>::zeros(num_dofs);
            let mut local_rhs = Complex64::new(0.0, 0.0);

            // Add free terms (c = +1/2 for exterior problem)
            let avg_bc = bc_value.iter().sum::<Complex64>() / bc_value.len() as f64;
            match bc_type {
                0 => {
                    local_row[source_dof] += gamma * 0.5;
                    local_rhs += avg_bc * beta * tau * 0.5;
                }
                1 => {
                    local_row[source_dof] += beta * tau * 0.5;
                    local_rhs += avg_bc * tau * 0.5;
                }
                _ => {}
            }

            // Loop over field elements
            for (jel, field_elem) in elements.iter().enumerate() {
                if field_elem.property.is_evaluation() {
                    continue;
                }

                let element_coords = get_element_coords(field_elem, nodes);
                let field_dof = field_elem.dof_addresses[0];
                let (field_bc_type, field_bc_values) =
                    get_bc_type_and_value(&field_elem.boundary_condition);
                let compute_rhs = has_nonzero_bc(&field_bc_values);

                let mut result = if jel == iel {
                    singular_integration(
                        source_point,
                        source_normal,
                        &element_coords,
                        field_elem.element_type,
                        physics,
                        if compute_rhs {
                            Some(&field_bc_values)
                        } else {
                            None
                        },
                        field_bc_type,
                        compute_rhs,
                    )
                } else {
                    regular_integration(
                        source_point,
                        source_normal,
                        &element_coords,
                        field_elem.element_type,
                        field_elem.area,
                        physics,
                        if compute_rhs {
                            Some(&field_bc_values)
                        } else {
                            None
                        },
                        field_bc_type,
                        compute_rhs,
                    )
                };

                // Apply sign switch
                result.dg_dn_integral *= dg_dn_sign;

                let coeff = match field_bc_type {
                    // Velocity BC: K' + βH for direct formulation
                    0 => result.dg_dn_integral * gamma * tau + result.d2g_dnxdny_integral * beta,
                    // Pressure BC: G + βH^T for direct formulation
                    1 => -(result.g_integral * gamma * tau + result.dg_dnx_integral * beta),
                    _ => Complex64::new(0.0, 0.0),
                };

                local_row[field_dof] += coeff;

                if compute_rhs {
                    local_rhs += result.rhs_contribution;
                }
            }

            // Write row to global system
            let mut sys = system.lock().unwrap();
            for j in 0..num_dofs {
                sys.matrix[[source_dof, j]] += local_row[j];
            }
            sys.rhs[source_dof] += local_rhs;
        });

    system.into_inner().unwrap()
}

/// Apply row sum correction to enforce the theoretical row sum property
///
/// For a closed surface with exterior Burton-Miller formulation:
/// - Row sum should be approximately 0 (c + K[1] + β*E[1] ≈ 0.5 - 0.5 + 0 = 0)
///
/// Due to numerical integration errors in E[1], the computed row sum may be nonzero.
/// This correction subtracts the row sum from the diagonal to enforce the property.
///
/// # Arguments
/// * `system` - The assembled TBEM system (modified in place)
///
/// # Returns
/// The average row sum magnitude before correction (for diagnostics)
pub fn apply_row_sum_correction(system: &mut TbemSystem) -> f64 {
    let n = system.num_dofs;
    let mut max_row_sum_norm = 0.0f64;
    let mut total_row_sum = Complex64::new(0.0, 0.0);

    for i in 0..n {
        // Compute row sum
        let mut row_sum = Complex64::new(0.0, 0.0);
        for j in 0..n {
            row_sum += system.matrix[[i, j]];
        }

        total_row_sum += row_sum;
        max_row_sum_norm = max_row_sum_norm.max(row_sum.norm());

        // Correct diagonal to make row sum zero
        system.matrix[[i, i]] -= row_sum;
    }

    total_row_sum.norm() / n as f64
}

/// Build TBEM system with row sum correction
///
/// This is recommended for closed surface scattering problems where
/// the E[1] = 0 property should hold but numerical errors cause drift.
pub fn build_tbem_system_corrected(
    elements: &[Element],
    nodes: &Array2<f64>,
    physics: &PhysicsParams,
) -> (TbemSystem, f64) {
    let mut system = build_tbem_system(elements, nodes, physics);
    let correction = apply_row_sum_correction(&mut system);
    (system, correction)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::types::{ElementProperty, ElementType};
    use ndarray::array;

    fn make_simple_mesh() -> (Vec<Element>, Array2<f64>) {
        // Simple 2-element mesh for testing
        let nodes = Array2::from_shape_vec(
            (4, 3),
            vec![
                0.0, 0.0, 0.0, // node 0
                1.0, 0.0, 0.0, // node 1
                0.5, 1.0, 0.0, // node 2
                1.5, 1.0, 0.0, // node 3
            ],
        )
        .unwrap();

        let elem0 = Element {
            connectivity: vec![0, 1, 2],
            element_type: ElementType::Tri3,
            property: ElementProperty::Surface,
            normal: array![0.0, 0.0, 1.0],
            node_normals: Array2::zeros((3, 3)),
            center: array![0.5, 1.0 / 3.0, 0.0],
            area: 0.5,
            boundary_condition: BoundaryCondition::Velocity(vec![Complex64::new(1.0, 0.0)]),
            group: 0,
            dof_addresses: vec![0],
        };

        let elem1 = Element {
            connectivity: vec![1, 3, 2],
            element_type: ElementType::Tri3,
            property: ElementProperty::Surface,
            normal: array![0.0, 0.0, 1.0],
            node_normals: Array2::zeros((3, 3)),
            center: array![1.0, 2.0 / 3.0, 0.0],
            area: 0.5,
            boundary_condition: BoundaryCondition::Velocity(vec![Complex64::new(0.0, 0.0)]),
            group: 0,
            dof_addresses: vec![1],
        };

        (vec![elem0, elem1], nodes)
    }

    #[test]
    fn test_build_tbem_system() {
        let (elements, nodes) = make_simple_mesh();
        let physics = PhysicsParams::new(100.0, 343.0, 1.21, false);

        let system = build_tbem_system(&elements, &nodes, &physics);

        assert_eq!(system.num_dofs, 2);
        assert_eq!(system.matrix.shape(), &[2, 2]);
        assert_eq!(system.rhs.len(), 2);

        // Matrix should have non-zero diagonal entries
        assert!(system.matrix[[0, 0]].norm() > 1e-15);
        assert!(system.matrix[[1, 1]].norm() > 1e-15);
    }

    #[test]
    fn test_count_dofs() {
        let (elements, _) = make_simple_mesh();
        assert_eq!(count_dofs(&elements), 2);
    }

    #[test]
    fn test_get_element_coords() {
        let (elements, nodes) = make_simple_mesh();
        let coords = get_element_coords(&elements[0], &nodes);

        assert_eq!(coords.shape(), &[3, 3]);
        assert!((coords[[0, 0]] - 0.0).abs() < 1e-10); // node 0
        assert!((coords[[1, 0]] - 1.0).abs() < 1e-10); // node 1
    }
}
