//! Texture mapping from camera frames to 3D mesh
//!
//! This module implements UV mapping and texture projection to apply
//! camera images as textures on the reconstructed 3D head model.

use crate::camera::Frame;
use crate::error::{ScannerError, ScannerResult};
use crate::mesh::{Mesh, Triangle, Vertex};
use crate::reconstruction::CameraPose;
use image::{ImageBuffer, Rgb, RgbImage};
use nalgebra::{Point2, Point3, Vector2};
use opencv::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// UV coordinates for texture mapping
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct UVCoord {
    pub u: f32,
    pub v: f32,
}

impl UVCoord {
    pub fn new(u: f32, v: f32) -> Self {
        Self { u, v }
    }
}

/// Textured mesh with UV coordinates and texture image
#[derive(Debug, Clone)]
pub struct TexturedMesh {
    /// Base mesh geometry
    pub mesh: Mesh,

    /// UV coordinates for each vertex
    pub uv_coords: Vec<UVCoord>,

    /// Texture image (RGB)
    pub texture: RgbImage,
}

impl TexturedMesh {
    /// Create a new textured mesh
    pub fn new(mesh: Mesh, uv_coords: Vec<UVCoord>, texture: RgbImage) -> ScannerResult<Self> {
        if uv_coords.len() != mesh.vertices().len() {
            return Err(ScannerError::InvalidInput(
                format!(
                    "UV coordinate count ({}) must match vertex count ({})",
                    uv_coords.len(),
                    mesh.vertices().len()
                )
            ));
        }

        Ok(Self {
            mesh,
            uv_coords,
            texture,
        })
    }

    /// Export textured mesh to OBJ format with MTL material
    pub fn export_obj(&self, obj_path: &str, mtl_path: &str, texture_path: &str) -> ScannerResult<()> {
        use std::fs::File;
        use std::io::Write;

        // Save texture image
        self.texture
            .save(texture_path)
            .map_err(|e| {
                ScannerError::Io(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("Failed to save texture to '{}': {}", texture_path, e)
                ))
            })?;

        // Write MTL file
        let mut mtl_file = File::create(mtl_path)?;

        writeln!(mtl_file, "newmtl material0")?;
        writeln!(mtl_file, "Ka 1.0 1.0 1.0")?;
        writeln!(mtl_file, "Kd 1.0 1.0 1.0")?;
        writeln!(mtl_file, "Ks 0.0 0.0 0.0")?;
        writeln!(mtl_file, "map_Kd {}", texture_path)?;

        // Write OBJ file
        let mut obj_file = File::create(obj_path)?;

        writeln!(obj_file, "# Textured head model")?;
        writeln!(obj_file, "mtllib {}", mtl_path)?;
        writeln!(obj_file)?;

        // Write vertices
        for vertex in self.mesh.vertices() {
            writeln!(obj_file, "v {} {} {}", vertex.x, vertex.y, vertex.z)?;
        }
        writeln!(obj_file)?;

        // Write UV coordinates
        for uv in &self.uv_coords {
            writeln!(obj_file, "vt {} {}", uv.u, uv.v)?;
        }
        writeln!(obj_file)?;

        // Write faces with UV indices
        writeln!(obj_file, "usemtl material0")?;
        for triangle in self.mesh.triangles() {
            writeln!(
                obj_file,
                "f {}/{} {}/{} {}/{}",
                triangle.v0 + 1, triangle.v0 + 1,
                triangle.v1 + 1, triangle.v1 + 1,
                triangle.v2 + 1, triangle.v2 + 1
            )?;
        }

        Ok(())
    }
}

/// Texture mapper for projecting camera images onto mesh
pub struct TextureMapper {
    /// Texture resolution (width x height)
    texture_width: u32,
    texture_height: u32,
}

impl TextureMapper {
    /// Create a new texture mapper
    pub fn new(texture_width: u32, texture_height: u32) -> Self {
        Self {
            texture_width,
            texture_height,
        }
    }

    /// Apply texture from a single camera frame
    pub fn apply_single_frame(
        &self,
        mesh: &Mesh,
        frame: &Frame,
        camera_pose: &CameraPose,
    ) -> ScannerResult<TexturedMesh> {
        // Generate UV coordinates by projecting vertices onto camera
        let uv_coords = self.project_vertices_to_uv(mesh, camera_pose)?;

        // Convert frame to RGB image
        let texture = self.frame_to_rgb_image(frame)?;

        TexturedMesh::new(mesh.clone(), uv_coords, texture)
    }

    /// Apply texture from multiple camera frames (better coverage)
    pub fn apply_multi_frame(
        &self,
        mesh: &Mesh,
        frames: &[(Frame, CameraPose)],
    ) -> ScannerResult<TexturedMesh> {
        if frames.is_empty() {
            return Err(ScannerError::InvalidInput(
                "At least one frame required for texturing".to_string()
            ));
        }

        // Create texture atlas
        let mut texture = RgbImage::new(self.texture_width, self.texture_height);

        // Generate UV coordinates using spherical mapping
        let uv_coords = self.generate_spherical_uv(mesh);

        // For each triangle, find the best camera view
        let triangle_cameras = self.select_best_camera_per_triangle(mesh, frames);

        // Project and blend textures
        self.project_multi_view_texture(
            mesh,
            &uv_coords,
            frames,
            &triangle_cameras,
            &mut texture,
        )?;

        TexturedMesh::new(mesh.clone(), uv_coords, texture)
    }

    /// Project mesh vertices to UV coordinates based on camera view
    fn project_vertices_to_uv(
        &self,
        mesh: &Mesh,
        camera_pose: &CameraPose,
    ) -> ScannerResult<Vec<UVCoord>> {
        let mut uv_coords = Vec::new();

        for vertex in mesh.vertices() {
            // Transform vertex to camera space
            let vertex_3d = Point3::new(vertex.x, vertex.y, vertex.z);
            let camera_space = camera_pose.to_camera(&vertex_3d);

            // Project to image plane (normalized [0, 1])
            let u = (camera_space.x / camera_space.z + 1.0) / 2.0;
            let v = (camera_space.y / camera_space.z + 1.0) / 2.0;

            uv_coords.push(UVCoord::new(u, v));
        }

        Ok(uv_coords)
    }

    /// Generate UV coordinates using spherical mapping
    fn generate_spherical_uv(&self, mesh: &Mesh) -> Vec<UVCoord> {
        let mut uv_coords = Vec::new();

        // Find mesh center
        let center = mesh.compute_centroid();

        for vertex in mesh.vertices() {
            // Vector from center to vertex
            let dir = Point3::new(vertex.x, vertex.y, vertex.z) - center;
            let dir_normalized = dir.normalize();

            // Convert to spherical coordinates
            let theta = dir_normalized.z.atan2(dir_normalized.x); // Azimuth
            let phi = (dir_normalized.y).asin(); // Elevation

            // Map to UV [0, 1]
            let u = (theta + std::f32::consts::PI) / (2.0 * std::f32::consts::PI);
            let v = (phi + std::f32::consts::FRAC_PI_2) / std::f32::consts::PI;

            uv_coords.push(UVCoord::new(u, v));
        }

        uv_coords
    }

    /// Select the best camera for each triangle based on viewing angle
    fn select_best_camera_per_triangle(
        &self,
        mesh: &Mesh,
        frames: &[(Frame, CameraPose)],
    ) -> Vec<usize> {
        let mut best_cameras = Vec::new();

        for triangle in mesh.triangles() {
            // Get triangle vertices
            let v0 = &mesh.vertices()[triangle.v0];
            let v1 = &mesh.vertices()[triangle.v1];
            let v2 = &mesh.vertices()[triangle.v2];

            // Compute triangle normal
            let p0 = Point3::new(v0.x, v0.y, v0.z);
            let p1 = Point3::new(v1.x, v1.y, v1.z);
            let p2 = Point3::new(v2.x, v2.y, v2.z);

            let edge1 = p1 - p0;
            let edge2 = p2 - p0;
            let normal = edge1.cross(&edge2).normalize();

            // Triangle centroid
            let centroid = Point3::from((p0.coords + p1.coords + p2.coords) / 3.0);

            // Find camera with best viewing angle
            let mut best_camera = 0;
            let mut best_score = f32::MIN;

            for (idx, (_, pose)) in frames.iter().enumerate() {
                // Vector from triangle to camera
                let to_camera = (pose.position - centroid).normalize();

                // Score based on dot product (prefer cameras facing the triangle)
                let score = normal.dot(&to_camera);

                if score > best_score {
                    best_score = score;
                    best_camera = idx;
                }
            }

            best_cameras.push(best_camera);
        }

        best_cameras
    }

    /// Project multi-view textures onto the mesh
    fn project_multi_view_texture(
        &self,
        mesh: &Mesh,
        uv_coords: &[UVCoord],
        frames: &[(Frame, CameraPose)],
        triangle_cameras: &[usize],
        texture: &mut RgbImage,
    ) -> ScannerResult<()> {
        // For each triangle, sample color from the best camera view
        for (tri_idx, triangle) in mesh.triangles().iter().enumerate() {
            let camera_idx = triangle_cameras[tri_idx];
            let (frame, _pose) = &frames[camera_idx];

            // Get UV coordinates for triangle vertices
            let uv0 = uv_coords[triangle.v0];
            let uv1 = uv_coords[triangle.v1];
            let uv2 = uv_coords[triangle.v2];

            // Rasterize triangle in texture space and sample from frame
            self.rasterize_triangle_texture(
                texture,
                frame,
                &uv0,
                &uv1,
                &uv2,
            )?;
        }

        Ok(())
    }

    /// Rasterize a triangle in texture space
    fn rasterize_triangle_texture(
        &self,
        texture: &mut RgbImage,
        frame: &Frame,
        uv0: &UVCoord,
        uv1: &UVCoord,
        uv2: &UVCoord,
    ) -> ScannerResult<()> {
        // Convert UV to pixel coordinates
        let p0 = (
            (uv0.u * self.texture_width as f32) as i32,
            (uv0.v * self.texture_height as f32) as i32,
        );
        let p1 = (
            (uv1.u * self.texture_width as f32) as i32,
            (uv1.v * self.texture_height as f32) as i32,
        );
        let p2 = (
            (uv2.u * self.texture_width as f32) as i32,
            (uv2.v * self.texture_height as f32) as i32,
        );

        // Bounding box
        let min_x = p0.0.min(p1.0).min(p2.0).max(0);
        let max_x = p0.0.max(p1.0).max(p2.0).min(self.texture_width as i32 - 1);
        let min_y = p0.1.min(p1.1).min(p2.1).max(0);
        let max_y = p0.1.max(p1.1).max(p2.1).min(self.texture_height as i32 - 1);

        // Rasterize
        for y in min_y..=max_y {
            for x in min_x..=max_x {
                // Check if point is inside triangle using barycentric coordinates
                if self.point_in_triangle(x, y, p0, p1, p2) {
                    // Sample color from frame (simplified - just use center color)
                    let color = self.sample_frame_color(frame, x as u32, y as u32)?;
                    texture.put_pixel(x as u32, y as u32, color);
                }
            }
        }

        Ok(())
    }

    /// Check if point is inside triangle
    fn point_in_triangle(
        &self,
        px: i32,
        py: i32,
        p0: (i32, i32),
        p1: (i32, i32),
        p2: (i32, i32),
    ) -> bool {
        let sign = |p1: (i32, i32), p2: (i32, i32), p3: (i32, i32)| -> i32 {
            (p1.0 - p3.0) * (p2.1 - p3.1) - (p2.0 - p3.0) * (p1.1 - p3.1)
        };

        let d1 = sign((px, py), p0, p1);
        let d2 = sign((px, py), p1, p2);
        let d3 = sign((px, py), p2, p0);

        let has_neg = (d1 < 0) || (d2 < 0) || (d3 < 0);
        let has_pos = (d1 > 0) || (d2 > 0) || (d3 > 0);

        !(has_neg && has_pos)
    }

    /// Sample color from frame
    fn sample_frame_color(&self, frame: &Frame, x: u32, y: u32) -> ScannerResult<Rgb<u8>> {
        // Convert frame to RGB if needed
        let rgb = frame.to_rgb()?;

        // Sample pixel (with bounds checking)
        let pixel = rgb.at_2d::<opencv::core::Vec3b>(
            (y % frame.height) as i32,
            (x % frame.width) as i32,
        )?;

        Ok(Rgb([pixel[2], pixel[1], pixel[0]])) // BGR to RGB
    }

    /// Convert frame to RGB image
    fn frame_to_rgb_image(&self, frame: &Frame) -> ScannerResult<RgbImage> {
        let rgb_mat = frame.to_rgb()?;

        let mut img = RgbImage::new(frame.width, frame.height);

        for y in 0..frame.height {
            for x in 0..frame.width {
                let pixel = rgb_mat.at_2d::<opencv::core::Vec3b>(y as i32, x as i32)?;
                img.put_pixel(x, y, Rgb([pixel[2], pixel[1], pixel[0]]));
            }
        }

        Ok(img)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_uv_coord_creation() {
        let uv = UVCoord::new(0.5, 0.75);
        assert_eq!(uv.u, 0.5);
        assert_eq!(uv.v, 0.75);
    }

    #[test]
    fn test_texture_mapper_creation() {
        let mapper = TextureMapper::new(1024, 1024);
        assert_eq!(mapper.texture_width, 1024);
        assert_eq!(mapper.texture_height, 1024);
    }

    #[test]
    fn test_point_in_triangle() {
        let mapper = TextureMapper::new(256, 256);

        // Triangle: (0, 0), (10, 0), (5, 10)
        assert!(mapper.point_in_triangle(5, 5, (0, 0), (10, 0), (5, 10)));
        assert!(!mapper.point_in_triangle(0, 10, (0, 0), (10, 0), (5, 10)));
    }
}
