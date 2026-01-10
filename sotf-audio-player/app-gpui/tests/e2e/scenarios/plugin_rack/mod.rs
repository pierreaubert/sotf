//! Plugin UI tests.
//!
//! Tests for all audio plugin interfaces:
//! - Processing plugins (EQ, compressor, limiter, etc.)
//! - Analyzer plugins (spectrum, loudness monitor)
//! - Channel/Spatial plugins (mute/solo, upmixer, matrix, binaural)
//! - Effects plugins (delay, convolution, crossover, XTC)
//! - Advanced plugins (multiband compressor/expander, denoiser, PND, A/B compare)
//! - Plugin chain management (rack, graph)

pub mod all_plugins;
pub mod ab_compare;
pub mod binaural;
pub mod compressor;
pub mod convolution;
pub mod crossover;
pub mod delay;
pub mod denoiser;
pub mod eq;
pub mod expander;
pub mod gain;
pub mod gate;


pub mod limiter;
pub mod loudness;
pub mod loudness_monitor;
pub mod matrix;
pub mod matrix_channel_propagation;
pub mod mb_compressor;
pub mod mb_expander;
pub mod mute_solo;
pub mod pnd;
pub mod rack;
pub mod spectrum;
pub mod upmixer;
pub mod workflow;
pub mod xtc;
