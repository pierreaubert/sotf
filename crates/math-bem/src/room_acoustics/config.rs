//! BEM-specific configuration extensions for room acoustics simulations
//!
//! This module provides BEM-specific configuration that extends the common configuration
//! from math-xem-common. For the base configuration types, see math_audio_xem_common::config.

// Re-export the common configuration types
pub use math_audio_xem_common::{
    CrossoverConfig, DirectivityConfig, FmmConfig, FrequencyConfig, GmresConfig, IluConfig,
    MetadataConfig, Point3DConfig, RoomConfig, RoomGeometryConfig, SolverConfig, SourceConfig,
    VisualizationConfig,
};

// Any BEM-specific configuration extensions would go here
