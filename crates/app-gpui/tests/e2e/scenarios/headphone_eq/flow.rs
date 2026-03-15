use crate::driver::AppDriver;
use crate::runner::{E2ERunner, TestScenario};
use gpui::{TestAppContext, VisualTestContext, WindowHandle};
use sotf_audio_player_gpui::app::types::HeadphoneEqStep;
use sotf_audio_player_gpui::ui::PlayerView;
use std::error::Error;

pub struct HeadphoneEqFlowScenario;

impl TestScenario for HeadphoneEqFlowScenario {
    fn name(&self) -> &'static str {
        "Headphone EQ Flow"
    }

    fn execute(
        &self,
        cx: &mut VisualTestContext,
        window: WindowHandle<PlayerView>,
    ) -> Result<(), Box<dyn Error>> {
        let mut driver = AppDriver::new(cx, window);

        driver.navigate_to(sotf_audio_player_gpui::app::Screen::HeadphoneEq);

        let mut page = driver.headphone_eq();

        page.inject_mock_data();

        let step = page.get_current_step();
        if step != HeadphoneEqStep::Optimization {
            return Err(format!(
                "Expected Optimization step after inject_mock_data, got {:?}",
                step
            )
            .into());
        }

        page.set_target_curve("harman-over-ear-2018");
        page.set_loss_type("score");
        page.set_optimization_params(5, 20);
        page.set_frequency_limits(20.0, 20000.0);
        page.set_gain_limits(-12.0, 12.0);
        page.set_q_limits(0.5, 10.0);

        page.start_optimization();

        page.wait_for_optimization_completion()?;

        let status = page.get_optimization_status();
        if status != sotf_audio_player_gpui::app::types::OptimizationStatus::Completed {
            return Err(format!("Expected Completed status, got {:?}", status).into());
        }

        Ok(())
    }
}

#[gpui::test]
async fn headphone_eq_flow(cx: &mut TestAppContext) {
    let scenario = HeadphoneEqFlowScenario;
    let runner = E2ERunner::new(scenario);
    runner.run(cx).await.unwrap();
}
