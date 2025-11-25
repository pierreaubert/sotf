//! Room EQ - Multi-channel room equalization optimizer
//!
//! Copyright (C) 2025 Pierre Aubert pierre(at)spinorama(dot)org
//!
//! This program is free software: you can redistribute it and/or modify
//! it under the terms of the GNU General Public License as published by
//! the Free Software Foundation, either version 3 of the License, or
//! (at your option) any later version.
//!
//! This program is distributed in the hope that it will be useful,
//! but WITHOUT ANY WARRANTY; without even the implied warranty of
//! MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
//! GNU General Public License for more details.
//!
//! You should have received a copy of the GNU General Public License
//! along with this program.  If not, see <https://www.gnu.org/licenses/>.

use clap::Parser;
use std::collections::HashMap;
use std::error::Error;
use std::path::PathBuf;

// Include roomeq modules
#[path = "roomeq/crossover_optim.rs"]
mod crossover_optim;
#[path = "roomeq/eq_optim.rs"]
mod eq_optim;
#[path = "roomeq/level_norm.rs"]
mod level_norm;
#[path = "roomeq/load.rs"]
mod load;
#[path = "roomeq/output.rs"]
mod output;
#[path = "roomeq/types.rs"]
mod types;

use types::{ChannelDspChain, OptimizationMetadata, RoomConfig, SpeakerConfig};

/// Room EQ - Optimize multi-channel speaker systems
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to room configuration JSON file
    #[arg(short, long)]
    config: PathBuf,

    /// Output DSP chain JSON file
    #[arg(short, long)]
    output: PathBuf,

    /// Sample rate for filter design (default: 48000 Hz)
    #[arg(long, default_value_t = 48000.0)]
    sample_rate: f64,

    /// Verbose output
    #[arg(short, long)]
    verbose: bool,
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    // Load room configuration
    if args.verbose {
        println!("Loading room configuration from {:?}", args.config);
    }

    let config_json = std::fs::read_to_string(&args.config)?;
    let room_config: RoomConfig = serde_json::from_str(&config_json)?;

    if args.verbose {
        println!("Found {} speakers", room_config.speakers.len());
    }

    // Process each speaker
    let mut channel_chains = HashMap::new();
    let mut pre_scores = Vec::new();
    let mut post_scores = Vec::new();

    for (channel_name, speaker_config) in &room_config.speakers {
        if args.verbose {
            println!("\nProcessing channel: {}", channel_name);
        }

        let (chain, pre_score, post_score) = process_speaker(
            channel_name,
            speaker_config,
            &room_config,
            args.sample_rate,
            args.verbose,
        )?;

        channel_chains.insert(channel_name.clone(), chain);
        pre_scores.push(pre_score);
        post_scores.push(post_score);
    }

    // Aggregate scores (average across channels)
    let avg_pre_score = if !pre_scores.is_empty() {
        pre_scores.iter().sum::<f64>() / pre_scores.len() as f64
    } else {
        0.0
    };
    let avg_post_score = if !post_scores.is_empty() {
        post_scores.iter().sum::<f64>() / post_scores.len() as f64
    } else {
        0.0
    };

    // Create DSP chain output
    let dsp_output = output::create_dsp_chain_output(
        channel_chains,
        Some(OptimizationMetadata {
            pre_score: avg_pre_score,
            post_score: avg_post_score,
            algorithm: room_config.optimizer.algorithm.clone(),
            iterations: room_config.optimizer.max_iter,
            timestamp: chrono::Utc::now().to_rfc3339(),
        }),
    );

    // Save output
    if args.verbose {
        println!("\nSaving DSP chain to {:?}", args.output);
    }

    output::save_dsp_chain(&dsp_output, &args.output)?;

    if args.verbose {
        println!("Done!");
    }

    Ok(())
}

/// Process a single speaker (simple or group)
///
/// Returns: (DSP chain, pre_score, post_score)
fn process_speaker(
    channel_name: &str,
    speaker_config: &SpeakerConfig,
    room_config: &RoomConfig,
    sample_rate: f64,
    verbose: bool,
) -> Result<(ChannelDspChain, f64, f64), Box<dyn Error>> {
    match speaker_config {
        SpeakerConfig::Single(measurement) => {
            // Simple case: single measurement
            process_single_speaker(channel_name, measurement, room_config, sample_rate, verbose)
        }
        SpeakerConfig::Group(group) => {
            // Multi-driver case: optimize crossover and EQ
            process_speaker_group(channel_name, group, room_config, sample_rate, verbose)
        }
    }
}

/// Process a simple speaker with a single measurement
///
/// Returns: (DSP chain, pre_score, post_score)
fn process_single_speaker(
    channel_name: &str,
    measurement: &types::MeasurementRef,
    room_config: &RoomConfig,
    sample_rate: f64,
    verbose: bool,
) -> Result<(ChannelDspChain, f64, f64), Box<dyn Error>> {
    // Load measurement
    let curve = load::load_measurement(measurement)?;

    if verbose {
        println!(
            "  Loaded measurement: {} Hz - {} Hz",
            curve.freq[0],
            curve.freq[curve.freq.len() - 1]
        );
    }

    // Compute pre-score: normalize curve and compute flat loss
    let min_freq = room_config.optimizer.min_freq;
    let max_freq = room_config.optimizer.max_freq;

    // Normalize curve (subtract mean in evaluation range)
    let mut sum = 0.0;
    let mut count = 0;
    for i in 0..curve.freq.len() {
        if curve.freq[i] >= min_freq && curve.freq[i] <= max_freq {
            sum += curve.spl[i];
            count += 1;
        }
    }
    let mean = if count > 0 { sum / count as f64 } else { 0.0 };
    let normalized_spl = &curve.spl - mean;
    let pre_score = autoeq::loss::flat_loss(&curve.freq, &normalized_spl, min_freq, max_freq);

    // Optimize EQ (returns filters and post_score)
    let (eq_filters, post_score) = eq_optim::optimize_channel_eq(&curve, &room_config.optimizer, sample_rate)?;

    if verbose {
        println!("  Optimized {} EQ filters", eq_filters.len());
        println!("  Pre-score: {:.6}, Post-score: {:.6}", pre_score, post_score);
    }

    // Build DSP chain (no gain, no crossover for simple speaker)
    let chain = output::build_channel_dsp_chain(
        channel_name,
        None,
        Vec::new(),
        &eq_filters,
    );

    Ok((chain, pre_score, post_score))
}

/// Process a speaker group with multiple drivers and crossovers
///
/// Returns: (DSP chain, pre_score, post_score)
fn process_speaker_group(
    channel_name: &str,
    group: &types::SpeakerGroup,
    room_config: &RoomConfig,
    sample_rate: f64,
    verbose: bool,
) -> Result<(ChannelDspChain, f64, f64), Box<dyn Error>> {
    // Load all measurements in the group
    let mut driver_curves = Vec::new();
    for measurement in &group.measurements {
        let curve = load::load_measurement(measurement)?;
        driver_curves.push(curve);
    }

    if verbose {
        println!("  Loaded {} driver measurements", driver_curves.len());
    }

    // Get crossover configuration
    let crossover_config = if let Some(crossover_ref) = &group.crossover {
        room_config
            .crossovers
            .as_ref()
            .and_then(|xovers| xovers.get(crossover_ref))
            .ok_or_else(|| format!("Crossover configuration '{}' not found", crossover_ref))?
    } else {
        return Err("Speaker group requires crossover configuration".into());
    };

    let crossover_type = crossover_optim::parse_crossover_type(&crossover_config.crossover_type)?;

    // Compute pre-score: use initial gains (0 dB) and geometric mean crossover frequencies
    let n_drivers = driver_curves.len();
    let initial_gains = vec![0.0; n_drivers];

    // Compute geometric mean crossover frequencies as initial guess
    let mut initial_xover_freqs = Vec::new();
    for i in 0..(n_drivers - 1) {
        // Geometric mean between adjacent driver frequency ranges
        let lower_mean = driver_curves[i].freq.iter().sum::<f64>() / driver_curves[i].freq.len() as f64;
        let upper_mean = driver_curves[i + 1].freq.iter().sum::<f64>() / driver_curves[i + 1].freq.len() as f64;
        let geom_mean = (lower_mean * upper_mean).sqrt();
        initial_xover_freqs.push(geom_mean);
    }

    // Convert curves to DriverMeasurement
    let driver_measurements: Vec<autoeq::loss::DriverMeasurement> = driver_curves.iter()
        .map(|curve| autoeq::loss::DriverMeasurement {
            freq: curve.freq.clone(),
            spl: curve.spl.clone(),
            phase: None,
        })
        .collect();

    let drivers_data = autoeq::loss::DriversLossData::new(driver_measurements, crossover_type);
    let pre_score = autoeq::loss::drivers_flat_loss(
        &drivers_data,
        &initial_gains,
        &initial_xover_freqs,
        sample_rate,
        room_config.optimizer.min_freq,
        room_config.optimizer.max_freq,
    );

    // Optimize crossover
    let (gains, crossover_freqs, combined_curve) = crossover_optim::optimize_crossover(
        driver_curves,
        crossover_type,
        sample_rate,
        room_config.optimizer.min_freq,
        room_config.optimizer.max_freq,
    )?;

    if verbose {
        println!(
            "  Optimized crossover: freqs={:?}, gains={:?}",
            crossover_freqs, gains
        );
    }

    // Optimize EQ on the combined response (returns filters and post_score)
    let (eq_filters, post_score) =
        eq_optim::optimize_channel_eq(&combined_curve, &room_config.optimizer, sample_rate)?;

    if verbose {
        println!("  Optimized {} EQ filters", eq_filters.len());
        println!("  Pre-score: {:.6}, Post-score: {:.6}", pre_score, post_score);
    }

    // Build multi-driver DSP chain with per-driver crossovers
    let chain = output::build_multidriver_dsp_chain(
        channel_name,
        &gains,
        &crossover_freqs,
        crossover_optim::crossover_type_to_string(&crossover_type),
        &eq_filters,
    );

    Ok((chain, pre_score, post_score))
}
