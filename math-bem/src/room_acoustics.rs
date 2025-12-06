//! Room Acoustics Simulator using BEM
//!
//! This module implements a 3D room acoustics simulator for calculating sound pressure
//! levels (SPL) at listening positions from directional sources with frequency-dependent
//! radiation patterns.
//!
//! **Note**: The solver functionality requires either the `native` or `wasm` feature for parallel processing.
//! Data structures (RoomGeometry, Source, etc.) are always available.

// Room acoustics is still experimental; allow missing docs for now
#![allow(missing_docs)]

mod config;
#[cfg(any(feature = "native", feature = "wasm"))]
mod solver;

pub use config::*;
#[cfg(any(feature = "native", feature = "wasm"))]
pub use solver::*;

use ndarray::Array2;
use serde::{Deserialize, Serialize};
use std::f64::consts::PI;

/// 3D point in space
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Point3D {
    /// X coordinate
    pub x: f64,
    /// Y coordinate
    pub y: f64,
    /// Z coordinate
    pub z: f64,
}

impl Point3D {
    /// Create a new 3D point
    pub fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }

    /// Calculate Euclidean distance to another point
    pub fn distance_to(&self, other: &Point3D) -> f64 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        let dz = self.z - other.z;
        (dx * dx + dy * dy + dz * dz).sqrt()
    }

    /// Convert to spherical coordinates (r, theta, phi)
    pub fn to_spherical(&self) -> (f64, f64, f64) {
        let r = (self.x * self.x + self.y * self.y + self.z * self.z).sqrt();
        let theta = (self.z / r).acos(); // polar angle (0 to π)
        let phi = self.y.atan2(self.x); // azimuthal angle (-π to π)
        (r, theta, phi)
    }
}

/// Room geometry types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RoomGeometry {
    /// Rectangular (shoebox) room
    Rectangular(RectangularRoom),
    /// L-shaped room
    LShaped(LShapedRoom),
}

impl RoomGeometry {
    /// Generate a surface mesh with specified resolution
    pub fn generate_mesh(&self, elements_per_meter: usize) -> RoomMesh {
        match self {
            RoomGeometry::Rectangular(room) => room.generate_mesh(elements_per_meter),
            RoomGeometry::LShaped(room) => room.generate_mesh(elements_per_meter),
        }
    }

    /// Generate frequency-adaptive mesh with selective refinement
    ///
    /// Refines mesh based on:
    /// - Wavelength criterion (λ/6 to λ/10 per element)
    /// - Distance to sources (finer near sources)
    /// - Geometric features (corners, edges get extra refinement)
    pub fn generate_adaptive_mesh(
        &self,
        base_elements_per_meter: usize,
        frequency: f64,
        sources: &[Source],
        speed_of_sound: f64,
    ) -> RoomMesh {
        match self {
            RoomGeometry::Rectangular(room) => room.generate_adaptive_mesh(
                base_elements_per_meter,
                frequency,
                sources,
                speed_of_sound,
            ),
            RoomGeometry::LShaped(room) => room.generate_adaptive_mesh(
                base_elements_per_meter,
                frequency,
                sources,
                speed_of_sound,
            ),
        }
    }

    /// Get the edges of the room geometry
    pub fn get_edges(&self) -> Vec<(Point3D, Point3D)> {
        match self {
            RoomGeometry::Rectangular(room) => room.get_edges(),
            RoomGeometry::LShaped(room) => room.get_edges(),
        }
    }
}

/// Surface information for adaptive mesh generation
struct SurfaceInfo {
    origin: Point3D,
    u_dir: Point3D,
    v_dir: Point3D,
    u_length: f64,
    v_length: f64,
}

/// Rectangular room defined by dimensions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RectangularRoom {
    /// Room width (x dimension)
    pub width: f64,
    /// Room depth (y dimension)
    pub depth: f64,
    /// Room height (z dimension)
    pub height: f64,
}

impl RectangularRoom {
    /// Create a new rectangular room with specified dimensions
    pub fn new(width: f64, depth: f64, height: f64) -> Self {
        Self {
            width,
            depth,
            height,
        }
    }

    /// Generate surface mesh for BEM
    pub fn generate_mesh(&self, elements_per_meter: usize) -> RoomMesh {
        let nx = (self.width * elements_per_meter as f64).ceil() as usize;
        let ny = (self.depth * elements_per_meter as f64).ceil() as usize;
        let nz = (self.height * elements_per_meter as f64).ceil() as usize;

        let mut nodes = Vec::new();
        let mut elements = Vec::new();

        // Floor (z=0)
        self.add_surface_mesh(
            &mut nodes,
            &mut elements,
            Point3D::new(0.0, 0.0, 0.0),
            Point3D::new(self.width, 0.0, 0.0),
            Point3D::new(0.0, self.depth, 0.0),
            nx,
            ny,
        );

        // Ceiling (z=height)
        self.add_surface_mesh(
            &mut nodes,
            &mut elements,
            Point3D::new(0.0, 0.0, self.height),
            Point3D::new(self.width, 0.0, self.height),
            Point3D::new(0.0, self.depth, self.height),
            nx,
            ny,
        );

        // Front wall (y=0)
        self.add_surface_mesh(
            &mut nodes,
            &mut elements,
            Point3D::new(0.0, 0.0, 0.0),
            Point3D::new(self.width, 0.0, 0.0),
            Point3D::new(0.0, 0.0, self.height),
            nx,
            nz,
        );

        // Back wall (y=depth)
        self.add_surface_mesh(
            &mut nodes,
            &mut elements,
            Point3D::new(0.0, self.depth, 0.0),
            Point3D::new(self.width, self.depth, 0.0),
            Point3D::new(0.0, self.depth, self.height),
            nx,
            nz,
        );

        // Left wall (x=0)
        self.add_surface_mesh(
            &mut nodes,
            &mut elements,
            Point3D::new(0.0, 0.0, 0.0),
            Point3D::new(0.0, self.depth, 0.0),
            Point3D::new(0.0, 0.0, self.height),
            ny,
            nz,
        );

        // Right wall (x=width)
        self.add_surface_mesh(
            &mut nodes,
            &mut elements,
            Point3D::new(self.width, 0.0, 0.0),
            Point3D::new(self.width, self.depth, 0.0),
            Point3D::new(self.width, 0.0, self.height),
            ny,
            nz,
        );

        RoomMesh { nodes, elements }
    }

    /// Generate frequency-adaptive mesh with selective refinement
    pub fn generate_adaptive_mesh(
        &self,
        base_elements_per_meter: usize,
        frequency: f64,
        sources: &[Source],
        speed_of_sound: f64,
    ) -> RoomMesh {
        let wavelength = speed_of_sound / frequency;

        // Target: λ/8 per element as a good compromise
        let target_element_size = wavelength / 8.0;

        let mut nodes = Vec::new();
        let mut elements = Vec::new();

        // Define all 6 surfaces with their refinement strategies
        let surfaces = vec![
            // Floor (z=0)
            SurfaceInfo {
                origin: Point3D::new(0.0, 0.0, 0.0),
                u_dir: Point3D::new(self.width, 0.0, 0.0),
                v_dir: Point3D::new(0.0, self.depth, 0.0),
                u_length: self.width,
                v_length: self.depth,
            },
            // Ceiling (z=height)
            SurfaceInfo {
                origin: Point3D::new(0.0, 0.0, self.height),
                u_dir: Point3D::new(self.width, 0.0, self.height),
                v_dir: Point3D::new(0.0, self.depth, self.height),
                u_length: self.width,
                v_length: self.depth,
            },
            // Front wall (y=0)
            SurfaceInfo {
                origin: Point3D::new(0.0, 0.0, 0.0),
                u_dir: Point3D::new(self.width, 0.0, 0.0),
                v_dir: Point3D::new(0.0, 0.0, self.height),
                u_length: self.width,
                v_length: self.height,
            },
            // Back wall (y=depth)
            SurfaceInfo {
                origin: Point3D::new(0.0, self.depth, 0.0),
                u_dir: Point3D::new(self.width, self.depth, 0.0),
                v_dir: Point3D::new(0.0, self.depth, self.height),
                u_length: self.width,
                v_length: self.height,
            },
            // Left wall (x=0)
            SurfaceInfo {
                origin: Point3D::new(0.0, 0.0, 0.0),
                u_dir: Point3D::new(0.0, self.depth, 0.0),
                v_dir: Point3D::new(0.0, 0.0, self.height),
                u_length: self.depth,
                v_length: self.height,
            },
            // Right wall (x=width)
            SurfaceInfo {
                origin: Point3D::new(self.width, 0.0, 0.0),
                u_dir: Point3D::new(self.width, self.depth, 0.0),
                v_dir: Point3D::new(self.width, 0.0, self.height),
                u_length: self.depth,
                v_length: self.height,
            },
        ];

        for surface in surfaces {
            self.add_adaptive_surface_mesh(
                &mut nodes,
                &mut elements,
                &surface,
                target_element_size,
                base_elements_per_meter,
                sources,
            );
        }

        RoomMesh { nodes, elements }
    }

    fn add_adaptive_surface_mesh(
        &self,
        nodes: &mut Vec<Point3D>,
        elements: &mut Vec<SurfaceElement>,
        surface: &SurfaceInfo,
        target_element_size: f64,
        base_elements_per_meter: usize,
        sources: &[Source],
    ) {
        // Compute adaptive resolution based on:
        // 1. Target element size from wavelength
        // 2. Distance to sources (refine near sources)
        // 3. Base resolution

        // Base number of elements from wavelength criterion
        let nu_base = (surface.u_length / target_element_size).ceil() as usize;
        let nv_base = (surface.v_length / target_element_size).ceil() as usize;

        // Ensure minimum resolution from base setting
        let nu_min = (surface.u_length * base_elements_per_meter as f64).ceil() as usize;
        let nv_min = (surface.v_length * base_elements_per_meter as f64).ceil() as usize;

        // Use the finer of the two criteria
        let nu = nu_base.max(nu_min);
        let nv = nv_base.max(nv_min);

        // Check if this surface is near any source
        let near_source = sources.iter().any(|source| {
            let dist_to_surface = self.distance_point_to_surface(&source.position, surface);
            dist_to_surface < target_element_size * 2.0
        });

        // Refine 2x near sources
        let (nu_final, nv_final) = if near_source {
            (nu * 2, nv * 2)
        } else {
            (nu, nv)
        };

        // Generate the mesh for this surface
        let base_idx = nodes.len();

        // Generate nodes with optional grading near edges/corners
        for j in 0..=nv_final {
            for i in 0..=nu_final {
                // Use graded spacing near edges (0 and 1) for better corner resolution
                let u = self.graded_parameter(i as f64 / nu_final as f64);
                let v = self.graded_parameter(j as f64 / nv_final as f64);

                let u_vec = Point3D::new(
                    surface.u_dir.x - surface.origin.x,
                    surface.u_dir.y - surface.origin.y,
                    surface.u_dir.z - surface.origin.z,
                );
                let v_vec = Point3D::new(
                    surface.v_dir.x - surface.origin.x,
                    surface.v_dir.y - surface.origin.y,
                    surface.v_dir.z - surface.origin.z,
                );

                let node = Point3D::new(
                    surface.origin.x + u * u_vec.x + v * v_vec.x,
                    surface.origin.y + u * u_vec.y + v * v_vec.y,
                    surface.origin.z + u * u_vec.z + v * v_vec.z,
                );

                nodes.push(node);
            }
        }

        // Generate triangular elements
        for j in 0..nv_final {
            for i in 0..nu_final {
                let i0 = base_idx + j * (nu_final + 1) + i;
                let i1 = i0 + 1;
                let i2 = i0 + (nu_final + 1);
                let i3 = i2 + 1;

                // First triangle
                elements.push(SurfaceElement {
                    nodes: vec![i0, i1, i2],
                });

                // Second triangle
                elements.push(SurfaceElement {
                    nodes: vec![i1, i3, i2],
                });
            }
        }
    }

    /// Graded parameter for finer spacing near edges (0 and 1)
    /// Uses smooth transition with quadratic grading
    fn graded_parameter(&self, t: f64) -> f64 {
        // Linear near center, finer near edges
        // Grading strength: 0.1 means subtle grading, 0.3 means stronger
        let grading = 0.15;

        if t < grading {
            // Near t=0: quadratic densification
            0.5 * (t / grading).powi(2) * grading
        } else if t > 1.0 - grading {
            // Near t=1: quadratic densification
            let t_rel = (t - (1.0 - grading)) / grading;
            1.0 - 0.5 * grading * (1.0 - t_rel).powi(2)
        } else {
            // Linear in the middle
            let t_mid = (t - grading) / (1.0 - 2.0 * grading);
            grading + t_mid * (1.0 - 2.0 * grading)
        }
    }

    /// Compute distance from a point to a surface
    fn distance_point_to_surface(&self, point: &Point3D, surface: &SurfaceInfo) -> f64 {
        // Compute plane normal
        let u_vec = Point3D::new(
            surface.u_dir.x - surface.origin.x,
            surface.u_dir.y - surface.origin.y,
            surface.u_dir.z - surface.origin.z,
        );
        let v_vec = Point3D::new(
            surface.v_dir.x - surface.origin.x,
            surface.v_dir.y - surface.origin.y,
            surface.v_dir.z - surface.origin.z,
        );

        // Cross product for normal
        let normal = Point3D::new(
            u_vec.y * v_vec.z - u_vec.z * v_vec.y,
            u_vec.z * v_vec.x - u_vec.x * v_vec.z,
            u_vec.x * v_vec.y - u_vec.y * v_vec.x,
        );

        let normal_length =
            (normal.x * normal.x + normal.y * normal.y + normal.z * normal.z).sqrt();

        if normal_length < 1e-10 {
            return point.distance_to(&surface.origin);
        }

        // Normalized normal
        let nx = normal.x / normal_length;
        let ny = normal.y / normal_length;
        let nz = normal.z / normal_length;

        // Distance to plane
        let dx = point.x - surface.origin.x;
        let dy = point.y - surface.origin.y;
        let dz = point.z - surface.origin.z;

        (dx * nx + dy * ny + dz * nz).abs()
    }

    /// Get room edges for visualization
    pub fn get_edges(&self) -> Vec<(Point3D, Point3D)> {
        vec![
            // Floor edges
            (
                Point3D::new(0.0, 0.0, 0.0),
                Point3D::new(self.width, 0.0, 0.0),
            ),
            (
                Point3D::new(self.width, 0.0, 0.0),
                Point3D::new(self.width, self.depth, 0.0),
            ),
            (
                Point3D::new(self.width, self.depth, 0.0),
                Point3D::new(0.0, self.depth, 0.0),
            ),
            (
                Point3D::new(0.0, self.depth, 0.0),
                Point3D::new(0.0, 0.0, 0.0),
            ),
            // Ceiling edges
            (
                Point3D::new(0.0, 0.0, self.height),
                Point3D::new(self.width, 0.0, self.height),
            ),
            (
                Point3D::new(self.width, 0.0, self.height),
                Point3D::new(self.width, self.depth, self.height),
            ),
            (
                Point3D::new(self.width, self.depth, self.height),
                Point3D::new(0.0, self.depth, self.height),
            ),
            (
                Point3D::new(0.0, self.depth, self.height),
                Point3D::new(0.0, 0.0, self.height),
            ),
            // Vertical edges
            (
                Point3D::new(0.0, 0.0, 0.0),
                Point3D::new(0.0, 0.0, self.height),
            ),
            (
                Point3D::new(self.width, 0.0, 0.0),
                Point3D::new(self.width, 0.0, self.height),
            ),
            (
                Point3D::new(self.width, self.depth, 0.0),
                Point3D::new(self.width, self.depth, self.height),
            ),
            (
                Point3D::new(0.0, self.depth, 0.0),
                Point3D::new(0.0, self.depth, self.height),
            ),
        ]
    }

    fn add_surface_mesh(
        &self,
        nodes: &mut Vec<Point3D>,
        elements: &mut Vec<SurfaceElement>,
        origin: Point3D,
        u_dir: Point3D,
        v_dir: Point3D,
        nu: usize,
        nv: usize,
    ) {
        let base_idx = nodes.len();

        // Generate nodes
        for j in 0..=nv {
            for i in 0..=nu {
                let u = i as f64 / nu as f64;
                let v = j as f64 / nv as f64;

                let node = Point3D::new(
                    origin.x + u * (u_dir.x - origin.x) + v * (v_dir.x - origin.x),
                    origin.y + u * (u_dir.y - origin.y) + v * (v_dir.y - origin.y),
                    origin.z + u * (u_dir.z - origin.z) + v * (v_dir.z - origin.z),
                );
                nodes.push(node);
            }
        }

        // Generate quadrilateral elements
        for j in 0..nv {
            for i in 0..nu {
                let n0 = base_idx + j * (nu + 1) + i;
                let n1 = base_idx + j * (nu + 1) + i + 1;
                let n2 = base_idx + (j + 1) * (nu + 1) + i + 1;
                let n3 = base_idx + (j + 1) * (nu + 1) + i;

                elements.push(SurfaceElement {
                    nodes: vec![n0, n1, n2, n3],
                });
            }
        }
    }
}

/// L-shaped room defined by two rectangular sections
/// Section 1: main room (0, 0) to (width1, depth1)
/// Section 2: extension from (0, depth1) to (width2, depth1 + depth2)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LShapedRoom {
    /// Main section width (x dimension)
    pub width1: f64,
    /// Main section depth (y dimension)
    pub depth1: f64,
    /// Extension width (x dimension), typically < width1
    pub width2: f64,
    /// Extension depth (y dimension)
    pub depth2: f64,
    /// Common height for both sections (z dimension)
    pub height: f64,
}

impl LShapedRoom {
    /// Create a new L-shaped room with specified dimensions
    pub fn new(width1: f64, depth1: f64, width2: f64, depth2: f64, height: f64) -> Self {
        Self {
            width1,
            depth1,
            width2,
            depth2,
            height,
        }
    }

    /// Generate surface mesh for L-shaped room
    pub fn generate_mesh(&self, elements_per_meter: usize) -> RoomMesh {
        let mut nodes = Vec::new();
        let mut elements = Vec::new();

        // Main section dimensions
        let nx1 = (self.width1 * elements_per_meter as f64).ceil() as usize;
        let ny1 = (self.depth1 * elements_per_meter as f64).ceil() as usize;

        // Extension dimensions
        let nx2 = (self.width2 * elements_per_meter as f64).ceil() as usize;
        let ny2 = (self.depth2 * elements_per_meter as f64).ceil() as usize;

        let nz = (self.height * elements_per_meter as f64).ceil() as usize;

        // Floor - Main section
        self.add_surface_mesh_lshaped(
            &mut nodes,
            &mut elements,
            Point3D::new(0.0, 0.0, 0.0),
            Point3D::new(self.width1, 0.0, 0.0),
            Point3D::new(0.0, self.depth1, 0.0),
            nx1,
            ny1,
        );

        // Floor - Extension section
        self.add_surface_mesh_lshaped(
            &mut nodes,
            &mut elements,
            Point3D::new(0.0, self.depth1, 0.0),
            Point3D::new(self.width2, self.depth1, 0.0),
            Point3D::new(0.0, self.depth1 + self.depth2, 0.0),
            nx2,
            ny2,
        );

        // Ceiling - Main section
        self.add_surface_mesh_lshaped(
            &mut nodes,
            &mut elements,
            Point3D::new(0.0, 0.0, self.height),
            Point3D::new(self.width1, 0.0, self.height),
            Point3D::new(0.0, self.depth1, self.height),
            nx1,
            ny1,
        );

        // Ceiling - Extension section
        self.add_surface_mesh_lshaped(
            &mut nodes,
            &mut elements,
            Point3D::new(0.0, self.depth1, self.height),
            Point3D::new(self.width2, self.depth1, self.height),
            Point3D::new(0.0, self.depth1 + self.depth2, self.height),
            nx2,
            ny2,
        );

        // Walls - Main section
        // Front wall (y=0)
        self.add_surface_mesh_lshaped(
            &mut nodes,
            &mut elements,
            Point3D::new(0.0, 0.0, 0.0),
            Point3D::new(self.width1, 0.0, 0.0),
            Point3D::new(0.0, 0.0, self.height),
            nx1,
            nz,
        );

        // Right wall of main section (x=width1)
        self.add_surface_mesh_lshaped(
            &mut nodes,
            &mut elements,
            Point3D::new(self.width1, 0.0, 0.0),
            Point3D::new(self.width1, self.depth1, 0.0),
            Point3D::new(self.width1, 0.0, self.height),
            ny1,
            nz,
        );

        // Left wall (x=0) - full height
        let total_depth = self.depth1 + self.depth2;
        let ny_total = ((total_depth) * elements_per_meter as f64).ceil() as usize;
        self.add_surface_mesh_lshaped(
            &mut nodes,
            &mut elements,
            Point3D::new(0.0, 0.0, 0.0),
            Point3D::new(0.0, total_depth, 0.0),
            Point3D::new(0.0, 0.0, self.height),
            ny_total,
            nz,
        );

        // Back wall of extension (y=depth1+depth2)
        self.add_surface_mesh_lshaped(
            &mut nodes,
            &mut elements,
            Point3D::new(0.0, total_depth, 0.0),
            Point3D::new(self.width2, total_depth, 0.0),
            Point3D::new(0.0, total_depth, self.height),
            nx2,
            nz,
        );

        // Right wall of extension (x=width2)
        self.add_surface_mesh_lshaped(
            &mut nodes,
            &mut elements,
            Point3D::new(self.width2, self.depth1, 0.0),
            Point3D::new(self.width2, total_depth, 0.0),
            Point3D::new(self.width2, self.depth1, self.height),
            ny2,
            nz,
        );

        // Internal walls at the L junction
        // Vertical wall from (width2, depth1) to (width1, depth1)
        let internal_width = self.width1 - self.width2;
        let nx_internal = (internal_width * elements_per_meter as f64).ceil() as usize;
        self.add_surface_mesh_lshaped(
            &mut nodes,
            &mut elements,
            Point3D::new(self.width2, self.depth1, 0.0),
            Point3D::new(self.width1, self.depth1, 0.0),
            Point3D::new(self.width2, self.depth1, self.height),
            nx_internal,
            nz,
        );

        RoomMesh { nodes, elements }
    }

    /// Get room edges for visualization
    pub fn get_edges(&self) -> Vec<(Point3D, Point3D)> {
        let total_depth = self.depth1 + self.depth2;
        vec![
            // Floor edges - Main section
            (
                Point3D::new(0.0, 0.0, 0.0),
                Point3D::new(self.width1, 0.0, 0.0),
            ),
            (
                Point3D::new(self.width1, 0.0, 0.0),
                Point3D::new(self.width1, self.depth1, 0.0),
            ),
            (
                Point3D::new(self.width1, self.depth1, 0.0),
                Point3D::new(self.width2, self.depth1, 0.0),
            ),
            (
                Point3D::new(self.width2, self.depth1, 0.0),
                Point3D::new(self.width2, total_depth, 0.0),
            ),
            (
                Point3D::new(self.width2, total_depth, 0.0),
                Point3D::new(0.0, total_depth, 0.0),
            ),
            (
                Point3D::new(0.0, total_depth, 0.0),
                Point3D::new(0.0, 0.0, 0.0),
            ),
            // Ceiling edges
            (
                Point3D::new(0.0, 0.0, self.height),
                Point3D::new(self.width1, 0.0, self.height),
            ),
            (
                Point3D::new(self.width1, 0.0, self.height),
                Point3D::new(self.width1, self.depth1, self.height),
            ),
            (
                Point3D::new(self.width1, self.depth1, self.height),
                Point3D::new(self.width2, self.depth1, self.height),
            ),
            (
                Point3D::new(self.width2, self.depth1, self.height),
                Point3D::new(self.width2, total_depth, self.height),
            ),
            (
                Point3D::new(self.width2, total_depth, self.height),
                Point3D::new(0.0, total_depth, self.height),
            ),
            (
                Point3D::new(0.0, total_depth, self.height),
                Point3D::new(0.0, 0.0, self.height),
            ),
            // Vertical edges
            (
                Point3D::new(0.0, 0.0, 0.0),
                Point3D::new(0.0, 0.0, self.height),
            ),
            (
                Point3D::new(self.width1, 0.0, 0.0),
                Point3D::new(self.width1, 0.0, self.height),
            ),
            (
                Point3D::new(self.width1, self.depth1, 0.0),
                Point3D::new(self.width1, self.depth1, self.height),
            ),
            (
                Point3D::new(self.width2, self.depth1, 0.0),
                Point3D::new(self.width2, self.depth1, self.height),
            ),
            (
                Point3D::new(self.width2, total_depth, 0.0),
                Point3D::new(self.width2, total_depth, self.height),
            ),
            (
                Point3D::new(0.0, total_depth, 0.0),
                Point3D::new(0.0, total_depth, self.height),
            ),
        ]
    }

    fn add_surface_mesh_lshaped(
        &self,
        nodes: &mut Vec<Point3D>,
        elements: &mut Vec<SurfaceElement>,
        origin: Point3D,
        u_dir: Point3D,
        v_dir: Point3D,
        nu: usize,
        nv: usize,
    ) {
        let base_idx = nodes.len();

        // Generate nodes
        for j in 0..=nv {
            for i in 0..=nu {
                let u = i as f64 / nu as f64;
                let v = j as f64 / nv as f64;

                let node = Point3D::new(
                    origin.x + u * (u_dir.x - origin.x) + v * (v_dir.x - origin.x),
                    origin.y + u * (u_dir.y - origin.y) + v * (v_dir.y - origin.y),
                    origin.z + u * (u_dir.z - origin.z) + v * (v_dir.z - origin.z),
                );
                nodes.push(node);
            }
        }

        // Generate quadrilateral elements
        for j in 0..nv {
            for i in 0..nu {
                let n0 = base_idx + j * (nu + 1) + i;
                let n1 = base_idx + j * (nu + 1) + i + 1;
                let n2 = base_idx + (j + 1) * (nu + 1) + i + 1;
                let n3 = base_idx + (j + 1) * (nu + 1) + i;

                elements.push(SurfaceElement {
                    nodes: vec![n0, n1, n2, n3],
                });
            }
        }
    }

    /// Generate frequency-adaptive mesh (stub - uses regular mesh for now)
    pub fn generate_adaptive_mesh(
        &self,
        base_elements_per_meter: usize,
        _frequency: f64,
        _sources: &[Source],
        _speed_of_sound: f64,
    ) -> RoomMesh {
        // TODO: Implement adaptive meshing for L-shaped rooms
        // For now, just use the regular mesh
        self.generate_mesh(base_elements_per_meter)
    }
}

/// Surface element (quadrilateral)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SurfaceElement {
    pub nodes: Vec<usize>,
}

/// Room mesh for BEM
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomMesh {
    pub nodes: Vec<Point3D>,
    pub elements: Vec<SurfaceElement>,
}

/// Directivity pattern sampled on a grid
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectivityPattern {
    /// Horizontal angles (azimuth) in degrees [0, 360) with step 10°
    pub horizontal_angles: Vec<f64>,
    /// Vertical angles (elevation) in degrees [0, 180] with step 10°
    pub vertical_angles: Vec<f64>,
    /// Magnitude at each (horizontal, vertical) angle pair
    /// Shape: [n_vertical, n_horizontal]
    pub magnitude: Array2<f64>,
}

impl DirectivityPattern {
    /// Create omnidirectional pattern (uniform radiation)
    pub fn omnidirectional() -> Self {
        let horizontal_angles: Vec<f64> = (0..36).map(|i| i as f64 * 10.0).collect();
        let vertical_angles: Vec<f64> = (0..19).map(|i| i as f64 * 10.0).collect();

        let magnitude = Array2::ones((vertical_angles.len(), horizontal_angles.len()));

        Self {
            horizontal_angles,
            vertical_angles,
            magnitude,
        }
    }

    /// Interpolate directivity at arbitrary direction
    pub fn interpolate(&self, theta: f64, phi: f64) -> f64 {
        // Convert spherical to degrees
        let theta_deg = theta.to_degrees();
        let mut phi_deg = phi.to_degrees();

        // Normalize phi to [0, 360)
        while phi_deg < 0.0 {
            phi_deg += 360.0;
        }
        while phi_deg >= 360.0 {
            phi_deg -= 360.0;
        }

        // Find surrounding angles
        let h_idx = (phi_deg / 10.0).floor() as usize;
        let v_idx = (theta_deg / 10.0).floor() as usize;

        let h_idx = h_idx.min(self.horizontal_angles.len() - 1);
        let v_idx = v_idx.min(self.vertical_angles.len() - 1);

        let h_next = (h_idx + 1) % self.horizontal_angles.len();
        let v_next = (v_idx + 1).min(self.vertical_angles.len() - 1);

        // Bilinear interpolation
        let h_frac = (phi_deg / 10.0) - h_idx as f64;
        let v_frac = (theta_deg / 10.0) - v_idx as f64;

        let m00 = self.magnitude[[v_idx, h_idx]];
        let m01 = self.magnitude[[v_idx, h_next]];
        let m10 = self.magnitude[[v_next, h_idx]];
        let m11 = self.magnitude[[v_next, h_next]];

        let m0 = m00 * (1.0 - h_frac) + m01 * h_frac;
        let m1 = m10 * (1.0 - h_frac) + m11 * h_frac;

        m0 * (1.0 - v_frac) + m1 * v_frac
    }
}

/// Crossover filter for frequency-limited sources
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CrossoverFilter {
    /// Full range (no filtering)
    FullRange,
    /// Lowpass filter (for subwoofers)
    Lowpass {
        cutoff_freq: f64,
        order: u32, // Filter order (2, 4, 6, 8)
    },
    /// Highpass filter (for tweeters/small speakers)
    Highpass { cutoff_freq: f64, order: u32 },
    /// Bandpass filter
    Bandpass {
        low_cutoff: f64,
        high_cutoff: f64,
        order: u32,
    },
}

impl CrossoverFilter {
    /// Get amplitude multiplier at a given frequency
    pub fn amplitude_at_frequency(&self, frequency: f64) -> f64 {
        match self {
            CrossoverFilter::FullRange => 1.0,
            CrossoverFilter::Lowpass { cutoff_freq, order } => {
                let ratio = frequency / cutoff_freq;
                1.0 / (1.0 + ratio.powi(*order as i32 * 2)).sqrt()
            }
            CrossoverFilter::Highpass { cutoff_freq, order } => {
                let ratio = cutoff_freq / frequency;
                1.0 / (1.0 + ratio.powi(*order as i32 * 2)).sqrt()
            }
            CrossoverFilter::Bandpass {
                low_cutoff,
                high_cutoff,
                order,
            } => {
                // Cascade of highpass and lowpass
                let high_ratio = low_cutoff / frequency;
                let low_ratio = frequency / high_cutoff;
                let hp_response = 1.0 / (1.0 + high_ratio.powi(*order as i32 * 2)).sqrt();
                let lp_response = 1.0 / (1.0 + low_ratio.powi(*order as i32 * 2)).sqrt();
                hp_response * lp_response
            }
        }
    }
}

/// Sound source with position and directivity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Source {
    pub position: Point3D,
    pub directivity: DirectivityPattern,
    pub amplitude: f64, // Source strength
    pub crossover: CrossoverFilter,
    pub name: String, // Optional name for the source (e.g., "Left Main", "Subwoofer")
}

impl Source {
    pub fn new(position: Point3D, directivity: DirectivityPattern, amplitude: f64) -> Self {
        Self {
            position,
            directivity,
            amplitude,
            crossover: CrossoverFilter::FullRange,
            name: String::from("Source"),
        }
    }

    pub fn with_crossover(mut self, crossover: CrossoverFilter) -> Self {
        self.crossover = crossover;
        self
    }

    pub fn with_name(mut self, name: String) -> Self {
        self.name = name;
        self
    }

    /// Get directional amplitude towards a point at a specific frequency
    pub fn amplitude_towards(&self, point: &Point3D, frequency: f64) -> f64 {
        let dx = point.x - self.position.x;
        let dy = point.y - self.position.y;
        let dz = point.z - self.position.z;

        let r = (dx * dx + dy * dy + dz * dz).sqrt();
        if r < 1e-10 {
            return self.amplitude * self.crossover.amplitude_at_frequency(frequency);
        }

        let theta = (dz / r).acos();
        let phi = dy.atan2(dx);

        let directivity_factor = self.directivity.interpolate(theta, phi);
        let crossover_factor = self.crossover.amplitude_at_frequency(frequency);
        self.amplitude * directivity_factor * crossover_factor
    }
}

/// Listening position
pub type ListeningPosition = Point3D;

/// Room acoustics simulation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomSimulation {
    pub room: RoomGeometry,
    pub sources: Vec<Source>,
    pub listening_positions: Vec<ListeningPosition>,
    pub frequencies: Vec<f64>,
    pub speed_of_sound: f64,
}

impl RoomSimulation {
    pub fn new(
        room: RoomGeometry,
        sources: Vec<Source>,
        listening_positions: Vec<ListeningPosition>,
    ) -> Self {
        // Generate logarithmically spaced frequencies 20Hz to 20kHz
        let frequencies = Self::log_space(20.0, 20000.0, 200);

        Self {
            room,
            sources,
            listening_positions,
            frequencies,
            speed_of_sound: 343.0, // m/s at 20°C
        }
    }

    /// Create simulation with custom frequency configuration
    pub fn with_frequencies(
        room: RoomGeometry,
        sources: Vec<Source>,
        listening_positions: Vec<ListeningPosition>,
        min_freq: f64,
        max_freq: f64,
        num_points: usize,
    ) -> Self {
        let frequencies = Self::log_space(min_freq, max_freq, num_points);

        Self {
            room,
            sources,
            listening_positions,
            frequencies,
            speed_of_sound: 343.0,
        }
    }

    fn log_space(start: f64, end: f64, num: usize) -> Vec<f64> {
        let log_start = start.ln();
        let log_end = end.ln();
        (0..num)
            .map(|i| {
                let log_val = log_start + (log_end - log_start) * i as f64 / (num - 1) as f64;
                log_val.exp()
            })
            .collect()
    }

    /// Calculate wavenumber k = 2π f / c
    pub fn wavenumber(&self, frequency: f64) -> f64 {
        2.0 * PI * frequency / self.speed_of_sound
    }
}

/// Result of room simulation at one frequency
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrequencyResult {
    pub frequency: f64,
    pub spl_at_lp: Vec<f64>, // SPL (dB) at each listening position
}

/// Complete simulation results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationResults {
    pub frequencies: Vec<f64>,
    pub lp_frequency_responses: Vec<Vec<f64>>, // [n_lp][n_freq]
    pub horizontal_slice: Option<SliceData>,
    pub vertical_slice: Option<SliceData>,
}

/// Pressure field data on a 2D slice
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SliceData {
    pub x: Vec<f64>,
    pub y: Vec<f64>,
    pub spl: Array2<f64>, // [y, x]
    pub frequency: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rectangular_room() {
        let room = RectangularRoom::new(5.0, 4.0, 3.0);
        assert_eq!(room.width, 5.0);
        assert_eq!(room.depth, 4.0);
        assert_eq!(room.height, 3.0);
    }

    #[test]
    fn test_omnidirectional_pattern() {
        let pattern = DirectivityPattern::omnidirectional();
        // Should be 1.0 in all directions
        assert!((pattern.interpolate(0.0, 0.0) - 1.0).abs() < 1e-6);
        assert!((pattern.interpolate(PI / 2.0, PI) - 1.0).abs() < 1e-6);
        assert!((pattern.interpolate(PI, 0.0) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_log_space() {
        let freqs = RoomSimulation::log_space(20.0, 20000.0, 200);
        assert_eq!(freqs.len(), 200);
        assert!((freqs[0] - 20.0).abs() < 1e-6);
        assert!((freqs[199] - 20000.0).abs() < 1e-6);
        // Check logarithmic spacing
        assert!(freqs[1] / freqs[0] > 1.0);
    }

    #[test]
    fn test_point_distance() {
        let p1 = Point3D::new(0.0, 0.0, 0.0);
        let p2 = Point3D::new(3.0, 4.0, 0.0);
        assert!((p1.distance_to(&p2) - 5.0).abs() < 1e-6);
    }
}
