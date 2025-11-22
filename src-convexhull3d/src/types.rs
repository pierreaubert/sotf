//! Core data types for 3D convex hull computation

use serde::{Deserialize, Serialize};
use std::fmt;

/// A 3D vertex/point
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Vertex {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Vertex {
    /// Create a new vertex
    pub fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }

    /// Create a vertex from spherical coordinates (azimuth, elevation in radians, radius)
    pub fn from_spherical(azimuth: f64, elevation: f64, radius: f64) -> Self {
        let x = radius * elevation.cos() * azimuth.cos();
        let y = radius * elevation.cos() * azimuth.sin();
        let z = radius * elevation.sin();
        Self { x, y, z }
    }

    /// Create a vertex from spherical coordinates in degrees
    pub fn from_spherical_deg(azimuth_deg: f64, elevation_deg: f64, radius: f64) -> Self {
        Self::from_spherical(azimuth_deg.to_radians(), elevation_deg.to_radians(), radius)
    }

    /// Dot product with another vertex
    pub fn dot(&self, other: &Vertex) -> f64 {
        self.x * other.x + self.y * other.y + self.z * other.z
    }

    /// Cross product with another vertex
    pub fn cross(&self, other: &Vertex) -> Vertex {
        Vertex {
            x: self.y * other.z - self.z * other.y,
            y: self.z * other.x - self.x * other.z,
            z: self.x * other.y - self.y * other.x,
        }
    }

    /// Subtract another vertex
    pub fn sub(&self, other: &Vertex) -> Vertex {
        Vertex {
            x: self.x - other.x,
            y: self.y - other.y,
            z: self.z - other.z,
        }
    }

    /// Add another vertex
    pub fn add(&self, other: &Vertex) -> Vertex {
        Vertex {
            x: self.x + other.x,
            y: self.y + other.y,
            z: self.z + other.z,
        }
    }

    /// Scale by a scalar
    pub fn scale(&self, s: f64) -> Vertex {
        Vertex {
            x: self.x * s,
            y: self.y * s,
            z: self.z * s,
        }
    }

    /// Compute the magnitude/length
    pub fn magnitude(&self) -> f64 {
        (self.x * self.x + self.y * self.y + self.z * self.z).sqrt()
    }

    /// Normalize to unit length
    pub fn normalize(&self) -> Vertex {
        let mag = self.magnitude();
        if mag > 1e-10 {
            self.scale(1.0 / mag)
        } else {
            *self
        }
    }

    /// Distance to another vertex
    pub fn distance(&self, other: &Vertex) -> f64 {
        self.sub(other).magnitude()
    }

    /// Add noise for numerical stability
    pub fn add_noise(&self, epsilon: f64) -> Vertex {
        use rand::Rng;
        let mut rng = rand::rng();
        Vertex {
            x: self.x + epsilon * (rng.random::<f64>() - 0.5),
            y: self.y + epsilon * (rng.random::<f64>() - 0.5),
            z: self.z + epsilon * (rng.random::<f64>() - 0.5),
        }
    }
}

impl fmt::Display for Vertex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({:.6}, {:.6}, {:.6})", self.x, self.y, self.z)
    }
}

/// A face of the convex hull (triangle defined by 3 vertex indices)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Face {
    pub v0: usize,
    pub v1: usize,
    pub v2: usize,
}

impl Face {
    /// Create a new face from three vertex indices
    pub fn new(v0: usize, v1: usize, v2: usize) -> Self {
        Self { v0, v1, v2 }
    }

    /// Get vertex indices as an array
    pub fn indices(&self) -> [usize; 3] {
        [self.v0, self.v1, self.v2]
    }

    /// Check if this face contains a vertex index
    pub fn contains(&self, v: usize) -> bool {
        self.v0 == v || self.v1 == v || self.v2 == v
    }

    /// Compute the normal vector of this face
    pub fn normal(&self, vertices: &[Vertex]) -> Vertex {
        let v0 = &vertices[self.v0];
        let v1 = &vertices[self.v1];
        let v2 = &vertices[self.v2];

        let e1 = v1.sub(v0);
        let e2 = v2.sub(v0);
        e1.cross(&e2).normalize()
    }

    /// Compute the centroid of this face
    pub fn centroid(&self, vertices: &[Vertex]) -> Vertex {
        let v0 = &vertices[self.v0];
        let v1 = &vertices[self.v1];
        let v2 = &vertices[self.v2];

        Vertex {
            x: (v0.x + v1.x + v2.x) / 3.0,
            y: (v0.y + v1.y + v2.y) / 3.0,
            z: (v0.z + v1.z + v2.z) / 3.0,
        }
    }

    /// Check if a point is visible from this face
    pub fn is_visible_from(&self, point: &Vertex, vertices: &[Vertex]) -> bool {
        let v0 = &vertices[self.v0];
        let normal = self.normal(vertices);
        let to_point = point.sub(v0);
        normal.dot(&to_point) > 1e-10
    }
}

/// The result of a convex hull computation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConvexHull3D {
    /// Original vertices
    vertices: Vec<Vertex>,
    /// Faces of the convex hull (each face is a triangle)
    faces: Vec<Face>,
}

impl ConvexHull3D {
    /// Create a new convex hull from vertices and faces
    pub(crate) fn new(vertices: Vec<Vertex>, faces: Vec<Face>) -> Self {
        Self { vertices, faces }
    }

    /// Build a convex hull from vertices using the Quickhull algorithm
    pub fn build(vertices: &[Vertex]) -> crate::Result<Self> {
        // Use the 3D-specific quickhull implementation
        crate::quickhull::quickhull_3d(vertices)
    }

    /// Get the vertices
    pub fn vertices(&self) -> &[Vertex] {
        &self.vertices
    }

    /// Get the faces
    pub fn faces(&self) -> &[Face] {
        &self.faces
    }

    /// Get the number of faces
    pub fn num_faces(&self) -> usize {
        self.faces.len()
    }

    /// Get the number of vertices
    pub fn num_vertices(&self) -> usize {
        self.vertices.len()
    }

    /// Compute the volume of the convex hull
    pub fn volume(&self) -> f64 {
        let mut volume = 0.0;

        for face in &self.faces {
            let v0 = &self.vertices[face.v0];
            let v1 = &self.vertices[face.v1];
            let v2 = &self.vertices[face.v2];

            // Volume of tetrahedron formed by origin and face
            let tetrahedron_volume = v0.dot(&v1.cross(v2)) / 6.0;
            volume += tetrahedron_volume;
        }

        volume.abs()
    }

    /// Compute the surface area of the convex hull
    pub fn surface_area(&self) -> f64 {
        let mut area = 0.0;

        for face in &self.faces {
            let v0 = &self.vertices[face.v0];
            let v1 = &self.vertices[face.v1];
            let v2 = &self.vertices[face.v2];

            let e1 = v1.sub(v0);
            let e2 = v2.sub(v0);
            let cross = e1.cross(&e2);
            area += cross.magnitude() / 2.0;
        }

        area
    }
}
