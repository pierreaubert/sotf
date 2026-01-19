//! Simple sphere test to debug BEM accuracy
//!
//! Tests a rigid sphere scattering problem and compares with analytical Mie solution.
//! Helps isolate accuracy issues in the BEM implementation.

use math_audio_bem::analytical::sphere_scattering_3d;
use math_audio_bem::core::assembly::tbem::build_tbem_system_with_beta;
use math_audio_bem::core::incident::IncidentField;
use math_audio_bem::core::mesh::generators::generate_icosphere_mesh;
use math_audio_bem::core::solver::direct::lu_solve;
use math_audio_bem::core::types::{BoundaryCondition, PhysicsParams};
use ndarray::Array2;
use num_complex::Complex64;
use std::f64::consts::PI;

fn main() {
    // Problem parameters
    let radius = 0.1;
    let speed_of_sound = 343.0;
    let density = 1.21;

    // Test at ka = 1.0 (Mie regime)
    let ka = 1.0;
    let k = ka / radius;
    let frequency = k * speed_of_sound / (2.0 * PI);

    println!("=== Simple Sphere BEM Test ===");
    println!("Radius: {} m", radius);
    println!("Frequency: {:.1} Hz", frequency);
    println!("Wave number k: {:.4} rad/m", k);
    println!("ka: {:.2}", ka);
    println!();

    // Create mesh (subdivisions = 2 for ~80 elements)
    let mesh = generate_icosphere_mesh(radius, 2);
    let n_elements = mesh.elements.len();
    println!(
        "Mesh: {} elements, {} nodes",
        n_elements,
        mesh.nodes.nrows()
    );

    // Set up physics
    let physics = PhysicsParams::new(frequency, speed_of_sound, density, false);
    let beta = physics.burton_miller_beta();
    println!("Burton-Miller beta: {:.4} + {:.4}i", beta.re, beta.im);
    println!();

    // Prepare elements with rigid BC (velocity = 0)
    let mut elements = mesh.elements.clone();
    for (i, elem) in elements.iter_mut().enumerate() {
        elem.boundary_condition = BoundaryCondition::Velocity(vec![Complex64::new(0.0, 0.0)]);
        elem.dof_addresses = vec![i];
    }

    // Build system matrix with scaled Burton-Miller beta
    // Empirically, β_scale ≈ 3-4 gives best accuracy at ka=1
    println!("Assembling TBEM system...");
    let (beta, beta_scale) = physics.burton_miller_beta_adaptive(radius);
    println!(
        "Using adaptive β (scale={:.1}): {:.4} + {:.4}i",
        beta_scale, beta.re, beta.im
    );
    let system = build_tbem_system_with_beta(&elements, &mesh.nodes, &physics, beta);

    println!("Matrix assembled: {}x{}", system.num_dofs, system.num_dofs);

    // Check matrix properties
    let mut diag_sum = Complex64::new(0.0, 0.0);
    for i in 0..system.num_dofs {
        diag_sum += system.matrix[[i, i]];
    }
    println!("Diagonal sum: {:.6} + {:.6}i", diag_sum.re, diag_sum.im);

    // Check row sums (should be ~0 for closed surface)
    let mut max_row_sum = 0.0_f64;
    let mut avg_row_sum = Complex64::new(0.0, 0.0);
    for i in 0..system.num_dofs {
        let mut row_sum = Complex64::new(0.0, 0.0);
        for j in 0..system.num_dofs {
            row_sum += system.matrix[[i, j]];
        }
        max_row_sum = max_row_sum.max(row_sum.norm());
        avg_row_sum += row_sum;
    }
    avg_row_sum /= system.num_dofs as f64;
    println!("Max row sum: {:.6}", max_row_sum);
    println!(
        "Avg row sum: {:.6} + {:.6}i",
        avg_row_sum.re, avg_row_sum.im
    );

    // Check diagonal dominance
    let mut avg_diag_ratio = 0.0;
    for i in 0..system.num_dofs {
        let diag = system.matrix[[i, i]].norm();
        let mut off_diag_sum = 0.0;
        for j in 0..system.num_dofs {
            if i != j {
                off_diag_sum += system.matrix[[i, j]].norm();
            }
        }
        if off_diag_sum > 0.0 {
            avg_diag_ratio += diag / off_diag_sum;
        }
    }
    avg_diag_ratio /= system.num_dofs as f64;
    println!("Avg diag/off-diag ratio: {:.6}", avg_diag_ratio);

    // Compute K'[1] = sum of off-diagonal elements (for constant pressure)
    // Should be -1/2 for exterior problem with outward normals
    let mut sum_off_diag = Complex64::new(0.0, 0.0);
    for i in 0..system.num_dofs {
        for j in 0..system.num_dofs {
            if i != j {
                sum_off_diag += system.matrix[[i, j]];
            }
        }
    }
    sum_off_diag /= system.num_dofs as f64; // Average per row
    println!(
        "K'[1] (avg off-diag sum per row): {:.6} + {:.6}i",
        sum_off_diag.re, sum_off_diag.im
    );

    // The diagonal term (c)
    let mut avg_diag = Complex64::new(0.0, 0.0);
    for i in 0..system.num_dofs {
        avg_diag += system.matrix[[i, i]];
    }
    avg_diag /= system.num_dofs as f64;
    println!("Avg diagonal (c): {:.6} + {:.6}i", avg_diag.re, avg_diag.im);
    println!(
        "c + K'[1] should be 0: {:.6} + {:.6}i",
        avg_diag.re + sum_off_diag.re,
        avg_diag.im + sum_off_diag.im
    );
    println!();

    // Compute RHS from incident field
    // For DIRECT formulation: RHS = p_inc + β*∂p_inc/∂n at collocation points
    let incident_field = IncidentField::plane_wave_z();

    // Get element centers and normals
    let n = elements.len();
    let mut centers = Array2::zeros((n, 3));
    let mut normals = Array2::zeros((n, 3));
    for (i, elem) in elements.iter().enumerate() {
        for j in 0..3 {
            centers[[i, j]] = elem.center[j];
            normals[[i, j]] = elem.normal[j];
        }
    }

    let incident_rhs = incident_field.compute_rhs_with_beta(&centers, &normals, &physics, beta);

    println!("RHS from incident field:");
    println!(
        "  Max |RHS|: {:.6}",
        incident_rhs.iter().map(|z| z.norm()).fold(0.0, f64::max)
    );
    println!(
        "  Sum RHS: {:.6} + {:.6}i",
        incident_rhs.iter().sum::<Complex64>().re,
        incident_rhs.iter().sum::<Complex64>().im
    );
    println!();

    // Total RHS = TBEM RHS (should be 0 for v=0) + incident RHS
    println!("Total RHS:");
    println!(
        "  TBEM RHS sum: {:.6} + {:.6}i",
        system.rhs.iter().sum::<Complex64>().re,
        system.rhs.iter().sum::<Complex64>().im
    );
    let rhs = &system.rhs + &incident_rhs;
    println!(
        "  Max |total RHS|: {:.6}",
        rhs.iter().map(|z| z.norm()).fold(0.0, f64::max)
    );
    println!();

    // Solve the system
    println!("Solving linear system...");
    let result = lu_solve(&system.matrix, &rhs);
    let x = match result {
        Ok(x) => x,
        Err(e) => {
            eprintln!("Direct solver failed: {}", e);
            return;
        }
    };

    println!("Solution found!");
    println!(
        "  Max |p_surface|: {:.6}",
        x.iter().map(|z| z.norm()).fold(0.0, f64::max)
    );
    println!(
        "  Min |p_surface|: {:.6}",
        x.iter().map(|z| z.norm()).fold(f64::MAX, f64::min)
    );
    println!();

    // Compare with analytical at surface
    println!("=== Comparison with Mie Theory ===");

    // Sample a few elements at different theta angles
    let mut comparisons: Vec<(f64, f64, f64)> = Vec::new();

    for (i, elem) in elements.iter().enumerate() {
        // Compute theta from z-coordinate (cos(theta) = z/r)
        let z = elem.center[2];
        let theta = (z / radius).acos();

        let p_bem = x[i].norm();

        // Get analytical at same theta, at surface (r = radius)
        let analytical = sphere_scattering_3d(k, radius, 50, vec![radius * 1.001], vec![theta]);
        let p_ana = if analytical.pressure.is_empty() {
            0.0
        } else {
            analytical.pressure[0].norm()
        };

        comparisons.push((theta * 180.0 / PI, p_bem, p_ana));
    }

    // Sort by theta and print representative samples
    comparisons.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());

    println!("Theta(deg)  BEM |p|      Mie |p|      Error");
    println!("--------------------------------------------");

    let step = comparisons.len() / 10;
    for i in (0..comparisons.len()).step_by(step.max(1)) {
        let (theta, p_bem, p_ana) = comparisons[i];
        let error = if p_ana > 0.0 {
            (p_bem - p_ana).abs() / p_ana * 100.0
        } else {
            0.0
        };
        println!(
            "{:8.1}    {:10.6}   {:10.6}    {:6.1}%",
            theta, p_bem, p_ana, error
        );
    }

    // Overall statistics
    let total_err: f64 = comparisons
        .iter()
        .filter(|(_, _, p_ana)| *p_ana > 0.0)
        .map(|(_, p_bem, p_ana)| (p_bem - p_ana).abs() / p_ana)
        .sum();
    let avg_err = total_err / comparisons.len() as f64 * 100.0;

    let max_err: f64 = comparisons
        .iter()
        .filter(|(_, _, p_ana)| *p_ana > 0.0)
        .map(|(_, p_bem, p_ana)| (p_bem - p_ana).abs() / p_ana)
        .fold(0.0, f64::max)
        * 100.0;

    // Average magnitudes for scaling check
    let avg_bem: f64 =
        comparisons.iter().map(|(_, p, _)| p).sum::<f64>() / comparisons.len() as f64;
    let avg_mie: f64 =
        comparisons.iter().map(|(_, _, p)| p).sum::<f64>() / comparisons.len() as f64;

    println!();
    println!(
        "Avg |p| BEM: {:.4}, Mie: {:.4}, ratio: {:.3}",
        avg_bem,
        avg_mie,
        avg_bem / avg_mie
    );
    println!("Average error: {:.1}%", avg_err);
    println!("Max error: {:.1}%", max_err);
}
