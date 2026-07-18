//! Shared plugin factory.
//!
//! Creates plugin instances from a type string and JSON parameters.
//! Used by the audio engine and by the A/B Compare plugin's sub-rack builder.

mod catalog;
mod consts;
mod create;
mod external;
mod is;
mod misc;
mod parse;
mod sandboxed_plugin_creation_options;
#[cfg(test)]
mod tests;
mod types;
mod validate;

pub use catalog::*;
pub use consts::*;
pub use create::*;
pub use is::*;
pub use sandboxed_plugin_creation_options::*;
pub use validate::*;
