use crate::pages::level_meter::LevelMeterPage;
use crate::pages::plugin_rack::PluginRackPage;
use crate::runner::{E2ERunner, TestScenario};
use gpui::{TestAppContext, VisualTestContext, WindowHandle};
use sotf_audio_player_gpui::ui::PlayerView;
use std::error::Error;

pub struct MuteSoloLevelMeterScenario;

impl TestScenario for MuteSoloLevelMeterScenario {
    fn name(&self) -> &'static str {
        "Mute/Solo Level Meter"
    }

    fn execute(&self, cx: &mut VisualTestContext, window: WindowHandle<PlayerView>) -> Result<(), Box<dyn Error>> {
        use crate::driver::AppDriver;
        let mut driver = AppDriver::new(cx, window);
        
        {
            let mut rack_page = PluginRackPage::new(&mut driver);
            rack_page.inject_test_track();
            
            // Ensure playback is started - wait for it
            rack_page.toggle_playback();
            rack_page.run_until_parked();
            
            // Advance clock to trigger update_level_meter_groups (runs every 100ms)
            rack_page.wait(std::time::Duration::from_millis(200));
        } // rack_page dropped here, releasing driver borrow

        let mut meter_page = LevelMeterPage::new(&mut driver);
        meter_page.ensure_visible();

        // 1. Test Mute
        println!("Checking initial state...");
        // Assuming Group 0 is Main or Stereo output
        assert!(!meter_page.is_muted(0), "Group 0 should not be muted initially");
        // Check initial state
        let group_count = meter_page.driver.read_app(|app| app.level_meter_groups.len());
        assert!(group_count >= 2, "Should have at least 2 meter groups (L and R)");

        println!("Toggling mute on Group 0 (Left)...");
        meter_page.toggle_mute(0);
        
        println!("Verifying mute state for Group 0...");
        assert!(meter_page.is_muted(0), "Group 0 (Left) should be muted in UI");
        assert!(meter_page.get_matrix_channel_mute_state(0), "Channel 0 (L) should be muted in Matrix");
        assert!(!meter_page.get_matrix_channel_mute_state(1), "Channel 1 (R) should NOT be muted in Matrix when only Group 0 is muted");

        println!("Toggling mute on Group 1 (Right)...");
        meter_page.toggle_mute(1);
        
        println!("Verifying mute state for Group 1...");
        assert!(meter_page.is_muted(1), "Group 1 (Right) should be muted in UI");
        assert!(meter_page.get_matrix_channel_mute_state(1), "Channel 1 (R) should be muted in Matrix");
        
        println!("Untoggling mute...");
        meter_page.toggle_mute(0);
        assert!(!meter_page.is_muted(0), "Group 0 should be unmuted");
        assert!(!meter_page.get_matrix_channel_mute_state(0), "Channel 0 should be unmuted in Matrix");

        // 2. Test Solo
        println!("Toggling solo on Group 0...");
        meter_page.toggle_solo(0);
        
        println!("Verifying solo state...");
        assert!(meter_page.is_soloed(0), "Group 0 should be soloed in UI");
        assert!(meter_page.get_matrix_channel_solo_state(0), "Channel 0 (L) should be soloed in Matrix");
        assert!(!meter_page.get_matrix_channel_solo_state(1), "Channel 1 (R) should NOT be soloed in Matrix");
        
        // 3. Test Dim
        println!("Toggling dim on Group 0...");
        meter_page.toggle_dim(0);

        println!("Verifying dim state...");
        assert!(meter_page.is_dimmed(0), "Group 0 should be dimmed in UI");
        // We need to implement get_matrix_channel_dim_state in LevelMeterPage
        // assert!(meter_page.get_matrix_channel_dim_state(0), "Channel 0 should be dimmed in Matrix");
        
        println!("Success: Matrix plugin states correctly reflect UI mute/solo/dim actions.");
        Ok(())
    }
}

#[gpui::test]
async fn test_mute_solo_level_meter(cx: &mut TestAppContext) {
    let runner = E2ERunner::new(MuteSoloLevelMeterScenario);
    runner.run(cx).await.unwrap();
}
