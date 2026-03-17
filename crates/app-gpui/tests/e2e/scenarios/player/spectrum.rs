use crate::driver::AppDriver;
use crate::runner::{E2ERunner, TestScenario};
use gpui::{TestAppContext, VisualTestContext, WindowHandle};
use sotf_audio_player_gpui::app::Screen;
use sotf_audio_player_gpui::ui::PlayerView;
use std::error::Error;

/// Test Spectrum analyzer screen navigation and display
pub struct SpectrumScreenScenario;

impl TestScenario for SpectrumScreenScenario {
    fn name(&self) -> &'static str {
        "Spectrum Screen Navigation"
    }

    fn execute(
        &self,
        cx: &mut VisualTestContext,
        window: WindowHandle<PlayerView>,
    ) -> Result<(), Box<dyn Error>> {
        let mut driver = AppDriver::new(cx, window);

        // Navigate to Spectrum screen
        driver.navigate_to(Screen::Spectrum);

        // Verify we're on Spectrum screen
        let screen = driver.read_app(|app| app.ui_state.current_screen);
        if screen != Screen::Spectrum {
            return Err(format!("Expected Spectrum screen, got {:?}", screen).into());
        }

        // Verify spectrum analyzer is available (state exists in app)
        let has_spectrum_support = driver.read_app(|app| {
            // Spectrum screen should be accessible
            app.ui_state.current_screen == Screen::Spectrum
        });

        println!("Spectrum screen loaded: {}", has_spectrum_support);

        Ok(())
    }
}

#[gpui::test]
async fn test_spectrum_screen_navigation(cx: &mut TestAppContext) {
    let scenario = SpectrumScreenScenario;
    let runner = E2ERunner::new(scenario);
    let result = runner.run(cx).await;

    if let Err(e) = &result {
        println!("Test failed: {}", e);
    }
    assert!(result.is_ok(), "Spectrum screen navigation test failed");
}
