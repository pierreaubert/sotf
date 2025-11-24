//! Performance benchmarks for head scanner
//!
//! Run with: cargo bench --package head-scanner

use head_scanner::*;
use pointcloud::{Point, PointCloud};
use reconstruction::{CameraIntrinsics, SfMReconstructor};
use std::time::Instant;
use vision::Feature;

/// Benchmark convex hull computation
fn bench_convex_hull() {
    let sizes = [100, 500, 1000, 5000, 10000];

    println!("\n=== Convex Hull Performance ===");
    println!("{:<12} {:<15} {:<15}", "Points", "Time (ms)", "Points/sec");
    println!("{}", "-".repeat(45));

    for &size in &sizes {
        let mut point_cloud = PointCloud::new();

        // Generate random points on a sphere
        for i in 0..size {
            let theta = (i as f32 / size as f32) * 2.0 * std::f32::consts::PI;
            let phi = ((i % 100) as f32 / 100.0) * std::f32::consts::PI;

            let radius = 10.0;
            let x = radius * phi.sin() * theta.cos();
            let y = radius * phi.sin() * theta.sin();
            let z = radius * phi.cos();

            point_cloud.add_point(Point::new(x, y, z));
        }

        let start = Instant::now();
        let hull = convexhull::compute_convex_hull_3d(&point_cloud);
        let elapsed = start.elapsed();

        if let Ok(hull) = hull {
            let ms = elapsed.as_secs_f64() * 1000.0;
            let points_per_sec = size as f64 / elapsed.as_secs_f64();
            println!("{:<12} {:<15.2} {:<15.0}", size, ms, points_per_sec);
        }
    }
}

/// Benchmark Structure-from-Motion reconstruction
fn bench_sfm_reconstruction() {
    let frame_counts = [10, 50, 100];

    println!("\n=== SfM Reconstruction Performance ===");
    println!("{:<12} {:<15} {:<15}", "Frames", "Time (ms)", "Frames/sec");
    println!("{}", "-".repeat(45));

    for &num_frames in &frame_counts {
        let intrinsics = CameraIntrinsics::default_webcam(1280, 720);
        let mut sfm = SfMReconstructor::new(intrinsics);

        let start = Instant::now();

        for frame_idx in 0..num_frames {
            let features = vec![
                Feature::new(640.0 + frame_idx as f32, 360.0, "nose".to_string(), 0.9),
                Feature::new(600.0 + frame_idx as f32, 340.0, "left_eye".to_string(), 0.8),
                Feature::new(
                    680.0 + frame_idx as f32,
                    340.0,
                    "right_eye".to_string(),
                    0.8,
                ),
                Feature::new(640.0 + frame_idx as f32, 400.0, "mouth".to_string(), 0.85),
            ];

            let _ = sfm.add_frame(features);
        }

        let elapsed = start.elapsed();
        let ms = elapsed.as_secs_f64() * 1000.0;
        let fps = num_frames as f64 / elapsed.as_secs_f64();

        println!("{:<12} {:<15.2} {:<15.1}", num_frames, ms, fps);
    }
}

/// Benchmark bundle adjustment
fn bench_bundle_adjustment() {
    use bundle_adjustment::{BundleAdjuster, Point3DWithObservations};
    use nalgebra::{Point2, Point3};
    use reconstruction::CameraPose;

    let point_counts = [10, 50, 100, 500];

    println!("\n=== Bundle Adjustment Performance ===");
    println!(
        "{:<12} {:<12} {:<15} {:<15}",
        "Points", "Cameras", "Time (ms)", "Points/sec"
    );
    println!("{}", "-".repeat(60));

    for &num_points in &point_counts {
        let intrinsics = CameraIntrinsics::default_webcam(1280, 720);
        let adjuster = BundleAdjuster::new(intrinsics).with_max_iterations(10);

        // Create test data
        let num_cameras = 5;
        let mut poses = Vec::new();
        for i in 0..num_cameras {
            let mut pose = CameraPose::identity();
            pose.position.z = i as f32 * 10.0;
            poses.push(pose);
        }

        let mut points = Vec::new();
        for i in 0..num_points {
            let point = Point3DWithObservations {
                position: Point3::new(
                    (i as f32 % 10.0) - 5.0,
                    ((i / 10) as f32 % 10.0) - 5.0,
                    50.0,
                ),
                observations: vec![
                    (0, Point2::new(640.0, 360.0)),
                    (1, Point2::new(640.0, 360.0)),
                ],
            };
            points.push(point);
        }

        let start = Instant::now();
        let _ = adjuster.optimize(&poses, &points);
        let elapsed = start.elapsed();

        let ms = elapsed.as_secs_f64() * 1000.0;
        let points_per_sec = num_points as f64 / elapsed.as_secs_f64();

        println!(
            "{:<12} {:<12} {:<15.2} {:<15.0}",
            num_points, num_cameras, ms, points_per_sec
        );
    }
}

/// Benchmark point cloud operations
fn bench_point_cloud_ops() {
    let sizes = [1000, 5000, 10000, 50000];

    println!("\n=== Point Cloud Operations Performance ===");
    println!(
        "{:<12} {:<20} {:<20}",
        "Points", "Add Time (ms)", "Downsample (ms)"
    );
    println!("{}", "-".repeat(55));

    for &size in &sizes {
        // Benchmark adding points
        let mut cloud = PointCloud::new();
        let start = Instant::now();

        for i in 0..size {
            let x = (i as f32 % 100.0) / 10.0;
            let y = ((i / 100) as f32 % 100.0) / 10.0;
            let z = ((i / 10000) as f32 % 100.0) / 10.0;
            cloud.add_point(Point::new(x, y, z));
        }

        let add_time = start.elapsed().as_secs_f64() * 1000.0;

        // Benchmark downsampling
        let start = Instant::now();
        cloud.voxel_downsample(0.5);
        let downsample_time = start.elapsed().as_secs_f64() * 1000.0;

        println!("{:<12} {:<20.2} {:<20.2}", size, add_time, downsample_time);
    }
}

/// Benchmark texture mapping
fn bench_texture_mapping() {
    use texture::TextureMapper;

    let texture_sizes = [256, 512, 1024];

    println!("\n=== Texture Mapping Performance ===");
    println!("{:<12} {:<20}", "Resolution", "Init Time (ms)");
    println!("{}", "-".repeat(35));

    for &size in &texture_sizes {
        let start = Instant::now();
        let _mapper = TextureMapper::new(size, size);
        let elapsed = start.elapsed();

        let ms = elapsed.as_secs_f64() * 1000.0;
        println!("{:<12} {:<20.2}", format!("{}x{}", size, size), ms);
    }
}

fn main() {
    println!("Head Scanner Performance Benchmarks");
    println!("===================================\n");

    bench_convex_hull();
    bench_sfm_reconstruction();
    bench_bundle_adjustment();
    bench_point_cloud_ops();
    bench_texture_mapping();

    println!("\nBenchmarks complete!");
}
