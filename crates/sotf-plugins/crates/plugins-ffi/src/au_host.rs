//! AU Host State — implements `PluginViewHost` for Audio Unit plugin views.
//!
//! Bridges GPUI plugin UI rendering to the AU parameter system.
//!
//! # Thread safety
//!
//! `AuHostState` does NOT hold a `PluginHandle` pointer. Instead it uses:
//! - **Reads**: An `AtomicParamCache` populated by Swift's `AUParameterTree` observer
//! - **Writes**: C callback function pointers routed through Swift's `AUParameterTree`
//!
//! # Parameter index mapping
//!
//! Band-based plugins (EQ, MultibandCompressor, etc.) have global params followed by
//! per-band params. Custom UIs use band-relative indices (`band_idx * params_per_band + field`).
//! `set_plugin_param` adds `global_param_count` to map from UI indices to AU cache indices.
//! Non-band plugins have `global_param_count = 0`, so the mapping is identity.

use crate::param_cache::AtomicParamCache;
use crate::parameter_map;
use gpui::prelude::*;
use gpui::*;
use gpui_audio_kit::PotentiometerSize;
use math_audio_iir_fir::BiquadFilterType;
use plugins_gpui::common::render_knob_sized;
use plugins_gpui::{PluginViewHost, PluginViewTheme};
use std::sync::Arc;

/// C function pointer type for parameter writes.
pub type SetParamCallback = extern "C" fn(*mut std::ffi::c_void, usize, f64);

/// C function pointer type for parameter reset (to default).
pub type ResetParamCallback = extern "C" fn(*mut std::ffi::c_void, usize);

/// Band layout info for band-based plugins.
struct BandLayout {
    /// Number of global params before band params.
    global_param_count: usize,
    /// Number of params per band.
    params_per_band: usize,
    /// Maximum number of bands.
    max_bands: usize,
}

/// Root GPUI entity for AU plugin views.
pub struct AuHostState {
    cache: Arc<AtomicParamCache>,
    set_param_cb: SetParamCallback,
    reset_param_cb: ResetParamCallback,
    cb_userdata: *mut std::ffi::c_void,
    plugin_type: String,
    param_snapshot: Vec<f64>,
    /// Band layout for band-based plugins (EQ, multiband, etc.), None for simple plugins.
    band_layout: Option<BandLayout>,
    selected_param: usize,
    selected_band: usize,
    selected_channel: usize,
    editing: bool,
    theme: PluginViewTheme,
    self_entity: Option<Entity<Self>>,
    is_dragging: bool,
    drag_plugin_idx: usize,
    drag_start_y: f32,
    drag_start_value: f64,
    drag_min: f64,
    drag_max: f64,
}

impl AuHostState {
    pub fn new(
        cache: Arc<AtomicParamCache>,
        set_param_cb: SetParamCallback,
        reset_param_cb: ResetParamCallback,
        cb_userdata: *mut std::ffi::c_void,
        plugin_type: String,
    ) -> Self {
        let param_count = cache.len();
        let mut param_snapshot = vec![0.0; param_count];
        cache.read_all(&mut param_snapshot);

        // Determine band layout from ParamSpec metadata
        let global_specs = parameter_map::global_param_specs(&plugin_type);
        let band_layout =
            parameter_map::band_template_info(&plugin_type).map(|(params_per_band, max_bands)| {
                BandLayout {
                    global_param_count: global_specs.len(),
                    params_per_band,
                    max_bands,
                }
            });

        Self {
            cache,
            set_param_cb,
            reset_param_cb,
            cb_userdata,
            plugin_type,
            param_snapshot,
            band_layout,
            selected_param: 0,
            selected_band: 0,
            selected_channel: 0,
            editing: false,
            theme: PluginViewTheme::default_dark(),
            self_entity: None,
            is_dragging: false,
            drag_plugin_idx: 0,
            drag_start_y: 0.0,
            drag_start_value: 0.0,
            drag_min: 0.0,
            drag_max: 0.0,
        }
    }

    pub fn set_entity(&mut self, entity: Entity<Self>) {
        self.self_entity = Some(entity);
    }

    fn refresh_params(&mut self) {
        self.cache.read_all(&mut self.param_snapshot);
    }

    /// Convert a UI band-relative param index to an absolute AU param index.
    /// For band-based plugins, adds the global param offset.
    /// For non-band plugins, returns the index unchanged.
    fn ui_to_au_param_index(&self, ui_idx: usize) -> usize {
        match &self.band_layout {
            Some(layout) => layout.global_param_count + ui_idx,
            None => ui_idx,
        }
    }

    /// Get current band count from the first global param (which is typically `max_filters`/`num_bands`).
    fn band_count(&self) -> usize {
        match &self.band_layout {
            Some(layout) if !self.param_snapshot.is_empty() => {
                (self.param_snapshot[0] as usize).min(layout.max_bands)
            }
            _ => 0,
        }
    }

    /// Set an AU parameter via the callback and update local snapshot.
    fn set_au_param(&mut self, au_idx: usize, value: f64) {
        (self.set_param_cb)(self.cb_userdata, au_idx, value);
        if au_idx < self.param_snapshot.len() {
            self.param_snapshot[au_idx] = value;
        }
    }

    fn build_eq_bands(&self) -> Vec<sotf_plugin_eq::ui::EqBandView> {
        let layout = match &self.band_layout {
            Some(l) => l,
            None => return Vec::new(),
        };
        let mut bands = Vec::new();
        let count = self.band_count();

        for band in 0..count {
            let base = layout.global_param_count + band * layout.params_per_band;
            if base + layout.params_per_band > self.param_snapshot.len() {
                break;
            }

            let frequency = self.param_snapshot[base];
            let q = self.param_snapshot[base + 1];
            let gain_db = self.param_snapshot[base + 2];
            let filter_type_raw = self.param_snapshot[base + 3] as u32;
            let order = self.param_snapshot[base + 4] as usize;

            let filter_type = match filter_type_raw {
                0 => BiquadFilterType::Peak,
                1 => BiquadFilterType::Lowshelf,
                2 => BiquadFilterType::Highshelf,
                3 => BiquadFilterType::Lowpass,
                4 => BiquadFilterType::Highpass,
                5 => BiquadFilterType::Bandpass,
                6 => BiquadFilterType::Notch,
                other => {
                    eprintln!("Unknown EQ filter type index: {other}, defaulting to Peak");
                    BiquadFilterType::Peak
                }
            };

            bands.push(sotf_plugin_eq::ui::EqBandView {
                filter_type,
                frequency,
                q,
                gain_db,
                order,
                sample_rate: sotf_host::DEFAULT_PREVIEW_SAMPLE_RATE,
                muted: false,
                solo: false,
            });
        }

        bands
    }

    /// Render a generic UI with a grid of knobs.
    fn render_generic_knob_grid(&self, entity: Entity<Self>) -> AnyElement {
        let theme = &self.theme;
        let knobs_per_row = 4;

        let header = div().w_full().flex().justify_center().py(px(8.0)).child(
            div()
                .text_color(rgb(0xffffff))
                .text_xl()
                .child(format!("SOTF: {}", self.plugin_type)),
        );

        let mut rows = div().w_full().flex().flex_col().gap(px(8.0)).px(px(12.0));
        let mut row = div()
            .w_full()
            .flex()
            .flex_row()
            .gap(px(8.0))
            .justify_center();
        let mut col_in_row = 0;

        for i in 0..self.param_snapshot.len() {
            let meta = self.cache.meta(i);
            let name = meta.map(|m| m.name.as_str()).unwrap_or("");
            if name.is_empty() {
                continue;
            }
            let unit = meta.map(|m| m.unit.as_str()).unwrap_or("");
            let min = meta.map(|m| m.min_value).unwrap_or(0.0);
            let max = meta.map(|m| m.max_value).unwrap_or(1.0);
            let value = self.param_snapshot.get(i).copied().unwrap_or(0.0);

            let knob = render_knob_sized::<AuHostState>(
                entity.clone(),
                0,
                name,
                value,
                min,
                max,
                unit,
                i, // AU cache index (non-band: identity, band: passed through directly)
                self.selected_param,
                self.editing,
                None,
                PotentiometerSize::Md,
                theme,
            );

            row = row.child(knob);
            col_in_row += 1;

            if col_in_row >= knobs_per_row {
                rows = rows.child(row);
                row = div()
                    .w_full()
                    .flex()
                    .flex_row()
                    .gap(px(8.0))
                    .justify_center();
                col_in_row = 0;
            }
        }

        if col_in_row > 0 {
            rows = rows.child(row);
        }

        div()
            .size_full()
            .bg(rgb(0x1a1a2e))
            .flex()
            .flex_col()
            .items_center()
            .overflow_hidden()
            .child(header)
            .child(rows)
            .into_any_element()
    }
}

impl PluginViewHost for AuHostState {
    fn set_plugin_param(&mut self, _plugin_idx: usize, param_idx: usize, value: f64) {
        let au_idx = self.ui_to_au_param_index(param_idx);
        self.set_au_param(au_idx, value);
    }

    fn reset_plugin_param(&mut self, _plugin_idx: usize, param_idx: usize) {
        let au_idx = self.ui_to_au_param_index(param_idx);
        (self.reset_param_cb)(self.cb_userdata, au_idx);
    }

    fn set_editing_plugin(&mut self, _plugin_idx: usize) {
        self.editing = true;
    }

    fn set_selected_param(&mut self, _plugin_idx: usize, param_idx: usize) {
        self.selected_param = param_idx;
    }

    fn on_knob_drag_start(
        &mut self,
        plugin_idx: usize,
        _param_idx: usize,
        start_y: f32,
        start_value: f64,
        min: f64,
        max: f64,
    ) {
        self.is_dragging = true;
        self.drag_plugin_idx = plugin_idx;
        self.drag_start_y = start_y;
        self.drag_start_value = start_value;
        self.drag_min = min;
        self.drag_max = max;
    }

    fn on_knob_drag_end(&mut self) {
        self.is_dragging = false;
    }

    fn knob_drag_state(&self) -> (bool, usize, f32, f64, f64, f64) {
        (
            self.is_dragging,
            self.drag_plugin_idx,
            self.drag_start_y,
            self.drag_start_value,
            self.drag_min,
            self.drag_max,
        )
    }

    fn set_selected_band(&mut self, _plugin_idx: usize, band: usize) {
        self.selected_band = band;
    }

    fn set_selected_channel(&mut self, _plugin_idx: usize, channel: usize) {
        self.selected_channel = channel;
    }

    fn add_band(&mut self, _plugin_idx: usize) {
        let (global_count, ppb, max_bands) = match &self.band_layout {
            Some(l) => (l.global_param_count, l.params_per_band, l.max_bands),
            None => return,
        };
        let current = self.band_count();
        if current >= max_bands {
            return;
        }
        // Collect defaults before mutating
        let base = global_count + current * ppb;
        let defaults: Vec<(usize, f64)> = (0..ppb)
            .map(|f| {
                let au_idx = base + f;
                let default = self
                    .cache
                    .meta(au_idx)
                    .map(|m| m.default_value)
                    .unwrap_or(0.0);
                (au_idx, default)
            })
            .collect();
        // Set band count
        self.set_au_param(0, (current + 1) as f64);
        // Initialize the new band
        for (idx, val) in defaults {
            self.set_au_param(idx, val);
        }
        self.selected_band = current;
    }

    fn remove_band(&mut self, _plugin_idx: usize, band_idx: usize) {
        let (global_count, ppb) = match &self.band_layout {
            Some(l) => (l.global_param_count, l.params_per_band),
            None => return,
        };
        let current = self.band_count();
        if current == 0 || band_idx >= current {
            return;
        }
        // Collect shifted values first to avoid borrow conflict
        let mut updates: Vec<(usize, f64)> = Vec::new();
        for i in band_idx..current.saturating_sub(1) {
            let src_base = global_count + (i + 1) * ppb;
            let dst_base = global_count + i * ppb;
            for f in 0..ppb {
                let val = self
                    .param_snapshot
                    .get(src_base + f)
                    .copied()
                    .unwrap_or(0.0);
                updates.push((dst_base + f, val));
            }
        }
        for (idx, val) in updates {
            self.set_au_param(idx, val);
        }
        self.set_au_param(0, (current - 1) as f64);
        if self.selected_band >= current - 1 {
            self.selected_band = (current - 1).saturating_sub(1);
        }
    }

    fn toggle_band_mute(&mut self, _plugin_idx: usize, _band_idx: usize) {
        // Mute/solo are not exposed as AU parameters — no-op in AU context.
    }

    fn toggle_band_solo(&mut self, _plugin_idx: usize, _band_idx: usize) {
        // Solo is not exposed as an AU parameter — no-op in AU context.
    }
}

impl Render for AuHostState {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        self.refresh_params();

        let entity = match self.self_entity.clone() {
            Some(e) => e,
            None => {
                return div()
                    .size_full()
                    .bg(rgb(0x1a1a2e))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(rgb(0xffffff))
                    .child("SOTF: initializing...")
                    .into_any_element();
            }
        };

        match self.plugin_type.as_str() {
            "EQ" | "eq" => {
                let bands = self.build_eq_bands();
                let selected_band = self.selected_band.min(bands.len().saturating_sub(1));

                sotf_plugin_eq::ui::render_eq_plugin(
                    entity,
                    0,
                    sotf_plugin_eq::ui::EqRenderState {
                        channels: 2,
                        filters: &bands,
                        channel_filters: &None,
                        per_channel_mode: false,
                        is_editing: self.editing,
                        selected_param: self.selected_param,
                        selected_band_idx: selected_band,
                        selected_eq_channel: self.selected_channel,
                        midi_overlay: None,
                    },
                    &self.theme,
                )
                .into_any_element()
            }
            _ => self.render_generic_knob_grid(entity),
        }
    }
}
