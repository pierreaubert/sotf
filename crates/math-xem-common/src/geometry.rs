//! Room geometry definitions

use crate::source::Source;
use crate::types::{Point3D, RoomMesh, SurfaceElement};
use serde::{Deserialize, Serialize};

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

    /// Get the edges of the room geometry for visualization
    pub fn get_edges(&self) -> Vec<(Point3D, Point3D)> {
        match self {
            RoomGeometry::Rectangular(room) => room.get_edges(),
            RoomGeometry::LShaped(room) => room.get_edges(),
        }
    }

    /// Get room dimensions (width, depth, height)
    pub fn dimensions(&self) -> (f64, f64, f64) {
        match self {
            RoomGeometry::Rectangular(r) => (r.width, r.depth, r.height),
            RoomGeometry::LShaped(r) => (r.width1.max(r.width2), r.depth1 + r.depth2, r.height),
        }
    }

    /// Get room volume in cubic meters
    pub fn volume(&self) -> f64 {
        match self {
            RoomGeometry::Rectangular(r) => r.width * r.depth * r.height,
            RoomGeometry::LShaped(r) => {
                r.width1 * r.depth1 * r.height + r.width2 * r.depth2 * r.height
            }
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
        Self::add_surface_mesh(
            &mut nodes,
            &mut elements,
            Point3D::new(0.0, 0.0, 0.0),
            Point3D::new(self.width, 0.0, 0.0),
            Point3D::new(0.0, self.depth, 0.0),
            nx,
            ny,
        );

        // Ceiling (z=height)
        Self::add_surface_mesh(
            &mut nodes,
            &mut elements,
            Point3D::new(0.0, 0.0, self.height),
            Point3D::new(self.width, 0.0, self.height),
            Point3D::new(0.0, self.depth, self.height),
            nx,
            ny,
        );

        // Front wall (y=0)
        Self::add_surface_mesh(
            &mut nodes,
            &mut elements,
            Point3D::new(0.0, 0.0, 0.0),
            Point3D::new(self.width, 0.0, 0.0),
            Point3D::new(0.0, 0.0, self.height),
            nx,
            nz,
        );

        // Back wall (y=depth)
        Self::add_surface_mesh(
            &mut nodes,
            &mut elements,
            Point3D::new(0.0, self.depth, 0.0),
            Point3D::new(self.width, self.depth, 0.0),
            Point3D::new(0.0, self.depth, self.height),
            nx,
            nz,
        );

        // Left wall (x=0)
        Self::add_surface_mesh(
            &mut nodes,
            &mut elements,
            Point3D::new(0.0, 0.0, 0.0),
            Point3D::new(0.0, self.depth, 0.0),
            Point3D::new(0.0, 0.0, self.height),
            ny,
            nz,
        );

        // Right wall (x=width)
        Self::add_surface_mesh(
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
        let target_element_size = wavelength / 8.0;

        let mut nodes = Vec::new();
        let mut elements = Vec::new();

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
        let nu_base = (surface.u_length / target_element_size).ceil() as usize;
        let nv_base = (surface.v_length / target_element_size).ceil() as usize;

        let nu_min = (surface.u_length * base_elements_per_meter as f64).ceil() as usize;
        let nv_min = (surface.v_length * base_elements_per_meter as f64).ceil() as usize;

        let nu = nu_base.max(nu_min);
        let nv = nv_base.max(nv_min);

        let near_source = sources.iter().any(|source| {
            let dist_to_surface = self.distance_point_to_surface(&source.position, surface);
            dist_to_surface < target_element_size * 2.0
        });

        let (nu_final, nv_final) = if near_source {
            (nu * 2, nv * 2)
        } else {
            (nu, nv)
        };

        let base_idx = nodes.len();

        for j in 0..=nv_final {
            for i in 0..=nu_final {
                let u = Self::graded_parameter(i as f64 / nu_final as f64);
                let v = Self::graded_parameter(j as f64 / nv_final as f64);

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

        for j in 0..nv_final {
            for i in 0..nu_final {
                let i0 = base_idx + j * (nu_final + 1) + i;
                let i1 = i0 + 1;
                let i2 = i0 + (nu_final + 1);
                let i3 = i2 + 1;

                elements.push(SurfaceElement::triangle(i0, i1, i2));
                elements.push(SurfaceElement::triangle(i1, i3, i2));
            }
        }
    }

    fn graded_parameter(t: f64) -> f64 {
        let grading = 0.15;

        if t < grading {
            0.5 * (t / grading).powi(2) * grading
        } else if t > 1.0 - grading {
            let t_rel = (t - (1.0 - grading)) / grading;
            1.0 - 0.5 * grading * (1.0 - t_rel).powi(2)
        } else {
            let t_mid = (t - grading) / (1.0 - 2.0 * grading);
            grading + t_mid * (1.0 - 2.0 * grading)
        }
    }

    fn distance_point_to_surface(&self, point: &Point3D, surface: &SurfaceInfo) -> f64 {
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

        let normal = u_vec.cross(&v_vec);
        let normal_length = normal.length();

        if normal_length < 1e-10 {
            return point.distance_to(&surface.origin);
        }

        let nx = normal.x / normal_length;
        let ny = normal.y / normal_length;
        let nz = normal.z / normal_length;

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
        nodes: &mut Vec<Point3D>,
        elements: &mut Vec<SurfaceElement>,
        origin: Point3D,
        u_dir: Point3D,
        v_dir: Point3D,
        nu: usize,
        nv: usize,
    ) {
        let base_idx = nodes.len();

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

        for j in 0..nv {
            for i in 0..nu {
                let n0 = base_idx + j * (nu + 1) + i;
                let n1 = base_idx + j * (nu + 1) + i + 1;
                let n2 = base_idx + (j + 1) * (nu + 1) + i + 1;
                let n3 = base_idx + (j + 1) * (nu + 1) + i;

                elements.push(SurfaceElement::quad(n0, n1, n2, n3));
            }
        }
    }
}

/// L-shaped room defined by two rectangular sections
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

        let nx1 = (self.width1 * elements_per_meter as f64).ceil() as usize;
        let ny1 = (self.depth1 * elements_per_meter as f64).ceil() as usize;
        let nx2 = (self.width2 * elements_per_meter as f64).ceil() as usize;
        let ny2 = (self.depth2 * elements_per_meter as f64).ceil() as usize;
        let nz = (self.height * elements_per_meter as f64).ceil() as usize;

        // Floor - Main section
        Self::add_surface_mesh_lshaped(
            &mut nodes,
            &mut elements,
            Point3D::new(0.0, 0.0, 0.0),
            Point3D::new(self.width1, 0.0, 0.0),
            Point3D::new(0.0, self.depth1, 0.0),
            nx1,
            ny1,
        );

        // Floor - Extension section
        Self::add_surface_mesh_lshaped(
            &mut nodes,
            &mut elements,
            Point3D::new(0.0, self.depth1, 0.0),
            Point3D::new(self.width2, self.depth1, 0.0),
            Point3D::new(0.0, self.depth1 + self.depth2, 0.0),
            nx2,
            ny2,
        );

        // Ceiling - Main section
        Self::add_surface_mesh_lshaped(
            &mut nodes,
            &mut elements,
            Point3D::new(0.0, 0.0, self.height),
            Point3D::new(self.width1, 0.0, self.height),
            Point3D::new(0.0, self.depth1, self.height),
            nx1,
            ny1,
        );

        // Ceiling - Extension section
        Self::add_surface_mesh_lshaped(
            &mut nodes,
            &mut elements,
            Point3D::new(0.0, self.depth1, self.height),
            Point3D::new(self.width2, self.depth1, self.height),
            Point3D::new(0.0, self.depth1 + self.depth2, self.height),
            nx2,
            ny2,
        );

        // Walls
        // Front wall (y=0)
        Self::add_surface_mesh_lshaped(
            &mut nodes,
            &mut elements,
            Point3D::new(0.0, 0.0, 0.0),
            Point3D::new(self.width1, 0.0, 0.0),
            Point3D::new(0.0, 0.0, self.height),
            nx1,
            nz,
        );

        // Right wall of main section (x=width1)
        Self::add_surface_mesh_lshaped(
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
        Self::add_surface_mesh_lshaped(
            &mut nodes,
            &mut elements,
            Point3D::new(0.0, 0.0, 0.0),
            Point3D::new(0.0, total_depth, 0.0),
            Point3D::new(0.0, 0.0, self.height),
            ny_total,
            nz,
        );

        // Back wall of extension (y=depth1+depth2)
        Self::add_surface_mesh_lshaped(
            &mut nodes,
            &mut elements,
            Point3D::new(0.0, total_depth, 0.0),
            Point3D::new(self.width2, total_depth, 0.0),
            Point3D::new(0.0, total_depth, self.height),
            nx2,
            nz,
        );

        // Right wall of extension (x=width2)
        Self::add_surface_mesh_lshaped(
            &mut nodes,
            &mut elements,
            Point3D::new(self.width2, self.depth1, 0.0),
            Point3D::new(self.width2, total_depth, 0.0),
            Point3D::new(self.width2, self.depth1, self.height),
            ny2,
            nz,
        );

        // Internal walls at the L junction
        let internal_width = self.width1 - self.width2;
        let nx_internal = (internal_width * elements_per_meter as f64).ceil() as usize;
        Self::add_surface_mesh_lshaped(
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
        nodes: &mut Vec<Point3D>,
        elements: &mut Vec<SurfaceElement>,
        origin: Point3D,
        u_dir: Point3D,
        v_dir: Point3D,
        nu: usize,
        nv: usize,
    ) {
        let base_idx = nodes.len();

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

        for j in 0..nv {
            for i in 0..nu {
                let n0 = base_idx + j * (nu + 1) + i;
                let n1 = base_idx + j * (nu + 1) + i + 1;
                let n2 = base_idx + (j + 1) * (nu + 1) + i + 1;
                let n3 = base_idx + (j + 1) * (nu + 1) + i;

                elements.push(SurfaceElement::quad(n0, n1, n2, n3));
            }
        }
    }

    /// Generate frequency-adaptive mesh (uses regular mesh for now)
    pub fn generate_adaptive_mesh(
        &self,
        base_elements_per_meter: usize,
        _frequency: f64,
        _sources: &[Source],
        _speed_of_sound: f64,
    ) -> RoomMesh {
        // TODO: Implement adaptive meshing for L-shaped rooms
        self.generate_mesh(base_elements_per_meter)
    }
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
    fn test_rectangular_room_mesh() {
        let room = RectangularRoom::new(2.0, 2.0, 2.0);
        let mesh = room.generate_mesh(1);
        assert!(mesh.num_nodes() > 0);
        assert!(mesh.num_elements() > 0);
    }

    #[test]
    fn test_lshaped_room() {
        let room = LShapedRoom::new(5.0, 4.0, 3.0, 3.0, 2.5);
        assert_eq!(room.width1, 5.0);
        assert_eq!(room.depth1, 4.0);
        assert_eq!(room.width2, 3.0);
        assert_eq!(room.depth2, 3.0);
        assert_eq!(room.height, 2.5);
    }
}
