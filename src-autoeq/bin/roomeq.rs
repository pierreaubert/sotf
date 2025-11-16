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
#[path = "roomeq/types.rs"]
mod types;
#[path = "roomeq/load.rs"]
mod load;
#[path = "roomeq/level_norm.rs"]
mod level_norm;
#[path = "roomeq/crossover_optim.rs"]
mod crossover_optim;
#[path = "roomeq/eq_optim.rs"]
mod eq_optim;
#[path = "roomeq/output.rs"]
mod output;

use types::{RoomConfig, SpeakerConfig, ChannelDspChain, OptimizationMetadata};

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

    for (channel_name, speaker_config) in &room_config.speakers {
        if args.verbose {
            println!("\nProcessing channel: {}", channel_name);
        }

        let chain = process_speaker(
            channel_name,
            speaker_config,
            &room_config,
            args.sample_rate,
            args.verbose,
        )?;

        channel_chains.insert(channel_name.clone(), chain);
    }

    // Create DSP chain output
    let dsp_output = output::create_dsp_chain_output(
        channel_chains,
        Some(OptimizationMetadata {
            pre_score: 0.0,  // TODO: Compute actual scores
            post_score: 0.0,
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
fn process_speaker(
    channel_name: &str,
    speaker_config: &SpeakerConfig,
    room_config: &RoomConfig,
    sample_rate: f64,
    verbose: bool,
) -> Result<ChannelDspChain, Box<dyn Error>> {
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
fn process_single_speaker(
    channel_name: &str,
    measurement: &types::MeasurementRef,
    room_config: &RoomConfig,
    sample_rate: f64,
    verbose: bool,
) -> Result<ChannelDspChain, Box<dyn Error>> {
    // Load measurement
    let curve = load::load_measurement(measurement)?;

    if verbose {
        println!("  Loaded measurement: {} Hz - {} Hz", curve.freq[0], curve.freq[curve.freq.len() - 1]);
    }

    // Optimize EQ
    let eq_filters = eq_optim::optimize_channel_eq(&curve, &room_config.optimizer, sample_rate)?;

    if verbose {
        println!("  Optimized {} EQ filters", eq_filters.len());
    }

    // Build DSP chain (no gain, no crossover for simple speaker)
    Ok(output::build_channel_dsp_chain(
        channel_name,
        None,
        Vec::new(),
        &eq_filters,
    ))
}

/// Process a speaker group with multiple drivers and crossovers
fn process_speaker_group(
    channel_name: &str,
    group: &types::SpeakerGroup,
    room_config: &RoomConfig,
    sample_rate: f64,
    verbose: bool,
) -> Result<ChannelDspChain, Box<dyn Error>> {
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

    // Optimize crossover
    let (gains, crossover_freqs, combined_curve) = crossover_optim::optimize_crossover(
        driver_curves,
        crossover_type,
        sample_rate,
        room_config.optimizer.min_freq,
        room_config.optimizer.max_freq,
    )?;

    if verbose {
        println!("  Optimized crossover: freqs={:?}, gains={:?}", crossover_freqs, gains);
    }

    // Optimize EQ on the combined response
    let eq_filters = eq_optim::optimize_channel_eq(&combined_curve, &room_config.optimizer, sample_rate)?;

    if verbose {
        println!("  Optimized {} EQ filters", eq_filters.len());
    }

    // Build crossover plugins
    // For now, we'll create a simplified chain. A full implementation would
    // create separate paths for each driver with appropriate crossovers.
    let crossover_plugins = Vec::new(); // TODO: Implement full multi-driver DSP chain

    // Build DSP chain
    Ok(output::build_channel_dsp_chain(
        channel_name,
        Some(gains[0]), // Use first driver's gain as overall gain
        crossover_plugins,
        &eq_filters,
    ))
}
