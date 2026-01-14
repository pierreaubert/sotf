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

        // 1. Initialize and start playback
        {
            let mut rack_page = PluginRackPage::new(&mut driver);
            // Inject track to ensure we have something to play
            rack_page.inject_test_track();

            println!("Starting playback...");
            // Use start_playback_from_queue instead of toggle_playback
            // toggle_playback only pauses/resumes, it doesn't load and start playing
            rack_page.start_playback_from_queue();
            driver.run_until_parked();
            driver.run_until_parked();
        }

        // Verify initial playing state
        let mut started = false;
        for _ in 0..20 {
            // 20 iterations * 50ms = 1 second timeout
            driver.run_until_parked();
            driver
                .cx
                .executor()
                .advance_clock(std::time::Duration::from_millis(50));
            if driver.read_app(|app| app.playback.is_playing) {
                started = true;
                break;
            }
        }
        assert!(
            started,
            "Playback should be started within 1 second timeout"
        );
        let initial_position = driver.read_app(|app| app.playback.position_secs);

        println!("Initial position: {:.3}s", initial_position);

        // 2. Switch to Studio Rack (Screen::Studio)
        println!("Switching to Studio Rack...");
        driver.navigate_to(Screen::Studio);

        // Wait for some time to simulate user activity and allow audio engine to process
        // We need to wait long enough for position to advance significantly
        let wait_duration = Duration::from_millis(500);
        driver.cx.executor().advance_clock(wait_duration);
        driver.run_until_parked();

        // 3. Verify Playback Stability
        let studio_position = driver.read_app(|app| app.playback.position_secs);
        let is_playing_studio = driver.read_app(|app| app.playback.is_playing);

        println!("Position in Studio Rack: {:.3}s", studio_position);

        assert!(is_playing_studio, "Playback should continue in Studio Rack");
        assert!(
            studio_position > initial_position + 0.3,
            "Position should have advanced by at least 0.3s (expected ~0.5s). Got start={:.3}, current={:.3}",
            initial_position,
            studio_position
        );

        // 4. Switch to Queue (Screen::Queue)
        println!("Switching to Queue...");
        driver.navigate_to(Screen::Queue);

        driver.cx.executor().advance_clock(wait_duration);
        driver.run_until_parked();

        // 5. Verify Playback Stability again
        let queue_position = driver.read_app(|app| app.playback.position_secs);
        let is_playing_queue = driver.read_app(|app| app.playback.is_playing);

        println!("Position in Queue: {:.3}s", queue_position);

        assert!(is_playing_queue, "Playback should continue in Queue");
        assert!(
            queue_position > studio_position + 0.3,
            "Position should have advanced further in Queue. Got studio={:.3}, current={:.3}",
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
