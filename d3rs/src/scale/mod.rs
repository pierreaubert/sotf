//! Scale module for mapping data domains to visual ranges
//!
//! Scales are the foundation of data visualization, providing functions to map
//! abstract data values to visual representations.

mod traits;
mod linear;
mod log;
mod ticks;

pub use traits::Scale;
pub use linear::LinearScale;
pub use log::LogScale;
pub use ticks::{nice_number, generate_linear_ticks, generate_log_ticks};
