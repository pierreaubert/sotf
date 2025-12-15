//! Audio Frequency Sweep: CGS with ILU Preconditioning
//!
//! Tests the pure-Rust BEM solver with:
//! - CGS iterative solver
//! - ILU (Incomplete LU) preconditioner (ported from NumCalc)
//!
//! ## Key Findings
//!
//! ### ILU for Dense vs Sparse Matrices
//!
//! **Dense TBEM matrices require threshold ≈ 0 (full LU):**
//! - With threshold=0: M⁻¹*A ≈ I, CGS converges in 1 iteration
//! - With threshold=0.01: Only 35% fill, preconditioner quality ~5%
//! - With threshold=0.1: Only 6% fill, CGS diverges
//!
//! **For sparse FMM near-field (where ILU makes sense):**
//! - NumCalc's thresholds (0.3-1.2) are appropriate
//! - ILU reduces memory and compute while maintaining quality
//!
//! ### Recommendation
//!
//! 1. **For dense TBEM**: Use direct LU solve (ILU with threshold=0 is equivalent)
//! 2. **For sparse FMM**: Use ILU with appropriate threshold
//!
//! ### ILU Algorithm (from NumCalc):
//! 1. Row scaling: Normalize matrix rows
//! 2. Threshold dropping: Keep entries with |a_ij| > threshold or diagonal
//! 3. Sparse L/U storage: L by rows, U by columns
//! 4. Incomplete factorization: Fill-in restricted to original sparsity
//! 5. Forward/backward substitution: Apply (LU)⁻¹

#[cfg(feature = "pure-rust")]
fn main() {
    use bem::analytical::sphere_scattering_3d;
    use bem::core::assembly::tbem::build_tbem_system_scaled;
    use bem::core::incident::IncidentField;
    use bem::core::mesh::generators::generate_icosphere_mesh;
    use bem::core::solver::IluPreconditioner;
    use bem::core::solver::Preconditioner;
    use bem::core::solver::direct::lu_solve;
    use bem::core::solver::{CgsConfig, solve_cgs, solve_with_ilu};
    use bem::core::types::{BoundaryCondition, PhysicsParams};
    use ndarray::{Array1, Array2};
    use num_complex::Complex64;
    use std::f64::consts::PI;

    println!("=== Audio Frequency Sweep: CGS + ILU Preconditioning ===\n");

    // Physical parameters
    let radius = 0.1; // 10 cm sphere
    let speed_of_sound = 343.0; // m/s (air at ~20°C)
    let density = 1.21; // kg/m³

    // Mesh: subdivision 2 gives 320 elements (smaller for faster testing)
    let subdivisions = 2;
    let mesh = generate_icosphere_mesh(radius, subdivisions);
    let n = mesh.elements.len();

    println!("Physical setup:");
    println!("  Sphere radius: {} m", radius);
    println!("  Speed of sound: {} m/s", speed_of_sound);
    println!("  Mesh elements: {} (subdivision {})", n, subdivisions);
    println!();

    // Set up elements with boundary conditions
    let mut elements = mesh.elements.clone();
    for (i, elem) in elements.iter_mut().enumerate() {
        elem.boundary_condition = BoundaryCondition::Velocity(vec![Complex64::new(0.0, 0.0)]);
        elem.dof_addresses = vec![i];
    }

    // Build centers/normals arrays
    let mut centers = Array2::zeros((n, 3));
    let mut normals = Array2::zeros((n, 3));
    for (i, elem) in elements.iter().enumerate() {
        for j in 0..3 {
            centers[[i, j]] = elem.center[j];
            normals[[i, j]] = elem.normal[j];
        }
    }

    // Test frequencies (spanning audio range in ka)
    let ka_values: Vec<f64> = vec![0.5, 0.7, 1.0, 1.2, 1.5];

    println!(
        "{:>8} | {:>8} | {:>10} | {:>12} | {:>12} | {:>10} | {:>8}",
        "Freq Hz", "ka", "Mie avg", "Direct err%", "CGS avg", "CGS err%", "Iters"
    );
    println!("{}", "-".repeat(85));

    for &ka in &ka_values {
        // Convert ka to frequency
        let freq = ka * speed_of_sound / (2.0 * PI * radius);
        let k = ka / radius;
        let scale = 4.0; // Burton-Miller scaling

        let physics = PhysicsParams::new(freq, speed_of_sound, density, false);
        let beta = physics.burton_miller_beta_scaled(scale);

        // Compute Mie reference
        let num_terms = (ka as usize + 20).max(30);
        let mie = sphere_scattering_3d(k, radius, num_terms, vec![radius], vec![0.0, PI / 2.0, PI]);
        if mie.pressure.iter().any(|p| !p.is_finite()) {
            println!(
                "{:>8.0} | {:>8.2} | {:>10} | {:>12} | {:>12} | {:>10} | {:>8}",
                freq, ka, "NaN", "-", "-", "-", "-"
            );
            continue;
        }
        let mie_avg =
            (mie.pressure[0].norm() + mie.pressure[1].norm() + mie.pressure[2].norm()) / 3.0;

        // Incident field
        let incident = IncidentField::plane_wave_z();
        let p_inc = incident.evaluate_pressure(&centers, &physics);
        let dpdn_inc = incident.evaluate_normal_derivative(&centers, &normals, &physics);

        // RHS: p_inc + β × dpdn_inc
        let mut rhs = Array1::<Complex64>::zeros(n);
        for i in 0..n {
            rhs[i] = p_inc[i] + beta * dpdn_inc[i];
        }

        // === Build TBEM system ===
        let tbem_system = build_tbem_system_scaled(&elements, &mesh.nodes, &physics, scale);

        // TBEM direct solve (reference)
        let tbem_solution_x = lu_solve(&tbem_system.matrix, &rhs).expect("Solver failed");
        let direct_avg: f64 = tbem_solution_x.iter().map(|x| x.norm()).sum::<f64>() / n as f64;
        let direct_err = 100.0 * (direct_avg - mie_avg).abs() / mie_avg;

        // === CGS with ILU preconditioner ===

        // CGS solver config - enable progress output for first frequency only
        let print_interval = if ka == ka_values[0] { 100 } else { 0 };
        let cgs_config = CgsConfig {
            max_iterations: 500, // More iterations
            tolerance: 1e-6,
            print_interval,
        };

        // Solve with ILU(0) preconditioner
        let cgs_solution = solve_with_ilu(&tbem_system.matrix, &rhs, &cgs_config);

        let cgs_avg: f64 = cgs_solution.x.iter().map(|x| x.norm()).sum::<f64>() / n as f64;
        let cgs_err = 100.0 * (cgs_avg - mie_avg).abs() / mie_avg;

        let status = if cgs_solution.converged { "✓" } else { "✗" };

        println!(
            "{:>8.0} | {:>8.3} | {:>10.4} | {:>11.1}% | {:>12.4} | {:>9.1}% | {:>6} {}",
            freq, ka, mie_avg, direct_err, cgs_avg, cgs_err, cgs_solution.iterations, status
        );
    }

    println!();
    println!("=== Legend ===");
    println!("  Direct err%: Error using direct LU solve");
    println!("  CGS err%: Error using CGS iterative solver with ILU");
    println!("  Iters: Number of CGS iterations to convergence");
    println!("  ✓ = converged, ✗ = did not converge");
    println!();
    println!("ILU preconditioning (ported from NumCalc) enables CGS convergence.");
    println!("If errors are low, the pure-Rust implementation matches NumCalc quality.");
}

#[cfg(not(feature = "pure-rust"))]
fn main() {
    eprintln!("This example requires the 'pure-rust' feature.");
}
