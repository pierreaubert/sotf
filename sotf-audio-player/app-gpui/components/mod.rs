pub mod album_card;
mod dialogs;
mod footer;
pub mod eq_curve;
pub mod graphs;
mod header;
pub mod icon;
pub mod image_cache;
pub mod measure_dialog;
pub mod optimization_forms;

// Re-export Icon types for convenience
pub use eq_curve::{CompactEQCurve, EQCurveColors, EQCurveElement};
pub use icon::{Icon, IconName, IconSize};
// Level meter and spectrum types are now in crate::plugins module
pub use crate::plugins::{
    LevelMeterElement, MeterColors, MeterData, SpectrumColors, SpectrumElement,
};
// Re-export plugin-related functions for backward compatibility
pub use crate::plugins::{get_param_count, render_plugin_content};
