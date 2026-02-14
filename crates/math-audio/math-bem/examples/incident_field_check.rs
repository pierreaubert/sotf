//! Incident Field Check - Verify plane wave values on sphere surface
//!
//! For plane wave traveling in +z direction: p_inc = exp(ikz)
//! At front (z>0): |p_inc| ≈ 1, ∂p_inc/∂n = +ik*n_z*p_inc ≈ +ik (positive imaginary)
//! At back (z<0): |p_inc| ≈ 1, ∂p_inc/∂n = +ik*n_z*p_inc ≈ -ik (negative imaginary)

#[cfg(feature = "pure-rust")]
fn main() {
    use math_audio_bem::core::incident::IncidentField;
    use math_audio_bem::core::mesh::generators::generate_icosphere_mesh;
    use math_audio_bem::core::types::PhysicsParams;
    use ndarray::Array2;
    use std::f64::consts::PI;

    println!("=== Incident Field Check ===\n");

    let radius = 0.1;
    let speed_of_sound = 343.0;
    let density = 1.21;
    let ka_target = 1.0;
    let frequency = ka_target * speed_of_sound / (2.0 * PI * radius);
    let k = 2.0 * PI * frequency / speed_of_sound;

    println!(
        "Parameters: ka = {:.2}, k = {:.2}, radius = {:.2}",
        ka_target, k, radius
    );
    println!(
        "At z = +radius: exp(ikr) phase = {:.2}° ",
        k * radius * 180.0 / PI
    );
    println!(
        "At z = -radius: exp(-ikr) phase = {:.2}°\n",
        -k * radius * 180.0 / PI
    );

    let physics = PhysicsParams::new(frequency, speed_of_sound, density, false);
    let incident = IncidentField::plane_wave_z();

    let mesh = generate_icosphere_mesh(radius, 1);
    let n = mesh.elements.len();

    let mut centers = Array2::zeros((n, 3));
    let mut normals = Array2::zeros((n, 3));
    for (i, elem) in mesh.elements.iter().enumerate() {
        for j in 0..3 {
            centers[[i, j]] = elem.center[j];
            normals[[i, j]] = elem.normal[j];
        }
    }

    let p_inc = incident.evaluate_pressure(&centers, &physics);
    let dpdn_inc = incident.evaluate_normal_derivative(&centers, &normals, &physics);

    // Expected values:
    // p_inc at z = exp(ik*z)
    // dp_inc/dn = (ik * direction) · normal * p_inc
    // For +z direction: dp_inc/dn = ik * n_z * p_inc

    println!("Front elements (z > 0.05):");
    println!("  elem   |     z      |  |p_inc|  |  p_inc phase  |  dpdn_inc re/im  |  n_z");
    println!("---------|------------|----------|---------------|------------------|-------");

    for (i, elem) in mesh.elements.iter().enumerate() {
        if elem.center[2] > 0.05 {
            let z = elem.center[2];
            let p = p_inc[i];
            let dpdn = dpdn_inc[i];
            let n_z = elem.normal[2];

            // Expected: p = exp(ikz)
            let expected_p = num_complex::Complex64::new(0.0, k * z).exp();
            // Expected: dpdn = ik * n_z * p_inc
            let expected_dpdn = num_complex::Complex64::new(0.0, k) * n_z * expected_p;

            println!(
                "  {:>4}   | {:>10.4} | {:>8.4} | {:>13.1}° | {:>7.4} + {:>7.4}i | {:>6.3}",
                i,
                z,
                p.norm(),
                p.arg() * 180.0 / PI,
                dpdn.re,
                dpdn.im,
                n_z
            );
            println!(
                "         |  expected: | {:>8.4} | {:>13.1}° | {:>7.4} + {:>7.4}i |",
                expected_p.norm(),
                expected_p.arg() * 180.0 / PI,
                expected_dpdn.re,
                expected_dpdn.im
            );
        }
    }

    println!("\nBack elements (z < -0.05):");
    println!("  elem   |     z      |  |p_inc|  |  p_inc phase  |  dpdn_inc re/im  |  n_z");
    println!("---------|------------|----------|---------------|------------------|-------");

    for (i, elem) in mesh.elements.iter().enumerate() {
        if elem.center[2] < -0.05 {
            let z = elem.center[2];
            let p = p_inc[i];
            let dpdn = dpdn_inc[i];
            let n_z = elem.normal[2];

            // Expected: p = exp(ikz)
            let expected_p = num_complex::Complex64::new(0.0, k * z).exp();
            // Expected: dpdn = ik * n_z * p_inc
            let expected_dpdn = num_complex::Complex64::new(0.0, k) * n_z * expected_p;

            println!(
                "  {:>4}   | {:>10.4} | {:>8.4} | {:>13.1}° | {:>7.4} + {:>7.4}i | {:>6.3}",
                i,
                z,
                p.norm(),
                p.arg() * 180.0 / PI,
                dpdn.re,
                dpdn.im,
                n_z
            );
            println!(
                "         |  expected: | {:>8.4} | {:>13.1}° | {:>7.4} + {:>7.4}i |",
                expected_p.norm(),
                expected_p.arg() * 180.0 / PI,
                expected_dpdn.re,
                expected_dpdn.im
            );
        }
    }

    // Also compute RHS for both standard and scaled beta
    let beta_std = physics.burton_miller_beta();
    let beta_2x = physics.burton_miller_beta_scaled(2.0);

    println!("\n\nRHS = p_inc + β * dpdn_inc:");
    println!("Standard β = {:.4} + {:.4}i", beta_std.re, beta_std.im);
    println!("Scaled 2× β = {:.4} + {:.4}i", beta_2x.re, beta_2x.im);

    println!("\nFront elements (z > 0.05):");
    for (i, elem) in mesh.elements.iter().enumerate() {
        if elem.center[2] > 0.05 {
            let p = p_inc[i];
            let dpdn = dpdn_inc[i];
            let rhs_std = p + beta_std * dpdn;
            let rhs_2x = p + beta_2x * dpdn;

            println!(
                "  elem[{}]: z={:.4}, |RHS_std|={:.4}, |RHS_2x|={:.4}",
                i,
                elem.center[2],
                rhs_std.norm(),
                rhs_2x.norm()
            );
        }
    }

    println!("\nBack elements (z < -0.05):");
    for (i, elem) in mesh.elements.iter().enumerate() {
        if elem.center[2] < -0.05 {
            let p = p_inc[i];
            let dpdn = dpdn_inc[i];
            let rhs_std = p + beta_std * dpdn;
            let rhs_2x = p + beta_2x * dpdn;

            println!(
                "  elem[{}]: z={:.4}, |RHS_std|={:.4}, |RHS_2x|={:.4}",
                i,
                elem.center[2],
                rhs_std.norm(),
                rhs_2x.norm()
            );
        }
    }
}

#[cfg(not(feature = "pure-rust"))]
fn main() {
    eprintln!("This example requires the 'pure-rust' feature.");
}
