pub mod params;

#[path = "lib/consts.rs"]
mod consts;
#[path = "lib/matrix_plugin.rs"]
mod matrix_plugin;

pub use matrix_plugin::*;
