//! Federation sources and server configuration business logic.

mod local;
mod misc;
#[cfg(any(feature = "tidal", feature = "spotify"))]
mod service_login;
#[cfg(test)]
mod tests;

pub use misc::*;
