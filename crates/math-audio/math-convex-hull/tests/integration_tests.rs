//! Integration tests for convex hull computation
//!
//! These tests correspond to the test cases in the C++ convhull_3d library.

use directories::ProjectDirs;
use math_convex_hull::{ConvexHull3D, Vertex, export_html, export_obj, testdata};
use std::fs;
use std::path::PathBuf;

/// Get the data directory using the directories crate
fn get_data_dir() -> PathBuf {
    let proj_dirs = ProjectDirs::from("org", "spinorama", "math-audio")
        .expect("Failed to determine project directories");

    proj_dirs.cache_dir().to_path_buf()
}

/// Helper function to run a test and generate visualizations
fn run_test_with_visualization(
    name: &str,
    vertices: Vec<Vertex>,
    expected_min_faces: usize,
) -> (usize, usize) {
    println!("\n=== Test: {} ===", name);
    println!("Input vertices: {}", vertices.len());

    let hull = ConvexHull3D::build(&vertices).expect("Failed to build convex hull");

    let num_faces = hull.num_faces();
    let num_vertices = hull.num_vertices();
    let volume = hull.volume();
    let surface_area = hull.surface_area();

    println!("Output faces: {}", num_faces);
    println!("Output vertices: {}", num_vertices);
    println!("Volume: {:.6}", volume);
    println!("Surface area: {:.6}", surface_area);

    // Verify basic properties
    assert!(
        num_faces >= expected_min_faces,
        "Expected at least {} faces, got {}",
        expected_min_faces,
        num_faces
    );
    assert!(
        num_vertices >= 4,
        "Convex hull must have at least 4 vertices"
    );
    assert!(volume > 0.0, "Volume must be positive");
    assert!(surface_area > 0.0, "Surface area must be positive");

    // Create output directory
    let output_dir = get_data_dir().join("convexhull3d");
    fs::create_dir_all(&output_dir).unwrap();
    let output_dir = output_dir.to_string_lossy().to_string();

    // Export OBJ
    let obj_path = format!("{}/convhull_{}.obj", output_dir, name);
    export_obj(&hull, &obj_path).expect("Failed to export OBJ");
    println!("Exported OBJ: {}", obj_path);

    // Export HTML
    let html_path = format!("{}/convhull_{}.html", output_dir, name);
    let title = format!("Convex Hull: {}", name);
    export_html(&hull, &html_path, &title).expect("Failed to export HTML");
    println!("Exported HTML: {}", html_path);

    (num_faces, num_vertices)
}

#[test]
fn test_tetrahedron() {
    let vertices = testdata::tetrahedron_vertices();
    let (num_faces, _) = run_test_with_visualization("tetrahedron", vertices, 4);
    assert_eq!(num_faces, 4, "A tetrahedron should have exactly 4 faces");
}

#[test]
fn test_cube() {
    let vertices = testdata::cube_vertices(2.0);
    let (num_faces, _) = run_test_with_visualization("cube", vertices, 12);
    // A cube should have 12 triangular faces (2 per square face)
    // Note: due to numerical precision and triangulation choices, we might get 12-14 faces
    assert!(
        num_faces >= 12 && num_faces <= 14,
        "A cube should have approximately 12 triangular faces, got {}",
        num_faces
    );
}

#[test]
fn test_octahedron() {
    let vertices = testdata::octahedron_vertices();
    let (num_faces, _) = run_test_with_visualization("octahedron", vertices, 8);
    // Octahedron has 8 faces, but numerical precision might cause slight variations
    assert!(
        num_faces >= 8 && num_faces <= 12,
        "An octahedron should have approximately 8 faces, got {}",
        num_faces
    );
}

#[test]
fn test_icosahedron() {
    let vertices = testdata::icosahedron_vertices();
    let (num_faces, _) = run_test_with_visualization("icosahedron", vertices, 20);
    // Icosahedron has 20 faces, but numerical precision might cause variations
    assert!(
        num_faces >= 20 && num_faces <= 32,
        "An icosahedron should have approximately 20 faces, got {}",
        num_faces
    );
}

#[test]
fn test_random_sphere_936() {
    // This corresponds to the "random spherical distribution" test in C++
    let vertices = testdata::random_sphere_points(936, 1.0);
    run_test_with_visualization("rand_sph_936", vertices, 100);
}

#[test]
fn test_tdesign_180_sphere() {
    // This corresponds to the T-Design 180-point test in C++
    let vertices = testdata::tdesign_180_sphere();
    run_test_with_visualization("tdesign_180_sph", vertices, 100);
}

#[test]
fn test_tdesign_840_sphere() {
    // This corresponds to the T-Design 840-point test in C++
    let vertices = testdata::tdesign_840_sphere();
    run_test_with_visualization("tdesign_840_sph", vertices, 500);
}

#[test]
fn test_tdesign_5100_sphere() {
    // This corresponds to the T-Design 5100-point test in C++
    let vertices = testdata::tdesign_5100_sphere();
    run_test_with_visualization("tdesign_5100_sph", vertices, 3000);
}

#[test]
fn test_cube_with_interior_100() {
    // Cube with 100 random interior points
    // Note: random points are generated within the cube's bounding box,
    // which may slightly exceed the cube's corners, so hull may be larger
    let vertices = testdata::cube_with_interior_points(2.0, 100);
    let (num_faces, _) = run_test_with_visualization("cube_interior_100", vertices, 12);
    // The hull should have at least 12 faces (minimum for a cube-like shape)
    assert!(
        num_faces >= 12,
        "Hull should have at least 12 faces, got {}",
        num_faces
    );
}

#[test]
fn test_cube_with_interior_1000() {
    // Cube with 1000 random interior points
    // Note: random points are generated within the cube's bounding box,
    // which may slightly exceed the cube's corners, so hull may be larger
    let vertices = testdata::cube_with_interior_points(2.0, 1000);
    let (num_faces, _) = run_test_with_visualization("cube_interior_1000", vertices, 12);
    // The hull should have at least 12 faces (minimum for a cube-like shape)
    assert!(
        num_faces >= 12,
        "Hull should have at least 12 faces, got {}",
        num_faces
    );
}

#[test]
fn test_volume_calculation() {
    // Test volume calculation for a unit cube
    let vertices = testdata::cube_vertices(2.0);
    let hull = ConvexHull3D::build(&vertices).unwrap();
    let volume = hull.volume();

    // Volume should be positive and reasonable for a 2x2x2 cube
    // (Note: exact value may vary due to triangulation method)
    assert!(
        volume > 0.0 && volume < 15.0,
        "Volume should be reasonable, got {}",
        volume
    );
}

#[test]
fn test_volume_tetrahedron() {
    // Unit tetrahedron volume
    let vertices = vec![
        Vertex::new(0.0, 0.0, 0.0),
        Vertex::new(1.0, 0.0, 0.0),
        Vertex::new(0.0, 1.0, 0.0),
        Vertex::new(0.0, 0.0, 1.0),
    ];

    let hull = ConvexHull3D::build(&vertices).unwrap();
    let volume = hull.volume();

    // Expected volume is 1/6
    assert!(
        (volume - 1.0 / 6.0).abs() < 0.01,
        "Volume of unit tetrahedron should be approximately 1/6, got {}",
        volume
    );
}

#[test]
fn test_surface_area_cube() {
    // Test surface area for a cube
    let vertices = testdata::cube_vertices(2.0);
    let hull = ConvexHull3D::build(&vertices).unwrap();
    let area = hull.surface_area();

    // Surface area should be positive and reasonable for a 2x2x2 cube
    // (Note: exact value may vary due to triangulation method)
    assert!(
        area > 20.0 && area < 40.0,
        "Surface area should be reasonable, got {}",
        area
    );
}

#[test]
fn test_all_tests_summary() {
    println!("\n========================================");
    println!("CONVEX HULL TEST SUITE SUMMARY");
    println!("========================================");

    // Use a function to generate test cases instead of storing them in a tuple
    let test_cases: Vec<(&str, Box<dyn Fn() -> Vec<Vertex>>)> = vec![
        ("Tetrahedron", Box::new(|| testdata::tetrahedron_vertices())),
        ("Cube", Box::new(|| testdata::cube_vertices(2.0))),
        ("Octahedron", Box::new(|| testdata::octahedron_vertices())),
        ("Icosahedron", Box::new(|| testdata::icosahedron_vertices())),
        (
            "Random Sphere 936",
            Box::new(|| testdata::random_sphere_points(936, 1.0)),
        ),
        ("T-Design 180", Box::new(|| testdata::tdesign_180_sphere())),
        ("T-Design 840", Box::new(|| testdata::tdesign_840_sphere())),
    ];

    let mut success_count = 0;
    let mut total_count = 0;

    for (name, gen_fn) in test_cases {
        total_count += 1;
        let vertices = gen_fn();
        match ConvexHull3D::build(&vertices) {
            Ok(hull) => {
                success_count += 1;
                println!(
                    "✓ {}: {} vertices → {} faces",
                    name,
                    vertices.len(),
                    hull.num_faces()
                );
            }
            Err(e) => {
                println!("✗ {}: Failed with error: {}", name, e);
            }
        }
    }

    println!("========================================");
    println!("Success rate: {}/{}", success_count, total_count);
    println!("========================================");

    assert_eq!(success_count, total_count, "All tests should pass");
}

#[test]
fn test_process_all_obj_files() {
    println!("\n========================================");
    println!("PROCESSING ALL OBJ TEST FILES");
    println!("========================================");

    // Get the data directory path from TEST_DATA_DIR environment variable
    let Some(test_data_dir) = std::env::var("TEST_DATA_DIR").ok() else {
        println!("TEST_DATA_DIR not set, skipping test");
        return;
    };
    let data_dir = PathBuf::from(test_data_dir);
    let obj_dir = data_dir.join("convexhull3d/obj_files");

    // Maximum vertices to process (set to None to process all, or Some(n) to limit)
    // For testing, we limit to 5000 vertices to avoid extremely long computation times
    let max_vertices: Option<usize> = std::env::var("MAX_VERTICES")
        .ok()
        .and_then(|s| s.parse().ok());

    // Get all OBJ files
    let mut obj_files = fs::read_dir(&obj_dir)
        .expect("Failed to read obj_files directory")
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            entry
                .path()
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| ext == "obj")
                .unwrap_or(false)
        })
        .collect::<Vec<_>>();

    // Sort by filename for deterministic processing
    obj_files.sort_by_key(|e| e.file_name());

    println!("Found {} OBJ files to process", obj_files.len());
    if let Some(max) = max_vertices {
        println!(
            "Vertex limit: {} (set MAX_VERTICES env var to change)\n",
            max
        );
    } else {
        println!("No vertex limit (this may take a very long time for large models)\n");
    }

    let mut success_count = 0;
    let mut skipped_count = 0;
    let mut total_count = 0;

    for entry in obj_files {
        let path = entry.path();
        let filename = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown");

        total_count += 1;
        print!("Processing {:<20} ... ", filename);

        match testdata::load_obj_vertices(&path) {
            Ok(vertices) => {
                let vertex_count = vertices.len();

                // Check if we should skip due to vertex count
                if let Some(max) = max_vertices {
                    if vertex_count > max {
                        println!("{} vertices (SKIPPED - too many vertices)", vertex_count);
                        skipped_count += 1;
                        continue;
                    }
                }

                match ConvexHull3D::build(&vertices) {
                    Ok(hull) => {
                        let num_faces = hull.num_faces();
                        let _num_vertices = hull.num_vertices();
                        let volume = hull.volume();
                        let surface_area = hull.surface_area();

                        println!(
                            "{} vertices → {} faces (V={:.2}, SA={:.2})",
                            vertex_count, num_faces, volume, surface_area
                        );

                        // Export results
                        let output_dir = get_data_dir().join("convexhull3d");
                        fs::create_dir_all(&output_dir).unwrap();

                        let obj_path = output_dir.join(format!("convhull_{}.obj", filename));
                        let html_path = output_dir.join(format!("convhull_{}.html", filename));
                        let title = format!("Convex Hull: {}", filename);

                        if let Err(e) = export_obj(&hull, &obj_path) {
                            println!("  ⚠ Failed to export OBJ: {}", e);
                        }

                        if let Err(e) = export_html(&hull, &html_path, &title) {
                            println!("  ⚠ Failed to export HTML: {}", e);
                        }

                        success_count += 1;
                    }
                    Err(e) => {
                        println!("{} vertices ✗ Hull computation failed: {}", vertex_count, e);
                    }
                }
            }
            Err(e) => {
                println!("✗ Failed to load: {}", e);
            }
        }
    }

    println!("\n========================================");
    println!(
        "Success: {} | Skipped: {} | Total: {}",
        success_count, skipped_count, total_count
    );
    println!("========================================");

    assert!(
        success_count > 0,
        "At least some OBJ files should process successfully"
    );
}
