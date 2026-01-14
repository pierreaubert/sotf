use crate::driver::AppDriver;
use crate::runner::{E2ERunner, TestScenario};
use gpui::{TestAppContext, VisualTestContext, WindowHandle, prelude::*};
use sotf_audio_player_gpui::ui::PlayerView;
use std::error::Error;

pub struct SpinoramaFlowScenario;

impl TestScenario for SpinoramaFlowScenario {
    fn name(&self) -> &'static str {
        "Spinorama Flow"
    }

    fn execute(
        &self,
        cx: &mut VisualTestContext,
        window: WindowHandle<PlayerView>,
    ) -> Result<(), Box<dyn Error>> {
        let mut driver = AppDriver::new(cx, window);

        // 1. Navigate to Spinorama Tab
        // Assuming tab or navigation way. "Spinorama EQ" might be a tab or accessible via menu.
        // For now, assume it's a tab or we use driver navigation helper
        // driver.navigate_to(Screen::Spinorama); // If Screen/Navigation exists

        let mut page = driver.spinorama();

        // Inject mock data for testing (bypass API calls)
        page.inject_mock_data();

        // 2. Select Speaker
        page.select_speaker("Kef R3");

        // 3. Select Version
        // page.select_version("ASR"); // handled by inject_mock_data defaults for now logic-wise

        // 4. Set Target Curve
        page.set_target_curve("Harman In-Room 2018");

        // 5. Configure Optimizer
        page.set_optimization_params(3, 50); // Low filters and iterations for speed
        page.set_optimization_mode(
            sotf_audio_player_gpui::app::types::SpinoramaOptimizationMode::SpeakerScore,
        );
        page.set_frequency_limits(100.0, 10000.0);
        page.set_gain_limits(-6.0, 3.0);
        page.set_q_limits(1.0, 4.0);
        page.set_algorithm_params(20, 0.7, 0.8);
        page.set_smoothing(true, 12);
        page.set_advanced_config(2.0, 0.1);

        // 6. Start Optimization
        page.start_optimization();

        // 7. Verify Progress/Completion
        page.wait_for_optimization_completion()?;

        // 8. Assert Success Toast
        // driver.expect_toast("Optimization completed")?;

        Ok(())
    }
}

#[gpui::test]
async fn spinorama_flow(cx: &mut TestAppContext) {
    let scenario = SpinoramaFlowScenario;
    let mut runner = E2ERunner::new(scenario);
    runner.run(cx).await.unwrap();
}
