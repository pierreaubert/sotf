//! Hypersingular Integral Test - Verify E[1] = 0 for constant function
//!
//! The hypersingular operator E applied to a constant function should give 0.
//! E[1] = ∫_S ∂²G/(∂n_x ∂n_y) dS_y = 0
//!
//! If this is not zero, there's an error in the E integral computation.

#[cfg(feature = "pure-rust")]
fn main() {
    use math_audio_bem::core::mesh::generators::{generate_icosphere_mesh, generate_sphere_mesh};
    use std::f64::consts::PI;

    let radius = 0.1;
    let speed_of_sound = 343.0;
    let density = 1.21;
    let ka_target = 0.2;
    let frequency = ka_target * speed_of_sound / (2.0 * PI * radius);

    println!("=== Hypersingular Integral Test: E[1] should equal 0 ===\n");
    println!("ka = {:.2}, β = i/k = {:.4}i", ka_target, 0.5);

    // Test UV-sphere
    println!("\n--- UV-sphere (8, 16) ---");
    let mesh_uv = generate_sphere_mesh(radius, 8, 16);
    test_e_constant(&mesh_uv, frequency, speed_of_sound, density);

    // Test icosphere
    println!("\n--- Icosphere (subdivisions=2) ---");
    let mesh_ico = generate_icosphere_mesh(radius, 2);
    test_e_constant(&mesh_ico, frequency, speed_of_sound, density);
}

#[cfg(feature = "pure-rust")]
fn test_e_constant(
    mesh: &math_audio_bem::core::types::Mesh,
    frequency: f64,
    speed_of_sound: f64,
    density: f64,
) {
    use math_audio_bem::core::assembly::tbem::build_tbem_system;
    use math_audio_bem::core::types::{BoundaryCondition, PhysicsParams};
    use ndarray::Array1;
    use num_complex::Complex64;

    let physics = PhysicsParams::new(frequency, speed_of_sound, density, false);
    let beta = physics.burton_miller_beta();

    // Prepare elements
    let mut elements = mesh.elements.clone();
    for (i, elem) in elements.iter_mut().enumerate() {
        elem.boundary_condition = BoundaryCondition::Velocity(vec![Complex64::new(0.0, 0.0)]);
        elem.dof_addresses = vec![i];
    }

    // Build system
    let system = build_tbem_system(&elements, &mesh.nodes, &physics);
    let n = system.num_dofs;

    // Apply matrix to constant vector p = [1, 1, ..., 1]
    // A * [1] = row sums of A
    let n = mesh.elements.len();
    println!("Matrix size: {}x{}", n, n);
    let mut a_times_one = Array1::zeros(n);
    for i in 0..n {
        for j in 0..n {
            a_times_one[i] += system.matrix[[i, j]];
        }
    }

    // The matrix A = c*I + K + β*E
    // A[1] = c*[1] + K[1] + β*E[1]
    //      = 0.5*[1] + (-0.5)*[1] + β*E[1]  (for exterior closed surface)
    //      = β*E[1]
    //
    // So the row sum should be approximately β*E[1].
    // For exact integration, E[1] = 0, so row sums should be ~0.

    // Compute row sum statistics
    let avg_row_sum: Complex64 = a_times_one.iter().sum::<Complex64>() / n as f64;
    let max_row_sum_mag = a_times_one.iter().map(|x| x.norm()).fold(0.0f64, f64::max);
    let min_row_sum_mag = a_times_one
        .iter()
        .map(|x| x.norm())
        .fold(f64::MAX, f64::min);

    println!("  {} DOFs", n);
    println!(
        "  Average row sum: {:.4}+{:.4}i (|.|={:.4})",
        avg_row_sum.re,
        avg_row_sum.im,
        avg_row_sum.norm()
    );
    println!(
        "  Row sum magnitude range: {:.4} to {:.4}",
        min_row_sum_mag, max_row_sum_mag
    );

    // If β*E[1] ≠ 0, compute what E[1] would be
    // row_sum ≈ β*E[1], so E[1] ≈ row_sum / β
    let e_one_approx = avg_row_sum / beta;
    println!("\n  Implied E[1] from row sums:");
    println!(
        "  E[1] ≈ row_sum / β = {:.4}+{:.4}i (|.|={:.4})",
        e_one_approx.re,
        e_one_approx.im,
        e_one_approx.norm()
    );
    println!("  Expected E[1] = 0 for constant function");

    // Decompose row sum into real and imaginary parts to isolate K and E
    // A = c*I + K + β*E
    // row_sum = c*1 + K[1] + β*E[1]
    //         = 0.5 + K[1] + β*E[1]
    //
    // For β = i*η (purely imaginary), β*E[1] is imaginary if E[1] is real.
    // K[1] should be real and ≈ -0.5 for exterior problem.
    //
    // Re(row_sum) = 0.5 + Re(K[1]) ≈ 0.5 + (-0.5) = 0
    // Im(row_sum) = Im(β*E[1]) = η*Re(E[1]) for real E[1]

    println!("\n  Decomposition:");
    println!(
        "  Re(row_sum) = 0.5 + K[1] ≈ {:.4} => K[1] ≈ {:.4} (expected -0.5)",
        avg_row_sum.re,
        avg_row_sum.re - 0.5
    );
    println!(
        "  Im(row_sum) = β*E[1] ≈ {:.4}i => E[1] ≈ {:.4}",
        avg_row_sum.im,
        avg_row_sum.im / beta.im
    );

    // Check individual row sum decomposition
    println!("\n  First 5 row sums:");
    for i in 0..5.min(n) {
        let row_sum = a_times_one[i];
        println!(
            "    Row {}: {:.4}+{:.4}i (|.|={:.4})",
            i,
            row_sum.re,
            row_sum.im,
            row_sum.norm()
        );
    }

    // The diagonal should be c + (self-integrals of K and β*E)
    println!("\n  Diagonal entries (first 5):");
    for i in 0..5.min(n) {
        let diag = system.matrix[[i, i]];
        println!(
            "    A[{},{}] = {:.4}+{:.4}i (|.|={:.4})",
            i,
            i,
            diag.re,
            diag.im,
            diag.norm()
        );
    }
}

#[cfg(not(feature = "pure-rust"))]
fn main() {
    eprintln!("This example requires the 'pure-rust' feature.");
}
