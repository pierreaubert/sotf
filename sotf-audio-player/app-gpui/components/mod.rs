pub mod album_card;
mod dialogs;
pub mod graphs;
mod footer;
mod header;
pub mod icon;
pub mod image_cache;
pub mod optimization_forms;
pub mod eq_curve;

// Re-export Icon types for convenience
pub use eq_curve::{CompactEQCurve, EQCurveColors, EQCurveElement};
pub use icon::{Icon, IconName, IconSize};
// Level meter and spectrum types are now in crate::plugins module
pub use crate::plugins::{LevelMeterElement, MeterColors, MeterData, SpectrumColors, SpectrumElement};
// Re-export plugin-related functions for backward compatibility
pub use crate::plugins::{get_param_count, render_plugin_content};
