//! Integration tests for the data generation pipeline
//!
//! Tests the full pipeline: scenario -> solver -> CSV export -> config generation

use autoeq_datagen::{bem_runner, csv_export, hf_extension, roomeq_config_gen, scenarios};
use tempfile::TempDir;

/// Helper: run full BEM pipeline for a given scenario name
fn run_bem_pipeline(scenario_name: &str) {
    let scenario = scenarios::scenario_by_name(scenario_name)
        .unwrap_or_else(|| panic!("Scenario '{scenario_name}' not found"));

    let output = bem_runner::run_bem(&scenario.simulation)
        .unwrap_or_else(|e| panic!("BEM solve failed for {scenario_name}: {e}"));

    // Verify output dimensions
    let n_freqs = output.frequencies.len();
    assert_eq!(n_freqs, 100, "Expected 100 frequency points");
    assert_eq!(
        output.source_names.len(),
        scenario.source_names.len(),
        "Source names count mismatch"
    );
    assert_eq!(
        output.pressures.len(),
        scenario.simulation.sources.len(),
        "Source count mismatch"
    );

    for (src_idx, src_pressures) in output.pressures.iter().enumerate() {
        assert_eq!(
            src_pressures.len(),
            scenario.simulation.listening_positions.len(),
            "LP count mismatch for source {src_idx}"
        );
        for (lp_idx, lp_pressures) in src_pressures.iter().enumerate() {
            assert_eq!(
                lp_pressures.len(),
                n_freqs,
                "Freq count mismatch for source {src_idx} lp {lp_idx}"
            );
        }
    }

    // Export CSVs
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let csv_files = csv_export::export_csvs(&output, temp_dir.path())
        .unwrap_or_else(|e| panic!("CSV export failed for {scenario_name}: {e}"));

    // Verify CSV file count: sources * listening_positions
    let expected_files =
        scenario.simulation.sources.len() * scenario.simulation.listening_positions.len();
    assert_eq!(
        csv_files.len(),
        expected_files,
        "Expected {expected_files} CSV files, got {}",
        csv_files.len()
    );

    // Verify CSV content
    for filename in &csv_files {
        let filepath = temp_dir.path().join(filename);
        let content = std::fs::read_to_string(&filepath)
            .unwrap_or_else(|e| panic!("Failed to read {filename}: {e}"));
        let lines: Vec<&str> = content.lines().collect();

        // Header + n_freqs data lines
        assert_eq!(
            lines.len(),
            n_freqs + 1,
            "Expected {} lines (header + {n_freqs} data) in {filename}",
            n_freqs + 1
        );
        assert_eq!(lines[0], "freq,spl,phase", "Bad header in {filename}");

        // Verify first data line is parseable
        let parts: Vec<&str> = lines[1].split(',').collect();
        assert_eq!(parts.len(), 3, "Expected 3 columns in {filename}");
        let freq: f64 = parts[0]
            .parse()
            .unwrap_or_else(|e| panic!("Bad freq in {filename}: {e}"));
        let spl: f64 = parts[1]
            .parse()
            .unwrap_or_else(|e| panic!("Bad spl in {filename}: {e}"));
        let phase: f64 = parts[2]
            .parse()
            .unwrap_or_else(|e| panic!("Bad phase in {filename}: {e}"));

        assert!(
            (19.0..=21.0).contains(&freq),
            "First freq should be ~20 Hz, got {freq}"
        );
        assert!(
            (-120.0..=150.0).contains(&spl),
            "SPL out of range in {filename}: {spl}"
        );
        assert!(
            (-180.0..=180.0).contains(&phase),
            "Phase out of range in {filename}: {phase}"
        );
    }

    // Generate roomeq config
    let config = roomeq_config_gen::generate_config(&scenario, temp_dir.path())
        .unwrap_or_else(|e| panic!("Config generation failed for {scenario_name}: {e}"));

    // Verify config structure
    assert!(!config.speakers.is_empty(), "Config should have speakers");
    assert_eq!(config.optimizer.num_filters, 7, "Expected 7 filters");
    assert_eq!(config.optimizer.seed, Some(42), "Expected seed 42");

    // Write and re-read config to verify serialization roundtrip
    let config_path = temp_dir.path().join("config.json");
    roomeq_config_gen::write_config(&config, &config_path)
        .unwrap_or_else(|e| panic!("Config write failed: {e}"));

    let json_str = std::fs::read_to_string(&config_path).expect("Failed to read config");
    let _parsed: autoeq::roomeq::RoomConfig =
        serde_json::from_str(&json_str).expect("Failed to parse written config");
}

#[test]
fn test_bem_pipeline_small_stereo() {
    run_bem_pipeline("small_stereo_2_0");
}

#[test]
fn test_bem_pipeline_small_2_1() {
    run_bem_pipeline("small_stereo_2_1");
}

#[test]
fn test_bem_pipeline_small_multi_sub() {
    run_bem_pipeline("small_stereo_2_2_mso");
}

#[test]
fn test_bem_pipeline_medium_multi_seat() {
    run_bem_pipeline("medium_multi_seat");
}

#[test]
fn test_config_stereo_has_left_right() {
    let scenario = scenarios::scenario_by_name("small_stereo_2_0").unwrap();
    let temp_dir = TempDir::new().unwrap();
    let config = roomeq_config_gen::generate_config(&scenario, temp_dir.path()).unwrap();

    assert!(
        config.speakers.contains_key("left"),
        "Missing 'left' speaker"
    );
    assert!(
        config.speakers.contains_key("right"),
        "Missing 'right' speaker"
    );
    assert!(
        !config.speakers.contains_key("lfe"),
        "Stereo should not have LFE"
    );
}

#[test]
fn test_config_2_1_has_lfe() {
    let scenario = scenarios::scenario_by_name("small_stereo_2_1").unwrap();
    let temp_dir = TempDir::new().unwrap();
    let config = roomeq_config_gen::generate_config(&scenario, temp_dir.path()).unwrap();

    assert!(config.speakers.contains_key("left"));
    assert!(config.speakers.contains_key("right"));
    assert!(config.speakers.contains_key("lfe"), "2.1 should have LFE");
}

#[test]
fn test_config_multi_sub_has_multisub_lfe() {
    let scenario = scenarios::scenario_by_name("small_stereo_2_2_mso").unwrap();
    let temp_dir = TempDir::new().unwrap();
    let config = roomeq_config_gen::generate_config(&scenario, temp_dir.path()).unwrap();

    assert!(
        config.speakers.contains_key("lfe"),
        "Multi-sub should have LFE"
    );
    match &config.speakers["lfe"] {
        autoeq::roomeq::SpeakerConfig::MultiSub(group) => {
            assert_eq!(group.subwoofers.len(), 2, "Expected 2 subs");
        }
        other => panic!("Expected MultiSub config for LFE, got {other:?}"),
    }
}

#[test]
fn test_hf_extension_pipeline() {
    let scenario = scenarios::scenario_by_name("small_stereo_2_1").expect("Scenario not found");

    let sim_output = bem_runner::run_bem(&scenario.simulation).expect("BEM solve failed");

    assert_eq!(
        sim_output.frequencies.len(),
        100,
        "Simulation should have 100 points"
    );

    let extended = hf_extension::extend_to_full_range(&sim_output);

    // 100 simulation + 100 HF extension = 200
    assert_eq!(
        extended.frequencies.len(),
        200,
        "Extended should have 200 points"
    );
    assert!(
        extended.frequencies[0] >= 19.0,
        "First freq should be ~20 Hz"
    );
    assert!(
        *extended.frequencies.last().unwrap() > 19000.0,
        "Last freq should be ~20 kHz"
    );

    // Source count preserved
    assert_eq!(extended.source_names.len(), sim_output.source_names.len());
    assert_eq!(extended.pressures.len(), sim_output.pressures.len());

    // All pressure arrays extended
    for (src_idx, src_pressures) in extended.pressures.iter().enumerate() {
        for (lp_idx, lp_pressures) in src_pressures.iter().enumerate() {
            assert_eq!(
                lp_pressures.len(),
                200,
                "Source {src_idx} LP {lp_idx} should have 200 points"
            );
        }
    }

    // Export extended CSVs and verify
    let temp_dir = tempfile::TempDir::new().unwrap();
    let csv_files = csv_export::export_csvs(&extended, temp_dir.path()).unwrap();

    for filename in &csv_files {
        let filepath = temp_dir.path().join(filename);
        let content = std::fs::read_to_string(&filepath).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 201, "Header + 200 data lines in {filename}");

        // Last line should have freq near 20 kHz
        let last_parts: Vec<&str> = lines[200].split(',').collect();
        let last_freq: f64 = last_parts[0].parse().unwrap();
        assert!(
            last_freq > 19000.0,
            "Last CSV freq should be ~20 kHz, got {last_freq}"
        );
    }
}

#[test]
fn test_config_5_0_has_surround_channels() {
    let scenario = scenarios::scenario_by_name("medium_surround_5_0").unwrap();
    let temp_dir = TempDir::new().unwrap();
    let config = roomeq_config_gen::generate_config(&scenario, temp_dir.path()).unwrap();

    assert!(config.speakers.contains_key("left"), "Missing 'left'");
    assert!(config.speakers.contains_key("right"), "Missing 'right'");
    assert!(config.speakers.contains_key("center"), "Missing 'center'");
    assert!(
        config.speakers.contains_key("surround_left"),
        "Missing 'surround_left'"
    );
    assert!(
        config.speakers.contains_key("surround_right"),
        "Missing 'surround_right'"
    );
    assert!(
        !config.speakers.contains_key("lfe"),
        "5.0 should not have LFE"
    );
}

#[test]
fn test_config_5_1_has_surround_and_lfe() {
    let scenario = scenarios::scenario_by_name("medium_surround_5_1").unwrap();
    let temp_dir = TempDir::new().unwrap();
    let config = roomeq_config_gen::generate_config(&scenario, temp_dir.path()).unwrap();

    assert!(config.speakers.contains_key("left"));
    assert!(config.speakers.contains_key("right"));
    assert!(config.speakers.contains_key("center"));
    assert!(config.speakers.contains_key("surround_left"));
    assert!(config.speakers.contains_key("surround_right"));
    assert!(config.speakers.contains_key("lfe"), "5.1 should have LFE");
    assert_eq!(
        config.speakers.len(),
        6,
        "5.1 should have 6 speaker entries"
    );
}

#[test]
fn test_config_5_1_4_has_height_channels() {
    let scenario = scenarios::scenario_by_name("medium_surround_5_1_4").unwrap();
    let temp_dir = TempDir::new().unwrap();
    let config = roomeq_config_gen::generate_config(&scenario, temp_dir.path()).unwrap();

    assert!(config.speakers.contains_key("left"));
    assert!(config.speakers.contains_key("right"));
    assert!(config.speakers.contains_key("center"));
    assert!(config.speakers.contains_key("surround_left"));
    assert!(config.speakers.contains_key("surround_right"));
    assert!(config.speakers.contains_key("lfe"), "5.1.4 should have LFE");
    assert!(
        config.speakers.contains_key("top_front_left"),
        "Missing 'top_front_left'"
    );
    assert!(
        config.speakers.contains_key("top_front_right"),
        "Missing 'top_front_right'"
    );
    assert!(
        config.speakers.contains_key("top_rear_left"),
        "Missing 'top_rear_left'"
    );
    assert!(
        config.speakers.contains_key("top_rear_right"),
        "Missing 'top_rear_right'"
    );
    assert_eq!(
        config.speakers.len(),
        10,
        "5.1.4 should have 10 speaker entries"
    );
}

#[test]
fn test_all_scenario_configs_serialize() {
    let temp_dir = TempDir::new().unwrap();
    for scenario in scenarios::all_scenarios() {
        let config = roomeq_config_gen::generate_config(&scenario, temp_dir.path())
            .unwrap_or_else(|e| panic!("Config gen failed for {}: {e}", scenario.name));
        let json = serde_json::to_string_pretty(&config)
            .unwrap_or_else(|e| panic!("Serialize failed for {}: {e}", scenario.name));
        let _roundtrip: autoeq::roomeq::RoomConfig = serde_json::from_str(&json)
            .unwrap_or_else(|e| panic!("Roundtrip failed for {}: {e}", scenario.name));
    }
}
