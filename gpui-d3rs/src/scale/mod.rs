//! Scale module for mapping data domains to visual ranges
//!
//! Scales are the foundation of data visualization, providing functions to map
//! abstract data values to visual representations.
//!
//! ## Continuous Scales
//!
//! - [`LinearScale`]: Map continuous numeric domain to continuous range
//! - [`LogScale`]: Logarithmic mapping for exponential data
//!
//! ## Categorical Scales
//!
//! - [`OrdinalScale`]: Map discrete domain values to discrete range values
//! - [`BandScale`]: Divide range into bands for categorical data (bar charts)
//! - [`PointScale`]: Position categorical data at points (scatter plots)

mod traits;
mod linear;
mod log;
mod ticks;
mod ordinal;

pub use traits::Scale;
pub use linear::LinearScale;
pub use log::LogScale;
pub use ticks::{nice_number, generate_linear_ticks, generate_log_ticks};
pub use ordinal::{OrdinalScale, BandScale, PointScale};
