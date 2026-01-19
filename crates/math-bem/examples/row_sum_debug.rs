//! Row Sum Debug - Analyze matrix row sums to verify BEM formulation
//!
//! For sound-hard exterior scattering, the Burton-Miller formulation is:
//! (c + K' + βE)[p] = p_inc + β*∂p_inc/∂n
//!
//! Key properties:
//! - c = +1/2 for exterior problem (diagonal free term)
//! - K'[1] should equal -1/2 for exterior problem (off-diagonal sum)
//! - E[1] should equal 0 for closed surface
//!
//! Therefore, row sum = c + K'[1] + β*E[1] = 1/2 - 1/2 + 0 = 0

#[cfg(feature = "pure-rust")]
fn main() {
    use math_audio_bem::core::assembly::tbem::build_tbem_system_with_beta;
    use math_audio_bem::core::mesh::generators::generate_icosphere_mesh;
    use math_audio_bem::core::types::{BoundaryCondition, PhysicsParams};
    use num_complex::Complex64;
    use std::f64::consts::PI;

    println!("=== Row Sum Debug ===\n");

    let radius = 0.1;
    let speed_of_sound = 343.0;
    let density = 1.21;

    for ka_target in [0.5, 1.0, 2.0] {
        let frequency = ka_target * speed_of_sound / (2.0 * PI * radius);
        let k = 2.0 * PI * frequency / speed_of_sound;

        println!("=== ka = {:.2} ===", ka_target);

        let physics = PhysicsParams::new(frequency, speed_of_sound, density, false);
        let beta = physics.burton_miller_beta();

        let mesh = generate_icosphere_mesh(radius, 2);
        let n = mesh.elements.len();

        let mut elements = mesh.elements.clone();
        for (i, elem) in elements.iter_mut().enumerate() {
            elem.boundary_condition = BoundaryCondition::Velocity(vec![Complex64::new(0.0, 0.0)]);
            elem.dof_addresses = vec![i];
        }

        // Build matrix with standard beta
        let system = build_tbem_system_with_beta(&elements, &mesh.nodes, &physics, beta);

        // Analyze row sums
        let mut row_sum_re_sum = 0.0;
        let mut row_sum_im_sum = 0.0;
        let mut diag_re_sum = 0.0;
        let mut diag_im_sum = 0.0;

        for i in 0..n {
            let mut row_sum = Complex64::new(0.0, 0.0);
            for j in 0..n {
                row_sum += system.matrix[[i, j]];
            }
            row_sum_re_sum += row_sum.re;
            row_sum_im_sum += row_sum.im;
            diag_re_sum += system.matrix[[i, i]].re;
            diag_im_sum += system.matrix[[i, i]].im;
        }

        let avg_row_sum = Complex64::new(row_sum_re_sum / n as f64, row_sum_im_sum / n as f64);
        let avg_diag = Complex64::new(diag_re_sum / n as f64, diag_im_sum / n as f64);

        // Off-diagonal sum = row_sum - diagonal
        let avg_offdiag = avg_row_sum - avg_diag;

        println!("  β = {:.4} + {:.4}i", beta.re, beta.im);
        println!("  {} elements", n);
        println!("  Avg diagonal = {:.6} + {:.6}i", avg_diag.re, avg_diag.im);
        println!(
            "  Avg off-diag sum = {:.6} + {:.6}i (should be ~ -0.5 for K'[1])",
            avg_offdiag.re, avg_offdiag.im
        );
        println!(
            "  Avg row sum = {:.6} + {:.6}i (should be ~ 0)",
            avg_row_sum.re, avg_row_sum.im
        );

        // What we expect:
        // - Diagonal ≈ 0.5 + K'_self + β*E_self
        // - Off-diag sum ≈ K'_offdiag + β*E_offdiag
        // - If K'[1] = -0.5: K'_self + K'_offdiag = -0.5
        // - If E[1] = 0: E_self + E_offdiag = 0
        // - Row sum = 0.5 + (K'_self + K'_offdiag) + β*(E_self + E_offdiag) = 0.5 - 0.5 + 0 = 0

        // Sample a few elements
        println!("\n  Sample element row sums:");
        for i in [0, n / 4, n / 2, 3 * n / 4] {
            let mut row_sum = Complex64::new(0.0, 0.0);
            for j in 0..n {
                row_sum += system.matrix[[i, j]];
            }
            println!(
                "    elem[{}]: row_sum = {:.6} + {:.6}i, diag = {:.6} + {:.6}i",
                i,
                row_sum.re,
                row_sum.im,
                system.matrix[[i, i]].re,
                system.matrix[[i, i]].im
            );
        }
        println!();
    }

    // Also test with Laplace (k=0) where K'[1] = -1/2 exactly
    println!("=== Laplace limit (k → 0) ===");

    let frequency = 1.0; // Very low frequency
    let k = 2.0 * PI * frequency / speed_of_sound;

    let physics = PhysicsParams::new(frequency, speed_of_sound, density, false);
    let beta = Complex64::new(0.0, 0.0); // No Burton-Miller for Laplace

    let mesh = generate_icosphere_mesh(radius, 2);
    let n = mesh.elements.len();

    let mut elements = mesh.elements.clone();
    for (i, elem) in elements.iter_mut().enumerate() {
        elem.boundary_condition = BoundaryCondition::Velocity(vec![Complex64::new(0.0, 0.0)]);
        elem.dof_addresses = vec![i];
    }

    let system = build_tbem_system_with_beta(&elements, &mesh.nodes, &physics, beta);

    let mut row_sum_re_sum = 0.0;
    let mut diag_re_sum = 0.0;

    for i in 0..n {
        let mut row_sum = Complex64::new(0.0, 0.0);
        for j in 0..n {
            row_sum += system.matrix[[i, j]];
        }
        row_sum_re_sum += row_sum.re;
        diag_re_sum += system.matrix[[i, i]].re;
    }

    let avg_row_sum = row_sum_re_sum / n as f64;
    let avg_diag = diag_re_sum / n as f64;
    let avg_offdiag = avg_row_sum - avg_diag;

    println!("  k = {:.4} (very low)", k);
    println!("  {} elements", n);
    println!("  Avg diagonal = {:.6} (should be ~ 0.5)", avg_diag);
    println!(
        "  Avg off-diag sum = {:.6} (should be ~ -0.5 for K'[1])",
        avg_offdiag
    );
    println!("  Avg row sum = {:.6} (should be ~ 0)", avg_row_sum);
}

#[cfg(not(feature = "pure-rust"))]
fn main() {
    eprintln!("This example requires the 'pure-rust' feature.");
}
