//! Fuzzer for roomeq binary
//!
//! Generates random speaker configurations and verifies optimization improves scores.

use clap::Parser;
use rand::Rng;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Fuzzer for roomeq
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Number of test scenarios to run
    #[arg(short = 'n', long, default_value_t = 100)]
    num_tests: usize,

    /// Random seed (for reproducibility)
    #[arg(long)]
    seed: Option<u64>,

    /// Output directory for generated configs and measurements
    #[arg(short, long)]
    output_dir: Option<PathBuf>,

    /// Sample rate for filter design
    #[arg(long, default_value_t = 48000.0)]
    sample_rate: f64,

    /// Maximum number of speakers per configuration
    #[arg(long, default_value_t = 4)]
    max_speakers: usize,

    /// Verbose output
    #[arg(short, long)]
    verbose: bool,
}

/// Filter type for synthetic speaker generation
#[derive(Debug, Clone, Copy)]
enum FilterType {
    Flat,
    Lowpass(f64),  // cutoff frequency
    Highpass(f64), // cutoff frequency
    Bandpass(f64, f64), // low, high cutoff
}

/// Synthetic speaker configuration
#[derive(Debug, Clone)]
struct SyntheticSpeaker {
    filter_type: FilterType,
    noise_level_db: f64,
    spl_offset_db: f64,
}

/// Information about a multi-driver group for plotting
#[derive(Debug, Clone)]
struct MultiDriverGroupInfo {
    channel_name: String,
    measurement_paths: Vec<PathBuf>,
    crossover_type: String,
}

/// Room configuration for serialization
#[derive(Debug, Clone, Serialize, Deserialize)]
struct RoomConfig {
    speakers: HashMap<String, SpeakerConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    crossovers: Option<HashMap<String, CrossoverConfig>>,
    optimizer: OptimizerConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
enum SpeakerConfig {
    Single(String),
    Group(SpeakerGroup),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SpeakerGroup {
    name: String,
    measurements: Vec<String>,
    crossover: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CrossoverConfig {
    #[serde(rename = "type")]
    crossover_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OptimizerConfig {
    num_filters: usize,
    algorithm: String,
    max_iter: usize,
    min_freq: f64,
    max_freq: f64,
    min_q: f64,
    max_q: f64,
    min_db: f64,
    max_db: f64,
    loss_type: String,
}

impl Default for OptimizerConfig {
    fn default() -> Self {
        Self {
            num_filters: 5,
            algorithm: "nlopt:cobyla".to_string(),
            max_iter: 500,
            min_freq: 100.0,
            max_freq: 10000.0,
            min_q: 0.5,
            max_q: 10.0,
            min_db: -12.0,
            max_db: 12.0,
            loss_type: "flat".to_string(),
        }
    }
}

/// Roomeq output for parsing
#[derive(Debug, Deserialize)]
struct RoomeqOutput {
    channels: HashMap<String, ChannelOutput>,
    metadata: Option<Metadata>,
}

#[derive(Debug, Deserialize)]
struct ChannelOutput {
    drivers: Option<Vec<DriverOutput>>,
}

#[derive(Debug, Deserialize)]
struct DriverOutput {
    plugins: Vec<PluginOutput>,
}

#[derive(Debug, Deserialize)]
struct PluginOutput {
    plugin_type: String,
    parameters: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct Metadata {
    pre_score: f64,
    post_score: f64,
}

/// Generate plots for all multi-driver groups
fn generate_plots_for_multi_drivers(
    output_json_path: &Path,
    multi_driver_groups: &[MultiDriverGroupInfo],
    output_dir: &Path,
    test_idx: usize,
    sample_rate: f64,
    verbose: bool,
) -> Result<(), Box<dyn Error>> {
    // Read the roomeq output JSON
    let output_json = fs::read_to_string(output_json_path)?;
    let roomeq_output: RoomeqOutput = serde_json::from_str(&output_json)?;

    // Process each multi-driver group
    for group in multi_driver_groups {
        if verbose {
            println!("    Generating plot for {} group", group.channel_name);
        }

        // Find the channel in the output
        let channel_output = roomeq_output
            .channels
            .get(&group.channel_name)
            .ok_or_else(|| format!("Channel {} not found in output", group.channel_name))?;

        // Extract drivers if present
        let drivers_output = channel_output
            .drivers
            .as_ref()
            .ok_or_else(|| format!("No drivers found for channel {}", group.channel_name))?;

        // Extract gains and crossover frequencies
        let mut gains = Vec::new();
        let mut all_crossover_freqs = Vec::new();

        for driver in drivers_output {
            let mut driver_gain = 0.0;

            for plugin in &driver.plugins {
                if plugin.plugin_type == "gain" {
                    if let Some(gain_db) = plugin.parameters.get("gain_db") {
                        driver_gain = gain_db.as_f64().unwrap_or(0.0);
                    }
                } else if plugin.plugin_type == "crossover" {
                    if let Some(freq) = plugin.parameters.get("frequency") {
                        // Collect all crossover frequencies (may have duplicates)
                        all_crossover_freqs.push(freq.as_f64().unwrap_or(1000.0));
                    }
                }
            }

            gains.push(driver_gain);
        }

        // Get unique crossover frequencies using a small epsilon for comparison
        let mut crossover_freqs: Vec<f64> = Vec::new();
        for freq in all_crossover_freqs {
            // Check if this frequency is already in the list (within 0.01 Hz tolerance)
            if !crossover_freqs.iter().any(|&f| (f - freq).abs() < 0.01) {
                crossover_freqs.push(freq);
            }
        }
        crossover_freqs.sort_by(|a, b| a.partial_cmp(b).unwrap());

        // Check if we have the expected number of crossover frequencies
        let expected_xover_count = drivers_output.len() - 1;
        if crossover_freqs.len() != expected_xover_count {
            if verbose {
                println!(
                    "      Warning: Expected {} crossover frequencies for {} drivers, got {}",
                    expected_xover_count,
                    drivers_output.len(),
                    crossover_freqs.len()
                );
                println!("      Frequencies: {:?}", crossover_freqs);
                println!("      Skipping plot generation for this group");
            }
            continue;
        }

        // Load measurements
        let mut driver_measurements = Vec::new();
        for path in &group.measurement_paths {
            let curve = autoeq::read::read_curve_from_csv(path)?;
            driver_measurements.push(autoeq::loss::DriverMeasurement {
                freq: curve.freq,
                spl: curve.spl,
                phase: None,
            });
        }

        // Parse crossover type
        let crossover_type = match group.crossover_type.as_str() {
            "LR24" => autoeq::loss::CrossoverType::LinkwitzRiley4,
            "LR12" => autoeq::loss::CrossoverType::LinkwitzRiley2,
            "Butterworth12" => autoeq::loss::CrossoverType::Butterworth2,
            _ => autoeq::loss::CrossoverType::LinkwitzRiley4, // default
        };

        // Create DriversLossData
        let drivers_data = autoeq::loss::DriversLossData::new(
            driver_measurements,
            crossover_type,
        );

        // Generate plot
        let plot_path = output_dir.join(format!(
            "test_{}_{}_drivers.html",
            test_idx, group.channel_name
        ));

        autoeq::plot::plot_drivers_results(
            &drivers_data,
            &gains,
            &crossover_freqs,
            sample_rate,
            &plot_path,
        )?;

        if verbose {
            println!("      Plot saved to {:?}", plot_path);
        }
    }

    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    // Determine output directory (default to data_generated/roomeq_fuzzer)
    let output_dir = if let Some(dir) = args.output_dir {
        dir
    } else {
        autoeq_env::get_data_generated_dir()?.join("roomeq_fuzzer")
    };

    // Create output directory
    fs::create_dir_all(&output_dir)?;

    // Initialize RNG
    let mut rng = if let Some(seed) = args.seed {
        ChaCha8Rng::seed_from_u64(seed)
    } else {
        ChaCha8Rng::from_os_rng()
    };

    println!("Running {} fuzzing tests...", args.num_tests);
    println!("Output directory: {:?}", output_dir);
    println!();

    let mut failures = Vec::new();
    let mut successes = 0;

    for test_idx in 0..args.num_tests {
        if args.verbose {
            println!("=== Test {}/{} ===", test_idx + 1, args.num_tests);
        }

        // Generate random configuration
        let (room_config, measurement_files, multi_driver_groups) = generate_random_config(
            &mut rng,
            &output_dir,
            test_idx,
            args.max_speakers,
            args.verbose,
        )?;

        // Save configuration
        let config_path = output_dir.join(format!("test_{}_config.json", test_idx));
        let config_json = serde_json::to_string_pretty(&room_config)?;
        fs::write(&config_path, config_json)?;

        // Run roomeq
        let output_path = output_dir.join(format!("test_{}_output.json", test_idx));
        let result = run_roomeq(&config_path, &output_path, args.sample_rate, args.verbose);

        match result {
            Ok((pre_score, post_score)) => {
                if post_score < pre_score {
                    successes += 1;
                    if args.verbose {
                        println!(
                            "  ✓ Success: pre={:.6}, post={:.6}, improvement={:.2}%",
                            pre_score,
                            post_score,
                            (1.0 - post_score / pre_score) * 100.0
                        );
                    }
                } else {
                    failures.push((test_idx, config_path.clone(), pre_score, post_score));
                    println!(
                        "  ✗ FAILURE Test {}: post_score ({:.6}) >= pre_score ({:.6})",
                        test_idx, post_score, pre_score
                    );
                    println!("    Config: {:?}", config_path);
                }

                // Generate plots for multi-driver groups
                if !multi_driver_groups.is_empty() {
                    let plot_result = generate_plots_for_multi_drivers(
                        &output_path,
                        &multi_driver_groups,
                        &output_dir,
                        test_idx,
                        args.sample_rate,
                        args.verbose,
                    );
                    if let Err(e) = plot_result {
                        if args.verbose {
                            println!("  Warning: Failed to generate plots: {}", e);
                        }
                    }
                }
            }
            Err(e) => {
                failures.push((test_idx, config_path.clone(), 0.0, 0.0));
                println!("  ✗ ERROR Test {}: {}", test_idx, e);
                println!("    Config: {:?}", config_path);
            }
        }

        // Clean up measurement files
        if !args.verbose {
            for file in measurement_files {
                let _ = fs::remove_file(file);
            }
        }

        if !args.verbose && test_idx % 10 == 9 {
            println!("Completed {}/{} tests", test_idx + 1, args.num_tests);
        }
    }

    // Summary
    println!();
    println!("=== Fuzzing Summary ===");
    println!("Total tests: {}", args.num_tests);
    println!("Successes: {}", successes);
    println!("Failures: {}", failures.len());
    println!(
        "Success rate: {:.1}%",
        (successes as f64 / args.num_tests as f64) * 100.0
    );

    if !failures.is_empty() {
        println!();
        println!("Failed configurations:");
        for (idx, config, pre, post) in failures {
            println!(
                "  Test {}: pre={:.6}, post={:.6}, config={:?}",
                idx, pre, post, config
            );
        }
    }

    Ok(())
}

/// Generate random room configuration
fn generate_random_config(
    rng: &mut ChaCha8Rng,
    output_dir: &Path,
    test_idx: usize,
    max_speakers: usize,
    verbose: bool,
) -> Result<(RoomConfig, Vec<PathBuf>, Vec<MultiDriverGroupInfo>), Box<dyn Error>> {
    let num_speakers = rng.random_range(1..=max_speakers);
    let mut speakers = HashMap::new();
    let mut crossovers = HashMap::new();
    let mut measurement_files = Vec::new();
    let mut multi_driver_groups = Vec::new();

    if verbose {
        println!("  Generating {} speakers", num_speakers);
    }

    for speaker_idx in 0..num_speakers {
        let channel_name = if num_speakers == 2 {
            if speaker_idx == 0 {
                "left"
            } else {
                "right"
            }
        } else {
            match speaker_idx {
                0 => "left",
                1 => "right",
                2 => "center",
                _ => "surround",
            }
        }
        .to_string();

        // 30% chance of multi-driver group
        if rng.random_bool(0.3) {
            let num_drivers = rng.random_range(2..=4);
            let mut driver_paths = Vec::new();
            let mut driver_csv_paths = Vec::new();

            if verbose {
                println!("    {}: multi-driver group ({} drivers)", channel_name, num_drivers);
            }

            for driver_idx in 0..num_drivers {
                let speaker_config = generate_random_speaker(rng);
                let csv_path = output_dir.join(format!(
                    "test_{}_{}_driver_{}.csv",
                    test_idx, channel_name, driver_idx
                ));
                generate_measurement_csv(&csv_path, &speaker_config, driver_idx, num_drivers)?;
                driver_paths.push(csv_path.to_string_lossy().to_string());
                driver_csv_paths.push(csv_path.clone());
                measurement_files.push(csv_path);
            }

            let crossover_name = format!("{}_crossover", channel_name);
            let crossover_type = "LR24".to_string();
            crossovers.insert(
                crossover_name.clone(),
                CrossoverConfig {
                    crossover_type: crossover_type.clone(),
                },
            );

            speakers.insert(
                channel_name.clone(),
                SpeakerConfig::Group(SpeakerGroup {
                    name: format!("{} Speaker Group", channel_name),
                    measurements: driver_paths,
                    crossover: Some(crossover_name),
                }),
            );

            // Store multi-driver group info for plotting
            multi_driver_groups.push(MultiDriverGroupInfo {
                channel_name: channel_name.clone(),
                measurement_paths: driver_csv_paths,
                crossover_type,
            });
        } else {
            if verbose {
                println!("    {}: single speaker", channel_name);
            }

            let speaker_config = generate_random_speaker(rng);
            let csv_path = output_dir.join(format!("test_{}_{}.csv", test_idx, channel_name));
            generate_measurement_csv(&csv_path, &speaker_config, 0, 1)?;
            speakers.insert(
                channel_name,
                SpeakerConfig::Single(csv_path.to_string_lossy().to_string()),
            );
            measurement_files.push(csv_path);
        }
    }

    let room_config = RoomConfig {
        speakers,
        crossovers: if crossovers.is_empty() {
            None
        } else {
            Some(crossovers)
        },
        optimizer: OptimizerConfig::default(),
    };

    Ok((room_config, measurement_files, multi_driver_groups))
}

/// Generate random speaker configuration
fn generate_random_speaker(rng: &mut ChaCha8Rng) -> SyntheticSpeaker {
    let filter_type = match rng.random_range(0..4) {
        0 => FilterType::Flat,
        1 => FilterType::Lowpass(rng.random_range(2000.0..15000.0)),
        2 => FilterType::Highpass(rng.random_range(50.0..500.0)),
        _ => {
            let low = rng.random_range(100.0..1000.0);
            let high = rng.random_range(low + 500.0..15000.0);
            FilterType::Bandpass(low, high)
        }
    };

    SyntheticSpeaker {
        filter_type,
        noise_level_db: rng.random_range(0.5..3.0),
        spl_offset_db: rng.random_range(-10.0..10.0),
    }
}

/// Generate synthetic measurement CSV file
fn generate_measurement_csv(
    path: &Path,
    config: &SyntheticSpeaker,
    driver_idx: usize,
    num_drivers: usize,
) -> Result<(), Box<dyn Error>> {
    let mut rng = ChaCha8Rng::from_os_rng();

    // Generate frequency points (logarithmic spacing)
    let num_points = 200;
    let min_freq: f64 = 20.0;
    let max_freq: f64 = 20000.0;
    let log_min = min_freq.ln();
    let log_max = max_freq.ln();

    let mut csv_content = String::from("freq,spl\n");

    for i in 0..num_points {
        let t = i as f64 / (num_points - 1) as f64;
        let log_freq = log_min + t * (log_max - log_min);
        let freq = log_freq.exp();

        // Start with flat response at 85 dB
        let mut spl = 85.0 + config.spl_offset_db;

        // Apply filter rolloff
        spl += match config.filter_type {
            FilterType::Flat => 0.0,
            FilterType::Lowpass(cutoff) => {
                if freq > cutoff {
                    // 24 dB/octave rolloff
                    let octaves = (freq / cutoff).log2();
                    -24.0 * octaves
                } else {
                    0.0
                }
            }
            FilterType::Highpass(cutoff) => {
                if freq < cutoff {
                    // 24 dB/octave rolloff
                    let octaves = (cutoff / freq).log2();
                    -24.0 * octaves
                } else {
                    0.0
                }
            }
            FilterType::Bandpass(low, high) => {
                let mut rolloff = 0.0;
                if freq < low {
                    let octaves = (low / freq).log2();
                    rolloff -= 24.0 * octaves;
                }
                if freq > high {
                    let octaves = (freq / high).log2();
                    rolloff -= 24.0 * octaves;
                }
                rolloff
            }
        };

        // For multi-driver, adjust frequency response based on driver position
        if num_drivers > 1 {
            let driver_center_freq = match driver_idx {
                0 => 200.0,                          // woofer
                1 if num_drivers == 2 => 5000.0,    // tweeter
                1 if num_drivers == 3 => 1000.0,    // midrange
                2 if num_drivers == 3 => 8000.0,    // tweeter
                1 if num_drivers == 4 => 500.0,     // lower mid
                2 if num_drivers == 4 => 2000.0,    // upper mid
                3 if num_drivers == 4 => 10000.0,   // tweeter
                _ => 1000.0,
            };

            // Natural rolloff away from driver's optimal range
            let distance = (freq / driver_center_freq).log2().abs();
            spl -= distance * 2.0; // Gentle rolloff
        }

        // Add noise
        let noise = rng.random_range(-config.noise_level_db..config.noise_level_db);
        spl += noise;

        csv_content.push_str(&format!("{:.2},{:.2}\n", freq, spl));
    }

    fs::write(path, csv_content)?;
    Ok(())
}

/// Run roomeq binary and parse output
fn run_roomeq(
    config_path: &Path,
    output_path: &Path,
    sample_rate: f64,
    verbose: bool,
) -> Result<(f64, f64), Box<dyn Error>> {
    let output = Command::new("cargo")
        .args([
            "run",
            "--bin",
            "roomeq",
            "--release",
            "--",
            "--config",
            config_path.to_str().unwrap(),
            "--output",
            output_path.to_str().unwrap(),
            "--sample-rate",
            &sample_rate.to_string(),
        ])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("roomeq failed: {}", stderr).into());
    }

    if verbose {
        let stdout = String::from_utf8_lossy(&output.stdout);
        println!("{}", stdout);
    }

    // Parse output JSON
    let output_json = fs::read_to_string(output_path)?;
    let roomeq_output: RoomeqOutput = serde_json::from_str(&output_json)?;

    let metadata = roomeq_output
        .metadata
        .ok_or("Missing metadata in roomeq output")?;

    Ok((metadata.pre_score, metadata.post_score))
}
