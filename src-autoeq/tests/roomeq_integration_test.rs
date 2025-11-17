//! Integration tests for the roomeq binary

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use tempfile::TempDir;

/// Get the path to the roomeq binary
fn get_roomeq_binary() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.pop(); // Go to workspace root
    path.push("target");

    // Try debug first, then release
    let debug_path = path.join("debug/roomeq");
    let release_path = path.join("release/roomeq");

    if debug_path.exists() {
        debug_path
    } else if release_path.exists() {
        release_path
    } else {
        panic!("roomeq binary not found. Please build with 'cargo build --bin roomeq'");
    }
}

#[test]
fn test_roomeq_stereo_config() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let output_path = temp_dir.path().join("output.json");

    let config_path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/roomeq/test_config_stereo.json");

    // Run roomeq binary
    let output = Command::new(get_roomeq_binary())
        .arg("--config")
        .arg(&config_path)
        .arg("--output")
        .arg(&output_path)
        .arg("--sample-rate")
        .arg("48000")
        .output()
        .expect("Failed to execute roomeq");

    // Check that it ran successfully
    if !output.status.success() {
        eprintln!("stdout: {}", String::from_utf8_lossy(&output.stdout));
        eprintln!("stderr: {}", String::from_utf8_lossy(&output.stderr));
        panic!("roomeq failed with status: {}", output.status);
    }

    // Verify output file was created
    assert!(output_path.exists(), "Output file was not created");

    // Parse and validate output
    let json_str = fs::read_to_string(&output_path).expect("Failed to read output file");
    let json: serde_json::Value =
        serde_json::from_str(&json_str).expect("Failed to parse output JSON");

    // Verify structure
    assert!(json.get("channels").is_some(), "Missing 'channels' field");
    assert!(json.get("metadata").is_some(), "Missing 'metadata' field");

    let channels = json["channels"]
        .as_object()
        .expect("channels should be an object");

    // Should have left and right channels
    assert!(channels.contains_key("left"), "Missing 'left' channel");
    assert!(channels.contains_key("right"), "Missing 'right' channel");

    // Validate left channel has plugins
    let left_channel = &channels["left"];
    assert!(
        left_channel.get("channel").is_some(),
        "Missing channel name"
    );
    assert!(left_channel.get("plugins").is_some(), "Missing plugins");

    let plugins = left_channel["plugins"]
        .as_array()
        .expect("plugins should be an array");

    // Should have at least an EQ plugin
    assert!(!plugins.is_empty(), "No plugins in DSP chain");

    // Check for EQ plugin
    let has_eq = plugins.iter().any(|p| {
        p.get("plugin_type")
            .and_then(|t| t.as_str())
            .map(|t| t == "eq")
            .unwrap_or(false)
    });
    assert!(has_eq, "Missing EQ plugin in DSP chain");
}

#[test]
fn test_roomeq_multidriver_config() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let output_path = temp_dir.path().join("output_multidriver.json");

    let config_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data/roomeq/test_config_multidriver.json");

    // Run roomeq binary
    let output = Command::new(get_roomeq_binary())
        .arg("--config")
        .arg(&config_path)
        .arg("--output")
        .arg(&output_path)
        .arg("--sample-rate")
        .arg("48000")
        .arg("--verbose")
        .output()
        .expect("Failed to execute roomeq");

    // Check that it ran successfully
    if !output.status.success() {
        eprintln!("stdout: {}", String::from_utf8_lossy(&output.stdout));
        eprintln!("stderr: {}", String::from_utf8_lossy(&output.stderr));
        panic!("roomeq failed with status: {}", output.status);
    }

    // Verify output file was created
    assert!(output_path.exists(), "Output file was not created");

    // Parse and validate output
    let json_str = fs::read_to_string(&output_path).expect("Failed to read output file");
    let json: serde_json::Value =
        serde_json::from_str(&json_str).expect("Failed to parse output JSON");

    // Verify structure
    let channels = json["channels"]
        .as_object()
        .expect("channels should be an object");

    // Should have left channel
    assert!(channels.contains_key("left"), "Missing 'left' channel");

    let left_channel = &channels["left"];
    let plugins = left_channel["plugins"]
        .as_array()
        .expect("plugins should be an array");

    // Should have plugins (gain + EQ at minimum)
    assert!(!plugins.is_empty(), "No plugins in multi-driver DSP chain");

    // Verify we can parse the optimizer metadata
    let metadata = json["metadata"]
        .as_object()
        .expect("metadata should be an object");
    assert!(
        metadata.contains_key("algorithm"),
        "Missing algorithm in metadata"
    );
    assert!(
        metadata.contains_key("iterations"),
        "Missing iterations in metadata"
    );
}

#[test]
fn test_roomeq_invalid_config() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let output_path = temp_dir.path().join("output_invalid.json");
    let config_path = temp_dir.path().join("invalid_config.json");

    // Create invalid config
    fs::write(&config_path, r#"{"invalid": "config"}"#).expect("Failed to write invalid config");

    // Run roomeq binary - should fail
    let output = Command::new(get_roomeq_binary())
        .arg("--config")
        .arg(&config_path)
        .arg("--output")
        .arg(&output_path)
        .output()
        .expect("Failed to execute roomeq");

    // Should fail
    assert!(
        !output.status.success(),
        "roomeq should fail with invalid config"
    );
}

#[test]
fn test_roomeq_missing_measurement() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let output_path = temp_dir.path().join("output_missing.json");
    let config_path = temp_dir.path().join("missing_measurement_config.json");

    // Create config with non-existent measurement
    let config = serde_json::json!({
        "speakers": {
            "left": "nonexistent_file.csv"
        },
        "optimizer": {
            "num_filters": 3,
            "algorithm": "nlopt:cobyla",
            "max_iter": 100,
            "min_freq": 20.0,
            "max_freq": 20000.0,
            "min_q": 0.5,
            "max_q": 10.0,
            "min_db": -12.0,
            "max_db": 12.0,
            "loss_type": "flat"
        }
    });

    fs::write(&config_path, serde_json::to_string(&config).unwrap())
        .expect("Failed to write config");

    // Run roomeq binary - should fail
    let output = Command::new(get_roomeq_binary())
        .arg("--config")
        .arg(&config_path)
        .arg("--output")
        .arg(&output_path)
        .output()
        .expect("Failed to execute roomeq");

    // Should fail
    assert!(
        !output.status.success(),
        "roomeq should fail with missing measurement file"
    );
}

#[test]
fn test_roomeq_help() {
    // Test that --help works
    let output = Command::new(get_roomeq_binary())
        .arg("--help")
        .output()
        .expect("Failed to execute roomeq --help");

    assert!(output.status.success(), "roomeq --help should succeed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Automatic equalization"),
        "Help text should contain description"
    );
    assert!(
        stdout.contains("--config"),
        "Help text should mention --config"
    );
    assert!(
        stdout.contains("--output"),
        "Help text should mention --output"
    );
}
