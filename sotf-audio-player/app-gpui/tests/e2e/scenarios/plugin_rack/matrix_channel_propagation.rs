use crate::driver::AppDriver;
use crate::pages::plugin_rack::PluginRackPage;
use crate::runner::{TestScenario, E2ERunner};
use gpui::{TestAppContext, VisualTestContext, WindowHandle};
use sotf_audio::plugins::{PluginType, PluginSettings};
use sotf_audio_player_gpui::ui::PlayerView;
use std::error::Error;

pub struct MatrixPropagationScenario;

impl TestScenario for MatrixPropagationScenario {
    fn name(&self) -> &'static str {
        "Matrix Plugin Channel Propagation"
    }

    fn execute(&self, cx: &mut VisualTestContext, window: WindowHandle<PlayerView>) -> Result<(), Box<dyn Error>> {
        let mut driver = AppDriver::new(cx, window);
        let mut page = PluginRackPage::new(&mut driver);

        // 1. Add Upmixer (2 -> 6 channels)
        // Ensure starting state is clean
        while page.get_plugin_count() > 0 {
            page.remove_plugin(0);
        }

        let upmixer_id = page.add_plugin(PluginType::Upmixer);
        let upmixer_index = page.find_plugin_index_by_id(upmixer_id).unwrap();

        // 2. Add Matrix (Should be 6x6)
        let matrix_id = page.add_plugin(PluginType::Matrix);
        let matrix_index = page.find_plugin_index_by_id(matrix_id).unwrap();

        assert!(matrix_index > upmixer_index, "Matrix should be after Upmixer");

        // 3. Verify Matrix Channels
        // We need a helper in PluginRackPage to get Matrix dimensions or properties
        // Since we can't easily access the inner Plugin struct from page without helpers,
        // we might need to add `get_matrix_dimensions` to PluginRackPage OR use `read_app`.
        
        // For now, let will use driver.read_app inside the TestScenario (which has access to driver)
        // or add helper to page. Let's add helper to page if possible, but here we can use driver directly to assert.
        
        let (inputs, outputs) = page.get_matrix_channels(matrix_index);

        // EXPECTATION: 6 inputs (from Upmixer), 6 outputs (default pass-through/identity)
        assert_eq!(inputs, 6, "Matrix should have 6 inputs (inherited from Upmixer)");
        assert_eq!(outputs, 6, "Matrix should have 6 outputs (inherited from Upmixer)");

        Ok(())
    }
}

#[gpui::test]
async fn test_matrix_propagation(cx: &mut TestAppContext) {
    let runner = E2ERunner::new(MatrixPropagationScenario);
    runner.run(cx).await.unwrap();
}
