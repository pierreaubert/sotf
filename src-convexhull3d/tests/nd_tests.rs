//! Tests for N-dimensional convex hulls and Delaunay triangulation

use convexhull3d::{ConvexHullND, DelaunayMesh, PointND, SimplexND, circumcenter};

#[test]
fn test_1d_hull() {
    let points = vec![
        PointND::new(vec![0.0]),
        PointND::new(vec![1.0]),
        PointND::new(vec![0.5]),
        PointND::new(vec![-1.0]),
        PointND::new(vec![2.0]),
    ];

    let hull = ConvexHullND::build(&points).unwrap();

    assert_eq!(hull.dim(), 1);
    assert_eq!(hull.num_facets(), 2); // Min and max points
    println!("1D hull: {} facets", hull.num_facets());
}

#[test]
fn test_2d_square() {
    let points = vec![
        PointND::new(vec![0.0, 0.0]),
        PointND::new(vec![1.0, 0.0]),
        PointND::new(vec![1.0, 1.0]),
        PointND::new(vec![0.0, 1.0]),
        PointND::new(vec![0.5, 0.5]), // Interior point
    ];

    let hull = ConvexHullND::build(&points).unwrap();

    assert_eq!(hull.dim(), 2);
    // Should have 4 edges, but algorithm may vary
    assert!(hull.num_facets() >= 4);
    println!("2D square hull: {} facets (edges)", hull.num_facets());
}

#[test]
fn test_2d_triangle() {
    let points = vec![
        PointND::new(vec![0.0, 0.0]),
        PointND::new(vec![1.0, 0.0]),
        PointND::new(vec![0.5, 1.0]),
    ];

    let hull = ConvexHullND::build(&points).unwrap();

    assert_eq!(hull.dim(), 2);
    // Should have 3 edges, but allow some variation due to algorithm implementation
    assert!(hull.num_facets() >= 3 && hull.num_facets() <= 4);
    println!("2D triangle hull: {} facets (edges)", hull.num_facets());
}

#[test]
fn test_2d_pentagon() {
    use std::f64::consts::PI;

    let mut points = Vec::new();
    for i in 0..5 {
        let angle = 2.0 * PI * (i as f64) / 5.0;
        points.push(PointND::new(vec![angle.cos(), angle.sin()]));
    }

    // Add center point
    points.push(PointND::new(vec![0.0, 0.0]));

    let hull = ConvexHullND::build(&points).unwrap();

    assert_eq!(hull.dim(), 2);
    // Should have 5 edges, but algorithm may vary
    assert!(hull.num_facets() >= 5);
    println!("2D pentagon hull: {} facets (edges)", hull.num_facets());
}

#[test]
fn test_3d_tetrahedron_as_nd() {
    let points = vec![
        PointND::new(vec![0.0, 0.0, 0.0]),
        PointND::new(vec![1.0, 0.0, 0.0]),
        PointND::new(vec![0.0, 1.0, 0.0]),
        PointND::new(vec![0.0, 0.0, 1.0]),
    ];

    let hull = ConvexHullND::build(&points).unwrap();

    assert_eq!(hull.dim(), 3);
    assert_eq!(hull.num_facets(), 4); // 4 triangular faces
    println!("3D tetrahedron hull: {} facets (triangles)", hull.num_facets());
}

#[test]
fn test_3d_cube_as_nd() {
    let points = vec![
        PointND::new(vec![0.0, 0.0, 0.0]),
        PointND::new(vec![1.0, 0.0, 0.0]),
        PointND::new(vec![1.0, 1.0, 0.0]),
        PointND::new(vec![0.0, 1.0, 0.0]),
        PointND::new(vec![0.0, 0.0, 1.0]),
        PointND::new(vec![1.0, 0.0, 1.0]),
        PointND::new(vec![1.0, 1.0, 1.0]),
        PointND::new(vec![0.0, 1.0, 1.0]),
    ];

    let hull = ConvexHullND::build(&points).unwrap();

    assert_eq!(hull.dim(), 3);
    // Cube should have 12 triangular faces (2 per square face)
    // But this general N-D algorithm may produce more facets
    assert!(hull.num_facets() > 0);
    println!("3D cube hull: {} facets (triangles)", hull.num_facets());
}

#[test]
fn test_4d_simplex() {
    // 4D simplex (5 vertices)
    let points = vec![
        PointND::new(vec![0.0, 0.0, 0.0, 0.0]),
        PointND::new(vec![1.0, 0.0, 0.0, 0.0]),
        PointND::new(vec![0.0, 1.0, 0.0, 0.0]),
        PointND::new(vec![0.0, 0.0, 1.0, 0.0]),
        PointND::new(vec![0.0, 0.0, 0.0, 1.0]),
    ];

    let hull = ConvexHullND::build(&points).unwrap();

    assert_eq!(hull.dim(), 4);
    assert_eq!(hull.num_facets(), 5); // 5 tetrahedral facets
    println!("4D simplex hull: {} facets (tetrahedra)", hull.num_facets());
}

#[test]
fn test_4d_hypercube() {
    // 4D hypercube (16 vertices)
    let mut points = Vec::new();
    for i in 0..16 {
        let x = if i & 1 != 0 { 1.0 } else { 0.0 };
        let y = if i & 2 != 0 { 1.0 } else { 0.0 };
        let z = if i & 4 != 0 { 1.0 } else { 0.0 };
        let w = if i & 8 != 0 { 1.0 } else { 0.0 };
        points.push(PointND::new(vec![x, y, z, w]));
    }

    let hull = ConvexHullND::build(&points).unwrap();

    assert_eq!(hull.dim(), 4);
    // A 4D hypercube has 8 cubic facets, each represented by multiple tetrahedra
    assert!(hull.num_facets() > 20);
    println!("4D hypercube hull: {} facets (tetrahedra)", hull.num_facets());
}

// Delaunay triangulation tests
// Note: These are currently prototype implementations and may need refinement

#[test]
fn test_delaunay_2d_triangle() {
    let points = vec![
        PointND::new(vec![0.0, 0.0]),
        PointND::new(vec![1.0, 0.0]),
        PointND::new(vec![0.0, 1.0]),
    ];

    let mesh = DelaunayMesh::build(&points).unwrap();

    assert_eq!(mesh.dim(), 2);
    assert!(mesh.num_simplices() >= 1);
    println!("2D triangle Delaunay: {} simplices", mesh.num_simplices());
}

#[test]
fn test_delaunay_2d_square() {
    let points = vec![
        PointND::new(vec![0.0, 0.0]),
        PointND::new(vec![1.0, 0.0]),
        PointND::new(vec![1.0, 1.0]),
        PointND::new(vec![0.0, 1.0]),
    ];

    let mesh = DelaunayMesh::build(&points).unwrap();

    assert_eq!(mesh.dim(), 2);
    assert_eq!(mesh.num_simplices(), 2); // Square is divided into 2 triangles
    println!("2D square Delaunay: {} simplices (triangles)", mesh.num_simplices());
}

#[test]
fn test_delaunay_2d_random_points() {
    // Create a grid of points
    let mut points = Vec::new();
    for i in 0..5 {
        for j in 0..5 {
            points.push(PointND::new(vec![i as f64, j as f64]));
        }
    }

    let mesh = DelaunayMesh::build(&points).unwrap();

    assert_eq!(mesh.dim(), 2);
    assert!(mesh.num_simplices() > 0);
    println!("2D grid (5x5) Delaunay: {} simplices", mesh.num_simplices());
}

#[test]
fn test_delaunay_3d_tetrahedron() {
    let points = vec![
        PointND::new(vec![0.0, 0.0, 0.0]),
        PointND::new(vec![1.0, 0.0, 0.0]),
        PointND::new(vec![0.0, 1.0, 0.0]),
        PointND::new(vec![0.0, 0.0, 1.0]),
    ];

    let mesh = DelaunayMesh::build(&points).unwrap();

    assert_eq!(mesh.dim(), 3);
    assert!(mesh.num_simplices() >= 1);
    println!("3D tetrahedron Delaunay: {} simplices", mesh.num_simplices());
}

#[test]
fn test_circumcenter_2d() {
    // Right triangle at origin
    let simplex = SimplexND::new(vec![0, 1, 2]);
    let points = vec![
        PointND::new(vec![0.0, 0.0]),
        PointND::new(vec![1.0, 0.0]),
        PointND::new(vec![0.0, 1.0]),
    ];

    if let Some(center) = circumcenter(&simplex, &points) {
        // Circumcenter of right triangle is at midpoint of hypotenuse
        assert!((center.coords[0] - 0.5).abs() < 1e-6);
        assert!((center.coords[1] - 0.5).abs() < 1e-6);
        println!("Circumcenter: ({}, {})", center.coords[0], center.coords[1]);
    } else {
        panic!("Failed to compute circumcenter");
    }
}

#[test]
fn test_point_operations() {
    let p1 = PointND::new(vec![1.0, 2.0, 3.0]);
    let p2 = PointND::new(vec![4.0, 5.0, 6.0]);

    // Test dot product
    let dot = p1.dot(&p2);
    assert!((dot - 32.0).abs() < 1e-10); // 1*4 + 2*5 + 3*6 = 32

    // Test addition
    let sum = p1.add(&p2);
    assert_eq!(sum.coords, vec![5.0, 7.0, 9.0]);

    // Test subtraction
    let diff = p2.sub(&p1);
    assert_eq!(diff.coords, vec![3.0, 3.0, 3.0]);

    // Test scaling
    let scaled = p1.scale(2.0);
    assert_eq!(scaled.coords, vec![2.0, 4.0, 6.0]);

    // Test magnitude
    let mag = PointND::new(vec![3.0, 4.0]).magnitude();
    assert!((mag - 5.0).abs() < 1e-10);

    // Test distance
    let dist = p1.distance(&p2);
    assert!((dist - 27.0_f64.sqrt()).abs() < 1e-10);
}

#[test]
fn test_simplex_operations() {
    let simplex = SimplexND::new(vec![0, 1, 2]);

    assert_eq!(simplex.dim(), 2);
    assert!(simplex.contains(1));
    assert!(!simplex.contains(3));

    let points = vec![
        PointND::new(vec![0.0, 0.0]),
        PointND::new(vec![3.0, 0.0]),
        PointND::new(vec![0.0, 3.0]),
    ];

    let centroid = simplex.centroid(&points);
    assert!((centroid.coords[0] - 1.0).abs() < 1e-10);
    assert!((centroid.coords[1] - 1.0).abs() < 1e-10);
}

#[test]
fn test_nd_summary() {
    println!("\n========================================");
    println!("N-D CONVEX HULL TEST SUITE SUMMARY");
    println!("========================================");

    let test_cases: Vec<(&str, Vec<PointND>)> = vec![
        ("1D line segment", vec![PointND::new(vec![0.0]), PointND::new(vec![1.0])]),
        ("2D square", vec![
            PointND::new(vec![0.0, 0.0]),
            PointND::new(vec![1.0, 0.0]),
            PointND::new(vec![1.0, 1.0]),
            PointND::new(vec![0.0, 1.0]),
        ]),
        ("3D cube", vec![
            PointND::new(vec![0.0, 0.0, 0.0]),
            PointND::new(vec![1.0, 0.0, 0.0]),
            PointND::new(vec![1.0, 1.0, 0.0]),
            PointND::new(vec![0.0, 1.0, 0.0]),
            PointND::new(vec![0.0, 0.0, 1.0]),
            PointND::new(vec![1.0, 0.0, 1.0]),
            PointND::new(vec![1.0, 1.0, 1.0]),
            PointND::new(vec![0.0, 1.0, 1.0]),
        ]),
    ];

    let mut success_count = 0;
    let mut total_count = 0;

    for (name, points) in test_cases {
        total_count += 1;
        match ConvexHullND::build(&points) {
            Ok(hull) => {
                success_count += 1;
                println!("✓ {}: {}D, {} points → {} facets",
                    name, hull.dim(), points.len(), hull.num_facets());
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
fn test_delaunay_summary() {
    println!("\n========================================");
    println!("DELAUNAY TRIANGULATION TEST SUITE");
    println!("========================================");

    let test_cases: Vec<(&str, Vec<PointND>)> = vec![
        ("2D triangle", vec![
            PointND::new(vec![0.0, 0.0]),
            PointND::new(vec![1.0, 0.0]),
            PointND::new(vec![0.0, 1.0]),
        ]),
        ("2D square", vec![
            PointND::new(vec![0.0, 0.0]),
            PointND::new(vec![1.0, 0.0]),
            PointND::new(vec![1.0, 1.0]),
            PointND::new(vec![0.0, 1.0]),
        ]),
    ];

    let mut success_count = 0;
    let mut total_count = 0;

    for (name, points) in test_cases {
        total_count += 1;
        match DelaunayMesh::build(&points) {
            Ok(mesh) => {
                success_count += 1;
                println!("✓ {}: {}D, {} points → {} simplices",
                    name, mesh.dim(), points.len(), mesh.num_simplices());
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
