//! Dev-only HTTP API for scripted E2E testing.
//!
//! Compiled only when the `dev-api` feature is on. Spawns a tiny
//! HTTP/1.1 server on `127.0.0.1:<port>` that translates JSON requests
//! into actions dispatched against the GPUI window.
//!
//! Phase 1: only `POST /action {"name": "<ActionName>"}` is wired,
//! and only `PlayPause` is recognised. Used to validate the plumbing
//! end to end before generalising in Phase 2.

mod commands;
mod dev_element;
mod performance;
mod queries;
#[doc(hidden)]
pub mod registry;
mod server;

pub use dev_element::DevTrackExt;
pub use registry::DevElementState;
pub use registry::clear as clear_tracked_elements;
#[doc(hidden)]
pub use server::merge_missing_tracked_elements;
#[doc(hidden)]
pub use server::{DevApiTestHandle, start, start_for_testing};

/// Whether an isolated QA run should keep its native window hidden.
#[doc(hidden)]
#[must_use]
pub fn background_window_enabled() -> bool {
    std::env::var_os("SOTF_QA_DIR").is_some()
        && std::env::var_os("SOTF_QA_BACKGROUND").is_some_and(|value| value == "1")
}
