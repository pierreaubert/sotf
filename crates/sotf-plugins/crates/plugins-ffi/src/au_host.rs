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

use crate::param_cache::AtomicParamCache;
use gpui::prelude::*;
use gpui::*;
use math_audio_iir_fir::BiquadFilterType;
use plugins_gpui::{PluginViewHost, PluginViewTheme};
use std::sync::Arc;

/// C function pointer type for parameter writes.
///
/// Called when the GPUI UI changes a parameter. The Swift side implements this
/// by setting the value on the `AUParameterTree`, which triggers the standard
/// AU parameter observation flow → `plugin_set_parameter()`.
///
/// Arguments: `(userdata, param_index, denormalized_value)`
pub type SetParamCallback = extern "C" fn(*mut std::ffi::c_void, usize, f64);

/// C function pointer type for parameter reset (to default).
///
/// Arguments: `(userdata, param_index)`
pub type ResetParamCallback = extern "C" fn(*mut std::ffi::c_void, usize);

/// Root GPUI entity for AU plugin views.
///
/// Reads parameters from a shared `AtomicParamCache` (lock-free).
/// Writes parameters through a callback that goes via Swift's `AUParameterTree`.
pub struct AuHostState {
    /// Atomic parameter cache, shared with Swift's AU parameter observer.
    cache: Arc<AtomicParamCache>,
    /// Callback to set a parameter (routes through AUParameterTree).
    set_param_cb: SetParamCallback,
    /// Callback to reset a parameter to default.
    reset_param_cb: ResetParamCallback,
    /// Opaque userdata pointer passed to callbacks (typically Swift `self`).
    cb_userdata: *mut std::ffi::c_void,
    /// Plugin type string (e.g., "EQ", "Compressor").
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
    /// Self-entity handle, set after creation so render functions can receive Entity<Self>.
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
    /// Create a new AU host state.
    ///
    /// - `cache`: Shared atomic parameter cache (populated by Swift AU observer)
    /// - `set_param_cb`: Callback for parameter writes (routes through AUParameterTree)
    /// - `reset_param_cb`: Callback for parameter reset
    /// - `cb_userdata`: Opaque pointer passed to callbacks
    /// - `plugin_type`: Plugin type string
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

    /// Set the self-entity handle (called once after creation).
    pub fn set_entity(&mut self, entity: Entity<Self>) {
        self.self_entity = Some(entity);
    }

    /// Refresh parameter snapshot from the atomic cache (called once per frame).
    fn refresh_params(&mut self) {
        self.cache.read_all(&mut self.param_snapshot);
    }

    /// Build EQ render state from cached parameters.
    /// EQ layout: 2 global params + 20 bands × 4 params (frequency, q, gain_db, filter_type)
    fn build_eq_bands(&self) -> Vec<sotf_plugin_eq::ui::EqBandView> {
        let mut bands = Vec::new();
        let global_count = 2; // max_filters, tdf2
        let params_per_band = 4; // frequency, q, gain_db, filter_type

        // Read max_filters from first global param
        let max_filters = if !self.param_snapshot.is_empty() {
            self.param_snapshot[0] as usize
        } else {
            0
        };

        for band in 0..max_filters.min(20) {
            let base = global_count + band * params_per_band;
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
        // Write through callback → Swift AUParameterTree → plugin
        (self.set_param_cb)(self.cb_userdata, param_idx, value);
        // Update local snapshot immediately for responsive UI
        if param_idx < self.param_snapshot.len() {
            self.param_snapshot[param_idx] = value;
        }
    }

    fn reset_plugin_param(&mut self, _plugin_idx: usize, param_idx: usize) {
        (self.reset_param_cb)(self.cb_userdata, param_idx);
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
}

impl Render for AuHostState {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        // Refresh parameter snapshot from atomic cache (lock-free, 60Hz)
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
                    0, // plugin_idx is always 0 in AU context
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
                // Fallback: show plugin type name (placeholder for future plugin UIs)
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
