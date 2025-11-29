//! Scale module for mapping data domains to visual ranges
//!
//! Scales are the foundation of data visualization, providing functions to map
//! abstract data values to visual representations.
//!
//! ## Continuous Scales
//!
//! - [`LinearScale`]: Map continuous numeric domain to continuous range
//! - [`LogScale`]: Logarithmic mapping for exponential data
//! - [`PowScale`]: Power/exponential mapping (includes sqrt)
//! - [`SymlogScale`]: Symmetric log that handles negative values and zero
//!
//! ## Quantizing Scales
//!
//! - [`QuantizeScale`]: Map continuous domain to discrete range (uniform segments)
//! - [`QuantileScale`]: Map sampled domain to discrete range (equal-count segments)
//! - [`ThresholdScale`]: Map continuous domain to discrete range (explicit thresholds)
//!
//! ## Categorical Scales
//!
//! - [`OrdinalScale`]: Map discrete domain values to discrete range values
//! - [`BandScale`]: Divide range into bands for categorical data (bar charts)
//! - [`PointScale`]: Position categorical data at points (scatter plots)

mod traits;
mod linear;
mod log;
mod pow;
mod symlog;
mod quantize;
mod quantile;
mod threshold;
mod ticks;
mod ordinal;

pub use traits::Scale;
pub use linear::LinearScale;
pub use log::LogScale;
pub use pow::{PowScale, SqrtScale, sqrt_scale};
pub use symlog::SymlogScale;
pub use quantize::QuantizeScale;
pub use quantile::QuantileScale;
pub use threshold::ThresholdScale;
pub use ticks::{nice_number, generate_linear_ticks, generate_log_ticks};
pub use ordinal::{OrdinalScale, BandScale, PointScale};
