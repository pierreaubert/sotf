//! Mesh2HRTF integration for NumCalc BEM simulation
//!
//! This module provides Rust implementations of Mesh2Input and Output2HRTF
//! functionality from the Mesh2HRTF project, enabling complete HRTF calculation
//! pipelines in Rust.
//!
//! # Modules
//!
//! - `types` - Core data structures for meshes, grids, and HRTF data
//! - `mesh_io` - Read/write Mesh2HRTF mesh format (Nodes.txt, Elements.txt)
//! - `evaluation_grid` - Evaluation grid generation (spherical, planar)
//! - `source_config` - Source configuration (ears, point source, plane wave)
//! - `project_builder` - Complete NumCalc project creation
//! - `nc_inp_writer` - NC.inp file generation
//!
//! # Workflow
//!
//! ```text
//! 1. Load or create head mesh
//! 2. Generate evaluation grids
//! 3. Configure sources (ear regions)
//! 4. Create NumCalc project
//! 5. Run BEM simulation (via src-bem FFI)
//! 6. Parse output and generate SOFA files
//! ```
//!
//! # Example
//!
//! ```rust,no_run
//! use head_scanner::mesh2hrtf::*;
//!
//! // Load head mesh from scanner or file
//! let mesh = MeshIO::read_from_obj("head_scan.obj")?;
//!
//! // Create project builder
//! let project = ProjectBuilder::new()
//!     .with_mesh(mesh)
//!     .with_evaluation_grid(GridType::Sphere { radius: 1.5 })
//!     .with_source_type(SourceType::BothEars {
//!         left_material: 1,
//!         right_material: 2,
//!     })
//!     .with_frequency_range(200.0, 20000.0, 100)
//!     .build()?;
//!
//! // Export to NumCalc format
//! project.export("/path/to/project")?;
//! # Ok::<(), anyhow::Error>(())
//! ```

pub mod types;
pub mod mesh_io;
pub mod evaluation_grid;

// Re-exports for convenience
pub use types::*;
pub use mesh_io::MeshIO;
pub use evaluation_grid::GridGenerator;
