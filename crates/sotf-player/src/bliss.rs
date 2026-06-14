//! Audio feature analysis for music similarity
//!
//! This module provides audio analysis using a pure Rust implementation
//! (math-dsp audio_features) with a Symphonia-based decoder. It extracts
//! features that can be used to compute similarity between tracks for
//! intelligent playlist generation.
//!
//! # Features extracted (23 total, bliss v2 compatible)
//! - Tempo (BPM)
//! - Zero-crossing rate (ZCR)
//! - Spectral centroid (mean/std deviation)
//! - Spectral rolloff (mean/std deviation)
//! - Spectral flatness (mean/std deviation)
//! - Loudness (mean/std deviation)
//! - Chroma interval features (13)

mod bliss_analysis;
mod bliss_scan_manager;
mod bliss_scanner;
mod misc;
mod types;

pub use bliss_analysis::*;
pub use bliss_scan_manager::*;
pub use bliss_scanner::*;
pub use misc::*;
pub use types::*;
