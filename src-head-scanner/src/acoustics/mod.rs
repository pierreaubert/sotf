//! Acoustics module for HRTF generation from 3D head scans
//!
//! This module provides complete functionality for generating HRTF (Head-Related Transfer Function)
//! data from 3D head meshes and exporting to industry-standard SOFA format.
//!
//! # Overview
//!
//! The HRTF generation pipeline consists of:
//!
//! 1. **Geometric Analysis** (`model.rs`)
//!    - Automatic ear detection from mesh
//!    - Head dimension measurement
//!    - Acoustic model creation
//!
//! 2. **HRTF Computation** (`hrtf.rs`)
//!    - **Analytical**: Fast Woodworth-Schlosberg sphere model
//!    - **BEM**: High-accuracy boundary element method (optional)
//!
//! 3. **SOFA Export** (`sofa.rs`)
//!    - Standard HDF5-based format
//!    - Compatible with audio software and research tools
//!
//! # Usage Example
//!
//! ```no_run
//! use head_scanner::acoustics::{AcousticHeadModel, AnalyticalHRTF, SOFAWriter};
//! use head_scanner::Mesh;
//!
//! # fn main() -> head_scanner::ScannerResult<()> {
//! // Load 3D head mesh (from scanning)
//! let mesh = Mesh::from_obj("head.obj")?;
//!
//! // Create acoustic model with automatic ear detection
//! let acoustic_model = AcousticHeadModel::from_mesh(&mesh)?;
//!
//! println!("Left ear: {:?}", acoustic_model.left_ear);
//! println!("Right ear: {:?}", acoustic_model.right_ear);
//!
//! // Generate HRTF using analytical model
//! let hrtf_generator = AnalyticalHRTF::new(acoustic_model.clone(), 44100.0);
//!
//! // Compute HRTFs for standard measurement grid
//! let (source_positions, impulse_responses) =
//!     hrtf_generator.compute_hrtf_grid(72, 36, 100.0); // 72 azimuth × 36 elevation, 1m distance
//!
//! // Export to SOFA file
//! let sofa_writer = SOFAWriter::new("output.sofa");
//! sofa_writer.write_sofa(&acoustic_model, &source_positions, &impulse_responses, 44100.0)?;
//!
//! println!("SOFA file generated: output.sofa");
//! # Ok(())
//! # }
//! ```
//!
//! # Analytical vs BEM
//!
//! ## Analytical (Woodworth-Schlosberg)
//! - **Pros**: Fast (seconds), no external dependencies, good for prototyping
//! - **Cons**: Less accurate, simplified physics
//! - **Use case**: Quick testing, real-time applications
//!
//! ## BEM (Boundary Element Method)
//! - **Pros**: High accuracy, accounts for head shape details
//! - **Cons**: Very slow (hours), requires external solver (MESH2HRTF)
//! - **Use case**: Research, high-quality HRTF datasets
//!
//! # SOFA Format
//!
//! SOFA (Spatially Oriented Format for Acoustics) is the standard format for HRTF data.
//! It's used by:
//! - Research institutions
//! - Audio software (Reaper, Max/MSP, etc.)
//! - VR/AR applications
//! - Hearing aid development
//!
//! Learn more: https://www.sofaconventions.org/

pub mod bem;
pub mod hrtf;
pub mod model;
pub mod sofa;

// Re-export main types for convenience
pub use bem::{BEMConfig, BEMSolver, estimate_bem_time, is_bem_available};
pub use hrtf::AnalyticalHRTF;
pub use model::AcousticHeadModel;
pub use sofa::SOFAWriter;

/// Generate SOFA file from mesh using analytical HRTF
///
/// This is a convenience function that performs the entire pipeline:
/// 1. Create acoustic model (detect ears)
/// 2. Generate HRTF grid using analytical model
/// 3. Write SOFA file
///
/// # Arguments
/// * `mesh` - 3D head mesh
/// * `output_path` - Path for output SOFA file
/// * `sample_rate` - Sample rate in Hz (e.g., 44100)
/// * `azimuth_resolution` - Number of azimuth angles (e.g., 72 = 5° spacing)
/// * `elevation_resolution` - Number of elevation angles (e.g., 36 = 5° spacing)
/// * `distance` - Source distance in cm (e.g., 100 = 1m)
///
/// # Example
/// ```no_run
/// use head_scanner::acoustics::generate_sofa_analytical;
/// use head_scanner::Mesh;
///
/// # fn main() -> head_scanner::ScannerResult<()> {
/// let mesh = Mesh::from_obj("head.obj")?;
///
/// generate_sofa_analytical(
///     &mesh,
///     "output.sofa",
///     44100.0,  // 44.1 kHz
///     72,       // 5° azimuth spacing
///     36,       // 5° elevation spacing
///     100.0,    // 1m distance
/// )?;
/// # Ok(())
/// # }
/// ```
pub fn generate_sofa_analytical(
    mesh: &crate::Mesh,
    output_path: &str,
    sample_rate: f32,
    azimuth_resolution: usize,
    elevation_resolution: usize,
    distance: f32,
) -> crate::ScannerResult<()> {
    use crate::error::ScannerResult;

    log::info!("Starting SOFA generation pipeline (analytical)");

    // Step 1: Create acoustic model
    log::info!("Step 1/3: Analyzing head geometry...");
    let acoustic_model = AcousticHeadModel::from_mesh(mesh)?;

    log::info!(
        "  Interaural distance: {:.1}cm",
        acoustic_model.interaural_distance()
    );

    // Step 2: Generate HRTFs
    log::info!(
        "Step 2/3: Computing HRTFs ({}az × {}el = {} positions)...",
        azimuth_resolution,
        elevation_resolution,
        azimuth_resolution * elevation_resolution
    );

    let hrtf_generator = AnalyticalHRTF::new(acoustic_model.clone(), sample_rate);
    let (source_positions, impulse_responses) =
        hrtf_generator.compute_hrtf_grid(azimuth_resolution, elevation_resolution, distance);

    // Step 3: Write SOFA file
    log::info!("Step 3/3: Writing SOFA file...");
    let sofa_writer = SOFAWriter::new(output_path);
    sofa_writer.write_sofa(
        &acoustic_model,
        &source_positions,
        &impulse_responses,
        sample_rate,
    )?;

    log::info!("✓ SOFA generation complete!");
    log::info!("  Output: {}", output_path);
    log::info!("  Positions: {}", source_positions.len());
    log::info!("  Sample rate: {} Hz", sample_rate);
    log::info!(
        "  IR length: {} samples ({:.1}ms)",
        impulse_responses[0][0].len(),
        impulse_responses[0][0].len() as f32 / sample_rate * 1000.0
    );

    Ok(())
}

/// Generate SOFA file from mesh using BEM solver
///
/// This performs the complete BEM pipeline:
/// 1. Create acoustic model (detect ears)
/// 2. Export mesh for BEM solver
/// 3. Run BEM simulation (external MESH2HRTF)
/// 4. Import results and convert to time domain (IFFT)
/// 5. Write SOFA file
///
/// **Note**: This requires MESH2HRTF to be installed and can take many hours!
///
/// # Arguments
/// * `mesh` - 3D head mesh
/// * `output_path` - Path for output SOFA file
/// * `sample_rate` - Sample rate in Hz (e.g., 44100)
/// * `config` - BEM solver configuration
/// * `source_positions` - Source measurement positions
/// * `work_dir` - Working directory for BEM files
///
/// # Example
/// ```no_run
/// use head_scanner::acoustics::{generate_sofa_bem, BEMConfig};
/// use head_scanner::Mesh;
/// use nalgebra::Point3;
/// use std::path::Path;
///
/// # fn main() -> head_scanner::ScannerResult<()> {
/// let mesh = Mesh::from_obj("head.obj")?;
///
/// // Define source positions (72 azimuth × 36 elevation = 2,592 positions)
/// let mut sources = Vec::new();
/// for az in 0..72 {
///     for el in 0..36 {
///         let azimuth = -180.0 + (az as f32 * 5.0);
///         let elevation = -45.0 + (el as f32 * 2.5);
///         // Convert to Cartesian...
///         sources.push(Point3::new(0.0, 0.0, 100.0)); // Placeholder
///     }
/// }
///
/// generate_sofa_bem(
///     &mesh,
///     "output.sofa",
///     44100.0,
///     BEMConfig::default(),
///     &sources,
///     Path::new("/tmp/bem_work"),
/// )?;
/// # Ok(())
/// # }
/// ```
#[cfg(feature = "bem")]
pub fn generate_sofa_bem(
    mesh: &crate::Mesh,
    output_path: &str,
    sample_rate: f32,
    config: BEMConfig,
    source_positions: &[nalgebra::Point3<f32>],
    work_dir: &std::path::Path,
) -> crate::ScannerResult<()> {
    use crate::error::ScannerResult;
    use std::path::Path;

    log::info!("Starting SOFA generation pipeline (BEM)");
    log::warn!(
        "BEM simulation can take many hours (estimated: {:.1}h)",
        estimate_bem_time(
            source_positions.len(),
            config.num_frequencies,
            mesh.vertex_count()
        )
    );

    // Create work directory
    std::fs::create_dir_all(work_dir).map_err(|e| {
        crate::error::ScannerError::IoError(format!("Failed to create work dir: {}", e))
    })?;

    // Step 1: Create acoustic model
    log::info!("Step 1/5: Analyzing head geometry...");
    let acoustic_model = AcousticHeadModel::from_mesh(mesh)?;
    log::info!(
        "  Interaural distance: {:.1}cm",
        acoustic_model.interaural_distance()
    );

    // Step 2: Export mesh for BEM
    log::info!("Step 2/5: Exporting mesh for BEM solver...");
    let mesh_path = work_dir.join("head_mesh.obj");
    let bem_solver = BEMSolver::new(config);
    bem_solver.export_mesh_for_bem(mesh, &mesh_path)?;

    // Step 3: Generate BEM configuration
    log::info!("Step 3/5: Generating BEM configuration...");
    let config_path = work_dir.join("bem_config.txt");
    bem_solver.generate_bem_config(&acoustic_model, source_positions, &config_path)?;

    // Step 4: Run BEM simulation
    log::info!("Step 4/5: Running BEM simulation (this will take hours)...");
    let output_dir = work_dir.join("bem_output");
    std::fs::create_dir_all(&output_dir).map_err(|e| {
        crate::error::ScannerError::IoError(format!("Failed to create output dir: {}", e))
    })?;

    bem_solver.run_bem_simulation(&mesh_path, &config_path, &output_dir)?;

    // Step 5: Import results and write SOFA
    log::info!("Step 5/5: Importing BEM results and converting to SOFA...");
    let (imported_positions, impulse_responses) =
        bem_solver.import_bem_results(&output_dir, sample_rate)?;

    let sofa_writer = SOFAWriter::new(output_path);
    sofa_writer.write_sofa(
        &acoustic_model,
        &imported_positions,
        &impulse_responses,
        sample_rate,
    )?;

    log::info!("✓ BEM-based SOFA generation complete!");
    log::info!("  Output: {}", output_path);
    log::info!("  Positions: {}", imported_positions.len());
    log::info!("  Sample rate: {} Hz", sample_rate);
    log::info!(
        "  IR length: {} samples ({:.1}ms)",
        impulse_responses[0][0].len(),
        impulse_responses[0][0].len() as f32 / sample_rate * 1000.0
    );

    Ok(())
}

/// Generate SOFA file from mesh using BEM solver (stub when feature disabled)
#[cfg(not(feature = "bem"))]
pub fn generate_sofa_bem(
    _mesh: &crate::Mesh,
    _output_path: &str,
    _sample_rate: f32,
    _config: BEMConfig,
    _source_positions: &[nalgebra::Point3<f32>],
    _work_dir: &std::path::Path,
) -> crate::ScannerResult<()> {
    Err(crate::error::ScannerError::InvalidConfig(
        "BEM support not enabled. Rebuild with --features bem".to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_exports() {
        // Just verify that all types are exported
        let _ = std::mem::size_of::<AcousticHeadModel>();
        let _ = std::mem::size_of::<AnalyticalHRTF>();
        let _ = std::mem::size_of::<SOFAWriter>();
        let _ = std::mem::size_of::<BEMSolver>();
    }
}
