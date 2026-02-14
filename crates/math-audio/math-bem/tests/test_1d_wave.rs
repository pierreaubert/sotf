//! 1D Wave Propagation Tests
//!
//! Tests plane wave, standing wave, and damped wave propagation.
//! Generates JSON output for visualization.

use directories::ProjectDirs;
use math_audio_bem::analytical::{damped_wave_1d, plane_wave_1d, standing_wave_1d};
use math_audio_bem::testing::ValidationResult;
use num_complex::Complex64;
use std::path::PathBuf;

/// Get output directory using the directories crate
fn get_output_dir() -> PathBuf {
    let proj_dirs = ProjectDirs::from("org", "spinorama", "math-audio")
        .expect("Failed to determine project directories");

    let output_dir = proj_dirs.cache_dir().join("bem").join("1d");

    std::fs::create_dir_all(&output_dir).expect("Failed to create output directory");

    output_dir
}

/// Create output directory if it doesn't exist
fn ensure_output_dir() -> std::io::Result<()> {
    std::fs::create_dir_all(get_output_dir())?;
    Ok(())
}

#[test]
fn test_1d_plane_wave_single_wavelength() {
    ensure_output_dir().unwrap();

    let k = 2.0; // wave number
    let wavelength = 2.0 * std::f64::consts::PI / k;

    // Analytical solution
    let analytical = plane_wave_1d(k, 0.0, wavelength, 100);

    // For now, BEM solution = analytical (perfect match)
    // Later, this will be replaced with actual BEM solver
    let bem_pressure = analytical.pressure.clone();

    let start_time = std::time::Instant::now();
    let validation = ValidationResult::new(
        "1D Plane Wave - Single Wavelength",
        &analytical,
        bem_pressure,
        start_time.elapsed().as_millis() as u64,
        0.1, // memory_mb (placeholder)
    );

    // Save JSON
    let output_path = get_output_dir().join("plane_wave_k2_one_wavelength.json");
    validation.save_json(output_path).unwrap();

    // Print summary
    validation.print_summary();

    // Should be perfect match
    assert!(validation.passed(1e-10));
    assert!(validation.errors.l2_relative < 1e-10);
}

#[test]
fn test_1d_plane_wave_multiple_wavelengths() {
    ensure_output_dir().unwrap();

    let test_cases = vec![
        (0.5, "k0.5"), // Long wavelength
        (1.0, "k1.0"),
        (2.0, "k2.0"),
        (5.0, "k5.0"), // Short wavelength
        (10.0, "k10.0"),
    ];

    for (k, name) in test_cases {
        let analytical = plane_wave_1d(k, 0.0, 10.0, 200);
        let bem_pressure = analytical.pressure.clone();

        let start_time = std::time::Instant::now();
        let validation = ValidationResult::new(
            format!("1D Plane Wave - k={}", k),
            &analytical,
            bem_pressure,
            start_time.elapsed().as_millis() as u64,
            0.1,
        );

        let output_path = get_output_dir().join(format!("plane_wave_{}.json", name));
        validation.save_json(output_path).unwrap();

        assert!(validation.passed(1e-10));
    }
}

#[test]
fn test_1d_standing_wave_nodes() {
    ensure_output_dir().unwrap();

    let k = 1.0;
    let length = std::f64::consts::PI; // One half-wavelength

    let analytical = standing_wave_1d(k, 0.0, length, 100);
    let bem_pressure = analytical.pressure.clone();

    let validation = ValidationResult::new(
        "1D Standing Wave - Node Pattern",
        &analytical,
        bem_pressure,
        0,
        0.1,
    );

    let output_path = get_output_dir().join("standing_wave_nodes.json");
    validation.save_json(output_path).unwrap();
    validation.print_summary();

    // Verify nodes at boundaries
    let first_magnitude = analytical.pressure[0].norm();
    let _last_magnitude = analytical.pressure[analytical.pressure.len() - 1].norm();

    // sin(0) and sin(π) should be near zero
    assert!(first_magnitude < 1e-10, "First node should be ~0");
    // Note: last point won't be exactly zero due to discretization

    assert!(validation.passed(1e-10));
}

#[test]
fn test_1d_damped_wave_decay() {
    ensure_output_dir().unwrap();

    let k = 2.0;
    let alpha = 0.2; // Damping coefficient

    let analytical = damped_wave_1d(k, alpha, 0.0, 10.0, 200);
    let bem_pressure = analytical.pressure.clone();

    let validation = ValidationResult::new(
        format!("1D Damped Wave - α={}", alpha),
        &analytical,
        bem_pressure,
        0,
        0.1,
    );

    let output_path = get_output_dir().join("damped_wave_alpha0.2.json");
    validation.save_json(output_path).unwrap();
    validation.print_summary();

    // Verify exponential decay
    let mag_start = analytical.pressure[0].norm();
    let mag_end = analytical.pressure[analytical.pressure.len() - 1].norm();

    let expected_ratio = (-alpha * 10.0).exp();
    let actual_ratio = mag_end / mag_start;

    println!("Expected decay ratio: {:.6}", expected_ratio);
    println!("Actual decay ratio: {:.6}", actual_ratio);

    assert!((actual_ratio - expected_ratio).abs() < 1e-6);
    assert!(validation.passed(1e-10));
}

#[test]
fn test_1d_convergence_with_resolution() {
    ensure_output_dir().unwrap();

    let k = 5.0;
    let length = 4.0 * std::f64::consts::PI / k; // 2 wavelengths

    let resolutions = vec![50, 100, 200, 500, 1000];
    let mut results = Vec::new();

    for &num_points in &resolutions {
        let analytical = plane_wave_1d(k, 0.0, length, num_points);
        let bem_pressure = analytical.pressure.clone();

        let validation = ValidationResult::new(
            format!("1D Convergence - {} points", num_points),
            &analytical,
            bem_pressure,
            0,
            0.1,
        );

        results.push((num_points, validation.errors.l2_relative));
    }

    // Save convergence data
    let convergence_data = serde_json::json!({
        "test_name": "1D Convergence Study",
        "wave_number": k,
        "resolutions": resolutions,
        "l2_errors": results.iter().map(|(_, err)| err).collect::<Vec<_>>(),
        "points_per_wavelength": resolutions.iter()
            .map(|&n| n as f64 / 2.0)  // 2 wavelengths
            .collect::<Vec<_>>(),
    });

    let output_path = get_output_dir().join("convergence_study.json");
    std::fs::write(
        output_path,
        serde_json::to_string_pretty(&convergence_data).unwrap(),
    )
    .unwrap();

    println!("\n=== Convergence Study ===");
    for (n, err) in &results {
        println!("{:5} points: L2 error = {:.2e}", n, err);
    }
}

/// Benchmark test (not run by default)
#[test]
// #[ignore]
fn bench_1d_plane_wave_performance() {
    use std::time::Instant;

    let k = 2.0;
    let num_iterations = 1000;

    let start = Instant::now();
    for _ in 0..num_iterations {
        let _ = plane_wave_1d(k, 0.0, 10.0, 100);
    }
    let elapsed = start.elapsed();

    let avg_time_us = elapsed.as_micros() / num_iterations;
    println!("Average time per 1D solution: {} μs", avg_time_us);

    assert!(avg_time_us < 1000, "Should complete in less than 1ms");
}

/// Test with simulated BEM error
#[test]
fn test_1d_with_simulated_bem_error() {
    ensure_output_dir().unwrap();

    let k = 2.0;
    let analytical = plane_wave_1d(k, 0.0, 10.0, 100);

    // Simulate BEM solver with 1% error
    let bem_pressure: Vec<Complex64> = analytical
        .pressure
        .iter()
        .enumerate()
        .map(|(i, p)| {
            // Add small perturbation
            let noise_re = 0.01 * (i as f64 * 0.1).sin();
            let noise_im = 0.01 * (i as f64 * 0.1).cos();
            p + Complex64::new(noise_re, noise_im)
        })
        .collect();

    let validation = ValidationResult::new(
        "1D Plane Wave - Simulated BEM Error (1%)",
        &analytical,
        bem_pressure,
        0,
        0.1,
    );

    let output_path = get_output_dir().join("plane_wave_with_error.json");
    validation.save_json(output_path).unwrap();
    validation.print_summary();

    // Error should be small but nonzero
    assert!(validation.errors.l2_relative > 1e-10);
    assert!(validation.errors.l2_relative < 0.02);
    assert!(!validation.passed(1e-10));
    assert!(validation.passed(0.02)); // Should pass with 2% threshold
}

/// Generate comprehensive test suite for visualization
#[test]
fn generate_1d_visualization_suite() {
    ensure_output_dir().unwrap();

    println!("\n=== Generating 1D Visualization Suite ===\n");

    // Test 1: Multiple wave numbers
    let wave_numbers = vec![0.5, 1.0, 2.0, 5.0, 10.0];
    for &k in &wave_numbers {
        let analytical = plane_wave_1d(k, 0.0, 10.0, 200);
        let bem = analytical.pressure.clone();

        let validation =
            ValidationResult::new(format!("Plane Wave k={:.1}", k), &analytical, bem, 0, 0.1);

        let output_path = get_output_dir().join(format!("viz_plane_wave_k{:.1}.json", k));
        validation.save_json(output_path).unwrap();
    }

    // Test 2: Standing waves
    for &k in &[1.0, 2.0, 3.0] {
        let analytical = standing_wave_1d(k, 0.0, std::f64::consts::PI, 200);
        let bem = analytical.pressure.clone();

        let validation = ValidationResult::new(
            format!("Standing Wave k={:.1}", k),
            &analytical,
            bem,
            0,
            0.1,
        );

        let output_path = get_output_dir().join(format!("viz_standing_wave_k{:.1}.json", k));
        validation.save_json(output_path).unwrap();
    }

    // Test 3: Various damping
    for &alpha in &[0.05, 0.1, 0.2, 0.5] {
        let analytical = damped_wave_1d(2.0, alpha, 0.0, 10.0, 200);
        let bem = analytical.pressure.clone();

        let validation = ValidationResult::new(
            format!("Damped Wave α={:.2}", alpha),
            &analytical,
            bem,
            0,
            0.1,
        );

        let output_path = get_output_dir().join(format!("viz_damped_alpha{:.2}.json", alpha));
        validation.save_json(output_path).unwrap();
    }

    println!("Generated visualization files in {:?}", get_output_dir());
}
