//! ============================================================================
//! A/B Comparison Plugin
//! ============================================================================
//!
//! This plugin allows fair comparison between two audio processing chains
//! with automatic loudness matching. Each path (A or B) can be:
//! - A single plugin
//! - A rack (linear chain of plugins)
//! - A graph (full DAG topology)

pub use config::*;

mod config;
mod factory;
pub mod params;

#[path = "lib/abcompare_plugin.rs"]
mod abcompare_plugin;
#[path = "lib/delay_line.rs"]
mod delay_line;
#[cfg(test)]
#[path = "lib/tests.rs"]
mod tests;
#[path = "lib/types.rs"]
mod types;

pub use abcompare_plugin::*;
pub use types::*;
