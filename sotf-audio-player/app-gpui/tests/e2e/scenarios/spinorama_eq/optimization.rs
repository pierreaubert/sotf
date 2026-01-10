use crate::runner::E2ERunner;
use crate::runner::TestScenario;
use crate::driver::AppDriver;
use crate::pages::spinorama::SpinoramaPage;
use gpui::{VisualTestContext, WindowHandle};
use sotf_audio_player_gpui::ui::PlayerView;
use sotf_audio_player_gpui::app::Screen;
use std::error::Error;

pub struct SpinoramaOptimizationScenario;

impl TestScenario for SpinoramaOptimizationScenario {
    fn name(&self) -> &'static str { "Spinorama Speaker Optimization" }
    
    fn setup(&mut self, _cx: &mut gpui::TestAppContext) -> Result<(), Box<dyn Error>> {
        Ok(())
    }

    fn execute(&self, cx: &mut VisualTestContext, view: WindowHandle<PlayerView>) -> Result<(), Box<dyn Error>> {
        let mut driver = AppDriver::new(cx, view);

        // 1. Navigate to Spinorama EQ
        driver.navigate_to(Screen::Spinorama);
        
        // 2. Interact with Spinorama Page
        let mut page = SpinoramaPage::new(&mut driver);
        
        // Use Mock Data to bypass network and ensure stability
        page.inject_mock_data();
        
        // Configure
        page.set_optimization_params(10, 50); // Low iterations for test speed

        // Optimize
        page.start_optimization();
        
        // Wait for completion
        page.wait_for_optimization_completion()?;
        
        Ok(())
    }

    fn teardown(&mut self, _cx: &mut gpui::TestAppContext) -> Result<(), Box<dyn Error>> {
        Ok(())
    }
}

#[gpui::test]
async fn test_spinorama_optimization(cx: &mut gpui::TestAppContext) {
    let scenario = SpinoramaOptimizationScenario;
    let runner = E2ERunner::new(scenario);
    let result = runner.run(cx).await;
    
    if let Err(e) = &result {
        println!("Test failed: {}", e);
    }
    assert!(result.is_ok());
}
