//! Plugin UI theming.
//!
//! Two unrelated concerns coexist here:
//!
//! - **Meter / LUFS / TruePeak styling** ([`meter`]) derives from the global
//!   app [`crate::theme::Theme`]. Used by every plugin that draws meters or
//!   level bars; semantic colors (warning / clip / info) follow the global
//!   palette so they remain consistent app-wide.
//!
//! - **Plugin chassis themes** ([`plugin_theme`]) are a *replacement* visual
//!   layer applied to plugin chassis (background, panel, knob arc colors,
//!   typography). They are **not** derived from the global theme — they are
//!   standalone presets the user picks per-rack and optionally per-plugin.
//!
//! Most plugins use only the meter side. Loudness and Upmixer are the first
//! adopters of plugin chassis themes; other plugins continue rendering with
//! the global app theme until they are migrated.

pub mod brutalist;
pub mod graphite;
pub mod meter;
pub mod plugin_theme;
pub mod studio_cream;

pub use meter::{LufsConfig, MeterTheme, TruePeakConfig};
pub use plugin_theme::{
    PluginTheme, PluginThemeId, RackThemeState, plugin_theme_id_for_app_theme, resolve_plugin_theme,
};
