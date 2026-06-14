//! Tiny HTTP/1.1 server for the dev API.
//!
//! Runs an OS thread bound to `127.0.0.1:<port>`. Each connection is
//! handled inline (one at a time is fine for testing), parsed into a
//! [`DevCommand`], and forwarded to the GPUI main thread via an mpsc
//! channel. The handler blocks on a synchronous reply channel before
//! writing the HTTP response and closing the connection.

mod dispatch;
mod get;
mod misc;
mod parse;
mod post;
mod qa;
mod types;
mod with;

pub use parse::*;
