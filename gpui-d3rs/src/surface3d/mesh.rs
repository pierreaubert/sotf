//! Mesh generation for 3D surfaces

use super::config::SurfacePlotType;
use super::data::SurfaceData;
use bytemuck::{Pod, Zeroable};
use glam::Vec3;

/// GPU vertex representation (must match shader layout)
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct GpuVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub value: f32,
    pub _padding: f32, // Align to 32 bytes
}

impl GpuVertex {
    pub fn new(position: Vec3, normal: Vec3, value: f32) -> Self {
        Self {
            position: position.to_array(),
            normal: normal.to_array(),
            value,
            _padding: 0.0,
        }
    }
}

/// Surface mesh containing GPU-ready vertex and index data
#[derive(Debug)]
pub struct SurfaceMesh {
    /// Vertex data
    pub vertices: Vec<GpuVertex>,
    /// Triangle indices
    pub indices: Vec<u32>,
    /// Number of vertices
    pub vertex_count: usize,
    /// Number of indices
    pub index_count: usize,
}

impl SurfaceMesh {
    /// Generate a surface mesh from surface data
    pub fn from_data(data: &SurfaceData, plot_type: SurfacePlotType) -> Self {
        let x_count = data.x_count();
        let y_count = data.y_count();

        if x_count < 2 || y_count < 2 {
            return Self::empty();
        }

        // Generate vertices
        let mut vertices = Vec::with_capacity(x_count * y_count);

        for yi in 0..y_count {
            for xi in 0..x_count {
                let x = data.x_values[xi];
                let y = data.y_values[yi];
                let z = data.z_values[yi][xi];

                // Normalize to [-1, 1] range for x and y, and scale z appropriately
                let nx = data.normalize_x(x);
                let ny = data.normalize_y(y);
                let nz = data.normalize_z(z);

                let position = match plot_type {
                    SurfacePlotType::Cartesian => {
                        // Map normalized z [0,1] to height [-0.5, 0.5]
                        let height = nz - 0.5;
                        Vec3::new(nx, height, ny)
                    }
                    SurfacePlotType::Spherical => {
                        // Map X (Freq) to Latitude (Phi): [-1, 1] -> [-PI/2, PI/2]
                        // Map Y (Angle) to Longitude (Theta): [-1, 1] -> [-PI, PI]
                        // Map Z (SPL) to Radius? Or just color.
                        // Let's use Radius = 1.0 + nz * 0.2 (slight extrusion)

                        let phi = nx * std::f32::consts::FRAC_PI_2; // -90 to 90 deg
                        let theta = ny * std::f32::consts::PI; // -180 to 180 deg
                        let radius = 1.0; // Unit sphere

                        // Spherical to Cartesian
                        // y is up (sin phi)
                        // x, z are horizontal plane
                        let y_pos = radius * phi.sin();
                        let r_xz = radius * phi.cos();
                        let x_pos = r_xz * theta.sin();
                        let z_pos = r_xz * theta.cos();

                        Vec3::new(x_pos, y_pos, z_pos)
                    }
                };

                let value = nz;

                // Placeholder normal - will be computed after
                vertices.push(GpuVertex::new(position, Vec3::Y, value));
            }
        }

        // Compute normals using central differences
        Self::compute_normals(&mut vertices, x_count, y_count);

        // Generate triangle indices (two triangles per grid cell)
        let mut indices = Vec::with_capacity((x_count - 1) * (y_count - 1) * 6);

        for yi in 0..(y_count - 1) {
            for xi in 0..(x_count - 1) {
                let i00 = (yi * x_count + xi) as u32;
                let i10 = (yi * x_count + xi + 1) as u32;
                let i01 = ((yi + 1) * x_count + xi) as u32;
                let i11 = ((yi + 1) * x_count + xi + 1) as u32;

                // First triangle
                indices.push(i00);
                indices.push(i10);
                indices.push(i01);

                // Second triangle
                indices.push(i10);
                indices.push(i11);
                indices.push(i01);
            }
        }

        Self {
            vertex_count: vertices.len(),
            index_count: indices.len(),
            vertices,
            indices,
        }
    }

    /// Create an empty mesh
    pub fn empty() -> Self {
        Self {
            vertices: Vec::new(),
            indices: Vec::new(),
            vertex_count: 0,
            index_count: 0,
        }
    }

    /// Compute vertex normals using central differences
    fn compute_normals(vertices: &mut [GpuVertex], x_count: usize, y_count: usize) {
        for yi in 0..y_count {
            for xi in 0..x_count {
                let idx = yi * x_count + xi;

                // Get neighboring heights
                let get_pos = |xi: usize, yi: usize| -> Vec3 {
                    let i = yi * x_count + xi;
                    Vec3::from_array(vertices[i].position)
                };

                // Use central differences where possible, forward/backward at edges
                let pos = get_pos(xi, yi);

                let dx = if xi == 0 {
                    get_pos(xi + 1, yi) - pos
                } else if xi == x_count - 1 {
                    pos - get_pos(xi - 1, yi)
                } else {
                    (get_pos(xi + 1, yi) - get_pos(xi - 1, yi)) * 0.5
                };

                let dy = if yi == 0 {
                    get_pos(xi, yi + 1) - pos
                } else if yi == y_count - 1 {
                    pos - get_pos(xi, yi - 1)
                } else {
                    (get_pos(xi, yi + 1) - get_pos(xi, yi - 1)) * 0.5
                };

                // Normal is cross product of tangent vectors
                let normal = dy.cross(dx).normalize_or_zero();

                // Ensure normal points upward (positive Y component)
                let normal = if normal.y < 0.0 { -normal } else { normal };

                vertices[idx].normal = normal.to_array();
            }
        }
    }

    /// Get vertex buffer data as bytes
    pub fn vertex_bytes(&self) -> &[u8] {
        bytemuck::cast_slice(&self.vertices)
    }

    /// Get index buffer data as bytes
    pub fn index_bytes(&self) -> &[u8] {
        bytemuck::cast_slice(&self.indices)
    }

    /// Check if mesh is empty
    pub fn is_empty(&self) -> bool {
        self.vertices.is_empty()
    }
}

/// Generate wireframe indices for the surface mesh
pub fn generate_wireframe_indices(x_count: usize, y_count: usize) -> Vec<u32> {
    let mut indices = Vec::new();

    // Horizontal lines
    for yi in 0..y_count {
        for xi in 0..(x_count - 1) {
            let i0 = (yi * x_count + xi) as u32;
            let i1 = (yi * x_count + xi + 1) as u32;
            indices.push(i0);
            indices.push(i1);
        }
    }

    // Vertical lines
    for yi in 0..(y_count - 1) {
        for xi in 0..x_count {
            let i0 = (yi * x_count + xi) as u32;
            let i1 = ((yi + 1) * x_count + xi) as u32;
            indices.push(i0);
            indices.push(i1);
        }
    }

    indices
}

/// Generate a bounding box mesh for the surface (for grid rendering)
pub fn generate_bounding_box_mesh() -> SurfaceMesh {
    let mut vertices = Vec::with_capacity(8);
    let mut indices = Vec::with_capacity(36);

    // Box corners: [-1, 1] x [-0.5, 0.5] x [-1, 1]
    // Matches the normalized surface coordinates
    let min = Vec3::new(-1.0, -0.5, -1.0);
    let max = Vec3::new(1.0, 0.5, 1.0);

    // 8 corners
    let corners = [
        Vec3::new(min.x, min.y, min.z), // 0: 000
        Vec3::new(max.x, min.y, min.z), // 1: 100
        Vec3::new(min.x, max.y, min.z), // 2: 010
        Vec3::new(max.x, max.y, min.z), // 3: 110
        Vec3::new(min.x, min.y, max.z), // 4: 001
        Vec3::new(max.x, min.y, max.z), // 5: 101
        Vec3::new(min.x, max.y, max.z), // 6: 011
        Vec3::new(max.x, max.y, max.z), // 7: 111
    ];

    for pos in corners {
        vertices.push(GpuVertex::new(pos, Vec3::ZERO, 0.0));
    }

    // Indices for 12 triangles (6 faces)
    // We want to see INSIDE faces, so winding order matters.
    // Standard CCW winding for outside faces.
    // If we use FrontFace::Ccw and CullMode::Front, we render back faces.
    // So we generate standard box indices.

    // Front (Z=0)
    indices.extend_from_slice(&[0, 2, 1, 1, 2, 3]);
    // Back (Z=1)
    indices.extend_from_slice(&[5, 7, 4, 4, 7, 6]);
    // Left (X=0)
    indices.extend_from_slice(&[4, 6, 0, 0, 6, 2]);
    // Right (X=1)
    indices.extend_from_slice(&[1, 3, 5, 5, 3, 7]);
    // Bottom (Y=-0.5)
    indices.extend_from_slice(&[4, 0, 5, 5, 0, 1]);
    // Top (Y=0.5)
    indices.extend_from_slice(&[2, 6, 3, 3, 6, 7]);

    SurfaceMesh {
        vertices,
        indices,
        vertex_count: 8,
        index_count: 36,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mesh_generation() {
        let data =
            SurfaceData::from_function((-1.0, 1.0), (-1.0, 1.0), 10, 10, |x, y| x * x + y * y);
        let mesh = SurfaceMesh::from_data(&data);

        assert_eq!(mesh.vertex_count, 100); // 10 * 10
        assert_eq!(mesh.index_count, 9 * 9 * 6); // (10-1) * (10-1) * 6
    }

    #[test]
    fn test_mesh_normals() {
        // Flat surface should have normals pointing up
        let data = SurfaceData::from_function((-1.0, 1.0), (-1.0, 1.0), 5, 5, |_, _| 0.0);
        let mesh = SurfaceMesh::from_data(&data);

        for vertex in &mesh.vertices {
            // Normal should be approximately (0, 1, 0) for flat surface
            assert!(
                vertex.normal[1] > 0.9,
                "Normal Y component should be high for flat surface"
            );
        }
    }

    #[test]
    fn test_wireframe_indices() {
        let indices = generate_wireframe_indices(3, 3);

        // Should have 2 * (3-1) * 3 + 2 * 3 * (3-1) = 12 + 12 = 24 indices
        // Which is 12 line segments
        assert_eq!(indices.len(), 24);
    }

    #[test]
    fn test_empty_mesh() {
        let data = SurfaceData::from_grid(vec![0.0], vec![0.0], vec![vec![0.0]]);
        let mesh = SurfaceMesh::from_data(&data);

        assert!(mesh.is_empty());
    }

    #[test]
    fn test_gpu_vertex_size() {
        // Ensure GpuVertex is properly aligned for GPU
        assert_eq!(std::mem::size_of::<GpuVertex>(), 32);
    }
}
