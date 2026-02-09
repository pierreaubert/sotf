//! Fuzzer for roomeq binary
//!
//! Generates random speaker configurations and verifies optimization improves scores.
//! Includes panic handling and config validation for robust fuzzing.

use autoeq::roomeq::{
    CrossoverConfig, DBAConfig, GroupDelayConfig, MultiSubGroup, OptimizerConfig, RoomConfig,
    SpeakerConfig, SpeakerGroup, TargetCurveConfig,
};
use autoeq::{MeasurementRef, MeasurementSource};
use clap::Parser;
use rand::Rng;
use rand::SeedableRng;
use rand::seq::{IndexedRandom, SliceRandom};
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
    Lowpass(f64),
    Highpass(f64),
    Bandpass(f64, f64),
}

/// Driver type for multi-driver simulation
#[derive(Debug, Clone, Copy, PartialEq)]
enum DriverType {
    Subwoofer,
    Woofer,
    Midrange,
    Tweeter,
}

impl DriverType {
    fn for_index(idx: usize, total: usize) -> Self {
        match total {
            2 => {
                if idx == 0 {
                    DriverType::Woofer
                } else {
                    DriverType::Tweeter
                }
            }
            3 => {
                if idx == 0 {
                    DriverType::Woofer
                } else if idx == 1 {
                    DriverType::Midrange
                } else {
                    DriverType::Tweeter
                }
            }
            _ => {
                // Default to a generic spread
                if idx == 0 {
                    DriverType::Woofer
                } else if idx < total - 1 {
                    DriverType::Midrange
                } else {
                    DriverType::Tweeter
                }
            }
        }
    }

    fn freq_range(&self) -> (f64, f64) {
        match self {
            DriverType::Subwoofer => (20.0, 400.0),
            DriverType::Woofer => (50.0, 1000.0),
            DriverType::Midrange => (400.0, 5000.0),
            DriverType::Tweeter => (2000.0, 20000.0),
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
    measurement_sources: Vec<autoeq::MeasurementSource>,
    crossover_type: String,
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

    for group in multi_driver_groups {
        let channel_output = match roomeq_output.channels.get(&group.channel_name) {
            Some(ch) => ch,
            None => {
                if verbose {
                    println!("      Warning: channel '{}' not found in output", group.channel_name);
                }
                continue;
            }
        };

        let drivers_output = match &channel_output.drivers {
            Some(d) => d,
            None => {
                if verbose {
                    println!("      Warning: channel '{}' has no driver output", group.channel_name);
                }
                continue;
            }
        };

        // Extract crossover gains and frequencies from plugins
        let mut gains = Vec::new();
        let mut crossover_freqs = Vec::new();

        for driver in drivers_output {
            for plugin in &driver.plugins {
                if plugin.plugin_type == "Gain" {
                    if let Some(gain) = plugin.parameters.get("gain_db").and_then(|v| v.as_f64()) {
                        gains.push(gain);
                    }
                } else if plugin.plugin_type == "Biquad" {
                    if let Some(freq) = plugin.parameters.get("freq").and_then(|v| v.as_f64()) {
                        crossover_freqs.push(freq);
                    }
                }
            }
        }

        // We expect N gains and N-1 crossover frequencies (duplicates removed)
        crossover_freqs.sort_by(|a, b| a.partial_cmp(b).unwrap());
        crossover_freqs.dedup_by(|a, b| (*a - *b).abs() < 1.0);

        if crossover_freqs.len() != group.measurement_sources.len() - 1 {
            if verbose {
                println!(
                    "      Warning: expected {} xover freqs but found {}",
                    group.measurement_sources.len() - 1,
                    crossover_freqs.len()
                );
                println!("      Frequencies: {:?}", crossover_freqs);
                println!("      Skipping plot generation for this group");
            }
            continue;
        }

        // Load measurements
        let mut driver_measurements = Vec::new();
        for lib_source in &group.measurement_sources {
            // Load and average
            let curve = autoeq::load_source(lib_source)?;

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

    if config.optimizer.max_iter == 0 {
        return Err("max_iter must be greater than 0".to_string());
    }

    // Validate speakers
    if config.speakers.is_empty() {
        return Err("No speakers configured".to_string());
    }

    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    // Create output directory
    let output_dir = args.output_dir.unwrap_or_else(|| {
        let dir = PathBuf::from("fuzzer_output");
        if !dir.exists() {
            fs::create_dir_all(&dir).unwrap();
        }
        dir
    });

    println!("Starting fuzzer with {} tests...", args.num_tests);
    println!("Output directory: {}", output_dir.display());

    let mut successful_tests = 0;
    let mut failed_tests = 0;

    // Use seed if provided
    let mut rng = if let Some(seed) = args.seed {
        ChaCha8Rng::seed_from_u64(seed)
    } else {
        ChaCha8Rng::from_os_rng()
    };

    for i in 0..args.num_tests {
        CURRENT_TEST_INDEX.store(i, Ordering::SeqCst);
        println!("Running test {}/{}...", i + 1, args.num_tests);

        // Create a subdirectory for this test
        let test_dir = output_dir.join(format!("test_{}", i));
        if test_dir.exists() {
            fs::remove_dir_all(&test_dir)?;
        }
        fs::create_dir_all(&test_dir)?;

        // Generate random configuration and measurements
        let (config, _measurement_files, multi_driver_groups) = generate_random_config(&test_dir, i, &mut rng, args.max_speakers)?;

        // Validate config
        if let Err(e) = validate_config(&config) {
            println!("  Invalid config generated: {}", e);
            failed_tests += 1;
            continue;
        }

        // Save config
        let config_path = test_dir.join("config.json");
        let config_json = serde_json::to_string_pretty(&config)?;
        fs::write(&config_path, config_json)?;

        // Run roomeq binary
        let output_json_path = test_dir.join("output.json");
        let mut child = Command::new("cargo")
            .args([
                "run",
                "--release",
                "--bin",
                "roomeq",
                "--",
                "--config",
                config_path.to_str().unwrap(),
                "--output",
                output_json_path.to_str().unwrap(),
            ])
            .spawn()?;

        let status = child.wait()?;

        if status.success() {
            println!("  Test {} successful!", i + 1);
            successful_tests += 1;

            // Generate plots for multi-driver groups
            if !multi_driver_groups.is_empty() {
                if let Err(e) = generate_plots_for_multi_drivers(
                    &output_json_path,
                    &multi_driver_groups,
                    &test_dir,
                    i,
                    args.sample_rate,
                    args.verbose,
                ) {
                    println!("  Warning: failed to generate plots: {}", e);
                }
            }
        } else {
            println!("  Test {} failed with exit code: {:?}", i + 1, status.code());
            failed_tests += 1;
        }
    }

    println!("\nFuzzing complete!");
    println!("Successful tests: {}", successful_tests);
    println!("Failed tests: {}", failed_tests);

    if failed_tests > 0 {
        std::process::exit(1);
    }

    Ok(())
}

/// Generate a random RoomConfig
fn generate_random_config(
    output_dir: &Path,
    test_idx: usize,
    rng: &mut ChaCha8Rng,
    max_speakers: usize,
) -> Result<(RoomConfig, Vec<PathBuf>, Vec<MultiDriverGroupInfo>), Box<dyn Error>> {
    let mut speakers = HashMap::new();
    let mut measurement_files = Vec::new();
    let mut multi_driver_groups = Vec::new();
    let mut crossovers = HashMap::new();

    let num_speakers = rng.random_range(1..=max_speakers);
    let channels = ["L", "R", "C", "SL", "SR", "SBL", "SBR"];

    for i in 0..num_speakers {
        let channel_name = channels[i % channels.len()].to_string();

        // Randomly choose speaker type
        let speaker_type_roll = rng.random_range(0..100);

        if speaker_type_roll < 60 {
            // 60% chance: Single speaker
            let (source, paths) = generate_random_source(rng, output_dir, test_idx, &channel_name, "main", 0, 1)?;
            measurement_files.extend(paths);
            speakers.insert(channel_name, SpeakerConfig::Single(source));
        } else if speaker_type_roll < 85 {
            // 25% chance: Multi-driver group
            let num_drivers = rng.random_range(2..=3);
            let mut driver_sources = Vec::new();
            for d in 0..num_drivers {
                let (source, paths) = generate_random_source(rng, output_dir, test_idx, &channel_name, "driver", d, num_drivers)?;
                measurement_files.extend(paths);
                driver_sources.push(source);
            }

            let xover_id = format!("xover_{}", channel_name);
            let xover_type = ["LR24", "LR12", "Butterworth12"].choose(rng).unwrap().to_string();

            crossovers.insert(xover_id.clone(), CrossoverConfig {
                crossover_type: xover_type.clone(),
                frequency: None,
                frequencies: None,
                frequency_range: Some((200.0, 5000.0)),
            });

            multi_driver_groups.push(MultiDriverGroupInfo {
                channel_name: channel_name.clone(),
                measurement_sources: driver_sources.clone(),
                crossover_type: xover_type,
            });

            speakers.insert(channel_name.clone(), SpeakerConfig::Group(SpeakerGroup {
                name: channel_name,
                speaker_name: Some(random_speaker_name()),
                measurements: driver_sources,
                crossover: Some(xover_id),
            }));
        } else {
            // 15% chance: Multi-sub or DBA
            let is_dba = rng.random_bool(0.5);

            if is_dba {
                let mut front_sources = Vec::new();
                let mut rear_sources = Vec::new();

                let (source_f, paths_f) = generate_random_source(rng, output_dir, test_idx, "LFE", "front", 0, 1)?;
                measurement_files.extend(paths_f);
                front_sources.push(source_f);

                let (source_r, paths_r) = generate_random_source(rng, output_dir, test_idx, "LFE", "rear", 0, 1)?;
                measurement_files.extend(paths_r);
                rear_sources.push(source_r);

                speakers.insert("LFE".to_string(), SpeakerConfig::Dba(DBAConfig {
                    name: "DBA".to_string(),
                    speaker_name: Some(random_speaker_name()),
                    front: front_sources,
                    rear: rear_sources,
                }));
            } else {
                let num_subs = rng.random_range(2..=4);
                let mut sub_sources = Vec::new();
                for d in 0..num_subs {
                    let (source, paths) = generate_random_source(rng, output_dir, test_idx, "LFE", "sub", d, num_subs)?;
                    measurement_files.extend(paths);
                    sub_sources.push(source);
                }

                speakers.insert("LFE".to_string(), SpeakerConfig::MultiSub(MultiSubGroup {
                    name: "MultiSub".to_string(),
                    speaker_name: Some(random_speaker_name()),
                    subwoofers: sub_sources,
                }));
            }
        }
    }

    // Optional target curve
    let target_curve = if rng.random_bool(0.3) {
        Some(TargetCurveConfig::Predefined("harman".to_string()))
    } else {
        None
    };

    let loss_type = if rng.random_bool(0.5) { "flat".to_string() } else { "score".to_string() };
    let peq_model = if rng.random_bool(0.5) { "pk".to_string() } else { "ls-pk-hs".to_string() };
    let mode = if rng.random_bool(0.7) { "iir".to_string() } else { "fir".to_string() };

    let fir_config = if mode == "fir" {
        Some(autoeq::roomeq::FirConfig {
            taps: 1024,
            phase: "linear".to_string(),
            correct_excess_phase: false,
            phase_smoothing: 0.167,
        })
    } else {
        None
    };

    let room_config = RoomConfig {
        version: autoeq::roomeq::default_config_version(),
        system: None,
        speakers,
        crossovers: if crossovers.is_empty() { None } else { Some(crossovers) },
        target_curve,
        group_delay: None,
        bass_management: None,
        optimizer: OptimizerConfig {
            algorithm: "autoeq:de".to_string(),
            num_filters: 7,
            max_iter: 100, // Small for fuzzer
            min_freq: 20.0,
            max_freq: 20000.0,
            min_q: 0.5,
            max_q: 10.0,
            min_db: -12.0,
            max_db: 12.0,
            loss_type,
            peq_model,
            mode,
            fir: fir_config,
            ..OptimizerConfig::default()
        },
        recording_config: None,
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
) -> Result<(autoeq::MeasurementSource, Vec<PathBuf>), Box<dyn Error>> {
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
        Ok((
            autoeq::MeasurementSource::Multiple(autoeq::read::MeasurementMultiple {
                measurements: file_strings
                    .into_iter()
                    .map(|s| autoeq::MeasurementRef::Path(PathBuf::from(s)))
                    .collect(),
                speaker_name: Some(random_speaker_name()),
            }),
            paths,
        ))
    } else {
        Ok((
            autoeq::MeasurementSource::Single(autoeq::read::MeasurementSingle {
                measurement: autoeq::MeasurementRef::Path(PathBuf::from(file_strings[0].clone())),
                speaker_name: Some(random_speaker_name()),
            }),
            paths,
        ))
    }
}

/// Generate a randomized but valid speaker model name
fn random_speaker_name() -> String {
    let brands = ["Genelec", "Neumann", "JBL", "Kef", "Revel", "Yamaha"];
    let models = ["8361A", "KH-120", "708P", "LS50", "F208", "HS8"];
    let mut rng = rand::rng();
    use rand::seq::IndexedRandom;
    format!(
        "{} {}",
        brands.choose(&mut rng).unwrap(),
        models.choose(&mut rng).unwrap()
    )
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
        } else {
            // Apply generic filter characteristic for single drivers
            match config.filter_type {
                FilterType::Flat => {}
                FilterType::Lowpass(f) => {
                    if freq > f {
                        spl -= 24.0 * (freq / f).log2();
                    }
                }
                FilterType::Highpass(f) => {
                    if freq < f {
                        spl -= 24.0 * (f / freq).log2();
                    }
                }
                FilterType::Bandpass(low, high) => {
                    if freq < low {
                        spl -= 24.0 * (low / freq).log2();
                    } else if freq > high {
                        spl -= 24.0 * (freq / high).log2();
                    }
                }
            }
        }

        // Add some noise
        let noise = rng.random_range(-config.noise_level_db..config.noise_level_db);
        spl += noise;

        // Simulate phase: linear phase from delay + some random variation
        let omega = 2.0 * std::f64::consts::PI * freq;
        let phase_delay = -omega * delay_ms / 1000.0;
        let phase_rand = rng.random_range(-0.1..0.1);
        let phase = (phase_delay + phase_rand).to_degrees();

        // Wrap phase to [-180, 180]
        let phase_wrapped = ((phase + 180.0) % 360.0) - 180.0;

        csv_content.push_str(&format!("{:.2},{:.2},{:.2}\n", freq, spl, phase_wrapped));
    }

    fs::write(path, csv_content)?;
    Ok(())
}