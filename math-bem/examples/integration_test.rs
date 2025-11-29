//! Integration Test - Verify quadrature weights produce correct integrals
//!
//! Tests that ∫1 dS = surface area and ∫G dS has expected magnitude

#[cfg(feature = "pure-rust")]
fn main() {
    use bem::core::integration::regular_integration;
    use bem::core::mesh::generators::{generate_icosphere_mesh, generate_sphere_mesh};
    use bem::core::types::{BoundaryCondition, ElementType, PhysicsParams};
    use ndarray::{Array1, Array2};
    use num_complex::Complex64;
    use std::f64::consts::PI;

    let radius = 0.1;
    let speed_of_sound = 343.0;
    let density = 1.21;
    let ka_target = 0.2;
    let frequency = ka_target * speed_of_sound / (2.0 * PI * radius);
    let k = 2.0 * PI * frequency / speed_of_sound;

    println!("=== Integration Verification Test ===\n");
    println!("ka = {}, k = {:.4}, radius = {}", ka_target, k, radius);
    println!("Expected surface area: {:.6e}", 4.0 * PI * radius * radius);

    // Test UV-sphere
    println!("\n--- UV-sphere (8, 16) ---");
    let mesh_uv = generate_sphere_mesh(radius, 8, 16);
    test_integration(&mesh_uv, frequency, speed_of_sound, density, radius);

    // Test icosphere
    println!("\n--- Icosphere (subdivisions=2) ---");
    let mesh_ico = generate_icosphere_mesh(radius, 2);
    test_integration(&mesh_ico, frequency, speed_of_sound, density, radius);
}

#[cfg(feature = "pure-rust")]
fn test_integration(
    mesh: &bem::core::types::Mesh,
    frequency: f64,
    speed_of_sound: f64,
    density: f64,
    radius: f64,
) {
    use bem::core::integration::regular_integration;
    use bem::core::types::{BoundaryCondition, PhysicsParams};
    use ndarray::{Array1, Array2};
    use num_complex::Complex64;
    use std::f64::consts::PI;

    let physics = PhysicsParams::new(frequency, speed_of_sound, density, false);
    let n = mesh.elements.len();

    // Sum of element areas (should match analytical surface area)
    let total_area: f64 = mesh.elements.iter().map(|e| e.area).sum();
    let expected_area = 4.0 * PI * radius * radius;
    println!("  {} elements", n);
    println!(
        "  Sum of element areas: {:.6e} (expected {:.6e}, error = {:.2}%)",
        total_area,
        expected_area,
        100.0 * (total_area - expected_area).abs() / expected_area
    );

    // Test: integrate G from a far-away source point
    // For r >> element_size, G ≈ exp(ikR)/(4πR) ≈ constant over element
    // So ∫G dS ≈ G(center) × area
    let far_source = Array1::from_vec(vec![10.0, 0.0, 0.0]); // 10m away
    let far_normal = Array1::from_vec(vec![1.0, 0.0, 0.0]);

    let mut total_g_integral = Complex64::new(0.0, 0.0);

    for elem in &mesh.elements {
        let element_coords = mesh.element_nodes(elem);

        let result = regular_integration(
            &far_source,
            &far_normal,
            &element_coords,
            elem.element_type,
            elem.area,
            &physics,
            None,
            0,
            false,
        );

        total_g_integral += result.g_integral;
    }

    // Expected: ∫G dS ≈ G(origin) × surface_area = exp(ik*10)/(4π*10) × 4πr²
    let k = physics.wave_number;
    let r_far = 10.0;
    let expected_g = Complex64::new((k * r_far).cos(), (k * r_far).sin()) / (4.0 * PI * r_far);
    let expected_g_integral = expected_g * expected_area;

    println!(
        "  G integral sum: {:.6e} + {:.6e}i (|.|={:.6e})",
        total_g_integral.re,
        total_g_integral.im,
        total_g_integral.norm()
    );
    println!(
        "  Expected G integral: {:.6e} + {:.6e}i (|.|={:.6e})",
        expected_g_integral.re,
        expected_g_integral.im,
        expected_g_integral.norm()
    );
    println!(
        "  Ratio: {:.4}",
        total_g_integral.norm() / expected_g_integral.norm()
    );

    // Test: integrate 1 using the quadrature (compute sum of weighted Jacobians)
    // This tests if the weight × Jacobian gives the correct element areas
    println!("\n  Testing integral of 1 (should equal element area for each element):");

    let test_source = Array1::from_vec(vec![100.0, 0.0, 0.0]); // Very far away
    let test_normal = Array1::from_vec(vec![1.0, 0.0, 0.0]);

    let mut errors = Vec::new();
    for (i, elem) in mesh.elements.iter().enumerate().take(5) {
        let element_coords = mesh.element_nodes(elem);

        let result = regular_integration(
            &test_source,
            &test_normal,
            &element_coords,
            elem.element_type,
            elem.area,
            &physics,
            None,
            0,
            false,
        );

        // For very far source, G ≈ constant, so g_integral ≈ G_at_center × ∫1 dS = G × area
        // The error in "area" computation shows if weights are correct
        let r = 100.0;
        let g_at_center = Complex64::new((k * r).cos(), (k * r).sin()) / (4.0 * PI * r);
        let computed_area = result.g_integral / g_at_center;
        let area_error = 100.0 * (computed_area.re - elem.area).abs() / elem.area;

        errors.push(area_error);
        if i < 5 {
            println!(
                "    Element {}: actual area={:.6e}, computed={:.6e} (error={:.2}%)",
                i, elem.area, computed_area.re, area_error
            );
        }
    }

    let avg_error: f64 = errors.iter().sum::<f64>() / errors.len() as f64;
    let max_error = errors.iter().cloned().fold(0.0f64, f64::max);
    println!(
        "  Area reconstruction: avg error={:.2}%, max error={:.2}%",
        avg_error, max_error
    );
}

#[cfg(not(feature = "pure-rust"))]
fn main() {
    eprintln!("This example requires the 'pure-rust' feature.");
}
