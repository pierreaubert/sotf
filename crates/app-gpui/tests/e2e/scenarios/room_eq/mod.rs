//! Room EQ E2E tests.
//!
//! Contains tests for Room EQ screen navigation and parameter validation.

use crate::driver::AppDriver;
use crate::runner::{E2ERunner, TestScenario};
use gpui::{TestAppContext, VisualTestContext, WindowHandle};
use sotf_audio_player_gpui::app::Screen;
use sotf_audio_player_gpui::ui::PlayerView;
use std::error::Error;

pub mod parameter_check;

struct RoomEqNavigationScenario;

impl TestScenario for RoomEqNavigationScenario {
    fn name(&self) -> &'static str {
        "Room EQ Navigation"
    }

    fn execute(
        &self,
        cx: &mut VisualTestContext,
        window: WindowHandle<PlayerView>,
    ) -> Result<(), Box<dyn Error>> {
        let mut driver = AppDriver::new(cx, window);

        // Navigate to RoomEq screen
        driver.update_app(|app, _| {
            app.ui_state.current_screen = Screen::RoomEq;
        });

        // Verify the screen is accessible
        let screen = driver.read_app(|app| app.ui_state.current_screen);
        if screen != Screen::RoomEq {
            return Err(format!("Expected RoomEq screen, got {:?}", screen).into());
        }

        Ok(())
    }
}

#[gpui::test]
async fn test_room_eq_navigation(cx: &mut TestAppContext) {
    let scenario = RoomEqNavigationScenario;
    let runner = E2ERunner::new(scenario);
    let result = runner.run(cx).await;
    assert!(
        result.is_ok(),
        "Room EQ navigation test failed: {:?}",
        result.err()
    );
}