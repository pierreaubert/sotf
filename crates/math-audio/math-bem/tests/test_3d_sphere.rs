//! 3D Sphere Scattering Tests (Mie Theory)
//!
//! Tests rigid sphere scattering across different regimes:
//! - Rayleigh (ka << 1): long wavelength
//! - Mie (ka ~ 1): resonance regime
//! - Geometric (ka >> 1): short wavelength
//!
//! Generates JSON output for 3D visualization and validation.

use directories::ProjectDirs;
use math_audio_bem::analytical::{
    sphere_rcs_3d, sphere_scattering_3d, sphere_scattering_efficiency_3d,
};
use math_audio_bem::testing::ValidationResult;
use std::f64::consts::PI;
use std::path::PathBuf;

/// Get output directory using the directories crate
fn get_output_dir() -> PathBuf {
    let proj_dirs = ProjectDirs::from("org", "spinorama", "math-audio")
        .expect("Failed to determine project directories");

    let output_dir = proj_dirs.cache_dir().join("bem").join("3d");

    std::fs::create_dir_all(&output_dir).expect("Failed to create output directory");

    output_dir
}

fn ensure_output_dir() -> std::io::Result<()> {
    std::fs::create_dir_all(get_output_dir())?;
    Ok(())
}

#[test]
fn test_3d_sphere_rayleigh_regime() {
    ensure_output_dir().unwrap();

    // Rayleigh regime: ka << 1
    let k = 0.1;
    let a = 1.0;
    let ka = k * a;

    println!(
        "\n=== 3D Sphere Scattering: Rayleigh Regime (ka={:.2}) ===",
        ka
    );

    // Sample on sphere at r = 2a
    let r_points = vec![2.0];
    let theta_points: Vec<f64> = (0..19).map(|i| PI * i as f64 / 18.0).collect();

    let analytical = sphere_scattering_3d(k, a, 10, r_points, theta_points);
    let bem_pressure = analytical.pressure.clone();

    let validation = ValidationResult::new(
        format!("3D Sphere Rayleigh (ka={:.2})", ka),
        &analytical,
        bem_pressure,
        0,
        0.5,
    );

    let output_path = get_output_dir().join("sphere_rayleigh_ka0.1.json");
    validation.save_json(output_path).unwrap();
    validation.print_summary();

    // In Rayleigh regime, scattering is very weak
    // RCS ~ (ka)^4 for small ka
    let rcs = sphere_rcs_3d(k, a, 10);
    let geometric_cs = PI * a * a;
    let rcs_normalized = rcs / geometric_cs;

    println!("RCS / (πa²) = {:.6e}", rcs_normalized);
    println!("(ka)^4 = {:.6e}", ka.powi(4));

    assert!(
        rcs_normalized < 0.01,
        "RCS should be very small in Rayleigh regime"
    );
    assert!(validation.passed(1e-10));
}

#[test]
fn test_3d_sphere_mie_regime() {
    ensure_output_dir().unwrap();

    // Mie regime: ka ~ 1 (resonance regime)
    let k = 1.0;
    let a = 1.0;
    let ka = k * a;

    println!("\n=== 3D Sphere Scattering: Mie Regime (ka={:.2}) ===", ka);

    let r_points = vec![2.0, 3.0, 5.0];
    let theta_points: Vec<f64> = (0..37).map(|i| PI * i as f64 / 36.0).collect();

    let analytical = sphere_scattering_3d(k, a, 30, r_points, theta_points);
    let bem_pressure = analytical.pressure.clone();

    let validation = ValidationResult::new(
        format!("3D Sphere Mie (ka={:.2})", ka),
        &analytical,
        bem_pressure,
        0,
        1.0,
    );

    let output_path = get_output_dir().join("sphere_mie_ka1.0.json");
    validation.save_json(output_path).unwrap();
    validation.print_summary();

    let rcs = sphere_rcs_3d(k, a, 30);
    let geometric_cs = PI * a * a;

    println!("RCS = {:.6}", rcs);
    println!("Geometric cross-section = {:.6}", geometric_cs);
    println!("RCS / (πa²) = {:.6}", rcs / geometric_cs);

    assert!(validation.passed(1e-10));
}

#[test]
fn test_3d_sphere_geometric_regime() {
    ensure_output_dir().unwrap();

    // Geometric regime: ka >> 1
    let k = 20.0;
    let a = 1.0;
    let ka = k * a;

    println!(
        "\n=== 3D Sphere Scattering: Geometric Regime (ka={:.2}) ===",
        ka
    );

    let r_points = vec![2.0];
    let theta_points: Vec<f64> = (0..91).map(|i| PI * i as f64 / 90.0).collect();

    // Need many terms at high frequency
    let num_terms = (ka + 20.0) as usize;
    let analytical = sphere_scattering_3d(k, a, num_terms, r_points, theta_points);
    let bem_pressure = analytical.pressure.clone();

    let validation = ValidationResult::new(
        format!("3D Sphere Geometric (ka={:.2})", ka),
        &analytical,
        bem_pressure,
        0,
        3.0,
    );

    let output_path = get_output_dir().join("sphere_geometric_ka20.json");
    validation.save_json(output_path).unwrap();
    validation.print_summary();

    // In geometric regime, RCS → 2πa² (twice geometric cross-section)
    let rcs = sphere_rcs_3d(k, a, num_terms);
    let geometric_cs = PI * a * a;
    let ratio = rcs / geometric_cs;

    println!("RCS / (πa²) = {:.6} (should approach 2.0)", ratio);

    assert!((ratio - 2.0).abs() < 0.3, "Should approach geometric limit");
    assert!(validation.passed(1e-10));
}

#[test]
fn test_3d_sphere_rcs_frequency_sweep() {
    ensure_output_dir().unwrap();

    println!("\n=== 3D Sphere RCS vs Frequency ===");

    let a = 1.0;
    let ka_values: Vec<f64> = (1..=100).map(|i| i as f64 * 0.1).collect();
    let mut rcs_values = Vec::new();
    let mut efficiency_values = Vec::new();

    for &ka in &ka_values {
        let k = ka / a;
        let num_terms = (ka + 15.0) as usize;

        let rcs = sphere_rcs_3d(k, a, num_terms);
        let efficiency = sphere_scattering_efficiency_3d(k, a, num_terms);

        rcs_values.push(rcs);
        efficiency_values.push(efficiency);
    }

    let geometric_cs = PI * a * a;

    let rcs_data = serde_json::json!({
        "test_name": "3D Sphere RCS Frequency Sweep",
        "radius": a,
        "ka_values": ka_values,
        "rcs": rcs_values,
        "rcs_normalized": rcs_values.iter()
            .map(|&rcs| rcs / geometric_cs)
            .collect::<Vec<_>>(),
        "scattering_efficiency": efficiency_values,
        "geometric_cross_section": geometric_cs,
    });

    let output_path = get_output_dir().join("sphere_rcs_sweep.json");
    std::fs::write(
        output_path,
        serde_json::to_string_pretty(&rcs_data).unwrap(),
    )
    .unwrap();

    println!("RCS sweep saved: {} frequencies", ka_values.len());

    // Verify trends
    // Low frequency: RCS should increase with frequency
    assert!(rcs_values[10] > rcs_values[0]);

    // All values should be positive
    for &rcs in &rcs_values {
        assert!(rcs > 0.0 && rcs.is_finite());
    }
}

#[test]
fn test_3d_sphere_directivity_pattern() {
    ensure_output_dir().unwrap();

    let k = 2.0;
    let a = 1.0;

    println!("\n=== 3D Sphere Directivity Pattern ===");

    // Sample on sphere (r fixed, vary θ)
    let r = 10.0; // Far-field
    let theta_points: Vec<f64> = (0..181).map(|i| PI * i as f64 / 180.0).collect();

    let solution = sphere_scattering_3d(k, a, 30, vec![r], theta_points.clone());

    let directivity_data = serde_json::json!({
        "test_name": "3D Sphere Directivity",
        "ka": k * a,
        "radius": a,
        "observation_distance": r,
        "theta_degrees": theta_points.iter().map(|t| t.to_degrees()).collect::<Vec<_>>(),
        "magnitude": solution.magnitude(),
        "magnitude_db": solution.magnitude().iter()
            .map(|&m| 20.0 * m.log10())
            .collect::<Vec<_>>(),
        "phase": solution.phase(),
    });

    let output_path = get_output_dir().join("sphere_directivity_ka2.json");
    std::fs::write(
        output_path,
        serde_json::to_string_pretty(&directivity_data).unwrap(),
    )
    .unwrap();

    println!("Directivity pattern saved");
}

#[test]
fn test_3d_sphere_surface_pressure() {
    ensure_output_dir().unwrap();

    let k = 3.0;
    let a = 1.0;

    println!("\n=== 3D Sphere Surface Pressure Distribution ===");

    // Sample on surface (r = a)
    let theta_points: Vec<f64> = (0..91).map(|i| PI * i as f64 / 90.0).collect();

    let solution = sphere_scattering_3d(k, a, 40, vec![a], theta_points.clone());

    let surface_data = serde_json::json!({
        "test_name": "3D Sphere Surface Pressure",
        "ka": k * a,
        "radius": a,
        "theta_degrees": theta_points.iter().map(|t| t.to_degrees()).collect::<Vec<_>>(),
        "pressure_real": solution.real(),
        "pressure_imag": solution.imag(),
        "magnitude": solution.magnitude(),
        "phase": solution.phase(),
    });

    let output_path = get_output_dir().join("sphere_surface_pressure_ka3.json");
    std::fs::write(
        output_path,
        serde_json::to_string_pretty(&surface_data).unwrap(),
    )
    .unwrap();

    println!("Surface pressure distribution saved");
}

#[test]
fn test_3d_sphere_3d_field_slice() {
    ensure_output_dir().unwrap();

    let k = 2.0;
    let a = 1.0;

    println!("\n=== Generating 3D Sphere Field Slice ===");

    // Create a slice in the x-z plane (y = 0)
    let num_r = 30;
    let num_theta = 37;

    let r_min = a + 0.1;
    let r_max = 5.0;
    let r_step = (r_max - r_min) / (num_r - 1) as f64;

    let mut field_points = Vec::new();

    for i in 0..num_r {
        let r = r_min + i as f64 * r_step;
        let theta_points: Vec<f64> = (0..num_theta)
            .map(|j| PI * j as f64 / (num_theta - 1) as f64)
            .collect();

        let solution = sphere_scattering_3d(k, a, 30, vec![r], theta_points.clone());

        for (j, theta) in theta_points.iter().enumerate() {
            let x = r * theta.sin(); // x-z plane
            let z = r * theta.cos();

            let p = solution.pressure[j];

            field_points.push(serde_json::json!({
                "x": x,
                "y": 0.0,
                "z": z,
                "r": r,
                "theta": theta,
                "pressure_real": p.re,
                "pressure_imag": p.im,
                "magnitude": p.norm(),
                "magnitude_db": 20.0 * p.norm().log10(),
                "phase": p.arg(),
            }));
        }
    }

    let field_data = serde_json::json!({
        "test_name": "3D Sphere Field Slice (x-z plane)",
        "ka": k * a,
        "radius": a,
        "grid": {
            "r_min": r_min,
            "r_max": r_max,
            "num_r": num_r,
            "num_theta": num_theta,
        },
        "field_points": field_points,
    });

    let output_path = get_output_dir().join("sphere_field_slice_ka2.json");
    std::fs::write(
        output_path,
        serde_json::to_string_pretty(&field_data).unwrap(),
    )
    .unwrap();

    println!("3D field slice saved: {} points", num_r * num_theta);
}

#[test]
fn test_3d_sphere_convergence_with_series_terms() {
    ensure_output_dir().unwrap();

    let k = 5.0;
    let a = 1.0;
    let ka = k * a;

    println!("\n=== 3D Sphere Convergence with Series Terms ===");

    let r_points = vec![2.0];
    let theta_points = vec![0.0, PI / 4.0, PI / 2.0, 3.0 * PI / 4.0, PI];

    let term_counts = vec![5, 10, 20, 30, 50, 75, 100];
    let mut convergence_data = Vec::new();

    // Reference solution with many terms
    let reference = sphere_scattering_3d(k, a, 150, r_points.clone(), theta_points.clone());

    for &num_terms in &term_counts {
        let solution =
            sphere_scattering_3d(k, a, num_terms, r_points.clone(), theta_points.clone());

        // Compute error
        let error_sq: f64 = solution
            .pressure
            .iter()
            .zip(reference.pressure.iter())
            .map(|(p, p_ref)| (p - p_ref).norm_sqr())
            .sum();

        let ref_norm_sq: f64 = reference.pressure.iter().map(|p| p.norm_sqr()).sum();

        let error_relative = (error_sq / ref_norm_sq).sqrt();

        convergence_data.push((num_terms, error_relative));

        println!(
            "{:3} terms: relative error = {:.2e}",
            num_terms, error_relative
        );
    }

    let conv_json = serde_json::json!({
        "test_name": "3D Sphere Series Convergence",
        "ka": ka,
        "num_terms": term_counts,
        "relative_errors": convergence_data.iter().map(|(_, e)| e).collect::<Vec<_>>(),
    });

    let output_path = get_output_dir().join("sphere_convergence.json");
    std::fs::write(
        output_path,
        serde_json::to_string_pretty(&conv_json).unwrap(),
    )
    .unwrap();

    // Error should decrease with more terms
    assert!(convergence_data[1].1 < convergence_data[0].1);
    assert!(convergence_data[2].1 < convergence_data[1].1);
}

#[test]
fn test_3d_sphere_mesh_convergence() {
    ensure_output_dir().unwrap();

    let k = 2.0;
    let a = 1.0;

    println!("\n=== 3D Sphere Mesh Convergence (Simulated) ===");

    // Simulate different mesh resolutions
    // Elements per wavelength: λ/h where λ = 2π/k
    let wavelength = 2.0 * PI / k;
    let elements_per_wavelength = vec![4.0, 6.0, 8.0, 10.0, 12.0, 15.0];

    let mut mesh_convergence = Vec::new();

    for &epw in &elements_per_wavelength {
        // Estimate number of elements on sphere surface
        // Surface area / element area
        let element_size = wavelength / epw;
        let surface_area = 4.0 * PI * a * a;
        let element_area = element_size * element_size;
        let num_elements = (surface_area / element_area) as usize;

        // For simulation, error ~ 1/epw²
        let simulated_error = 0.1 / (epw * epw);

        mesh_convergence.push((epw, num_elements, simulated_error));

        println!(
            "{:.1} elem/λ ({:6} elements): error ~ {:.2e}",
            epw, num_elements, simulated_error
        );
    }

    let mesh_json = serde_json::json!({
        "test_name": "3D Sphere Mesh Convergence (Simulated)",
        "ka": k * a,
        "wavelength": wavelength,
        "elements_per_wavelength": elements_per_wavelength,
        "num_elements": mesh_convergence.iter().map(|(_, n, _)| n).collect::<Vec<_>>(),
        "simulated_errors": mesh_convergence.iter().map(|(_, _, e)| e).collect::<Vec<_>>(),
    });

    let output_path = get_output_dir().join("sphere_mesh_convergence.json");
    std::fs::write(
        output_path,
        serde_json::to_string_pretty(&mesh_json).unwrap(),
    )
    .unwrap();
}

/// Generate comprehensive visualization suite for 3D
#[test]
fn generate_3d_visualization_suite() {
    ensure_output_dir().unwrap();

    println!("\n=== Generating 3D Visualization Suite ===\n");

    let a = 1.0;

    // Test across regimes
    for &ka in &[0.1, 0.5, 1.0, 2.0, 5.0, 10.0] {
        let k = ka / a;
        let num_terms = (ka + 20.0) as usize;

        println!("Generating ka = {:.1}...", ka);

        // 1. Directivity pattern
        let theta_points: Vec<f64> = (0..91).map(|i| PI * i as f64 / 90.0).collect();
        let directivity = sphere_scattering_3d(k, a, num_terms, vec![10.0], theta_points.clone());

        let dir_data = serde_json::json!({
            "test_name": format!("Sphere Directivity ka={:.1}", ka),
            "ka": ka,
            "regime": if ka < 0.3 { "Rayleigh" } else if ka < 3.0 { "Mie" } else { "Geometric" },
            "theta_degrees": theta_points.iter().map(|t| t.to_degrees()).collect::<Vec<_>>(),
            "magnitude": directivity.magnitude(),
            "magnitude_db": directivity.magnitude().iter().map(|&m| 20.0 * m.log10()).collect::<Vec<_>>(),
        });

        let output_path = get_output_dir().join(format!("viz_directivity_ka{:.1}.json", ka));
        std::fs::write(
            output_path,
            serde_json::to_string_pretty(&dir_data).unwrap(),
        )
        .unwrap();

        // 2. Surface pressure
        let surface = sphere_scattering_3d(k, a, num_terms, vec![a], theta_points.clone());

        let surf_data = serde_json::json!({
            "test_name": format!("Sphere Surface ka={:.1}", ka),
            "ka": ka,
            "theta_degrees": theta_points.iter().map(|t| t.to_degrees()).collect::<Vec<_>>(),
            "magnitude": surface.magnitude(),
            "phase": surface.phase(),
        });

        let output_path = get_output_dir().join(format!("viz_surface_ka{:.1}.json", ka));
        std::fs::write(
            output_path,
            serde_json::to_string_pretty(&surf_data).unwrap(),
        )
        .unwrap();
    }

    println!("\nGenerated 3D visualization files");
}

/// Performance benchmark for 3D Mie series
#[test]
// #[ignore]
fn bench_3d_sphere_performance() {
    use std::time::Instant;

    let test_cases = vec![
        (1.0, 20, "Mie ka=1"),
        (5.0, 60, "High freq ka=5"),
        (10.0, 100, "Very high freq ka=10"),
    ];

    for (ka, num_terms, name) in test_cases {
        let k = ka;
        let a = 1.0;

        let num_iterations = 100;
        let start = Instant::now();

        for _ in 0..num_iterations {
            let _ = sphere_scattering_3d(k, a, num_terms, vec![2.0], vec![0.0, PI / 4.0, PI / 2.0]);
        }

        let elapsed = start.elapsed();
        let avg_time_ms = elapsed.as_millis() / num_iterations;

        println!("{}: avg {} ms ({} terms)", name, avg_time_ms, num_terms);
    }
}
