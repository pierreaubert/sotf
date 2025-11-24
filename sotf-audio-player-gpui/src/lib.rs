// ============================================================================
// SOTF GPUI Audio Player Library
// ============================================================================
//
// This library module exposes internal types and functions for testing.
// The main application binary is in main.rs.

pub mod app;
pub mod config;

// Re-export commonly used types for testing
pub use app::{App, AppState, InputMode, Screen, ToastMessage, ToastType};
