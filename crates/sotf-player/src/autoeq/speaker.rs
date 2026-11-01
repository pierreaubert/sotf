//! Speaker EQ optimization
//!
//! Provides thin wrappers around the autoeq library for speaker equalization.
//! Most functionality is delegated to the library.

pub use autoeq_optim::CrossoverType;
pub use autoeq_optim::de::CallbackAction;
pub use autoeq_workflow::Cea2034Data;
pub use autoeq_workflow::OptimizationOutput;
pub use autoeq_workflow::ProgressCallbackConfig;
pub use autoeq_workflow::ProgressUpdate;
pub use autoeq_workflow::SpeakerOptResult;
pub use autoeq_workflow::VisualizationCurves;

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
