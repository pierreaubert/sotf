//! Render Plan — testable snapshot of layout decisions
//!
//! Combines `PluginParamDef::PARAMS`, `PluginParamDef::LAYOUT`, and `solve_layout()`
//! into a serializable `PluginRenderPlan` that captures every structural decision
//! the layout renderer will make — without any GPUI dependency.
//!
//! Used for snapshot testing: 33 plugins × 10 device profiles = ~330 JSON snapshots.
//! Any layout regression at any screen size produces a diff.

mod build;
mod control_plan;
mod format;
mod misc;
mod types;

pub use build::*;
pub use types::*;
