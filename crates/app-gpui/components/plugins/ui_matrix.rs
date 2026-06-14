//! Matrix Plugin UI Component
//!
//! Channel routing/mixing matrix with:
//! - Interactive grid visualization (inputs as columns, outputs as rows)
//! - dB display for gain values
//! - Click to toggle, scroll to adjust
//! - Preset buttons (Identity, Swap L/R, Mono Mix)

mod consts;
mod misc;
mod render;
mod types;

pub use render::*;
pub use types::*;
