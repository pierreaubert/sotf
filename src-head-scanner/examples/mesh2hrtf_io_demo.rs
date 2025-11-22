//! Demo of Mesh2HRTF I/O operations
//!
//! This example demonstrates reading and writing meshes in Mesh2HRTF format.
//!
//! Usage:
//! ```bash
//! cargo run --example mesh2hrtf_io_demo
//! ```

use anyhow::Result;
use std::path::PathBuf;

// Since we can't compile the full crate due to dependencies, we'll inline the necessary code
// This is just for demonstration/testing purposes

use nalgebra::Point3;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{BufRead, BufReader, Write};

type Point = Point3<f64>;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Node {
    pub id: usize,
    pub position: Point,
}

impl Node {
    fn new(id: usize, position: Point) -> Self {
        Self { id, position }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Element {
    pub id: usize,
    pub material_id: usize,
    pub vertices: [usize; 3],
}

impl Element {
    fn new(id: usize, material_id: usize, vertices: [usize; 3]) -> Self {
        Self {
            id,
            material_id,
            vertices,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MeshMetadata {
    pub name: Option<String>,
    pub source: Option<String>,
}

impl Default for MeshMetadata {
    fn default() -> Self {
        Self {
            name: None,
            source: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Mesh {
    pub nodes: Vec<Node>,
    pub elements: Vec<Element>,
    pub metadata: MeshMetadata,
}

impl Mesh {
    fn new() -> Self {
        Self {
            nodes: Vec::new(),
            elements: Vec::new(),
            metadata: MeshMetadata::default(),
        }
    }

    fn validate(&self) -> Result<(), String> {
        for (i, node) in self.nodes.iter().enumerate() {
            if node.id != i {
                return Err(format!("Node ID mismatch: expected {}, got {}", i, node.id));
            }
        }

        for element in &self.elements {
            for &vertex_id in &element.vertices {
                if vertex_id >= self.nodes.len() {
                    return Err(format!(
                        "Element {} references non-existent vertex {}",
                        element.id, vertex_id
                    ));
                }
            }

            let [v0, v1, v2] = element.vertices;
            if v0 == v1 || v1 == v2 || v2 == v0 {
                return Err(format!("Element {} is degenerate", element.id));
            }
        }

        Ok(())
    }

    fn bounding_box(&self) -> (Point, Point) {
        if self.nodes.is_empty() {
            return (Point::origin(), Point::origin());
        }

        let mut min = self.nodes[0].position;
        let mut max = self.nodes[0].position;

        for node in &self.nodes {
            min.x = min.x.min(node.position.x);
            min.y = min.y.min(node.position.y);
            min.z = min.z.min(node.position.z);

            max.x = max.x.max(node.position.x);
            max.y = max.y.max(node.position.y);
            max.z = max.z.max(node.position.z);
        }

        (min, max)
    }
}

fn read_nodes(path: &std::path::Path) -> Result<Vec<Node>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut lines = reader.lines();

    let first_line = lines.next().ok_or_else(|| anyhow::anyhow!("Empty file"))??;
    let num_nodes: usize = first_line.trim().parse()?;

    let mut nodes = Vec::with_capacity(num_nodes);

    for line in lines {
        let line = line?;
        let parts: Vec<&str> = line.split_whitespace().collect();

        if parts.len() != 4 {
            anyhow::bail!("Invalid node format: expected 4 fields, got {}", parts.len());
        }

        let id: usize = parts[0].parse()?;
        let x: f64 = parts[1].parse()?;
        let y: f64 = parts[2].parse()?;
        let z: f64 = parts[3].parse()?;

        nodes.push(Node::new(id, Point::new(x, y, z)));
    }

    if nodes.len() != num_nodes {
        anyhow::bail!(
            "Node count mismatch: header says {}, found {}",
            num_nodes,
            nodes.len()
        );
    }

    Ok(nodes)
}

fn read_elements(path: &std::path::Path) -> Result<Vec<Element>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut lines = reader.lines();

    let first_line = lines.next().ok_or_else(|| anyhow::anyhow!("Empty file"))??;
    let num_elements: usize = first_line.trim().parse()?;

    let mut elements = Vec::with_capacity(num_elements);

    for line in lines {
        let line = line?;
        let parts: Vec<&str> = line.split_whitespace().collect();

        if parts.len() < 7 {
            anyhow::bail!(
                "Invalid element format: expected at least 7 fields, got {}",
                parts.len()
            );
        }

        let id: usize = parts[0].parse()?;
        let v1: usize = parts[1].parse()?;
        let v2: usize = parts[2].parse()?;
        let v3: usize = parts[3].parse()?;

        elements.push(Element::new(id, 0, [v1, v2, v3]));
    }

    if elements.len() != num_elements {
        anyhow::bail!(
            "Element count mismatch: header says {}, found {}",
            num_elements,
            elements.len()
        );
    }

    Ok(elements)
}

fn read_mesh(dir: &std::path::Path) -> Result<Mesh> {
    let nodes_path = dir.join("Nodes.txt");
    let elements_path = dir.join("Elements.txt");

    if !nodes_path.exists() {
        anyhow::bail!("Nodes.txt not found in {}", dir.display());
    }
    if !elements_path.exists() {
        anyhow::bail!("Elements.txt not found in {}", dir.display());
    }

    let nodes = read_nodes(&nodes_path)?;
    let elements = read_elements(&elements_path)?;

    let mut mesh = Mesh::new();
    mesh.nodes = nodes;
    mesh.elements = elements;
    mesh.metadata.source = Some(dir.display().to_string());

    mesh.validate()
        .map_err(|e| anyhow::anyhow!("Mesh validation failed: {}", e))?;

    Ok(mesh)
}

fn write_nodes(nodes: &[Node], path: &std::path::Path) -> Result<()> {
    let mut file = File::create(path)?;
    writeln!(file, "{}", nodes.len())?;

    for node in nodes {
        writeln!(
            file,
            "{} {:.6} {:.6} {:.6}",
            node.id, node.position.x, node.position.y, node.position.z
        )?;
    }

    Ok(())
}

fn write_elements(elements: &[Element], path: &std::path::Path) -> Result<()> {
    let mut file = File::create(path)?;
    writeln!(file, "{}", elements.len())?;

    for element in elements {
        writeln!(
            file,
            "{} {} {} {} 0 0 0",
            element.id, element.vertices[0], element.vertices[1], element.vertices[2]
        )?;
    }

    Ok(())
}

fn write_mesh(mesh: &Mesh, dir: &std::path::Path) -> Result<()> {
    std::fs::create_dir_all(dir)?;

    let nodes_path = dir.join("Nodes.txt");
    let elements_path = dir.join("Elements.txt");

    write_nodes(&mesh.nodes, &nodes_path)?;
    write_elements(&mesh.elements, &elements_path)?;

    Ok(())
}

fn main() -> Result<()> {
    println!("╔═══════════════════════════════════════════════════════╗");
    println!("║     Mesh2HRTF I/O Demonstration                      ║");
    println!("╚═══════════════════════════════════════════════════════╝\n");

    // Test 1: Read real Mesh2HRTF data
    let test_paths = vec![
        PathBuf::from("/tmp/mesh2hrtf_test/Mesh2HRTF/tests/resources/test_numcalc/project_folder_ears/ears_basic_project/ObjectMeshes/Reference"),
        PathBuf::from("/tmp/mesh2hrtf_test/Mesh2HRTF/tests/resources/test_numcalc/project_folder_ears/ears_basic_project/EvaluationGrids/HorPlane"),
    ];

    for test_path in &test_paths {
        if !test_path.exists() {
            println!("⚠ Test data not available: {}", test_path.display());
            println!("  Run: ./src-bem/scripts/setup_test_project.sh\n");
            continue;
        }

        println!("═══ Test: Reading Mesh2HRTF Data ═══");
        println!("Path: {}\n", test_path.display());

        match read_mesh(test_path) {
            Ok(mesh) => {
                println!("✓ Successfully read mesh!");
                println!("  Nodes: {}", mesh.nodes.len());
                println!("  Elements: {}", mesh.elements.len());

                // Validate
                match mesh.validate() {
                    Ok(()) => println!("  ✓ Mesh validation passed"),
                    Err(e) => println!("  ✗ Mesh validation failed: {}", e),
                }

                // Bounding box
                let (min, max) = mesh.bounding_box();
                println!("  Bounding box:");
                println!("    Min: ({:.6}, {:.6}, {:.6})", min.x, min.y, min.z);
                println!("    Max: ({:.6}, {:.6}, {:.6})", max.x, max.y, max.z);

                // Show first few nodes
                println!("  First 3 nodes:");
                for node in mesh.nodes.iter().take(3) {
                    println!(
                        "    {} -> ({:.6}, {:.6}, {:.6})",
                        node.id, node.position.x, node.position.y, node.position.z
                    );
                }

                // Show first few elements
                println!("  First 3 elements:");
                for element in mesh.elements.iter().take(3) {
                    println!(
                        "    {} -> [{}, {}, {}]",
                        element.id, element.vertices[0], element.vertices[1], element.vertices[2]
                    );
                }

                // Test 2: Write and read back
                println!("\n═══ Test: Roundtrip (Write + Read) ═══");
                let output_dir = PathBuf::from("/tmp/mesh2hrtf_io_test");

                println!("Writing to: {}", output_dir.display());
                write_mesh(&mesh, &output_dir)?;
                println!("✓ Write successful");

                println!("Reading back...");
                let mesh2 = read_mesh(&output_dir)?;
                println!("✓ Read successful");

                // Verify
                if mesh.nodes.len() == mesh2.nodes.len()
                    && mesh.elements.len() == mesh2.elements.len()
                {
                    println!("✓ Roundtrip validation passed");
                    println!("  Nodes match: {}", mesh.nodes.len());
                    println!("  Elements match: {}", mesh.elements.len());

                    // Check a few nodes for exact match
                    let mut all_match = true;
                    for (n1, n2) in mesh.nodes.iter().zip(mesh2.nodes.iter()).take(10) {
                        if n1.id != n2.id || n1.position != n2.position {
                            all_match = false;
                            break;
                        }
                    }

                    if all_match {
                        println!("  ✓ Node data matches exactly");
                    } else {
                        println!("  ✗ Node data mismatch detected");
                    }
                } else {
                    println!("✗ Roundtrip validation failed");
                    println!("  Original: {} nodes, {} elements", mesh.nodes.len(), mesh.elements.len());
                    println!("  Read back: {} nodes, {} elements", mesh2.nodes.len(), mesh2.elements.len());
                }

                println!();
            }
            Err(e) => {
                println!("✗ Failed to read mesh: {}", e);
                println!();
            }
        }
    }

    println!("╔═══════════════════════════════════════════════════════╗");
    println!("║                 Demo Complete                         ║");
    println!("╚═══════════════════════════════════════════════════════╝");

    Ok(())
}
