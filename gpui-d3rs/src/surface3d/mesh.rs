//! Mesh generation for 3D surfaces

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
    pub fn from_data(data: &SurfaceData) -> Self {
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

                // Map normalized z [0,1] to height [-0.5, 0.5] for better visualization
                let height = nz - 0.5;

                let position = Vec3::new(nx, height, ny);
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
