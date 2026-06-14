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

mod auto;
mod misc;
mod mode_selector_info;
mod pot;
mod render;
mod types;

pub use render::*;
