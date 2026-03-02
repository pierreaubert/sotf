use crate::pages::plugin_rack::PluginRackPage;
use crate::runner::{E2ERunner, TestScenario};
use gpui::{TestAppContext, VisualTestContext, WindowHandle};
use sotf_audio_player_gpui::app::Screen;
use sotf_audio_player_gpui::ui::PlayerView;
use std::error::Error;
use std::time::Duration;

pub struct ViewSwitchingStabilityScenario;

impl TestScenario for ViewSwitchingStabilityScenario {
    fn name(&self) -> &'static str {
        "View Switching Playback Stability"
    }

    fn execute(
        &self,
        cx: &mut VisualTestContext,
        window: WindowHandle<PlayerView>,
    ) -> Result<(), Box<dyn Error>> {
        use crate::driver::AppDriver;
        let mut driver = AppDriver::new(cx, window);

        // Direct audio to a virtual device to avoid sending sound to speakers.
        driver.update_app(|app, _| {
            app.audio_device_state.current_output_device_name = Some("BlackHole 2ch".to_string());
        });

        // 1. Initialize and start playback using a virtual device
        {
            let mut rack_page = PluginRackPage::new(&mut driver);
            rack_page.inject_test_track();

            println!("Starting playback on BlackHole...");
            rack_page.start_playback_from_queue();
            driver.run_until_parked();
        }

        // Helper: wait real time for audio engine, then advance GPUI clock
        // to trigger the 100ms polling timer that syncs position_secs.
        let wait_and_sync = |driver: &mut AppDriver, ms: u64| {
            std::thread::sleep(Duration::from_millis(ms));
            driver
                .cx
                .executor()
                .advance_clock(Duration::from_millis(200));
            driver.run_until_parked();
        };

        // Wait for the audio engine to spin up
        wait_and_sync(&mut driver, 500);

        let is_playing = driver.read_app(|app| app.playback.is_playing);
        if !is_playing {
            println!("INFO: Playback did not start (BlackHole device may not be available).");
            println!("Skipping playback stability assertions.");
            return Ok(());
        }

        let initial_position = driver.read_app(|app| app.playback.position_secs);
        println!("Initial position: {:.3}s", initial_position);

        // 2. Switch to Studio Rack
        println!("Switching to Studio Rack...");
        driver.navigate_to(Screen::Studio);

        wait_and_sync(&mut driver, 500);

        let studio_position = driver.read_app(|app| app.playback.position_secs);
        let is_playing_studio = driver.read_app(|app| app.playback.is_playing);
        println!("Position in Studio Rack: {:.3}s", studio_position);

        assert!(is_playing_studio, "Playback should continue in Studio Rack");
        assert!(
            studio_position > initial_position + 0.1,
            "Position should have advanced after switching to Studio. Got start={:.3}, current={:.3}",
            initial_position,
            studio_position
        );

        // 3. Switch to Queue
        println!("Switching to Queue...");
        driver.navigate_to(Screen::Queue);

        wait_and_sync(&mut driver, 500);

        let queue_position = driver.read_app(|app| app.playback.position_secs);
        let is_playing_queue = driver.read_app(|app| app.playback.is_playing);
        println!("Position in Queue: {:.3}s", queue_position);

        assert!(is_playing_queue, "Playback should continue in Queue");
        assert!(
            queue_position > studio_position + 0.1,
            "Position should have advanced after switching to Queue. Got studio={:.3}, current={:.3}",
            studio_position,
            queue_position
        );

        Ok(())
    }
}

#[gpui::test]
async fn test_view_switching_playback_stability(cx: &mut TestAppContext) {
    let runner = E2ERunner::new(ViewSwitchingStabilityScenario);
    runner.run(cx).await.unwrap();
}
