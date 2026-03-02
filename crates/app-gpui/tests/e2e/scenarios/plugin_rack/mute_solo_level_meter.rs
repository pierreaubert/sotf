use crate::pages::level_meter::LevelMeterPage;
use crate::runner::{E2ERunner, TestScenario};
use gpui::{TestAppContext, VisualTestContext, WindowHandle};
use sotf_audio_player_gpui::ui::PlayerView;
use std::error::Error;

pub struct MuteSoloLevelMeterScenario;

impl TestScenario for MuteSoloLevelMeterScenario {
    fn name(&self) -> &'static str {
        "Mute/Solo Level Meter"
    }

    fn execute(
        &self,
        cx: &mut VisualTestContext,
        window: WindowHandle<PlayerView>,
    ) -> Result<(), Box<dyn Error>> {
        use crate::driver::AppDriver;
        let mut driver = AppDriver::new(cx, window);

        // No playback needed: App::new() already initializes default stereo meter groups.
        let mut meter_page = LevelMeterPage::new(&mut driver);
        meter_page.ensure_visible();

        // Stereo layout has 1 group "L/R" containing channels 0 (L) and 1 (R).
        println!("Checking initial state...");
        let group_count = meter_page
            .driver
            .read_app(|app| app.level_meter_groups.len());
        assert!(
            group_count >= 1,
            "Should have at least 1 meter group (L/R). Got {}",
            group_count
        );
        let channel_count = meter_page.driver.read_app(|app| {
            app.level_meter_groups
                .first()
                .map(|g| g.channels.len())
                .unwrap_or(0)
        });
        assert!(
            channel_count >= 2,
            "L/R group should have at least 2 channels. Got {}",
            channel_count
        );
        assert!(
            !meter_page.is_muted(0),
            "Group 0 should not be muted initially"
        );

        // 1. Test Mute on L/R group — mutes both L and R in matrix
        println!("Toggling mute on Group 0 (L/R)...");
        meter_page.toggle_mute(0);

        println!("Verifying mute state...");
        assert!(
            meter_page.is_muted(0),
            "Group 0 (L/R) should be muted in UI"
        );
        assert!(
            meter_page.get_matrix_channel_mute_state(0),
            "Channel 0 (L) should be muted in Matrix"
        );
        assert!(
            meter_page.get_matrix_channel_mute_state(1),
            "Channel 1 (R) should be muted in Matrix (same group)"
        );

        println!("Untoggling mute...");
        meter_page.toggle_mute(0);
        assert!(!meter_page.is_muted(0), "Group 0 should be unmuted");
        assert!(
            !meter_page.get_matrix_channel_mute_state(0),
            "Channel 0 should be unmuted in Matrix"
        );
        assert!(
            !meter_page.get_matrix_channel_mute_state(1),
            "Channel 1 should be unmuted in Matrix"
        );

        // 2. Test Solo
        println!("Toggling solo on Group 0 (L/R)...");
        meter_page.toggle_solo(0);

        println!("Verifying solo state...");
        assert!(meter_page.is_soloed(0), "Group 0 should be soloed in UI");
        assert!(
            meter_page.get_matrix_channel_solo_state(0),
            "Channel 0 (L) should be soloed in Matrix"
        );
        assert!(
            meter_page.get_matrix_channel_solo_state(1),
            "Channel 1 (R) should be soloed in Matrix (same group)"
        );

        // Unsolo
        meter_page.toggle_solo(0);
        assert!(!meter_page.is_soloed(0), "Group 0 should be unsoloed");

        // 3. Test Dim
        println!("Toggling dim on Group 0 (L/R)...");
        meter_page.toggle_dim(0);

        println!("Verifying dim state...");
        assert!(meter_page.is_dimmed(0), "Group 0 should be dimmed in UI");

        println!("Success: Matrix plugin states correctly reflect UI mute/solo/dim actions.");
        Ok(())
    }
}

#[gpui::test]
async fn test_mute_solo_level_meter(cx: &mut TestAppContext) {
    let runner = E2ERunner::new(MuteSoloLevelMeterScenario);
    runner.run(cx).await.unwrap();
}
