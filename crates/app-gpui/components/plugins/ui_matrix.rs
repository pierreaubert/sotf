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

#[doc(hidden)]
pub use misc::{checked_matrix_cell_index, matrix_settings_mut_by_instance_id};
pub use render::*;
pub use types::*;
