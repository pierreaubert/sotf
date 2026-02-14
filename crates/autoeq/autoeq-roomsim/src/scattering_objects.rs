//! Scattering Objects for BEM Room Acoustics
//!
//! This module provides support for scattering objects inside rooms:
//! - Boxes (furniture, equipment)
//! - Spheres (decorative objects, approximations)
//! - Cylinders (columns, stands)
//!
//! Each object is represented as a surface mesh that can be merged with
//! the room boundary mesh for BEM simulation.

use math_audio_bem::room_acoustics::{Point3D as BemPoint3D, RoomMesh, SurfaceElement};
use serde::{Deserialize, Serialize};
use std::f64::consts::PI;

use crate::{Point3DConfig, WallMaterial, WallMaterialConfig};

// ============================================================================
// Scattering Object Types
// ============================================================================

/// Configuration for a box-shaped scattering object
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoxObject {
    /// Name/identifier for the object
    #[serde(default)]
    pub name: String,
    /// Center position of the box
    pub center: Point3DConfig,
    /// Dimensions [width (x), depth (y), height (z)]
    pub dimensions: [f64; 3],
    /// Material for the box surface
    #[serde(default)]
    pub material: WallMaterialConfig,
}

/// Configuration for a sphere-shaped scattering object
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SphereObject {
    /// Name/identifier for the object
    #[serde(default)]
    pub name: String,
    /// Center position of the sphere
    pub center: Point3DConfig,
    /// Radius of the sphere
    pub radius: f64,
    /// Material for the sphere surface
    #[serde(default)]
    pub material: WallMaterialConfig,
    /// Number of subdivisions for mesh generation (default: 2)
    #[serde(default = "default_subdivisions")]
    pub subdivisions: usize,
}

fn default_subdivisions() -> usize {
    2
}

/// Configuration for a cylinder-shaped scattering object
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CylinderObject {
    /// Name/identifier for the object
    #[serde(default)]
    pub name: String,
    /// Center of the cylinder base
    pub base_center: Point3DConfig,
    /// Radius of the cylinder
    pub radius: f64,
    /// Height of the cylinder
    pub height: f64,
    /// Material for the cylinder surface
    #[serde(default)]
    pub material: WallMaterialConfig,
    /// Number of radial segments (default: 16)
    #[serde(default = "default_radial_segments")]
    pub radial_segments: usize,
    /// Include top and bottom caps
    #[serde(default = "default_include_caps")]
    pub include_caps: bool,
}

fn default_radial_segments() -> usize {
    16
}

fn default_include_caps() -> bool {
    true
}

/// Scattering object enum for JSON configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ScatteringObjectConfig {
    /// Box-shaped object (furniture, equipment)
    #[serde(rename = "box")]
    Box(BoxObject),
    /// Spherical object
    #[serde(rename = "sphere")]
    Sphere(SphereObject),
    /// Cylindrical object (columns, stands)
    #[serde(rename = "cylinder")]
    Cylinder(CylinderObject),
}

impl ScatteringObjectConfig {
    /// Get the material for this object
    pub fn material(&self) -> WallMaterial {
        match self {
            ScatteringObjectConfig::Box(b) => b.material.to_material(),
            ScatteringObjectConfig::Sphere(s) => s.material.to_material(),
            ScatteringObjectConfig::Cylinder(c) => c.material.to_material(),
        }
    }

    /// Get the name/identifier for this object
    pub fn name(&self) -> &str {
        match self {
            ScatteringObjectConfig::Box(b) => &b.name,
            ScatteringObjectConfig::Sphere(s) => &s.name,
            ScatteringObjectConfig::Cylinder(c) => &c.name,
        }
    }

    /// Get the center position of the object
    pub fn center(&self) -> Point3DConfig {
        match self {
            ScatteringObjectConfig::Box(b) => b.center,
            ScatteringObjectConfig::Sphere(s) => s.center,
            ScatteringObjectConfig::Cylinder(c) => Point3DConfig {
                x: c.base_center.x,
                y: c.base_center.y,
                z: c.base_center.z + c.height / 2.0,
            },
        }
    }
}

// ============================================================================
// Object Mesh Type
// ============================================================================

/// Mesh representation for a scattering object
pub struct ObjectMesh {
    pub nodes: Vec<BemPoint3D>,
    pub elements: Vec<SurfaceElement>,
}

impl ObjectMesh {
    /// Merge this object mesh with a room mesh
    pub fn merge_into(self, room_mesh: &mut RoomMesh) {
        let node_offset = room_mesh.nodes.len();

        // Add nodes
        room_mesh.nodes.extend(self.nodes);

        // Add elements with adjusted node indices
        for elem in self.elements {
            let new_nodes: Vec<usize> = elem.nodes.iter().map(|&i| i + node_offset).collect();
            room_mesh.elements.push(SurfaceElement { nodes: new_nodes });
        }
    }
}

// ============================================================================
// Mesh Generation for Scattering Objects
// ============================================================================

/// Generate mesh for a box object
///
/// Creates 6 faces (12 triangles) with outward-pointing normals
pub fn generate_box_mesh(obj: &BoxObject) -> ObjectMesh {
    let cx = obj.center.x;
    let cy = obj.center.y;
    let cz = obj.center.z;
    let hw = obj.dimensions[0] / 2.0; // half width
    let hd = obj.dimensions[1] / 2.0; // half depth
    let hh = obj.dimensions[2] / 2.0; // half height

    // 8 corner vertices
    let nodes = vec![
        BemPoint3D::new(cx - hw, cy - hd, cz - hh), // 0: front-left-bottom
        BemPoint3D::new(cx + hw, cy - hd, cz - hh), // 1: front-right-bottom
        BemPoint3D::new(cx + hw, cy + hd, cz - hh), // 2: back-right-bottom
        BemPoint3D::new(cx - hw, cy + hd, cz - hh), // 3: back-left-bottom
        BemPoint3D::new(cx - hw, cy - hd, cz + hh), // 4: front-left-top
        BemPoint3D::new(cx + hw, cy - hd, cz + hh), // 5: front-right-top
        BemPoint3D::new(cx + hw, cy + hd, cz + hh), // 6: back-right-top
        BemPoint3D::new(cx - hw, cy + hd, cz + hh), // 7: back-left-top
    ];

    // 6 faces, each as 2 triangles (12 total)
    // Normals point outward from the object (into the room)
    let elements = vec![
        // Bottom face (z = cz - hh), normal pointing down (-z)
        SurfaceElement {
            nodes: vec![0, 3, 2],
        },
        SurfaceElement {
            nodes: vec![0, 2, 1],
        },
        // Top face (z = cz + hh), normal pointing up (+z)
        SurfaceElement {
            nodes: vec![4, 5, 6],
        },
        SurfaceElement {
            nodes: vec![4, 6, 7],
        },
        // Front face (y = cy - hd), normal pointing forward (-y)
        SurfaceElement {
            nodes: vec![0, 1, 5],
        },
        SurfaceElement {
            nodes: vec![0, 5, 4],
        },
        // Back face (y = cy + hd), normal pointing backward (+y)
        SurfaceElement {
            nodes: vec![2, 3, 7],
        },
        SurfaceElement {
            nodes: vec![2, 7, 6],
        },
        // Left face (x = cx - hw), normal pointing left (-x)
        SurfaceElement {
            nodes: vec![3, 0, 4],
        },
        SurfaceElement {
            nodes: vec![3, 4, 7],
        },
        // Right face (x = cx + hw), normal pointing right (+x)
        SurfaceElement {
            nodes: vec![1, 2, 6],
        },
        SurfaceElement {
            nodes: vec![1, 6, 5],
        },
    ];

    ObjectMesh { nodes, elements }
}

/// Generate mesh for a sphere object using icosphere subdivision
///
/// Creates a triangulated sphere mesh with outward-pointing normals
pub fn generate_sphere_mesh(obj: &SphereObject) -> ObjectMesh {
    let cx = obj.center.x;
    let cy = obj.center.y;
    let cz = obj.center.z;
    let r = obj.radius;
    let subdivisions = obj.subdivisions;

    // Start with an icosahedron and subdivide
    let phi = (1.0 + 5.0_f64.sqrt()) / 2.0; // Golden ratio

    // Icosahedron vertices (normalized to unit sphere, then scaled)
    let mut nodes: Vec<BemPoint3D> = vec![
        BemPoint3D::new(-1.0, phi, 0.0),
        BemPoint3D::new(1.0, phi, 0.0),
        BemPoint3D::new(-1.0, -phi, 0.0),
        BemPoint3D::new(1.0, -phi, 0.0),
        BemPoint3D::new(0.0, -1.0, phi),
        BemPoint3D::new(0.0, 1.0, phi),
        BemPoint3D::new(0.0, -1.0, -phi),
        BemPoint3D::new(0.0, 1.0, -phi),
        BemPoint3D::new(phi, 0.0, -1.0),
        BemPoint3D::new(phi, 0.0, 1.0),
        BemPoint3D::new(-phi, 0.0, -1.0),
        BemPoint3D::new(-phi, 0.0, 1.0),
    ];

    // Normalize and scale to sphere
    for node in &mut nodes {
        let len = (node.x * node.x + node.y * node.y + node.z * node.z).sqrt();
        node.x = cx + r * node.x / len;
        node.y = cy + r * node.y / len;
        node.z = cz + r * node.z / len;
    }

    // Icosahedron faces (20 triangles)
    let mut faces: Vec<[usize; 3]> = vec![
        [0, 11, 5],
        [0, 5, 1],
        [0, 1, 7],
        [0, 7, 10],
        [0, 10, 11],
        [1, 5, 9],
        [5, 11, 4],
        [11, 10, 2],
        [10, 7, 6],
        [7, 1, 8],
        [3, 9, 4],
        [3, 4, 2],
        [3, 2, 6],
        [3, 6, 8],
        [3, 8, 9],
        [4, 9, 5],
        [2, 4, 11],
        [6, 2, 10],
        [8, 6, 7],
        [9, 8, 1],
    ];

    // Subdivide each face
    for _ in 0..subdivisions {
        let mut new_faces = Vec::new();
        let mut midpoint_cache: std::collections::HashMap<(usize, usize), usize> =
            std::collections::HashMap::new();

        for face in &faces {
            let v0 = face[0];
            let v1 = face[1];
            let v2 = face[2];

            // Get or create midpoints
            let m01 =
                get_or_create_midpoint(v0, v1, &mut nodes, &mut midpoint_cache, cx, cy, cz, r);
            let m12 =
                get_or_create_midpoint(v1, v2, &mut nodes, &mut midpoint_cache, cx, cy, cz, r);
            let m20 =
                get_or_create_midpoint(v2, v0, &mut nodes, &mut midpoint_cache, cx, cy, cz, r);

            // Create 4 new triangles
            new_faces.push([v0, m01, m20]);
            new_faces.push([v1, m12, m01]);
            new_faces.push([v2, m20, m12]);
            new_faces.push([m01, m12, m20]);
        }

        faces = new_faces;
    }

    // Create elements
    let elements: Vec<SurfaceElement> = faces
        .iter()
        .map(|face| SurfaceElement {
            nodes: vec![face[0], face[1], face[2]],
        })
        .collect();

    ObjectMesh { nodes, elements }
}

/// Helper function to get or create midpoint vertex for sphere subdivision
#[allow(clippy::too_many_arguments)]
fn get_or_create_midpoint(
    v0: usize,
    v1: usize,
    nodes: &mut Vec<BemPoint3D>,
    cache: &mut std::collections::HashMap<(usize, usize), usize>,
    cx: f64,
    cy: f64,
    cz: f64,
    r: f64,
) -> usize {
    let key = if v0 < v1 { (v0, v1) } else { (v1, v0) };

    if let Some(&idx) = cache.get(&key) {
        return idx;
    }

    let p0 = &nodes[v0];
    let p1 = &nodes[v1];

    // Midpoint
    let mx = (p0.x + p1.x) / 2.0;
    let my = (p0.y + p1.y) / 2.0;
    let mz = (p0.z + p1.z) / 2.0;

    // Project to sphere surface
    let dx = mx - cx;
    let dy = my - cy;
    let dz = mz - cz;
    let len = (dx * dx + dy * dy + dz * dz).sqrt();

    let new_node = BemPoint3D::new(cx + r * dx / len, cy + r * dy / len, cz + r * dz / len);

    let idx = nodes.len();
    nodes.push(new_node);
    cache.insert(key, idx);

    idx
}

/// Generate mesh for a cylinder object
///
/// Creates a cylindrical mesh with optional top and bottom caps
pub fn generate_cylinder_mesh(obj: &CylinderObject) -> ObjectMesh {
    let bx = obj.base_center.x;
    let by = obj.base_center.y;
    let bz = obj.base_center.z;
    let r = obj.radius;
    let h = obj.height;
    let n = obj.radial_segments;

    let mut nodes = Vec::new();
    let mut elements = Vec::new();

    // Generate bottom ring vertices
    for i in 0..n {
        let angle = 2.0 * PI * i as f64 / n as f64;
        nodes.push(BemPoint3D::new(
            bx + r * angle.cos(),
            by + r * angle.sin(),
            bz,
        ));
    }

    // Generate top ring vertices
    for i in 0..n {
        let angle = 2.0 * PI * i as f64 / n as f64;
        nodes.push(BemPoint3D::new(
            bx + r * angle.cos(),
            by + r * angle.sin(),
            bz + h,
        ));
    }

    // Generate side faces (quads split into 2 triangles each)
    for i in 0..n {
        let i_next = (i + 1) % n;
        let b0 = i;
        let b1 = i_next;
        let t0 = n + i;
        let t1 = n + i_next;

        // Two triangles per quad, normals pointing outward
        elements.push(SurfaceElement {
            nodes: vec![b0, b1, t1],
        });
        elements.push(SurfaceElement {
            nodes: vec![b0, t1, t0],
        });
    }

    // Add caps if requested
    if obj.include_caps {
        // Add center points for caps
        let bottom_center_idx = nodes.len();
        nodes.push(BemPoint3D::new(bx, by, bz));
        let top_center_idx = nodes.len();
        nodes.push(BemPoint3D::new(bx, by, bz + h));

        // Bottom cap (triangles with normal pointing down)
        for i in 0..n {
            let i_next = (i + 1) % n;
            elements.push(SurfaceElement {
                nodes: vec![bottom_center_idx, i_next, i],
            });
        }

        // Top cap (triangles with normal pointing up)
        for i in 0..n {
            let i_next = (i + 1) % n;
            elements.push(SurfaceElement {
                nodes: vec![top_center_idx, n + i, n + i_next],
            });
        }
    }

    ObjectMesh { nodes, elements }
}

/// Generate mesh for a scattering object configuration
pub fn generate_object_mesh(config: &ScatteringObjectConfig) -> ObjectMesh {
    match config {
        ScatteringObjectConfig::Box(obj) => generate_box_mesh(obj),
        ScatteringObjectConfig::Sphere(obj) => generate_sphere_mesh(obj),
        ScatteringObjectConfig::Cylinder(obj) => generate_cylinder_mesh(obj),
    }
}

/// Generate meshes for all scattering objects and merge into room mesh
pub fn add_scattering_objects_to_mesh(
    room_mesh: &mut RoomMesh,
    objects: &[ScatteringObjectConfig],
) {
    for obj in objects {
        let obj_mesh = generate_object_mesh(obj);
        obj_mesh.merge_into(room_mesh);
    }
}

/// Estimate the number of elements for scattering objects
#[allow(dead_code)]
pub fn estimate_scattering_element_count(objects: &[ScatteringObjectConfig]) -> usize {
    objects
        .iter()
        .map(|obj| match obj {
            ScatteringObjectConfig::Box(_) => 12, // 6 faces * 2 triangles
            ScatteringObjectConfig::Sphere(s) => {
                // Icosahedron: 20 faces, each subdivision multiplies by 4
                20 * 4_usize.pow(s.subdivisions as u32)
            }
            ScatteringObjectConfig::Cylinder(c) => {
                let side_elements = 2 * c.radial_segments;
                let cap_elements = if c.include_caps {
                    2 * c.radial_segments
                } else {
                    0
                };
                side_elements + cap_elements
            }
        })
        .sum()
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_box_mesh_generation() {
        let box_obj = BoxObject {
            name: "Test Box".to_string(),
            center: Point3DConfig {
                x: 1.0,
                y: 1.0,
                z: 0.5,
            },
            dimensions: [1.0, 1.0, 1.0],
            material: WallMaterialConfig::default(),
        };

        let mesh = generate_box_mesh(&box_obj);

        // Box should have 8 vertices
        assert_eq!(mesh.nodes.len(), 8);

        // Box should have 12 triangles (2 per face, 6 faces)
        assert_eq!(mesh.elements.len(), 12);

        // Check that all nodes are within expected bounds
        for node in &mesh.nodes {
            assert!((node.x - 1.0).abs() <= 0.5 + 1e-10);
            assert!((node.y - 1.0).abs() <= 0.5 + 1e-10);
            assert!((node.z - 0.5).abs() <= 0.5 + 1e-10);
        }
    }

    #[test]
    fn test_sphere_mesh_generation() {
        let sphere_obj = SphereObject {
            name: "Test Sphere".to_string(),
            center: Point3DConfig {
                x: 2.0,
                y: 2.0,
                z: 1.0,
            },
            radius: 0.5,
            material: WallMaterialConfig::default(),
            subdivisions: 1,
        };

        let mesh = generate_sphere_mesh(&sphere_obj);

        // Check that we have nodes and elements
        assert!(mesh.nodes.len() > 12); // More than initial icosahedron
        assert!(mesh.elements.len() > 20); // More than initial 20 faces

        // Check that all nodes are approximately on the sphere surface
        let cx = 2.0;
        let cy = 2.0;
        let cz = 1.0;
        let r = 0.5;

        for node in &mesh.nodes {
            let dist =
                ((node.x - cx).powi(2) + (node.y - cy).powi(2) + (node.z - cz).powi(2)).sqrt();
            assert!((dist - r).abs() < 1e-10, "Node not on sphere surface");
        }
    }

    #[test]
    fn test_cylinder_mesh_generation() {
        let cyl_obj = CylinderObject {
            name: "Test Cylinder".to_string(),
            base_center: Point3DConfig {
                x: 3.0,
                y: 3.0,
                z: 0.0,
            },
            radius: 0.3,
            height: 1.5,
            material: WallMaterialConfig::default(),
            radial_segments: 8,
            include_caps: true,
        };

        let mesh = generate_cylinder_mesh(&cyl_obj);

        // Should have 2*n ring vertices + 2 center vertices (for caps)
        assert_eq!(mesh.nodes.len(), 2 * 8 + 2);

        // Should have 2*n side triangles + 2*n cap triangles
        assert_eq!(mesh.elements.len(), 2 * 8 + 2 * 8);
    }

    #[test]
    fn test_mesh_merge() {
        // Create a simple "room mesh"
        let mut room_mesh = RoomMesh {
            nodes: vec![
                BemPoint3D::new(0.0, 0.0, 0.0),
                BemPoint3D::new(1.0, 0.0, 0.0),
                BemPoint3D::new(0.0, 1.0, 0.0),
            ],
            elements: vec![SurfaceElement {
                nodes: vec![0, 1, 2],
            }],
        };

        let initial_nodes = room_mesh.nodes.len();
        let initial_elements = room_mesh.elements.len();

        // Create a box and merge
        let box_obj = BoxObject {
            name: "Box".to_string(),
            center: Point3DConfig {
                x: 0.5,
                y: 0.5,
                z: 0.5,
            },
            dimensions: [0.2, 0.2, 0.2],
            material: WallMaterialConfig::default(),
        };

        let obj_mesh = generate_box_mesh(&box_obj);
        obj_mesh.merge_into(&mut room_mesh);

        // Check that nodes and elements were added
        assert!(room_mesh.nodes.len() > initial_nodes);
        assert!(room_mesh.elements.len() > initial_elements);
    }

    #[test]
    fn test_element_count_estimation() {
        let objects = vec![
            ScatteringObjectConfig::Box(BoxObject {
                name: "Box1".to_string(),
                center: Point3DConfig {
                    x: 1.0,
                    y: 1.0,
                    z: 0.5,
                },
                dimensions: [1.0, 1.0, 1.0],
                material: WallMaterialConfig::default(),
            }),
            ScatteringObjectConfig::Sphere(SphereObject {
                name: "Sphere1".to_string(),
                center: Point3DConfig {
                    x: 2.0,
                    y: 2.0,
                    z: 1.0,
                },
                radius: 0.5,
                material: WallMaterialConfig::default(),
                subdivisions: 2,
            }),
        ];

        let estimated = estimate_scattering_element_count(&objects);

        // Box: 12 elements, Sphere with 2 subdivisions: 20 * 4^2 = 320
        assert_eq!(estimated, 12 + 320);
    }
}
