//! Crosstalk cancellation filter computation.
//!
//! Contains all DSP math for computing XTC filters in the frequency domain,
//! including head shadowing models, regularization, and spectral normalization.

mod apply;
mod build;
mod compute;
mod head;
mod misc;
mod pinna;
mod resonance;
mod types;
mod xtc_filters;

pub(crate) use compute::*;
pub(crate) use head::*;
pub(crate) use misc::*;
pub(crate) use pinna::*;
pub(crate) use types::*;
pub(crate) use xtc_filters::*;
