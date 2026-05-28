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
    DspChainOutput, ExportFormat, PipelineControl, PipelineEvent, PipelineObserver, RoomConfig,
    RoomPipeline, RoomPipelineRequest, export_dsp_chain_with_convolution_sidecars, load_config,
    save_dsp_chain, validate_room_config,
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

    /// Path to override config JSON file (overrides any section: optimizer, speakers, crossovers, etc.)
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

    /// Validate configuration and check measurement files exist, but do not run optimization
    #[arg(long)]
    dry_run: bool,
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

        let export_path = args
            .export_path
            .unwrap_or_else(|| convert_path.with_extension(format.default_extension()));
        let source_dir = convert_path
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."));

        info!("Converting {:?} to {:?} format", convert_path, format);
        export_dsp_chain_with_convolution_sidecars(
            &dsp_output,
            format,
            &export_path,
            args.sample_rate,
            source_dir,
        )?;
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

    // Dry-run mode: validate config and check files exist
    if args.dry_run {
        return run_dry_run(config_path, args.override_config);
    }

    run(
        args.sample_rate,
        config_path,
        output_path,
        args.override_config,
        args.export_format,
        args.export_path,
    )
}

/// Pipeline observer that logs to stderr.
fn create_progress_observer() -> Box<dyn PipelineObserver> {
    Box::new(|event: &PipelineEvent| {
        // Status messages (no real iteration data) — log the message directly
        if let Some(msg) = &event.message {
            info!("  {}", msg);
            return PipelineControl::Continue;
        }

        let iteration = event.iteration.unwrap_or(0);
        let max_iterations = event.max_iterations.unwrap_or(0);
        let pct = if max_iterations > 0 {
            (iteration as f64 / max_iterations as f64) * 100.0
        } else {
            0.0
        };
        // Log every 100 iterations
        if iteration.is_multiple_of(100) {
            info!(
                "  [{}] ({}/{}) {:.1}% | iter {}/{} | loss: {:.6}",
                event.channel.as_deref().unwrap_or(""),
                event.channel_index.unwrap_or(0) + 1,
                event.total_channels.unwrap_or(0),
                pct,
                iteration,
                max_iterations,
                event.loss.unwrap_or(0.0)
            );
        }
        PipelineControl::Continue
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
    let observer = create_progress_observer();
    let out_dir = output_path.parent();
    let result = RoomPipeline::new(RoomPipelineRequest {
        config: &room_config,
        sample_rate,
        output_dir: out_dir,
        probe_arrival_overrides: None,
    })
    .run(Some(observer))
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
        let path = export_path.unwrap_or_else(|| format.default_export_path(&output_path));
        let source_dir = output_path
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."));
        info!("Exporting DSP chain to {:?} ({:?})", path, format);
        export_dsp_chain_with_convolution_sidecars(
            &dsp_output,
            format,
            &path,
            sample_rate,
            source_dir,
        )?;
        info!("Exported to {:?}", path);
    }

    info!("Done!");

    Ok(())
}

/// Validate configuration and check measurement files exist without running optimization
fn run_dry_run(config_path: PathBuf, override_config_path: Option<PathBuf>) -> Result<()> {
    info!("Loading room configuration from {:?}", config_path);

    let (room_config, _config_dir) = load_config(&config_path, override_config_path.as_deref())?;

    println!("\n=== Configuration Validation ===\n");

    // Run validation
    let validation = validate_room_config(&room_config);

    if validation.is_valid {
        println!("Configuration: VALID");
    } else {
        println!("Configuration: INVALID");
    }

    if !validation.warnings.is_empty() {
        println!("\nWarnings:");
        for warning in &validation.warnings {
            println!("  - {}", warning);
        }
    }

    if !validation.errors.is_empty() {
        println!("\nErrors:");
        for error in &validation.errors {
            println!("  - {}", error);
        }
    }

    println!("\n=== Speaker Configuration ===\n");
    println!("Found {} speakers:", room_config.speakers.len());

    let mut file_errors = Vec::new();

    for (name, speaker_config) in &room_config.speakers {
        println!("\n  Speaker: {}", name);

        // Get measurement paths for this speaker
        let paths = collect_measurement_paths(speaker_config);
        for path in &paths {
            if path.exists() {
                println!("    [OK] {:?}", path);
            } else {
                println!("    [MISSING] {:?}", path);
                file_errors.push(format!("Speaker '{}': file not found: {:?}", name, path));
            }
        }
    }

    println!("\n=== Result ===\n");

    if !validation.errors.is_empty() || !file_errors.is_empty() {
        println!("VALIDATION FAILED");
        if !validation.errors.is_empty() {
            println!("  {} configuration error(s)", validation.errors.len());
        }
        if !file_errors.is_empty() {
            println!("  {} file(s) missing", file_errors.len());
            for error in &file_errors {
                println!("    - {}", error);
            }
        }
        anyhow::bail!("Configuration validation failed");
    }

    println!("All checks passed! Configuration is valid and all files exist.");
    Ok(())
}

/// Collect all measurement file paths from a speaker configuration
fn collect_measurement_paths(
    speaker_config: &autoeq::roomeq::SpeakerConfig,
) -> Vec<std::path::PathBuf> {
    use autoeq::MeasurementSource;
    use autoeq::roomeq::SpeakerConfig;

    fn extract_paths_from_source(source: &MeasurementSource) -> Vec<std::path::PathBuf> {
        let mut paths = Vec::new();
        match source {
            MeasurementSource::Single(single) => {
                if let Some(p) = single.measurement.path() {
                    paths.push(p.clone());
                }
            }
            MeasurementSource::Multiple(mult) => {
                for m in &mult.measurements {
                    if let Some(p) = m.path() {
                        paths.push(p.clone());
                    }
                }
            }
            MeasurementSource::InMemory(_) | MeasurementSource::InMemoryMultiple(_) => {}
        }
        paths
    }

    match speaker_config {
        SpeakerConfig::Single(source) => extract_paths_from_source(source),
        SpeakerConfig::Group(group) => {
            let mut paths = Vec::new();
            for source in &group.measurements {
                paths.extend(extract_paths_from_source(source));
            }
            paths
        }
        SpeakerConfig::MultiSub(ms) => {
            let mut paths = Vec::new();
            for source in &ms.subwoofers {
                paths.extend(extract_paths_from_source(source));
            }
            paths
        }
        SpeakerConfig::Dba(dba) => {
            let mut paths = Vec::new();
            for source in &dba.front {
                paths.extend(extract_paths_from_source(source));
            }
            for source in &dba.rear {
                paths.extend(extract_paths_from_source(source));
            }
            paths
        }
        SpeakerConfig::Cardioid(cardioid) => {
            let mut paths = Vec::new();
            paths.extend(extract_paths_from_source(&cardioid.front));
            paths.extend(extract_paths_from_source(&cardioid.rear));
            paths
        }
    }
}
