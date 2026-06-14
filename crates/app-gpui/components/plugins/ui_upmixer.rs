//! Upmixer Plugin UI Component
//!
//! Layout:
//! - Main area: Channel Gains (4 faders) + Spatial Controls (4 faders) side by side
//! - Tab bar: LFE & Bass | Dialogue | Ambient | Height | HR Direct | Decorrelation | Analysis | Diagnostic
//! - Tab content: Expandable panel for the selected tab

mod misc;
mod render;
mod types;

pub use render::*;
pub use types::*;
