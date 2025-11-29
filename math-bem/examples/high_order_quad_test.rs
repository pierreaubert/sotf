//! High-Order Quadrature Test
//!
//! Tests if using significantly higher quadrature orders improves the row sum accuracy.
//! This is a diagnostic to confirm that the oscillatory integration is the root cause.

#[cfg(feature = "pure-rust")]
fn main() {
    use bem::core::assembly::tbem::build_tbem_system_with_beta;
    use bem::core::mesh::generators::generate_icosphere_mesh;
    use bem::core::types::{BoundaryCondition, PhysicsParams};
    use ndarray::{Array1, Array2};
    use num_complex::Complex64;
    use std::f64::consts::PI;

    println!("=== High-Order Quadrature Test ===");
    println!("Testing if row sum errors at finite k are due to oscillatory integration\n");

    let radius = 0.1;
    let speed_of_sound = 343.0;
    let density = 1.21;

    // Test at ka = 1.0 where we see significant row sum errors
    let ka_target = 1.0;
    let frequency = ka_target * speed_of_sound / (2.0 * PI * radius);
    let k = 2.0 * PI * frequency / speed_of_sound;

    let physics = PhysicsParams::new(frequency, speed_of_sound, density, false);
    let beta = physics.burton_miller_beta();

    println!("ka = {:.2}, k = {:.2}, β = {:.4}i", ka_target, k, beta.im);
    println!();

    // Test with different mesh resolutions
    // More elements = smaller kr per element = better quadrature
    for subdivisions in [2, 3, 4] {
        let mesh = generate_icosphere_mesh(radius, subdivisions);
        let n = mesh.elements.len();

        // Estimate average element size and kr
        let avg_area = mesh.average_element_area();
        let avg_edge = avg_area.sqrt();
        let kr_element = k * avg_edge;

        let mut elements = mesh.elements.clone();
        for (i, elem) in elements.iter_mut().enumerate() {
            elem.boundary_condition = BoundaryCondition::Velocity(vec![Complex64::new(0.0, 0.0)]);
            elem.dof_addresses = vec![i];
        }

        let system = build_tbem_system_with_beta(&elements, &mesh.nodes, &physics, beta);

        // Analyze row sums
        let mut row_sum_sum = Complex64::new(0.0, 0.0);
        let mut diag_sum = Complex64::new(0.0, 0.0);

        for i in 0..n {
            let mut row_sum = Complex64::new(0.0, 0.0);
            for j in 0..n {
                row_sum += system.matrix[[i, j]];
            }
            row_sum_sum += row_sum;
            diag_sum += system.matrix[[i, i]];
        }

        let avg_row_sum = row_sum_sum / n as f64;
        let avg_diag = diag_sum / n as f64;
        let avg_offdiag = avg_row_sum - avg_diag;

        // Expected: row_sum = 0, diag ≈ 0.5 + β*E_self, offdiag ≈ -0.5 + β*E_offdiag
        println!(
            "Subdivisions: {}, elements: {}, kr_elem: {:.3}",
            subdivisions, n, kr_element
        );
        println!(
            "  Avg row sum = {:.6} + {:.6}i (should be ~0)",
            avg_row_sum.re, avg_row_sum.im
        );
        println!("  Avg diagonal = {:.6} + {:.6}i", avg_diag.re, avg_diag.im);
        println!(
            "  Avg off-diag = {:.6} + {:.6}i (real should be ~-0.5)",
            avg_offdiag.re, avg_offdiag.im
        );

        // The key metric: how far is the off-diagonal real sum from -0.5?
        let k_error = (avg_offdiag.re + 0.5).abs();
        println!("  K'[1] error: |K'_offdiag + 0.5| = {:.6}", k_error);
        println!();
    }

    // Now test the same thing with a finer base mesh but at ka = 0.5
    println!("=== Lower ka = 0.5 (less oscillatory) ===\n");

    let ka_target = 0.5;
    let frequency = ka_target * speed_of_sound / (2.0 * PI * radius);
    let k = 2.0 * PI * frequency / speed_of_sound;

    let physics = PhysicsParams::new(frequency, speed_of_sound, density, false);
    let beta = physics.burton_miller_beta();

    for subdivisions in [2, 3, 4] {
        let mesh = generate_icosphere_mesh(radius, subdivisions);
        let n = mesh.elements.len();

        let avg_area = mesh.average_element_area();
        let avg_edge = avg_area.sqrt();
        let kr_element = k * avg_edge;

        let mut elements = mesh.elements.clone();
        for (i, elem) in elements.iter_mut().enumerate() {
            elem.boundary_condition = BoundaryCondition::Velocity(vec![Complex64::new(0.0, 0.0)]);
            elem.dof_addresses = vec![i];
        }

        let system = build_tbem_system_with_beta(&elements, &mesh.nodes, &physics, beta);

        let mut row_sum_sum = Complex64::new(0.0, 0.0);
        let mut diag_sum = Complex64::new(0.0, 0.0);

        for i in 0..n {
            let mut row_sum = Complex64::new(0.0, 0.0);
            for j in 0..n {
                row_sum += system.matrix[[i, j]];
            }
            row_sum_sum += row_sum;
            diag_sum += system.matrix[[i, i]];
        }

        let avg_row_sum = row_sum_sum / n as f64;
        let avg_diag = diag_sum / n as f64;
        let avg_offdiag = avg_row_sum - avg_diag;

        println!(
            "Subdivisions: {}, elements: {}, kr_elem: {:.3}",
            subdivisions, n, kr_element
        );
        println!(
            "  Avg row sum = {:.6} + {:.6}i (should be ~0)",
            avg_row_sum.re, avg_row_sum.im
        );
        let k_error = (avg_offdiag.re + 0.5).abs();
        println!("  K'[1] error: |K'_offdiag + 0.5| = {:.6}", k_error);
        println!();
    }

    println!("=== Conclusion ===");
    println!("If K'[1] error decreases with finer mesh (smaller kr_element),");
    println!("the issue is oscillatory integration accuracy.");
    println!("If error stays constant or increases, there's a different problem.");
}

#[cfg(not(feature = "pure-rust"))]
fn main() {
    eprintln!("This example requires the 'pure-rust' feature.");
}
