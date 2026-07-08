//! E2E Test Runner
//!
//! Orchestrates the execution of test scenarios, managing the test context,
//! window creation, and result collection.

use gpui::{TestAppContext, VisualTestContext, WindowHandle, prelude::*};
use std::error::Error;

/// Result of running an E2E test scenario.
#[derive(Debug, Default)]
pub struct TestResult {
    /// Whether the test passed.
    pub passed: bool,
    /// Optional error message if the test failed.
    pub error_message: Option<String>,
    /// Time taken to execute the scenario.
    pub duration_ms: u64,
}

impl TestResult {
    /// Create a successful result.
    pub fn success() -> Self {
        Self {
            passed: true,
            error_message: None,
            duration_ms: 0,
        }
    }

    /// Create a failed result with an error message.
    pub fn failed(message: impl Into<String>) -> Self {
        Self {
            passed: false,
            error_message: Some(message.into()),
            duration_ms: 0,
        }
    }

    /// Create a result from a Rust `Result`.
    pub fn from_result(result: Result<(), Box<dyn Error>>, duration_ms: u64) -> Self {
        match result {
            Ok(_) => Self {
                passed: true,
                error_message: None,
                duration_ms,
            },
            Err(e) => Self {
                passed: false,
                error_message: Some(e.to_string()),
                duration_ms,
            },
        }
    }
}

/// Trait for E2E test scenarios.
///
/// Implement this trait to create a reusable test scenario that can be
/// run by the E2ERunner.
pub trait TestScenario {
    /// Human-readable name for this scenario.
    fn name(&self) -> &'static str;

    /// Set up the test environment before running.
    /// This is called before the window is created.
    fn setup(&mut self, _cx: &mut TestAppContext) -> Result<(), Box<dyn Error>> {
        Ok(())
    }

    /// Execute the test scenario.
    /// This is where interactions and assertions happen.
    fn execute(
        &self,
        _cx: &mut VisualTestContext,
        window: WindowHandle<PlayerView>,
    ) -> Result<(), Box<dyn Error>> {
        let _ = window; // Default implementation ignores window
        Ok(())
    }

    /// Clean up after the test.
    fn teardown(&mut self, _cx: &mut TestAppContext) -> Result<(), Box<dyn Error>> {
        Ok(())
    }
}

use sotf_audio_player::Player;
use sotf_audio_player_gpui::app::player_handle::PlayerHandle;
use sotf_audio_player_gpui::app::state::ui::LayoutState;
use sotf_audio_player_gpui::app::{App, AppState};
use sotf_audio_player_gpui::ui::PlayerView;

/// Runner for executing E2E test scenarios.
pub struct E2ERunner<S: TestScenario> {
    scenario: S,
}

impl<S: TestScenario> E2ERunner<S> {
    /// Create a new runner for the given scenario.
    pub fn new(scenario: S) -> Self {
        Self { scenario }
    }

    /// Run the scenario and return the result.
    pub async fn run(mut self, cx: &mut TestAppContext) -> Result<TestResult, Box<dyn Error>> {
        crate::install_async_io_panic_filter();
        let start_time = std::time::Instant::now();

        // Setup phase
        self.scenario.setup(cx)?;

        // Execute phase - the actual test
        let result = self.execute_scenario(cx).await;

        // Teardown phase
        let _ = self.scenario.teardown(cx);

        let duration_ms = start_time.elapsed().as_millis() as u64;

        Ok(TestResult::from_result(result, duration_ms))
    }

    /// Execute the scenario with a window.
    async fn execute_scenario(&mut self, cx: &mut TestAppContext) -> Result<(), Box<dyn Error>> {
        // Create full App with PlayerView
        let window = cx.add_window(|_, cx| {
            let app_state = cx.new(|cx| {
                let app = App::new();
                let player = Player::new();
                let layout = cx.new(|_| LayoutState::default());

                AppState {
                    app,
                    layout,
                    player: PlayerHandle::new(player),
                }
            });

            PlayerView::new(app_state, cx)
        });

        let mut visual_cx = VisualTestContext::from_window(window.into(), cx);

        // Pump the loop to let the UI settle
        visual_cx.run_until_parked();

        // Execute the scenario
        self.scenario.execute(&mut visual_cx, window)?;

        // Drain any pending async work spawned during the test so that background
        // threads (e.g. async-io) don't try to wake tasks after the test scheduler
        // is torn down (which causes a panic-in-destructor → SIGABRT).
        visual_cx.run_until_parked();

        // Close the window to release GPUI entities.
        // The GPUI revision (ebb451ec) added leak detection in tests.
        let _ = window.update(&mut visual_cx, |_, window, _| {
            window.remove_window();
        });
        visual_cx
            .executor()
            .advance_clock(std::time::Duration::from_millis(500));
        visual_cx.run_until_parked();
        drop(visual_cx);
        cx.run_until_parked();

        Ok(())
    }
}
