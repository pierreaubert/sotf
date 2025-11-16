//! Test data for convex hull tests
//!
//! This module provides test datasets similar to those used in the C++ convhull_3d library.

use crate::types::Vertex;
use rand::Rng;

/// Generate random points on a sphere
pub fn random_sphere_points(n: usize, radius: f64) -> Vec<Vertex> {
    let mut rng = rand::thread_rng();
    let mut vertices = Vec::with_capacity(n);

    for _ in 0..n {
        let azimuth = rng.random::<f64>() * 2.0 * std::f64::consts::PI;
        let elevation = (rng.random::<f64>() * 2.0 - 1.0).asin();
        let r = radius * (0.9 + 0.2 * rng.random::<f64>()); // Add some radius variation

        vertices.push(Vertex::from_spherical(azimuth, elevation, r));
    }

    vertices
}

/// Generate uniformly distributed points on a sphere using Fibonacci lattice
pub fn fibonacci_sphere_points(n: usize, radius: f64) -> Vec<Vertex> {
    let mut vertices = Vec::with_capacity(n);
    let golden_ratio = (1.0 + 5.0_f64.sqrt()) / 2.0;

    for i in 0..n {
        let theta = 2.0 * std::f64::consts::PI * (i as f64) / golden_ratio;
        let phi = ((2 * i + 1) as f64 / n as f64 - 1.0).acos();

        let x = radius * phi.sin() * theta.cos();
        let y = radius * phi.sin() * theta.sin();
        let z = radius * phi.cos();

        vertices.push(Vertex::new(x, y, z));
    }

    vertices
}

/// T-Design 180-point sphere (approximation using Fibonacci lattice)
pub fn tdesign_180_sphere() -> Vec<Vertex> {
    fibonacci_sphere_points(180, 1.0)
}

/// T-Design 840-point sphere (approximation using Fibonacci lattice)
pub fn tdesign_840_sphere() -> Vec<Vertex> {
    fibonacci_sphere_points(840, 1.0)
}

/// T-Design 5100-point sphere (approximation using Fibonacci lattice)
pub fn tdesign_5100_sphere() -> Vec<Vertex> {
    fibonacci_sphere_points(5100, 1.0)
}

/// Generate a cube's vertices
pub fn cube_vertices(size: f64) -> Vec<Vertex> {
    let s = size / 2.0;
    vec![
        Vertex::new(-s, -s, -s),
        Vertex::new(s, -s, -s),
        Vertex::new(s, s, -s),
        Vertex::new(-s, s, -s),
        Vertex::new(-s, -s, s),
        Vertex::new(s, -s, s),
        Vertex::new(s, s, s),
        Vertex::new(-s, s, s),
    ]
}

/// Generate vertices for a more complex shape (cube with interior points)
pub fn cube_with_interior_points(size: f64, n_interior: usize) -> Vec<Vertex> {
    let mut vertices = cube_vertices(size);
    let mut rng = rand::thread_rng();
    let s = size / 2.0;

    for _ in 0..n_interior {
        let x = rng.random::<f64>() * size - s;
        let y = rng.random::<f64>() * size - s;
        let z = rng.random::<f64>() * size - s;
        vertices.push(Vertex::new(x, y, z));
    }

    vertices
}

/// Generate a simple tetrahedron
pub fn tetrahedron_vertices() -> Vec<Vertex> {
    vec![
        Vertex::new(0.0, 0.0, 0.0),
        Vertex::new(1.0, 0.0, 0.0),
        Vertex::new(0.5, (3.0_f64).sqrt() / 2.0, 0.0),
        Vertex::new(0.5, (3.0_f64).sqrt() / 6.0, (2.0 / 3.0_f64).sqrt()),
    ]
}

/// Generate vertices for an icosahedron
pub fn icosahedron_vertices() -> Vec<Vertex> {
    let phi = (1.0 + 5.0_f64.sqrt()) / 2.0; // Golden ratio

    vec![
        Vertex::new(-1.0, phi, 0.0),
        Vertex::new(1.0, phi, 0.0),
        Vertex::new(-1.0, -phi, 0.0),
        Vertex::new(1.0, -phi, 0.0),
        Vertex::new(0.0, -1.0, phi),
        Vertex::new(0.0, 1.0, phi),
        Vertex::new(0.0, -1.0, -phi),
        Vertex::new(0.0, 1.0, -phi),
        Vertex::new(phi, 0.0, -1.0),
        Vertex::new(phi, 0.0, 1.0),
        Vertex::new(-phi, 0.0, -1.0),
        Vertex::new(-phi, 0.0, 1.0),
    ]
}

/// Generate vertices for an octahedron
pub fn octahedron_vertices() -> Vec<Vertex> {
    vec![
        Vertex::new(1.0, 0.0, 0.0),
        Vertex::new(-1.0, 0.0, 0.0),
        Vertex::new(0.0, 1.0, 0.0),
        Vertex::new(0.0, -1.0, 0.0),
        Vertex::new(0.0, 0.0, 1.0),
        Vertex::new(0.0, 0.0, -1.0),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_random_sphere_points() {
        let points = random_sphere_points(100, 1.0);
        assert_eq!(points.len(), 100);

        // Check that all points are approximately on the sphere
        for p in &points {
            let dist = p.magnitude();
            assert!(dist > 0.8 && dist < 1.2); // Allow for radius variation
        }
    }

    #[test]
    fn test_fibonacci_sphere_points() {
        let points = fibonacci_sphere_points(100, 1.0);
        assert_eq!(points.len(), 100);

        // Check that all points are on the sphere
        for p in &points {
            let dist = p.magnitude();
            assert!((dist - 1.0).abs() < 1e-10);
        }
    }

    #[test]
    fn test_cube_vertices() {
        let vertices = cube_vertices(2.0);
        assert_eq!(vertices.len(), 8);

        // Check that all vertices are at the correct distance from origin
        for v in &vertices {
            let dist = v.magnitude();
            assert!((dist - 3.0_f64.sqrt()).abs() < 1e-10);
        }
    }
}
