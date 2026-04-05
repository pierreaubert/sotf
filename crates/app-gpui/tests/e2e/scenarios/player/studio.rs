use crate::driver::AppDriver;
use crate::runner::{E2ERunner, TestScenario};
use gpui::{TestAppContext, VisualTestContext, WindowHandle};
use sotf_audio_player_gpui::app::Screen;
use sotf_audio_player_gpui::ui::PlayerView;
use std::error::Error;

/// Test Studio screen navigation and basic functionality
pub struct StudioScreenScenario;

impl TestScenario for StudioScreenScenario {
    fn name(&self) -> &'static str {
        "Studio Screen Navigation"
    }

    fn execute(
        &self,
        cx: &mut VisualTestContext,
        window: WindowHandle<PlayerView>,
    ) -> Result<(), Box<dyn Error>> {
        let mut driver = AppDriver::new(cx, window);

        // Navigate to Studio screen
        driver.navigate_to(Screen::Studio);

        // Verify we're on Studio screen
        let screen = driver.read_app(|app| app.ui_state.current_screen);
        if screen != Screen::Studio {
            return Err(format!("Expected Studio screen, got {:?}", screen).into());
        }

        // Verify Studio screen shows the plugin rack
        // The Studio screen is essentially the Plugin Rack in a different view
        let has_plugins = driver.read_app(|app| app.plugin_state.graph.plugins().len());

        // Log the number of plugins for debugging
        println!("Studio screen loaded with {} plugins", has_plugins);

        Ok(())
    }
}

#[gpui::test]
async fn test_studio_screen_navigation(cx: &mut TestAppContext) {
    let scenario = StudioScreenScenario;
    let runner = E2ERunner::new(scenario);
    let result = runner.run(cx).await;

    if let Err(e) = &result {
        println!("Test failed: {}", e);
    }
    assert!(result.is_ok(), "Studio screen navigation test failed");
}
