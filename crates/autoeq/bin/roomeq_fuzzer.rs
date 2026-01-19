//! Fuzzer for roomeq binary
//!
//! Generates random speaker configurations and verifies optimization improves scores.
//! Includes panic handling and config validation for robust fuzzing.

use clap::Parser;
use rand::Rng;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::error::Error;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

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
    Lowpass(f64),       // cutoff frequency
    Highpass(f64),      // cutoff frequency
    Bandpass(f64, f64), // low, high cutoff
}

/// Driver type with realistic frequency ranges
/// Each driver covers a specific range with some overlap to adjacent drivers
#[derive(Debug, Clone, Copy)]
enum DriverType {
    /// Subwoofer: 10-400 Hz (primarily 20-200 Hz, rolls off above)
    Subwoofer,
    /// Woofer: 50-1000 Hz (primarily 80-500 Hz, rolls off at extremes)
    Woofer,
    /// Midrange: 400-4000 Hz (primarily 500-2500 Hz)
    Midrange,
    /// Tweeter: 1000-20000 Hz (primarily 2000-16000 Hz)
    Tweeter,
}

impl DriverType {
    /// Get the frequency range for this driver type (low_cutoff, high_cutoff)
    fn freq_range(&self) -> (f64, f64) {
        match self {
            DriverType::Subwoofer => (10.0, 400.0),
            DriverType::Woofer => (50.0, 1000.0),
            DriverType::Midrange => (400.0, 4000.0),
            DriverType::Tweeter => (1000.0, 20000.0),
        }
    }

    /// Get driver type for a given index in an N-driver system
    fn for_index(driver_idx: usize, num_drivers: usize) -> Self {
        match (num_drivers, driver_idx) {
            // 2-way system: woofer + tweeter
            (2, 0) => DriverType::Woofer,
            (2, 1) => DriverType::Tweeter,
            // 3-way system: woofer + midrange + tweeter
            (3, 0) => DriverType::Woofer,
            (3, 1) => DriverType::Midrange,
            (3, 2) => DriverType::Tweeter,
            // 4-way system: subwoofer + woofer + midrange + tweeter
            (4, 0) => DriverType::Subwoofer,
            (4, 1) => DriverType::Woofer,
            (4, 2) => DriverType::Midrange,
            (4, 3) => DriverType::Tweeter,
            // Default fallback
            _ => DriverType::Woofer,
        }
    }
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
    measurement_sources: Vec<MeasurementSource>,
    crossover_type: String,
}

/// Room configuration for serialization
#[derive(Debug, Clone, Serialize, Deserialize)]
struct RoomConfig {
    speakers: HashMap<String, SpeakerConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    crossovers: Option<HashMap<String, CrossoverConfig>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    target_curve: Option<TargetCurveConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    group_delay: Option<Vec<GroupDelayConfig>>,
    optimizer: OptimizerConfig,
}

/// Group delay optimization configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
struct GroupDelayConfig {
    subwoofer: String,
    speakers: Vec<String>,
    #[serde(default = "default_group_delay_min_freq")]
    min_freq: f64,
    #[serde(default = "default_group_delay_max_freq")]
    max_freq: f64,
}

fn default_group_delay_min_freq() -> f64 {
    30.0
}
fn default_group_delay_max_freq() -> f64 {
    120.0
}

/// Target curve configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
enum TargetCurveConfig {
    Path(std::path::PathBuf),
    Predefined(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
enum MeasurementSource {
    Single(String),
    Multiple(Vec<String>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
enum SpeakerConfig {
    Single(MeasurementSource),
    Group(SpeakerGroup),
    MultiSub(MultiSubGroup),
    Dba(DBAConfig),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SpeakerGroup {
    name: String,
    measurements: Vec<MeasurementSource>,
    crossover: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MultiSubGroup {
    name: String,
    subwoofers: Vec<MeasurementSource>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DBAConfig {
    name: String,
    front: Vec<MeasurementSource>,
    rear: Vec<MeasurementSource>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FirConfig {
    taps: usize,
    phase: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CrossoverConfig {
    #[serde(rename = "type")]
    crossover_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    frequency: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    frequencies: Option<Vec<f64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    frequency_range: Option<(f64, f64)>,
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
    #[serde(default = "default_peq_model")]
    peq_model: String,
    #[serde(default = "default_mode")]
    mode: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    fir: Option<FirConfig>,
}

fn default_peq_model() -> String {
    "pk".to_string()
}
fn default_mode() -> String {
    "iir".to_string()
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
            peq_model: "pk".to_string(),
            mode: "iir".to_string(),
            fir: None,
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
                } else if plugin.plugin_type == "crossover"
                    && let Some(freq) = plugin.parameters.get("frequency")
                {
                    // Collect all crossover frequencies (may have duplicates)
                    all_crossover_freqs.push(freq.as_f64().unwrap_or(1000.0));
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
        for source in &group.measurement_sources {
            // Convert fuzzer source to lib source
            let lib_source = match source {
                MeasurementSource::Single(path) => autoeq::MeasurementSource::Single(
                    autoeq::MeasurementRef::Path(PathBuf::from(path)),
                ),
                MeasurementSource::Multiple(paths) => autoeq::MeasurementSource::Multiple(
                    paths
                        .iter()
                        .map(|p| autoeq::MeasurementRef::Path(PathBuf::from(p)))
                        .collect(),
                ),
            };

            // Load and average
            let curve = autoeq::load_source(&lib_source)?;

            driver_measurements.push(autoeq::loss::DriverMeasurement {
                freq: curve.freq,
                spl: curve.spl,
                phase: curve.phase,
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
        let drivers_data = autoeq::loss::DriversLossData::new(driver_measurements, crossover_type);

        // Generate plot
        let plot_path = output_dir.join(format!(
            "test_{}_{}_drivers.html",
            test_idx, group.channel_name
        ));

        autoeq::plot::plot_drivers_results(
            &drivers_data,
            &gains,
            &crossover_freqs,
            None,
            sample_rate,
            &plot_path,
        )?;

        if verbose {
            println!("      Plot saved to {:?}", plot_path);
        }
    }

    Ok(())
}

/// Global counter for current test index (for panic reporting)
static CURRENT_TEST_INDEX: AtomicUsize = AtomicUsize::new(0);

/// Validate configuration before running roomeq
fn validate_config(config: &RoomConfig) -> Result<(), String> {
    // Validate optimizer config
    if config.optimizer.num_filters == 0 {
        // Warning only - still valid
        eprintln!("Warning: num_filters is 0, no EQ will be applied");
    }

    if config.optimizer.min_freq >= config.optimizer.max_freq {
        return Err(format!(
            "min_freq ({}) must be less than max_freq ({})",
            config.optimizer.min_freq, config.optimizer.max_freq
        ));
    }

    if config.optimizer.min_q > config.optimizer.max_q {
        return Err(format!(
            "min_q ({}) must be less than or equal to max_q ({})",
            config.optimizer.min_q, config.optimizer.max_q
        ));
    }

    if config.optimizer.min_db > config.optimizer.max_db {
        return Err(format!(
            "min_db ({}) must be less than or equal to max_db ({})",
            config.optimizer.min_db, config.optimizer.max_db
        ));
    }

    // Validate crossover references
    if let Some(ref crossovers) = config.crossovers {
        for (name, speaker) in &config.speakers {
            if let SpeakerConfig::Group(group) = speaker
                && let Some(ref crossover_ref) = group.crossover
                && !crossovers.contains_key(crossover_ref)
            {
                return Err(format!(
                    "Speaker '{}' references non-existent crossover '{}'",
                    name, crossover_ref
                ));
            }
        }
    }

    // Validate group delay references
    if let Some(ref gd_configs) = config.group_delay {
        for gd in gd_configs {
            if !config.speakers.contains_key(&gd.subwoofer) {
                return Err(format!(
                    "Group delay references non-existent subwoofer '{}'",
                    gd.subwoofer
                ));
            }
            for speaker in &gd.speakers {
                if !config.speakers.contains_key(speaker) {
                    return Err(format!(
                        "Group delay references non-existent speaker '{}'",
                        speaker
                    ));
                }
            }
            if gd.min_freq >= gd.max_freq {
                return Err(format!(
                    "Group delay min_freq ({}) must be less than max_freq ({})",
                    gd.min_freq, gd.max_freq
                ));
            }
        }
    }

    // Validate speakers are not empty
    if config.speakers.is_empty() {
        return Err("No speakers configured".to_string());
    }

    // Validate each speaker config
    for (name, speaker) in &config.speakers {
        match speaker {
            SpeakerConfig::Group(group) => {
                if group.measurements.is_empty() {
                    return Err(format!("Speaker group '{}' has no measurements", name));
                }
            }
            SpeakerConfig::MultiSub(ms) => {
                if ms.subwoofers.is_empty() {
                    return Err(format!("Multi-sub '{}' has no subwoofers", name));
                }
            }
            SpeakerConfig::Dba(dba) => {
                if dba.front.is_empty() {
                    return Err(format!("DBA '{}' has no front speakers", name));
                }
                if dba.rear.is_empty() {
                    return Err(format!("DBA '{}' has no rear speakers", name));
                }
            }
            SpeakerConfig::Single(_) => {} // Single speaker - no validation needed
        }
    }

    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    // Set up panic hook for crash reporting
    let crash_file_path = std::env::current_dir()?.join("roomeq_fuzzer_crashes.txt");
    std::panic::set_hook(Box::new(move |info| {
        let test_idx = CURRENT_TEST_INDEX.load(Ordering::SeqCst);
        let msg = info.to_string();
        eprintln!("FATAL PANIC in roomeq_fuzzer at test {}: {}", test_idx, msg);

        // Write crash report
        if let Ok(mut crash_file) = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&crash_file_path)
        {
            let timestamp = chrono::Utc::now().to_rfc3339();
            let _ = writeln!(
                crash_file,
                "[{}] Test {}: Panic: {}",
                timestamp, test_idx, msg
            );
        }
    }));

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
        // Update global test index for panic reporting
        CURRENT_TEST_INDEX.store(test_idx, Ordering::SeqCst);

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

        // Validate configuration before running
        if let Err(validation_error) = validate_config(&room_config) {
            if args.verbose {
                println!("  ⚠ Config validation failed: {}", validation_error);
                println!("    Regenerating config...");
            }
            // Skip this test and continue - the fuzzer generated an invalid config
            // which is actually a bug in the config generator, not roomeq
            continue;
        }

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
                    if let Err(e) = plot_result
                        && args.verbose
                    {
                        println!("  Warning: Failed to generate plots: {}", e);
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

/// Generate crossover frequency configuration based on number of drivers
/// Returns (frequency, frequencies, frequency_range) where only one should be Some
#[allow(clippy::type_complexity)]
fn generate_crossover_frequencies(
    rng: &mut ChaCha8Rng,
    num_drivers: usize,
    verbose: bool,
    channel_name: &str,
) -> (Option<f64>, Option<Vec<f64>>, Option<(f64, f64)>) {
    // 40% chance of fixed frequencies, 30% frequency range, 30% auto (all None)
    let config_type = rng.random_range(0..100);

    if config_type < 40 {
        // Fixed frequencies based on driver count
        if num_drivers == 2 {
            // 2-way: single crossover frequency (800-3000 Hz typical)
            let freq = rng.random_range(800.0..3000.0);
            if verbose {
                println!("      {}: fixed crossover at {:.0} Hz", channel_name, freq);
            }
            (Some(freq), None, None)
        } else {
            // 3-way or more: multiple crossover frequencies
            let mut freqs = Vec::new();
            // Generate sorted crossover points
            let mut prev_freq: f64 = 200.0;
            for _ in 0..(num_drivers - 1) {
                let max_freq = (prev_freq * 5.0).min(15000.0);
                let next_freq = rng.random_range(prev_freq * 2.0..max_freq);
                freqs.push(next_freq);
                prev_freq = next_freq;
            }
            if verbose {
                println!("      {}: fixed crossovers at {:?} Hz", channel_name, freqs);
            }
            (None, Some(freqs), None)
        }
    } else if config_type < 70 {
        // Frequency range for optimization
        let min_freq = rng.random_range(200.0..500.0);
        let max_freq = rng.random_range(5000.0..15000.0);
        if verbose {
            println!(
                "      {}: crossover range {:.0}-{:.0} Hz",
                channel_name, min_freq, max_freq
            );
        }
        (None, None, Some((min_freq, max_freq)))
    } else {
        // Auto (let optimizer decide)
        if verbose {
            println!("      {}: auto crossover optimization", channel_name);
        }
        (None, None, None)
    }
}

/// Generate random room configuration
#[allow(clippy::type_complexity)]
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
            if speaker_idx == 0 { "left" } else { "right" }
        } else {
            match speaker_idx {
                0 => "left",
                1 => "right",
                2 => "center",
                _ => "surround",
            }
        }
        .to_string();

        let config_type = rng.random_range(0..100);

        if config_type < 60 {
            // Single speaker
            if verbose {
                println!("    {}: single speaker", channel_name);
            }
            let (source, paths) =
                generate_random_source(rng, output_dir, test_idx, &channel_name, "driver", 0, 1)?;
            measurement_files.extend(paths);
            speakers.insert(channel_name, SpeakerConfig::Single(source));
        } else if config_type < 80 {
            // Multi-driver group
            let num_drivers = rng.random_range(2..=4);
            if verbose {
                println!(
                    "    {}: multi-driver group ({} drivers)",
                    channel_name, num_drivers
                );
            }

            let mut measurements = Vec::new();
            let mut group_paths = Vec::new();

            for driver_idx in 0..num_drivers {
                let (source, paths) = generate_random_source(
                    rng,
                    output_dir,
                    test_idx,
                    &channel_name,
                    "driver",
                    driver_idx,
                    num_drivers,
                )?;
                measurement_files.extend(paths.clone());
                group_paths.extend(paths);
                measurements.push(source);
            }

            let crossover_name = format!("{}_crossover", channel_name);
            let crossover_type = match rng.random_range(0..3) {
                0 => "LR24",
                1 => "LR2",
                _ => "Butterworth12",
            }
            .to_string();

            // Generate crossover frequency configuration
            let (frequency, frequencies, frequency_range) =
                generate_crossover_frequencies(rng, num_drivers, verbose, &channel_name);

            crossovers.insert(
                crossover_name.clone(),
                CrossoverConfig {
                    crossover_type: crossover_type.clone(),
                    frequency,
                    frequencies,
                    frequency_range,
                },
            );

            speakers.insert(
                channel_name.clone(),
                SpeakerConfig::Group(SpeakerGroup {
                    name: format!("{} Group", channel_name),
                    measurements: measurements.clone(),
                    crossover: Some(crossover_name),
                }),
            );

            multi_driver_groups.push(MultiDriverGroupInfo {
                channel_name: channel_name.clone(),
                measurement_sources: measurements.clone(),
                crossover_type,
            });
        } else if config_type < 90 {
            // MultiSub
            let num_subs = rng.random_range(2..=4);
            if verbose {
                println!("    {}: multi-sub ({} subs)", channel_name, num_subs);
            }

            let mut sub_sources = Vec::new();
            for i in 0..num_subs {
                let (source, paths) = generate_random_source(
                    rng,
                    output_dir,
                    test_idx,
                    &channel_name,
                    "sub",
                    i,
                    num_subs,
                )?;
                measurement_files.extend(paths);
                sub_sources.push(source);
            }

            speakers.insert(
                channel_name,
                SpeakerConfig::MultiSub(MultiSubGroup {
                    name: "subs".to_string(),
                    subwoofers: sub_sources,
                }),
            );
        } else {
            // DBA
            if verbose {
                println!("    {}: DBA", channel_name);
            }
            let num_front = rng.random_range(2..=4);
            let num_rear = rng.random_range(2..=4);

            let mut front_sources = Vec::new();
            for i in 0..num_front {
                let (source, paths) = generate_random_source(
                    rng,
                    output_dir,
                    test_idx,
                    &channel_name,
                    "front",
                    i,
                    num_front,
                )?;
                measurement_files.extend(paths);
                front_sources.push(source);
            }

            let mut rear_sources = Vec::new();
            for i in 0..num_rear {
                let (source, paths) = generate_random_source(
                    rng,
                    output_dir,
                    test_idx,
                    &channel_name,
                    "rear",
                    i,
                    num_rear,
                )?;
                measurement_files.extend(paths);
                rear_sources.push(source);
            }

            speakers.insert(
                channel_name,
                SpeakerConfig::Dba(DBAConfig {
                    name: "dba".to_string(),
                    front: front_sources,
                    rear: rear_sources,
                }),
            );
        }
    }

    // Collect subwoofer channel names for group delay optimization
    let subwoofer_channels: Vec<String> = speakers
        .iter()
        .filter_map(|(name, config)| match config {
            SpeakerConfig::MultiSub(_) | SpeakerConfig::Dba(_) => Some(name.clone()),
            _ => None,
        })
        .collect();

    // Collect single speaker channel names (potential targets for group delay alignment)
    let single_speaker_channels: Vec<String> = speakers
        .iter()
        .filter_map(|(name, config)| match config {
            SpeakerConfig::Single(_) | SpeakerConfig::Group(_) => Some(name.clone()),
            _ => None,
        })
        .collect();

    // Generate group delay config if we have subwoofers and other speakers
    let group_delay = if !subwoofer_channels.is_empty()
        && !single_speaker_channels.is_empty()
        && rng.random_bool(0.5)
    {
        if verbose {
            println!("  Adding group delay optimization");
        }
        let configs: Vec<GroupDelayConfig> = subwoofer_channels
            .iter()
            .map(|sub| {
                let min_freq = rng.random_range(20.0..50.0);
                let max_freq = rng.random_range(80.0..150.0);
                GroupDelayConfig {
                    subwoofer: sub.clone(),
                    speakers: single_speaker_channels.clone(),
                    min_freq,
                    max_freq,
                }
            })
            .collect();
        Some(configs)
    } else {
        None
    };

    // Generate target curve config (20% chance)
    let target_curve = if rng.random_bool(0.2) {
        let predefined = match rng.random_range(0..3) {
            0 => "flat",
            1 => "harman",
            _ => "bk",
        };
        if verbose {
            println!("  Using target curve: {}", predefined);
        }
        Some(TargetCurveConfig::Predefined(predefined.to_string()))
    } else {
        None
    };

    // Randomize optimizer config
    let mode = match rng.random_range(0..3) {
        0 => "iir",
        1 => "fir",
        _ => "mixed",
    }
    .to_string();

    let peq_model = match rng.random_range(0..3) {
        0 => "pk",
        1 => "ls-pk-hs",
        _ => "free",
    }
    .to_string();

    let algorithm = match rng.random_range(0..3) {
        0 => "cobyla",
        1 => "de",
        _ => "nlopt:cobyla",
    }
    .to_string();

    // Note: "score" loss type requires CEA2034 score data which synthetic measurements don't have
    let loss_type = "flat".to_string();

    let num_filters = rng.random_range(3..=12);
    let min_freq = rng.random_range(20.0..100.0);
    let max_freq = rng.random_range(10000.0..20000.0);
    let max_iter = rng.random_range(200..=1000);

    let fir_config = if mode != "iir" {
        Some(FirConfig {
            taps: match rng.random_range(0..3) {
                0 => 512,
                1 => 1024,
                _ => 2048,
            },
            phase: match rng.random_range(0..3) {
                0 => "linear".to_string(),
                1 => "minimum".to_string(),
                _ => "kirkeby".to_string(),
            },
        })
    } else {
        None
    };

    let room_config = RoomConfig {
        speakers,
        crossovers: if crossovers.is_empty() {
            None
        } else {
            Some(crossovers)
        },
        target_curve,
        group_delay,
        optimizer: OptimizerConfig {
            num_filters,
            algorithm,
            max_iter,
            min_freq,
            max_freq,
            min_q: 0.5,
            max_q: 10.0,
            min_db: -12.0,
            max_db: 12.0,
            loss_type,
            peq_model,
            mode,
            fir: fir_config,
        },
    };

    Ok((room_config, measurement_files, multi_driver_groups))
}

/// Helper to generate random source (single file or multiple files)
fn generate_random_source(
    rng: &mut ChaCha8Rng,
    output_dir: &Path,
    test_idx: usize,
    channel: &str,
    role: &str,
    idx: usize,
    count: usize,
) -> Result<(MeasurementSource, Vec<PathBuf>), Box<dyn Error>> {
    let mut paths = Vec::new();
    let is_multiple = rng.random_bool(0.1); // 10% chance of multiple measurements
    let num_files = if is_multiple {
        rng.random_range(2..=3)
    } else {
        1
    };

    let mut file_strings = Vec::new();

    for i in 0..num_files {
        let speaker_config = generate_random_speaker(rng);
        let filename = if is_multiple {
            format!(
                "test_{}_{}_{}_{}_pos{}.csv",
                test_idx, channel, role, idx, i
            )
        } else {
            format!("test_{}_{}_{}_{}.csv", test_idx, channel, role, idx)
        };
        let path = output_dir.join(filename);
        generate_measurement_csv(&path, &speaker_config, idx, count)?;

        file_strings.push(path.to_string_lossy().to_string());
        paths.push(path);
    }

    if is_multiple {
        Ok((MeasurementSource::Multiple(file_strings), paths))
    } else {
        Ok((MeasurementSource::Single(file_strings[0].clone()), paths))
    }
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

/// Generate synthetic measurement CSV file with realistic driver characteristics
///
/// For multi-driver systems, generates bandpass responses appropriate for each driver type:
/// - Subwoofer: 10-400 Hz
/// - Woofer: 50-1000 Hz
/// - Midrange: 400-4000 Hz
/// - Tweeter: 1000-20000 Hz
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

    let mut csv_content = String::from("freq,spl,phase\n");

    // Get driver type for multi-driver systems
    let driver_type = if num_drivers > 1 {
        Some(DriverType::for_index(driver_idx, num_drivers))
    } else {
        None
    };

    // Generate delay for phase simulation (0-5 ms)
    let delay_ms = rng.random_range(0.0..5.0);

    for i in 0..num_points {
        let t = i as f64 / (num_points - 1) as f64;
        let log_freq = log_min + t * (log_max - log_min);
        let freq = log_freq.exp();

        // Start with flat response at 85 dB with small offset (limited to ±4 dB)
        let spl_offset = config.spl_offset_db.clamp(-4.0, 4.0);
        let mut spl = 85.0 + spl_offset;

        // For multi-driver systems, apply realistic bandpass characteristics
        if let Some(dt) = driver_type {
            let (low_cutoff, high_cutoff) = dt.freq_range();

            // Apply highpass rolloff (24 dB/octave below low_cutoff)
            if freq < low_cutoff {
                let octaves = (low_cutoff / freq).log2();
                spl -= 24.0 * octaves;
            }

            // Apply lowpass rolloff (24 dB/octave above high_cutoff)
            if freq > high_cutoff {
                let octaves = (freq / high_cutoff).log2();
                spl -= 24.0 * octaves;
            }

            // Add gentle passband ripple (±1 dB variation in passband)
            if freq >= low_cutoff && freq <= high_cutoff {
                let passband_pos = (freq / low_cutoff).log2() / (high_cutoff / low_cutoff).log2();
                let ripple = (passband_pos * std::f64::consts::PI * 3.0).sin() * 1.0;
                spl += ripple;
            }
        } else {
            // Single speaker - apply the configured filter type
            spl += match config.filter_type {
                FilterType::Flat => 0.0,
                FilterType::Lowpass(cutoff) => {
                    if freq > cutoff {
                        let octaves = (freq / cutoff).log2();
                        -24.0 * octaves
                    } else {
                        0.0
                    }
                }
                FilterType::Highpass(cutoff) => {
                    if freq < cutoff {
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
        }

        // Add measurement noise (reduced for more realistic measurements)
        let noise_level = config.noise_level_db.min(3.0); // Limit noise to ±3 dB
        let noise = rng.random_range(-noise_level..noise_level);
        spl += noise;

        // Generate phase (delay + noise)
        let phase_delay = -360.0 * freq * (delay_ms / 1000.0);
        let phase_noise = rng.random_range(-10.0..10.0);
        let phase = (phase_delay + phase_noise) % 360.0;

        csv_content.push_str(&format!("{:.2},{:.2},{:.2}\n", freq, spl, phase));
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
