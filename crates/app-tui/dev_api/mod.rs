//! Dev-only HTTP API for scripted E2E testing.
//!
//! Compiled only when the `dev-api` feature is on. Spawns a tiny
//! HTTP/1.1 server on `127.0.0.1:<port>` that translates JSON requests
//! into actions dispatched against the TUI app state.

pub mod commands;
pub mod queries;
mod server;

pub use commands::{DevCommand, DevReply, DevQueryReply};
pub use server::start;
