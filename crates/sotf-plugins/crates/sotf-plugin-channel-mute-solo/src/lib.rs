#![allow(clippy::duplicate_mod)]
pub mod params;

#[path = "lib/channel_mute_solo_plugin.rs"]
mod channel_mute_solo_plugin;
#[cfg(test)]
#[path = "lib/tests.rs"]
mod tests;
#[path = "lib/types.rs"]
mod types;

pub use channel_mute_solo_plugin::*;
pub use types::*;
