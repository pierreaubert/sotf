//! Speaker EQ optimization
//!
//! Provides thin wrappers around the autoeq library for speaker equalization.
//! Most functionality is delegated to the library.

pub use autoeq::CrossoverType;
pub use autoeq::de::CallbackAction;
pub use autoeq::{
    Cea2034Data, OptimizationOutput, ProgressCallbackConfig, ProgressUpdate, SpeakerOptResult,
    VisualizationCurves,
};

mod callback_config;
mod load;
mod optimize;
mod run;
mod speaker_optimization_config;
mod speaker_optimization_config_ext;
mod speaker_optimization_progress;
mod speaker_optimization_result;
#[cfg(test)]
mod tests;
mod types;

pub use callback_config::*;
pub use load::*;
pub use run::*;
pub use speaker_optimization_config::*;
pub use speaker_optimization_config_ext::*;
pub use speaker_optimization_progress::*;
pub use speaker_optimization_result::*;
pub use types::*;
