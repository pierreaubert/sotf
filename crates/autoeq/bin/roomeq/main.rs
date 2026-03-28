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
    CallbackAction, DspChainOutput, ExportFormat, RoomConfig, RoomOptimizationCallback,
    RoomOptimizationProgress, export_dsp_chain, load_config, optimize_room, save_dsp_chain,
};

/// Room EQ - Optimize multi-channel speaker systems
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to room configuration JSON file
    #[arg(short, long, required_unless_present_any = ["schema", "convert"])]
    config: Option<PathBuf>,

    /// Output DSP chain JSON file
    #[arg(short, long, required_unless_present_any = ["schema", "convert"])]
    output: Option<PathBuf>,

    /// Sample rate for filter design (default: 48000 Hz)
    #[arg(long, default_value_t = 48000.0)]
    sample_rate: f64,

    /// Verbose output (deprecated, use RUST_LOG env var)
    #[arg(short, long)]
    verbose: bool,

    /// Dump JSON schema and exit. Values: "input" (RoomConfig), "output" (DspChainOutput)
    #[arg(long, value_name = "TYPE")]
    schema: Option<String>,

    /// Path to override config JSON file (overrides any section: optimizer, speakers, crossovers, group_delay, etc.)
    #[arg(long, alias = "optim-config")]
    override_config: Option<PathBuf>,

    /// Export DSP chain to external format (camilladsp, apo, easyeffects, wavelet, pipewire)
    #[arg(long, value_enum)]
    export_format: Option<ExportFormat>,

    /// Export output file path (defaults to output path with format-appropriate extension)
    #[arg(long)]
    export_path: Option<PathBuf>,

    /// Convert an existing DSP chain JSON to an export format (no optimization)
    #[arg(long)]
    convert: Option<PathBuf>,
}

fn main() -> Result<()> {
    // Initialize logger safely
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let args = Args::parse();

    if let Some(schema_type) = &args.schema {
        let json = match schema_type.as_str() {
            "input" => {
                let schema = schema_for!(RoomConfig);
                serde_json::to_string_pretty(&schema).unwrap()
            }
            "output" => {
                let schema = schema_for!(DspChainOutput);
                serde_json::to_string_pretty(&schema).unwrap()
            }
            other => {
                eprintln!("Unknown schema type: {other}. Use 'input' or 'output'.");
                std::process::exit(1);
            }
        };
        println!("{json}");
        return Ok(());
    }

    if args.verbose {
        warn!("The --verbose flag is deprecated. Use RUST_LOG=debug instead.");
    }

    // Convert mode: load existing DSP chain JSON and export
    if let Some(convert_path) = &args.convert {
        let format = args
            .export_format
            .ok_or_else(|| anyhow!("--export-format is required with --convert"))?;

        let json_str = std::fs::read_to_string(convert_path)
            .with_context(|| format!("Failed to read DSP chain from {:?}", convert_path))?;
        let dsp_output: DspChainOutput = serde_json::from_str(&json_str)
            .with_context(|| format!("Failed to parse DSP chain from {:?}", convert_path))?;

        let export_path = args.export_path.unwrap_or_else(|| {
            convert_path.with_extension(format.default_extension())
        });

        info!("Converting {:?} to {:?} format", convert_path, format);
        export_dsp_chain(&dsp_output, format, &export_path, args.sample_rate)?;
        info!("Exported to {:?}", export_path);
        return Ok(());
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
        args.export_format,
        args.export_path,
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
    export_format: Option<ExportFormat>,
    export_path: Option<PathBuf>,
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

    // Export to external format if requested
    if let Some(format) = export_format {
        let path = export_path
            .unwrap_or_else(|| output_path.with_extension(format.default_extension()));
        info!("Exporting DSP chain to {:?} ({:?})", path, format);
        export_dsp_chain(&dsp_output, format, &path, sample_rate)?;
        info!("Exported to {:?}", path);
    }

    info!("Done!");

    Ok(())
}
