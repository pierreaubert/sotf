//! Room reflection compensation for the XTC plugin.
//!
//! Adds early reflection awareness via two modes:
//! 1. **Image source model** — analytical first-order reflections from a rectangular room
//! 2. **Measured room IR** — loads a WAV impulse response file
//!
//! Reflections are integrated before the matrix inversion step in filter computation.
//! Each reflection adds a delayed, attenuated, head-shadowed contribution to the
//! ipsilateral and contralateral transfer functions. The XTC inverse then naturally
//! compensates for reflections.

mod build;
mod compute;
mod misc;
mod types;

pub(crate) use build::*;
#[cfg(test)]
pub(crate) use compute::{compute_image_sources, compute_reflection_beta_boost};
#[cfg(test)]
pub(crate) use misc::air_absorption;
pub(crate) use types::*;
