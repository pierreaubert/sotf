#![allow(deprecated)]
//! This is a collection of helper functions for performing common tasks on macOS.
//! These functions are only implemented for macOS, not iOS.

mod alive_listener;
mod get;
mod misc;
mod rate_listener;
mod set;

pub use alive_listener::*;
pub use get::*;
pub use misc::*;
pub use rate_listener::*;
pub use set::*;
