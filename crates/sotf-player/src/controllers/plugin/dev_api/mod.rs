//! Dev-API helpers for plugin-chain introspection.
//!
//! These utilities are consumed by the GPUI/TUI dev-API servers to answer
//! queries about the current plugin graph without duplicating logic in each
//! app shell.

pub mod actions;
pub mod queries;
