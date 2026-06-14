//! Headless server mode for SOTF.
//!
//! When launched with `--server`, the app skips UI and runs MPD/DLNA servers
//! directly, allowing remote clients to browse the library and control playback.

mod api;
mod api_library_album_query;
mod api_request;
mod consts;
mod dlna;
mod dlna_library_adapter;
mod generate;
mod handle;
mod misc;
mod mpd;
mod mpd_player_adapter;
mod parse;
mod run;
mod server_state;
mod sotf;
mod stream;
#[cfg(test)]
mod tests;
mod track;
mod types;
mod validate;

pub use dlna::*;
pub use generate::*;
pub use misc::*;
pub use parse::*;
pub use run::*;
pub use sotf::*;
pub use types::*;

/// Re-exported for snapshot tests of range-header parsing.
pub use api::api_parse_range_header;
