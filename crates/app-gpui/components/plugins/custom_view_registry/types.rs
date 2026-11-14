use crate::app::AppState;
use crate::components::plugins::theme::PluginTheme;
use crate::theme::Theme;
use crate::ui::PlayerView;
use gpui::{AnyElement, Context, Entity, FocusHandle, ScrollHandle};
use sotf_audio_player::{PluginGraph, PluginSettings};
use sotf_audio_player_midi::mapping::MidiOverlay;
use std::sync::Arc;

/// Shared context passed to every custom view render function.
pub struct CustomViewRenderContext<'a> {
    pub entity: Entity<AppState>,
    pub plugin_idx: usize,
    /// Stable engine/plugin instance identity, including nonlinear graph nodes.
    pub plugin_instance_id: Option<usize>,
    pub layout_choice_menu: Option<(usize, usize)>,
    pub layout_choice_highlight: Option<usize>,
    pub settings: &'a PluginSettings,
    pub available_width: f32,
    /// Measured scrollport height; short wide windows need compact controls too.
    pub available_height: f32,
    pub layout_scale: f32,
    pub is_editing: bool,
    pub selected_param: usize,
    pub selected_band_idx: usize,
    pub theme: &'a Theme,
    /// Resolved plugin chassis theme — cascade of rack default + per-plugin
    /// override. Renderers that have adopted the chassis theme system read
    /// from this; renderers still on the global app theme can ignore it.
    pub plugin_theme: &'a PluginTheme,
    pub loudness: Option<Arc<sotf_audio_player::LoudnessData>>,
    pub plugin_data: Option<Arc<dyn std::any::Any + Send + Sync>>,
    pub spectrum_tilt_select_open: bool,
    pub spectrum_reference_select_open: bool,
    pub plugin_graph: &'a PluginGraph,
    pub midi_overlay: Option<&'a MidiOverlay>,
    /// Stable chart focus target supplied by the owning `PlayerView`. Renderers
    /// must not re-read `PlayerView` while it is already rendering.
    pub eq_chart_focus_handle: FocusHandle,
    /// Stable focus target for the one selected plugin parameter exact editor.
    /// Only one selected knob renders its exact editor at a time.
    pub plugin_exact_entry_focus_handle: FocusHandle,
    /// Rack scrollport for custom controls that need focus-follow scrolling.
    pub rack_scroll_handle: Option<ScrollHandle>,
    /// View-owned Matrix grid; captures copy its offset into an independent handle.
    pub matrix_scroll_handle: ScrollHandle,
}

/// Function signature for custom view renderers.
pub type CustomViewRenderFn =
    fn(ctx: &CustomViewRenderContext, cx: &mut Context<PlayerView>) -> AnyElement;

impl std::fmt::Debug for CustomViewRenderContext<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CustomViewRenderContext")
            .field("plugin_idx", &self.plugin_idx)
            .field("is_editing", &self.is_editing)
            .field("available_width", &self.available_width)
            .finish_non_exhaustive()
    }
}
