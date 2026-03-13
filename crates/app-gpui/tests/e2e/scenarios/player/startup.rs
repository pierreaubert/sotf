//! Startup scenarios for E2E testing.
//!
//! Tests for verifying application startup behavior.

use crate::driver::AppDriver;
use crate::runner::{E2ERunner, TestScenario};
use gpui::{TestAppContext, VisualTestContext, WindowHandle};
use sotf_audio_player_gpui::app::Screen;
use sotf_audio_player_gpui::ui::PlayerView;
use std::error::Error;

struct StartupDefaultsScenario;

impl TestScenario for StartupDefaultsScenario {
    fn name(&self) -> &'static str {
        "Startup Defaults"
    }

    fn execute(
        &self,
        cx: &mut VisualTestContext,
        window: WindowHandle<PlayerView>,
    ) -> Result<(), Box<dyn Error>> {
        let mut driver = AppDriver::new(cx, window);

        // Default screen should be Library
        let screen = driver.read_app(|app| app.ui_state.current_screen);
        if screen != Screen::Library {
            return Err(format!("Expected Library screen, got {:?}", screen).into());
        }

        // Default volume should be 0.1 (10%)
        let volume = driver.read_app(|app| app.playback.volume);
        if (volume - 0.1).abs() > 0.001 {
            return Err(format!("Expected volume ~0.1, got {}", volume).into());
        }

        // Should not be playing
        let is_playing = driver.read_app(|app| app.playback.is_playing);
        if is_playing {
            return Err("Should not be playing on startup".into());
        }

        // Should not be muted
        let muted = driver.read_app(|app| app.playback.muted);
        if muted {
            return Err("Should not be muted on startup".into());
        }

        // Queue should be empty
        let queue_len = driver.read_app(|app| app.queue.len());
        if queue_len != 0 {
            return Err(format!("Expected empty queue, got {} items", queue_len).into());
        }

        Ok(())
    }
}

#[gpui::test]
async fn test_startup_defaults(cx: &mut TestAppContext) {
    let scenario = StartupDefaultsScenario;
    let runner = E2ERunner::new(scenario);
    let result = runner.run(cx).await;
    assert!(
        result.is_ok(),
        "Startup defaults test failed: {:?}",
        result.err()
    );
}
