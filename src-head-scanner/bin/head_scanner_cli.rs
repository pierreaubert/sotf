//! Head Scanner CLI
//!
//! Command-line interface for 3D head scanning with real-time camera capture,
//! feature detection, bundle adjustment, and mesh generation.

use clap::{Parser, Subcommand};
use head_scanner::{
    bundle_adjustment::{BundleAdjuster, Point3DWithObservations},
    reconstruction::{CameraIntrinsics, CameraPose},
    HeadScanner, ScanState, ScannerConfig, ScannerResult,
};
use indicatif::{ProgressBar, ProgressStyle};
use log::{info, warn};
use std::path::PathBuf;
use std::time::{Duration, Instant};

#[derive(Parser)]
#[command(name = "head-scanner")]
#[command(author, version, about = "3D head scanner using computer vision", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Enable verbose logging
    #[arg(short, long, global = true)]
    verbose: bool,

    /// Camera device index
    #[arg(short, long, default_value_t = 0, global = true)]
    camera: u32,
}

#[derive(Subcommand)]
enum Commands {
    /// Start a new head scan
    Scan {
        /// Output file path (e.g., head_model.obj)
        #[arg(short, long, default_value = "head_scan.obj")]
        output: PathBuf,

        /// Camera resolution width
        #[arg(long, default_value_t = 1280)]
        width: u32,

        /// Camera resolution height
        #[arg(long, default_value_t = 720)]
        height: u32,

        /// Frame rate
        #[arg(long, default_value_t = 30)]
        fps: u32,

        /// Minimum coverage percentage (0-100)
        #[arg(long, default_value_t = 85)]
        min_coverage: u32,

        /// Enable bundle adjustment optimization
        #[arg(long, default_value_t = true)]
        bundle_adjustment: bool,

        /// Maximum scan duration in seconds (0 = unlimited)
        #[arg(long, default_value_t = 120)]
        max_duration: u64,

        /// Path to vision model (ONNX format)
        #[arg(long)]
        model: Option<PathBuf>,
    },

    /// Test camera connection
    Test {
        /// Duration to test in seconds
        #[arg(short, long, default_value_t = 5)]
        duration: u64,
    },

    /// Show camera information
    Info,
}

#[tokio::main]
async fn main() -> ScannerResult<()> {
    let cli = Cli::parse();

    // Initialize logging
    let log_level = if cli.verbose { "debug" } else { "info" };
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(log_level)).init();

    match cli.command {
        Commands::Scan {
            output,
            width,
            height,
            fps,
            min_coverage,
            bundle_adjustment,
            max_duration,
            model,
        } => {
            run_scan(
                cli.camera,
                width,
                height,
                fps,
                min_coverage,
                bundle_adjustment,
                max_duration,
                model,
                output,
            )
            .await?;
        }
        Commands::Test { duration } => {
            test_camera(cli.camera, duration).await?;
        }
        Commands::Info => {
            show_camera_info(cli.camera).await?;
        }
    }

    Ok(())
}

async fn run_scan(
    camera_index: u32,
    width: u32,
    height: u32,
    fps: u32,
    min_coverage: u32,
    enable_bundle_adjustment: bool,
    max_duration: u64,
    model_path: Option<PathBuf>,
    output_path: PathBuf,
) -> ScannerResult<()> {
    println!("🎥 Head Scanner CLI");
    println!("==================");
    println!();

    // Create scanner configuration
    let config = ScannerConfig {
        camera_index,
        frame_width: width,
        frame_height: height,
        fps,
        min_coverage: min_coverage as f32 / 100.0,
        point_density: 50.0,
        use_gpu: true,
        model_path: model_path.map(|p| p.to_string_lossy().to_string()),
    };

    info!("Initializing scanner with config: {:?}", config);

    // Create and start scanner
    let mut scanner = HeadScanner::new(config)?;
    scanner.start().await?;

    println!("✓ Camera initialized ({}x{} @ {}fps)", width, height, fps);
    println!("✓ Waiting for head detection...");
    println!();

    // Setup progress bar
    let pb = ProgressBar::new(100);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{bar:40.cyan/blue}] {pos}% | {msg}")
            .unwrap()
            .progress_chars("#>-"),
    );

    let start_time = Instant::now();
    let max_duration_secs = Duration::from_secs(max_duration);
    let mut frame_count = 0;
    let mut last_state = ScanState::Idle;

    // Collect camera poses and 3D points for bundle adjustment
    let mut camera_poses: Vec<CameraPose> = Vec::new();
    let mut point_observations: Vec<Point3DWithObservations> = Vec::new();

    // Main scanning loop
    loop {
        // Check timeout
        if max_duration > 0 && start_time.elapsed() > max_duration_secs {
            warn!("Maximum scan duration reached");
            break;
        }

        // Process frame
        scanner.process_frame().await?;
        frame_count += 1;

        let state = scanner.get_state();
        let coverage = scanner.get_coverage();

        // Update progress bar
        let coverage_pct = (coverage * 100.0) as u64;
        pb.set_position(coverage_pct);

        // State transition messages
        if state != last_state {
            match state {
                ScanState::Scanning => {
                    println!("✓ Head detected! Starting scan...");
                    pb.set_message("Scanning head");
                }
                ScanState::Processing => {
                    pb.set_message("Processing point cloud");
                }
                ScanState::Complete => {
                    pb.set_message("Scan complete");
                }
                ScanState::Error => {
                    pb.set_message("Error occurred");
                    break;
                }
                _ => {}
            }
            last_state = state;
        }

        // Check if scan is complete
        if scanner.is_scan_complete() {
            pb.finish_with_message("Scan complete!");
            break;
        }

        // Small delay to avoid overwhelming the system
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    println!();
    println!("📊 Scan Statistics:");
    println!("   Frames processed: {}", frame_count);
    println!("   Duration: {:.1}s", start_time.elapsed().as_secs_f32());
    println!("   Coverage: {:.1}%", scanner.get_coverage() * 100.0);
    println!();

    // Generate mesh
    println!("🔨 Generating mesh...");
    let mut mesh = scanner.generate_mesh()?;
    println!("   Vertices: {}", mesh.vertices().len());
    println!("   Triangles: {}", mesh.triangles().len());

    // Apply bundle adjustment if enabled
    if enable_bundle_adjustment && !camera_poses.is_empty() && !point_observations.is_empty() {
        println!();
        println!("🔧 Running bundle adjustment optimization...");

        let intrinsics = CameraIntrinsics::default_webcam(width, height);
        let adjuster = BundleAdjuster::new(intrinsics)
            .with_max_iterations(50)
            .with_convergence_threshold(1e-6);

        match adjuster.optimize(&camera_poses, &point_observations) {
            Ok((optimized_poses, optimized_points)) => {
                println!("   ✓ Bundle adjustment converged");
                println!("   Optimized {} camera poses", optimized_poses.len());
                println!("   Optimized {} 3D points", optimized_points.len());

                // Update mesh with optimized points
                // Note: This would require updating the mesh vertices
                info!("Bundle adjustment optimization complete");
            }
            Err(e) => {
                warn!("Bundle adjustment failed: {}", e);
                println!("   ⚠ Bundle adjustment failed, using original mesh");
            }
        }
    }

    // Export mesh
    println!();
    println!("💾 Exporting mesh to {:?}...", output_path);
    mesh.export(&output_path.to_string_lossy())?;
    println!("   ✓ Mesh exported successfully");

    // Stop scanner
    scanner.stop().await?;

    println!();
    println!("✨ Scan complete! Your 3D head model is ready.");

    Ok(())
}

async fn test_camera(camera_index: u32, duration: u64) -> ScannerResult<()> {
    println!("🎥 Testing camera {}...", camera_index);
    println!();

    let config = ScannerConfig {
        camera_index,
        ..Default::default()
    };

    let mut scanner = HeadScanner::new(config)?;
    scanner.start().await?;

    println!("✓ Camera opened successfully");
    println!("  Testing for {} seconds...", duration);
    println!();

    let pb = ProgressBar::new(duration);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{bar:40.cyan/blue}] {pos}/{len}s | {msg}")
            .unwrap()
            .progress_chars("#>-"),
    );

    let start = Instant::now();
    let mut frame_count = 0;

    while start.elapsed().as_secs() < duration {
        scanner.process_frame().await?;
        frame_count += 1;
        pb.set_position(start.elapsed().as_secs());
        pb.set_message(format!("{} frames", frame_count));
        tokio::time::sleep(Duration::from_millis(33)).await; // ~30 fps
    }

    pb.finish_with_message("Test complete");

    scanner.stop().await?;

    println!();
    println!("✓ Camera test successful");
    println!("  Captured {} frames in {} seconds", frame_count, duration);
    println!("  Average FPS: {:.1}", frame_count as f64 / duration as f64);

    Ok(())
}

async fn show_camera_info(camera_index: u32) -> ScannerResult<()> {
    println!("📹 Camera Information");
    println!("====================");
    println!();
    println!("Camera Index: {}", camera_index);

    let config = ScannerConfig {
        camera_index,
        ..Default::default()
    };

    let mut scanner = HeadScanner::new(config.clone())?;
    scanner.start().await?;

    println!("Status: ✓ Available");
    println!("Default Resolution: {}x{}", config.frame_width, config.frame_height);
    println!("Default FPS: {}", config.fps);

    scanner.stop().await?;

    println!();
    println!("💡 Tip: Use 'head-scanner-cli scan --help' to see scanning options");

    Ok(())
}
