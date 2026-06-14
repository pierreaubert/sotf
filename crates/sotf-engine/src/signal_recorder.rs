//! Signal generation and recording module
//!
//! This module provides functionality to generate test signals, play them back,
//! record the output, and analyze the results.

mod build;
mod choose;
mod consts;
mod generate;
mod measurement;
mod misc;
mod probe;
mod record;
mod recording_session;
mod run;
mod signal_params;
mod signal_type;
#[cfg(test)]
mod tests;
mod types;
mod write;

pub use consts::*;
pub use generate::*;
pub use misc::*;
pub use probe::*;
pub use record::*;
pub use recording_session::*;
pub use run::*;
pub use signal_params::*;
pub use signal_type::*;
pub use types::*;
pub use write::*;
