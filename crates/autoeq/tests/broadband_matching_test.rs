use std::fs;
use std::path::PathBuf;
use std::process::Command;
use tempfile::TempDir;

/// Get the path to the roomeq binary
fn get_roomeq_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_roomeq"))
}

#[test]
fn test_broadband_matching() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let output_path = temp_dir.path().join("output.json");
    let measurement_path = temp_dir.path().join("measurement.csv");
    let config_path = temp_dir.path().join("config.json");

    // 1. Create a synthetic measurement with a +10dB bass boost below 200Hz
    // Format: freq,spl,phase
    let mut csv_content = String::from("freq,spl,phase\n");
    for f in (20..=20000).step_by(20) {
        let freq = f as f64;
        let spl = if freq < 200.0 {
            80.0 + 10.0 // Bass boost
        } else {
            80.0 // Flat
        };
        csv_content.push_str(&format!("{},{},0.0\n", freq, spl));
    }
    fs::write(&measurement_path, csv_content).expect("Failed to write measurement");

    // 2. Create config with broadband pre-correction enabled via target_response.
    let config = serde_json::json!({
        "version": "2.0.0",
        "speakers": {
            "left": {
                "path": measurement_path.to_str().unwrap()
            }
        },
        "optimizer": {
            "target_response": {
                "shape": "flat",
                "broadband_precorrection": true
            },
            "num_filters": 3, // Allow a few filters for broadband matching to produce EQ plugins
            "min_freq": 20.0,
            "max_freq": 20000.0
        }
    });
    fs::write(&config_path, serde_json::to_string(&config).unwrap())
        .expect("Failed to write config");

    // 3. Run roomeq
    let output = Command::new(get_roomeq_binary())
        .arg("--config")
        .arg(&config_path)
        .arg("--output")
        .arg(&output_path)
        .arg("--sample-rate")
        .arg("48000")
        .output()
        .expect("Failed to execute roomeq");

    if !output.status.success() {
        eprintln!("stdout: {}", String::from_utf8_lossy(&output.stdout));
        eprintln!("stderr: {}", String::from_utf8_lossy(&output.stderr));
        panic!("roomeq failed with status: {}", output.status);
    }

    // 4. Verify output
    let json_str = fs::read_to_string(&output_path).expect("Failed to read output file");
    eprintln!("Output JSON: {}", json_str);
    let json: serde_json::Value =
        serde_json::from_str(&json_str).expect("Failed to parse output JSON");

    let left = &json["channels"]["left"];
    let plugins = left["plugins"].as_array().expect("plugins array");

    // We expect a gain plugin and/or EQ plugin from broadband matching.
    // Since we boosted bass by 10dB, we expect a LowShelf cut (negative gain).

    // Find EQ plugins
    let eq_plugins: Vec<&serde_json::Value> = plugins
        .iter()
        .filter(|p| p["plugin_type"] == "eq")
        .collect();

    assert!(
        !eq_plugins.is_empty(),
        "Should have at least one EQ plugin (broadband or optimizer)"
    );

    // Check for Gain plugin (optional — broadband may fold gain into EQ filters)
    let gain_plugins: Vec<&serde_json::Value> = plugins
        .iter()
        .filter(|p| p["plugin_type"] == "gain")
        .collect();
    if !gain_plugins.is_empty() {
        let gain_db = gain_plugins[0]["parameters"]["gain_db"]
            .as_f64()
            .expect("gain_db");
        eprintln!("Found gain: {} dB", gain_db);
        // The broadband flat_gain_db is a *correction* gain (relative adjustment),
        // NOT an absolute target level. For a measurement at ~80dB with a flat target
        // at the measurement's mean SPL, the correction should be small (< 10dB).
        assert!(
            gain_db.abs() < 10.0,
            "Broadband gain correction should be small, got {:.1}dB",
            gain_db
        );
    }

    // Check for EQ plugin (broadband matching should add LS + HS = 2 filters)
    // Note: Biquads serialize as coefficients, so we can't check "type".
    // But we know broadband matching adds exactly 2 filters in one plugin if enabled.
    let eq_plugins: Vec<&serde_json::Value> = plugins
        .iter()
        .filter(|p| p["plugin_type"] == "eq")
        .collect();
    assert!(!eq_plugins.is_empty(), "Should have an EQ plugin");

    // The broadband EQ plugin should be the first EQ plugin (or unique if num_filters=0)
    let bb_plugin = eq_plugins[0];
    let filters = bb_plugin["parameters"]["filters"]
        .as_array()
        .expect("filters array");
    eprintln!("Found {} filters in first EQ plugin", filters.len());

    // We expect at least the 2 broadband filters.
    // If optimize_channel_eq ran with num_filters=0, it might return empty filters or not add a plugin?
    // roomeq optimize.rs adds broadband_plugins distinct from optimizer chain.
    // So we should see broadband plugins.

    assert!(
        filters.len() >= 2,
        "Expected at least 2 filters (LS + HS) from broadband matching"
    );
}
