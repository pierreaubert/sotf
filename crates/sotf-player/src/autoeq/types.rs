//! AutoEQ Types
//!
//! Shared type definitions for speaker EQ optimization.

use serde::{Deserialize, Serialize};

// Re-export CrossoverType from autoeq — single source of truth.
pub use autoeq::CrossoverType;

// ============================================================================
// Speaker Configuration
// ============================================================================

/// Speaker configuration type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SpeakerConfigType {
    /// Single measurement (simple speaker)
    #[default]
    Single,
    /// Multiple drivers with crossover
    MultiDriver,
}
