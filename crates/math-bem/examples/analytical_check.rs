//! Analytical check - Print expected values from Mie theory and compare to BEM

#[cfg(feature = "pure-rust")]
fn main() {
    use math_audio_bem::analytical::sphere_scattering_3d;
    use math_audio_bem::core::incident::IncidentField;
    use math_audio_bem::core::types::PhysicsParams;
    use ndarray::Array2;
    use std::f64::consts::PI;

    let radius = 0.1;
    let speed_of_sound = 343.0;
    let density = 1.21;
    let ka_target = 0.2;
    let frequency = ka_target * speed_of_sound / (2.0 * PI * radius);
    let k = 2.0 * PI * frequency / speed_of_sound;

    println!("=== Analytical Values Check ===\n");
    println!(
        "ka = {:.2}, k = {:.4}, radius = {}, frequency = {:.2} Hz",
        ka_target, k, radius, frequency
    );

    // Mie theory at the surface
    let theta_angles: Vec<f64> = (0..=18).map(|i| i as f64 * 10.0 * PI / 180.0).collect();
    let mie = sphere_scattering_3d(k, radius, 50, vec![radius], theta_angles.clone());

    println!("\n--- Mie theory at r = radius (surface) ---");
    println!("θ (deg) |  p_total (Mie)  |  |p_total|");
    println!("--------|-----------------|----------");
    for (i, &theta) in theta_angles.iter().enumerate() {
        let p = mie.pressure[i];
        println!(
            "{:>7.0}° | {:.4}+{:.4}i | {:.4}",
            theta * 180.0 / PI,
            p.re,
            p.im,
            p.norm()
        );
    }

    // Expected incident field at surface
    println!("\n--- Incident plane wave at surface (analytical) ---");
    let physics = PhysicsParams::new(frequency, speed_of_sound, density, false);
    let incident = IncidentField::plane_wave_z();

    // Points on sphere surface at various angles
    let mut centers = Array2::zeros((theta_angles.len(), 3));
    let mut normals = Array2::zeros((theta_angles.len(), 3));
    for (i, &theta) in theta_angles.iter().enumerate() {
        // Point on sphere at angle theta (spherical coords: z = r*cos(theta), x = r*sin(theta))
        centers[[i, 0]] = radius * theta.sin(); // x
        centers[[i, 1]] = 0.0; // y
        centers[[i, 2]] = radius * theta.cos(); // z

        // Normal pointing outward
        normals[[i, 0]] = theta.sin();
        normals[[i, 1]] = 0.0;
        normals[[i, 2]] = theta.cos();
    }

    let p_inc_values = incident.evaluate_pressure(&centers, &physics);
    let dpdn_values = incident.evaluate_normal_derivative(&centers, &normals, &physics);
    let beta = physics.burton_miller_beta();
    let rhs_values: Vec<_> = (0..theta_angles.len())
        .map(|i| p_inc_values[i] + beta * dpdn_values[i])
        .collect();

    println!("θ (deg) |  p_inc           |  ∂p_inc/∂n       |  RHS (p_inc + β*dpdn)");
    println!("--------|------------------|------------------|----------------------");
    for (i, &theta) in theta_angles.iter().enumerate() {
        let p = p_inc_values[i];
        let dpdn = dpdn_values[i];
        let rhs = rhs_values[i];
        println!(
            "{:>7.0}° | {:.4}+{:.4}i | {:.4}+{:.4}i | {:.4}+{:.4}i (|.|={:.4})",
            theta * 180.0 / PI,
            p.re,
            p.im,
            dpdn.re,
            dpdn.im,
            rhs.re,
            rhs.im,
            rhs.norm()
        );
    }

    // What should the solution be?
    println!("\n--- Expected solution (p_total should match Mie) ---");
    println!("At ka = 0.2 (Rayleigh regime), total surface pressure |p| ≈ 1");

    // For reference, compute incident pressure magnitude
    let avg_p_inc: f64 =
        p_inc_values.iter().map(|x| x.norm()).sum::<f64>() / p_inc_values.len() as f64;
    let avg_mie: f64 =
        mie.pressure.iter().map(|x| x.norm()).sum::<f64>() / mie.pressure.len() as f64;
    let avg_rhs: f64 = rhs_values.iter().map(|x| x.norm()).sum::<f64>() / rhs_values.len() as f64;

    println!("\nAverage |p_inc| = {:.4}", avg_p_inc);
    println!("Average |Mie total| = {:.4}", avg_mie);
    println!("Average |RHS| = {:.4}", avg_rhs);
    println!("β = {:.4}+{:.4}i", beta.re, beta.im);

    // Check: if A*p = b, and A ≈ 0.5*I (dominant diagonal), then p ≈ 2*b
    println!(
        "\nIf matrix was just 0.5*I, solution would be: 2 * RHS ≈ {:.4}",
        2.0 * avg_rhs
    );
    println!("With corrected BIE formulation (0.5 - K), BEM should match expected values.");
}

#[cfg(not(feature = "pure-rust"))]
fn main() {
    eprintln!("This example requires the 'pure-rust' feature.");
}
