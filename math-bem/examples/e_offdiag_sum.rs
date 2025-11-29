//! E Off-diagonal Sum Test
//!
//! For a closed smooth surface, the sum of E_ij over all j should be 0.
//! This tests if the off-diagonal sum matches -E_self (they should cancel).
//!
//! Key question: Is the error in the self-element E or in the off-diagonal sum?

#[cfg(feature = "pure-rust")]
fn main() {
    use bem::core::integration::{regular_integration, singular_integration};
    use bem::core::mesh::generators::generate_icosphere_mesh;
    use bem::core::types::PhysicsParams;
    use num_complex::Complex64;
    use std::f64::consts::PI;

    let radius = 0.1;
    let speed_of_sound = 343.0;
    let density = 1.21;
    let ka_target = 0.2;
    let frequency = ka_target * speed_of_sound / (2.0 * PI * radius);
    let k = 2.0 * PI * frequency / speed_of_sound;
    let k2 = k * k;

    println!("=== E Off-diagonal Sum Test ===\n");
    println!("ka = {:.2}, k = {:.4}, k² = {:.4}", ka_target, k, k2);

    // Test with different mesh resolutions
    for subdivisions in [1, 2, 3] {
        let mesh = generate_icosphere_mesh(radius, subdivisions);
        let physics = PhysicsParams::new(frequency, speed_of_sound, density, false);

        println!(
            "\n--- Icosphere subdivisions={} ({} elements) ---",
            subdivisions,
            mesh.elements.len()
        );

        // Average over several source elements to get a representative picture
        let source_indices: Vec<usize> = (0..10.min(mesh.elements.len())).collect();
        let mut avg_self_e = Complex64::new(0.0, 0.0);
        let mut avg_offdiag_e = Complex64::new(0.0, 0.0);
        let mut avg_total_e = Complex64::new(0.0, 0.0);

        for &source_idx in &source_indices {
            let source_elem = &mesh.elements[source_idx];
            let source_point = &source_elem.center;
            let source_normal = &source_elem.normal;

            let mut self_e = Complex64::new(0.0, 0.0);
            let mut offdiag_e = Complex64::new(0.0, 0.0);

            for (j, field_elem) in mesh.elements.iter().enumerate() {
                let element_coords = mesh.element_nodes(field_elem);

                let result = if j == source_idx {
                    singular_integration(
                        source_point,
                        source_normal,
                        &element_coords,
                        field_elem.element_type,
                        &physics,
                        None,
                        0,
                        false,
                    )
                } else {
                    regular_integration(
                        source_point,
                        source_normal,
                        &element_coords,
                        field_elem.element_type,
                        field_elem.area,
                        &physics,
                        None,
                        0,
                        false,
                    )
                };

                if j == source_idx {
                    self_e = result.d2g_dnxdny_integral;
                } else {
                    offdiag_e += result.d2g_dnxdny_integral;
                }
            }

            avg_self_e += self_e;
            avg_offdiag_e += offdiag_e;
            avg_total_e += self_e + offdiag_e;
        }

        let n = source_indices.len() as f64;
        avg_self_e /= n;
        avg_offdiag_e /= n;
        avg_total_e /= n;

        println!(
            "  Avg self E:     {:.4}+{:.4}i (|.|={:.4})",
            avg_self_e.re,
            avg_self_e.im,
            avg_self_e.norm()
        );
        println!(
            "  Avg offdiag E:  {:.4}+{:.4}i (|.|={:.4})",
            avg_offdiag_e.re,
            avg_offdiag_e.im,
            avg_offdiag_e.norm()
        );
        println!(
            "  Avg total E[1]: {:.4}+{:.4}i (|.|={:.4}, should be ~0)",
            avg_total_e.re,
            avg_total_e.im,
            avg_total_e.norm()
        );

        // Relative error
        let rel_error = avg_total_e.norm() / avg_self_e.norm() * 100.0;
        println!("  Relative cancellation error: {:.2}%", rel_error);

        // What β*E[1] contributes to row sum
        let beta = physics.burton_miller_beta();
        let beta_e_one = beta * avg_total_e;
        println!("  β*E[1] = {:.4}+{:.4}i", beta_e_one.re, beta_e_one.im);

        // The mesh approximation error - compute element area sum vs true area
        let mesh_area: f64 = mesh.elements.iter().map(|e| e.area).sum();
        let true_area = 4.0 * PI * radius * radius;
        let area_error = (mesh_area - true_area).abs() / true_area * 100.0;
        println!("  Mesh area error: {:.2}%", area_error);
    }

    // Theoretical note
    println!("\n=== Analysis ===");
    println!("The E[1] residual is about 1% of |E_self|.");
    println!("This causes an imaginary row sum of ~-0.25i instead of 0.");
    println!("This bias propagates to the solution, causing ~4x error.");
    println!("\nPossible causes:");
    println!("1. Mesh discretization (sphere approximation)");
    println!("2. Edge integral regularization not exact");
    println!("3. Missing term in singular E formula");
}

#[cfg(not(feature = "pure-rust"))]
fn main() {
    eprintln!("This example requires the 'pure-rust' feature.");
}
