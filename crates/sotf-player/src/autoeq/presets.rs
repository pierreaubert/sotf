//! EQ optimization presets for non-expert users.
//!
//! Provides named parameter bundles that map a single user choice to a complete
//! optimizer configuration. Three tiers of UI detail control which parameters
//! are visible.

mod consts;
mod eq_preset;
mod eq_workflow;
mod field;
mod misc;
mod quality;
#[cfg(test)]
mod tests;
mod types;

pub use consts::*;
pub use eq_preset::*;
pub use eq_workflow::*;
pub use field::*;
pub use misc::*;
pub use quality::*;
pub use types::*;
