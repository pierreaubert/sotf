use crate::driver::AppDriver;
use crate::pages::plugin_rack::PluginRackPage;
use crate::runner::{E2ERunner, TestScenario};
use gpui::{TestAppContext, VisualTestContext, WindowHandle};
use sotf_audio_player_gpui::app::Screen;
use sotf_audio_player_gpui::ui::PlayerView;
use std::error::Error;

pub struct ViewSwitchingStabilityScenario;

fn stop_test_playback(driver: &mut AppDriver<'_>) {
    let _ = driver.view.update(driver.cx, |view, _, cx| {
        view.state.update(cx, |state, _cx| {
            let _ = state.player.stop();
            state.app.playback.is_playing = false;
            state.app.playback.current_queue_index = None;
        });
        cx.notify();
    });
    driver.run_until_parked();
}

impl TestScenario for ViewSwitchingStabilityScenario {
    fn name(&self) -> &'static str {
        "View Switching Playback Stability"
    }

    fn execute(
        &self,
        cx: &mut VisualTestContext,
        window: WindowHandle<PlayerView>,
    ) -> Result<(), Box<dyn Error>> {
        let mut driver = AppDriver::new(cx, window);

        // 1. Seed a deterministic playback state. This test is about view
        // switching preserving playback UI state; real CoreAudio playback is
        // covered elsewhere and can leave platform handles alive in GPUI e2e.
        {
            let mut rack_page = PluginRackPage::new(&mut driver);
            rack_page.inject_test_track();
        }

        driver.update_app(|app, _| {
            app.playback.current_queue_index = Some(0);
            app.playback.is_playing = true;
            app.playback.position_secs = 0.25;
            app.playback.duration_secs = 30.0;
        });
        driver.run_until_parked();

        let initial_position = driver.read_app(|app| app.playback.position_secs);
        println!("Initial position: {:.3}s", initial_position);

        // 2. Switch to Studio Rack
        println!("Switching to Studio Rack...");
        driver.navigate_to(Screen::Studio);

        driver.update_app(|app, _| {
            app.playback.position_secs += 0.25;
        });
        driver.run_until_parked();

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

        driver.update_app(|app, _| {
            app.playback.position_secs += 0.25;
        });
        driver.run_until_parked();

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

        stop_test_playback(&mut driver);

        Ok(())
    }
}

#[gpui::test]
async fn test_view_switching_playback_stability(cx: &mut TestAppContext) {
    let runner = E2ERunner::new(ViewSwitchingStabilityScenario);
    runner.run(cx).await.unwrap();
}
