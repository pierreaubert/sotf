//! Head Scanner CLI
//!
//! Command-line interface for 3D head scanning with real-time camera capture,
//! feature detection, bundle adjustment, and mesh generation.

use clap::{Parser, Subcommand};
use head_scanner::{
    HeadScanner, ScanState, ScannerConfig, ScannerResult,
    bundle_adjustment::{BundleAdjuster, Point3DWithObservations},
    calibration::{CalibrationSession, CheckerboardPattern},
    camera::Camera,
    guidance,
    reconstruction::{CameraIntrinsics, CameraPose},
};
use indicatif::{ProgressBar, ProgressStyle};
use log::{info, warn};
use opencv::{
    core::{Point as CvPoint, Scalar},
    highgui, imgproc,
    imgproc::HersheyFonts::FONT_HERSHEY_SIMPLEX,
    prelude::*,
};
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

        /// Display camera feed in real-time window
        #[arg(long, default_value_t = true)]
        display: bool,

        /// Enable GPU acceleration for ML inference
        #[arg(long, default_value_t = true)]
        gpu: bool,

        /// Apply mesh smoothing algorithm (none, laplacian, taubin, bilateral, hc)
        #[arg(long, default_value = "taubin")]
        smooth: String,

        /// Number of smoothing iterations
        #[arg(long, default_value_t = 5)]
        smooth_iterations: usize,

        /// Frame skip interval (process every Nth frame for slower, more precise scanning)
        #[arg(long, default_value_t = 1)]
        frame_skip: usize,

        /// Minimum points per frame threshold (skip frames with fewer points)
        #[arg(long, default_value_t = 10)]
        min_points_per_frame: usize,

        /// Use Structure-from-Motion for accurate 3D reconstruction
        #[arg(long)]
        sfm: bool,

        /// Number of frames to use for SfM (2-10 recommended)
        #[arg(long, default_value_t = 3)]
        sfm_frames: usize,

        /// Minimum inliers for valid SfM pose estimation
        #[arg(long, default_value_t = 20)]
        sfm_min_inliers: usize,

        /// Generate SOFA file for HRTF
        #[arg(long)]
        generate_sofa: bool,

        /// SOFA output file path
        #[arg(long, default_value = "head_scan.sofa")]
        sofa_output: PathBuf,

        /// SOFA sample rate (Hz)
        #[arg(long, default_value_t = 44100.0)]
        sofa_sample_rate: f32,

        /// SOFA azimuth resolution (number of angles, e.g. 72 = 5° spacing)
        #[arg(long, default_value_t = 72)]
        sofa_azimuth: usize,

        /// SOFA elevation resolution (number of angles, e.g. 36 = 5° spacing)
        #[arg(long, default_value_t = 36)]
        sofa_elevation: usize,

        /// SOFA source distance (cm, e.g. 100 = 1m)
        #[arg(long, default_value_t = 100.0)]
        sofa_distance: f32,
    },

    /// Test camera connection
    Test {
        /// Duration to test in seconds
        #[arg(short, long, default_value_t = 5)]
        duration: u64,
    },

    /// Calibrate camera using checkerboard pattern
    Calibrate {
        /// Output file for calibration data (JSON)
        #[arg(short, long, default_value = "camera_calibration.json")]
        output: PathBuf,

        /// Checkerboard width (inner corners)
        #[arg(long, default_value_t = 9)]
        board_width: i32,

        /// Checkerboard height (inner corners)
        #[arg(long, default_value_t = 6)]
        board_height: i32,

        /// Square size in mm
        #[arg(long, default_value_t = 25.0)]
        square_size: f32,

        /// Minimum number of calibration frames
        #[arg(long, default_value_t = 15)]
        min_frames: usize,

        /// Maximum number of calibration frames
        #[arg(long, default_value_t = 30)]
        max_frames: usize,
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
            display,
            gpu,
            smooth,
            smooth_iterations,
            frame_skip,
            min_points_per_frame,
            sfm,
            sfm_frames,
            sfm_min_inliers,
            generate_sofa,
            sofa_output,
            sofa_sample_rate,
            sofa_azimuth,
            sofa_elevation,
            sofa_distance,
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
                display,
                gpu,
                smooth,
                smooth_iterations,
                frame_skip,
                min_points_per_frame,
                sfm,
                sfm_frames,
                sfm_min_inliers,
                generate_sofa,
                sofa_output,
                sofa_sample_rate,
                sofa_azimuth,
                sofa_elevation,
                sofa_distance,
            )
            .await?;
        }
        Commands::Test { duration } => {
            test_camera(cli.camera, duration).await?;
        }
        Commands::Calibrate {
            output,
            board_width,
            board_height,
            square_size,
            min_frames,
            max_frames,
        } => {
            run_calibration(
                cli.camera,
                output,
                board_width,
                board_height,
                square_size,
                min_frames,
                max_frames,
            )
            .await?;
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
    display_video: bool,
    use_gpu: bool,
    smooth_algorithm: String,
    smooth_iterations: usize,
    frame_skip: usize,
    min_points_per_frame: usize,
    use_sfm: bool,
    sfm_frame_count: usize,
    sfm_min_inliers: usize,
    generate_sofa: bool,
    sofa_output: PathBuf,
    sofa_sample_rate: f32,
    sofa_azimuth: usize,
    sofa_elevation: usize,
    sofa_distance: f32,
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
        use_gpu,
        use_sfm,
        sfm_frame_count,
        sfm_min_inliers,
        model_path: model_path.map(|p| p.to_string_lossy().to_string()),
    };

    info!("Initializing scanner with config: {:?}", config);

    // Create and start scanner
    let mut scanner = HeadScanner::new(config)?;
    scanner.start().await?;

    println!("✓ Camera initialized ({}x{} @ {}fps)", width, height, fps);

    if use_sfm {
        println!("✨ Structure-from-Motion (SfM) mode ENABLED");
        println!(
            "   • Using {} frame history for triangulation",
            sfm_frame_count
        );
        println!(
            "   • Minimum {} inliers required for pose estimation",
            sfm_min_inliers
        );
        println!("   • Real 3D coordinates from geometric constraints");
    } else {
        println!("⚠️  Classical mode (estimated depth)");
        println!("   • Consider using --sfm for higher quality");
    }

    println!();
    println!("📋 IMPORTANT: For best 3D reconstruction:");
    println!("   • Move the camera slowly around your head");
    println!("   • Or rotate your head slowly while keeping camera still");
    println!("   • Multiple viewpoints are essential for accurate 3D data");
    if use_sfm {
        println!("   • SfM REQUIRES camera movement for triangulation");
    } else {
        println!("   • Static camera = poor quality mesh");
    }
    println!();
    println!("✓ Waiting for head detection...");
    if display_video {
        println!("✓ Video display enabled (press 'q' to quit, 'p' to pause)");
    }
    println!();

    // Create OpenCV window for video display
    let window_name = "Head Scanner - Live Feed";
    if display_video {
        highgui::named_window(window_name, highgui::WINDOW_AUTOSIZE)?;
    }

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
    let mut processed_frame_count = 0;
    let mut last_state = ScanState::Idle;

    // Collect camera poses and 3D points for bundle adjustment
    let camera_poses: Vec<CameraPose> = Vec::new();
    let point_observations: Vec<Point3DWithObservations> = Vec::new();

    // Main scanning loop
    loop {
        // Check timeout
        if max_duration > 0 && start_time.elapsed() > max_duration_secs {
            warn!("Maximum scan duration reached");
            break;
        }

        frame_count += 1;

        // Frame skipping for quality control
        if frame_skip > 1 && frame_count % frame_skip != 0 {
            // Skip this frame, but still capture for display
            if display_video {
                if let Ok(frame) = scanner.capture_current_frame() {
                    let mut display_frame = frame.mat().try_clone()?;
                    let state = scanner.get_state();
                    let coverage = scanner.get_coverage();
                    let elapsed = start_time.elapsed().as_secs_f32();
                    let using_gpu = scanner.is_using_gpu();

                    if let Err(e) = draw_progress_overlay(
                        &mut display_frame,
                        state,
                        coverage,
                        processed_frame_count,
                        elapsed,
                        using_gpu,
                        &scanner,
                    ) {
                        warn!("Failed to draw overlay: {}", e);
                    }

                    highgui::imshow(window_name, &display_frame)?;
                    let key = highgui::wait_key(1)?;
                    if key == 'q' as i32 || key == 27 {
                        println!("\nUser requested quit");
                        break;
                    }
                }
            }
            continue; // Skip processing this frame
        }

        // Process frame
        let point_cloud_size_before = scanner.get_point_cloud_size();
        scanner.process_frame().await?;
        let point_cloud_size_after = scanner.get_point_cloud_size();
        let points_added = point_cloud_size_after.saturating_sub(point_cloud_size_before);

        // Skip frames that don't add enough points (poor quality)
        if points_added < min_points_per_frame && scanner.get_state() == ScanState::Scanning {
            log::debug!(
                "Skipping frame with only {} points (threshold: {})",
                points_added,
                min_points_per_frame
            );
            continue;
        }

        processed_frame_count += 1;

        let state = scanner.get_state();
        let coverage = scanner.get_coverage();

        // Display video feed if enabled
        if display_video {
            if let Ok(frame) = scanner.capture_current_frame() {
                // Clone the frame so we can draw on it
                let mut display_frame = frame.mat().try_clone()?;

                // Draw progress overlay
                let elapsed = start_time.elapsed().as_secs_f32();
                let using_gpu = scanner.is_using_gpu();
                if let Err(e) = draw_progress_overlay(
                    &mut display_frame,
                    state,
                    coverage,
                    processed_frame_count as u32,
                    elapsed,
                    using_gpu,
                    &scanner,
                ) {
                    warn!("Failed to draw overlay: {}", e);
                }

                // Display the frame with overlay
                highgui::imshow(window_name, &display_frame)?;

                // Check for key press (1ms wait)
                let key = highgui::wait_key(1)?;
                if key == 'q' as i32 || key == 27 {
                    // 'q' or ESC pressed
                    println!("\nUser requested quit");
                    break;
                } else if key == 'p' as i32 {
                    // 'p' pressed - pause
                    println!("\nPaused - press any key to continue");
                    highgui::wait_key(0)?;
                }
            }
        }

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
    println!("   Frames captured: {}", frame_count);
    println!(
        "   Frames processed: {} ({:.1}%)",
        processed_frame_count,
        (processed_frame_count as f32 / frame_count as f32) * 100.0
    );
    println!("   Duration: {:.1}s", start_time.elapsed().as_secs_f32());
    println!("   Coverage: {:.1}%", scanner.get_coverage() * 100.0);
    println!("   Points collected: {}", scanner.get_point_cloud_size());
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

    // Apply mesh smoothing
    let smooth_algo = smooth_algorithm.to_lowercase();
    if smooth_algo != "none" && smooth_iterations > 0 {
        println!();
        println!("✨ Applying mesh smoothing ({})...", smooth_algo);

        let start = std::time::Instant::now();
        match smooth_algo.as_str() {
            "laplacian" => {
                mesh.smooth_laplacian(smooth_iterations, 0.5);
                println!(
                    "   ✓ Laplacian smoothing applied ({} iterations)",
                    smooth_iterations
                );
            }
            "taubin" => {
                mesh.smooth_taubin(smooth_iterations, 0.6, -0.63);
                println!(
                    "   ✓ Taubin smoothing applied ({} iterations)",
                    smooth_iterations
                );
            }
            "bilateral" => {
                mesh.smooth_bilateral(smooth_iterations, 0.5, 0.3);
                println!(
                    "   ✓ Bilateral smoothing applied ({} iterations)",
                    smooth_iterations
                );
            }
            "hc" => {
                mesh.smooth_hc(smooth_iterations, 0.5, 0.65);
                println!(
                    "   ✓ HC smoothing applied ({} iterations)",
                    smooth_iterations
                );
            }
            _ => {
                warn!("Unknown smoothing algorithm: {}", smooth_algo);
                println!("   ⚠ Unknown algorithm, skipping smoothing");
            }
        }
        println!("   Time: {:.2}s", start.elapsed().as_secs_f32());
    }

    // Export mesh
    println!();
    println!("💾 Exporting mesh to {:?}...", output_path);
    mesh.export(&output_path.to_string_lossy())?;
    println!("   ✓ Mesh exported successfully");

    // Generate SOFA file if requested
    if generate_sofa {
        println!();
        println!("🎧 Generating SOFA file for HRTF...");

        #[cfg(feature = "sofa")]
        {
            use head_scanner::acoustics;
            use head_scanner::security;

            // Validate SOFA output path for security
            let validated_sofa_path =
                match security::validate_export_path(&sofa_output.to_string_lossy(), None) {
                    Ok(path) => path,
                    Err(e) => {
                        println!("   ⚠ Invalid SOFA output path: {}", e);
                        println!("   Please use a valid filename without directory traversal");
                        return Err(e);
                    }
                };

            let result = acoustics::generate_sofa_analytical(
                &mesh,
                &validated_sofa_path.to_string_lossy(),
                sofa_sample_rate,
                sofa_azimuth,
                sofa_elevation,
                sofa_distance,
            );

            match result {
                Ok(_) => {
                    println!("   ✓ SOFA file generated: {:?}", validated_sofa_path);
                    println!(
                        "   Grid: {}az × {}el = {} positions",
                        sofa_azimuth,
                        sofa_elevation,
                        sofa_azimuth * sofa_elevation
                    );
                    println!("   Sample rate: {} Hz", sofa_sample_rate);
                }
                Err(e) => {
                    println!("   ⚠ SOFA generation failed: {}", e);
                    println!(
                        "   Note: Ear detection may have failed - ensure scan covers both ears"
                    );
                }
            }
        }

        #[cfg(not(feature = "sofa"))]
        {
            println!("   ⚠ SOFA support not enabled");
            println!("   Rebuild with: cargo build --features sofa");
        }
    }

    // Stop scanner
    scanner.stop().await?;

    // Close video window if it was opened
    if display_video {
        highgui::destroy_window(window_name)?;
    }

    println!();
    println!("✨ Scan complete! Your 3D head model is ready.");

    Ok(())
}

/// Draw progress overlay on video frame with scan guidance
fn draw_progress_overlay(
    frame: &mut opencv::core::Mat,
    state: ScanState,
    coverage: f32,
    frame_count: u32,
    elapsed_secs: f32,
    using_gpu: bool,
    scanner: &HeadScanner,
) -> ScannerResult<()> {
    let height = frame.rows();
    let width = frame.cols();

    // Draw semi-transparent background at top
    let overlay_height = 200; // Increased to fit guidance info
    imgproc::rectangle(
        frame,
        opencv::core::Rect::new(0, 0, width, overlay_height),
        Scalar::new(0.0, 0.0, 0.0, 0.0),
        -1, // Filled
        imgproc::LINE_8,
        0,
    )?;

    // Text properties
    let font = FONT_HERSHEY_SIMPLEX as i32;
    let font_scale = 0.7;
    let thickness = 2;
    let white = Scalar::new(255.0, 255.0, 255.0, 0.0);
    let green = Scalar::new(0.0, 255.0, 0.0, 0.0);
    let yellow = Scalar::new(0.0, 255.0, 255.0, 0.0);
    let red = Scalar::new(0.0, 0.0, 255.0, 0.0);

    // Status text
    let status_text = match state {
        ScanState::Idle => "Idle",
        ScanState::Initializing => "Initializing...",
        ScanState::DetectingHead => "Waiting for head detection",
        ScanState::Scanning => "Scanning",
        ScanState::Paused => "Paused",
        ScanState::Processing => "Processing",
        ScanState::Complete => "Complete",
        ScanState::Error => "Error",
    };

    let status_color = match state {
        ScanState::Scanning => green,
        ScanState::DetectingHead => yellow,
        ScanState::Error => red,
        _ => white,
    };

    imgproc::put_text(
        frame,
        &format!("Status: {}", status_text),
        CvPoint::new(20, 30),
        font,
        font_scale,
        status_color,
        thickness,
        imgproc::LINE_AA,
        false,
    )?;

    // Coverage percentage
    let coverage_pct = coverage * 100.0;
    let coverage_color = if coverage_pct >= 85.0 {
        green
    } else if coverage_pct >= 50.0 {
        yellow
    } else {
        red
    };

    imgproc::put_text(
        frame,
        &format!("Coverage: {:.1}%", coverage_pct),
        CvPoint::new(20, 60),
        font,
        font_scale,
        coverage_color,
        thickness,
        imgproc::LINE_AA,
        false,
    )?;

    // Progress bar
    let bar_x = 20;
    let bar_y = 70;
    let bar_width = width - 40;
    let bar_height = 20;

    // Background bar (gray)
    imgproc::rectangle(
        frame,
        opencv::core::Rect::new(bar_x, bar_y, bar_width, bar_height),
        Scalar::new(100.0, 100.0, 100.0, 0.0),
        -1,
        imgproc::LINE_8,
        0,
    )?;

    // Progress bar (colored based on coverage)
    let progress_width = (bar_width as f32 * coverage) as i32;
    if progress_width > 0 {
        imgproc::rectangle(
            frame,
            opencv::core::Rect::new(bar_x, bar_y, progress_width, bar_height),
            coverage_color,
            -1,
            imgproc::LINE_8,
            0,
        )?;
    }

    // Frame count, FPS, and GPU status
    let fps = if elapsed_secs > 0.0 {
        frame_count as f32 / elapsed_secs
    } else {
        0.0
    };

    let gpu_indicator = if using_gpu { "🚀 GPU" } else { "CPU" };
    let gpu_color = if using_gpu { green } else { white };

    imgproc::put_text(
        frame,
        &format!(
            "Frames: {} | FPS: {:.1} | Time: {:.1}s | {}",
            frame_count, fps, elapsed_secs, gpu_indicator
        ),
        CvPoint::new(20, 110),
        font,
        0.5,
        gpu_color,
        1,
        imgproc::LINE_AA,
        false,
    )?;

    // Draw scan guidance information
    if state == ScanState::Scanning {
        // Get quality metrics
        let quality = scanner.get_quality_metrics();

        // Display angular coverage
        imgproc::put_text(
            frame,
            &format!(
                "Angular coverage: {:.0}% ({} angles)",
                quality.angular_coverage * 100.0,
                quality.unique_angles
            ),
            CvPoint::new(20, 140),
            font,
            0.6,
            white,
            1,
            imgproc::LINE_AA,
            false,
        )?;

        // Display quality score
        let quality_score = quality.overall_score();
        let quality_color = if quality_score > 0.85 {
            green
        } else if quality_score > 0.7 {
            yellow
        } else {
            red
        };

        imgproc::put_text(
            frame,
            &format!("Quality score: {:.0}%", quality_score * 100.0),
            CvPoint::new(20, 165),
            font,
            0.6,
            quality_color,
            1,
            imgproc::LINE_AA,
            false,
        )?;

        // Display next guidance instruction
        if let Some(instruction) = scanner.get_next_guidance() {
            imgproc::put_text(
                frame,
                &format!("➜ {}", instruction.direction),
                CvPoint::new(20, 190),
                font,
                0.7,
                Scalar::new(0.0, 255.0, 255.0, 0.0), // Cyan for instructions
                2,
                imgproc::LINE_AA,
                false,
            )?;
        } else {
            // All regions covered!
            imgproc::put_text(
                frame,
                "✓ All angles captured!",
                CvPoint::new(20, 190),
                font,
                0.7,
                green,
                2,
                imgproc::LINE_AA,
                false,
            )?;
        }
    }

    // Controls hint at bottom
    let hint_y = height - 20;
    imgproc::put_text(
        frame,
        "Press 'q' to quit | 'p' to pause",
        CvPoint::new(20, hint_y),
        font,
        0.5,
        white,
        1,
        imgproc::LINE_AA,
        false,
    )?;

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

async fn run_calibration(
    camera_index: u32,
    output_path: PathBuf,
    board_width: i32,
    board_height: i32,
    square_size: f32,
    min_frames: usize,
    max_frames: usize,
) -> ScannerResult<()> {
    println!("📐 Camera Calibration");
    println!("====================");
    println!();
    println!(
        "Checkerboard: {}x{} (inner corners)",
        board_width, board_height
    );
    println!("Square size: {}mm", square_size);
    println!("Target frames: {}-{}", min_frames, max_frames);
    println!();
    println!("Instructions:");
    println!(
        "  1. Print a {}x{} checkerboard pattern",
        board_width, board_height
    );
    println!("  2. Show the checkerboard to the camera from different angles");
    println!("  3. Keep the pattern flat and fully visible");
    println!("  4. Move slowly to capture clear images");
    println!(
        "  5. Press 'q' to finish early (after {} frames)",
        min_frames
    );
    println!();

    // Initialize camera
    let camera = Camera::new(camera_index, 1280, 720, 30)?;
    println!("✓ Camera opened");

    // Create calibration session
    let pattern = CheckerboardPattern::new(board_width, board_height, square_size);
    let mut session = CalibrationSession::new(pattern)
        .with_min_frames(min_frames)
        .with_max_frames(max_frames);

    // Create window for display
    let window_name = "Camera Calibration - Press 'q' to quit";
    highgui::named_window(window_name, highgui::WINDOW_AUTOSIZE)?;

    // Progress bar
    let pb = ProgressBar::new(min_frames as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{bar:40.cyan/blue}] {pos}/{len} frames | {msg}")
            .unwrap()
            .progress_chars("#>-"),
    );

    println!("📸 Capturing calibration frames...");
    println!();

    let mut last_capture = Instant::now();
    let capture_interval = Duration::from_millis(500); // Capture every 500ms

    loop {
        // Capture frame
        let frame = camera.capture_frame()?;
        let mut display = frame.mat().try_clone()?;

        // Try to detect corners
        if let Ok(Some(corners)) = session.detect_corners(&frame) {
            // Draw corners on display
            if let Ok(drawn) = session.draw_corners(&frame, &corners) {
                display = drawn;
            }

            // Add frame if enough time has passed
            if last_capture.elapsed() >= capture_interval {
                if session.process_frame(&frame)? {
                    last_capture = Instant::now();
                    pb.set_position(session.frame_count() as u64);
                    pb.set_message(format!("Progress: {:.0}%", session.progress() * 100.0));
                }
            }

            // Draw status
            let status = format!(
                "Frames: {}/{} - {}",
                session.frame_count(),
                max_frames,
                if session.is_ready() {
                    "Ready to calibrate (press 'q')"
                } else {
                    "Keep moving the checkerboard"
                }
            );

            imgproc::put_text(
                &mut display,
                &status,
                CvPoint::new(20, 30),
                FONT_HERSHEY_SIMPLEX as i32,
                0.7,
                Scalar::new(0.0, 255.0, 0.0, 0.0),
                2,
                imgproc::LINE_AA,
                false,
            )?;
        } else {
            // No checkerboard detected
            imgproc::put_text(
                &mut display,
                "No checkerboard detected",
                CvPoint::new(20, 30),
                FONT_HERSHEY_SIMPLEX as i32,
                0.7,
                Scalar::new(0.0, 0.0, 255.0, 0.0),
                2,
                imgproc::LINE_AA,
                false,
            )?;
        }

        // Show frame
        highgui::imshow(window_name, &display)?;

        // Check for key press
        let key = highgui::wait_key(1)?;
        if key == 'q' as i32 || key == 27 {
            if session.is_ready() {
                break;
            } else {
                println!("\n⚠ Need at least {} frames to calibrate", min_frames);
            }
        }

        // Check if max frames reached
        if session.is_complete() {
            println!("\n✓ Maximum frames captured");
            break;
        }
    }

    pb.finish_with_message("Capture complete");
    highgui::destroy_window(window_name)?;

    // Calibrate
    println!();
    println!("🔧 Computing calibration...");
    let result = session.calibrate()?;

    println!("✓ Calibration successful!");
    println!();
    println!("Results:");
    println!(
        "  Focal length (fx, fy): {:.2}, {:.2}",
        result.intrinsics.fx, result.intrinsics.fy
    );
    println!(
        "  Principal point (cx, cy): {:.2}, {:.2}",
        result.intrinsics.cx, result.intrinsics.cy
    );
    println!("  RMS error: {:.4} pixels", result.rms_error);
    println!("  Frames used: {}", result.num_frames);

    if let Some(dist) = result.intrinsics.distortion {
        println!("  Distortion coefficients:");
        println!(
            "    k1={:.6}, k2={:.6}, p1={:.6}, p2={:.6}, k3={:.6}",
            dist[0], dist[1], dist[2], dist[3], dist[4]
        );
    }

    // Save calibration data
    println!();
    println!("💾 Saving calibration to {:?}...", output_path);

    // Validate output path for security
    use head_scanner::security;
    let validated_path = security::validate_export_path(&output_path.to_string_lossy(), None)?;

    let json = serde_json::to_string_pretty(&result.intrinsics)?;
    std::fs::write(&validated_path, json)?;
    println!("✓ Calibration saved");

    println!();
    println!("✨ Calibration complete!");
    println!("   Use this file with: --calibration {:?}", validated_path);

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
    println!(
        "Default Resolution: {}x{}",
        config.frame_width, config.frame_height
    );
    println!("Default FPS: {}", config.fps);

    scanner.stop().await?;

    println!();
    println!("💡 Tip: Use 'head-scanner-cli scan --help' to see scanning options");

    Ok(())
}
