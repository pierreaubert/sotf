//! AU Host State — implements `PluginViewHost` for Audio Unit plugin views.
//!
//! Bridges GPUI plugin UI rendering to the AU parameter system.
//!
//! # Thread safety
//!
//! `AuHostState` does NOT hold a `PluginHandle` pointer. Instead it uses:
//! - **Reads**: An `AtomicParamCache` populated by Swift's `AUParameterTree` observer
//!   on the main thread. The GPUI render reads atomically — no locks, no races.
//! - **Writes**: A C function pointer callback that routes through Swift's
//!   `AUParameterTree`, which handles thread-safe parameter dispatch to the
//!   audio plugin via the AU framework's built-in synchronization.
//!
//! # Parameter index mapping (EQ)
//!
//! The EQ UI uses band-relative indices: `band_idx * 4 + field` (0=freq, 1=q, 2=gain, 3=type).
//! The AU parameter cache has absolute indices: 2 globals + 20*4 band params.
//! `set_plugin_param` adds the global offset (`GLOBAL_PARAM_COUNT`) when forwarding to AU.

use crate::param_cache::AtomicParamCache;
use gpui::prelude::*;
use gpui::*;
use math_audio_iir_fir::BiquadFilterType;
use plugins_gpui::{PluginViewHost, PluginViewTheme};
use std::sync::Arc;

/// C function pointer type for parameter writes.
pub type SetParamCallback = extern "C" fn(*mut std::ffi::c_void, usize, f64);

/// C function pointer type for parameter reset (to default).
pub type ResetParamCallback = extern "C" fn(*mut std::ffi::c_void, usize);

/// Number of global EQ params before band params.
const EQ_GLOBAL_PARAM_COUNT: usize = 2; // max_filters, tdf2
/// Number of params per EQ band.
const EQ_PARAMS_PER_BAND: usize = 4; // frequency, q, gain_db, filter_type

/// Root GPUI entity for AU plugin views.
pub struct AuHostState {
    /// Atomic parameter cache, shared with Swift's AU parameter observer.
    cache: Arc<AtomicParamCache>,
    /// Callback to set a parameter (routes through AUParameterTree).
    set_param_cb: SetParamCallback,
    /// Callback to reset a parameter to default.
    reset_param_cb: ResetParamCallback,
    /// Opaque userdata pointer passed to callbacks.
    cb_userdata: *mut std::ffi::c_void,
    /// Plugin type string.
    plugin_type: String,
    /// Local copy of param values for rendering (refreshed from cache each frame).
    param_snapshot: Vec<f64>,
    /// Currently selected parameter index.
    selected_param: usize,
    /// Currently selected EQ band.
    selected_band: usize,
    /// Selected EQ channel (for per-channel mode).
    selected_eq_channel: usize,
    /// Whether a plugin is being edited.
    editing: bool,
    /// Theme for rendering.
    theme: PluginViewTheme,
    /// Self-entity handle.
    self_entity: Option<Entity<Self>>,
    // Knob drag state
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

        Self {
            cache,
            set_param_cb,
            reset_param_cb,
            cb_userdata,
            plugin_type,
            param_snapshot,
            selected_param: 0,
            selected_band: 0,
            selected_eq_channel: 0,
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

    /// Check if this is an EQ-type plugin.
    fn is_eq(&self) -> bool {
        matches!(self.plugin_type.as_str(), "EQ" | "eq")
    }

    /// Convert a UI band-relative param index to an absolute AU param index.
    /// EQ UI uses `band_idx * 4 + field`, AU cache has `2 + band_idx * 4 + field`.
    fn ui_to_au_param_index(&self, ui_idx: usize) -> usize {
        if self.is_eq() {
            EQ_GLOBAL_PARAM_COUNT + ui_idx
        } else {
            ui_idx
        }
    }

    /// Get current max_filters value from the param snapshot.
    fn eq_max_filters(&self) -> usize {
        if !self.param_snapshot.is_empty() {
            self.param_snapshot[0] as usize
        } else {
            0
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
        let mut bands = Vec::new();
        let max_filters = self.eq_max_filters();

        for band in 0..max_filters.min(20) {
            let base = EQ_GLOBAL_PARAM_COUNT + band * EQ_PARAMS_PER_BAND;
            if base + 3 >= self.param_snapshot.len() {
                break;
            }

            let frequency = self.param_snapshot[base];
            let q = self.param_snapshot[base + 1];
            let gain_db = self.param_snapshot[base + 2];
            let filter_type_raw = self.param_snapshot[base + 3] as u32;

            let filter_type = match filter_type_raw {
                0 => BiquadFilterType::Peak,
                1 => BiquadFilterType::Lowshelf,
                2 => BiquadFilterType::Highshelf,
                3 => BiquadFilterType::Lowpass,
                4 => BiquadFilterType::Highpass,
                5 => BiquadFilterType::Bandpass,
                6 => BiquadFilterType::Notch,
                other => panic!("Unknown EQ filter type index: {other}"),
            };

            bands.push(sotf_plugin_eq::ui::EqBandView {
                filter_type,
                frequency,
                q,
                gain_db,
                muted: false,
                solo: false,
            });
        }

        bands
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

    fn set_selected_eq_channel(&mut self, _plugin_idx: usize, channel: usize) {
        self.selected_eq_channel = channel;
    }

    fn add_eq_band(&mut self, _plugin_idx: usize) {
        let current = self.eq_max_filters();
        if current >= 20 {
            return;
        }
        let new_count = current + 1;
        // Set max_filters (AU param index 0)
        self.set_au_param(0, new_count as f64);
        // Initialize the new band with defaults
        let base = EQ_GLOBAL_PARAM_COUNT + current * EQ_PARAMS_PER_BAND;
        self.set_au_param(base, 1000.0);     // frequency = 1000 Hz
        self.set_au_param(base + 1, 1.0);    // q = 1.0
        self.set_au_param(base + 2, 0.0);    // gain_db = 0.0
        self.set_au_param(base + 3, 0.0);    // filter_type = Peak (0)
        // Select the new band
        self.selected_band = current;
    }

    fn remove_eq_band(&mut self, _plugin_idx: usize, band_idx: usize) {
        let current = self.eq_max_filters();
        if current == 0 || band_idx >= current {
            return;
        }
        // Shift bands down: copy band[i+1] → band[i] for i >= band_idx
        for i in band_idx..current.saturating_sub(1) {
            let src_base = EQ_GLOBAL_PARAM_COUNT + (i + 1) * EQ_PARAMS_PER_BAND;
            let dst_base = EQ_GLOBAL_PARAM_COUNT + i * EQ_PARAMS_PER_BAND;
            for f in 0..EQ_PARAMS_PER_BAND {
                let val = self.param_snapshot.get(src_base + f).copied().unwrap_or(0.0);
                self.set_au_param(dst_base + f, val);
            }
        }
        // Decrease max_filters
        self.set_au_param(0, (current - 1) as f64);
        // Adjust selected band
        if self.selected_band >= current - 1 {
            self.selected_band = (current - 1).saturating_sub(1);
        }
    }

    fn toggle_eq_band_mute(&mut self, _plugin_idx: usize, _band_idx: usize) {
        // Mute/solo are not exposed as AU parameters — no-op in AU context.
        // These are UI-only states in the app-gpui player.
    }

    fn toggle_eq_band_solo(&mut self, _plugin_idx: usize, _band_idx: usize) {
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
                        selected_eq_channel: self.selected_eq_channel,
                        midi_overlay: None,
                    },
                    &self.theme,
                )
                .into_any_element()
            }
            _ => {
                let param_count = self.param_snapshot.len();
                div()
                    .size_full()
                    .bg(rgb(0x1a1a2e))
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .text_color(rgb(0xffffff))
                            .text_xl()
                            .child(format!("SOTF: {}", self.plugin_type)),
                    )
                    .child(
                        div()
                            .text_color(rgb(0x808090))
                            .text_sm()
                            .mt(px(8.0))
                            .child(format!("{param_count} parameters")),
                    )
                    .into_any_element()
            }
        }
    }
}
