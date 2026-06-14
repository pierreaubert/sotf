//! XTC Plugin Validation Infrastructure
//!
//! Provides functions to measure and validate:
//! - ITD accuracy vs Woodworth formula
//! - ILD accuracy vs KEMAR/analytical models
//! - Cancellation depth per frequency
//! - Spatial cue preservation
//! - Filter stability
//!
//! Usage:
//! ```ignore
//! use plugin_xtc::validation::{run_validation, ValidationResult};
//!
//! let params = XtcPluginParams::default();
//! let results = run_validation(&params, 48000);
//!
//! for result in &results {
//!     if !result.passed {
//!         println!("FAILED: {} (expected {}, got {})",
//!             result.metric_name, result.expected, result.measured);
//!     }
//! }
//! ```

mod measure;
mod misc;
mod reference;
mod run;
#[cfg(test)]
mod tests;
mod types;
mod validation_report;
mod validation_result;

pub use measure::*;
pub use misc::*;
pub use reference::*;
pub use run::*;
pub use types::*;
pub use validation_report::*;
pub use validation_result::*;
