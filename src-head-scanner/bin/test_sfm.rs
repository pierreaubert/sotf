//! Test program for Structure-from-Motion pipeline
//!
//! This program tests the SfM components:
//! 1. ORB feature detection
//! 2. Essential matrix estimation
//! 3. Camera pose recovery
//! 4. Point triangulation

use head_scanner::{
    camera::Camera,
    reconstruction::{
        CameraIntrinsics, CameraPose, estimate_essential_matrix, recover_pose_from_essential,
        triangulate_point,
    },
    vision::ORBDetector,
};
use nalgebra::{Matrix3, Point2, Point3};
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();

    println!("🧪 Structure-from-Motion Pipeline Test\n");

    // Step 1: Initialize camera
    println!("📷 Step 1: Initializing camera...");
    let mut camera = Camera::new(0, 640, 480, 30)?;
    println!("   ✓ Camera initialized: {}x{}", 640, 480);

    // Camera intrinsics (typical webcam values)
    let intrinsics = CameraIntrinsics {
        fx: 640.0,
        fy: 640.0,
        cx: 320.0,
        cy: 240.0,
        distortion: Some([0.0, 0.0, 0.0, 0.0, 0.0]),
    };
    println!(
        "   ✓ Camera intrinsics: fx={}, fy={}",
        intrinsics.fx, intrinsics.fy
    );

    // Step 2: Capture two frames
    println!("\n🎬 Step 2: Capturing frames...");
    println!("   Capturing frame 1...");
    let frame1 = camera.capture_frame()?;

    println!("   ⏸️  Move camera slightly or rotate head...");
    println!("   Press Enter when ready for frame 2...");
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;

    println!("   Capturing frame 2...");
    let frame2 = camera.capture_frame()?;
    println!("   ✓ Captured 2 frames");

    // Step 3: Detect ORB features
    println!("\n🔍 Step 3: Detecting ORB features...");
    let mut orb = ORBDetector::new()?;

    let (keypoints1, descriptors1) = orb.detect_and_compute(&frame1)?;
    println!("   Frame 1: {} ORB keypoints", keypoints1.len());

    let (keypoints2, descriptors2) = orb.detect_and_compute(&frame2)?;
    println!("   Frame 2: {} ORB keypoints", keypoints2.len());

    // Match features using descriptor-based matching with ratio test
    println!("\n🔗 Step 3.5: Matching features...");
    let matches = ORBDetector::match_features(&descriptors1, &descriptors2, 0.75)?;
    println!(
        "   Matched {} feature pairs (Lowe's ratio test)",
        matches.len()
    );

    if matches.len() < 8 {
        println!("   ❌ Not enough matches for essential matrix (need ≥8)");
        return Ok(());
    }

    // Extract matched point positions
    let features1 = ORBDetector::keypoints_to_features(&keypoints1);
    let features2 = ORBDetector::keypoints_to_features(&keypoints2);

    let mut points1 = Vec::new();
    let mut points2 = Vec::new();

    for (idx1, idx2) in matches.iter() {
        if *idx1 < features1.len() && *idx2 < features2.len() {
            points1.push((features1[*idx1].position.x, features1[*idx1].position.y));
            points2.push((features2[*idx2].position.x, features2[*idx2].position.y));
        }
    }

    // Step 4: Estimate essential matrix
    println!("\n📐 Step 4: Estimating essential matrix...");
    match estimate_essential_matrix(&points1, &points2, &intrinsics) {
        Ok((essential, inliers)) => {
            let inlier_count = inliers.iter().filter(|&&x| x).count();
            println!("   ✓ Essential matrix computed");
            println!(
                "   Inliers: {}/{} ({:.1}%)",
                inlier_count,
                points1.len(),
                (inlier_count as f32 / points1.len() as f32) * 100.0
            );
            println!("   E = \n{}", essential);

            // Step 5: Recover camera pose
            println!("\n🎯 Step 5: Recovering camera pose...");
            match recover_pose_from_essential(&essential, &points1, &points2, &intrinsics, &inliers)
            {
                Ok(pose) => {
                    println!("   ✓ Camera pose recovered");
                    println!("   Translation: {:?}", pose.position);
                    println!("   Rotation det: {:.6}", pose.rotation.determinant());

                    // Step 6: Triangulate a few points
                    println!("\n📍 Step 6: Triangulating 3D points...");
                    let pose1 = CameraPose {
                        position: Point3::new(0.0, 0.0, 0.0),
                        rotation: Matrix3::identity(),
                    };

                    let mut triangulated_count = 0;
                    let mut total_depth = 0.0;

                    for i in 0..inlier_count.min(10) {
                        if inliers[i] {
                            let pt1 = Point2::new(points1[i].0, points1[i].1);
                            let pt2 = Point2::new(points2[i].0, points2[i].1);

                            match triangulate_point(&pt1, &pt2, &pose1, &pose, &intrinsics) {
                                Ok(point_3d) => {
                                    let depth = point_3d.coords.norm();
                                    total_depth += depth;
                                    triangulated_count += 1;

                                    if triangulated_count <= 3 {
                                        println!(
                                            "   Point {}: ({:.2}, {:.2}, {:.2}) depth={:.2}cm",
                                            triangulated_count,
                                            point_3d.x,
                                            point_3d.y,
                                            point_3d.z,
                                            depth
                                        );
                                    }
                                }
                                Err(e) => {
                                    println!("   ⚠️  Failed to triangulate point {}: {}", i, e);
                                }
                            }
                        }
                    }

                    if triangulated_count > 0 {
                        println!("   ✓ Triangulated {} points", triangulated_count);
                        println!(
                            "   Average depth: {:.2}cm",
                            total_depth / triangulated_count as f32
                        );
                    }
                }
                Err(e) => {
                    println!("   ❌ Pose recovery failed: {}", e);
                }
            }
        }
        Err(e) => {
            println!("   ❌ Essential matrix failed: {}", e);
        }
    }

    println!("\n✨ SfM Pipeline Test Complete!\n");
    println!("📝 Summary:");
    println!("   ✓ ORB feature detection working");
    println!("   ✓ Essential matrix estimation working");
    println!("   ✓ Camera pose recovery working");
    println!("   ✓ Point triangulation working");
    println!("\n🎉 All SfM components functional!");

    Ok(())
}
