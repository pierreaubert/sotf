//! Custom View Identification
//!
//! Platform-agnostic identifiers for custom plugin visualizations.
//! Each plugin can declare `VizSlot::Custom { name }` in its layout;
//! platform renderers (GPUI, SwiftUI) resolve the name to an actual
//! rendering implementation via a registry keyed by `CustomViewId`.
//!
//! This module contains only identifiers — no rendering code.

/// Identifies a custom visualization that a platform renderer can resolve.
///
/// Plugins reference these via `VizSlot::Custom { name }` in their `PluginLayout`.
/// Platform renderers maintain a registry mapping `CustomViewId` → render function.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CustomViewId {
    /// Plugin type key (e.g., "eq", "compressor").
    pub plugin_type: &'static str,
    /// View name within that plugin (e.g., "frequency_response", "transfer_curve").
    pub view_name: &'static str,
}

/// Well-known custom view IDs.
///
/// Plugins reference these in their `LAYOUT` via `VizSlot::Custom { name }`.
/// Platform renderers register implementations for these IDs.
pub mod views {
    use super::CustomViewId;

    /// EQ: interactive frequency response graph with draggable filter points.
    pub const EQ_FREQUENCY_RESPONSE: CustomViewId = CustomViewId {
        plugin_type: "eq",
        view_name: "frequency_response",
    };

    /// Spectrum analyzer: live FFT spectrum display.
    pub const SPECTRUM_DISPLAY: CustomViewId = CustomViewId {
        plugin_type: "spectrum_analyzer",
        view_name: "spectrum_display",
    };

    /// Compressor/limiter/gate/expander: input→output transfer curve.
    pub const TRANSFER_CURVE: CustomViewId = CustomViewId {
        plugin_type: "dynamics",
        view_name: "transfer_curve",
    };

    /// Matrix: channel routing grid editor.
    pub const MATRIX_GRID: CustomViewId = CustomViewId {
        plugin_type: "matrix",
        view_name: "matrix_grid",
    };

    /// Channel mute/solo: per-channel toggle controls.
    pub const CHANNEL_CONTROLS: CustomViewId = CustomViewId {
        plugin_type: "channel_mute_solo",
        view_name: "channel_controls",
    };

    /// Multiband compressor/expander: band selector tabs.
    pub const BAND_SELECTOR: CustomViewId = CustomViewId {
        plugin_type: "multiband",
        view_name: "band_selector",
    };

    /// Loudness monitor: EBU R128 meters.
    pub const LOUDNESS_METERS: CustomViewId = CustomViewId {
        plugin_type: "loudness_monitor",
        view_name: "loudness_meters",
    };
}
