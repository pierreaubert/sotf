use crate::driver::AppDriver;
use crate::runner::{E2ERunner, TestScenario};
use gpui::{TestAppContext, VisualTestContext, WindowHandle};
use sotf_audio_player_gpui::app::Screen;
use sotf_audio_player_gpui::ui::PlayerView;
use std::error::Error;

/// Test Spectrum analyzer screen navigation and display
#[derive(Debug)]
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
        driver.update_app(|app, _| {
            app.set_screen(Screen::Spectrum, "SpectrumRegression");
        });

        // Verify we're on Spectrum screen
        let screen = driver.read_app(|app| app.ui_state.current_screen);
        if screen != Screen::Spectrum {
            return Err(format!("Expected Spectrum screen, got {:?}", screen).into());
        }

        let analyzer_count = driver.read_app(|app| {
            assert!(app.plugin_state.has_enabled_spectrum_analyzer());
            assert!(app.plugin_state.graph.spectrum_engine_index().is_some());
            app.plugin_state
                .graph
                .plugins()
                .iter()
                .filter(|plugin| {
                    plugin.plugin_type() == sotf_audio_player::PluginType::SpectrumAnalyzer
                })
                .count()
        });
        assert_eq!(analyzer_count, 1);
        // Repeated visits must not accumulate analyzers.
        driver.update_app(|app, _| {
            app.set_screen(Screen::Library, "SpectrumRegression");
            app.set_screen(Screen::Spectrum, "SpectrumRegression");
        });
        let analyzer_count_after = driver.read_app(|app| {
            app.plugin_state
                .graph
                .plugins()
                .iter()
                .filter(|plugin| {
                    plugin.plugin_type() == sotf_audio_player::PluginType::SpectrumAnalyzer
                })
                .count()
        });
        assert_eq!(analyzer_count_after, analyzer_count);

        Ok(())
    }
}

#[gpui::test]
async fn test_spectrum_screen_navigation(cx: &mut TestAppContext) {
    crate::runner::ensure_shared_test_config_dir();
    let scenario = SpectrumScreenScenario;
    let runner = E2ERunner::new(scenario);
    let result = runner.run(cx).await;

    let result = result.expect("Spectrum scenario runner failed");
    assert!(
        result.passed,
        "Spectrum screen failed: {:?}",
        result.error_message
    );
}
