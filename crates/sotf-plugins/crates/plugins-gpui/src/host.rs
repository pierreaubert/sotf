//! The [`PluginViewHost`] trait — core abstraction for plugin UI rendering.
//!
//! This trait replaces the direct `Entity<AppState>` coupling that previously
//! tied plugin UIs to the app-gpui player. Both the GPUI player (via `AppState`)
//! and Audio Unit views (via `AuHostState`) implement this trait.

/// Abstraction over the host environment, enabling plugin UIs to work
/// in both the full GPUI app and standalone AU plugin views.
///
/// All methods receive `plugin_idx` to identify which plugin in a chain
/// is being operated on. In AU context this is always 0 (single plugin).
///
/// # Implementation Notes
///
/// - In app-gpui: delegates to `App::set_plugin_param()`, `PluginState` fields, etc.
/// - In AU: delegates to `ParameterMap::set_normalized()` on the `PluginHandle`.
pub trait PluginViewHost: 'static {
    /// Set a plugin parameter value (denormalized, in display-scale units).
    ///
    /// The value uses display scaling (e.g., 0–100 for percentages, Hz for frequencies).
    /// The host is responsible for applying `ParamSpec::display_scale` before storing.
    fn set_plugin_param(&mut self, plugin_idx: usize, param_idx: usize, value: f64);

    /// Reset a plugin parameter to its default value (from ParamSpec).
    fn reset_plugin_param(&mut self, plugin_idx: usize, param_idx: usize);

    /// Mark a plugin as being actively edited (for selection highlight).
    fn set_editing_plugin(&mut self, plugin_idx: usize);

    /// Set the selected parameter index (for keyboard navigation highlights).
    fn set_selected_param(&mut self, plugin_idx: usize, param_idx: usize);

    /// Track knob drag state for smooth mouse interaction.
    fn on_knob_drag_start(
        &mut self,
        plugin_idx: usize,
        param_idx: usize,
        start_y: f32,
        start_value: f64,
        min: f64,
        max: f64,
    );

    /// Called when the user stops dragging a knob/curve.
    fn on_knob_drag_end(&mut self);

    /// Query current knob drag state for interactive controls (e.g., transfer curve).
    ///
    /// Returns `(is_dragging, plugin_idx, start_y, start_value, drag_min, drag_max)`.
    fn knob_drag_state(&self) -> (bool, usize, f32, f64, f64, f64);

    // ── Band-based plugin operations (EQ, multiband comp/exp, dynamic EQ) ──

    /// Set the selected band index for highlighting and editing.
    fn set_selected_band(&mut self, _plugin_idx: usize, _band: usize) {}

    /// Add a new band (EQ filter, multiband crossover, etc.).
    fn add_band(&mut self, _plugin_idx: usize) {}

    /// Remove a band by index.
    fn remove_band(&mut self, _plugin_idx: usize, _band_idx: usize) {}

    /// Toggle mute state on a band.
    fn toggle_band_mute(&mut self, _plugin_idx: usize, _band_idx: usize) {}

    /// Toggle solo state on a band.
    fn toggle_band_solo(&mut self, _plugin_idx: usize, _band_idx: usize) {}

    // ── Channel-specific operations ─────────────────────────────────────

    /// Set the selected channel (for per-channel processing mode).
    fn set_selected_channel(&mut self, _plugin_idx: usize, _channel: usize) {}

    /// Toggle per-channel mode on/off.
    fn set_per_channel_mode(&mut self, _plugin_idx: usize, _per_channel: bool) {}
}
