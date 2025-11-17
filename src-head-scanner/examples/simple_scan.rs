//! Simple head scanning example
//!
//! This example demonstrates basic usage of the head scanner with synthetic data.
//! In a real application, you would capture actual camera frames.
//!
//! Run with: cargo run --example simple_scan

use head_scanner::*;
use pointcloud::{Point, PointCloud};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    println!("Head Scanner - Simple Scan Example");
    println!("==================================\n");

    // 1. Create synthetic point cloud (in real app, this comes from camera)
    println!("1. Generating synthetic head point cloud...");
    let point_cloud = generate_synthetic_head(10.0, 1000);
    println!("   Generated {} points", point_cloud.len());

    // 2. Compute convex hull
    println!("\n2. Computing convex hull...");
    let hull = convexhull::compute_convex_hull_3d(&point_cloud)?;
    println!("   Hull has {} vertices and {} faces", hull.vertex_count(), hull.face_count());
    println!("   Volume: {:.2} cm³", hull.volume());
    println!("   Surface area: {:.2} cm²", hull.surface_area());

    // 3. Convert to mesh
    println!("\n3. Converting to triangulated mesh...");
    let mesh = mesh::Mesh::from_convex_hull(&hull);
    println!("   Mesh has {} vertices and {} triangles", mesh.vertices().len(), mesh.triangles().len());

    // 4. Export mesh
    println!("\n4. Exporting mesh to OBJ file...");
    let output_path = "/tmp/head_scan.obj";
    mesh.export(output_path)?;
    println!("   Saved to: {}", output_path);

    println!("\n✓ Scan complete!");

    Ok(())
}

/// Generate a synthetic head-shaped point cloud
fn generate_synthetic_head(radius_cm: f32, num_points: usize) -> PointCloud {
    let mut cloud = PointCloud::new();

    // Generate points on a sphere (representing a head)
    let points_per_ring = (num_points as f32).sqrt() as usize;

    for i in 0..points_per_ring {
        let theta = (i as f32 / points_per_ring as f32) * 2.0 * std::f32::consts::PI;

        for j in 0..points_per_ring {
            let phi = (j as f32 / points_per_ring as f32) * std::f32::consts::PI;

            // Slightly deform sphere to make it more head-like
            let r = radius_cm * (1.0 + 0.1 * (3.0 * phi).sin());

            let x = r * phi.sin() * theta.cos();
            let y = r * phi.sin() * theta.sin();
            let z = r * phi.cos();

            cloud.add_point(Point::new(x, y, z));
        }
    }

    cloud
}
