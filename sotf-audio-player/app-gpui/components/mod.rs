pub mod album_card;
// Re-export autoeq as optimization_params for backward compatibility
pub mod autoeq;
pub use autoeq as optimization_params;
mod dialogs;
mod footer;
pub mod graphs;
mod header;
pub mod icon;
pub mod image_cache;

// Re-export Icon types for convenience
pub use icon::{Icon, IconName, IconSize};

// Level meter and spectrum types are now in crate::plugins module
pub use crate::plugins::{
    LevelMeterElement, MeterColors, MeterData, SpectrumColors, SpectrumElement,
};

// Re-export plugin-related functions for backward compatibility
pub use crate::plugins::{get_param_count, render_plugin_content};
