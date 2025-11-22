//! Project builder for NumCalc simulations
//!
//! This module creates complete Mesh2HRTF project directory structures
//! ready for NumCalc BEM simulation.

use crate::mesh2hrtf::evaluation_grid::GridGenerator;
use crate::mesh2hrtf::mesh_io::MeshIO;
use crate::mesh2hrtf::nc_inp_writer::{NCInpWriter, NumCalcParameters};
use crate::mesh2hrtf::source_config::SourceConfiguration;
use crate::mesh2hrtf::types::*;
use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

/// Project builder for creating complete NumCalc projects
pub struct ProjectBuilder {
    /// Reference mesh (head)
    mesh: Option<Mesh>,

    /// Evaluation grids
    grids: Vec<(String, EvaluationGrid)>,

    /// Source configuration
    source_config: Option<SourceConfiguration>,

    /// Simulation frequencies (Hz)
    frequencies: Vec<f64>,

    /// Project title
    title: String,

    /// Speed of sound (m/s)
    speed_of_sound: f64,

    /// Density of medium (kg/m³)
    density_of_medium: f64,

    /// BEM method
    bem_method: BemMethod,

    /// Mesh2HRTF version
    version: String,
}

impl Default for ProjectBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl ProjectBuilder {
    /// Create a new project builder
    pub fn new() -> Self {
        Self {
            mesh: None,
            grids: Vec::new(),
            source_config: None,
            frequencies: Vec::new(),
            title: "Head-Related Transfer Functions".to_string(),
            speed_of_sound: 343.0,
            density_of_medium: 1.1839,
            bem_method: BemMethod::MlFmmBem,
            version: "1.0.0".to_string(),
        }
    }

    /// Set the reference mesh
    pub fn with_mesh(mut self, mesh: Mesh) -> Self {
        self.mesh = Some(mesh);
        self
    }

    /// Add an evaluation grid
    pub fn add_grid(mut self, name: impl Into<String>, grid: EvaluationGrid) -> Self {
        self.grids.push((name.into(), grid));
        self
    }

    /// Add a spherical evaluation grid
    pub fn add_sphere_grid(
        self,
        name: impl Into<String>,
        radius: f64,
        num_points: usize,
    ) -> Result<Self> {
        let grid = GridGenerator::generate_sphere(radius, num_points)?;
        Ok(self.add_grid(name, grid))
    }

    /// Add a horizontal plane evaluation grid
    pub fn add_horizontal_plane_grid(
        self,
        name: impl Into<String>,
        radius: f64,
        z_height: f64,
        num_points: usize,
    ) -> Result<Self> {
        let grid = GridGenerator::generate_horizontal_plane(radius, z_height, num_points)?;
        Ok(self.add_grid(name, grid))
    }

    /// Add a vertical plane evaluation grid
    pub fn add_vertical_plane_grid(
        self,
        name: impl Into<String>,
        radius: f64,
        azimuth: f64,
        num_points: usize,
    ) -> Result<Self> {
        let grid = GridGenerator::generate_vertical_plane(radius, azimuth, num_points)?;
        Ok(self.add_grid(name, grid))
    }

    /// Set the source configuration
    pub fn with_source_config(mut self, config: SourceConfiguration) -> Self {
        self.source_config = Some(config);
        self
    }

    /// Set simulation frequencies
    pub fn with_frequencies(mut self, frequencies: Vec<f64>) -> Self {
        self.frequencies = frequencies;
        self
    }

    /// Set frequency range with linear spacing
    pub fn with_frequency_range(
        mut self,
        min_freq: f64,
        max_freq: f64,
        num_freqs: usize,
    ) -> Self {
        let step = (max_freq - min_freq) / (num_freqs - 1) as f64;
        self.frequencies = (0..num_freqs)
            .map(|i| min_freq + step * i as f64)
            .collect();
        self
    }

    /// Set project title
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    /// Set speed of sound
    pub fn with_speed_of_sound(mut self, speed: f64) -> Self {
        self.speed_of_sound = speed;
        self
    }

    /// Set density of medium
    pub fn with_density(mut self, density: f64) -> Self {
        self.density_of_medium = density;
        self
    }

    /// Set BEM method
    pub fn with_bem_method(mut self, method: BemMethod) -> Self {
        self.bem_method = method;
        self
    }

    /// Build the project and export to directory
    pub fn build_and_export<P: AsRef<Path>>(self, output_dir: P) -> Result<Project> {
        let project = self.build()?;
        project.export(output_dir)?;
        Ok(project)
    }

    /// Build the project without exporting
    pub fn build(self) -> Result<Project> {
        let mesh = self
            .mesh
            .ok_or_else(|| anyhow::anyhow!("No mesh provided"))?;

        let source_config = self
            .source_config
            .ok_or_else(|| anyhow::anyhow!("No source configuration provided"))?;

        if self.frequencies.is_empty() {
            anyhow::bail!("No frequencies provided");
        }

        if self.grids.is_empty() {
            anyhow::bail!("No evaluation grids provided");
        }

        Ok(Project {
            mesh,
            grids: self.grids,
            source_config,
            parameters: NumCalcParameters {
                title: self.title,
                version: self.version,
                speed_of_sound: self.speed_of_sound,
                density_of_medium: self.density_of_medium,
                frequencies: self.frequencies,
                bem_method: self.bem_method,
                evaluation_grids: Vec::new(), // Will be filled during export
                num_mesh_nodes: 0,            // Will be filled during export
                num_mesh_elements: 0,         // Will be filled during export
                num_eval_nodes: 0,            // Will be filled during export
                num_eval_elements: 0,         // Will be filled during export
            },
        })
    }
}

/// A complete NumCalc project ready for export
pub struct Project {
    /// Reference mesh
    mesh: Mesh,

    /// Evaluation grids
    grids: Vec<(String, EvaluationGrid)>,

    /// Source configuration
    source_config: SourceConfiguration,

    /// NumCalc parameters
    parameters: NumCalcParameters,
}

impl Project {
    /// Export the project to a directory
    pub fn export<P: AsRef<Path>>(&self, output_dir: P) -> Result<()> {
        let base_dir = output_dir.as_ref();

        // Create directory structure
        self.create_directory_structure(base_dir)?;

        // Write reference mesh
        self.export_mesh(base_dir)?;

        // Write evaluation grids
        self.export_grids(base_dir)?;

        // Write NC.inp files for each source
        self.export_nc_inp_files(base_dir)?;

        Ok(())
    }

    fn create_directory_structure(&self, base_dir: &Path) -> Result<()> {
        // Create base directory
        fs::create_dir_all(base_dir)
            .with_context(|| format!("Failed to create base directory: {:?}", base_dir))?;

        // Create ObjectMeshes/Reference
        let object_mesh_dir = base_dir.join("ObjectMeshes").join("Reference");
        fs::create_dir_all(&object_mesh_dir).with_context(|| {
            format!("Failed to create ObjectMeshes directory: {:?}", object_mesh_dir)
        })?;

        // Create EvaluationGrids subdirectories
        for (grid_name, _) in &self.grids {
            let grid_dir = base_dir.join("EvaluationGrids").join(grid_name);
            fs::create_dir_all(&grid_dir)
                .with_context(|| format!("Failed to create grid directory: {:?}", grid_dir))?;
        }

        // Create NumCalc/source_N directories
        for source_idx in 0..self.source_config.num_sources {
            let source_dir = base_dir
                .join("NumCalc")
                .join(format!("source_{}", source_idx + 1));
            fs::create_dir_all(&source_dir)
                .with_context(|| format!("Failed to create source directory: {:?}", source_dir))?;
        }

        Ok(())
    }

    fn export_mesh(&self, base_dir: &Path) -> Result<()> {
        let mesh_dir = base_dir.join("ObjectMeshes").join("Reference");
        MeshIO::write_mesh2hrtf(&self.mesh, &mesh_dir)
            .context("Failed to write reference mesh")?;
        Ok(())
    }

    fn export_grids(&self, base_dir: &Path) -> Result<()> {
        for (grid_name, grid) in &self.grids {
            let grid_dir = base_dir.join("EvaluationGrids").join(grid_name);

            // Convert EvaluationGrid to Mesh for writing
            let grid_mesh = grid.to_mesh();
            MeshIO::write_mesh2hrtf(&grid_mesh, &grid_dir)
                .with_context(|| format!("Failed to write grid: {}", grid_name))?;
        }
        Ok(())
    }

    fn export_nc_inp_files(&self, base_dir: &Path) -> Result<()> {
        // Update parameters with actual counts
        let mut params = self.parameters.clone();
        params.num_mesh_nodes = self.mesh.nodes.len();
        params.num_mesh_elements = self.mesh.elements.len();
        params.num_eval_nodes = self.grids.iter().map(|(_, g)| g.nodes.len()).sum();
        params.num_eval_elements = self.grids.iter().map(|(_, g)| g.elements.len()).sum();
        params.evaluation_grids = self.grids.iter().map(|(name, _)| name.clone()).collect();

        // Write NC.inp for each source
        for source_idx in 0..self.source_config.num_sources {
            let nc_inp_path = base_dir
                .join("NumCalc")
                .join(format!("source_{}", source_idx + 1))
                .join("NC.inp");

            NCInpWriter::write(&nc_inp_path, &params, &self.source_config, source_idx)
                .with_context(|| format!("Failed to write NC.inp for source {}", source_idx + 1))?;
        }

        Ok(())
    }

    /// Get the number of sources
    pub fn num_sources(&self) -> usize {
        self.source_config.num_sources
    }

    /// Get source name
    pub fn source_name(&self, index: usize) -> String {
        self.source_config.source_name(index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nalgebra::Point3;
    use tempfile::TempDir;

    #[test]
    fn test_project_builder_basic() {
        let mut mesh = Mesh::new();
        mesh.nodes = vec![
            Node::from_coords(0, 0.0, 0.0, 0.0),
            Node::from_coords(1, 1.0, 0.0, 0.0),
            Node::from_coords(2, 0.0, 1.0, 0.0),
        ];
        mesh.elements = vec![Element::new(0, 0, [0, 1, 2])];

        let source_config = SourceConfiguration::plane_wave(Vec3::new(0.0, -1.0, 0.0));

        let builder = ProjectBuilder::new()
            .with_mesh(mesh)
            .with_source_config(source_config)
            .with_frequency_range(1000.0, 5000.0, 5);

        let grid = GridGenerator::generate_sphere(1.0, 10).unwrap();
        let builder = builder.add_grid("test_sphere", grid);

        let project = builder.build();
        assert!(project.is_ok());

        let project = project.unwrap();
        assert_eq!(project.num_sources(), 1);
        assert_eq!(project.parameters.frequencies.len(), 5);
    }

    #[test]
    fn test_project_export() {
        let temp_dir = TempDir::new().unwrap();

        let mut mesh = Mesh::new();
        mesh.nodes = vec![
            Node::from_coords(0, 0.0, 0.0, 0.0),
            Node::from_coords(1, 1.0, 0.0, 0.0),
            Node::from_coords(2, 0.0, 1.0, 0.0),
        ];
        mesh.elements = vec![Element::new(0, 0, [0, 1, 2])];

        let source_config = SourceConfiguration::point_source(Point3::new(0.0, 0.0, 10.0));

        let grid = GridGenerator::generate_horizontal_plane(1.0, 0.0, 12).unwrap();

        let project = ProjectBuilder::new()
            .with_mesh(mesh)
            .with_source_config(source_config)
            .with_frequencies(vec![1000.0, 2000.0])
            .add_grid("HorPlane", grid)
            .build()
            .unwrap();

        let result = project.export(temp_dir.path());
        assert!(result.is_ok());

        // Verify directory structure
        assert!(temp_dir.path().join("ObjectMeshes/Reference").exists());
        assert!(temp_dir.path().join("EvaluationGrids/HorPlane").exists());
        assert!(temp_dir.path().join("NumCalc/source_1").exists());

        // Verify files exist
        assert!(temp_dir
            .path()
            .join("ObjectMeshes/Reference/Nodes.txt")
            .exists());
        assert!(temp_dir
            .path()
            .join("ObjectMeshes/Reference/Elements.txt")
            .exists());
        assert!(temp_dir
            .path()
            .join("EvaluationGrids/HorPlane/Nodes.txt")
            .exists());
        assert!(temp_dir
            .path()
            .join("NumCalc/source_1/NC.inp")
            .exists());
    }
}
