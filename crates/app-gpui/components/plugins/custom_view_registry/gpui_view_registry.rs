use super::render::render_ab_compare;
use super::render::render_dynamic_eq;
use super::render::render_eq;
use super::render::render_external;
use super::render::render_fir_designer;
use super::render::render_linear_phase_eq;
use super::render::render_loudness;
use super::render::render_matrix;
use super::render::render_mb_compressor;
use super::render::render_mb_expander;
use super::render::render_mute_solo;
use super::render::render_spectrum;
use super::render::render_upmixer;
use super::types::CustomViewRenderFn;
use std::collections::HashMap;

/// Registry mapping plugin type keys to custom render functions.
pub struct GpuiViewRegistry {
    pub(super) views: HashMap<&'static str, CustomViewRenderFn>,
}

impl GpuiViewRegistry {
    /// Create a new registry with all known custom views registered.
    pub fn new() -> Self {
        let mut views: HashMap<&'static str, CustomViewRenderFn> = HashMap::new();

        views.insert("eq", render_eq);
        views.insert("dynamic_eq", render_dynamic_eq);
        views.insert("fir_designer", render_fir_designer);
        views.insert("linear_phase_eq", render_linear_phase_eq);
        views.insert("spectrum_analyzer", render_spectrum);
        views.insert("channel_mute_solo", render_mute_solo);
        views.insert("matrix", render_matrix);
        views.insert("loudness_monitor", render_loudness);
        views.insert("multiband_compressor", render_mb_compressor);
        views.insert("multiband_expander", render_mb_expander);
        views.insert("ab_compare", render_ab_compare);
        views.insert("upmixer", render_upmixer);
        views.insert("external", render_external);

        Self { views }
    }

    /// Look up a custom render function for a plugin type.
    pub fn get(&self, plugin_type_key: &str) -> Option<CustomViewRenderFn> {
        self.views.get(plugin_type_key).copied()
    }
}

impl Default for GpuiViewRegistry {
    fn default() -> Self {
        Self::new()
    }
}
