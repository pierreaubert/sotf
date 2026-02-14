//! Singular integration handling
//!
//! Adaptive subdivision for nearly-singular and singular element integrals.
//! Direct port of NC_SingularIntegration from NC_EquationSystem.cpp.

use ndarray::{Array1, Array2, array};
use num_complex::Complex64;
use std::f64::consts::PI;

use crate::core::integration::gauss::gauss_legendre;
use crate::core::mesh::element::{cross_product, normalize};
use crate::core::types::{ElementType, IntegrationResult, PhysicsParams};

/// Maximum number of subelements for adaptive integration
pub const MAX_SUBELEMENTS: usize = 110;

/// Quadrature parameters for singular integration
///
/// Controls the accuracy of numerical integration. Higher values give
/// better accuracy at higher computational cost.
#[derive(Debug, Clone, Copy)]
pub struct QuadratureParams {
    /// Number of Gauss points for 1D edge integration (default: 3)
    pub edge_gauss_order: usize,
    /// Number of Gauss points per dimension for 2D subelement integration (default: 4)
    pub subelement_gauss_order: usize,
    /// Number of edge sections for hypersingular integration (default: 4)
    pub edge_sections: usize,
    /// Number of subtriangles per edge section (default: 2)
    pub subtriangles_per_section: usize,
}

impl Default for QuadratureParams {
    fn default() -> Self {
        Self {
            edge_gauss_order: 3,
            subelement_gauss_order: 4,
            edge_sections: 4,
            subtriangles_per_section: 2,
        }
    }
}

impl QuadratureParams {
    /// Create quadrature parameters optimized for the given ka (wave number × element size)
    ///
    /// Higher ka values require more integration points for accuracy.
    pub fn for_ka(ka: f64) -> Self {
        if ka < 0.3 {
            // Very low frequency: coarse quadrature is fine
            Self {
                edge_gauss_order: 3,
                subelement_gauss_order: 4,
                edge_sections: 4,
                subtriangles_per_section: 2,
            }
        } else if ka < 1.0 {
            // Low-medium frequency: increase quadrature
            Self {
                edge_gauss_order: 4,
                subelement_gauss_order: 5,
                edge_sections: 6,
                subtriangles_per_section: 2,
            }
        } else if ka < 2.0 {
            // Medium frequency: further increase
            Self {
                edge_gauss_order: 5,
                subelement_gauss_order: 6,
                edge_sections: 8,
                subtriangles_per_section: 3,
            }
        } else {
            // High frequency: maximum quadrature
            Self {
                edge_gauss_order: 6,
                subelement_gauss_order: 7,
                edge_sections: 10,
                subtriangles_per_section: 4,
            }
        }
    }

    /// High-accuracy quadrature for validation/debugging
    pub fn high_accuracy() -> Self {
        Self {
            edge_gauss_order: 8,
            subelement_gauss_order: 8,
            edge_sections: 12,
            subtriangles_per_section: 4,
        }
    }
}

/// Local coordinates for 6-point triangle subdivision (vertices + midpoints)
/// Standard vertex mapping: v0 at (0,0), v1 at (1,0), v2 at (0,1)
/// Midpoints: m3 = mid(v0,v1) at (0.5,0), m4 = mid(v1,v2) at (0.5,0.5), m5 = mid(v2,v0) at (0,0.5)
const CSI6: [f64; 6] = [0.0, 1.0, 0.0, 0.5, 0.5, 0.0];
const ETA6: [f64; 6] = [0.0, 0.0, 1.0, 0.0, 0.5, 0.5];

/// Local coordinates for 8-point quad subdivision (vertices + midpoints)
const CSI8: [f64; 8] = [1.0, -1.0, -1.0, 1.0, 0.0, -1.0, 0.0, 1.0];
const ETA8: [f64; 8] = [1.0, 1.0, -1.0, -1.0, 1.0, 0.0, -1.0, 0.0];

/// Perform singular integration for self-element
///
/// This implements NC_SingularIntegration from the C++ code.
/// Uses edge-based integration for the hypersingular kernel and
/// subelement triangulation for accurate integration near singularity.
///
/// # Arguments
/// * `source_point` - Source point (collocation point)
/// * `source_normal` - Unit normal at source point
/// * `element_coords` - Node coordinates of the field element (num_nodes × 3)
/// * `element_type` - Triangle or quad element
/// * `physics` - Physics parameters (wave number, etc.)
/// * `bc_values` - Boundary condition values at element nodes (for RHS)
/// * `bc_type` - Boundary condition type (0=velocity, 1=pressure)
/// * `compute_rhs` - Whether to compute RHS contribution
///
/// # Returns
/// IntegrationResult with G, H, H^T, E integrals and RHS contribution
pub fn singular_integration(
    source_point: &Array1<f64>,
    source_normal: &Array1<f64>,
    element_coords: &Array2<f64>,
    element_type: ElementType,
    physics: &PhysicsParams,
    bc_values: Option<&[Complex64]>,
    bc_type: i32,
    compute_rhs: bool,
) -> IntegrationResult {
    // Estimate element size for frequency-dependent quadrature
    let element_size = estimate_element_size(element_coords, element_type);
    let ka = physics.wave_number * element_size;
    let quadrature = QuadratureParams::for_ka(ka);

    singular_integration_with_params(
        source_point,
        source_normal,
        element_coords,
        element_type,
        physics,
        bc_values,
        bc_type,
        compute_rhs,
        &quadrature,
    )
}

/// Perform singular integration with explicit quadrature parameters
///
/// This version allows fine-grained control over integration accuracy.
pub fn singular_integration_with_params(
    source_point: &Array1<f64>,
    source_normal: &Array1<f64>,
    element_coords: &Array2<f64>,
    element_type: ElementType,
    physics: &PhysicsParams,
    bc_values: Option<&[Complex64]>,
    bc_type: i32,
    compute_rhs: bool,
    quadrature: &QuadratureParams,
) -> IntegrationResult {
    let num_nodes = element_type.num_nodes();
    let wavruim = physics.harmonic_factor * physics.wave_number;
    let k2 = physics.wave_number * physics.wave_number;

    let mut result = IntegrationResult::default();

    // Gauss-Legendre for edge integration
    let ngpo1 = quadrature.edge_gauss_order;
    let (coord_gau, weit_gau) = gauss_legendre(ngpo1);
    let nsec1 = quadrature.edge_sections;
    let nsec2 = quadrature.subtriangles_per_section;

    // Loop over edges of the element
    for ieg in 0..num_nodes {
        let ig1 = (ieg + 1) % num_nodes;
        let ig2 = ieg + num_nodes;

        // Compute edge length and direction
        let mut diff_poi = Array1::zeros(3);
        let mut leneg = 0.0;
        for i in 0..3 {
            diff_poi[i] = element_coords[[ig1, i]] - element_coords[[ieg, i]];
            leneg += diff_poi[i] * diff_poi[i];
        }
        leneg = leneg.sqrt();

        // Unit vector along edge
        let diff_poo = &diff_poi / leneg;
        let leneg_scaled = leneg / (2.0 * nsec1 as f64);

        // Edge integration for hypersingular kernel (E integral)
        let mut zre = Complex64::new(0.0, 0.0);

        // Each edge is divided into nsec1 sections
        let delsec = 2.0 / nsec1 as f64;
        let mut secmid = -1.0 - delsec / 2.0;

        for _isec in 0..nsec1 {
            secmid += delsec;

            // Loop over Gauss points
            for ig in 0..ngpo1 {
                let sga = secmid + coord_gau[ig] / nsec1 as f64;
                let wga = weit_gau[ig] * leneg_scaled;

                // Gauss point coordinates
                let mut crd_gp = Array1::zeros(3);
                let mut diff_fsp = Array1::zeros(3);
                for i in 0..3 {
                    crd_gp[i] = element_coords[[ieg, i]] + diff_poi[i] * (sga + 1.0) / 2.0;
                    diff_fsp[i] = crd_gp[i] - source_point[i];
                }

                // Normalize and get distance
                let (unit_r, dis_fsp) = normalize(&diff_fsp);

                if dis_fsp < 1e-15 {
                    continue;
                }

                // Green's function and gradient factor
                let re1 = wavruim * dis_fsp;
                let re2 = 4.0 * PI * dis_fsp;
                let zg = Complex64::new(re1.cos() / re2, re1.sin() / re2);

                // Factor (ik - 1/r)
                let z1 = Complex64::new(-1.0 / dis_fsp, wavruim);
                let zg_factor = zg * z1;

                // Compute ∇G × edge_direction
                let mut zdg_dy = Array1::<Complex64>::zeros(3);
                for i in 0..3 {
                    zdg_dy[i] = zg_factor * unit_r[i];
                }

                // Cross product: (∇G) × (edge direction)
                let zwk = array![
                    zdg_dy[1] * diff_poo[2] - zdg_dy[2] * diff_poo[1],
                    zdg_dy[2] * diff_poo[0] - zdg_dy[0] * diff_poo[2],
                    zdg_dy[0] * diff_poo[1] - zdg_dy[1] * diff_poo[0]
                ];

                // Dot with source normal
                zre += (zwk[0] * source_normal[0]
                    + zwk[1] * source_normal[1]
                    + zwk[2] * source_normal[2])
                    * wga;
            }
        }
        result.d2g_dnxdny_integral += zre;

        // Subelement integration for G, H, H^T kernels
        for isec in 0..nsec2 {
            // Compute subtriangle vertices in local coordinates
            let (ssub, tsub, aresub) = match element_type {
                ElementType::Tri3 => {
                    let aresub = 1.0 / 24.0 / nsec2 as f64;
                    let mut ssub = [0.0; 3];
                    let mut tsub = [0.0; 3];

                    ssub[0] = 1.0 / 3.0;
                    tsub[0] = 1.0 / 3.0;

                    if isec == 0 {
                        ssub[1] = CSI6[ieg];
                        ssub[2] = CSI6[ig2];
                        tsub[1] = ETA6[ieg];
                        tsub[2] = ETA6[ig2];
                    } else {
                        ssub[1] = CSI6[ig2];
                        ssub[2] = CSI6[ig1];
                        tsub[1] = ETA6[ig2];
                        tsub[2] = ETA6[ig1];
                    }
                    (ssub, tsub, aresub)
                }
                ElementType::Quad4 => {
                    let aresub = 0.25 / nsec2 as f64;
                    let mut ssub = [0.0; 3];
                    let mut tsub = [0.0; 3];

                    ssub[0] = 0.0;
                    tsub[0] = 0.0;

                    if isec == 0 {
                        ssub[1] = CSI8[ieg];
                        ssub[2] = CSI8[ig2];
                        tsub[1] = ETA8[ieg];
                        tsub[2] = ETA8[ig2];
                    } else {
                        ssub[1] = CSI8[ig2];
                        ssub[2] = CSI8[ig1];
                        tsub[1] = ETA8[ig2];
                        tsub[2] = ETA8[ig1];
                    }
                    (ssub, tsub, aresub)
                }
            };

            // Get quadrature points for subtriangle integration
            // The mapping uses a tensor-product domain [-1,1]² with Duffy-type transformation
            let ngausin = quadrature.subelement_gauss_order;
            let quad_points = gauss_legendre(ngausin);
            let (gau_coords, gau_weights) = quad_points;

            for (i, &sga) in gau_coords.iter().enumerate() {
                for (j, &tga) in gau_coords.iter().enumerate() {
                    let wei_gau = gau_weights[i] * gau_weights[j];

                    // Map from subtriangle to original element coordinates
                    let sgg = 0.5 * (1.0 - sga) * ssub[0]
                        + 0.25 * (1.0 + sga) * ((1.0 - tga) * ssub[1] + (1.0 + tga) * ssub[2]);
                    let tgg = 0.5 * (1.0 - sga) * tsub[0]
                        + 0.25 * (1.0 + sga) * ((1.0 - tga) * tsub[1] + (1.0 + tga) * tsub[2]);

                    // Compute shape functions and Jacobian
                    let (shape_fn, _shape_ds, _shape_dt, jacobian, el_norm, crd_poi) =
                        compute_shape_and_jacobian(element_coords, element_type, sgg, tgg);

                    let wga = wei_gau * (1.0 + sga) * aresub * jacobian;

                    // Compute kernel functions
                    let mut diff_fsp = Array1::zeros(3);
                    for i in 0..3 {
                        diff_fsp[i] = crd_poi[i] - source_point[i];
                    }

                    let (unit_r, dis_fsp) = normalize(&diff_fsp);

                    if dis_fsp < 1e-15 {
                        continue;
                    }

                    // G kernel
                    let re1 = wavruim * dis_fsp;
                    let re2 = wga / (4.0 * PI * dis_fsp);
                    let zg = Complex64::new(re1.cos() * re2, re1.sin() * re2);

                    // H and H^T kernels
                    let z1 = Complex64::new(-1.0 / dis_fsp, wavruim);
                    let zhh_base = zg * z1;
                    let re1_h = unit_r.dot(&el_norm); // (y-x)·n_y / r
                    let re2_h = -unit_r.dot(source_normal); // -(y-x)·n_x / r
                    let zhh = zhh_base * re1_h;
                    let zht = zhh_base * re2_h;

                    // Accumulate integrals
                    result.g_integral += zg;
                    result.dg_dn_integral += zhh;
                    result.dg_dnx_integral += zht;

                    // E kernel contribution
                    result.d2g_dnxdny_integral += zg * k2 * source_normal.dot(&el_norm);

                    // RHS contribution if needed
                    if compute_rhs && bc_type == 0 {
                        // Velocity BC
                        if let Some(bc) = bc_values {
                            let mut zbgao = Complex64::new(0.0, 0.0);
                            for (ii, &n) in shape_fn.iter().enumerate() {
                                if ii < bc.len() {
                                    zbgao += bc[ii] * n;
                                }
                            }
                            let gamma = physics.gamma();
                            let tau = physics.tau;
                            let beta = physics.burton_miller_beta();
                            result.rhs_contribution += (zg * gamma * tau + zht * beta) * zbgao;
                        }
                    }
                }
            }
        }
    }

    // RHS for pressure BC
    if compute_rhs
        && bc_type == 1
        && let Some(bc) = bc_values
    {
        let zbgao = bc.iter().sum::<Complex64>() / bc.len() as f64;
        let gamma = physics.gamma();
        let tau = physics.tau;
        let beta = physics.burton_miller_beta();
        result.rhs_contribution =
            -(result.dg_dn_integral * gamma * tau + result.d2g_dnxdny_integral * beta) * zbgao;
    }

    result
}

/// Compute shape functions, derivatives, Jacobian and position at local coordinates
#[allow(clippy::type_complexity)]
fn compute_shape_and_jacobian(
    element_coords: &Array2<f64>,
    element_type: ElementType,
    s: f64,
    t: f64,
) -> (Vec<f64>, Vec<f64>, Vec<f64>, f64, Array1<f64>, Array1<f64>) {
    let num_nodes = element_type.num_nodes();

    let (shape_fn, shape_ds, shape_dt) = match element_type {
        ElementType::Tri3 => {
            // Triangle area coordinates:
            // N0 = 1-s-t (vertex at (0,0))
            // N1 = s (vertex at (1,0))
            // N2 = t (vertex at (0,1))
            let shape_fn = vec![1.0 - s - t, s, t];
            let shape_ds = vec![-1.0, 1.0, 0.0];
            let shape_dt = vec![-1.0, 0.0, 1.0];
            (shape_fn, shape_ds, shape_dt)
        }
        ElementType::Quad4 => {
            // Bilinear quad
            let s1 = 0.25 * (s + 1.0);
            let s2 = 0.25 * (s - 1.0);
            let t1 = t + 1.0;
            let t2 = t - 1.0;

            let shape_fn = vec![s1 * t1, -s2 * t1, s2 * t2, -s1 * t2];
            let shape_ds = vec![
                0.25 * (t + 1.0),
                -0.25 * (t + 1.0),
                0.25 * (t - 1.0),
                -0.25 * (t - 1.0),
            ];
            let shape_dt = vec![
                0.25 * (s + 1.0),
                0.25 * (1.0 - s),
                0.25 * (s - 1.0),
                -0.25 * (s + 1.0),
            ];
            (shape_fn, shape_ds, shape_dt)
        }
    };

    // Compute global position and tangent vectors
    let mut crd_poi = Array1::zeros(3);
    let mut dx_ds = Array1::zeros(3);
    let mut dx_dt = Array1::zeros(3);

    for i in 0..num_nodes {
        for j in 0..3 {
            crd_poi[j] += shape_fn[i] * element_coords[[i, j]];
            dx_ds[j] += shape_ds[i] * element_coords[[i, j]];
            dx_dt[j] += shape_dt[i] * element_coords[[i, j]];
        }
    }

    // Normal = dx_ds × dx_dt
    let normal = cross_product(&dx_ds, &dx_dt);
    let jacobian = normal.dot(&normal).sqrt();

    let el_norm = if jacobian > 1e-15 {
        normal / jacobian
    } else {
        Array1::zeros(3)
    };

    (shape_fn, shape_ds, shape_dt, jacobian, el_norm, crd_poi)
}

/// Subelement data for adaptive integration
#[derive(Debug, Clone)]
pub struct Subelement {
    /// Center local coordinate (xi) - used for quads
    pub xi_center: f64,
    /// Center local coordinate (eta) - used for quads
    pub eta_center: f64,
    /// Dimension factor (2^-j for subdivision level j) - used for quads
    pub factor: f64,
    /// Gauss order for this subelement
    pub gauss_order: usize,
    /// Vertex local coordinates for triangular subelements (for affine mapping)
    /// Format: [(xi0, eta0), (xi1, eta1), (xi2, eta2)]
    pub tri_vertices: Option<[(f64, f64); 3]>,
}

/// Generate adaptive subelements based on source point proximity
///
/// This implements NC_GenerateSubelements from the C++ code.
/// Elements are recursively subdivided until the distance from the
/// source point to subelement center exceeds a tolerance.
///
/// # Arguments
/// * `source_point` - Source point coordinates
/// * `element_coords` - Node coordinates of the element
/// * `element_type` - Triangle or quad element
/// * `element_area` - Area of the element
///
/// # Returns
/// Vector of Subelements with their centers, scale factors, and Gauss orders
pub fn generate_subelements(
    source_point: &Array1<f64>,
    element_coords: &Array2<f64>,
    element_type: ElementType,
    element_area: f64,
) -> Vec<Subelement> {
    const MAX_NSE: usize = 60;
    const NSE: usize = 4; // Subdivision factor
    // NumCalc uses ~3.0 for quasi-singular threshold
    // Increased from 1.5 to improve near-singular integration accuracy
    const TOL_F: f64 = 3.0; // Distance tolerance factor (was 1.5)
    const GAU_MAX: usize = 7; // Maximum Gauss order
    const GAU_MIN: usize = 4; // Minimum Gauss order (was 3, increased for better accuracy)
    const GAU_ACCU: f64 = 0.0005; // Accuracy tolerance

    let num_vertices = element_type.num_nodes();
    let mut result = Vec::new();

    // Local coordinates of vertices
    let (csi_nodes, eta_nodes): (Vec<f64>, Vec<f64>) = match element_type {
        ElementType::Tri3 => (CSI6[0..3].to_vec(), ETA6[0..3].to_vec()),
        ElementType::Quad4 => (CSI8[0..4].to_vec(), ETA8[0..4].to_vec()),
    };

    // Working arrays for subelement vertices
    let mut xi_sfp: Vec<Vec<f64>> = vec![vec![0.0; num_vertices]; MAX_NSE];
    let mut et_sfp: Vec<Vec<f64>> = vec![vec![0.0; num_vertices]; MAX_NSE];

    // Initialize with original element
    xi_sfp[0][..num_vertices].copy_from_slice(&csi_nodes[..num_vertices]);
    et_sfp[0][..num_vertices].copy_from_slice(&eta_nodes[..num_vertices]);

    let mut nsfl = 1; // Number of subelements at current level
    let mut faclin = 2.0; // Relative edge length factor

    loop {
        let mut ndie = 0; // Number of elements to subdivide
        faclin *= 0.5;
        let arels = element_area * faclin * faclin;
        let nsel = nsfl;

        // Copy current level
        let xi_sep: Vec<Vec<f64>> = xi_sfp[..nsel].to_vec();
        let et_sep: Vec<Vec<f64>> = et_sfp[..nsel].to_vec();

        for idi in 0..nsel {
            // Compute center of current subelement
            let scent: f64 =
                xi_sep[idi].iter().take(num_vertices).sum::<f64>() / num_vertices as f64;
            let tcent: f64 =
                et_sep[idi].iter().take(num_vertices).sum::<f64>() / num_vertices as f64;

            // Global coordinates of center
            let crd_poip = local_to_global(element_coords, element_type, scent, tcent);

            // Distance ratio
            let dist = distance(&crd_poip, source_point);
            let ratdis = dist / arels.sqrt();

            if ratdis < TOL_F {
                // Need to subdivide
                ndie += 1;
                if ndie > 15 {
                    // Too many subdivisions, stop
                    break;
                }

                nsfl = ndie * NSE;
                let nsf0 = nsfl - NSE;

                // Generate midside points
                let mut xisp = vec![0.0; num_vertices * 2];
                let mut etsp = vec![0.0; num_vertices * 2];

                for j in 0..num_vertices {
                    let j1 = (j + 1) % num_vertices;
                    xisp[j] = xi_sep[idi][j];
                    xisp[j + num_vertices] = (xi_sep[idi][j] + xi_sep[idi][j1]) / 2.0;
                    etsp[j] = et_sep[idi][j];
                    etsp[j + num_vertices] = (et_sep[idi][j] + et_sep[idi][j1]) / 2.0;
                }

                // Generate 4 (or 3 for triangle) subelements
                for j in 0..num_vertices {
                    let nsu = nsf0 + j;
                    let j1 = j + num_vertices;
                    let j2 = if j1 > num_vertices {
                        j1 - 1
                    } else {
                        j1 + num_vertices - 1
                    };

                    match element_type {
                        ElementType::Quad4 => {
                            xi_sfp[nsu] = vec![xisp[j], xisp[j1], scent, xisp[j2]];
                            et_sfp[nsu] = vec![etsp[j], etsp[j1], tcent, etsp[j2]];
                        }
                        ElementType::Tri3 => {
                            xi_sfp[nsu] = vec![xisp[j], xisp[j1], xisp[j2]];
                            et_sfp[nsu] = vec![etsp[j], etsp[j1], etsp[j2]];

                            // Central triangle for Tri3
                            if j == num_vertices - 1 {
                                let nsu_center = nsf0 + NSE - 1;
                                xi_sfp[nsu_center] = vec![
                                    xisp[num_vertices],
                                    xisp[num_vertices + 1],
                                    xisp[num_vertices + 2],
                                ];
                                et_sfp[nsu_center] = vec![
                                    etsp[num_vertices],
                                    etsp[num_vertices + 1],
                                    etsp[num_vertices + 2],
                                ];
                            }
                        }
                    }
                }
            } else {
                // Store subelement result
                let (xi_center, eta_center, scale, tri_verts) = match element_type {
                    ElementType::Quad4 => {
                        let xc = xi_sep[idi].iter().sum::<f64>() / 4.0;
                        let ec = et_sep[idi].iter().sum::<f64>() / 4.0;
                        (xc, ec, faclin, None)
                    }
                    ElementType::Tri3 => {
                        // Store the actual triangle vertices for proper affine mapping
                        let xc = (xi_sep[idi][0] + xi_sep[idi][1] + xi_sep[idi][2]) / 3.0;
                        let ec = (et_sep[idi][0] + et_sep[idi][1] + et_sep[idi][2]) / 3.0;
                        let vertices = [
                            (xi_sep[idi][0], et_sep[idi][0]),
                            (xi_sep[idi][1], et_sep[idi][1]),
                            (xi_sep[idi][2], et_sep[idi][2]),
                        ];
                        (xc, ec, faclin, Some(vertices))
                    }
                };

                // Compute Gauss order based on distance
                let disfac = 0.5 / ratdis;
                let gauss_order = compute_gauss_order(disfac, GAU_MIN, GAU_MAX, GAU_ACCU);

                result.push(Subelement {
                    xi_center,
                    eta_center,
                    factor: scale,
                    gauss_order,
                    tri_vertices: tri_verts,
                });

                if result.len() >= MAX_SUBELEMENTS {
                    return result;
                }
            }
        }

        if ndie == 0 {
            break;
        }
    }

    result
}

/// Compute optimal Gauss order for given distance factor
fn compute_gauss_order(disfac: f64, gau_min: usize, gau_max: usize, accuracy: f64) -> usize {
    for order in gau_min..=gau_max {
        let err_g = estimate_error_g(order, disfac);
        let err_h = estimate_error_h(order, disfac);
        let err_e = estimate_error_e(order, disfac);

        if err_g < accuracy && err_h < accuracy && err_e < accuracy {
            return order;
        }
    }
    gau_max
}

/// Error estimate for G kernel (1/r) integration
fn estimate_error_g(order: usize, disfac: f64) -> f64 {
    // Empirical error formula from NC_IntegrationConstants.h
    let n = order as f64;
    (disfac / (2.0 * n + 1.0)).powi(2 * order as i32 + 1)
}

/// Error estimate for H kernel (1/r²) integration
fn estimate_error_h(order: usize, disfac: f64) -> f64 {
    let n = order as f64;
    (disfac / (2.0 * n + 1.0)).powi(2 * order as i32 + 2)
}

/// Error estimate for E kernel (1/r³) integration
fn estimate_error_e(order: usize, disfac: f64) -> f64 {
    let n = order as f64;
    (disfac / (2.0 * n + 1.0)).powi(2 * order as i32 + 3)
}

/// Convert local coordinates to global coordinates
fn local_to_global(
    element_coords: &Array2<f64>,
    element_type: ElementType,
    s: f64,
    t: f64,
) -> Array1<f64> {
    let shape_fn = match element_type {
        ElementType::Tri3 => vec![1.0 - s - t, s, t],
        ElementType::Quad4 => {
            let s1 = 0.25 * (s + 1.0);
            let s2 = 0.25 * (s - 1.0);
            let t1 = t + 1.0;
            let t2 = t - 1.0;
            vec![s1 * t1, -s2 * t1, s2 * t2, -s1 * t2]
        }
    };

    let num_nodes = element_type.num_nodes();
    let mut result = Array1::zeros(3);
    for i in 0..num_nodes {
        for j in 0..3 {
            result[j] += shape_fn[i] * element_coords[[i, j]];
        }
    }
    result
}

/// Euclidean distance between two points
fn distance(a: &Array1<f64>, b: &Array1<f64>) -> f64 {
    let diff = a - b;
    diff.dot(&diff).sqrt()
}

/// Estimate characteristic element size (average edge length)
fn estimate_element_size(element_coords: &Array2<f64>, element_type: ElementType) -> f64 {
    let num_nodes = element_type.num_nodes();
    let mut total_length = 0.0;

    for i in 0..num_nodes {
        let j = (i + 1) % num_nodes;
        let mut edge_length_sq = 0.0;
        for k in 0..3 {
            let diff = element_coords[[j, k]] - element_coords[[i, k]];
            edge_length_sq += diff * diff;
        }
        total_length += edge_length_sq.sqrt();
    }

    total_length / num_nodes as f64
}

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::Array2;

    fn make_test_triangle() -> Array2<f64> {
        Array2::from_shape_vec((3, 3), vec![0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0]).unwrap()
    }

    #[test]
    fn test_local_to_global_triangle() {
        let coords = make_test_triangle();

        // At center (1/3, 1/3)
        let center = local_to_global(&coords, ElementType::Tri3, 1.0 / 3.0, 1.0 / 3.0);
        assert!((center[0] - 1.0 / 3.0).abs() < 1e-10);
        assert!((center[1] - 1.0 / 3.0).abs() < 1e-10);
        assert!(center[2].abs() < 1e-10);
    }

    #[test]
    fn test_generate_subelements_far_point() {
        let coords = make_test_triangle();
        let source = array![10.0, 10.0, 0.0]; // Far from element

        let subelements = generate_subelements(&source, &coords, ElementType::Tri3, 0.5);

        // Far point should not require many subdivisions
        assert!(subelements.len() <= 4);
    }

    #[test]
    fn test_singular_integration_basic() {
        let coords = make_test_triangle();
        let source = array![1.0 / 3.0, 1.0 / 3.0, 0.0]; // At center
        let normal = array![0.0, 0.0, 1.0];

        // Use low frequency so kr << 1 and cos(kr) ≈ 1 (positive real part)
        let physics = PhysicsParams::new(10.0, 343.0, 1.21, false);

        let result = singular_integration(
            &source,
            &normal,
            &coords,
            ElementType::Tri3,
            &physics,
            None,
            0,
            false,
        );

        // G integral should be finite and non-zero
        assert!(result.g_integral.norm().is_finite());
        assert!(result.g_integral.norm() > 0.0, "G integral was zero");

        // At low frequency (small kr), the real part should be positive
        assert!(
            result.g_integral.re > 0.0,
            "G integral real part was: {} (expected positive at low freq)",
            result.g_integral.re
        );

        // H integral should be zero (source normal perpendicular to element)
        assert!(
            result.dg_dn_integral.norm() < 1e-10,
            "H integral should be zero, was: {:?}",
            result.dg_dn_integral
        );
    }

    #[test]
    fn test_compute_shape_and_jacobian() {
        let coords = make_test_triangle();

        let (shape, _, _, jac, normal, _pos) =
            compute_shape_and_jacobian(&coords, ElementType::Tri3, 0.5, 0.25);

        // Shape functions should sum to 1
        let sum: f64 = shape.iter().sum();
        assert!((sum - 1.0).abs() < 1e-10);

        // Jacobian should be 2 * area = 1 for this triangle
        assert!((jac - 1.0).abs() < 1e-10);

        // Normal should be (0, 0, 1)
        assert!(normal[2].abs() > 0.99);
    }
}
