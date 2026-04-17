//! Re-exports shared test factories from the common module.
//!
//! All factory implementations live in `tests/common/factories.rs`.
//! This module re-exports them so E2E tests can continue using `crate::factories::*`.

#[path = "../common/factories.rs"]
mod shared_factories;

pub use shared_factories::*;
