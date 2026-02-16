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

    // 2. Create config with broadband matching enabled
    let config = serde_json::json!({
        "version": "1.3.0",
        "speakers": {
            "left": {
                "path": measurement_path.to_str().unwrap()
            }
        },
        "target_curve": "flat",
        "optimizer": {
            "broadband_target_matching": {
                "enabled": true
            },
            "num_filters": 0, // Disable PEQ to verify broadband only (or see mixed results)
             // We give it 0 filters so the only correction should come from broadband matching!
             // Wait, optimize_channel_eq might fail with 0 filters if not handled.
             // Let's allow small number of filters.
            "min_freq": 20.0,
            "max_freq": 20000.0
        }
    });
    fs::write(&config_path, serde_json::to_string(&config).unwrap()).expect("Failed to write config");

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
    let json: serde_json::Value = serde_json::from_str(&json_str).expect("Failed to parse output JSON");

    let left = &json["channels"]["left"];
    let plugins = left["plugins"].as_array().expect("plugins array");

    // We expect a gain plugin and/or EQ plugin from broadband matching.
    // Since we boosted bass by 10dB, we expect a LowShelf cut (negative gain).
    
    // Find EQ plugins
    let eq_plugins: Vec<&serde_json::Value> = plugins.iter()
        .filter(|p| p["plugin_type"] == "eq")
        .collect();
    
    assert!(!eq_plugins.is_empty(), "Should have at least one EQ plugin (broadband or optimizer)");

    // Check for Gain plugin
    let gain_plugins: Vec<&serde_json::Value> = plugins.iter()
        .filter(|p| p["plugin_type"] == "gain")
        .collect();
    assert!(!gain_plugins.is_empty(), "Should have a gain plugin from broadband matching");
    let gain_db = gain_plugins[0]["parameters"]["gain_db"].as_f64().expect("gain_db");
    eprintln!("Found gain: {} dB", gain_db);
    assert!(gain_db < -50.0, "Gain should be significantly negative to match flat target level (approx -80dB)");

    // Check for EQ plugin (broadband matching should add LS + HS = 2 filters)
    // Note: Biquads serialize as coefficients, so we can't check "type".
    // But we know broadband matching adds exactly 2 filters in one plugin if enabled.
    let eq_plugins: Vec<&serde_json::Value> = plugins.iter() 
        .filter(|p| p["plugin_type"] == "eq")
        .collect();
    assert!(!eq_plugins.is_empty(), "Should have an EQ plugin");
    
    // The broadband EQ plugin should be the first EQ plugin (or unique if num_filters=0)
    let bb_plugin = eq_plugins[0];
    let filters = bb_plugin["parameters"]["filters"].as_array().expect("filters array");
    eprintln!("Found {} filters in first EQ plugin", filters.len());
    
    // We expect at least the 2 broadband filters. 
    // If optimize_channel_eq ran with num_filters=0, it might return empty filters or not add a plugin?
    // roomeq optimize.rs adds broadband_plugins distinct from optimizer chain.
    // So we should see broadband plugins.
    
    assert!(filters.len() >= 2, "Expected at least 2 filters (LS + HS) from broadband matching");
}
