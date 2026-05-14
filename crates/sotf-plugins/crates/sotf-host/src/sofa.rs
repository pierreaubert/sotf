//! Compatibility re-exports for SOFA/HRTF loading.
//!
//! The implementation lives in `sofa-reader` so crates that only need HRTF
//! file parsing do not have to depend on the full plugin host crate.

pub use sofa_reader::{CoordinateSystem, HrtfData, SofaFile, SourcePosition};
