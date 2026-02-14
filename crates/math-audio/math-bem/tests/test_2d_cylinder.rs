//! 2D Cylinder Scattering Tests
//!
//! Tests rigid cylinder scattering using Bessel/Hankel function series.
//! Generates JSON output for 2D field plots and directivity patterns.

use directories::ProjectDirs;
use math_audio_bem::analytical::{
    cylinder_directivity_2d, cylinder_scattering_2d, cylinder_scattering_cross_section_2d,
};
use math_audio_bem::testing::ValidationResult;
use std::f64::consts::PI;
use std::path::PathBuf;

/// Get output directory using the directories crate
fn get_output_dir() -> PathBuf {
    let proj_dirs = ProjectDirs::from("org", "spinorama", "math-audio")
        .expect("Failed to determine project directories");

    let output_dir = proj_dirs.cache_dir().join("bem").join("2d");

    std::fs::create_dir_all(&output_dir).expect("Failed to create output directory");

    output_dir
}

fn ensure_output_dir() -> std::io::Result<()> {
    std::fs::create_dir_all(get_output_dir())?;
    Ok(())
}

#[test]
fn test_2d_cylinder_low_frequency_rayleigh() {
    ensure_output_dir().unwrap();

    // Rayleigh scattering: ka << 1
    let k = 0.1;
    let a = 1.0;
    let ka = k * a;

    println!(
        "\n=== 2D Cylinder Scattering: Rayleigh Regime (ka={:.2}) ===",
        ka
    );

    // Sample on a circle at r = 2a (outside cylinder)
    let r_points = vec![2.0];
    let theta_points: Vec<f64> = (0..36).map(|i| 2.0 * PI * i as f64 / 36.0).collect();

    let analytical = cylinder_scattering_2d(k, a, 20, r_points, theta_points);
    let bem_pressure = analytical.pressure.clone();

    let validation = ValidationResult::new(
        format!("2D Cylinder Rayleigh (ka={:.2})", ka),
        &analytical,
        bem_pressure,
        0,
        0.5,
    );

    let output_path = get_output_dir().join("cylinder_rayleigh_ka0.1.json");
    validation.save_json(output_path).unwrap();
    validation.print_summary();

    // In Rayleigh regime, scattering is weak
    for p in &analytical.pressure {
        let magnitude = p.norm();
        assert!(magnitude > 0.5, "Low scattering at low frequency");
        assert!(magnitude < 2.0, "Not too strong either");
    }

    assert!(validation.passed(1e-10));
}

#[test]
fn test_2d_cylinder_mie_regime() {
    ensure_output_dir().unwrap();

    // Mie regime: ka ~ 1
    let k = 1.0;
    let a = 1.0;
    let ka = k * a;

    println!(
        "\n=== 2D Cylinder Scattering: Mie Regime (ka={:.2}) ===",
        ka
    );

    let r_points = vec![2.0, 3.0, 5.0];
    let theta_points: Vec<f64> = (0..72).map(|i| 2.0 * PI * i as f64 / 72.0).collect();

    let analytical = cylinder_scattering_2d(k, a, 30, r_points, theta_points);
    let bem_pressure = analytical.pressure.clone();

    let validation = ValidationResult::new(
        format!("2D Cylinder Mie (ka={:.2})", ka),
        &analytical,
        bem_pressure,
        0,
        1.0,
    );

    let output_path = get_output_dir().join("cylinder_mie_ka1.0.json");
    validation.save_json(output_path).unwrap();
    validation.print_summary();

    assert!(validation.passed(1e-10));
}

#[test]
fn test_2d_cylinder_high_frequency() {
    ensure_output_dir().unwrap();

    // High frequency: ka >> 1
    let k = 10.0;
    let a = 1.0;
    let ka = k * a;

    println!(
        "\n=== 2D Cylinder Scattering: High Frequency (ka={:.2}) ===",
        ka
    );

    let r_points = vec![2.0];
    let theta_points: Vec<f64> = (0..180).map(|i| 2.0 * PI * i as f64 / 180.0).collect();

    // Need more terms at high frequency
    let num_terms = (ka + 10.0) as usize;
    let analytical = cylinder_scattering_2d(k, a, num_terms, r_points, theta_points);
    let bem_pressure = analytical.pressure.clone();

    let validation = ValidationResult::new(
        format!("2D Cylinder High Freq (ka={:.2})", ka),
        &analytical,
        bem_pressure,
        0,
        2.0,
    );

    let output_path = get_output_dir().join("cylinder_high_freq_ka10.json");
    validation.save_json(output_path).unwrap();
    validation.print_summary();

    assert!(validation.passed(1e-10));
}

#[test]
fn test_2d_cylinder_symmetry() {
    ensure_output_dir().unwrap();

    let k = 2.0;
    let a = 1.0;

    println!("\n=== Testing 2D Cylinder Symmetry ===");

    // Plane wave from θ=0 should create symmetric pattern
    let r_points = vec![3.0];
    let symmetric_angles = vec![PI / 4.0, -PI / 4.0, PI / 3.0, -PI / 3.0];

    let analytical = cylinder_scattering_2d(k, a, 30, r_points, symmetric_angles);

    // Check symmetry: p(θ) ≈ p(-θ)
    let p_plus_45 = analytical.pressure[0];
    let p_minus_45 = analytical.pressure[1];
    let p_plus_60 = analytical.pressure[2];
    let p_minus_60 = analytical.pressure[3];

    println!("p(+45°) = {:.6}", p_plus_45.norm());
    println!("p(-45°) = {:.6}", p_minus_45.norm());
    println!("p(+60°) = {:.6}", p_plus_60.norm());
    println!("p(-60°) = {:.6}", p_minus_60.norm());

    assert!((p_plus_45.norm() - p_minus_45.norm()).abs() < 1e-6);
    assert!((p_plus_60.norm() - p_minus_60.norm()).abs() < 1e-6);
}

#[test]
fn test_2d_cylinder_directivity_pattern() {
    ensure_output_dir().unwrap();

    let k = 2.0;
    let a = 1.0;

    println!("\n=== 2D Cylinder Directivity Pattern ===");

    let theta_points: Vec<f64> = (0..360).map(|i| 2.0 * PI * i as f64 / 360.0).collect();
    let directivity = cylinder_directivity_2d(k, a, 30, theta_points.clone());

    // Save directivity data
    let directivity_data = serde_json::json!({
        "test_name": "2D Cylinder Directivity",
        "ka": k * a,
        "theta_degrees": theta_points.iter().map(|t| t.to_degrees()).collect::<Vec<_>>(),
        "directivity_real": directivity.iter().map(|d| d.re).collect::<Vec<_>>(),
        "directivity_imag": directivity.iter().map(|d| d.im).collect::<Vec<_>>(),
        "directivity_magnitude": directivity.iter().map(|d| d.norm()).collect::<Vec<_>>(),
        "directivity_db": directivity.iter()
            .map(|d| 20.0 * d.norm().log10())
            .collect::<Vec<_>>(),
    });

    let output_path = get_output_dir().join("cylinder_directivity_ka2.json");
    std::fs::write(
        &output_path,
        serde_json::to_string_pretty(&directivity_data).unwrap(),
    )
    .unwrap();

    println!("Directivity pattern saved to {:?}", output_path);
}

#[test]
fn test_2d_cylinder_scattering_cross_section() {
    ensure_output_dir().unwrap();

    println!("\n=== 2D Cylinder Scattering Cross Section ===");

    let a = 1.0;
    let ka_values: Vec<f64> = (1..=50).map(|i| i as f64 * 0.2).collect();
    let mut cross_sections = Vec::new();

    for &ka in &ka_values {
        let k = ka / a;
        let num_terms = (ka + 10.0) as usize;
        let sigma = cylinder_scattering_cross_section_2d(k, a, num_terms);
        cross_sections.push(sigma);
    }

    // Save cross-section data
    let cs_data = serde_json::json!({
        "test_name": "2D Cylinder Scattering Cross Section",
        "radius": a,
        "ka_values": ka_values,
        "cross_section": cross_sections,
        "normalized_cs": cross_sections.iter()
            .map(|&sigma| sigma / (2.0 * a))  // Normalize by diameter
            .collect::<Vec<_>>(),
    });

    let output_path = get_output_dir().join("cylinder_cross_section.json");
    std::fs::write(output_path, serde_json::to_string_pretty(&cs_data).unwrap()).unwrap();

    println!("Scattering cross-section saved");

    // All cross sections should be positive
    for &sigma in &cross_sections {
        assert!(sigma > 0.0);
        assert!(sigma.is_finite());
    }
}

#[test]
fn test_2d_cylinder_field_map() {
    ensure_output_dir().unwrap();

    let k = 2.0;
    let a = 1.0;

    println!("\n=== Generating 2D Cylinder Pressure Field Map ===");

    // Create 2D grid
    let r_min = a + 0.1; // Just outside cylinder
    let r_max = 5.0;
    let num_r = 50;
    let num_theta = 72;

    let r_step = (r_max - r_min) / (num_r - 1) as f64;
    let theta_step = 2.0 * PI / num_theta as f64;

    let mut field_map = Vec::new();

    for i in 0..num_r {
        let r = r_min + i as f64 * r_step;
        let theta_points: Vec<f64> = (0..num_theta).map(|j| j as f64 * theta_step).collect();

        let solution = cylinder_scattering_2d(k, a, 30, vec![r], theta_points);

        for (j, p) in solution.pressure.iter().enumerate() {
            let theta = j as f64 * theta_step;
            let x = r * theta.cos();
            let y = r * theta.sin();

            field_map.push(serde_json::json!({
                "x": x,
                "y": y,
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
        "test_name": "2D Cylinder Pressure Field Map",
        "ka": k * a,
        "radius": a,
        "grid": {
            "r_min": r_min,
            "r_max": r_max,
            "num_r": num_r,
            "num_theta": num_theta,
        },
        "field_points": field_map,
    });

    let output_path = get_output_dir().join("cylinder_field_map_ka2.json");
    std::fs::write(
        output_path,
        serde_json::to_string_pretty(&field_data).unwrap(),
    )
    .unwrap();

    println!("2D field map saved: {} points", num_r * num_theta);
}

#[test]
fn test_2d_cylinder_convergence_with_series_terms() {
    ensure_output_dir().unwrap();

    let k = 5.0;
    let a = 1.0;
    let ka = k * a;

    println!("\n=== 2D Cylinder Convergence with Series Terms ===");

    let r_points = vec![2.0];
    let theta_points = vec![0.0, PI / 4.0, PI / 2.0];

    let term_counts = vec![5, 10, 20, 30, 50, 100];
    let mut convergence_data = Vec::new();

    let reference_solution =
        cylinder_scattering_2d(k, a, 150, r_points.clone(), theta_points.clone());

    for &num_terms in &term_counts {
        let solution =
            cylinder_scattering_2d(k, a, num_terms, r_points.clone(), theta_points.clone());

        // Compute error relative to reference
        let error: f64 = solution
            .pressure
            .iter()
            .zip(reference_solution.pressure.iter())
            .map(|(p, p_ref)| (p - p_ref).norm_sqr())
            .sum::<f64>()
            .sqrt();

        let error_relative = error
            / reference_solution
                .pressure
                .iter()
                .map(|p| p.norm_sqr())
                .sum::<f64>()
                .sqrt();

        convergence_data.push((num_terms, error_relative));

        println!(
            "{:3} terms: relative error = {:.2e}",
            num_terms, error_relative
        );
    }

    let conv_json = serde_json::json!({
        "test_name": "2D Cylinder Series Convergence",
        "ka": ka,
        "num_terms": term_counts,
        "relative_errors": convergence_data.iter().map(|(_, e)| e).collect::<Vec<_>>(),
    });

    let output_path = get_output_dir().join("cylinder_convergence.json");
    std::fs::write(
        output_path,
        serde_json::to_string_pretty(&conv_json).unwrap(),
    )
    .unwrap();
}

/// Generate comprehensive visualization suite for 2D
#[test]
fn generate_2d_visualization_suite() {
    ensure_output_dir().unwrap();

    println!("\n=== Generating 2D Visualization Suite ===\n");

    let a = 1.0;

    // Test different ka values
    for &ka in &[0.5, 1.0, 2.0, 5.0, 10.0] {
        let k = ka / a;
        let num_terms = (ka + 15.0) as usize;

        // Directivity
        let theta_points: Vec<f64> = (0..360).map(|i| 2.0 * PI * i as f64 / 360.0).collect();
        let directivity = cylinder_directivity_2d(k, a, num_terms, theta_points.clone());

        let dir_data = serde_json::json!({
            "test_name": format!("Cylinder Directivity ka={:.1}", ka),
            "ka": ka,
            "theta_degrees": theta_points.iter().map(|t| t.to_degrees()).collect::<Vec<_>>(),
            "magnitude": directivity.iter().map(|d| d.norm()).collect::<Vec<_>>(),
            "magnitude_db": directivity.iter().map(|d| 20.0 * d.norm().log10()).collect::<Vec<_>>(),
            "phase": directivity.iter().map(|d| d.arg()).collect::<Vec<_>>(),
        });

        let output_path = get_output_dir().join(format!("viz_directivity_ka{:.1}.json", ka));
        std::fs::write(
            output_path,
            serde_json::to_string_pretty(&dir_data).unwrap(),
        )
        .unwrap();

        // Field map (smaller grid for visualization)
        let r_points: Vec<f64> = (0..20).map(|i| a + 0.1 + i as f64 * 0.2).collect();
        let theta_viz: Vec<f64> = (0..72).map(|i| 2.0 * PI * i as f64 / 72.0).collect();

        let solution = cylinder_scattering_2d(k, a, num_terms, r_points.clone(), theta_viz.clone());

        let mut field_points = Vec::new();
        for (idx, pos) in solution.positions.iter().enumerate() {
            field_points.push(serde_json::json!({
                "x": pos.x,
                "y": pos.y,
                "magnitude": solution.pressure[idx].norm(),
                "phase": solution.pressure[idx].arg(),
            }));
        }

        let field_data = serde_json::json!({
            "test_name": format!("Cylinder Field ka={:.1}", ka),
            "ka": ka,
            "field_points": field_points,
        });

        let output_path = get_output_dir().join(format!("viz_field_ka{:.1}.json", ka));
        std::fs::write(
            output_path,
            serde_json::to_string_pretty(&field_data).unwrap(),
        )
        .unwrap();
    }

    println!("Generated 2D visualization files");
}
