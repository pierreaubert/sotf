use crate::app::AppState;
use crate::components::plugins::theme::PluginTheme;
use crate::theme::Theme;
use crate::ui::PlayerView;
use gpui::*;
use sotf_audio_player::{PluginGraph, PluginSettings};
use sotf_audio_player_midi::mapping::MidiOverlay;
use std::sync::Arc;

/// Shared context passed to every custom view render function.
pub struct CustomViewRenderContext<'a> {
    pub entity: Entity<AppState>,
    pub plugin_idx: usize,
    pub settings: &'a PluginSettings,
    pub available_width: f32,
    pub is_editing: bool,
    pub selected_param: usize,
    pub selected_band_idx: usize,
    pub theme: &'a Theme,
    /// Resolved plugin chassis theme — cascade of rack default + per-plugin
    /// override. Renderers that have adopted the chassis theme system read
    /// from this; renderers still on the global app theme can ignore it.
    pub plugin_theme: &'a PluginTheme,
    pub loudness: Option<sotf_audio_player::LoudnessData>,
    pub plugin_data: Option<Arc<dyn std::any::Any + Send + Sync>>,
    pub spectrum_tilt_select_open: bool,
    pub spectrum_reference_select_open: bool,
    pub plugin_graph: &'a PluginGraph,
    pub midi_overlay: Option<&'a MidiOverlay>,
}

/// Function signature for custom view renderers.
pub type CustomViewRenderFn =
    fn(ctx: &CustomViewRenderContext, cx: &mut Context<PlayerView>) -> AnyElement;
