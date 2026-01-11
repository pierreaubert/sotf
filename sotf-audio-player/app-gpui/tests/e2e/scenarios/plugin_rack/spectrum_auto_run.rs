use crate::driver::AppDriver;
use crate::pages::plugin_rack::PluginRackPage;
use crate::runner::{TestScenario, E2ERunner};
use gpui::{TestAppContext, VisualTestContext, WindowHandle};
use sotf_audio::plugins::PluginType;
use sotf_audio_player_gpui::ui::PlayerView;
use std::error::Error;
use std::time::Duration;

pub struct SpectrumAutoRunScenario;

impl TestScenario for SpectrumAutoRunScenario {
    fn name(&self) -> &'static str {
        "Spectrum Analyzer Auto Run"
    }

    fn execute(&self, cx: &mut VisualTestContext, window: WindowHandle<PlayerView>) -> Result<(), Box<dyn Error>> {
        let mut driver = AppDriver::new(cx, window);
        let mut page = PluginRackPage::new(&mut driver);

        // 1. Add Spectrum Analyzer Plugin
        // Ensure starting state is clean
        while page.get_plugin_count() > 0 {
            page.remove_plugin(0);
        }

        let spectrum_id = page.add_plugin(PluginType::SpectrumAnalyzer);
        assert!(spectrum_id > 0, "Failed to add Spectrum Analyzer");

        // 1.5 Load a track into queue so playback can start
        page.inject_test_track();

        // 2. Start Playback to generate spectrum data
        page.toggle_playback();

        // 3. Wait for spectrum data to be populated
        page.wait_for_spectrum(std::time::Duration::from_secs(5));
        
        // Check if data is present
        let has_data = page.has_spectrum_info();

        assert!(has_data, "Spectrum Analyzer did not auto-start (no spectrum_info found after adding plugin)");

        // 3. Verify Spectrum Data Content
        let magnitudes = page.get_spectrum_magnitudes();

        assert!(!magnitudes.is_empty(), "Spectrum magnitudes should not be empty");
        
        // Even with silence, we expect some data (likely minimal dB values)
        // This confirms the analyzer is actually processing buffers.
        println!("Spectrum data size: {}", magnitudes.len());

        Ok(())
    }
}

#[gpui::test]
async fn test_spectrum_auto_run(cx: &mut TestAppContext) {
    let runner = E2ERunner::new(SpectrumAutoRunScenario);
    runner.run(cx).await.unwrap();
}
