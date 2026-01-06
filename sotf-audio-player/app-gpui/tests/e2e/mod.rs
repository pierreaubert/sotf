//! End-to-End Testing Framework for sotf-audio-player-gpui
//!
//! Headless E2E tests that simulate real user interactions with the application.
//! All audio is routed to Blackhole virtual device to avoid playing on real speakers.
//!
//! # Usage
//!
//! ```bash
//! # Run all E2E tests
//! cargo test -p sotf-gpui --test e2e
//!
//! # Run specific test
//! cargo test -p sotf-gpui --test e2e startup_volume
//!
//! # Run with output
//! cargo test -p sotf-gpui --test e2e -- --nocapture
//! ```
//!
//! # Examples
//!
//! ```ignore
//! #[gpui::test]
//! async fn test_startup_volume_is_10(cx: &mut TestAppContext) {
//!     let scenario = StartupVolumeScenario::default();
//!     let runner = E2ERunner::new(scenario);
//!     let result = runner.run(cx).await;
//!     assert!(result.is_ok());
//! }
//! ```

pub mod runner;
pub mod scenarios;
pub mod simulator;
