//! Domain-separated state modules.
//!
//! This module contains focused state structs that can be used as GPUI Entities.
//! The goal is to separate concerns and allow independent observation/subscription.

pub mod library;

pub use library::LibraryState;
