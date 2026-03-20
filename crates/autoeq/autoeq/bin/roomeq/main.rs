//! Room EQ - Multi-channel room equalization optimizer
//!
//! Copyright (C) 2025-2026 Pierre Aubert pierre(at)spinorama(dot)org
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

use anyhow::{Context, Result, anyhow};
use clap::Parser;
use log::{info, warn};
use schemars::schema_for;
use std::path::PathBuf;

// Use the library types
use autoeq::roomeq::{
    CallbackAction, DspChainOutput, RoomOptimizationCallback, RoomOptimizationProgress,
    load_config, optimize_room, save_dsp_chain,
};

/// Room EQ - Optimize multi-channel speaker systems
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to room configuration JSON file
    #[arg(short, long, required_unless_present = "schema")]
    config: Option<PathBuf>,

    /// Output DSP chain JSON file
    #[arg(short, long, required_unless_present = "schema")]
    output: Option<PathBuf>,

    /// Sample rate for filter design (default: 48000 Hz)
    #[arg(long, default_value_t = 48000.0)]
    sample_rate: f64,

    /// Verbose output (deprecated, use RUST_LOG env var)
    #[arg(short, long)]
    verbose: bool,

    /// Dump JSON schema for the output format
    #[arg(long)]
    schema: bool,

    /// Path to override config JSON file (overrides any section: optimizer, speakers, crossovers, group_delay, etc.)
    #[arg(long, alias = "optim-config")]
    override_config: Option<PathBuf>,
}

fn main() -> Result<()> {
    // Initialize logger safely
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let args = Args::parse();

    if args.schema {
        let schema = schema_for!(DspChainOutput);
        println!("{}", serde_json::to_string_pretty(&schema).unwrap());
        return Ok(());
    }

    if args.verbose {
        warn!("The --verbose flag is deprecated. Use RUST_LOG=debug instead.");
    }

    // Unwrap required args (safe because of required_unless_present)
    let config_path = args
        .config
        .ok_or_else(|| anyhow!("Config file is required"))?;
    let output_path = args
        .output
        .ok_or_else(|| anyhow!("Output file is required"))?;

    run(
        args.sample_rate,
        config_path,
        output_path,
        args.override_config,
    )
}

/// Progress callback that logs to stderr
fn create_progress_callback() -> RoomOptimizationCallback {
    Box::new(|progress: &RoomOptimizationProgress| {
        // Status messages (no real iteration data) — log the message directly
        if let Some(msg) = &progress.message {
            info!("  {}", msg);
            return CallbackAction::Continue;
        }

        let pct = if progress.max_iterations > 0 {
            (progress.iteration as f64 / progress.max_iterations as f64) * 100.0
        } else {
            0.0
        };
        // Log every 100 iterations
        if progress.iteration.is_multiple_of(100) {
            info!(
                "  [{}] ({}/{}) {:.1}% | iter {}/{} | loss: {:.6}",
                progress.current_speaker,
                progress.speaker_index + 1,
                progress.total_speakers,
                pct,
                progress.iteration,
                progress.max_iterations,
                progress.loss
            );
        }
        CallbackAction::Continue
    })
}

fn run(
    sample_rate: f64,
    config_path: PathBuf,
    output_path: PathBuf,
    override_config_path: Option<PathBuf>,
) -> Result<()> {
    // Load room configuration
    info!("Loading room configuration from {:?}", config_path);

    let (room_config, _config_dir) = load_config(&config_path, override_config_path.as_deref())?;

    info!("Found {} speakers", room_config.speakers.len());

    // Run optimization using the library
    let callback = create_progress_callback();
    let out_dir = output_path.parent();
    let result = optimize_room(&room_config, sample_rate, Some(callback), out_dir)
        .map_err(|e| anyhow!("{}", e))
        .with_context(|| "Room optimization failed")?;

    // Log summary
    info!(
        "Average pre-score: {:.4}, post-score: {:.4}",
        result.combined_pre_score, result.combined_post_score
    );

    // Save output
    info!("Saving DSP chain to {:?}", output_path);

    let dsp_output = result.to_dsp_chain_output();
    save_dsp_chain(&dsp_output, &output_path)
        .map_err(|e| anyhow!("{}", e))
        .with_context(|| format!("Failed to save DSP chain to {:?}", output_path))?;

    info!("Done!");

    Ok(())
}
