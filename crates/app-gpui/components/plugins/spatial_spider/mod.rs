//! Spatial spider visualizer — live 2D and 3D view of per-channel SPL and
//! inter-channel correlation, anchored to the active speaker layout.
//!
//! Layered so the data pipeline is testable in isolation:
//!
//! - [`data`]: pure mapping `(SpeakerConfig, dBTP|correlation row) →
//!   spider polygon vertices`. No GPUI, no wgpu.
//! - render layer (next phase): `gpu2d` for the horizontal disc, `gpu3d`
//!   for the two intersecting planes.

pub mod data;
pub mod render;

pub use data::{ChannelMetric, SpeakerVertex, SpiderMode, SpiderPolygon};
pub use render::{
    SpiderColors, SpiderDisc2D, SpiderView3D, render_spatial_spider_controls,
    render_spatial_spider_graph, render_spatial_spider_panel, resolve_speaker_config,
};

use d3rs::gpu3d::Lines3DState;
use std::cell::RefCell;
use std::rc::Rc;

/// Top-level rendering mode for the spider widget.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SpiderViewMode {
    /// Top-down horizontal disc.
    #[default]
    Disc2D,
    /// Two intersecting reference planes.
    View3D,
}

/// Persistent UI state shared across plugin views that host the spider
/// (upmixer / XTC / AAE). Lives on `AppState`. Holds the orbit camera so
/// the 3D scene remains stable across re-renders, and the mode/reference
/// selectors so they survive tab toggles.
#[derive(Clone)]
pub struct SpatialSpiderUiState {
    pub view_mode: SpiderViewMode,
    pub spider_mode: SpiderMode,
    /// Reference channel for correlation mode. Default 0 (FL).
    pub correlation_ref_channel: usize,
    /// Whether the reference-channel Select dropdown is currently open.
    /// Drives the `is_open` prop on the gpui-ui-kit `Select` so it stays
    /// expanded between renders.
    pub ref_channel_select_open: bool,
    /// Shared orbit-camera state for the 3D view. Wrapped in `Rc<RefCell>`
    /// so the GPUI element can mutate it from mouse handlers without
    /// borrowing `AppState`.
    pub camera_3d: Rc<RefCell<Lines3DState>>,
}

impl Default for SpatialSpiderUiState {
    fn default() -> Self {
        // Default camera: 3.5 units back, 20° azimuth, 20° elevation.
        // Picks an angle where both reference planes are clearly visible.
        let camera_3d = Rc::new(RefCell::new(Lines3DState::new(3.5, 20.0, 20.0)));
        Self {
            view_mode: SpiderViewMode::Disc2D,
            spider_mode: SpiderMode::Spl,
            correlation_ref_channel: 0,
            ref_channel_select_open: false,
            camera_3d,
        }
    }
}

impl std::fmt::Debug for SpatialSpiderUiState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SpatialSpiderUiState")
            .field("view_mode", &self.view_mode)
            .field("spider_mode", &self.spider_mode)
            .field("correlation_ref_channel", &self.correlation_ref_channel)
            // Skip camera_3d — Rc<RefCell<_>> would just print pointers.
            .finish_non_exhaustive()
    }
}

/// Cheaply-cloneable bundle the layout renderer reads at paint time. Built
/// at the `render_from_layout` call site (which has `cx`) and threaded
/// through so deep render functions don't need to re-borrow `AppState`.
///
/// The correlation matrix lives on `LoudnessData.correlation_matrix` —
/// `LoudnessMonitor` now computes both at the same poll point — so a
/// single field carries both SPL and correlation data.
///
/// # Source of truth: chain-output (current limitation)
///
/// `loudness` always reflects the **chain-output** `LoudnessMonitor` (the
/// last permanent monitor in the rack). When the spider widget is hosted on
/// the upmixer's Spatial tab, on AAE's `VizSlot::Custom("spatial_spider")`,
/// or anywhere else, it shows the *same* data — what the final output looks
/// like after every downstream plugin has run.
///
/// This means putting `Upmixer → Channel Mute → LoudnessMonitor` and
/// opening the upmixer's Spatial tab shows post-mute data, not the
/// upmixer's own output. For most chains the difference is small, but it
/// is real.
///
/// # Future per-plugin support
///
/// Two paths exist when this becomes a felt limitation:
///
/// 1. Insert a `LoudnessMonitor` (with `spatial_enabled = true`) downstream
///    of each plugin that hosts the spider. Heavy: changes the engine
///    graph topology.
/// 2. Have each spider-hosting plugin compute its own per-channel SPL and
///    correlation matrix internally and expose it via `Plugin::get_data()`.
///    Lighter, but each plugin pays the O(N²) compute cost.
///
/// Until one of those lands, the widget header carries a small "(chain
/// out)" label so the user knows what they're looking at.
#[derive(Clone)]
pub struct SpatialSpiderSnapshot {
    pub loudness: Option<sotf_audio_player::LoudnessData>,
    pub ui: SpatialSpiderUiState,
}
