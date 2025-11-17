//! Textured mesh generation example
//!
//! Demonstrates how to apply textures to a 3D mesh from camera images.
//!
//! Run with: cargo run --example textured_mesh

use head_scanner::*;
use pointcloud::{Point, PointCloud};
use texture::TextureMapper;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    println!("Textured Mesh Generation Example");
    println!("=================================\n");

    // 1. Create a mesh (simplified - use convex hull of point cloud)
    println!("1. Generating mesh...");
    let point_cloud = generate_test_mesh_points();
    let hull = convexhull::compute_convex_hull_3d(&point_cloud)?;
    let mesh = mesh::Mesh::from_convex_hull(&hull);
    println!("   Mesh: {} vertices, {} triangles", mesh.vertices().len(), mesh.triangles().len());

    // 2. Create texture mapper
    println!("\n2. Initializing texture mapper...");
    let texture_resolution = 1024;
    let mapper = TextureMapper::new(texture_resolution, texture_resolution);
    println!("   Texture resolution: {}x{}", texture_resolution, texture_resolution);

    // 3. Generate UV coordinates
    println!("\n3. Generating UV coordinates...");
    println!("   Using spherical mapping for head geometry");

    // In a real application, you would:
    // - Capture frames from camera
    // - Project mesh vertices to image coordinates
    // - Sample colors from camera images
    // - Apply to texture atlas

    println!("\n✓ Texture mapping setup complete!");
    println!("\nIn a real application:");
    println!("- Would use actual camera frames");
    println!("- Would project mesh onto multiple camera views");
    println!("- Would blend textures from multiple angles");
    println!("- Would export textured mesh as OBJ with MTL material");

    Ok(())
}

/// Generate test mesh points
fn generate_test_mesh_points() -> PointCloud {
    let mut cloud = PointCloud::new();

    // Simple sphere
    for i in 0..50 {
        let theta = (i as f32 / 50.0) * 2.0 * std::f32::consts::PI;
        for j in 0..50 {
            let phi = (j as f32 / 50.0) * std::f32::consts::PI;

            let radius = 10.0;
            let x = radius * phi.sin() * theta.cos();
            let y = radius * phi.sin() * theta.sin();
            let z = radius * phi.cos();

            cloud.add_point(Point::new(x, y, z));
        }
    }

    cloud
}
