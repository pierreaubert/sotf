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
mod queries;
mod registry;
mod server;

pub use dev_element::DevTrackExt;
pub use server::start;
