//! Mesh data structures and operations

use crate::convexhull::ConvexHull3D;
use crate::error::{ScannerError, ScannerResult};
use nalgebra::{Point3, Vector3};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::path::Path;

/// Validate and sanitize file path to prevent path traversal attacks
fn validate_export_path(path: &str) -> ScannerResult<()> {
    let path_obj = Path::new(path);

    // Check for path traversal attempts
    if path.contains("..") {
        return Err(ScannerError::InvalidConfig(
            "Path traversal detected in export path".to_string(),
        ));
    }

    // Check for absolute paths in untrusted contexts (optional, depends on use case)
    // For now, we allow both relative and absolute paths

    // Ensure the path is valid UTF-8
    if path_obj.to_str().is_none() {
        return Err(ScannerError::InvalidConfig(
            "Invalid UTF-8 in file path".to_string(),
        ));
    }

    // Check parent directory exists (if path has a parent)
    if let Some(parent) = path_obj.parent() {
        if !parent.as_os_str().is_empty() && !parent.exists() {
            return Err(ScannerError::InvalidConfig(format!(
                "Parent directory does not exist: {:?}",
                parent
            )));
        }
    }

    Ok(())
}

/// A 3D vertex with position, normal, and optional texture coordinates
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vertex {
    /// 3D position
    pub position: Point3<f32>,

    /// Surface normal (unit vector)
    pub normal: Option<Vector3<f32>>,

    /// Texture coordinates (u, v)
    pub texcoord: Option<[f32; 2]>,

    /// Vertex color (RGB, 0-255)
    pub color: Option<[u8; 3]>,
}

impl Vertex {
    /// Create a new vertex with position only
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self {
            position: Point3::new(x, y, z),
            normal: None,
            texcoord: None,
            color: None,
        }
    }

    /// Create a vertex from a Point3
    pub fn from_point(point: Point3<f32>) -> Self {
        Self {
            position: point,
            normal: None,
            texcoord: None,
            color: None,
        }
    }

    /// Set the normal vector
    pub fn with_normal(mut self, normal: Vector3<f32>) -> Self {
        self.normal = Some(normal.normalize());
        self
    }

    /// Set the texture coordinates
    pub fn with_texcoord(mut self, u: f32, v: f32) -> Self {
        self.texcoord = Some([u, v]);
        self
    }

    /// Set the vertex color
    pub fn with_color(mut self, color: [u8; 3]) -> Self {
        self.color = Some(color);
        self
    }
}

/// A triangular face defined by three vertex indices
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Triangle {
    /// Indices of the three vertices (counter-clockwise winding)
    pub indices: [usize; 3],
}

impl Triangle {
    /// Create a new triangle from three vertex indices
    pub fn new(i0: usize, i1: usize, i2: usize) -> Self {
        Self {
            indices: [i0, i1, i2],
        }
    }

    /// Compute the normal of this triangle given the vertices
    pub fn compute_normal(&self, vertices: &[Vertex]) -> Vector3<f32> {
        let v0 = vertices[self.indices[0]].position;
        let v1 = vertices[self.indices[1]].position;
        let v2 = vertices[self.indices[2]].position;

        let edge1 = v1 - v0;
        let edge2 = v2 - v0;

        edge1.cross(&edge2).normalize()
    }

    /// Compute the area of this triangle
    pub fn area(&self, vertices: &[Vertex]) -> f32 {
        let v0 = vertices[self.indices[0]].position;
        let v1 = vertices[self.indices[1]].position;
        let v2 = vertices[self.indices[2]].position;

        let edge1 = v1 - v0;
        let edge2 = v2 - v0;

        edge1.cross(&edge2).magnitude() * 0.5
    }
}

/// A triangulated 3D mesh
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mesh {
    /// All vertices in the mesh
    vertices: Vec<Vertex>,

    /// All triangles in the mesh
    triangles: Vec<Triangle>,
}

impl Mesh {
    /// Create a new empty mesh
    pub fn new() -> Self {
        Self {
            vertices: Vec::new(),
            triangles: Vec::new(),
        }
    }

    /// Create a mesh from vertices and triangles
    pub fn from_parts(vertices: Vec<Vertex>, triangles: Vec<Triangle>) -> Self {
        Self {
            vertices,
            triangles,
        }
    }

    /// Create a mesh from a convex hull
    pub fn from_convex_hull(hull: &ConvexHull3D) -> Self {
        let vertices: Vec<Vertex> = hull
            .vertices()
            .iter()
            .map(|&p| Vertex::from_point(p))
            .collect();

        let triangles: Vec<Triangle> = hull
            .faces()
            .iter()
            .map(|&[i0, i1, i2]| Triangle::new(i0, i1, i2))
            .collect();

        let mut mesh = Self::from_parts(vertices, triangles);
        mesh.compute_normals();
        mesh
    }

    /// Get the vertices
    pub fn vertices(&self) -> &[Vertex] {
        &self.vertices
    }

    /// Get the triangles
    pub fn triangles(&self) -> &[Triangle] {
        &self.triangles
    }

    /// Get the number of vertices
    pub fn vertex_count(&self) -> usize {
        self.vertices.len()
    }

    /// Get the number of triangles
    pub fn triangle_count(&self) -> usize {
        self.triangles.len()
    }

    /// Compute smooth vertex normals by averaging face normals
    pub fn compute_normals(&mut self) {
        // Initialize all normals to zero
        let mut normals = vec![Vector3::zeros(); self.vertices.len()];

        // Accumulate face normals
        for triangle in &self.triangles {
            let normal = triangle.compute_normal(&self.vertices);

            for &idx in &triangle.indices {
                normals[idx] += normal;
            }
        }

        // Normalize and assign to vertices
        for (vertex, normal) in self.vertices.iter_mut().zip(normals.iter()) {
            if normal.magnitude() > 1e-6 {
                vertex.normal = Some(normal.normalize());
            }
        }
    }

    /// Compute the total surface area of the mesh
    pub fn surface_area(&self) -> f32 {
        self.triangles
            .iter()
            .map(|tri| tri.area(&self.vertices))
            .sum()
    }

    /// Compute the bounding box of the mesh
    pub fn bounding_box(&self) -> Option<(Point3<f32>, Point3<f32>)> {
        if self.vertices.is_empty() {
            return None;
        }

        let mut min = self.vertices[0].position;
        let mut max = self.vertices[0].position;

        for vertex in &self.vertices {
            min.x = min.x.min(vertex.position.x);
            min.y = min.y.min(vertex.position.y);
            min.z = min.z.min(vertex.position.z);
            max.x = max.x.max(vertex.position.x);
            max.y = max.y.max(vertex.position.y);
            max.z = max.z.max(vertex.position.z);
        }

        Some((min, max))
    }

    /// Export mesh to Wavefront OBJ format
    pub fn export_obj(&self, path: &str) -> ScannerResult<()> {
        // Validate path for security
        validate_export_path(path)?;

        let mut file = File::create(path)
            .map_err(|e| ScannerError::Io(e))?;

        writeln!(file, "# Head Scanner Mesh")?;
        writeln!(file, "# Vertices: {}", self.vertices.len())?;
        writeln!(file, "# Faces: {}", self.triangles.len())?;
        writeln!(file)?;

        // Write vertices
        for vertex in &self.vertices {
            writeln!(
                file,
                "v {} {} {}",
                vertex.position.x, vertex.position.y, vertex.position.z
            )?;
        }

        // Write normals
        if self.vertices.iter().any(|v| v.normal.is_some()) {
            writeln!(file)?;
            for vertex in &self.vertices {
                if let Some(normal) = vertex.normal {
                    writeln!(file, "vn {} {} {}", normal.x, normal.y, normal.z)?;
                }
            }
        }

        // Write texture coordinates
        if self.vertices.iter().any(|v| v.texcoord.is_some()) {
            writeln!(file)?;
            for vertex in &self.vertices {
                if let Some([u, v]) = vertex.texcoord {
                    writeln!(file, "vt {} {}", u, v)?;
                }
            }
        }

        // Write faces (OBJ uses 1-based indexing)
        writeln!(file)?;
        let has_normals = self.vertices.iter().any(|v| v.normal.is_some());
        let has_texcoords = self.vertices.iter().any(|v| v.texcoord.is_some());

        for triangle in &self.triangles {
            match (has_texcoords, has_normals) {
                (true, true) => writeln!(
                    file,
                    "f {}/{}/{} {}/{}/{} {}/{}/{}",
                    triangle.indices[0] + 1,
                    triangle.indices[0] + 1,
                    triangle.indices[0] + 1,
                    triangle.indices[1] + 1,
                    triangle.indices[1] + 1,
                    triangle.indices[1] + 1,
                    triangle.indices[2] + 1,
                    triangle.indices[2] + 1,
                    triangle.indices[2] + 1,
                )?,
                (false, true) => writeln!(
                    file,
                    "f {}//{} {}//{} {}//{}",
                    triangle.indices[0] + 1,
                    triangle.indices[0] + 1,
                    triangle.indices[1] + 1,
                    triangle.indices[1] + 1,
                    triangle.indices[2] + 1,
                    triangle.indices[2] + 1,
                )?,
                _ => writeln!(
                    file,
                    "f {} {} {}",
                    triangle.indices[0] + 1,
                    triangle.indices[1] + 1,
                    triangle.indices[2] + 1,
                )?,
            }
        }

        Ok(())
    }

    /// Export mesh to PLY format
    pub fn export_ply(&self, path: &str) -> ScannerResult<()> {
        // Validate path for security
        validate_export_path(path)?;

        let mut file = File::create(path)?;

        let has_normals = self.vertices.iter().any(|v| v.normal.is_some());
        let has_colors = self.vertices.iter().any(|v| v.color.is_some());

        // Write PLY header
        writeln!(file, "ply")?;
        writeln!(file, "format ascii 1.0")?;
        writeln!(file, "element vertex {}", self.vertices.len())?;
        writeln!(file, "property float x")?;
        writeln!(file, "property float y")?;
        writeln!(file, "property float z")?;

        if has_normals {
            writeln!(file, "property float nx")?;
            writeln!(file, "property float ny")?;
            writeln!(file, "property float nz")?;
        }

        if has_colors {
            writeln!(file, "property uchar red")?;
            writeln!(file, "property uchar green")?;
            writeln!(file, "property uchar blue")?;
        }

        writeln!(file, "element face {}", self.triangles.len())?;
        writeln!(file, "property list uchar int vertex_indices")?;
        writeln!(file, "end_header")?;

        // Write vertices
        for vertex in &self.vertices {
            write!(
                file,
                "{} {} {}",
                vertex.position.x, vertex.position.y, vertex.position.z
            )?;

            if has_normals {
                if let Some(normal) = vertex.normal {
                    write!(file, " {} {} {}", normal.x, normal.y, normal.z)?;
                } else {
                    write!(file, " 0 0 0")?;
                }
            }

            if has_colors {
                if let Some(color) = vertex.color {
                    write!(file, " {} {} {}", color[0], color[1], color[2])?;
                } else {
                    write!(file, " 128 128 128")?;
                }
            }

            writeln!(file)?;
        }

        // Write faces
        for triangle in &self.triangles {
            writeln!(
                file,
                "3 {} {} {}",
                triangle.indices[0], triangle.indices[1], triangle.indices[2]
            )?;
        }

        Ok(())
    }

    /// Export mesh to STL format (binary)
    pub fn export_stl(&self, path: &str) -> ScannerResult<()> {
        // Validate path for security
        validate_export_path(path)?;

        use byteorder::{LittleEndian, WriteBytesExt};

        let mut file = File::create(path)?;

        // Write 80-byte header
        let header = b"Head Scanner STL Export                                                         ";
        file.write_all(header)?;

        // Write number of triangles
        file.write_u32::<LittleEndian>(self.triangles.len() as u32)?;

        // Write each triangle
        for triangle in &self.triangles {
            let normal = triangle.compute_normal(&self.vertices);

            // Normal vector
            file.write_f32::<LittleEndian>(normal.x)?;
            file.write_f32::<LittleEndian>(normal.y)?;
            file.write_f32::<LittleEndian>(normal.z)?;

            // Vertices
            for &idx in &triangle.indices {
                let pos = self.vertices[idx].position;
                file.write_f32::<LittleEndian>(pos.x)?;
                file.write_f32::<LittleEndian>(pos.y)?;
                file.write_f32::<LittleEndian>(pos.z)?;
            }

            // Attribute byte count (unused)
            file.write_u16::<LittleEndian>(0)?;
        }

        Ok(())
    }

    /// Generic export method that chooses format based on file extension
    pub fn export(&self, path: &str) -> ScannerResult<()> {
        let path_lower = path.to_lowercase();

        if path_lower.ends_with(".obj") {
            self.export_obj(path)
        } else if path_lower.ends_with(".ply") {
            self.export_ply(path)
        } else if path_lower.ends_with(".stl") {
            self.export_stl(path)
        } else {
            Err(ScannerError::InvalidConfig(format!(
                "Unsupported mesh format: {}. Supported formats: OBJ, PLY, STL",
                path
            )))
        }
    }
}

impl Default for Mesh {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vertex_creation() {
        let v = Vertex::new(1.0, 2.0, 3.0);
        assert_eq!(v.position.x, 1.0);
        assert_eq!(v.position.y, 2.0);
        assert_eq!(v.position.z, 3.0);
        assert!(v.normal.is_none());
        assert!(v.texcoord.is_none());
    }

    #[test]
    fn test_triangle_normal() {
        let vertices = vec![
            Vertex::new(0.0, 0.0, 0.0),
            Vertex::new(1.0, 0.0, 0.0),
            Vertex::new(0.0, 1.0, 0.0),
        ];

        let tri = Triangle::new(0, 1, 2);
        let normal = tri.compute_normal(&vertices);

        // Normal should point in +Z direction
        assert!((normal.z - 1.0).abs() < 1e-6);
        assert!(normal.x.abs() < 1e-6);
        assert!(normal.y.abs() < 1e-6);
    }

    #[test]
    fn test_mesh_basic() {
        let vertices = vec![
            Vertex::new(0.0, 0.0, 0.0),
            Vertex::new(1.0, 0.0, 0.0),
            Vertex::new(0.0, 1.0, 0.0),
            Vertex::new(0.0, 0.0, 1.0),
        ];

        let triangles = vec![
            Triangle::new(0, 1, 2),
            Triangle::new(0, 1, 3),
            Triangle::new(0, 2, 3),
            Triangle::new(1, 2, 3),
        ];

        let mesh = Mesh::from_parts(vertices, triangles);

        assert_eq!(mesh.vertex_count(), 4);
        assert_eq!(mesh.triangle_count(), 4);

        let (min, max) = mesh.bounding_box().unwrap();
        assert_eq!(min, Point3::new(0.0, 0.0, 0.0));
        assert_eq!(max, Point3::new(1.0, 1.0, 1.0));
    }
}
