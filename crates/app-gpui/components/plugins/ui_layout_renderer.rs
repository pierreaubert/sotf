//! Generic Layout Renderer
//!
//! Renders any plugin that has a declarative `PluginLayout` definition.
//! Replaces 20+ hand-coded `render_*_plugin()` functions with a single
//! generic renderer driven by `PluginLayout` data + the constraint solver.
//!
//! Layout:
//! ```text
//! +--------------------------------------------+------------------+
//! | MAIN (groups side-by-side or stacked)       | OUTPUT           |
//! | [Tab1] [Tab2] ...  (+ collapsed groups)    |                  |
//! +--------------------------------------------+------------------+
//! ```

mod misc;
mod mode_selector_info;
mod pot;
mod render;
mod types;

#[doc(hidden)]
pub use misc::extract_file_paths;
pub use render::*;
use sotf_audio_player::PluginSettings;
use sotf_plugins::layout_solver::solve_layout_scaled;
use sotf_plugins::plugin_layout::ColumnRole;

/// Whether the generated layout currently has atomic groups in overflow.
pub(super) fn generated_layout_has_overflow(
    settings: &PluginSettings,
    available_width: f32,
    layout_scale: f32,
) -> bool {
    let Some(layout) = settings.layout() else {
        return false;
    };
    let params = settings.param_specs();
    let values: Vec<_> = (0..params.len())
        .map(|index| settings.param_value(index).unwrap_or(0.0))
        .collect();
    let solved_columns =
        solve_layout_scaled(layout.column_constraints, available_width, layout_scale);
    let main_width = solved_columns
        .column_width(ColumnRole::Main)
        .unwrap_or(available_width);
    let mode = mode_selector_info::detect_mode_selector(layout, params);
    !mode_selector_info::solve_main_groups(layout, &values, mode.as_ref(), main_width, layout_scale)
        .1
        .is_empty()
}
