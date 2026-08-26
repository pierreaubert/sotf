//! Shared protocol-v2 types and bounded HTTP plumbing for SOTF QA builds.
//!
//! The wire model is always available so clients can deserialize traces. The
//! listener is behind the `server` feature and intentionally refuses release
//! builds: production applications must never expose this control surface.

#![forbid(unsafe_code)]

#[cfg(all(feature = "server", not(debug_assertions)))]
compile_error!("sotf-dev-api's server feature is restricted to debug/QA builds");

pub mod auth;
pub mod http;
pub mod protocol;
pub mod queue;

#[cfg(feature = "server")]
pub mod server;

pub use auth::{RUN_ID_HEADER, RunId, RunIdError};
pub use http::{HttpError, HttpRequest, HttpResponse, Method};
pub use protocol::*;
pub use queue::{BoundedReceiver, BoundedSender, QueueError, QueueTelemetry, bounded_channel};
