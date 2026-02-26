//! CLI tool for computing bliss audio analysis values
//!
//! This tool scans the music library database and computes bliss audio analysis
//! features for all tracks that don't have them yet. These features can be used
//! for music similarity detection and intelligent playlist generation.
//!
//! Usage:
//!   sotf-bliss-scan              # Scan tracks without bliss analysis
//!   sotf-bliss-scan --all        # Rescan all tracks (recompute existing)
//!   sotf-bliss-scan --jobs 4     # Use 4 worker threads
//!   sotf-bliss-scan <file>       # Analyze a single file

use clap::Parser;
use sotf_audio_player::bliss::{BlissScanManager, BlissScanMessage, analyze_file};
use sotf_audio_player::config;
use sotf_audio_player::database::MusicDatabase;
use std::path::{Path, PathBuf};
use std::sync::mpsc::TryRecvError;
use std::time::Instant;

#[derive(Parser, Debug)]
#[command(name = "sotf-bliss-scan")]
#[command(about = "Compute bliss audio analysis values for music similarity")]
#[command(
    long_about = "Analyzes audio files to extract features like tempo, spectral characteristics, \
                        and loudness that can be used for finding similar tracks."
)]
struct Args {
    /// Single file to analyze (prints analysis to stdout)
    #[arg(value_name = "FILE")]
    file: Option<PathBuf>,

    /// Rescan all tracks, even those already analyzed
    #[arg(short, long)]
    all: bool,

    /// Number of worker threads (default: number of CPUs, max 4)
    #[arg(short, long)]
    jobs: Option<usize>,

    /// Verbose output
    #[arg(short, long)]
    verbose: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    let log_level = if std::env::var("RUST_LOG").is_ok() {
        log::LevelFilter::Debug
    } else {
        log::LevelFilter::Info
    };

    env_logger::Builder::from_default_env()
        .filter_level(log_level)
        .init();

    let args = Args::parse();

    // Handle single file analysis
    if let Some(file) = args.file {
        return analyze_single_file(&file, args.verbose);
    }

    // Batch analysis from database
    let db_path = config::get_music_db_path().ok_or("Could not determine database path")?;

    log::info!("Opening database at: {:?}", db_path);
    let db = MusicDatabase::open(&db_path)?;

    // Get tracks to analyze
    let tracks = if args.all {
        log::info!("Scanning all tracks (--all flag)");
        db.get_all_track_paths()?
    } else {
        log::info!("Scanning tracks without bliss analysis");
        db.get_tracks_without_bliss()?
    };

    if tracks.is_empty() {
        println!("No tracks to analyze.");
        return Ok(());
    }

    println!("Found {} tracks to analyze", tracks.len());

    // Determine number of worker threads
    let num_threads = args.jobs.unwrap_or_else(|| num_cpus::get().clamp(1, 4));
    println!("Using {} worker threads", num_threads);

    // Start scanning
    let mut manager = BlissScanManager::new();
    let start_time = Instant::now();

    manager.start(db_path.clone(), tracks);

    // Process messages and show progress
    let mut last_progress = 0.0f32;

    while manager.in_progress {
        manager.update();

        // Show progress updates
        let progress = manager.progress();
        if progress - last_progress >= 1.0 || manager.processed == manager.total {
            print!(
                "\rProgress: {}/{} ({:.1}%) - {} succeeded, {} failed",
                manager.processed, manager.total, progress, manager.succeeded, manager.failed
            );
            std::io::Write::flush(&mut std::io::stdout())?;
            last_progress = progress;
        }

        // Show detailed messages in verbose mode
        if args.verbose
            && let Some(scanner) = &manager.scanner
        {
            let rx = scanner.messages();
            let rx = rx.lock().unwrap();

            loop {
                match rx.try_recv() {
                    Ok(msg) => match msg {
                        BlissScanMessage::Started { path } => {
                            println!("\n  Analyzing: {}", path.display());
                        }
                        BlissScanMessage::Success {
                            path,
                            tempo,
                            features_count,
                        } => {
                            println!(
                                "\n  Done: {} (tempo={:.1} BPM, {} features)",
                                path.display(),
                                tempo,
                                features_count
                            );
                        }
                        BlissScanMessage::Error { path, error } => {
                            println!("\n  Error: {} - {}", path.display(), error);
                        }
                        BlissScanMessage::Complete { .. } => {}
                    },
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => break,
                }
            }
        }

        // Small sleep to avoid busy waiting
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    let elapsed = start_time.elapsed();
    println!("\n");
    println!("Bliss analysis complete!");
    println!(
        "  Processed: {} tracks in {:.1}s",
        manager.total,
        elapsed.as_secs_f64()
    );
    println!("  Succeeded: {}", manager.succeeded);
    println!("  Failed: {}", manager.failed);

    if manager.total > 0 {
        let avg_time = elapsed.as_secs_f64() / manager.total as f64;
        println!("  Average: {:.2}s per track", avg_time);
    }

    Ok(())
}

fn analyze_single_file(path: &Path, verbose: bool) -> Result<(), Box<dyn std::error::Error>> {
    println!("Analyzing: {}", path.display());

    let start = Instant::now();
    let analysis = analyze_file(path)?;
    let elapsed = start.elapsed();

    println!("\nBliss Analysis Results:");
    println!("  Tempo: {:.1} BPM", analysis.tempo);
    println!("  Zero-crossing rate: {:.6}", analysis.zcr);
    println!(
        "  Spectral centroid (mean): {:.2}",
        analysis.spectral_centroid_mean
    );
    println!(
        "  Spectral rolloff (mean): {:.2}",
        analysis.spectral_rolloff_mean
    );
    println!(
        "  Spectral flatness (mean): {:.6}",
        analysis.spectral_flatness_mean
    );
    println!("  Loudness (mean): {:.2}", analysis.loudness_mean);
    println!("\n  Analysis took: {:.2}s", elapsed.as_secs_f64());

    if verbose {
        println!(
            "\n  Full feature vector ({} features):",
            analysis.features.len()
        );
        for (i, f) in analysis.features.iter().enumerate() {
            println!("    [{:2}] {:.6}", i, f);
        }
    }

    Ok(())
}
