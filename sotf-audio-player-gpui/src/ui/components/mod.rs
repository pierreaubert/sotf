pub mod album_card;
mod dialogs;
pub mod graphs;
mod footer;
mod header;
pub mod host;
pub mod icon;
pub mod image_cache;
pub mod optimization_forms;
pub mod plugins;
pub mod eq_curve;

// Re-export Icon types for convenience
pub use eq_curve::{CompactEQCurve, EQCurveColors, EQCurveElement};
pub use icon::{Icon, IconName, IconSize};
// Level meter and spectrum types are now in plugins module
pub use plugins::{LevelMeterElement, MeterColors, MeterData, SpectrumColors, SpectrumElement};
