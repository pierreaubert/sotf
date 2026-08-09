//! Level meter UI components and logic.
//!
//! This module consolidates all level meter functionality:
//! - GPU-accelerated level meter element (`LevelMeterElement`)
//! - Level meter group rendering (with M/S/D buttons)
//! - Level meter panel rendering (for queue screen)
//! - App methods for level meter group management (mute/solo/dim)

mod level_meter_manager;
mod misc;
mod render;

pub use level_meter_manager::*;
pub use render::*;
