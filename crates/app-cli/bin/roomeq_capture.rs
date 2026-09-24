//! RoomEQ multi-microphone capture command.

use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[path = "sotf_recorder_cli/capture.rs"]
mod capture;
mod error_output;

#[derive(Parser)]
#[command(
    name = "roomeq-capture",
    about = "Simultaneous multi-microphone RoomEQ capture"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Correct saved raw captures offline, retaining magnitude-only failures.
    Process {
        /// Directory containing capture-raw.json and its recordings.
        raw_directory: PathBuf,
        /// New directory for processed audio and per-take clock provenance.
        #[arg(long)]
        output_dir: PathBuf,
    },
    /// Check a session declaration without opening audio devices.
    Validate {
        /// Session JSON file.
        plan: PathBuf,
    },
    /// Record every source on all microphones and save raw acquisition artifacts.
    Record {
        /// Session JSON file; calibration paths are relative to this file.
        plan: PathBuf,
        /// New capture directory; its parent must already exist.
        #[arg(long)]
        output_dir: PathBuf,
    },
}

fn main() {
    error_output::set_show_urls(false);
    let result = match Cli::parse().command {
        Command::Process { raw_directory, output_dir } => {
            sotf_audio_player::capture_session::clock::io::process_capture_session(&raw_directory, &output_dir)
                .map(|report| {
                    let fallback = report.takes.iter().filter(|take| take.clock.magnitude_only_reason.is_some()).count();
                let analyzed = report.takes.iter().filter(|take| take.analysis.as_ref().is_some_and(|analysis| analysis.magnitude_file.is_some())).count();
                println!("Processed {} takes; {} require magnitude-only use; {} have calibrated magnitude exports.", report.takes.len(), fallback, analyzed);
                for take in &report.takes {
                    if let Some(analysis) = &take.analysis {
                        for issue in &analysis.issues {
                            println!("{} / {}: {}", take.raw.source_id, take.raw.microphone_id, issue);
                        }
                    }
                }
                for source in &report.reflection_reports {
                    let directions = source.early_reflections.iter().filter(|event|
                        event.direction.is_some() && !event.mirror_ambiguous).count();
                    println!("{}: {} early-arrival candidates; {} conditional directions without mirror ambiguity.",
                        source.source_id, source.early_reflections.len(), directions);
                    for issue in &source.issues { println!("{}: {issue}", source.source_id); }
                    for event in source.direct_sound.iter().chain(&source.early_reflections) {
                        if event.direction.is_none() || event.mirror_ambiguous {
                            println!("{} / {:.2} ms: {}", source.source_id, event.relative_ms, event.issues.join("; "));
                        }
                    }
                }
                println!("Pending: {}", if report.pending_processing.is_empty() { "none".into() }
                    else { report.pending_processing.join(", ") });
                if let Some(manifest) = &report.recording_manifest {
                    println!("Recording manifest: {}", output_dir.join(manifest).display());
                }
                })
        }
        Command::Validate { plan } => capture::validate(&plan),
        Command::Record { plan, output_dir } => capture::record(&plan, &output_dir),
    };
    if let Err(error) = result {
        eprintln!("Error: {}", error_output::redact_secrets(&error));
        std::process::exit(1);
    }
}
