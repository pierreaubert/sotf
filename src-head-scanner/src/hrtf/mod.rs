//! HRTF post-processing for Mesh2HRTF output
//!
//! This module provides functionality for processing NumCalc BEM simulation output:
//! - Parse NumCalc be.out files (complex pressure and velocity data)
//! - Reference HRTFs to head center
//! - Compute HRIRs via inverse FFT
//! - Export to SOFA format
//!
//! # Workflow
//!
//! ```text
//! 1. Run NumCalc BEM simulation (via src-bem FFI)
//! 2. Parse be.out files → HrtfData
//! 3. Reference to head center → Referenced HRTFs
//! 4. Inverse FFT → HRIRs
//! 5. Export to SOFA files
//! ```
//!
//! # Example
//!
//! ```rust,no_run
//! use head_scanner::hrtf::*;
//!
//! // Parse NumCalc output
//! let parser = NumCalcParser::new("/path/to/project")?;
//! let data = parser.parse_source(0)?;  // Parse source 1
//!
//! // Reference to head center
//! let referenced = data.reference_to_head_center()?;
//!
//! // Compute HRIRs
//! let hrir = referenced.compute_hrir(48000.0)?;
//!
//! // Export to SOFA
//! hrir.write_sofa("/path/to/output.sofa")?;
//! # Ok::<(), anyhow::Error>(())
//! ```

pub mod types;
pub mod numcalc_parser;
pub mod hrir;
pub mod sofa_writer;

// Re-exports
pub use types::*;
pub use numcalc_parser::NumCalcParser;
pub use hrir::{
    apply_blackman_window, apply_hamming_window, apply_hann_window, compute_hrir,
};
pub use sofa_writer::{
    cartesian_to_spherical, spherical_to_cartesian, CoordinateSystem, SofaMetadata, SofaWriter,
};
