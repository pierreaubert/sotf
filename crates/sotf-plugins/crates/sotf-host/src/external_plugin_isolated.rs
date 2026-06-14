//! Plugin wrapper that routes an external plugin through an isolated worker.
//!
//! This implements the normal [`Plugin`] trait while keeping unknown plugin
//! execution in a worker process. The audio callback path only publishes a block
//! to shared memory and consumes the worker result; restart decisions remain on
//! the owner/control side through the process supervisor.

mod consts;
mod isolated_external_plugin;
mod isolated_external_plugin_config;
mod misc;
#[cfg(test)]
mod tests;

pub use isolated_external_plugin::*;
pub use isolated_external_plugin_config::*;
