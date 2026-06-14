pub mod params;

#[path = "lib/consts.rs"]
mod consts;
#[path = "lib/default.rs"]
mod default;
#[path = "lib/mono_to_stereo_plugin.rs"]
mod mono_to_stereo_plugin;
#[cfg(test)]
#[path = "lib/tests.rs"]
mod tests;
#[path = "lib/types.rs"]
mod types;

pub use mono_to_stereo_plugin::*;
pub use types::*;
