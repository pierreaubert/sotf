//! FMM + ILU Solver for Large-Scale BEM
//!
//! Demonstrates the combination of:
//! - **Fast Multipole Method (FMM)** for O(N log N) matvec
//! - **ILU preconditioning** for CGS convergence
//! - **Adaptive β tuning** for wide frequency range
//!
//! ## Key Points
//!
//! ### ILU Threshold Selection
//!
//! ILU threshold depends on matrix sparsity:
//! - **Dense TBEM**: threshold ≈ 0 (full LU needed)
//! - **SLFMM near-field**: threshold 0.3-0.9 (sparse)
//! - **MLFMM near-field**: threshold 0.05-0.65 (very sparse)
//!
//! ### When to Use FMM vs Direct
//!
//! | Mesh Size | Method | Reason |
//! |-----------|--------|--------|
//! | < 500 | Direct LU | O(N³) is still fast |
//! | 500-5000 | SLFMM + ILU | O(N² log N) |
//! | > 5000 | MLFMM + ILU | O(N log N) |
//!
//! ### Adaptive β + FMM
//!
//! The adaptive β from `physics.burton_miller_beta_adaptive(radius)` works
//! with both TBEM and FMM assembly methods.

#[cfg(feature = "pure-rust")]
fn main() {
    use math_audio_bem::analytical::sphere_scattering_3d;
    use math_audio_bem::core::assembly::mlfmm::{build_cluster_tree, build_mlfmm_system};
    use math_audio_bem::core::assembly::tbem::build_tbem_system_scaled;
    use math_audio_bem::core::incident::IncidentField;
    use math_audio_bem::core::mesh::generators::generate_icosphere_mesh;
    use math_audio_bem::core::solver::IluPreconditioner;
    use math_audio_bem::core::solver::Preconditioner;
    use math_audio_bem::core::solver::direct::lu_solve;
    use math_audio_bem::core::solver::{CgsConfig, solve_cgs, solve_with_ilu};
    use math_audio_bem::core::types::{BoundaryCondition, PhysicsParams};
    use ndarray::{Array1, Array2};
    use num_complex::Complex64;
    use std::f64::consts::PI;

    println!("=== FMM + ILU Solver for Large-Scale BEM ===\n");

    // Physical parameters
    let radius = 0.1;
    let speed_of_sound = 343.0;
    let density = 1.21;

    // Test different mesh sizes
    let mesh_configs = vec![(2, "Small (320 elements)"), (3, "Medium (1280 elements)")];

    // Test frequency (ka = 1.0 for best accuracy)
    let ka = 1.0;
    let freq = ka * speed_of_sound / (2.0 * PI * radius);
    let k = ka / radius;

    println!("Test frequency: {:.0} Hz (ka = {:.1})\n", freq, ka);

    // Mie reference
    let num_terms = 50;
    let mie = sphere_scattering_3d(k, radius, num_terms, vec![radius], vec![0.0, PI / 2.0, PI]);
    let mie_avg = (mie.pressure[0].norm() + mie.pressure[1].norm() + mie.pressure[2].norm()) / 3.0;

    println!("Mie analytical reference: {:.4}\n", mie_avg);

    for (subdivisions, desc) in &mesh_configs {
        println!("=== {} ===", desc);

        let mesh = generate_icosphere_mesh(radius, *subdivisions);
        let n = mesh.elements.len();

        println!("  Mesh elements: {}", n);

        // Prepare elements
        let mut elements = mesh.elements.clone();
        for (i, elem) in elements.iter_mut().enumerate() {
            elem.boundary_condition = BoundaryCondition::Velocity(vec![Complex64::new(0.0, 0.0)]);
            elem.dof_addresses = vec![i];
        }

        // Build centers/normals
        let mut centers = Array2::zeros((n, 3));
        let mut normals = Array2::zeros((n, 3));
        for (i, elem) in elements.iter().enumerate() {
            for j in 0..3 {
                centers[[i, j]] = elem.center[j];
                normals[[i, j]] = elem.normal[j];
            }
        }

        let physics = PhysicsParams::new(freq, speed_of_sound, density, false);

        // Use adaptive β
        let (beta, scale) = physics.burton_miller_beta_adaptive(radius);
        println!("  Adaptive β scale: {:.1}", scale);

        // Incident field
        let incident = IncidentField::plane_wave_z();
        let p_inc = incident.evaluate_pressure(&centers, &physics);
        let dpdn_inc = incident.evaluate_normal_derivative(&centers, &normals, &physics);

        // RHS
        let mut rhs = Array1::<Complex64>::zeros(n);
        for i in 0..n {
            rhs[i] = p_inc[i] + beta * dpdn_inc[i];
        }

        // === Method 1: Direct TBEM solve ===
        println!("\n  --- Direct TBEM Solve ---");
        let tbem_start = std::time::Instant::now();
        let tbem_system = build_tbem_system_scaled(&elements, &mesh.nodes, &physics, scale);
        let direct_x = lu_solve(&tbem_system.matrix, &rhs).expect("Direct solve failed");
        let tbem_time = tbem_start.elapsed();

        let direct_avg: f64 = direct_x.iter().map(|x| x.norm()).sum::<f64>() / n as f64;
        let direct_err = 100.0 * (direct_avg - mie_avg).abs() / mie_avg;
        println!("    Time: {:?}", tbem_time);
        println!("    BEM avg: {:.4}, Error: {:.1}%", direct_avg, direct_err);

        // === Method 2: TBEM + ILU + CGS ===
        println!("\n  --- TBEM + ILU + CGS ---");
        let ilu_start = std::time::Instant::now();

        // For dense TBEM, use very low threshold (or 0 for full LU)
        // Note: solve_with_ilu uses ILU(0) from math-solvers

        let cgs_config = CgsConfig {
            max_iterations: 200,
            tolerance: 1e-6,
            print_interval: 0,
        };

        let cgs_solution = solve_with_ilu(&tbem_system.matrix, &rhs, &cgs_config);
        let ilu_time = ilu_start.elapsed();

        let cgs_avg: f64 = cgs_solution.x.iter().map(|x| x.norm()).sum::<f64>() / n as f64;
        let cgs_err = 100.0 * (cgs_avg - mie_avg).abs() / mie_avg;
        let status = if cgs_solution.converged { "✓" } else { "✗" };

        println!("    Time: {:?}", ilu_time);
        println!("    Iterations: {} {}", cgs_solution.iterations, status);
        println!("    BEM avg: {:.4}, Error: {:.1}%", cgs_avg, cgs_err);

        // === Method 3: MLFMM (structure only - full solve needs ILU on near-field) ===
        println!("\n  --- MLFMM Structure ---");
        let fmm_start = std::time::Instant::now();
        let cluster_tree = build_cluster_tree(&elements, 20, &physics);
        let mlfmm_system = build_mlfmm_system(&elements, &mesh.nodes, cluster_tree, &physics);
        let fmm_build_time = fmm_start.elapsed();

        println!("    Build time: {:?}", fmm_build_time);
        println!("    Cluster levels: {}", mlfmm_system.num_levels);
        for level in 0..mlfmm_system.num_levels {
            println!(
                "      Level {}: {} clusters",
                level,
                mlfmm_system.num_clusters_at_level(level)
            );
        }

        // Test FMM matvec
        let test_x: Array1<Complex64> =
            Array1::from_iter((0..n).map(|i| Complex64::new((i as f64).sin(), (i as f64).cos())));

        let fmm_matvec_start = std::time::Instant::now();
        let _fmm_y = mlfmm_system.matvec(&test_x);
        let fmm_matvec_time = fmm_matvec_start.elapsed();

        let tbem_matvec_start = std::time::Instant::now();
        let _tbem_y = tbem_system.matrix.dot(&test_x);
        let tbem_matvec_time = tbem_matvec_start.elapsed();

        println!("    FMM matvec: {:?}", fmm_matvec_time);
        println!("    TBEM matvec: {:?}", tbem_matvec_time);

        if fmm_matvec_time < tbem_matvec_time {
            println!(
                "    FMM is {:.1}x faster for matvec",
                tbem_matvec_time.as_secs_f64() / fmm_matvec_time.as_secs_f64()
            );
        }

        println!();
    }

    println!("=== Summary ===\n");
    println!("For production use with large meshes (> 1000 elements):");
    println!("  1. Use MLFMM for O(N log N) matvec");
    println!("  2. Build ILU on near-field matrix (sparse) with threshold 0.1-0.5");
    println!("  3. Use CGS with preconditioned system");
    println!("  4. Use adaptive β for wide frequency range");
    println!();
    println!("For small meshes (< 500 elements):");
    println!("  Direct LU solve is faster and simpler");
}

#[cfg(not(feature = "pure-rust"))]
fn main() {
    eprintln!("This example requires the 'pure-rust' feature.");
}
