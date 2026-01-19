use assert_fs::TempDir;
use assert_fs::prelude::*;
use std::path::PathBuf;
use std::process::Command;

/// Get the path to the autoeq binary
fn get_autoeq_binary() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.pop();
    path.push("target");

    let debug = path.join("debug/autoeq");
    let release = path.join("release/autoeq");

    if debug.exists() { debug } else { release }
}

#[test]
fn test_apo_output_format_golden() {
    let temp_dir = TempDir::new().unwrap();

    // Create test measurement
    let csv = temp_dir.child("test.csv");
    csv.write_str("freq,spl\n100,75.0\n500,78.0\n1000,80.0\n5000,75.0\n10000,72.0\n")
        .unwrap();

    let output = temp_dir.child("output");

    let status = Command::new(get_autoeq_binary())
        .args(&[
            "--curve",
            csv.path().to_str().unwrap(),
            "--output",
            output.path().to_str().unwrap(),
            "--num-filters",
            "3",
        ])
        .status()
        .expect("Failed to run autoeq");

    assert!(status.success());

    // Check APO file structure
    // The output file is generated in the same directory as the plot output
    // Since output is "temp_dir/output", the parent is temp_dir.
    let apo = temp_dir.child("iir-autoeq-flat.txt");
    assert!(apo.exists());

    let content = std::fs::read_to_string(apo.path()).unwrap();

    // Golden assertions - structure should not change
    assert!(content.starts_with("# AutoEQ"));
    assert!(content.contains("Filter"));
    assert!(content.contains("ON PK")); // Check for standard APO filter format
    
    // Check that we have 3 filters (count occurrences of "Filter")
    let filter_count = content.lines().filter(|l| l.trim().starts_with("Filter")).count();
    assert_eq!(filter_count, 3, "Should have exactly 3 filters");

    assert!(content.contains("Fc")); // Frequency marker
    assert!(content.contains("Q")); // Q marker
    assert!(content.contains("Gain")); // Gain marker
}

#[test]
fn test_peq_parameters_reasonable() {
    let temp_dir = TempDir::new().unwrap();

    // Create measurement with a peak
    let csv = temp_dir.child("test.csv");
    csv.write_str("freq,spl\n100,75.0\n200,80.0\n300,85.0\n400,82.0\n500,78.0\n1000,75.0\n")
        .unwrap();

    let output = temp_dir.child("output");

    let _ = Command::new(get_autoeq_binary())
        .args(&[
            "--curve",
            csv.path().to_str().unwrap(),
            "--output",
            output.path().to_str().unwrap(),
            "--num-filters",
            "2",
        ])
        .output()
        .expect("Failed to run autoeq");

    let apo = temp_dir.child("iir-autoeq-flat.txt");
    let content = std::fs::read_to_string(apo.path()).unwrap();

    // Parse PEQ parameters and verify they are reasonable
    let lines: Vec<&str> = content.lines().collect();

    // Find filter lines (starts with "Filter")
    let mut filter_lines = Vec::new();
    for line in &lines {
        if line.starts_with("Filter") && line.contains("ON PK") {
            filter_lines.push(*line);
        }
    }

    assert!(filter_lines.len() >= 2, "Should have at least 2 filters");

    // Verify each filter has valid parameters
    // Format example: "Filter  1: ON PK Fc 300.00 Hz Gain -4.00 dB Q 2.00"
    for filter_line in filter_lines {
        let parts: Vec<&str> = filter_line.split_whitespace().collect();
        
        // Find indices of parameters
        let fc_idx = parts.iter().position(|&r| r == "Fc");
        let gain_idx = parts.iter().position(|&r| r == "Gain");
        let q_idx = parts.iter().position(|&r| r == "Q");

        assert!(fc_idx.is_some(), "Missing Fc parameter");
        assert!(gain_idx.is_some(), "Missing Gain parameter");
        assert!(q_idx.is_some(), "Missing Q parameter");

        // Check Frequency
        let freq_str = parts[fc_idx.unwrap() + 1];
        let freq: f64 = freq_str.parse().expect("Failed to parse frequency");
        assert!(freq >= 20.0 && freq <= 20000.0, "Frequency out of range: {}", freq);

        // Check Gain
        let gain_str = parts[gain_idx.unwrap() + 1];
        let gain: f64 = gain_str.parse().expect("Failed to parse gain");
        assert!(gain >= -20.0 && gain <= 20.0, "Gain out of range: {}", gain);

        // Check Q
        let q_str = parts[q_idx.unwrap() + 1];
        let q: f64 = q_str.parse().expect("Failed to parse Q");
        assert!(q >= 0.1 && q <= 50.0, "Q out of range: {}", q);
    }
}
