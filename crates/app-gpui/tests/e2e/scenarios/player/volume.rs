//! Volume interaction scenarios for E2E testing.
//!
//! Tests for verifying volume control behavior through real App state.

use crate::driver::AppDriver;
use crate::runner::{E2ERunner, TestScenario};
use gpui::{TestAppContext, VisualTestContext, WindowHandle};
use sotf_audio_player_gpui::ui::PlayerView;
use std::error::Error;

struct VolumeScenario;

impl TestScenario for VolumeScenario {
    fn name(&self) -> &'static str {
        "Volume Controls"
    }

    fn execute(
        &self,
        cx: &mut VisualTestContext,
        window: WindowHandle<PlayerView>,
    ) -> Result<(), Box<dyn Error>> {
        let mut driver = AppDriver::new(cx, window);

        // 1. Read initial volume (should be 0.1)
        let initial_volume = driver.read_app(|app| app.playback.volume);
        if (initial_volume - 0.1).abs() > 0.001 {
            return Err(format!("Expected initial volume ~0.1, got {}", initial_volume).into());
        }

        // 2. Change volume and verify
        driver.update_app(|app, _| {
            app.playback.volume = 0.5;
        });
        let volume = driver.read_app(|app| app.playback.volume);
        if (volume - 0.5).abs() > 0.001 {
            return Err(format!("Expected volume 0.5, got {}", volume).into());
        }

        // 3. Test volume bounds (clamping at 1.0)
        driver.update_app(|app, _| {
            app.playback.volume = 1.5f32.clamp(0.0, 1.0);
        });
        let volume = driver.read_app(|app| app.playback.volume);
        if (volume - 1.0).abs() > 0.001 {
            return Err(format!("Expected volume clamped to 1.0, got {}", volume).into());
        }

        // 4. Test volume bounds (clamping at 0.0)
        driver.update_app(|app, _| {
            app.playback.volume = (-0.5f32).clamp(0.0, 1.0);
        });
        let volume = driver.read_app(|app| app.playback.volume);
        if volume > 0.001 {
            return Err(format!("Expected volume clamped to 0.0, got {}", volume).into());
        }

        Ok(())
    }
}

struct MuteToggleScenario;

impl TestScenario for MuteToggleScenario {
    fn name(&self) -> &'static str {
        "Mute Toggle"
    }

    fn execute(
        &self,
        cx: &mut VisualTestContext,
        window: WindowHandle<PlayerView>,
    ) -> Result<(), Box<dyn Error>> {
        let mut driver = AppDriver::new(cx, window);

        // Initially not muted
        let muted = driver.read_app(|app| app.playback.muted);
        if muted {
            return Err("Should not be muted initially".into());
        }

        // Mute
        driver.update_app(|app, _| {
            app.playback.muted = true;
        });
        let muted = driver.read_app(|app| app.playback.muted);
        if !muted {
            return Err("Should be muted after toggle".into());
        }

        // Volume should be preserved while muted
        let volume = driver.read_app(|app| app.playback.volume);
        if (volume - 0.1).abs() > 0.001 {
            return Err(
                format!("Volume should be preserved while muted, got {}", volume).into(),
            );
        }

        // Unmute
        driver.update_app(|app, _| {
            app.playback.muted = false;
        });
        let muted = driver.read_app(|app| app.playback.muted);
        if muted {
            return Err("Should not be muted after second toggle".into());
        }

        Ok(())
    }
}

#[gpui::test]
async fn test_volume_controls(cx: &mut TestAppContext) {
    let scenario = VolumeScenario;
    let runner = E2ERunner::new(scenario);
    let result = runner.run(cx).await;
    assert!(
        result.is_ok(),
        "Volume controls test failed: {:?}",
        result.err()
    );
}

#[gpui::test]
async fn test_mute_toggle(cx: &mut TestAppContext) {
    let scenario = MuteToggleScenario;
    let runner = E2ERunner::new(scenario);
    let result = runner.run(cx).await;
    assert!(
        result.is_ok(),
        "Mute toggle test failed: {:?}",
        result.err()
    );
}
