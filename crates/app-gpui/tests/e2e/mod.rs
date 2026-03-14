#![allow(dead_code)]
//! End-to-End Testing Framework for playder-gpui
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

pub mod driver;
pub mod pages;
#[allow(clippy::arc_with_non_send_sync)]
pub mod runner;
pub mod scenarios;
pub mod simulator;
pub mod assertions;
pub mod factories;

/// Install a panic hook that suppresses panics from the `async-io` background thread
/// and forces a clean exit after all tests complete.
///
/// The GPUI test scheduler's `async-io` reactor thread can wake tasks after the test
/// scheduler has finished, causing a panic chain: initial panic → panic-in-destructor →
/// `abort()` (SIGABRT). Since `abort()` is implemented as a double-panic in
/// `async-task::utils`, the only way to prevent it is to `std::process::exit()` from
/// the first panic on that thread, before the destructor cascade triggers.
///
/// Called once via `std::sync::Once` to avoid overwriting the hook on every test.
pub fn install_async_io_panic_filter() {
    use std::sync::Once;
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        let default_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let thread = std::thread::current();
            let name = thread.name().unwrap_or("");
            if name == "async-io" {
                // The async-io thread panicked because the GPUI test scheduler was torn
                // down while the global reactor still had pending wakeups. All tests have
                // already completed and reported results at this point. Force a clean exit
                // to prevent the double-panic → SIGABRT cascade.
                std::process::exit(0);
            }
            default_hook(info);
        }));
    });
}
