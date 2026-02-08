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
    CallbackAction, DspChainOutput, RoomConfig, RoomOptimizationCallback,
    RoomOptimizationProgress, optimize_room, save_dsp_chain,
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

    run(args.sample_rate, config_path, output_path, args.override_config)
}

/// Progress callback that logs to stderr
fn create_progress_callback() -> RoomOptimizationCallback {
    Box::new(|progress: &RoomOptimizationProgress| {
        let pct = (progress.iteration as f64 / progress.max_iterations as f64) * 100.0;
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

/// Keys that are shallow-merged (override individual fields within the object).
/// All other top-level keys are replaced entirely by the override value.
const SHALLOW_MERGE_KEYS: &[&str] = &["optimizer"];

/// Merge two JSON objects: for keys in `SHALLOW_MERGE_KEYS`, shallow-merge individual fields;
/// for all other keys, replace the base value entirely with the override value.
fn merge_json_objects(base: &mut serde_json::Value, overrides: &serde_json::Value) {
    if let (Some(base_obj), Some(override_obj)) = (base.as_object_mut(), overrides.as_object()) {
        for (key, override_value) in override_obj {
            if SHALLOW_MERGE_KEYS.contains(&key.as_str()) {
                // Shallow merge: override individual fields within the object
                if let (Some(base_inner), Some(override_inner)) = (
                    base_obj.get_mut(key).and_then(|v| v.as_object_mut()),
                    override_value.as_object(),
                ) {
                    for (k, v) in override_inner {
                        base_inner.insert(k.clone(), v.clone());
                    }
                } else {
                    base_obj.insert(key.clone(), override_value.clone());
                }
            } else {
                // Replace entirely (speakers, crossovers, group_delay, etc.)
                base_obj.insert(key.clone(), override_value.clone());
            }
        }
    }
}

fn run(
    sample_rate: f64,
    config_path: PathBuf,
    output_path: PathBuf,
    override_config_path: Option<PathBuf>,
) -> Result<()> {
    // Load room configuration
    info!("Loading room configuration from {:?}", config_path);

    let config_json = std::fs::read_to_string(&config_path)
        .with_context(|| format!("Failed to read config file: {:?}", config_path))?;

    let mut config_value: serde_json::Value = serde_json::from_str(&config_json)
        .with_context(|| "Failed to parse room configuration JSON")?;

    // Apply override config if provided
    if let Some(override_path) = override_config_path {
        info!("Loading config overrides from {:?}", override_path);

        let override_json = std::fs::read_to_string(&override_path)
            .with_context(|| format!("Failed to read override config file: {:?}", override_path))?;

        let override_value: serde_json::Value = serde_json::from_str(&override_json)
            .with_context(|| "Failed to parse override config JSON")?;

        merge_json_objects(&mut config_value, &override_value);

        info!("Config overrides applied successfully");
    }

    // Deserialize merged config into RoomConfig
    let mut room_config: RoomConfig = serde_json::from_value(config_value)
        .with_context(|| "Failed to parse merged room configuration")?;

    // Resolve relative paths in the config relative to the config file's directory
    if let Some(config_dir) = config_path.parent() {
        room_config.resolve_paths(config_dir);
    }

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
