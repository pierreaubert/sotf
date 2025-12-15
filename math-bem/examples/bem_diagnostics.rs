//! BEM Diagnostics - Step-by-step investigation of high-frequency issues
//!
//! This example systematically investigates the BEM solver to find
//! where the high-frequency accuracy breaks down.
//!
//! # Usage
//! ```bash
//! cargo run --release --example bem_diagnostics --features pure-rust
//! ```

#[cfg(feature = "pure-rust")]
fn main() {
    use bem::analytical::sphere_scattering_3d;
    use bem::core::assembly::tbem::build_tbem_system;
    use bem::core::incident::IncidentField;
    use bem::core::mesh::generators::generate_sphere_mesh;
    use bem::core::solver::direct::lu_solve;
    use bem::core::types::{BoundaryCondition, PhysicsParams};
    use ndarray::{Array1, Array2};
    use num_complex::Complex64;
    use std::f64::consts::PI;

    println!("=== BEM Diagnostics: Step-by-Step Investigation ===\n");

    let radius = 0.1;
    let speed_of_sound = 343.0;
    let density = 1.21;

    // Test at different ka values
    for ka_target in [0.2, 0.5, 1.0, 2.0] {
        let frequency = ka_target * speed_of_sound / (2.0 * PI * radius);
        let k = 2.0 * PI * frequency / speed_of_sound;
        let ka = k * radius;

        println!("\n============================================================");
        println!(
            "=== ka = {:.2} (f = {:.1} Hz, k = {:.4}) ===",
            ka, frequency, k
        );
        println!("============================================================");

        // Use a moderate mesh for diagnostics
        let n_theta = 8;
        let n_phi = 16;
        let mesh = generate_sphere_mesh(radius, n_theta, n_phi);
        let physics = PhysicsParams::new(frequency, speed_of_sound, density, false);

        println!("\n--- Step 1: Incident Field RHS ---");
        investigate_incident_field(&mesh, &physics, k, radius);

        println!("\n--- Step 2: System Matrix Properties ---");
        investigate_system_matrix(&mesh, &physics);

        println!("\n--- Step 3: Solution Comparison ---");
        investigate_solution(&mesh, &physics, k, radius);
    }
}

#[cfg(feature = "pure-rust")]
fn investigate_incident_field(
    mesh: &bem::core::types::Mesh,
    physics: &bem::core::types::PhysicsParams,
    k: f64,
    radius: f64,
) {
    use bem::core::incident::IncidentField;
    use ndarray::Array2;
    use std::f64::consts::PI;

    let incident = IncidentField::plane_wave_z();

    // Collect element centers and normals
    let n = mesh.elements.len();
    let mut centers = Array2::zeros((n, 3));
    let mut normals = Array2::zeros((n, 3));

    for (i, elem) in mesh.elements.iter().enumerate() {
        for j in 0..3 {
            centers[[i, j]] = elem.center[j];
            normals[[i, j]] = elem.normal[j];
        }
    }

    // Compute incident pressure and normal derivative
    let p_inc = incident.evaluate_pressure(&centers, physics);
    let dpdn_inc = incident.evaluate_normal_derivative(&centers, &normals, physics);

    // Compute RHS (without and with Burton-Miller)
    let rhs_cbie = incident.compute_rhs(&centers, &normals, physics, false);
    let rhs_bm = incident.compute_rhs(&centers, &normals, physics, true);

    // Statistics
    let p_inc_max = p_inc.iter().map(|x| x.norm()).fold(0.0f64, f64::max);
    let p_inc_min = p_inc.iter().map(|x| x.norm()).fold(f64::INFINITY, f64::min);
    let dpdn_max = dpdn_inc.iter().map(|x| x.norm()).fold(0.0f64, f64::max);
    let dpdn_min = dpdn_inc
        .iter()
        .map(|x| x.norm())
        .fold(f64::INFINITY, f64::min);
    let rhs_cbie_max = rhs_cbie.iter().map(|x| x.norm()).fold(0.0f64, f64::max);
    let rhs_bm_max = rhs_bm.iter().map(|x| x.norm()).fold(0.0f64, f64::max);

    println!(
        "  Incident pressure |p_inc|: min={:.6}, max={:.6}",
        p_inc_min, p_inc_max
    );
    println!(
        "  Normal derivative |dp/dn|: min={:.6}, max={:.6}",
        dpdn_min, dpdn_max
    );
    println!("  RHS (CBIE only) |rhs|_max: {:.6}", rhs_cbie_max);
    println!("  RHS (Burton-Miller) |rhs|_max: {:.6}", rhs_bm_max);

    // Check a specific element (forward-facing, theta=0)
    // Find element closest to (0, 0, radius)
    let mut forward_idx = 0;
    let mut min_dist = f64::INFINITY;
    for (i, elem) in mesh.elements.iter().enumerate() {
        let dist = ((elem.center[0]).powi(2)
            + (elem.center[1]).powi(2)
            + (elem.center[2] - radius).powi(2))
        .sqrt();
        if dist < min_dist {
            min_dist = dist;
            forward_idx = i;
        }
    }

    println!("\n  Forward element (idx={}):", forward_idx);
    println!(
        "    center: ({:.4}, {:.4}, {:.4})",
        mesh.elements[forward_idx].center[0],
        mesh.elements[forward_idx].center[1],
        mesh.elements[forward_idx].center[2]
    );
    println!(
        "    normal: ({:.4}, {:.4}, {:.4})",
        mesh.elements[forward_idx].normal[0],
        mesh.elements[forward_idx].normal[1],
        mesh.elements[forward_idx].normal[2]
    );
    println!(
        "    p_inc = {:.6} + {:.6}i",
        p_inc[forward_idx].re, p_inc[forward_idx].im
    );
    println!(
        "    dp/dn = {:.6} + {:.6}i",
        dpdn_inc[forward_idx].re, dpdn_inc[forward_idx].im
    );
    println!(
        "    rhs_bm = {:.6} + {:.6}i",
        rhs_bm[forward_idx].re, rhs_bm[forward_idx].im
    );

    // Expected values for plane wave at forward point
    // p_inc = exp(ik*z) where z ≈ radius (on surface)
    let expected_p = (k * radius).cos(); // Real part of exp(ik*r) at z=r
    let expected_dpdn = -k * (k * radius).sin(); // ik * exp(ik*r) * cos(theta) where theta=0, so d·n = -1
    println!("\n  Expected (analytical):");
    println!(
        "    p_inc ≈ exp(ik*r) = cos({:.4}) = {:.6}",
        k * radius,
        expected_p
    );
    println!("    dp/dn ≈ -ik*p (at forward point where d·n = -1)");
}

#[cfg(feature = "pure-rust")]
fn investigate_system_matrix(
    mesh: &bem::core::types::Mesh,
    physics: &bem::core::types::PhysicsParams,
) {
    use bem::core::assembly::tbem::build_tbem_system;
    use bem::core::integration::singular_integration;
    use bem::core::types::BoundaryCondition;
    use ndarray::Array2;
    use num_complex::Complex64;

    // Prepare elements with velocity BC (rigid surface)
    let mut elements = mesh.elements.clone();
    for (i, elem) in elements.iter_mut().enumerate() {
        elem.boundary_condition = BoundaryCondition::Velocity(vec![Complex64::new(0.0, 0.0)]);
        elem.dof_addresses = vec![i];
    }

    // Build system
    let system = build_tbem_system(&elements, &mesh.nodes, physics);

    // Matrix statistics
    let n = system.num_dofs;
    let mut diag_magnitudes = Vec::with_capacity(n);
    let mut off_diag_magnitudes = Vec::new();

    for i in 0..n {
        diag_magnitudes.push(system.matrix[[i, i]].norm());
        for j in 0..n {
            if i != j {
                off_diag_magnitudes.push(system.matrix[[i, j]].norm());
            }
        }
    }

    let diag_max = diag_magnitudes.iter().cloned().fold(0.0f64, f64::max);
    let diag_min = diag_magnitudes
        .iter()
        .cloned()
        .fold(f64::INFINITY, f64::min);
    let diag_avg: f64 = diag_magnitudes.iter().sum::<f64>() / n as f64;

    let off_diag_max = off_diag_magnitudes.iter().cloned().fold(0.0f64, f64::max);
    let off_diag_avg: f64 =
        off_diag_magnitudes.iter().sum::<f64>() / off_diag_magnitudes.len() as f64;

    println!("  Matrix size: {} x {}", n, n);
    println!(
        "  Diagonal |A[i,i]|: min={:.6}, avg={:.6}, max={:.6}",
        diag_min, diag_avg, diag_max
    );
    println!(
        "  Off-diagonal: avg={:.6}, max={:.6}",
        off_diag_avg, off_diag_max
    );
    println!("  Diagonal dominance ratio: {:.3}", diag_avg / off_diag_avg);

    // Check Burton-Miller coupling
    let beta = physics.burton_miller_beta();
    let gamma = physics.gamma();
    let tau = physics.tau;
    println!("\n  Burton-Miller params:");
    println!("    gamma = {:.6}", gamma);
    println!(
        "    beta = {:.6} + {:.6}i (i.e., i/k where k={:.4})",
        beta.re, beta.im, physics.wave_number
    );
    println!("    tau = {:.6}", tau);

    // Expected free term: -gamma * 0.5 = -0.5 for velocity BC
    println!("    Expected diagonal free term: {:.6}", -gamma * 0.5);

    // Show first few diagonal entries
    println!("\n  First 5 diagonal entries:");
    for i in 0..5.min(n) {
        let d = system.matrix[[i, i]];
        println!(
            "    A[{},{}] = {:.6} + {:.6}i (|.| = {:.6})",
            i,
            i,
            d.re,
            d.im,
            d.norm()
        );
    }

    // === Detailed singular integration investigation ===
    println!("\n  --- Singular Integration Details (first element) ---");

    // Get first element's coordinates
    let elem = &elements[0];
    let num_nodes = elem.connectivity.len();
    let mut elem_coords = Array2::zeros((num_nodes, 3));
    for (i, &node_idx) in elem.connectivity.iter().enumerate() {
        for j in 0..3 {
            elem_coords[[i, j]] = mesh.nodes[[node_idx, j]];
        }
    }

    // Call singular integration directly
    let result = singular_integration(
        &elem.center,
        &elem.normal,
        &elem_coords,
        elem.element_type,
        physics,
        None,
        0, // velocity BC
        false,
    );

    // Estimate k² * G contribution vs edge contribution
    let k = physics.wave_number;
    let k2_g_contribution = result.g_integral * k * k;
    // Note: For n_x · n_y ≈ 1 on self-element
    let edge_e_estimate = result.d2g_dnxdny_integral - k2_g_contribution;

    println!("\n    Breakdown of E integral:");
    println!(
        "      Total E        = {:.6} + {:.6}i",
        result.d2g_dnxdny_integral.re, result.d2g_dnxdny_integral.im
    );
    println!(
        "      k²*G*(n·n)     = {:.6} + {:.6}i (should dominate at high k)",
        k2_g_contribution.re, k2_g_contribution.im
    );
    println!(
        "      Edge contrib   = {:.6} + {:.6}i (E - k²*G)",
        edge_e_estimate.re, edge_e_estimate.im
    );
    println!(
        "      Ratio |edge|/|k²G| = {:.2}",
        edge_e_estimate.norm() / k2_g_contribution.norm().max(1e-15)
    );

    println!(
        "    G integral   = {:.6} + {:.6}i (|.| = {:.6})",
        result.g_integral.re,
        result.g_integral.im,
        result.g_integral.norm()
    );
    println!(
        "    H integral   = {:.6} + {:.6}i (|.| = {:.6})",
        result.dg_dn_integral.re,
        result.dg_dn_integral.im,
        result.dg_dn_integral.norm()
    );
    println!(
        "    H^T integral = {:.6} + {:.6}i (|.| = {:.6})",
        result.dg_dnx_integral.re,
        result.dg_dnx_integral.im,
        result.dg_dnx_integral.norm()
    );
    println!(
        "    E integral   = {:.6} + {:.6}i (|.| = {:.6})",
        result.d2g_dnxdny_integral.re,
        result.d2g_dnxdny_integral.im,
        result.d2g_dnxdny_integral.norm()
    );

    // Compute the matrix coefficient from integrals
    // For velocity BC: coeff = gamma * tau * H + beta * E
    let gamma_c = Complex64::new(gamma, 0.0);
    let tau_c = Complex64::new(tau, 0.0);
    let coeff_from_integrals =
        result.dg_dn_integral * gamma_c * tau_c + result.d2g_dnxdny_integral * beta;
    let free_term = Complex64::new(-gamma * 0.5, 0.0);
    let total_diag = free_term + coeff_from_integrals;

    println!("\n    Computed diagonal contribution:");
    println!(
        "      Free term (-γ/2)     = {:.6} + {:.6}i",
        free_term.re, free_term.im
    );
    println!(
        "      γτH                  = {:.6} + {:.6}i",
        (result.dg_dn_integral * gamma_c * tau_c).re,
        (result.dg_dn_integral * gamma_c * tau_c).im
    );
    println!(
        "      βE                   = {:.6} + {:.6}i",
        (result.d2g_dnxdny_integral * beta).re,
        (result.d2g_dnxdny_integral * beta).im
    );
    println!(
        "      Total (free + γτH + βE) = {:.6} + {:.6}i",
        total_diag.re, total_diag.im
    );
    println!(
        "      Actual A[0,0]        = {:.6} + {:.6}i",
        system.matrix[[0, 0]].re,
        system.matrix[[0, 0]].im
    );

    // Scaling analysis
    let k = physics.wave_number;
    println!("\n    Scaling analysis (k = {:.4}):", k);
    println!(
        "      |G| / (1/k)    = {:.6} (expect O(1))",
        result.g_integral.norm() * k
    );
    println!(
        "      |H| / k        = {:.6} (expect O(1) for curved surface)",
        result.dg_dn_integral.norm() / k
    );
    println!(
        "      |E| / k²       = {:.6} (expect O(1))",
        result.d2g_dnxdny_integral.norm() / (k * k)
    );
    println!(
        "      |βE|           = {:.6} (expect O(k))",
        (result.d2g_dnxdny_integral * beta).norm()
    );

    // Show RHS
    let rhs_max = system.rhs.iter().map(|x| x.norm()).fold(0.0f64, f64::max);
    println!("\n  System RHS (from assembly): |rhs|_max = {:.6}", rhs_max);
}

#[cfg(feature = "pure-rust")]
fn investigate_solution(
    mesh: &bem::core::types::Mesh,
    physics: &bem::core::types::PhysicsParams,
    k: f64,
    radius: f64,
) {
    use bem::analytical::sphere_scattering_3d;
    use bem::core::assembly::tbem::{build_tbem_system, build_tbem_system_with_beta};
    use bem::core::incident::IncidentField;
    use bem::core::solver::direct::lu_solve;
    use bem::core::types::BoundaryCondition;
    use ndarray::{Array1, Array2};
    use num_complex::Complex64;
    use std::f64::consts::PI;

    // Prepare elements with velocity BC
    let mut elements = mesh.elements.clone();
    for (i, elem) in elements.iter_mut().enumerate() {
        elem.boundary_condition = BoundaryCondition::Velocity(vec![Complex64::new(0.0, 0.0)]);
        elem.dof_addresses = vec![i];
    }

    // Compute floored beta based on edge E magnitude (~70 for this mesh)
    let edge_e_magnitude = 70.0; // Typical value from diagnostics
    let min_beta_e = 10.0; // Keep |β*E| above this threshold
    let beta_floored = physics.burton_miller_beta_floored(edge_e_magnitude, min_beta_e);

    // Build systems with both beta formulations
    let system_traditional = build_tbem_system(&elements, &mesh.nodes, physics);

    // Build with floored beta
    let system_floored = build_tbem_system_with_beta(&elements, &mesh.nodes, physics, beta_floored);

    // Add incident field RHS
    let incident = IncidentField::plane_wave_z();
    let n = elements.len();
    let mut centers = Array2::zeros((n, 3));
    let mut normals = Array2::zeros((n, 3));
    for (i, elem) in elements.iter().enumerate() {
        for j in 0..3 {
            centers[[i, j]] = elem.center[j];
            normals[[i, j]] = elem.normal[j];
        }
    }

    // RHS needs to match the beta formulation used
    // Traditional: β = i/k
    let incident_rhs_trad = incident.compute_rhs(&centers, &normals, physics, true);
    let total_rhs_trad = &system_traditional.rhs + &incident_rhs_trad;

    // Floored: use consistent beta in RHS
    let incident_rhs_floored =
        incident.compute_rhs_with_beta(&centers, &normals, physics, beta_floored);
    let total_rhs_floored = &system_floored.rhs + &incident_rhs_floored;

    // Also test CBIE-only (β=0) to see if hypersingular coupling is the issue
    let beta_zero = Complex64::new(0.0, 0.0);
    let system_cbie = build_tbem_system_with_beta(&elements, &mesh.nodes, physics, beta_zero);
    let incident_rhs_cbie = incident.compute_rhs_with_beta(&centers, &normals, physics, beta_zero);
    let total_rhs_cbie = &system_cbie.rhs + &incident_rhs_cbie;

    // Solve all systems
    let solution_trad =
        lu_solve(&system_traditional.matrix, &total_rhs_trad).expect("Solver failed");
    let solution_floored =
        lu_solve(&system_floored.matrix, &total_rhs_floored).expect("Solver failed");
    let solution_cbie = lu_solve(&system_cbie.matrix, &total_rhs_cbie).expect("Solver failed");

    // Traditional solution statistics
    let p_max_trad = solution_trad
        .iter()
        .map(|x| x.norm())
        .fold(0.0f64, f64::max);
    let p_avg_trad: f64 = solution_trad.iter().map(|x| x.norm()).sum::<f64>() / n as f64;

    // Floored solution statistics
    let p_max_floored = solution_floored
        .iter()
        .map(|x| x.norm())
        .fold(0.0f64, f64::max);
    let p_avg_floored: f64 = solution_floored.iter().map(|x| x.norm()).sum::<f64>() / n as f64;

    // CBIE-only solution statistics
    let p_max_cbie = solution_cbie
        .iter()
        .map(|x| x.norm())
        .fold(0.0f64, f64::max);
    let p_avg_cbie: f64 = solution_cbie.iter().map(|x| x.norm()).sum::<f64>() / n as f64;

    println!("  CBIE-only (β=0, no hypersingular):");
    println!(
        "    Surface |p|: avg={:.6}, max={:.6}",
        p_avg_cbie, p_max_cbie
    );
    println!(
        "    Diagonal A[0,0] = {:.6} + {:.6}i",
        system_cbie.matrix[[0, 0]].re,
        system_cbie.matrix[[0, 0]].im
    );

    println!("\n  Traditional Burton-Miller β=i/k:");
    println!(
        "    β = {:.6}i, |β*E| ≈ {:.1}",
        physics.burton_miller_beta().im,
        physics.burton_miller_beta().im * edge_e_magnitude
    );
    println!(
        "    Surface |p|: avg={:.6}, max={:.6}",
        p_avg_trad, p_max_trad
    );
    println!(
        "    Diagonal A[0,0] = {:.6} + {:.6}i",
        system_traditional.matrix[[0, 0]].re,
        system_traditional.matrix[[0, 0]].im
    );

    println!(
        "\n  Floored β = max(i/k, i*{:.4}) to keep |β*E| ≥ {:.0}:",
        min_beta_e / edge_e_magnitude,
        min_beta_e
    );
    println!(
        "    β = {:.6}i, |β*E| ≈ {:.1}",
        beta_floored.im,
        beta_floored.im * edge_e_magnitude
    );
    println!(
        "    Surface |p|: avg={:.6}, max={:.6}",
        p_avg_floored, p_max_floored
    );
    println!(
        "    Diagonal A[0,0] = {:.6} + {:.6}i",
        system_floored.matrix[[0, 0]].re,
        system_floored.matrix[[0, 0]].im
    );

    // Analytical reference at surface
    let analytical_surface = sphere_scattering_3d(k, radius, 50, vec![radius], vec![0.0]);
    let analytical_p_surface = analytical_surface.pressure[0].norm();
    println!(
        "\n  Analytical surface |p| at forward point: {:.6}",
        analytical_p_surface
    );
    println!("  (Expected range for rigid sphere: ~1.5 to 2.5 depending on ka)");
}

#[cfg(not(feature = "pure-rust"))]
fn main() {
    eprintln!("This example requires the 'pure-rust' feature.");
    eprintln!("Run with: cargo run --example bem_diagnostics --features pure-rust");
}
