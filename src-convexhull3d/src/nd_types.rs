//! N-dimensional data types for convex hulls and Delaunay triangulation

use serde::{Deserialize, Serialize};
use std::fmt;

/// A point in N-dimensional space
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PointND {
    pub coords: Vec<f64>,
}

impl PointND {
    /// Create a new N-dimensional point
    pub fn new(coords: Vec<f64>) -> Self {
        Self { coords }
    }

    /// Get the dimensionality of this point
    pub fn dim(&self) -> usize {
        self.coords.len()
    }

    /// Dot product with another point
    pub fn dot(&self, other: &PointND) -> f64 {
        assert_eq!(self.dim(), other.dim());
        self.coords
            .iter()
            .zip(other.coords.iter())
            .map(|(a, b)| a * b)
            .sum()
    }

    /// Subtract another point
    pub fn sub(&self, other: &PointND) -> PointND {
        assert_eq!(self.dim(), other.dim());
        let coords = self
            .coords
            .iter()
            .zip(other.coords.iter())
            .map(|(a, b)| a - b)
            .collect();
        PointND { coords }
    }

    /// Add another point
    pub fn add(&self, other: &PointND) -> PointND {
        assert_eq!(self.dim(), other.dim());
        let coords = self
            .coords
            .iter()
            .zip(other.coords.iter())
            .map(|(a, b)| a + b)
            .collect();
        PointND { coords }
    }

    /// Scale by a scalar
    pub fn scale(&self, s: f64) -> PointND {
        let coords = self.coords.iter().map(|x| x * s).collect();
        PointND { coords }
    }

    /// Compute the magnitude/length
    pub fn magnitude(&self) -> f64 {
        self.coords.iter().map(|x| x * x).sum::<f64>().sqrt()
    }

    /// Distance to another point
    pub fn distance(&self, other: &PointND) -> f64 {
        self.sub(other).magnitude()
    }
}

impl fmt::Display for PointND {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "(")?;
        for (i, coord) in self.coords.iter().enumerate() {
            if i > 0 {
                write!(f, ", ")?;
            }
            write!(f, "{:.6}", coord)?;
        }
        write!(f, ")")
    }
}

/// A simplex (face) in N-dimensional space
/// For d-dimensional space, a face is a (d-1)-simplex defined by d vertex indices
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SimplexND {
    pub vertices: Vec<usize>,
}

impl SimplexND {
    /// Create a new simplex from vertex indices
    pub fn new(vertices: Vec<usize>) -> Self {
        Self { vertices }
    }

    /// Get the dimensionality of this simplex (number of vertices - 1)
    pub fn dim(&self) -> usize {
        self.vertices.len().saturating_sub(1)
    }

    /// Check if this simplex contains a vertex index
    pub fn contains(&self, v: usize) -> bool {
        self.vertices.contains(&v)
    }

    /// Compute the centroid of this simplex
    pub fn centroid(&self, points: &[PointND]) -> PointND {
        let n = self.vertices.len() as f64;
        let dim = points[0].dim();

        let mut coords = vec![0.0; dim];
        for &idx in &self.vertices {
            for (i, &coord) in points[idx].coords.iter().enumerate() {
                coords[i] += coord;
            }
        }

        for coord in &mut coords {
            *coord /= n;
        }

        PointND::new(coords)
    }
}

/// The result of an N-dimensional convex hull computation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConvexHullND {
    /// Original points
    points: Vec<PointND>,
    /// Facets of the convex hull (each facet is a (d-1)-simplex)
    facets: Vec<SimplexND>,
    /// Dimensionality of the space
    dim: usize,
}

impl ConvexHullND {
    /// Create a new N-D convex hull
    pub(crate) fn new(points: Vec<PointND>, facets: Vec<SimplexND>, dim: usize) -> Self {
        Self { points, facets, dim }
    }

    /// Build an N-dimensional convex hull from points
    pub fn build(points: &[PointND]) -> crate::Result<Self> {
        crate::quickhull_nd::quickhull_nd(points)
    }

    /// Get the points
    pub fn points(&self) -> &[PointND] {
        &self.points
    }

    /// Get the facets
    pub fn facets(&self) -> &[SimplexND] {
        &self.facets
    }

    /// Get the number of facets
    pub fn num_facets(&self) -> usize {
        self.facets.len()
    }

    /// Get the number of points
    pub fn num_points(&self) -> usize {
        self.points.len()
    }

    /// Get the dimensionality
    pub fn dim(&self) -> usize {
        self.dim
    }
}

/// The result of a Delaunay triangulation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DelaunayMesh {
    /// Original points (in d dimensions)
    points: Vec<PointND>,
    /// Simplices of the Delaunay mesh (each simplex has d+1 vertices)
    simplices: Vec<SimplexND>,
    /// Dimensionality of the space
    dim: usize,
}

impl DelaunayMesh {
    /// Create a new Delaunay mesh
    pub(crate) fn new(points: Vec<PointND>, simplices: Vec<SimplexND>, dim: usize) -> Self {
        Self { points, simplices, dim }
    }

    /// Build a Delaunay triangulation from points
    pub fn build(points: &[PointND]) -> crate::Result<Self> {
        crate::delaunay::delaunay_nd(points)
    }

    /// Get the points
    pub fn points(&self) -> &[PointND] {
        &self.points
    }

    /// Get the simplices
    pub fn simplices(&self) -> &[SimplexND] {
        &self.simplices
    }

    /// Get the number of simplices
    pub fn num_simplices(&self) -> usize {
        self.simplices.len()
    }

    /// Get the number of points
    pub fn num_points(&self) -> usize {
        self.points.len()
    }

    /// Get the dimensionality
    pub fn dim(&self) -> usize {
        self.dim
    }
}
