//! Mesh file I/O for Mesh2HRTF format
//!
//! This module handles reading and writing mesh files in the Mesh2HRTF format:
//! - `Nodes.txt`: Vertex positions
//! - `Elements.txt`: Triangular or quadrilateral face definitions
//!
//! # Format Specification
//!
//! ## Nodes.txt
//!
//! ```text
//! <num_nodes>
//! <node_id> <x> <y> <z>
//! <node_id> <x> <y> <z>
//! ...
//! ```
//!
//! ## Elements.txt
//!
//! ```text
//! <num_elements>
//! <element_id> <v1> <v2> <v3> 0 0 0          (triangle)
//! <element_id> <v1> <v2> <v3> <v4> 0 0 0     (quad)
//! ...
//! ```
//!
//! Note: The last `0 0 0` are padding/flags. Material IDs are assigned
//! separately via NC.inp BOUNDARY directives.

use super::types::{Element, Mesh, MeshMetadata, Node, Point};
use anyhow::{Context, Result};
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

/// Mesh I/O operations
pub struct MeshIO;

impl MeshIO {
    /// Read a mesh from Mesh2HRTF format (Nodes.txt + Elements.txt)
    ///
    /// # Arguments
    ///
    /// * `dir` - Directory containing Nodes.txt and Elements.txt
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use head_scanner::mesh2hrtf::MeshIO;
    ///
    /// let mesh = MeshIO::read_mesh2hrtf("/path/to/mesh")?;
    /// println!("Loaded {} nodes, {} elements", mesh.num_nodes(), mesh.num_elements());
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn read_mesh2hrtf<P: AsRef<Path>>(dir: P) -> Result<Mesh> {
        let dir = dir.as_ref();
        let nodes_path = dir.join("Nodes.txt");
        let elements_path = dir.join("Elements.txt");

        if !nodes_path.exists() {
            anyhow::bail!("Nodes.txt not found in {}", dir.display());
        }
        if !elements_path.exists() {
            anyhow::bail!("Elements.txt not found in {}", dir.display());
        }

        let nodes = Self::read_nodes(&nodes_path)?;
        let elements = Self::read_elements(&elements_path)?;

        let mut mesh = Mesh::new();
        mesh.nodes = nodes;
        mesh.elements = elements;
        mesh.metadata.source = Some(dir.display().to_string());

        mesh.validate().context("Mesh validation failed")?;

        Ok(mesh)
    }

    /// Write a mesh to Mesh2HRTF format
    ///
    /// Creates Nodes.txt and Elements.txt in the specified directory.
    ///
    /// # Arguments
    ///
    /// * `mesh` - The mesh to write
    /// * `dir` - Output directory
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use head_scanner::mesh2hrtf::{Mesh, MeshIO};
    ///
    /// let mesh = Mesh::new();
    /// MeshIO::write_mesh2hrtf(&mesh, "/path/to/output")?;
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn write_mesh2hrtf<P: AsRef<Path>>(mesh: &Mesh, dir: P) -> Result<()> {
        let dir = dir.as_ref();
        std::fs::create_dir_all(dir)
            .context(format!("Failed to create directory: {}", dir.display()))?;

        let nodes_path = dir.join("Nodes.txt");
        let elements_path = dir.join("Elements.txt");

        Self::write_nodes(&mesh.nodes, &nodes_path)?;
        Self::write_elements(&mesh.elements, &elements_path)?;

        Ok(())
    }

    /// Read nodes from Nodes.txt
    fn read_nodes<P: AsRef<Path>>(path: P) -> Result<Vec<Node>> {
        let path = path.as_ref();
        let file = File::open(path).context(format!("Failed to open {}", path.display()))?;
        let reader = BufReader::new(file);
        let mut lines = reader.lines();

        // Read number of nodes
        let first_line = lines
            .next()
            .ok_or_else(|| anyhow::anyhow!("Empty Nodes.txt file"))??;
        let num_nodes: usize = first_line
            .trim()
            .parse()
            .context("Failed to parse number of nodes")?;

        let mut nodes = Vec::with_capacity(num_nodes);

        // Read nodes
        for (line_num, line) in lines.enumerate() {
            let line = line?;
            let parts: Vec<&str> = line.split_whitespace().collect();

            if parts.len() != 4 {
                anyhow::bail!(
                    "Invalid node format at line {}: expected 4 fields, got {}",
                    line_num + 2,
                    parts.len()
                );
            }

            let id: usize = parts[0]
                .parse()
                .context(format!("Failed to parse node ID at line {}", line_num + 2))?;
            let x: f64 = parts[1].parse().context(format!(
                "Failed to parse X coordinate at line {}",
                line_num + 2
            ))?;
            let y: f64 = parts[2].parse().context(format!(
                "Failed to parse Y coordinate at line {}",
                line_num + 2
            ))?;
            let z: f64 = parts[3].parse().context(format!(
                "Failed to parse Z coordinate at line {}",
                line_num + 2
            ))?;

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

    /// Write nodes to Nodes.txt
    fn write_nodes<P: AsRef<Path>>(nodes: &[Node], path: P) -> Result<()> {
        let path = path.as_ref();
        let mut file =
            File::create(path).context(format!("Failed to create {}", path.display()))?;

        // Write number of nodes
        writeln!(file, "{}", nodes.len())?;

        // Write nodes
        for node in nodes {
            writeln!(
                file,
                "{} {:.6} {:.6} {:.6}",
                node.id, node.position.x, node.position.y, node.position.z
            )?;
        }

        Ok(())
    }

    /// Read elements from Elements.txt
    fn read_elements<P: AsRef<Path>>(path: P) -> Result<Vec<Element>> {
        let path = path.as_ref();
        let file = File::open(path).context(format!("Failed to open {}", path.display()))?;
        let reader = BufReader::new(file);
        let mut lines = reader.lines();

        // Read number of elements
        let first_line = lines
            .next()
            .ok_or_else(|| anyhow::anyhow!("Empty Elements.txt file"))??;
        let num_elements: usize = first_line
            .trim()
            .parse()
            .context("Failed to parse number of elements")?;

        let mut elements = Vec::with_capacity(num_elements);

        // Read elements
        for (line_num, line) in lines.enumerate() {
            let line = line?;
            let parts: Vec<&str> = line.split_whitespace().collect();

            if parts.len() < 7 {
                anyhow::bail!(
                    "Invalid element format at line {}: expected at least 7 fields, got {}",
                    line_num + 2,
                    parts.len()
                );
            }

            let id: usize = parts[0].parse().context(format!(
                "Failed to parse element ID at line {}",
                line_num + 2
            ))?;

            // Elements can be triangles (v1, v2, v3, 0, 0, 0) or quads (v1, v2, v3, v4, 0, 0, 0)
            // For now, we only support triangles
            let v1: usize = parts[1]
                .parse()
                .context(format!("Failed to parse vertex 1 at line {}", line_num + 2))?;
            let v2: usize = parts[2]
                .parse()
                .context(format!("Failed to parse vertex 2 at line {}", line_num + 2))?;
            let v3: usize = parts[3]
                .parse()
                .context(format!("Failed to parse vertex 3 at line {}", line_num + 2))?;

            // Material ID defaults to 0 (will be assigned via NC.inp BOUNDARY directives)
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

    /// Write elements to Elements.txt
    fn write_elements<P: AsRef<Path>>(elements: &[Element], path: P) -> Result<()> {
        let path = path.as_ref();
        let mut file =
            File::create(path).context(format!("Failed to create {}", path.display()))?;

        // Write number of elements
        writeln!(file, "{}", elements.len())?;

        // Write elements (triangles only)
        for element in elements {
            writeln!(
                file,
                "{} {} {} {} 0 0 0",
                element.id, element.vertices[0], element.vertices[1], element.vertices[2]
            )?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    fn create_test_nodes_file(dir: &Path) -> Result<()> {
        let path = dir.join("Nodes.txt");
        let mut file = File::create(path)?;
        writeln!(file, "3")?;
        writeln!(file, "0 0.000000 0.000000 0.000000")?;
        writeln!(file, "1 1.000000 0.000000 0.000000")?;
        writeln!(file, "2 0.000000 1.000000 0.000000")?;
        Ok(())
    }

    fn create_test_elements_file(dir: &Path) -> Result<()> {
        let path = dir.join("Elements.txt");
        let mut file = File::create(path)?;
        writeln!(file, "1")?;
        writeln!(file, "0 0 1 2 0 0 0")?;
        Ok(())
    }

    #[test]
    fn test_read_nodes() {
        let dir = TempDir::new().unwrap();
        create_test_nodes_file(dir.path()).unwrap();

        let nodes = MeshIO::read_nodes(&dir.path().join("Nodes.txt")).unwrap();
        assert_eq!(nodes.len(), 3);
        assert_eq!(nodes[0].id, 0);
        assert_eq!(nodes[0].position, Point::new(0.0, 0.0, 0.0));
        assert_eq!(nodes[1].id, 1);
        assert_eq!(nodes[1].position, Point::new(1.0, 0.0, 0.0));
        assert_eq!(nodes[2].id, 2);
        assert_eq!(nodes[2].position, Point::new(0.0, 1.0, 0.0));
    }

    #[test]
    fn test_read_elements() {
        let dir = TempDir::new().unwrap();
        create_test_elements_file(dir.path()).unwrap();

        let elements = MeshIO::read_elements(&dir.path().join("Elements.txt")).unwrap();
        assert_eq!(elements.len(), 1);
        assert_eq!(elements[0].id, 0);
        assert_eq!(elements[0].vertices, [0, 1, 2]);
    }

    #[test]
    fn test_read_write_roundtrip() {
        let dir = TempDir::new().unwrap();
        create_test_nodes_file(dir.path()).unwrap();
        create_test_elements_file(dir.path()).unwrap();

        // Read mesh
        let mesh = MeshIO::read_mesh2hrtf(dir.path()).unwrap();
        assert_eq!(mesh.num_nodes(), 3);
        assert_eq!(mesh.num_elements(), 1);

        // Write to different directory
        let out_dir = TempDir::new().unwrap();
        MeshIO::write_mesh2hrtf(&mesh, out_dir.path()).unwrap();

        // Read back and compare
        let mesh2 = MeshIO::read_mesh2hrtf(out_dir.path()).unwrap();
        assert_eq!(mesh2.num_nodes(), mesh.num_nodes());
        assert_eq!(mesh2.num_elements(), mesh.num_elements());

        for (n1, n2) in mesh.nodes.iter().zip(mesh2.nodes.iter()) {
            assert_eq!(n1.id, n2.id);
            assert_eq!(n1.position, n2.position);
        }

        for (e1, e2) in mesh.elements.iter().zip(mesh2.elements.iter()) {
            assert_eq!(e1.id, e2.id);
            assert_eq!(e1.vertices, e2.vertices);
        }
    }

    #[test]
    fn test_read_real_mesh2hrtf() {
        // This test requires a real Mesh2HRTF project
        let test_path = "/tmp/mesh2hrtf_test/Mesh2HRTF/tests/resources/test_numcalc/project_folder_ears/ears_basic_project/ObjectMeshes/Reference";

        if !Path::new(test_path).exists() {
            eprintln!("Skipping real mesh test - test data not available");
            return;
        }

        let mesh = MeshIO::read_mesh2hrtf(test_path).unwrap();
        assert!(mesh.num_nodes() > 0);
        assert!(mesh.num_elements() > 0);
        assert!(mesh.validate().is_ok());

        println!(
            "Read real mesh: {} nodes, {} elements",
            mesh.num_nodes(),
            mesh.num_elements()
        );
    }
}
