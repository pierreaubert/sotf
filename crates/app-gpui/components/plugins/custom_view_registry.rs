//! Custom View Registry
//!
//! Maps plugin type keys to custom render functions, replacing the
//! match-arm dispatch in `render_plugin_content()`. Plugins without
//! a registered custom view fall through to the generic layout renderer.

mod gpui_view_registry;
mod misc;
mod render;
mod types;

pub use gpui_view_registry::*;
pub use misc::*;
pub use types::*;
